// Data-driven construction of ADC-backed sensor chains from a JSON config file -- see
// DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md. Replaces the hand-written chain literals that used
// to live in main.rs's add_adc_sensor_chains/setup_button_sensors: tuning a debounce count,
// scale factor, or warning threshold is now an edit to sensor_config.json, picked up on the
// next restart (see util::shutdown::watch_for_updates) instead of a rebuild.
//
// The resistive senders (coolant temp, oil pressure, fuel level) are the `calibrated_analog`
// kind here -- a datasheet resistance curve plus the live 12V supply reading, see
// CalibratedVariableResistanceAnalogSensor and SENSOR_CALIBRATION_DESIGN.md.
//
// Out of scope (see design doc's Scope section): the pulse-period sensors (SpeedSensor,
// TachoSensor) and non-ADC providers (GNSS/UPS/BNO085/link-health) stay hand-built in
// main.rs.

use crate::hardware::analog_signal_processing::{
    AnalogSignalProcessor, AnalogSignalProcessorDampener, AnalogSignalProcessorMovingAverage,
};
use crate::hardware::digital_signal_processing::{DigitalSignalDebouncer, DigitalSignalProcessor};
use crate::hardware::hw_providers::{ADCChannelProvider, HWInput, OneWireTempChannelProvider};
use crate::hardware::sensor_manager::{SensorAnalogInputChain, SensorDigitalInputChain, SensorManager};
use crate::hardware::sensor_value::ValueConstraints;
use crate::hardware::sensors::{
    CalibratedVariableResistanceAnalogSensor, GenericAnalogSensor, GenericDigitalSensor,
    OneWireTempSensor, SupplyVoltagePublisher, VoltageDividerSensor, NOMINAL_SUPPLY_V,
};
use crate::util::adc_data_provider::{ADCFrame, AdcTempFrame};

use rppal::gpio::Level;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize)]
struct ChainConfig {
    /// Which caller this entry belongs to -- "sensor" for add_adc_sensor_chains'
    /// dashboard-facing chains, "button" for setup_button_sensors' physical MFD buttons.
    /// Both live in one file (see design doc's Open questions); `load_chains` filters by
    /// this field instead of splitting into two files with near-identical schemas.
    group: String,
    hw_input: String,
    provider: String,
    /// Required for `provider: "adc_temp"` — the 16-char lowercase hex DS18B20 ROM address
    /// this chain reads from the one-wire bus. Ignored by every other provider.
    #[serde(default)]
    rom: Option<String>,
    #[serde(default)]
    digital_processors: Vec<DigitalProcessorConfig>,
    #[serde(default)]
    analog_processors: Vec<AnalogProcessorConfig>,
    sensor: SensorConfig,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum DigitalProcessorConfig {
    Debounce { stable_count: u8, stable_delay_ms: u64 },
}

impl DigitalProcessorConfig {
    fn build(&self) -> Box<dyn DigitalSignalProcessor + Send> {
        match self {
            DigitalProcessorConfig::Debounce { stable_count, stable_delay_ms } => {
                Box::new(DigitalSignalDebouncer::new(*stable_count, Duration::from_millis(*stable_delay_ms)))
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnalogProcessorConfig {
    MovingAverage { window: usize },
    Dampener { alpha: f32 },
}

impl AnalogProcessorConfig {
    fn build(&self) -> Box<dyn AnalogSignalProcessor + Send> {
        match self {
            AnalogProcessorConfig::MovingAverage { window } => Box::new(AnalogSignalProcessorMovingAverage::new(*window)),
            AnalogProcessorConfig::Dampener { alpha } => Box::new(AnalogSignalProcessorDampener::new(*alpha)),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SensorConfig {
    GenericDigital {
        id: String,
        name: String,
        active_level: ActiveLevelConfig,
        constraints: ConstraintsConfig,
    },
    GenericAnalog {
        id: String,
        name: String,
        units: String,
        scale: f32,
        constraints: ConstraintsConfig,
    },
    /// DS18B20 one-wire temperature (see OneWireTempSensor). Paired with
    /// `provider: "adc_temp"` and a top-level `rom`. Unit is always °C and the raw→°C
    /// divisor is intrinsic, so neither `units` nor `scale` is carried here.
    OneWireTemp {
        id: String,
        name: String,
        constraints: ConstraintsConfig,
    },
    /// Direct resistive-divider voltage tap (see VoltageDividerSensor), for the `Hw12v`
    /// 12V-system channel. The divider ratio and ADC reference are fixed PCB constants in
    /// VoltageDividerSensor, not config; `trim` (optional, default 1.0) is the only
    /// field-adjustable knob -- a multiplicative bench correction.
    VoltageDividerAnalog {
        id: String,
        name: String,
        units: String,
        #[serde(default = "default_trim")]
        trim: f32,
        constraints: ConstraintsConfig,
    },
    /// Resistive sender (oil pressure / fuel level / coolant temp) converted through a
    /// datasheet resistance curve and the live 12 V supply reading -- see
    /// CalibratedVariableResistanceAnalogSensor and SENSOR_CALIBRATION_DESIGN.md. Paired
    /// with `provider: "adc"`. `curve` is `(ohm, value)` points (>= 2, sorted by `ohm` on
    /// load); `value_offset` (optional, default 0.0) is reserved for the field-calibration
    /// overlay that doesn't exist yet.
    CalibratedAnalog {
        id: String,
        name: String,
        units: String,
        r_series_ohm: f32,
        curve: Vec<CurvePointConfig>,
        #[serde(default)]
        value_offset: f32,
        constraints: ConstraintsConfig,
    },
}

#[derive(Deserialize)]
struct CurvePointConfig {
    ohm: f32,
    value: f32,
}

fn default_trim() -> f32 {
    1.0
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ActiveLevelConfig {
    High,
    Low,
}

impl ActiveLevelConfig {
    fn as_level(&self) -> Level {
        match self {
            ActiveLevelConfig::High => Level::High,
            ActiveLevelConfig::Low => Level::Low,
        }
    }
}

/// Either the raw six `ValueConstraints` fields, or a `digital_preset` shorthand for the
/// common digital-indicator case (see `ValueConstraints::digital_default/warning/critical`).
/// `#[serde(untagged)]` picks between them by which keys are present in the JSON object.
#[derive(Deserialize)]
#[serde(untagged)]
enum ConstraintsConfig {
    Preset {
        digital_preset: String,
    },
    Raw {
        min: f32,
        max: f32,
        #[serde(default)]
        critical_low: Option<f32>,
        #[serde(default)]
        warning_low: Option<f32>,
        #[serde(default)]
        warning_high: Option<f32>,
        #[serde(default)]
        critical_high: Option<f32>,
    },
}

impl ConstraintsConfig {
    fn build(&self, hw_input: &str) -> Result<ValueConstraints, String> {
        match self {
            ConstraintsConfig::Preset { digital_preset } => match digital_preset.as_str() {
                "default" => Ok(ValueConstraints::digital_default()),
                "warning" => Ok(ValueConstraints::digital_warning()),
                "critical" => Ok(ValueConstraints::digital_critical()),
                other => Err(format!(
                    "sensor config: hw_input '{hw_input}' has unknown digital_preset '{other}' (expected default/warning/critical)"
                )),
            },
            ConstraintsConfig::Raw { min, max, critical_low, warning_low, warning_high, critical_high } => {
                Ok(ValueConstraints::analog_with_thresholds(*min, *max, *critical_low, *warning_low, *warning_high, *critical_high))
            }
        }
    }
}

/// Runtime-loaded, not `include_str!`'d -- tuning takes effect on the next restart, which
/// util::shutdown::watch_for_updates triggers automatically when this file changes.
/// Lives alongside Cargo.toml in the niva_dashboard crate dir (systemd's WorkingDirectory,
/// and where `cargo run`/`cargo build` are invoked from) -- same HOME-based construction as
/// State/config.json (see util::config), one path segment further in, and outside State/
/// since this file is user-authored input, not app-written runtime state.
pub fn default_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
    PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard/sensor_config.json"))
}

/// Loads `path`, builds every entry tagged with `group`, and adds it to `mgr`. Fail-fast:
/// a missing file, malformed JSON, unknown `hw_input`, unsupported `provider`, or a chain
/// mixing digital/analog processors with the wrong sensor kind is a `Err` naming the
/// problem -- callers are expected to treat this like a build-time error (see design doc's
/// Open questions), not skip the one bad entry and carry on.
///
/// `temp_frame` backs `provider: "adc_temp"` entries (one-wire DS18B20 temperatures, see
/// ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md). Pass `None` for a group that has none — an
/// `adc_temp` entry with no frame available is itself a fail-fast load error.
pub fn load_chains(
    path: &Path,
    group: &str,
    frame: ADCFrame,
    temp_frame: Option<AdcTempFrame>,
    mgr: &mut SensorManager,
) -> Result<(), String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("sensor config: failed to read {path:?}: {e}"))?;
    let entries: Vec<ChainConfig> = serde_json::from_str(&contents)
        .map_err(|e| format!("sensor config: failed to parse {path:?}: {e}"))?;

    // One shared cell per load, carrying the live `Hw12v` reading (f32 bits) that every
    // `calibrated_analog` sensor's raw→Ω conversion needs. Seeded with a nominal until the
    // `Hw12v` chain's `SupplyVoltagePublisher` writes a real reading. See
    // SENSOR_CALIBRATION_DESIGN.md, "Cross-sensor dependency".
    let v_supply = Arc::new(AtomicU32::new(NOMINAL_SUPPLY_V.to_bits()));

    for entry in entries.iter().filter(|e| e.group == group) {
        build_chain(entry, frame.clone(), temp_frame.as_ref(), &v_supply, mgr)?;
    }
    Ok(())
}

fn build_chain(
    entry: &ChainConfig,
    frame: ADCFrame,
    temp_frame: Option<&AdcTempFrame>,
    v_supply: &Arc<AtomicU32>,
    mgr: &mut SensorManager,
) -> Result<(), String> {
    let input = HWInput::from_config_name(&entry.hw_input)
        .ok_or_else(|| format!("sensor config: unknown hw_input '{}'", entry.hw_input))?;

    match entry.provider.as_str() {
        "adc" => build_adc_chain(entry, input, frame, v_supply, mgr),
        "adc_temp" => build_adc_temp_chain(entry, input, temp_frame, mgr),
        other => Err(format!(
            "sensor config: hw_input '{}' has unsupported provider '{}' (expected \"adc\" or \"adc_temp\")",
            entry.hw_input, other
        )),
    }
}

fn build_adc_chain(
    entry: &ChainConfig,
    input: HWInput,
    frame: ADCFrame,
    v_supply: &Arc<AtomicU32>,
    mgr: &mut SensorManager,
) -> Result<(), String> {
    match &entry.sensor {
        SensorConfig::GenericDigital { id, name, active_level, constraints } => {
            if !entry.analog_processors.is_empty() {
                return Err(format!(
                    "sensor config: hw_input '{}' is a digital sensor but lists analog_processors",
                    entry.hw_input
                ));
            }
            let processors: Vec<Box<dyn DigitalSignalProcessor + Send>> =
                entry.digital_processors.iter().map(DigitalProcessorConfig::build).collect();
            let chain = SensorDigitalInputChain::new(
                Box::new(ADCChannelProvider::new(input, frame)),
                processors,
                Box::new(GenericDigitalSensor::new(
                    id.clone(),
                    name.clone(),
                    active_level.as_level(),
                    constraints.build(&entry.hw_input)?,
                )),
            );
            mgr.add_digital_sensor_chain(chain);
        }
        SensorConfig::GenericAnalog { id, name, units, scale, constraints } => {
            if !entry.digital_processors.is_empty() {
                return Err(format!(
                    "sensor config: hw_input '{}' is an analog sensor but lists digital_processors",
                    entry.hw_input
                ));
            }
            let processors: Vec<Box<dyn AnalogSignalProcessor + Send>> =
                entry.analog_processors.iter().map(AnalogProcessorConfig::build).collect();
            let chain = SensorAnalogInputChain::new(
                Box::new(ADCChannelProvider::new(input, frame)),
                processors,
                Box::new(GenericAnalogSensor::new(id.clone(), name.clone(), units.clone(), constraints.build(&entry.hw_input)?, *scale)),
            );
            mgr.add_analog_sensor_chain(chain);
        }
        SensorConfig::VoltageDividerAnalog { id, name, units, trim, constraints } => {
            if !entry.digital_processors.is_empty() {
                return Err(format!(
                    "sensor config: hw_input '{}' is an analog sensor but lists digital_processors",
                    entry.hw_input
                ));
            }
            let processors: Vec<Box<dyn AnalogSignalProcessor + Send>> =
                entry.analog_processors.iter().map(AnalogProcessorConfig::build).collect();
            // This kind is the system supply voltage, so its chain's sensor is wrapped to
            // publish each reading into the shared cell the calibrated senders consume.
            let chain = SensorAnalogInputChain::new(
                Box::new(ADCChannelProvider::new(input, frame)),
                processors,
                Box::new(SupplyVoltagePublisher::new(
                    Box::new(VoltageDividerSensor::new(
                        id.clone(), name.clone(), units.clone(),
                        constraints.build(&entry.hw_input)?, *trim,
                    )),
                    v_supply.clone(),
                )),
            );
            mgr.add_analog_sensor_chain(chain);
        }
        SensorConfig::CalibratedAnalog { id, name, units, r_series_ohm, curve, value_offset, constraints } => {
            if !entry.digital_processors.is_empty() {
                return Err(format!(
                    "sensor config: hw_input '{}' is an analog sensor but lists digital_processors",
                    entry.hw_input
                ));
            }
            if curve.len() < 2 {
                return Err(format!(
                    "sensor config: hw_input '{}' calibrated_analog curve needs at least 2 points, has {}",
                    entry.hw_input, curve.len()
                ));
            }
            let mut points: Vec<(f32, f32)> = curve.iter().map(|p| (p.ohm, p.value)).collect();
            if points.iter().any(|(o, v)| !o.is_finite() || !v.is_finite()) {
                return Err(format!(
                    "sensor config: hw_input '{}' calibrated_analog curve has a non-finite ohm/value",
                    entry.hw_input
                ));
            }
            points.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("curve ohm values are finite"));
            let processors: Vec<Box<dyn AnalogSignalProcessor + Send>> =
                entry.analog_processors.iter().map(AnalogProcessorConfig::build).collect();
            let chain = SensorAnalogInputChain::new(
                Box::new(ADCChannelProvider::new(input, frame)),
                processors,
                Box::new(CalibratedVariableResistanceAnalogSensor::new(
                    id.clone(), name.clone(), units.clone(), *r_series_ohm,
                    points, *value_offset, constraints.build(&entry.hw_input)?,
                    v_supply.clone(),
                )),
            );
            mgr.add_analog_sensor_chain(chain);
        }
        SensorConfig::OneWireTemp { .. } => {
            return Err(format!(
                "sensor config: hw_input '{}' has sensor kind \"one_wire_temp\" but provider is not \"adc_temp\"",
                entry.hw_input
            ));
        }
    }
    Ok(())
}

fn build_adc_temp_chain(
    entry: &ChainConfig,
    input: HWInput,
    temp_frame: Option<&AdcTempFrame>,
    mgr: &mut SensorManager,
) -> Result<(), String> {
    let temp_frame = temp_frame.ok_or_else(|| format!(
        "sensor config: hw_input '{}' uses provider \"adc_temp\" but no one-wire temperature frame is available",
        entry.hw_input
    ))?;

    let (id, name, constraints) = match &entry.sensor {
        SensorConfig::OneWireTemp { id, name, constraints } => (id, name, constraints),
        _ => return Err(format!(
            "sensor config: hw_input '{}' uses provider \"adc_temp\" but sensor kind is not \"one_wire_temp\"",
            entry.hw_input
        )),
    };

    let rom = entry.rom.as_deref().ok_or_else(|| format!(
        "sensor config: hw_input '{}' uses provider \"adc_temp\" but has no \"rom\" address",
        entry.hw_input
    ))?;
    if rom.len() != 16 || !rom.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "sensor config: hw_input '{}' has malformed rom '{}' (expected 16 hex characters)",
            entry.hw_input, rom
        ));
    }

    if !entry.digital_processors.is_empty() {
        return Err(format!(
            "sensor config: hw_input '{}' is a temperature (analog) sensor but lists digital_processors",
            entry.hw_input
        ));
    }
    let processors: Vec<Box<dyn AnalogSignalProcessor + Send>> =
        entry.analog_processors.iter().map(AnalogProcessorConfig::build).collect();
    let chain = SensorAnalogInputChain::new(
        Box::new(OneWireTempChannelProvider::new(input, rom, temp_frame.clone())),
        processors,
        Box::new(OneWireTempSensor::new(id.clone(), name.clone(), constraints.build(&entry.hw_input)?)),
    );
    mgr.add_analog_sensor_chain(chain);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::adc_data_provider::TestADCDataProvider;

    fn write_temp_config(json: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "niva_sensor_config_test_{}_{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, json).unwrap();
        path
    }

    #[test]
    fn loads_only_the_requested_group() {
        let json = r#"[
            {"group":"sensor","hw_input":"HwParkBrake","provider":"adc",
             "digital_processors":[{"type":"debounce","stable_count":5,"stable_delay_ms":50}],
             "sensor":{"kind":"generic_digital","id":"HwParkBrake","name":"test","active_level":"high","constraints":{"digital_preset":"warning"}}},
            {"group":"button","hw_input":"HwButton0","provider":"adc",
             "sensor":{"kind":"generic_digital","id":"HwButton0","name":"HwButton0","active_level":"high","constraints":{"digital_preset":"default"}}}
        ]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        // TestADCDataProvider's background thread needs a moment to write its first
        // synthetic frame -- reading before that gives "channel not in frame" errors.
        std::thread::sleep(Duration::from_millis(50));

        let mut sensor_mgr = SensorManager::new();
        load_chains(&path, "sensor", frame.clone(), None, &mut sensor_mgr).expect("sensor group should load");
        sensor_mgr.read_all_sensors().ok();
        assert!(sensor_mgr.get_sensor_value(&HWInput::HwParkBrake).is_some());
        assert!(sensor_mgr.get_sensor_value(&HWInput::HwButton0).is_none());

        // TestADCDataProvider's synthetic frame only carries channels 0-15 (see its
        // generate_channels) -- button channels (16-23) are never populated by it, so a
        // successful *load* (not a successful *read*) is what this asserts: the read
        // error naming channel 16 confirms load_chains actually built a chain wired to
        // HwButton0's mapped ADC channel, not that the value came back populated.
        let mut button_mgr = SensorManager::new();
        load_chains(&path, "button", frame, None, &mut button_mgr).expect("button group should load");
        let err = button_mgr.read_all_sensors().expect_err("button channel 16 isn't in the self-test frame");
        assert!(err.contains("16"), "expected error naming ADC channel 16, got: {err}");
        assert!(button_mgr.get_sensor_value(&HWInput::HwParkBrake).is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn analog_scale_and_thresholds_round_trip_into_the_sensor_value() {
        let json = r#"[
            {"group":"sensor","hw_input":"HwFuelLvl","provider":"adc",
             "analog_processors":[{"type":"moving_average","window":1}],
             "sensor":{"kind":"generic_analog","id":"HwFuelLvl","name":"test","units":"%","scale":0.1,
               "constraints":{"min":0.0,"max":100.0,"critical_low":10.0,"warning_low":20.0}}}
        ]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        std::thread::sleep(Duration::from_millis(50));
        let mut mgr = SensorManager::new();
        load_chains(&path, "sensor", frame, None, &mut mgr).expect("should load");
        mgr.read_all_sensors().ok();
        let value = mgr.get_sensor_value(&HWInput::HwFuelLvl).expect("HwFuelLvl should have a value");
        assert_eq!(value.constraints.max_value, 100.0);
        assert_eq!(value.constraints.critical_low, Some(10.0));
        assert_eq!(value.constraints.warning_low, Some(20.0));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn voltage_divider_kind_loads_with_default_trim_and_produces_a_bounded_value() {
        let json = r#"[
            {"group":"sensor","hw_input":"Hw12v","provider":"adc",
             "analog_processors":[{"type":"moving_average","window":1}],
             "sensor":{"kind":"voltage_divider_analog","id":"Hw12v","name":"БОРТ СЕТЬ","units":"В",
               "constraints":{"min":0.0,"max":20.0,"critical_low":11.0,"warning_high":14.7}}}
        ]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        std::thread::sleep(Duration::from_millis(50));
        let mut mgr = SensorManager::new();
        load_chains(&path, "sensor", frame, None, &mut mgr).expect("should load without an explicit trim");
        mgr.read_all_sensors().ok();
        let value = mgr.get_sensor_value(&HWInput::Hw12v).expect("Hw12v should have a value");
        assert_eq!(value.constraints.max_value, 20.0);
        assert_eq!(value.constraints.warning_high, Some(14.7));
        let volts = value.as_f32();
        assert!((0.0..=20.0).contains(&volts), "reading should be within the divider's clamped range, got {volts}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn calibrated_analog_kind_loads_and_round_trips_constraints() {
        let json = r#"[
            {"group":"sensor","hw_input":"Hw12v","provider":"adc",
             "analog_processors":[{"type":"moving_average","window":1}],
             "sensor":{"kind":"voltage_divider_analog","id":"Hw12v","name":"БОРТ СЕТЬ","units":"В",
               "constraints":{"min":0.0,"max":20.0}}},
            {"group":"sensor","hw_input":"HwOilPress","provider":"adc",
             "analog_processors":[{"type":"moving_average","window":1}],
             "sensor":{"kind":"calibrated_analog","id":"HwOilPress","name":"ДАВЛ МАСЛА","units":"кгс/см²",
               "r_series_ohm":130.8,
               "curve":[{"ohm":305.0,"value":0.0},{"ohm":7.5,"value":8.0},{"ohm":118.0,"value":4.0}],
               "constraints":{"min":0.0,"max":8.0,"critical_low":0.5,"warning_low":1.0}}}
        ]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        std::thread::sleep(Duration::from_millis(50));
        let mut mgr = SensorManager::new();
        load_chains(&path, "sensor", frame, None, &mut mgr).expect("should load (curve accepted out of order)");
        mgr.read_all_sensors().ok();
        // The self-test frame can drive this chain into its fault branch on some ticks
        // (V_sensor_wire >= supply), so a value may legitimately be absent; when present it
        // must be within the constrained range and carry the configured thresholds.
        if let Some(v) = mgr.get_sensor_value(&HWInput::HwOilPress) {
            assert!((0.0..=8.0).contains(&v.as_f32()), "got {}", v.as_f32());
            assert_eq!(v.constraints.critical_low, Some(0.5));
            assert_eq!(v.constraints.warning_low, Some(1.0));
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn calibrated_analog_curve_with_one_point_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwOilPress","provider":"adc",
            "sensor":{"kind":"calibrated_analog","id":"x","name":"x","units":"b",
              "r_series_ohm":130.8,"curve":[{"ohm":100.0,"value":1.0}],
              "constraints":{"min":0.0,"max":8.0}}}]"#;
        let path = write_temp_config(json);
        let frame = TestADCDataProvider::start().frame();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", frame, None, &mut mgr).unwrap_err();
        assert!(err.contains("at least 2 points"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_hw_input_is_a_load_error_naming_the_string() {
        let json = r#"[{"group":"sensor","hw_input":"NotARealInput","provider":"adc",
            "sensor":{"kind":"generic_digital","id":"x","name":"x","active_level":"high","constraints":{"digital_preset":"default"}}}]"#;
        let path = write_temp_config(json);
        let frame = TestADCDataProvider::start().frame();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", frame, None, &mut mgr).unwrap_err();
        assert!(err.contains("NotARealInput"), "error should name the bad string, got: {err}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn mismatched_processor_kind_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwParkBrake","provider":"adc",
            "analog_processors":[{"type":"moving_average","window":10}],
            "sensor":{"kind":"generic_digital","id":"HwParkBrake","name":"x","active_level":"high","constraints":{"digital_preset":"default"}}}]"#;
        let path = write_temp_config(json);
        let frame = TestADCDataProvider::start().frame();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", frame, None, &mut mgr).unwrap_err();
        assert!(err.contains("analog_processors"), "error should mention the mismatch, got: {err}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unsupported_provider_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwParkBrake","provider":"gnss",
            "sensor":{"kind":"generic_digital","id":"x","name":"x","active_level":"high","constraints":{"digital_preset":"default"}}}]"#;
        let path = write_temp_config(json);
        let frame = TestADCDataProvider::start().frame();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", frame, None, &mut mgr).unwrap_err();
        assert!(err.contains("gnss"), "error should name the unsupported provider, got: {err}");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_is_a_load_error() {
        let frame = TestADCDataProvider::start().frame();
        let mut mgr = SensorManager::new();
        assert!(load_chains(Path::new("/nonexistent/sensor_config.json"), "sensor", frame, None, &mut mgr).is_err());
    }

    /// Exercises the repo's actual sensor_config.json end to end, so a transcription
    /// mistake (bad hw_input name, mismatched processor/sensor kind, malformed JSON)
    /// fails `cargo test` instead of only surfacing at dashboard startup.
    #[test]
    fn repo_sensor_config_json_loads_successfully() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sensor_config.json");
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        // TestADCDataProvider's background thread needs a moment to write its first
        // synthetic frame -- reading before that gives "channel not in frame" errors.
        std::thread::sleep(Duration::from_millis(50));

        let mut sensor_mgr = SensorManager::new();
        load_chains(&path, "sensor", frame.clone(), Some(provider.temp_frame()), &mut sensor_mgr)
            .expect("repo config's \"sensor\" group should load");

        let mut button_mgr = SensorManager::new();
        load_chains(&path, "button", frame, None, &mut button_mgr).expect("repo config's \"button\" group should load");
    }

    /// A `provider: "adc_temp"` entry builds a working OneWireTempSensor chain against the
    /// synthetic one-wire frame TestADCDataProvider now writes (the bench ROMs sweep a °C
    /// band during SELF_TEST_DURATION).
    #[test]
    fn adc_temp_chain_loads_and_reads_a_temperature() {
        let json = r#"[
            {"group":"sensor","hw_input":"HwTempOut","provider":"adc_temp","rom":"2854df6b000000d9",
             "analog_processors":[{"type":"moving_average","window":1}],
             "sensor":{"kind":"one_wire_temp","id":"HwTempOut","name":"НАРУЖ",
               "constraints":{"min":-40.0,"max":80.0,"warning_high":45.0}}}
        ]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let (frame, temp_frame) = (provider.frame(), provider.temp_frame());
        std::thread::sleep(Duration::from_millis(50));

        let mut mgr = SensorManager::new();
        load_chains(&path, "sensor", frame, Some(temp_frame), &mut mgr).expect("should load");
        mgr.read_all_sensors().ok();
        let value = mgr.get_sensor_value(&HWInput::HwTempOut).expect("HwTempOut should have a value");
        // Sweep spans 10..70 °C; any reading in that band means the i16→°C decode ran.
        assert!(value.as_f32() >= 9.0 && value.as_f32() <= 71.0, "got {}", value.as_f32());
        assert_eq!(value.constraints.warning_high, Some(45.0));

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn adc_temp_without_rom_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwTempOut","provider":"adc_temp",
            "sensor":{"kind":"one_wire_temp","id":"x","name":"x","constraints":{"min":-40.0,"max":80.0}}}]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", provider.frame(), Some(provider.temp_frame()), &mut mgr).unwrap_err();
        assert!(err.contains("rom"), "error should mention the missing rom, got: {err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn adc_temp_with_short_rom_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwTempOut","provider":"adc_temp","rom":"28ff",
            "sensor":{"kind":"one_wire_temp","id":"x","name":"x","constraints":{"min":-40.0,"max":80.0}}}]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", provider.frame(), Some(provider.temp_frame()), &mut mgr).unwrap_err();
        assert!(err.contains("malformed rom"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn one_wire_temp_kind_with_plain_adc_provider_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwTempOut","provider":"adc","rom":"2854df6b000000d9",
            "sensor":{"kind":"one_wire_temp","id":"x","name":"x","constraints":{"min":-40.0,"max":80.0}}}]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", provider.frame(), Some(provider.temp_frame()), &mut mgr).unwrap_err();
        assert!(err.contains("one_wire_temp"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn adc_temp_with_digital_processors_is_a_load_error() {
        let json = r#"[{"group":"sensor","hw_input":"HwTempOut","provider":"adc_temp","rom":"2854df6b000000d9",
            "digital_processors":[{"type":"debounce","stable_count":5,"stable_delay_ms":50}],
            "sensor":{"kind":"one_wire_temp","id":"x","name":"x","constraints":{"min":-40.0,"max":80.0}}}]"#;
        let path = write_temp_config(json);
        let provider = TestADCDataProvider::start();
        let mut mgr = SensorManager::new();
        let err = load_chains(&path, "sensor", provider.frame(), Some(provider.temp_frame()), &mut mgr).unwrap_err();
        assert!(err.contains("digital_processors"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }
}
