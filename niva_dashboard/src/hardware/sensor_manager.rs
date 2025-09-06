//! Sensor Manager implements a sensor management system using a chain-of-responsibility
//! pattern to handle sensor data from raw hardware inputs to logical values.
//!
//! The system is built around three-stage processing chains:
//!
//! Hardware Provider → Signal Processors → Logical Sensor
//!     (Raw Data)      (Filtering/Smoothing)  (Value Conversion)
//!
//! ### Components
//!
//! 1. **Hardware Providers** (`HWDigitalProvider`/`HWAnalogProvider`)
//!    - Abstract hardware interface layer
//!    - Read raw sensor data from GPIO pins, I2C devices, or test data
//!    - Return unprocessed digital levels (High/Low) or analog values (u16)
//!
//! 2. **Signal Processors** (`DigitalSignalProcessor`/`AnalogSignalProcessor`)
//!    - Pipeline of configurable processing stages
//!    - Digital: debouncing, edge detection, state smoothing
//!    - Analog: moving averages, low-pass filtering, noise reduction
//!    - Multiple processors can be chained for complex signal conditioning
//!
//! 3. **Logical Sensors** (`DigitalSensor`/`AnalogSensor`)
//!    - Convert processed signals to meaningful values
//!    - Digital: map to boolean states (active/inactive) considering polarity
//!    - Analog: convert to physical units (voltage → temperature, ADC → fuel level)
//!
//! ### Sensor Chains
//!
//! **SensorDigitalInputChain**: Hardware → [Processors] → Digital Sensor → bool
//! - Example: GPIO pin → debounce → active-low interpretation → brake_engaged
//!
//! **SensorAnalogInputChain**: Hardware → [Processors] → Analog Sensor → f32
//! - Example: ADC → moving average → voltage divider math → fuel_percentage
//!
//! ### SensorManager
//!
//! - Maintains collections of configured sensor chains
//! - Routes read requests to appropriate chains by input type
//! - Executes the full processing pipeline for each sensor read
//! - Returns processed, ready-to-display values to the UI layer
//!
//! ### Usage
//!
//! ```rust
//! let mut manager = SensorManager::new();
//! 
//! // Configure a digital sensor chain
//! let chain = SensorDigitalInputChain::new(
//!     Box::new(gpio_provider),
//!     vec![Box::new(debouncer)],
//!     Box::new(active_low_sensor)
//! );
//! manager.add_digital_sensor_chain(chain);
//! 
//! // Read processed sensor value
//! let brake_active = manager.read_digital_sensor(HWDigitalInput::ParkBrake(Level::Low))?;
//! ```

use crate::hardware::sensors::{AnalogSensor, DigitalSensor};
use crate::hardware::hw_providers::{HWDigitalInput, HWAnalogInput, HWAnalogProvider, HWDigitalProvider};
use crate::hardware::analog_signal_processing::AnalogSignalProcessor;
use crate::hardware::digital_signal_processing::DigitalSignalProcessor;
use rppal::gpio::Level;

// Sensor management - chains hardware providers, signal processors, and logical sensors
pub struct SensorDigitalInputChain {
    hw_provider: Box<dyn HWDigitalProvider>,
    // Signal processors are applied in sequence
    signal_processors: Vec<Box<dyn DigitalSignalProcessor>>,
    sensor: Box<dyn DigitalSensor>,
}

impl SensorDigitalInputChain {
    pub fn new(
        hw_provider: Box<dyn HWDigitalProvider>,
        signal_processors: Vec<Box<dyn DigitalSignalProcessor>>,
        sensor: Box<dyn DigitalSensor>,
    ) -> Self {
        SensorDigitalInputChain {
            hw_provider,
            signal_processors,
            sensor,
        }
    }
}

// Analog sensor input chain, similar to SensorDigitalInputChain
pub struct SensorAnalogInputChain {
    hw_provider: Box<dyn HWAnalogProvider>,
    // Signal processors are applied in sequence
    signal_processors: Vec<Box<dyn AnalogSignalProcessor>>,
    sensor: Box<dyn AnalogSensor>,
}

impl SensorAnalogInputChain {
    pub fn new(
        hw_provider: Box<dyn HWAnalogProvider>,
        signal_processors: Vec<Box<dyn AnalogSignalProcessor>>,
        sensor: Box<dyn AnalogSensor>,
    ) -> Self {
        SensorAnalogInputChain {
            hw_provider,
            signal_processors,
            sensor,
        }
    }
}

pub struct SensorManager {
    digital_sensors: Vec<SensorDigitalInputChain>,
    analog_sensors: Vec<SensorAnalogInputChain>,
}

impl SensorManager {
    pub fn new() -> Self {
        SensorManager {
            digital_sensors: Vec::new(),
            analog_sensors: Vec::new(),
        }
    }

    pub fn add_digital_sensor_chain(&mut self, chain: SensorDigitalInputChain) {
        self.digital_sensors.push(chain);
    }

    pub fn add_analog_sensor_chain(&mut self, chain: SensorAnalogInputChain) {
        self.analog_sensors.push(chain);
    }

    pub fn read_digital_sensor(&mut self, input: HWDigitalInput) -> Result<bool, String> {
        for chain in &mut self.digital_sensors {
            if (chain.hw_provider.input() != input) {
                continue;
            }
            // Read raw input from hardware provider
            let mut level = chain.hw_provider.read_digital(input.clone())?;
            
            // Process through signal processors
            for processor in &mut chain.signal_processors {
                level = processor.read(level)?;
            }
            
            // Convert to logical sensor value
            return chain.sensor.active(level);
        }
        Err(format!("Digital sensor chain not found for input: {:?}", input))
    }

    pub fn read_analog_sensor(&mut self, input: HWAnalogInput) -> Result<f32, String> {
        for chain in &mut self.analog_sensors {
            if (chain.hw_provider.input() != input) {
                continue;
            }
            // Read raw input from hardware provider
            let mut value = chain.hw_provider.read_analog(input.clone())?;
            
            // Process through signal processors
            for processor in &mut chain.signal_processors {
                value = processor.read(value)?;
            }
            
            // Convert to logical sensor value
            return chain.sensor.value(value);
        }
        Err("Analog sensor chain not found".to_string())
    }
}