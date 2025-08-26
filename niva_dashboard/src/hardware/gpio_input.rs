use rppal::gpio::{Gpio, InputPin, Level, Bias, Result as GpioResult};
use std::error::Error;
use std::fmt;

/// Represents the logical state of a GPIO input pin
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinState {
    High,
    Low,
}

impl From<Level> for PinState {
    fn from(level: Level) -> Self {
        match level {
            Level::High => PinState::High,
            Level::Low => PinState::Low,
        }
    }
}

impl fmt::Display for PinState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinState::High => write!(f, "HIGH"),
            PinState::Low => write!(f, "LOW"),
        }
    }
}

/// Configuration for a GPIO input pin
#[derive(Debug, Clone)]
pub struct GpioInputConfig {
    pub pin_number: u8,
    pub bias: Bias,
    pub active_low: bool,  // If true, LOW level means active/pressed
}

impl Default for GpioInputConfig {
    fn default() -> Self {
        Self {
            pin_number: 2,
            bias: Bias::PullUp,
            active_low: true,  // Common for buttons with pull-up resistors
        }
    }
}

/// GPIO input reader for digital inputs
pub struct GpioInput {
    pin: InputPin,
    config: GpioInputConfig,
}

impl GpioInput {
    /// Create a new GPIO input with the specified configuration
    pub fn new(config: GpioInputConfig) -> GpioResult<Self> {
        let gpio = Gpio::new()?;
        let mut pin = gpio.get(config.pin_number)?.into_input();
        
        // Configure pull-up/pull-down resistor
        pin.set_bias(config.bias);
        
        Ok(Self { pin, config })
    }
    
    /// Create a new GPIO input with default configuration for the specified pin
    pub fn new_with_pin(pin_number: u8) -> GpioResult<Self> {
        let config = GpioInputConfig {
            pin_number,
            ..Default::default()
        };
        Self::new(config)
    }
    
    /// Read the raw pin level (High/Low)
    pub fn read_raw(&self) -> PinState {
        self.pin.read().into()
    }
    
    /// Read the logical state considering active_low configuration
    /// Returns true when the input is considered "active" (e.g., button pressed)
    pub fn read_logical(&self) -> bool {
        let raw_state = self.read_raw();
        if self.config.active_low {
            raw_state == PinState::Low
        } else {
            raw_state == PinState::High
        }
    }
    
    /// Get the pin number
    pub fn pin_number(&self) -> u8 {
        self.config.pin_number
    }
    
    /// Get the configuration
    pub fn config(&self) -> &GpioInputConfig {
        &self.config
    }
    
    /// Check if the pin is configured as active low
    pub fn is_active_low(&self) -> bool {
        self.config.active_low
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pin_state_conversion() {
        assert_eq!(PinState::from(Level::High), PinState::High);
        assert_eq!(PinState::from(Level::Low), PinState::Low);
    }
    
    #[test]
    fn test_pin_state_display() {
        assert_eq!(format!("{}", PinState::High), "HIGH");
        assert_eq!(format!("{}", PinState::Low), "LOW");
    }
    
    #[test]
    fn test_config_default() {
        let config = GpioInputConfig::default();
        assert_eq!(config.pin_number, 2);
        assert_eq!(config.bias, Bias::PullUp);
        assert!(config.active_low);
    }
}
