#![allow(dead_code)]
use rppal::gpio::Level;

use crate::hardware::hw_providers::GNSS_ALTITUDE_OFFSET_M;
use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueMetadata};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(test)]
use std::time::Duration;

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

/// PA3 12V-system-voltage divider and ADC constants (stm32_adc_module/WIRING.md, "PA3 --
/// 12V system voltage"): R1 = 51 kΩ from the 12V line, R2 = 10 kΩ to GND, ADC pin taps the
/// junction; ADC is 12-bit referenced to 3.3V. These mirror osc_page.rs's OSC_DIVIDER_* /
/// OSC_ADC_* -- one physical circuit read from opposite ends of the codebase; keep the two
/// sets in sync if the divider ever changes.
const V12_DIVIDER_R1_OHM: f32 = 51_000.0;
const V12_DIVIDER_R2_OHM: f32 = 10_000.0;
const V12_ADC_VREF: f32 = 3.3;
const V12_ADC_MAX_CODE: f32 = 4095.0;

/// Converts a raw 12-bit ADC code from a resistive voltage divider back to the real tapped
/// voltage: `code / ADC_MAX * V_REF * (R1 + R2) / R2 * trim`. Used for the `Hw12v` channel.
/// Unlike the resistive senders (see SENSOR_CALIBRATION_DESIGN.md) this needs no resistance
/// curve or live-supply cross-dependency -- the divider is a single exact linear relation.
///
/// `trim` is one multiplicative correction (1.0 = none) for the combined tolerance of the
/// two divider resistors, the ADC reference, and ADC gain error; set it from a bench
/// measurement against a known supply voltage.
pub struct VoltageDividerSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    volts_per_code: f32,
}

impl VoltageDividerSensor {
    pub fn new(id: String, name: String, units: String,
               constraints: ValueConstraints, trim: f32) -> Self {
        let volts_per_code = V12_ADC_VREF / V12_ADC_MAX_CODE
            * (V12_DIVIDER_R1_OHM + V12_DIVIDER_R2_OHM) / V12_DIVIDER_R2_OHM
            * trim;
        VoltageDividerSensor {
            value: SensorValue::empty(),
            constraints,
            metadata: ValueMetadata::new(units, name, id),
            volts_per_code,
        }
    }
}

impl Sensor for VoltageDividerSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for VoltageDividerSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let volts = input as f32 * self.volts_per_code;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            volts.clamp(self.constraints.min_value, self.constraints.max_value),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

/// Voltage-divider and ADC constants for the PA0/PA1/PA2 resistive-sender inputs (oil
/// pressure, fuel level, coolant temp): R1 = 39 kΩ / R2 = 10 kΩ divider, 12-bit ADC
/// referenced to 3.3 V (stm32_adc_module/WIRING.md; SENSOR_CALIBRATION_DESIGN.md). Separate
/// from the V12_* set above -- that divider (51k/10k) is PA3, this one is PA0-PA2.
const SENDER_DIVIDER_R1_OHM: f32 = 39_000.0;
const SENDER_DIVIDER_R2_OHM: f32 = 10_000.0;
const SENDER_ADC_VREF: f32 = 3.3;
const SENDER_ADC_MAX_CODE: f32 = 4095.0;
/// Minimum `V_supply - V_sensor_wire` headroom before a reading is treated as a fault
/// instead of converted. A disconnected sender or ADC noise pushes `V_sensor_wire` to or
/// past `V_supply`, which would blow the inferred resistance up to a huge or negative
/// value; a few mV of margin makes noise right at the boundary fault cleanly rather than
/// oscillate (SENSOR_CALIBRATION_DESIGN.md, "Fault handling").
const SENDER_FAULT_MARGIN_V: f32 = 0.005;

/// Low-side counterpart to `SENDER_FAULT_MARGIN_V`. If the PA0/PA1/PA2 divider's *input*
/// wire (to the instrument cluster) is disconnected, R2 pulls the ADC pin to ~0 V, which
/// decodes to a near-short sender resistance -- otherwise indistinguishable from a
/// legitimate full-scale reading, since low resistance means high temp / pressure / level
/// for all three senders. A computed `R_sender` below the curve's lowest tabulated point
/// times this fraction (floored at `SENDER_SHORT_FAULT_MIN_OHM`) is treated as that fault
/// and returns `Err`. The high-Ω end stays a plain clamp, so an open *sender* still reads
/// minimum like the stock gauge (SENSOR_CALIBRATION_DESIGN.md, "Failure modes").
const SENDER_SHORT_FAULT_CURVE_FRACTION: f32 = 0.4;
const SENDER_SHORT_FAULT_MIN_OHM: f32 = 2.0;

/// Supply voltage assumed for the resistive-sender conversion until `Hw12v` produces its
/// first real reading -- a representative mid-charge idle voltage, never 0 (which would make
/// the conversion denominator degenerate). See SENSOR_CALIBRATION_DESIGN.md,
/// "Startup/failure safety".
pub const NOMINAL_SUPPLY_V: f32 = 12.2;

/// Piecewise-linear lookup of `value` from a `(ohm, value)` curve sorted ascending by
/// `ohm`. Outside the tabulated range it clamps to the nearest endpoint value rather than
/// extrapolating. `curve` must hold at least two points (the config loader enforces this).
fn interpolate_curve(curve: &[(f32, f32)], ohm: f32) -> f32 {
    if ohm <= curve[0].0 {
        return curve[0].1;
    }
    let last = curve[curve.len() - 1];
    if ohm >= last.0 {
        return last.1;
    }
    let hi = curve.iter().position(|&(o, _)| o >= ohm).unwrap();
    let (o0, v0) = curve[hi - 1];
    let (o1, v1) = curve[hi];
    v0 + (v1 - v0) * (ohm - o0) / (o1 - o0)
}

/// Converts a raw ADC count from a PA0/PA1/PA2 resistive sender to a physical value through
/// a datasheet resistance curve, using the *live* 12 V supply reading rather than an assumed
/// nominal. The sender forms a plain two-resistor divider with the OEM gauge coil, so the
/// tapped node voltage (and hence the raw count) scales directly with supply voltage -- see
/// SENSOR_CALIBRATION_DESIGN.md for the full derivation.
///
/// `read()` returns `Err` when the divider headroom collapses (disconnected sender / ADC
/// noise). `SensorManager::read_all_sensors` already drops one chain's `Err` for that tick
/// without disturbing the others.
pub struct CalibratedVariableResistanceAnalogSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    /// `(ohm, value)` datasheet points, sorted ascending by `ohm`.
    curve: Vec<(f32, f32)>,
    /// Variable-coil winding resistance of the OEM gauge this sender shares its divider with
    /// (measured per gauge; SENSOR_CALIBRATION_DESIGN.md).
    r_series_ohm: f32,
    /// Field-calibration offset added to the interpolated curve output. Stays 0.0 until the
    /// calibration overlay/UI lands (SENSOR_CALIBRATION_DESIGN.md, "Calibration overlay file").
    value_offset: f32,
    /// Live `Hw12v` reading as `f32` bits, published by `SupplyVoltagePublisher`.
    v_supply: Arc<AtomicU32>,
}

impl CalibratedVariableResistanceAnalogSensor {
    pub fn new(id: String, name: String, units: String, r_series_ohm: f32,
               curve: Vec<(f32, f32)>, value_offset: f32,
               constraints: ValueConstraints, v_supply: Arc<AtomicU32>) -> Self {
        CalibratedVariableResistanceAnalogSensor {
            value: SensorValue::empty(),
            constraints,
            metadata: ValueMetadata::new(units, name, id),
            curve,
            r_series_ohm,
            value_offset,
            v_supply,
        }
    }
}

impl Sensor for CalibratedVariableResistanceAnalogSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for CalibratedVariableResistanceAnalogSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let v_supply = f32::from_bits(self.v_supply.load(Ordering::Relaxed));
        let v_adc_pin = input as f32 / SENDER_ADC_MAX_CODE * SENDER_ADC_VREF;
        let v_sensor_wire = v_adc_pin
            * (SENDER_DIVIDER_R1_OHM + SENDER_DIVIDER_R2_OHM) / SENDER_DIVIDER_R2_OHM;
        let headroom = v_supply - v_sensor_wire;
        if headroom <= SENDER_FAULT_MARGIN_V {
            return Err(format!(
                "{}: sensor-wire {:.3} V at/above supply {:.3} V (disconnected sender or ADC noise)",
                self.metadata.sensor_id, v_sensor_wire, v_supply
            ));
        }
        let r_sender = self.r_series_ohm * v_sensor_wire / headroom;
        let short_fault_ohm = (self.curve[0].0 * SENDER_SHORT_FAULT_CURVE_FRACTION)
            .max(SENDER_SHORT_FAULT_MIN_OHM);
        if r_sender < short_fault_ohm {
            return Err(format!(
                "{}: sensor-wire {:.3} V implausibly low (r≈{:.1} Ω < {:.1} Ω) -- divider input wire disconnected?",
                self.metadata.sensor_id, v_sensor_wire, r_sender, short_fault_ohm
            ));
        }
        let value = interpolate_curve(&self.curve, r_sender) + self.value_offset;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            value.clamp(self.min_value(), self.max_value()),
            self.constraints.clone(),
            self.metadata.clone(),
        );
        Ok(&self.value)
    }
}

/// Wraps the `Hw12v` chain's sensor and republishes each successful reading into a shared
/// cell, so `CalibratedVariableResistanceAnalogSensor` chains can read the live supply
/// voltage their raw→Ω conversion needs (SENSOR_CALIBRATION_DESIGN.md, "Cross-sensor
/// dependency"). Publishing only on a successful read leaves the last-known-good voltage in
/// place across an ADC link drop. Everything except `read` delegates to the inner sensor.
pub struct SupplyVoltagePublisher {
    inner: Box<dyn AnalogSensor + Send>,
    cell: Arc<AtomicU32>,
}

impl SupplyVoltagePublisher {
    pub fn new(inner: Box<dyn AnalogSensor + Send>, cell: Arc<AtomicU32>) -> Self {
        SupplyVoltagePublisher { inner, cell }
    }
}

impl Sensor for SupplyVoltagePublisher {
    fn id(&self) -> &String { self.inner.id() }
    fn name(&self) -> &String { self.inner.name() }
    fn value(&self) -> Result<&SensorValue, String> { self.inner.value() }
    fn constraints(&self) -> &ValueConstraints { self.inner.constraints() }
    fn metadata(&self) -> &ValueMetadata { self.inner.metadata() }
    fn min_value(&self) -> f32 { self.inner.min_value() }
    fn max_value(&self) -> f32 { self.inner.max_value() }
}

impl AnalogSensor for SupplyVoltagePublisher {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let volts = self.inner.read(input)?.as_f32();
        self.cell.store(volts.to_bits(), Ordering::Relaxed);
        self.inner.value()
    }
}

/// Converts a DS18B20 raw temperature register (1/16 °C, signed) to °C. A plain
/// GenericAnalogSensor can't express this: its `input as f32 * scale` treats the value as
/// unsigned, so a sub-zero reading (which arrives as a large u16 bit pattern across the
/// HWAnalogProvider boundary — see OneWireTempChannelProvider) would come out as a huge
/// positive number. The `(input as i16)` reinterpret here undoes that, same trick as
/// UpsCurrentSensor. Name and thresholds come from sensor_config.json; the unit is always
/// °C and the /16 divisor is intrinsic to the wire format, so neither is a parameter.
pub struct OneWireTempSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl OneWireTempSensor {
    pub fn new(id: String, name: String, constraints: ValueConstraints) -> Self {
        OneWireTempSensor {
            value: SensorValue::empty(),
            constraints,
            metadata: ValueMetadata::new("°C", name, id),
        }
    }
}

impl Sensor for OneWireTempSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl AnalogSensor for OneWireTempSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let celsius = (input as i16) as f32 / 16.0;
        self.value = SensorValue::analog_with_constraints_and_metadata(
            celsius.clamp(self.constraints.min_value, self.constraints.max_value),
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
// for extended periods, and dips <-200 mA under load - -300 mA accounts for that.
pub const UPS_ON_BATTERY_CURRENT_THRESHOLD_MA: f32 = -300.0;

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
/// to be able to represent unambiguously at the low-speed end (see design doc). The STM32
/// firmware already latches the last measured period across reports and only reports this
/// sentinel once SPEED_TIMEOUT_US has elapsed since the last real edge (see main.cpp) -- so
/// it, not a second Rust-side staleness check, is authoritative for "vehicle stopped."
/// (An earlier Rust-side staleness timeout keyed off "has the read() input value changed"
/// was removed: at a perfectly steady speed the raw period legitimately repeats forever,
/// which that check misread as staleness and zeroed the reading within ~100ms.)
const SPEED_PERIOD_IDLE_RAW: u16 = 0;

pub struct SpeedSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl SpeedSensor {
    pub fn new() -> Self {
        SpeedSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(0.0, 180.0),
            metadata: ValueMetadata::new("км/ч", "СКОР", "speed_sensor"),
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
        let speed_kmh = if input == SPEED_PERIOD_IDLE_RAW {
            0.0
        } else {
            Self::speed_kmh_from_period_raw(input)
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
/// since startup) -- mirrors SPEED_PERIOD_IDLE_RAW, same rationale for trusting the
/// firmware's own TACHO_TIMEOUT_US latch instead of a second Rust-side staleness check.
const TACHO_PERIOD_IDLE_RAW: u16 = 0;

pub struct TachoSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl TachoSensor {
    pub fn new() -> Self {
        TachoSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(0.0, 6000.0),
            metadata: ValueMetadata::new("об/мин", "ТАХОМЕТР", "tacho_sensor"),
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
        let rpm = if input == TACHO_PERIOD_IDLE_RAW {
            0.0
        } else {
            Self::rpm_from_period_raw(input)
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
        // that must NOT be treated as staleness -- an earlier Rust-side staleness timeout
        // keyed off "has the input value changed" misread a steady speed as stale and
        // zeroed the reading after ~100ms. The STM32 firmware is the sole authority on
        // "no recent pulse": it latches the period across reports and only reports
        // SPEED_PERIOD_IDLE_RAW itself once SPEED_TIMEOUT_US has elapsed since the last
        // real edge, so a sensor cruising at constant speed must read correctly forever.
        let mut sensor = SpeedSensor::new();
        let raw = raw_period_for_100_kmh();
        for _ in 0..30 {
            let speed = sensor.read(raw).unwrap().as_f32();
            assert!((speed - 100.0).abs() < 1.0, "expected ~100 km/h, got {}", speed);
            std::thread::sleep(Duration::from_millis(16));
        }
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
        // that must NOT be treated as staleness. Mirrors the SpeedSensor case: the STM32
        // firmware is the sole authority on "engine stalled" (TACHO_TIMEOUT_US), so a
        // steady rpm reading must not decay to 0 just because the value stopped changing.
        let mut sensor = TachoSensor::new();
        let raw = raw_period_for_3000_rpm();
        for _ in 0..30 {
            let rpm = sensor.read(raw).unwrap().as_f32();
            assert!((rpm - 3000.0).abs() < 10.0, "expected ~3000 rpm, got {}", rpm);
            std::thread::sleep(Duration::from_millis(16));
        }
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
    fn test_voltage_divider_sensor_converts_code_to_volts() {
        let mut sensor = VoltageDividerSensor::new(
            "Hw12v".to_string(), "БОРТ СЕТЬ".to_string(), "В".to_string(),
            ValueConstraints::analog(0.0, 20.0), 1.0,
        );
        // factor = 3.3/4095 * 61/10 ≈ 0.0049159 V/code; 12.6 V -> ~2563 codes.
        let volts = sensor.read(2563).unwrap().as_f32();
        assert!((volts - 12.6).abs() < 0.05, "expected ~12.6 V, got {volts}");
    }

    #[test]
    fn test_voltage_divider_sensor_trim_scales_output() {
        let mut plain = VoltageDividerSensor::new(
            "v".to_string(), "v".to_string(), "В".to_string(),
            ValueConstraints::analog(0.0, 30.0), 1.0,
        );
        let mut trimmed = VoltageDividerSensor::new(
            "v".to_string(), "v".to_string(), "В".to_string(),
            ValueConstraints::analog(0.0, 30.0), 1.05,
        );
        let base = plain.read(2000).unwrap().as_f32();
        let scaled = trimmed.read(2000).unwrap().as_f32();
        assert!((scaled - base * 1.05).abs() < 0.001, "trim should scale linearly");
    }

    #[test]
    fn test_voltage_divider_sensor_clamps_to_constraints() {
        let mut sensor = VoltageDividerSensor::new(
            "v".to_string(), "v".to_string(), "В".to_string(),
            ValueConstraints::analog(0.0, 20.0), 1.0,
        );
        // Full-scale code ~20.13 V raw -> clamped to the 20.0 V gauge max.
        assert_eq!(sensor.read(4095).unwrap().as_f32(), 20.0);
    }

    /// Test-local inverse of the raw→Ω conversion (Ω→raw), for driving
    /// CalibratedVariableResistanceAnalogSensor from a known resistance. Deliberately not
    /// shared production code -- see SENSOR_CALIBRATION_DESIGN.md's resolved open question
    /// on a self-test helper.
    fn raw_for_resistance(r_sender: f32, r_series: f32, v_supply: f32) -> u16 {
        let frac = r_sender / (r_series + r_sender);
        let v_sensor_wire = v_supply * frac;
        let v_adc_pin = v_sensor_wire * SENDER_DIVIDER_R2_OHM
            / (SENDER_DIVIDER_R1_OHM + SENDER_DIVIDER_R2_OHM);
        (v_adc_pin / SENDER_ADC_VREF * SENDER_ADC_MAX_CODE).round().clamp(0.0, 4095.0) as u16
    }

    /// ТМ106 coolant curve (datasheet-band midpoints), abbreviated to the points these
    /// tests exercise.
    fn coolant_curve() -> Vec<(f32, f32)> {
        vec![(58.0, 130.0), (98.0, 110.0), (175.5, 90.0), (335.0, 70.0), (1615.0, 30.0)]
    }

    #[test]
    fn test_calibrated_vr_sensor_interpolates_at_a_curve_point() {
        let v_supply = Arc::new(AtomicU32::new(13.5f32.to_bits()));
        let raw = raw_for_resistance(175.5, 110.4, 13.5); // -> 90 °C
        let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
            "HwEngineCoolantTemp".to_string(), "ТЕМП".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 120.0), v_supply,
        );
        let t = sensor.read(raw).unwrap().as_f32();
        assert!((t - 90.0).abs() < 0.5, "expected ~90 °C, got {t}");
    }

    #[test]
    fn test_calibrated_vr_sensor_tracks_live_supply_voltage() {
        // The same raw count implies a different sender resistance -- hence a different
        // reading -- at a different supply voltage. That's the whole reason read() consults
        // the live Hw12v cell instead of a nominal constant.
        let cell = Arc::new(AtomicU32::new(12.2f32.to_bits()));
        let raw = raw_for_resistance(175.5, 110.4, 12.2);
        let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 120.0), cell.clone(),
        );
        let at_12v = sensor.read(raw).unwrap().as_f32();
        cell.store(14.5f32.to_bits(), Ordering::Relaxed);
        let at_14v5 = sensor.read(raw).unwrap().as_f32();
        assert!((at_12v - at_14v5).abs() > 1.0,
                "supply change should move the reading, {at_12v} vs {at_14v5}");
    }

    #[test]
    fn test_calibrated_vr_sensor_faults_when_wire_reaches_supply() {
        let v_supply = Arc::new(AtomicU32::new(12.2f32.to_bits()));
        let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 120.0), v_supply,
        );
        // Full-scale raw -> V_sensor_wire ~16 V, well above the 12.2 V supply.
        assert!(sensor.read(4095).is_err());
    }

    #[test]
    fn test_calibrated_vr_sensor_clamps_past_curve_ends() {
        let v_supply = Arc::new(AtomicU32::new(13.5f32.to_bits()));
        let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 200.0), v_supply, // wide clamp so the curve ends show
        );
        // 35 Ω is below the curve's lowest point (58 Ω) but above the low-side short-fault
        // floor (58 * 0.4 = 23.2 Ω), so it clamps rather than faulting.
        let hot = sensor.read(raw_for_resistance(35.0, 110.4, 13.5)).unwrap().as_f32();
        assert!((hot - 130.0).abs() < 0.001, "below curve start clamps to 130 °C, got {hot}");
        let cold = sensor.read(raw_for_resistance(5000.0, 110.4, 13.5)).unwrap().as_f32();
        assert!((cold - 30.0).abs() < 0.001, "above curve end clamps to 30 °C, got {cold}");
    }

    #[test]
    fn test_calibrated_vr_sensor_faults_on_implausibly_low_resistance() {
        // Mode 3: the PA0/PA1/PA2 divider's input wire floats -> R2 pulls the ADC pin to
        // ~0 V -> decodes to a near-short sender resistance. Must fault, not read full-scale.
        let v_supply = Arc::new(AtomicU32::new(13.5f32.to_bits()));
        let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 120.0), v_supply,
        );
        assert!(sensor.read(0).is_err(), "raw 0 (floating divider input) should fault");
        assert!(sensor.read(raw_for_resistance(2.0, 110.4, 13.5)).is_err());
        // A genuine near-full-scale reading (low but plausible R) still converts.
        assert!(sensor.read(raw_for_resistance(60.0, 110.4, 13.5)).is_ok());
    }

    #[test]
    fn test_calibrated_vr_sensor_applies_value_offset() {
        let v_supply = Arc::new(AtomicU32::new(13.5f32.to_bits()));
        let raw = raw_for_resistance(175.5, 110.4, 13.5);
        let mut base = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 200.0), v_supply.clone(),
        );
        let mut offset = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 5.0,
            ValueConstraints::analog(0.0, 200.0), v_supply,
        );
        let b = base.read(raw).unwrap().as_f32();
        let o = offset.read(raw).unwrap().as_f32();
        assert!((o - b - 5.0).abs() < 0.01, "offset should shift by +5, {b} vs {o}");
    }

    #[test]
    fn test_supply_voltage_publisher_writes_cell_on_successful_read() {
        let cell = Arc::new(AtomicU32::new(0));
        let inner = Box::new(VoltageDividerSensor::new(
            "Hw12v".to_string(), "БОРТ СЕТЬ".to_string(), "В".to_string(),
            ValueConstraints::analog(0.0, 20.0), 1.0,
        ));
        let mut publisher = SupplyVoltagePublisher::new(inner, cell.clone());
        // ~12.6 V worth of codes (see test_voltage_divider_sensor_converts_code_to_volts).
        let volts = publisher.read(2563).unwrap().as_f32();
        let published = f32::from_bits(cell.load(Ordering::Relaxed));
        assert!((published - volts).abs() < 0.001, "cell should hold the reading");
        assert!((published - 12.6).abs() < 0.1, "got {published}");
    }

    #[test]
    fn test_one_wire_temp_sensor_decodes_raw_16ths() {
        let mut sensor = OneWireTempSensor::new(
            "t".to_string(), "НАРУЖ".to_string(),
            ValueConstraints::analog_with_thresholds(-40.0, 80.0, None, None, Some(45.0), None),
        );

        // 404 (1/16 °C) -> 25.25 °C, the live bench reading.
        sensor.read(404).unwrap();
        assert!((Sensor::value(&sensor).unwrap().as_f32() - 25.25).abs() < 0.001);

        // Sub-zero: the STM32 sends raw -88, which arrives across the u16 boundary as its
        // i16 bit pattern; the sensor must reinterpret it back to -5.5 °C.
        sensor.read((-88_i16) as u16).unwrap();
        assert!((Sensor::value(&sensor).unwrap().as_f32() - (-5.5)).abs() < 0.001);

        assert_eq!(sensor.metadata().unit, "°C");
    }

    #[test]
    fn test_one_wire_temp_sensor_clamps_to_constraints() {
        let mut sensor = OneWireTempSensor::new(
            "t".to_string(), "x".to_string(),
            ValueConstraints::analog(-40.0, 60.0),
        );
        sensor.read(2000).unwrap(); // 125.0 °C -> clamped to 60.0
        assert_eq!(Sensor::value(&sensor).unwrap().as_f32(), 60.0);
        sensor.read((-1000_i16) as u16).unwrap(); // -62.5 °C -> clamped to -40.0
        assert_eq!(Sensor::value(&sensor).unwrap().as_f32(), -40.0);
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

        // Test CalibratedVariableResistanceAnalogSensor implements AnalogSensor trait
        let mut temp_sensor = CalibratedVariableResistanceAnalogSensor::new(
            "c".to_string(), "c".to_string(), "°C".to_string(),
            110.4, coolant_curve(), 0.0,
            ValueConstraints::analog(0.0, 120.0),
            Arc::new(AtomicU32::new(13.5f32.to_bits())),
        );
        assert_eq!(temp_sensor.min_value(), 0.0);
        assert_eq!(temp_sensor.max_value(), 120.0);

        let temp_result = temp_sensor.read(raw_for_resistance(175.5, 110.4, 13.5));
        assert!(temp_result.is_ok());

        let temp_value_result = Sensor::value(&temp_sensor);
        assert!(temp_value_result.is_ok());
    }
}