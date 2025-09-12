use rppal::gpio::Level;

use crate::hardware::digital_signal_processing::DigitalSignalProcessor;

// Digital sensor trait - represents on/off state based on active level
// Active level could be low in case of pull-up input configuration
pub trait DigitalSensor {
    fn active_level(&self) -> Level;

    fn active(&self, input: Level) -> Result<bool, String> {
        Ok(input == self.active_level())
    }
}

pub struct GenericDigitalSensor {
    active_level: Level,
}

impl GenericDigitalSensor {
    pub fn new(active_level: Level) -> Self {
        GenericDigitalSensor { active_level }
    }
}

impl DigitalSensor for GenericDigitalSensor {
    fn active_level(&self) -> Level {
        self.active_level
    }

    fn active(&self, input: Level) -> Result<bool, String> {
        Ok(input == self.active_level)
    }
}

// Analog sensor trait - represents a numeric value based on raw input
// Value should be a processed input, e.g. voltage level converted to temperature
// All voltage divider calculations, pulse count to speed, and other 
// raw input conversion into meaningful values are done here
pub trait AnalogSensor {
    fn value(&self, input: u16) -> Result<f32, String>;

    fn min_value(&self) -> f32 {
        0.0
    }

    fn max_value(&self) -> f32 {
        100.0
    }
}

struct TemperatureSensor {
    // Internal state, e.g. calibration data
}

impl TemperatureSensor {
    fn new() -> Self {
        TemperatureSensor {
            // Initialize internal state
        }
    }
}

impl AnalogSensor for TemperatureSensor {
    fn value(&self, input: u16) -> Result<f32, String> {
        // Convert raw input (e.g. ADC value) to temperature
        // Placeholder conversion logic
        let temperature = (input as f32) * 0.1; // Example conversion
        Ok(temperature)
    }
}

pub struct GenericAnalogSensor {
    min_value: f32,
    max_value: f32,
    scale_factor: f32,
}

impl GenericAnalogSensor {
    pub fn new(min_value: f32, max_value: f32, scale_factor: f32) -> Self {
        GenericAnalogSensor {
            min_value,
            max_value,
            scale_factor,
        }
    }
}

impl AnalogSensor for GenericAnalogSensor {
    fn value(&self, input: u16) -> Result<f32, String> {
        let value = (input as f32) * self.scale_factor;
        Ok(value.clamp(self.min_value, self.max_value))
    }

    fn min_value(&self) -> f32 {
        self.min_value
    }

    fn max_value(&self) -> f32 {
        self.max_value
    }
}