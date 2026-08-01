#![allow(dead_code)]
use rppal::gpio::Level;

use crate::hardware::hw_providers::GNSS_ALTITUDE_OFFSET_M;
use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueMetadata};
use std::time::{Duration, Instant};

// Used by all sensor types
pub trait Sensor {
    fn id(&self) -> &String;
    fn name(&self) -> &String;
    // Get last sensor value without modifying state
    fn value(&self) -> Result<&SensorValue, String>;
    fn constraints(&self) -> &ValueConstraints;
    fn metadata(&self) -> &ValueMetadata;
    fn min_value(&self) -> f32;
    fn max_value(&self) -> f32;
}

// Digital sensor trait - represents on/off state based on active level
// Active level could be low in case of pull-up input configuration
pub trait DigitalSensor: Sensor {
    fn active_level(&self) -> Level;

    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: Level) -> Result<&SensorValue, String>;
}

// Analog sensor trait - represents a numeric value based on raw input
// Value should be a processed input, e.g. voltage level converted to temperature
// All voltage divider calculations, pulse count to speed, and other 
// raw input conversion into meaningful values are done here
pub trait AnalogSensor: Sensor {
    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: u16) -> Result<&SensorValue, String>;
}

pub struct GenericDigitalSensor {
    value: SensorValue,
    active_level: Level,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl GenericDigitalSensor {
    pub fn new(id: String, name: String, active_level: Level,
               constraints: ValueConstraints) -> Self {
        let metadata = ValueMetadata::new("", name, id); // Empty unit for digital sensors
        GenericDigitalSensor { value: SensorValue::empty(),
                               active_level, constraints, metadata}
    }
}

impl Sensor for GenericDigitalSensor {
    fn id(&self) -> &String {
        &self.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl DigitalSensor for GenericDigitalSensor {
    fn active_level(&self) -> Level {
        self.active_level
    }

    fn read(&mut self, input: Level) -> Result<&SensorValue, String> {
        self.value = SensorValue::digital_with_constraints_and_metadata(
            input == self.active_level,
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

pub struct GenericAnalogSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    scale_factor: f32,
}

impl GenericAnalogSensor {
    pub fn new(id: String, name: String, units: String,
               constraints: ValueConstraints, scale_factor: f32) -> Self {
        let metadata = ValueMetadata::new(units, name, id);
        GenericAnalogSensor {
            value: SensorValue::empty(),
            constraints,
            metadata,
            scale_factor,
        }
    }
}

impl Sensor for GenericAnalogSensor {
    fn id(&self) -> &String {
        &self.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl AnalogSensor for GenericAnalogSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let value = (input as f32) * self.scale_factor;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            value.clamp(self.min_value(), self.max_value()),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

pub struct EngineTemperatureSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl EngineTemperatureSensor {
    pub fn new() -> Self {
        EngineTemperatureSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog_with_thresholds(
                0.0, 120.0,
                None, None,
                Some(100.0), Some(110.0),
            ),
            metadata: ValueMetadata {
                unit: "°C".to_string(),
                label: "ТЕМП".to_string(),
                sensor_id: "engine_temp".to_string(),
            },
        }
    }
}

impl Sensor for EngineTemperatureSensor {
    fn id(&self) -> &String {
        &self.value.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.value.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl AnalogSensor for EngineTemperatureSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        // Convert raw input (e.g. ADC value) to temperature
        // Placeholder conversion logic
        let temperature = (input as f32) * 0.1; // Placeholder conversion
        self.value = SensorValue::analog_with_constraints_and_metadata(
            temperature.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

/// Decodes GnssChannelProvider's altitude encoding (raw = altitude_m + GNSS_ALTITUDE_OFFSET_M,
/// see hw_providers.rs) back to meters. A plain GenericAnalogSensor can't express this since
/// it only supports a multiplicative scale, not an additive offset.
pub struct GnssAltitudeSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl GnssAltitudeSensor {
    pub fn new() -> Self {
        GnssAltitudeSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(-500.0, 9000.0), // Dead Sea to above Everest
            metadata: ValueMetadata::new("м", "ВЫСОТА", "gnss_altitude"),
        }
    }
}

impl Sensor for GnssAltitudeSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for GnssAltitudeSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let altitude_m = input as f32 - GNSS_ALTITUDE_OFFSET_M;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            altitude_m.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

/// Current LSB scale for the INA219 as configured by UpsI2CDataProvider's calibration
/// (set_calibration_16V_5A: 0.01 ohm shunt, 5A max) — mA per raw register count.
const UPS_CURRENT_LSB_MA: f32 = 0.1524;

/// Current draw below this (mA) counts as "discharging" (running on battery, mains absent
/// or insufficient). UPS reported current at full charge normally floats arount 0..-150 mA 
// for extended periods, -200 mA threshold accounts for that.
pub const UPS_ON_BATTERY_CURRENT_THRESHOLD_MA: f32 = -200.0;

/// Bus voltage (V) mapped to state of charge (0-100%), ported from Waveshare's INA219.py
/// demo (`p = (bus_voltage - 3) / 1.2 * 100`) — a linear estimate between an empty single
/// Li-ion cell (3.0V) and a full one (4.2V), clamped to the valid range.
const UPS_SOC_EMPTY_V: f32 = 3.0;
const UPS_SOC_FULL_V: f32 = 4.2;

/// Converts the INA219's raw current register to signed mA. Positive means charging/mains
/// present, negative means discharging/on-battery (see UPS_ON_BATTERY_CURRENT_THRESHOLD_MA).
pub struct UpsCurrentSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl UpsCurrentSensor {
    pub fn new() -> Self {
        UpsCurrentSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog_with_thresholds(
                -5000.0, 5000.0,
                None, Some(UPS_ON_BATTERY_CURRENT_THRESHOLD_MA),
                None, None,
            ),
            metadata: ValueMetadata::new("мА", "ТОК ИБП", "ups_current"),
        }
    }
}

impl Sensor for UpsCurrentSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for UpsCurrentSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        // The INA219 current register is two's complement 16-bit, so the raw u16 bit
        // pattern reinterprets losslessly as i16 — no separate sign-extension needed.
        // INA219.py negates the raw register before interpreting its sign
        // (`current = -ina219.getCurrent_mA()`) — confirmed empirically: on mains power the
        // raw register reads negative (~-1.2A) while the negated, user-facing value reads
        // positive (~+1.2A), consistent with "positive = charging/mains present".
        let current_ma = -((input as i16) as f32 * UPS_CURRENT_LSB_MA);
        self.value = SensorValue::analog_with_constraints_and_metadata(
            current_ma.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

/// Converts the INA219's raw bus voltage register to estimated battery state of charge.
pub struct UpsChargeSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl UpsChargeSensor {
    pub fn new() -> Self {
        UpsChargeSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog_with_thresholds(
                0.0, 100.0,
                Some(15.0), Some(25.0),
                None, None,
            ),
            metadata: ValueMetadata::new("%", "ЗАРЯД АКБ", "ups_charge"),
        }
    }
}

impl Sensor for UpsChargeSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for UpsChargeSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        // Matches Waveshare's INA219.py getBusVoltage_V: raw register's top 13 bits (>>3)
        // are the voltage reading, in 4mV steps.
        let bus_voltage_v = (input >> 3) as f32 * 0.004;
        let percent = (bus_voltage_v - UPS_SOC_EMPTY_V) / (UPS_SOC_FULL_V - UPS_SOC_EMPTY_V) * 100.0;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            percent.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

// HwSpeed (channel 5): period-based measurement, replacing the count-based interim approach
// (see SPEED_TACHO_PULSE_PERIOD_DESIGN.md). STM32 firmware now sends inter-pulse periods
// (SPEED_PERIOD_UNIT_US = 10us/unit, same encoding as HwTacho below).

/// Wheel-speed sensor pulses/revolution (WIRING.md).
const SPEED_PULSES_PER_REV: f32 = 6.0;
/// 235/75/15 tire circumference, meters. Width 235mm, aspect ratio 75%, rim 15in ->
/// diameter = 15in (381mm) + 2*(235mm*0.75) = 733.5mm -> circumference = pi*733.5mm.
const SPEED_WHEEL_CIRCUMFERENCE_M: f32 = 2.304;
/// Timer tick rate for the raw period channel: firmware's SPEED_PERIOD_UNIT_US = 10us/unit,
/// i.e. 1/10us = 100_000 ticks/sec. A u16 raw period doesn't wrap before speed drops to a
/// "may as well be stopped" ~0.55 km/h (65535 ticks / 100_000 Hz = 0.655s period), while
/// still giving sub-km/h resolution at highway speed (100 km/h's 13.82ms inter-pulse period
/// is ~1382 ticks at this rate).
const SPEED_PERIOD_TIMER_HZ: f32 = 100_000.0;
/// Raw period value reserved to mean "no pulse observed" (stationary, or no pulse seen yet
/// since startup) -- distinct from a real, merely long period, which the wire format needs
/// to be able to represent unambiguously at the low-speed end (see design doc).
const SPEED_PERIOD_IDLE_RAW: u16 = 0;
/// How many multiples of the last known inter-pulse period to wait, once that period value
/// stops updating, before concluding the vehicle has slowed further (or stopped) and the
/// latched period no longer reflects the current speed. Scaled to the last observed period
/// rather than a fixed constant (contrast ADCFrame::is_stale()'s fixed ADC_LINK_MAX_AGE)
/// since "no new pulse in N ms" means something different at 5 km/h than at 100 km/h.
const SPEED_STALE_PERIOD_MULTIPLIER: f32 = 3.0;
/// Floor under the multiplier above so a tiny last-known period (near top speed) can't
/// produce an implausibly short timeout that flickers to 0 on ordinary pulse-to-pulse jitter.
const SPEED_MIN_STALE_THRESHOLD: Duration = Duration::from_millis(100);

pub struct SpeedSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    last_raw: u16,
    last_change: Instant,
}

impl SpeedSensor {
    pub fn new() -> Self {
        SpeedSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(0.0, 180.0),
            metadata: ValueMetadata::new("км/ч", "СКОР", "speed_sensor"),
            last_raw: SPEED_PERIOD_IDLE_RAW,
            last_change: Instant::now(),
        }
    }

    /// Inter-pulse period (raw timer ticks) -> speed (km/h): inverting a period into a rate,
    /// instead of dividing an accumulated count by a fixed window. Caller is responsible for
    /// excluding SPEED_PERIOD_IDLE_RAW before calling this (division by it isn't meaningful).
    fn speed_kmh_from_period_raw(raw: u16) -> f32 {
        let period_s = raw as f32 / SPEED_PERIOD_TIMER_HZ;
        SPEED_WHEEL_CIRCUMFERENCE_M / (period_s * SPEED_PULSES_PER_REV) * 3.6
    }
}

/// Inverse of speed_kmh_from_period_raw, exposed for TestADCDataProvider's self-test sweep
/// generator so its synthetic HwSpeed data is derived from the exact same formula the real
/// conversion uses, rather than a separately maintained copy that could silently drift out
/// of sync with it. Never returns SPEED_PERIOD_IDLE_RAW for a positive speed -- the rounded
/// period is floored at 1 tick instead, so a genuinely nonzero (if implausibly high) speed
/// can never be misread as the "no pulse" sentinel.
pub fn speed_period_raw_from_kmh(speed_kmh: f32) -> u16 {
    if speed_kmh <= 0.0 {
        return SPEED_PERIOD_IDLE_RAW;
    }
    let period_s = SPEED_WHEEL_CIRCUMFERENCE_M / ((speed_kmh / 3.6) * SPEED_PULSES_PER_REV);
    (period_s * SPEED_PERIOD_TIMER_HZ).round().clamp(1.0, u16::MAX as f32) as u16
}

impl Sensor for SpeedSensor {
    fn id(&self) -> &String {
        &self.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl AnalogSensor for SpeedSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        if input != self.last_raw {
            self.last_raw = input;
            self.last_change = Instant::now();
        }

        let speed_kmh = if self.last_raw == SPEED_PERIOD_IDLE_RAW {
            0.0
        } else {
            let last_period_s = self.last_raw as f32 / SPEED_PERIOD_TIMER_HZ;
            let stale_after = Duration::from_secs_f32(last_period_s * SPEED_STALE_PERIOD_MULTIPLIER)
                .max(SPEED_MIN_STALE_THRESHOLD);
            if self.last_change.elapsed() > stale_after {
                0.0
            } else {
                Self::speed_kmh_from_period_raw(self.last_raw)
            }
        };

        self.value = SensorValue::analog_with_constraints_and_metadata(
            speed_kmh.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

// HwTacho (channel 4): period-based measurement, same wire scheme as HwSpeed above (see
// SPEED_TACHO_PULSE_PERIOD_DESIGN.md and the STM32 firmware's "Tachometer timing" comment) --
// replaces the old boolean "engine running" debounce, which was mathematically unable to
// latch true anywhere in the normal idle range (400-800 rpm) because a pulse-count-per-frame
// encoding can't represent sub-1-count-per-frame rates.

/// Tachometer pulses/revolution (STM32 firmware: PIN_TACHO, 2 PPR).
const TACHO_PULSES_PER_REV: f32 = 2.0;
/// Wire units are TACHO_PERIOD_UNIT_US = 10us/unit on the firmware side, same encoding (and
/// same 100_000 Hz) as SPEED_PERIOD_TIMER_HZ above.
const TACHO_PERIOD_TIMER_HZ: f32 = 100_000.0;
/// Raw period value reserved to mean "no pulse observed" (stalled, or no pulse seen yet
/// since startup) -- mirrors SPEED_PERIOD_IDLE_RAW.
const TACHO_PERIOD_IDLE_RAW: u16 = 0;
/// Same role as SPEED_STALE_PERIOD_MULTIPLIER: how many multiples of the last known
/// inter-pulse period to wait, once that period stops updating, before concluding the
/// engine has slowed further (or stalled).
const TACHO_STALE_PERIOD_MULTIPLIER: f32 = 3.0;
/// Floor under the multiplier above, shorter than SPEED_MIN_STALE_THRESHOLD because engine
/// rpm changes (and stalls) faster than road speed -- matches the firmware's own
/// TACHO_TIMEOUT_US (500ms) being half of SPEED_TIMEOUT_US (1s).
const TACHO_MIN_STALE_THRESHOLD: Duration = Duration::from_millis(50);

pub struct TachoSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    last_raw: u16,
    last_change: Instant,
}

impl TachoSensor {
    pub fn new() -> Self {
        TachoSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(0.0, 6000.0),
            metadata: ValueMetadata::new("об/мин", "ТАХОМЕТР", "tacho_sensor"),
            last_raw: TACHO_PERIOD_IDLE_RAW,
            last_change: Instant::now(),
        }
    }

    /// Inter-pulse period (raw timer ticks) -> rpm. Caller is responsible for excluding
    /// TACHO_PERIOD_IDLE_RAW before calling this (division by it isn't meaningful).
    fn rpm_from_period_raw(raw: u16) -> f32 {
        let period_s = raw as f32 / TACHO_PERIOD_TIMER_HZ;
        60.0 / (period_s * TACHO_PULSES_PER_REV)
    }
}

/// Inverse of rpm_from_period_raw, exposed for TestADCDataProvider's self-test sweep
/// generator, same rationale as speed_period_raw_from_kmh.
pub fn tacho_period_raw_from_rpm(rpm: f32) -> u16 {
    if rpm <= 0.0 {
        return TACHO_PERIOD_IDLE_RAW;
    }
    let period_s = 60.0 / (rpm * TACHO_PULSES_PER_REV);
    (period_s * TACHO_PERIOD_TIMER_HZ).round().clamp(1.0, u16::MAX as f32) as u16
}

impl Sensor for TachoSensor {
    fn id(&self) -> &String {
        &self.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl AnalogSensor for TachoSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        if input != self.last_raw {
            self.last_raw = input;
            self.last_change = Instant::now();
        }

        let rpm = if self.last_raw == TACHO_PERIOD_IDLE_RAW {
            0.0
        } else {
            let last_period_s = self.last_raw as f32 / TACHO_PERIOD_TIMER_HZ;
            let stale_after = Duration::from_secs_f32(last_period_s * TACHO_STALE_PERIOD_MULTIPLIER)
                .max(TACHO_MIN_STALE_THRESHOLD);
            if self.last_change.elapsed() > stale_after {
                0.0
            } else {
                Self::rpm_from_period_raw(self.last_raw)
            }
        };

        self.value = SensorValue::analog_with_constraints_and_metadata(
            rpm.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::sensor_value::{ValueData, ValueConstraints};

    /// Raw period (in SPEED_PERIOD_TIMER_HZ ticks) that a steady 100 km/h implies, used by
    /// several tests below as a known-good reference point.
    fn raw_period_for_100_kmh() -> u16 {
        let period_s = SPEED_WHEEL_CIRCUMFERENCE_M / ((100.0 / 3.6) * SPEED_PULSES_PER_REV);
        (period_s * SPEED_PERIOD_TIMER_HZ).round() as u16
    }

    #[test]
    fn test_speed_sensor_idle_raw_reads_zero() {
        let mut sensor = SpeedSensor::new();
        let speed = sensor.read(SPEED_PERIOD_IDLE_RAW).unwrap().as_f32();
        assert_eq!(speed, 0.0);
    }

    #[test]
    fn test_speed_sensor_converts_period_to_expected_speed() {
        let mut sensor = SpeedSensor::new();
        let raw = raw_period_for_100_kmh();
        let speed = sensor.read(raw).unwrap().as_f32();
        assert!((speed - 100.0).abs() < 1.0, "expected ~100 km/h, got {}", speed);
    }

    #[test]
    fn test_speed_sensor_repeated_same_period_stays_fresh() {
        // A steady speed means the same raw period value keeps being reported every read --
        // that must NOT be treated as staleness (see SPEED_STALE_PERIOD_MULTIPLIER), or a
        // sensor cruising at constant speed would incorrectly drop to 0.
        let mut sensor = SpeedSensor::new();
        let raw = raw_period_for_100_kmh();
        for _ in 0..5 {
            let speed = sensor.read(raw).unwrap().as_f32();
            assert!((speed - 100.0).abs() < 1.0, "expected ~100 km/h, got {}", speed);
        }
    }

    #[test]
    fn test_speed_sensor_decays_to_zero_once_period_goes_stale() {
        // A low speed's long inter-pulse period, once it stops being refreshed by a new
        // pulse for long enough, must decay to 0 rather than reporting the stale speed
        // forever -- this is what lets the sensor detect the vehicle has slowed further
        // (or stopped) even though nothing ever explicitly reports SPEED_PERIOD_IDLE_RAW.
        let mut sensor = SpeedSensor::new();
        // ~5 km/h implies a period long enough that SPEED_MIN_STALE_THRESHOLD's floor
        // doesn't dominate, so this actually exercises the multiplier-based timeout.
        let period_s = SPEED_WHEEL_CIRCUMFERENCE_M / ((5.0 / 3.6) * SPEED_PULSES_PER_REV);
        let raw = (period_s * SPEED_PERIOD_TIMER_HZ).round() as u16;

        let speed = sensor.read(raw).unwrap().as_f32();
        assert!(speed > 0.0, "expected a nonzero initial reading, got {}", speed);

        std::thread::sleep(Duration::from_secs_f32(period_s * SPEED_STALE_PERIOD_MULTIPLIER) + Duration::from_millis(50));
        let speed = sensor.read(raw).unwrap().as_f32();
        assert_eq!(speed, 0.0, "expected stale period to decay to 0, got {}", speed);
    }

    #[test]
    fn test_speed_sensor_clamps_to_gauge_max() {
        let mut sensor = SpeedSensor::new();
        // raw=1 is the shortest non-idle period the format allows -- far beyond the gauge's
        // 180 km/h max at any plausible SPEED_PERIOD_TIMER_HZ, so this exercises the clamp.
        let speed = sensor.read(1).unwrap().as_f32();
        assert_eq!(speed, 180.0);
    }

    /// Raw period (in TACHO_PERIOD_TIMER_HZ ticks) that a steady 3000 rpm implies, used by
    /// several tests below as a known-good reference point.
    fn raw_period_for_3000_rpm() -> u16 {
        let period_s = 60.0 / (3000.0 * TACHO_PULSES_PER_REV);
        (period_s * TACHO_PERIOD_TIMER_HZ).round() as u16
    }

    #[test]
    fn test_tacho_sensor_idle_raw_reads_zero() {
        let mut sensor = TachoSensor::new();
        let rpm = sensor.read(TACHO_PERIOD_IDLE_RAW).unwrap().as_f32();
        assert_eq!(rpm, 0.0);
    }

    #[test]
    fn test_tacho_sensor_converts_period_to_expected_rpm() {
        let mut sensor = TachoSensor::new();
        let raw = raw_period_for_3000_rpm();
        let rpm = sensor.read(raw).unwrap().as_f32();
        assert!((rpm - 3000.0).abs() < 10.0, "expected ~3000 rpm, got {}", rpm);
    }

    #[test]
    fn test_tacho_sensor_repeated_same_period_stays_fresh() {
        // A steady rpm means the same raw period value keeps being reported every read --
        // that must NOT be treated as staleness (see TACHO_STALE_PERIOD_MULTIPLIER), since
        // at low rpm a single inter-pulse period can span several 20ms report frames.
        let mut sensor = TachoSensor::new();
        let raw = raw_period_for_3000_rpm();
        for _ in 0..5 {
            let rpm = sensor.read(raw).unwrap().as_f32();
            assert!((rpm - 3000.0).abs() < 10.0, "expected ~3000 rpm, got {}", rpm);
        }
    }

    #[test]
    fn test_tacho_sensor_decays_to_zero_once_period_goes_stale() {
        // Idle's long inter-pulse period, once it stops being refreshed by a new pulse for
        // long enough, must decay to 0 -- this is how the sensor detects a stall even though
        // nothing ever explicitly reports TACHO_PERIOD_IDLE_RAW.
        let mut sensor = TachoSensor::new();
        // ~500 rpm implies a period long enough that TACHO_MIN_STALE_THRESHOLD's floor
        // doesn't dominate, so this actually exercises the multiplier-based timeout.
        let period_s = 60.0 / (500.0 * TACHO_PULSES_PER_REV);
        let raw = (period_s * TACHO_PERIOD_TIMER_HZ).round() as u16;

        let rpm = sensor.read(raw).unwrap().as_f32();
        assert!(rpm > 0.0, "expected a nonzero initial reading, got {}", rpm);

        std::thread::sleep(Duration::from_secs_f32(period_s * TACHO_STALE_PERIOD_MULTIPLIER) + Duration::from_millis(50));
        let rpm = sensor.read(raw).unwrap().as_f32();
        assert_eq!(rpm, 0.0, "expected stale period to decay to 0, got {}", rpm);
    }

    #[test]
    fn test_tacho_sensor_clamps_to_gauge_max() {
        let mut sensor = TachoSensor::new();
        // raw=1 is the shortest non-idle period the format allows -- far beyond the gauge's
        // 6000 rpm max, so this exercises the clamp.
        let rpm = sensor.read(1).unwrap().as_f32();
        assert_eq!(rpm, 6000.0);
    }

    #[test]
    fn test_generic_digital_sensor_creation() {
        let constraints = ValueConstraints::digital_default();
        let sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::High,
            constraints
        );

        assert_eq!(sensor.active_level(), Level::High);
        assert_eq!(Sensor::value(&sensor).unwrap().value, ValueData::Empty);
    }

    #[test]
    fn test_generic_digital_sensor_active_high() {
        let constraints = ValueConstraints::digital_default();
        let mut sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::High,
            constraints
        );

        // Test active level (High)
        sensor.read(Level::High).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, true);
        } else {
            panic!("Expected digital value");
        }

        // Test inactive level (Low)
        sensor.read(Level::Low).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, false);
        } else {
            panic!("Expected digital value");
        }
    }

    #[test]
    fn test_generic_digital_sensor_active_low() {
        let constraints = ValueConstraints::digital_default();
        let mut sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::Low,
            constraints
        );

        // Test active level (Low)
        sensor.read(Level::Low).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, true);
        } else {
            panic!("Expected digital value");
        }

        // Test inactive level (High)
        sensor.read(Level::High).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, false);
        } else {
            panic!("Expected digital value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_creation() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 1.0);

        assert_eq!(sensor.min_value(), 0.0);
        assert_eq!(sensor.max_value(), 100.0);
        assert_eq!(Sensor::value(&sensor).unwrap().value, ValueData::Empty);
    }

    #[test]
    fn test_generic_analog_sensor_reading() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 0.1);

        // Test normal reading
        sensor.read(500).unwrap(); // 500 * 0.1 = 50.0
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert!((value - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_clamping() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 1.0);

        // Test value above maximum gets clamped
        sensor.read(150).unwrap();
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*value, 100.0);
        } else {
            panic!("Expected analog value");
        }

        // Test value below minimum gets clamped
        sensor.read(0).unwrap(); // This should clamp to 0.0 (minimum)
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*value, 0.0);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_scaling() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 0.01);

        sensor.read(5000).unwrap(); // 5000 * 0.01 = 50.0
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert!((value - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_engine_temperature_sensor_creation() {
        let sensor = EngineTemperatureSensor::new();
        
        assert_eq!(sensor.constraints.min_value, 0.0);
        assert_eq!(sensor.constraints.max_value, 120.0);
        assert_eq!(sensor.metadata.unit, "°C");
        assert_eq!(sensor.metadata.label, "ТЕМП");
        assert_eq!(sensor.metadata.sensor_id, "engine_temp");
    }

    #[test]
    fn test_engine_temperature_sensor_reading() {
        let mut sensor = EngineTemperatureSensor::new();

        // Test normal temperature reading
        sensor.read(500).unwrap(); // 500 * 0.1 = 50.0°C
        if let ValueData::Analog(temp) = &Sensor::value(&sensor).unwrap().value {
            assert!((temp - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog temperature value");
        }

        // Test high temperature reading with clamping
        sensor.read(1500).unwrap(); // 1500 * 0.1 = 150.0°C, should clamp to 120.0°C
        if let ValueData::Analog(temp) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*temp, 120.0);
        } else {
            panic!("Expected analog temperature value");
        }
    }

    #[test]
    fn test_gnss_altitude_sensor_decoding() {
        let mut sensor = GnssAltitudeSensor::new();

        // Encoded as (altitude_m + GNSS_ALTITUDE_OFFSET_M).round() by GnssChannelProvider.
        sensor.read(1150).unwrap(); // 1150 - 1000 = 150.0 m
        if let ValueData::Analog(alt) = &Sensor::value(&sensor).unwrap().value {
            assert!((alt - 150.0).abs() < 0.001);
        } else {
            panic!("Expected analog altitude value");
        }

        // Below-sea-level altitude survives the unsigned u16 boundary via the offset.
        sensor.read(600).unwrap(); // 600 - 1000 = -400.0 m
        if let ValueData::Analog(alt) = &Sensor::value(&sensor).unwrap().value {
            assert!((alt - (-400.0)).abs() < 0.001);
        } else {
            panic!("Expected analog altitude value");
        }
    }

    #[test]
    fn test_ups_current_sensor_mains_present() {
        let mut sensor = UpsCurrentSensor::new();
        // Raw register reads negative while on mains (INA219.py's sign convention before
        // negation) — should come out positive after conversion.
        let raw = (-8000i16) as u16; // -8000 * 0.1524 = -1219.2 mA raw, negated -> +1219.2
        sensor.read(raw).unwrap();
        if let ValueData::Analog(ma) = &Sensor::value(&sensor).unwrap().value {
            assert!(*ma > 0.0, "expected positive current on mains, got {}", ma);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_ups_current_sensor_on_battery() {
        let mut sensor = UpsCurrentSensor::new();
        // Positive raw register (pre-negation) -> negative mA -> on-battery.
        let raw = 2000u16;
        sensor.read(raw).unwrap();
        let value = Sensor::value(&sensor).unwrap();
        assert!(value.as_f32() < 0.0);
        assert!(value.is_warning(), "on-battery current should trip the warning threshold");
    }

    #[test]
    fn test_ups_charge_sensor_full_and_empty() {
        let mut sensor = UpsChargeSensor::new();

        // 4.2V bus voltage -> raw register = (4.2 / 0.004) << 3
        let full_raw = ((4.2 / 0.004) as u16) << 3;
        sensor.read(full_raw).unwrap();
        if let ValueData::Analog(pct) = &Sensor::value(&sensor).unwrap().value {
            assert!((pct - 100.0).abs() < 1.0);
        } else {
            panic!("Expected analog value");
        }

        // 3.0V bus voltage -> 0%
        let empty_raw = ((3.0 / 0.004) as u16) << 3;
        sensor.read(empty_raw).unwrap();
        if let ValueData::Analog(pct) = &Sensor::value(&sensor).unwrap().value {
            assert!(pct.abs() < 1.0);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_sensor_trait_implementations() {
        // Test GenericDigitalSensor implements Sensor trait correctly
        let constraints = ValueConstraints::digital_default();
        let digital_sensor = GenericDigitalSensor::new(
            "digital_test".to_string(),
            "Digital Test".to_string(),
            Level::High,
            constraints
        );
        
        assert!(digital_sensor.id().contains("digital_test"));
        assert!(digital_sensor.name().contains("Digital Test"));
        assert!(Sensor::value(&digital_sensor).is_ok());
        assert!(digital_sensor.constraints().min_value >= 0.0);
        // Note: empty SensorValue has empty metadata, so we skip that check

        // Test GenericAnalogSensor implements Sensor trait correctly
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, None, None, None, None);
        let analog_sensor = GenericAnalogSensor::new("analog_test".to_string(), "Analog Test".to_string(), "V".to_string(), constraints, 1.0);
        
        assert_eq!(analog_sensor.id(), "analog_test");
        assert_eq!(analog_sensor.name(), "Analog Test");
        assert!(Sensor::value(&analog_sensor).is_ok());
        assert_eq!(analog_sensor.constraints().min_value, 0.0);
        assert_eq!(analog_sensor.metadata().unit, "V");
    }

    #[test]
    fn test_analog_sensor_trait_implementations() {
        // Test GenericAnalogSensor implements AnalogSensor trait
        let constraints = ValueConstraints::analog_with_thresholds(10.0, 90.0, None, None, None, None);
        let mut sensor = GenericAnalogSensor::new("test".to_string(), "Test".to_string(), "V".to_string(), constraints, 1.0);
        
        assert_eq!(sensor.min_value(), 10.0);
        assert_eq!(sensor.max_value(), 90.0);
        
        let result = sensor.read(50);
        assert!(result.is_ok());

        let value_result = Sensor::value(&sensor);
        assert!(value_result.is_ok());

        // Test EngineTemperatureSensor implements AnalogSensor trait
        let mut temp_sensor = EngineTemperatureSensor::new();
        assert_eq!(temp_sensor.min_value(), 0.0);
        assert_eq!(temp_sensor.max_value(), 120.0);
        
        let temp_result = temp_sensor.read(800);
        assert!(temp_result.is_ok());
        
        let temp_value_result = Sensor::value(&temp_sensor);
        assert!(temp_value_result.is_ok());
    }
}