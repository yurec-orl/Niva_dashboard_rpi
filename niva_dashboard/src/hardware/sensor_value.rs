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