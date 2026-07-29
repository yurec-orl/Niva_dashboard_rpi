#![allow(dead_code)]
/// Enhanced sensor value that carries both the value and its context/constraints
#[derive(Debug, Clone, PartialEq)]
pub struct SensorValue {
    pub value: ValueData,
    pub constraints: ValueConstraints,
    pub metadata: ValueMetadata,
}

/// The actual sensor value data
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueData {
    Empty,
    /// Digital/boolean value (on/off, active/inactive)
    Digital(bool),
    /// Analog value (temperature, pressure, voltage, etc.)
    Analog(f32),
    /// Percentage value (0.0 to 100.0)
    Percentage(f32),
    /// Integer value (RPM, count, etc.)
    Integer(i32),
}

/// Constraints and range information for the value
#[derive(Debug, Clone, PartialEq)]
pub struct ValueConstraints {
    /// Minimum expected value
    pub min_value: f32,
    /// Maximum expected value  
    pub max_value: f32,
    /// Critical low threshold (red zone)
    pub critical_low: Option<f32>,
    /// Warning low threshold (yellow zone)
    pub warning_low: Option<f32>,
    /// Warning high threshold (yellow zone)
    pub warning_high: Option<f32>,
    /// Critical high threshold (red zone)
    pub critical_high: Option<f32>,
}

impl ValueConstraints {
    pub fn new(min_value: Option<f32>, max_value: Option<f32>,
               critical_low: Option<f32>, warning_low: Option<f32>,
               warning_high: Option<f32>, critical_high: Option<f32>) -> Self {
        ValueConstraints {
            min_value: min_value.unwrap_or(0.0),
            max_value: max_value.unwrap_or(100.0),
            critical_low,
            warning_low,
            warning_high,
            critical_high,
        }
    }

    // Default 0-1 range, no critical/warning thresholds
    // For sensors like turn signals, headlights, etc.
    pub fn digital_default() -> Self {
        ValueConstraints {
            min_value: 0.0,
            max_value: 1.0,
            critical_low: None,
            warning_low: None,
            warning_high: None,
            critical_high: None,
        }
    }

    // 0-1 range, with critical level at 1.0 (brake fluid, oil pressure or battery charge)
    // For sensors which are active when condition is critical
    pub fn digital_critical() -> Self {
        ValueConstraints {
            min_value: 0.0,
            max_value: 1.0,
            critical_low: None,
            warning_low: None,
            warning_high: None,
            critical_high: Some(1.0),
        }
    }

    // 0-1 range, with warning level at zero
    // (parking brake, high beam, diff lock indicators)
    // For sensors which are active when condition is non-critical
    pub fn digital_warning() -> Self {
        ValueConstraints {
            min_value: 0.0,
            max_value: 1.0,
            critical_low: None,
            warning_low: None,
            warning_high: Some(1.0),
            critical_high: None,
        }
    }

    // Parametrized analog sensor constraints
    pub fn analog(min_value: f32, max_value: f32) -> Self {
        ValueConstraints {
            min_value,
            max_value,
            critical_low: None,
            warning_low: None,
            warning_high: None,
            critical_high: None,
        }
    }

    pub fn analog_with_thresholds(min_value: f32, max_value: f32,
                                  critical_low: Option<f32>, warning_low: Option<f32>,
                                  warning_high: Option<f32>, critical_high: Option<f32>) -> Self {
        ValueConstraints {
            min_value,
            max_value,
            critical_low,
            warning_low,
            warning_high,
            critical_high,
        }
    }
}

/// Additional metadata about the sensor value
#[derive(Debug, Clone, PartialEq)]
pub struct ValueMetadata {
    /// Unit of measurement ("°C", "kgf/cm²", "V", "RPM", etc.)
    pub unit: String,
    /// Human-readable label ("Engine Temp", "Oil Pressure", etc.)
    pub label: String,
    /// Sensor identifier for debugging
    pub sensor_id: String,
}

impl ValueMetadata {
    pub fn new(unit: impl Into<String>, label: impl Into<String>, sensor_id: impl Into<String>) -> Self {
        Self {
            unit: unit.into(),
            label: label.into(),
            sensor_id: sensor_id.into(),
        }
    }
}

impl SensorValue {
    /// Create a new sensor value with full context
    pub fn new(value: ValueData, constraints: ValueConstraints, metadata: ValueMetadata) -> Self {
        Self { value, constraints, metadata }
    }

    /// Create empty sensor value
    pub fn empty() -> Self {
        Self {
            value: ValueData::Empty,
            constraints: ValueConstraints::new(None, None, None, None, None, None),
            metadata: ValueMetadata {
                unit: String::new(),
                label: String::new(),
                sensor_id: String::new(),
            },
        }
    }
    
    /// Create a digital sensor value
    pub fn digital(value: bool, label: impl Into<String>, sensor_id: impl Into<String>) -> Self {
        Self {
            value: ValueData::Digital(value),
            constraints: ValueConstraints {
                min_value: 0.0,
                max_value: 1.0,
                critical_low: None,
                warning_low: None,
                warning_high: None,
                critical_high: None,
            },
            metadata: ValueMetadata {
                unit: String::new(),
                label: label.into(),
                sensor_id: sensor_id.into(),
            },
        }
    }

    pub fn digital_with_constraints_and_metadata(
        value: bool,
        constraints: ValueConstraints,
        metadata: ValueMetadata
    ) -> Self {
        Self {
            value: ValueData::Digital(value),
            constraints,
            metadata,
        }
    }

    /// Create an analog sensor value
    pub fn analog(
        value: f32,
        min_value: f32, 
        max_value: f32,
        unit: impl Into<String>,
        label: impl Into<String>,
        sensor_id: impl Into<String>
    ) -> Self {
        Self {
            value: ValueData::Analog(value),
            constraints: ValueConstraints {
                min_value,
                max_value,
                critical_low: None,
                warning_low: None,
                warning_high: None,
                critical_high: None,
            },
            metadata: ValueMetadata {
                unit: unit.into(),
                label: label.into(),
                sensor_id: sensor_id.into(),
            },
        }
    }
    
    /// Create an analog sensor value with warning thresholds
    pub fn analog_with_thresholds(
        value: f32,
        min_value: f32,
        max_value: f32,
        warning_low: Option<f32>,
        warning_high: Option<f32>,
        critical_low: Option<f32>,
        critical_high: Option<f32>,
        unit: impl Into<String>,
        label: impl Into<String>,
        sensor_id: impl Into<String>
    ) -> Self {
        Self {
            value: ValueData::Analog(value),
            constraints: ValueConstraints {
                min_value,
                max_value,
                critical_low,
                warning_low,
                warning_high,
                critical_high,
            },
            metadata: ValueMetadata {
                unit: unit.into(),
                label: label.into(),
                sensor_id: sensor_id.into(),
            },
        }
    }

    pub fn analog_with_constraints_and_metadata(
        value: f32,
        constraints: ValueConstraints,
        metadata: ValueMetadata
    ) -> Self {
        Self {
            value: ValueData::Analog(value),
            constraints,
            metadata,
        }
    }
    
    /// Get the numeric value as f32
    pub fn as_f32(&self) -> f32 {
        match self.value {
            ValueData::Empty => f32::NAN,
            ValueData::Digital(b) => if b { 1.0 } else { 0.0 },
            ValueData::Analog(v) => v,
            ValueData::Percentage(p) => p,
            ValueData::Integer(i) => i as f32,
        }
    }
    
    /// Get value as percentage of range (0.0 to 1.0)
    pub fn as_normalized(&self) -> f32 {
        let val = self.as_f32();
        if self.constraints.max_value == self.constraints.min_value {
            0.0
        } else {
            ((val - self.constraints.min_value) / 
             (self.constraints.max_value - self.constraints.min_value))
             .clamp(0.0, 1.0)
        }
    }
    
    /// Check if value is in critical range
    pub fn is_critical(&self) -> bool {
        let val = self.as_f32();
        if let Some(crit_low) = self.constraints.critical_low {
            if val <= crit_low { return true; }
        }
        if let Some(crit_high) = self.constraints.critical_high {
            if val >= crit_high { return true; }
        }
        false
    }
    
    /// Check if value is in warning range
    pub fn is_warning(&self) -> bool {
        if self.is_critical() { return false; } // Critical overrides warning
        let val = self.as_f32();
        if let Some(warn_low) = self.constraints.warning_low {
            if val <= warn_low { return true; }
        }
        if let Some(warn_high) = self.constraints.warning_high {
            if val >= warn_high { return true; }
        }
        false
    }
    
    /// Check if value represents an "active" state
    pub fn is_active(&self) -> bool {
        match self.value {
            ValueData::Empty => false,
            ValueData::Digital(b) => b,
            ValueData::Analog(v) => v > self.constraints.min_value,
            ValueData::Percentage(p) => p > 0.0,
            ValueData::Integer(i) => i > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_f32_for_every_value_data_variant() {
        assert!(SensorValue::empty().as_f32().is_nan());
        assert_eq!(SensorValue::digital(true, "l", "id").as_f32(), 1.0);
        assert_eq!(SensorValue::digital(false, "l", "id").as_f32(), 0.0);
        assert_eq!(SensorValue::analog(42.5, 0.0, 100.0, "u", "l", "id").as_f32(), 42.5);

        let percentage = SensorValue::new(ValueData::Percentage(37.0), ValueConstraints::analog(0.0, 100.0), ValueMetadata::new("", "", ""));
        assert_eq!(percentage.as_f32(), 37.0);

        let integer = SensorValue::new(ValueData::Integer(-5), ValueConstraints::analog(-10.0, 10.0), ValueMetadata::new("", "", ""));
        assert_eq!(integer.as_f32(), -5.0);
    }

    #[test]
    fn test_as_normalized_clamps_to_0_1_range() {
        let value = SensorValue::analog(50.0, 0.0, 100.0, "", "", "");
        assert_eq!(value.as_normalized(), 0.5);

        let below_min = SensorValue::analog(-20.0, 0.0, 100.0, "", "", "");
        assert_eq!(below_min.as_normalized(), 0.0, "values below min_value must clamp to 0.0, not go negative");

        let above_max = SensorValue::analog(150.0, 0.0, 100.0, "", "", "");
        assert_eq!(above_max.as_normalized(), 1.0, "values above max_value must clamp to 1.0");
    }

    #[test]
    fn test_as_normalized_avoids_division_by_zero_when_range_is_degenerate() {
        let value = SensorValue::analog(5.0, 10.0, 10.0, "", "", "");
        assert_eq!(value.as_normalized(), 0.0, "min_value == max_value must not panic/NaN from a 0/0 division");
    }

    #[test]
    fn test_is_critical_low_and_high_thresholds() {
        let low = SensorValue::analog_with_thresholds(5.0, 0.0, 100.0, None, None, Some(10.0), Some(90.0), "", "", "");
        assert!(low.is_critical(), "value at/below critical_low must be critical");

        let high = SensorValue::analog_with_thresholds(95.0, 0.0, 100.0, None, None, Some(10.0), Some(90.0), "", "", "");
        assert!(high.is_critical(), "value at/above critical_high must be critical");

        let normal = SensorValue::analog_with_thresholds(50.0, 0.0, 100.0, None, None, Some(10.0), Some(90.0), "", "", "");
        assert!(!normal.is_critical());

        let no_thresholds = SensorValue::analog(9999.0, 0.0, 100.0, "", "", "");
        assert!(!no_thresholds.is_critical(), "no critical thresholds configured means never critical");
    }

    #[test]
    fn test_is_critical_boundary_values_are_inclusive() {
        let at_critical_low = SensorValue::analog_with_thresholds(10.0, 0.0, 100.0, None, None, Some(10.0), None, "", "", "");
        assert!(at_critical_low.is_critical(), "critical_low comparison is <=, so the exact threshold counts");

        let at_critical_high = SensorValue::analog_with_thresholds(90.0, 0.0, 100.0, None, None, None, Some(90.0), "", "", "");
        assert!(at_critical_high.is_critical(), "critical_high comparison is >=, so the exact threshold counts");
    }

    #[test]
    fn test_is_warning_low_and_high_thresholds() {
        let low = SensorValue::analog_with_thresholds(25.0, 0.0, 100.0, Some(30.0), Some(70.0), None, None, "", "", "");
        assert!(low.is_warning());

        let high = SensorValue::analog_with_thresholds(75.0, 0.0, 100.0, Some(30.0), Some(70.0), None, None, "", "", "");
        assert!(high.is_warning());

        let normal = SensorValue::analog_with_thresholds(50.0, 0.0, 100.0, Some(30.0), Some(70.0), None, None, "", "", "");
        assert!(!normal.is_warning());
    }

    #[test]
    fn test_critical_overrides_warning() {
        // Critical thresholds are wider than warning thresholds, so a value deep enough to
        // be critical is also past the warning threshold — is_warning must still say false.
        let value = SensorValue::analog_with_thresholds(
            98.0, 0.0, 100.0,
            Some(70.0), Some(90.0), // warning_low, warning_high
            Some(10.0), Some(95.0), // critical_low, critical_high
            "", "", "",
        );
        assert!(value.is_critical());
        assert!(!value.is_warning(), "is_critical() overriding is_warning() means both should never report true together");
    }

    #[test]
    fn test_is_active_for_every_value_data_variant() {
        assert!(!SensorValue::empty().is_active());
        assert!(SensorValue::digital(true, "l", "id").is_active());
        assert!(!SensorValue::digital(false, "l", "id").is_active());

        let percentage_active = SensorValue::new(ValueData::Percentage(1.0), ValueConstraints::analog(0.0, 100.0), ValueMetadata::new("", "", ""));
        assert!(percentage_active.is_active());
        let percentage_inactive = SensorValue::new(ValueData::Percentage(0.0), ValueConstraints::analog(0.0, 100.0), ValueMetadata::new("", "", ""));
        assert!(!percentage_inactive.is_active());

        let integer_active = SensorValue::new(ValueData::Integer(1), ValueConstraints::analog(0.0, 100.0), ValueMetadata::new("", "", ""));
        assert!(integer_active.is_active());
        let integer_inactive = SensorValue::new(ValueData::Integer(0), ValueConstraints::analog(0.0, 100.0), ValueMetadata::new("", "", ""));
        assert!(!integer_inactive.is_active());
    }

    #[test]
    fn test_is_active_for_analog_is_strictly_greater_than_min_value() {
        let at_min = SensorValue::analog(10.0, 10.0, 100.0, "", "", "");
        assert!(!at_min.is_active(), "analog value exactly at min_value must not count as active");

        let above_min = SensorValue::analog(10.01, 10.0, 100.0, "", "", "");
        assert!(above_min.is_active());
    }

    #[test]
    fn test_value_constraints_presets() {
        let default = ValueConstraints::digital_default();
        assert_eq!((default.min_value, default.max_value), (0.0, 1.0));
        assert_eq!(default.critical_high, None);
        assert_eq!(default.warning_high, None);

        let critical = ValueConstraints::digital_critical();
        assert_eq!(critical.critical_high, Some(1.0));
        assert_eq!(critical.warning_high, None);

        let warning = ValueConstraints::digital_warning();
        assert_eq!(warning.warning_high, Some(1.0));
        assert_eq!(warning.critical_high, None);
    }

    #[test]
    fn test_digital_critical_and_warning_presets_drive_is_critical_is_warning() {
        // A sensor using digital_critical() (e.g. brake fluid low) should read critical when active.
        let critical_active = SensorValue::digital_with_constraints_and_metadata(
            true, ValueConstraints::digital_critical(), ValueMetadata::new("", "", ""),
        );
        assert!(critical_active.is_critical());
        assert!(!critical_active.is_warning());

        // A sensor using digital_warning() (e.g. parking brake engaged) should read warning when active.
        let warning_active = SensorValue::digital_with_constraints_and_metadata(
            true, ValueConstraints::digital_warning(), ValueMetadata::new("", "", ""),
        );
        assert!(!warning_active.is_critical());
        assert!(warning_active.is_warning());

        // Inactive (false) must trigger neither, for both presets.
        let inactive = SensorValue::digital_with_constraints_and_metadata(
            false, ValueConstraints::digital_warning(), ValueMetadata::new("", "", ""),
        );
        assert!(!inactive.is_critical());
        assert!(!inactive.is_warning());
    }

    #[test]
    fn test_empty_sensor_value_is_never_critical_or_warning() {
        let empty = SensorValue::empty();
        assert!(!empty.is_critical());
        assert!(!empty.is_warning());
        assert!(!empty.is_active());
    }
}