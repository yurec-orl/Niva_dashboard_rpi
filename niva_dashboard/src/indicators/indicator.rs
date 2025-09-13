use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::UIStyle;

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

impl SensorValue {
    /// Create a new sensor value with full context
    pub fn new(value: ValueData, constraints: ValueConstraints, metadata: ValueMetadata) -> Self {
        Self { value, constraints, metadata }
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
    
    /// Get the numeric value as f32
    pub fn as_f32(&self) -> f32 {
        match self.value {
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
            ValueData::Digital(b) => b,
            ValueData::Analog(v) => v > self.constraints.min_value,
            ValueData::Percentage(p) => p > 0.0,
            ValueData::Integer(i) => i > 0,
        }
    }
}

/// Position and size information for indicator rendering
#[derive(Debug, Clone, Copy)]
pub struct IndicatorBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl IndicatorBounds {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    
    /// Get center point of the bounds
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// Main indicator trait for rendering various dashboard indicators
pub trait Indicator {
    /// Render the indicator with the given value, bounds, style and graphics context
    /// 
    /// # Parameters
    /// - `value`: The sensor value with its constraints and metadata
    /// - `bounds`: Position and size constraints for the indicator
    /// - `style`: UI styling parameters (colors, fonts, sizes, etc.)
    /// - `context`: Graphics context for OpenGL rendering operations
    fn render(&self, 
              value: &SensorValue, 
              bounds: IndicatorBounds, 
              style: &UIStyle, 
              context: &mut GraphicsContext) -> Result<(), String>;
    /// Get indicator type name for debugging and configuration
    fn indicator_type(&self) -> &'static str;
    
    /// Check if indicator can handle the given value type efficiently
    fn supports_value_type(&self, value: &ValueData) -> bool {
        // Individual indicators can override for optimization
        false
    }
}