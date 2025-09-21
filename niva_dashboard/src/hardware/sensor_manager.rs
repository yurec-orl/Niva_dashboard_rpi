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
//! let brake_active = manager.read_digital_sensor(HWInput::ParkBrake(Level::Low))?;
//! ```

use crate::hardware::sensors::{AnalogSensor, DigitalSensor};
use crate::hardware::hw_providers::{HWInput, HWAnalogProvider, HWDigitalProvider};
use crate::hardware::analog_signal_processing::AnalogSignalProcessor;
use crate::hardware::digital_signal_processing::DigitalSignalProcessor;
use crate::hardware::sensor_value::SensorValue;

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
    sensor_values: Vec<(HWInput, SensorValue)>,
}

impl SensorManager {
    pub fn new() -> Self {
        SensorManager {
            digital_sensors: Vec::new(),
            analog_sensors: Vec::new(),
            sensor_values: Vec::new(),
        }
    }

    pub fn add_digital_sensor_chain(&mut self, chain: SensorDigitalInputChain) {
        self.digital_sensors.push(chain);
    }

    pub fn add_analog_sensor_chain(&mut self, chain: SensorAnalogInputChain) {
        self.analog_sensors.push(chain);
    }

    fn read_digital_sensor(&mut self, input: HWInput) -> Result<SensorValue, String> {
        for chain in &mut self.digital_sensors {
            if chain.hw_provider.input() != input {
                continue;
            }
            // Read raw input from hardware provider
            let mut level = chain.hw_provider.read_digital(input.clone())?;
            
            // Process through signal processors
            for processor in &mut chain.signal_processors {
                level = processor.read(level)?;
            }
            
            // Convert to logical sensor value
            return Ok(chain.sensor.read(level)?.clone());
        }
        Err(format!("Digital sensor chain not found for input: {:?}", input))
    }

    fn read_analog_sensor(&mut self, input: HWInput) -> Result<SensorValue, String> {
        for chain in &mut self.analog_sensors {
            if chain.hw_provider.input() != input {
                continue;
            }
            // Read raw input from hardware provider
            let mut value = chain.hw_provider.read_analog(input.clone())?;
            
            // Process through signal processors
            for processor in &mut chain.signal_processors {
                value = processor.read(value)?;
            }
            
            // Convert to logical sensor value
            return Ok(chain.sensor.read(value)?.clone());
        }
        Err("Analog sensor chain not found".to_string())
    }

    // Should be called periodically from event loop to update all sensors
    pub fn read_all_sensors(&mut self) -> Result<(), String> {
        self.sensor_values.clear();

        // Collect inputs first to avoid borrowing issues
        let digital_inputs: Vec<HWInput> = self.digital_sensors.iter()
            .map(|chain| chain.hw_provider.input())
            .collect();
        let analog_inputs: Vec<HWInput> = self.analog_sensors.iter()
            .map(|chain| chain.hw_provider.input())
            .collect();

        // Read digital sensors
        for input in digital_inputs {
            let value = self.read_digital_sensor(input)?;
            //print!("Read digital sensor {:?}: {:?}\r\n", input, value);
            self.sensor_values.push((input, value));
        }

        // Read analog sensors  
        for input in analog_inputs {
            let value = self.read_analog_sensor(input)?;
            //print!("Read analog sensor {:?}: {:?}\r\n", input, value);
            self.sensor_values.push((input, value));
        }

        Ok(())
    }

    pub fn get_sensor_values(&self) -> &Vec<(HWInput, SensorValue)> {
        &self.sensor_values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::hw_providers::{TestDigitalDataProvider, TestAnalogDataProvider};
    use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
    use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
    use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor};
    use crate::hardware::sensor_value::ValueConstraints;
    use rppal::gpio::Level;
    use std::time::Duration;

    #[test]
    fn test_sensor_manager_digital_chain() {
        println!("=== Testing Digital Sensor Chain ===");
        
        let mut manager = SensorManager::new();
        
        // Create test digital input for park brake (active low)
        let park_brake_input = HWInput::HwParkBrake;
        
        // Create digital sensor chain components
        let hw_provider = Box::new(TestDigitalDataProvider::new(park_brake_input));
        let debouncer = Box::new(DigitalSignalDebouncer::new(3, Duration::from_millis(50)));
        let sensor = Box::new(GenericDigitalSensor::new("test_park_brake".to_string(), "Test Park Brake".to_string(), 
                                                        Level::Low, ValueConstraints::digital_warning())); // Active low sensor
        
        // Create and add the chain
        let chain = SensorDigitalInputChain::new(
            hw_provider,
            vec![debouncer],
            sensor,
        );
        manager.add_digital_sensor_chain(chain);
        
        // Test reading the sensor during active period (first 4 seconds)
        let result = manager.read_digital_sensor(park_brake_input);
        assert!(result.is_ok(), "Digital sensor read should succeed");
        
        let sensor_value = result.unwrap();
        println!("Park brake active (during active period): {}", sensor_value.is_active());
        
        // In TestDigitalDataProvider, the first 4 seconds return the active level (Low)
        // Since our sensor is active-low, it should read as true (active)
        assert!(sensor_value.is_active(), "Park brake should be active during test period");
        
        println!("✓ Digital sensor chain test passed");
    }

    #[test]
    fn test_sensor_manager_analog_chain() {
        println!("=== Testing Analog Sensor Chain ===");
        
        let mut manager = SensorManager::new();
        
        // Create test analog input for fuel level
        let fuel_input = HWInput::HwFuelLvl;
        
        // Create analog sensor chain components
        let hw_provider = Box::new(TestAnalogDataProvider::new(fuel_input));
        let moving_avg = Box::new(AnalogSignalProcessorMovingAverage::new(3));
        let sensor = Box::new(GenericAnalogSensor::new(
            "test_fuel".to_string(), "Test Fuel Level".to_string(), "%".to_string(),
            ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None),
            1.0
        ));
        
        // Create and add the chain
        let chain = SensorAnalogInputChain::new(
            hw_provider,
            vec![moving_avg],
            sensor,
        );
        manager.add_analog_sensor_chain(chain);
        
        // Test reading the sensor
        let result = manager.read_analog_sensor(fuel_input);
        assert!(result.is_ok(), "Analog sensor read should succeed");
        
        let sensor_value = result.unwrap();
        println!("Fuel level: {:.1}%", sensor_value.as_f32());
        
        // Fuel percentage should be between 0 and 100
        let fuel_percentage = sensor_value.as_f32();
        assert!(fuel_percentage >= 0.0 && fuel_percentage <= 100.0, 
               "Fuel percentage should be between 0 and 100");
        
        println!("✓ Analog sensor chain test passed");
    }

    #[test]
    fn test_sensor_manager_multiple_chains() {
        println!("=== Testing Multiple Sensor Chains ===");
        
        let mut manager = SensorManager::new();
        
        // Add digital chain for high beam indicator (active high)
        let high_beam_input = HWInput::HwHighBeam;
        let digital_chain = SensorDigitalInputChain::new(
            Box::new(TestDigitalDataProvider::new(high_beam_input)),
            vec![], // No signal processing for this test
            Box::new(GenericDigitalSensor::new("test_high_beam".to_string(), "Test High Beam".to_string(),
                                              Level::High, ValueConstraints::digital_default())), // Active high sensor
        );
        manager.add_digital_sensor_chain(digital_chain);
        
        // Add analog chain for temperature
        let temp_input = HWInput::HwEngineCoolantTemp;
        let analog_chain = SensorAnalogInputChain::new(
            Box::new(TestAnalogDataProvider::new(temp_input)),
            vec![Box::new(AnalogSignalProcessorMovingAverage::new(5))],
            Box::new(GenericAnalogSensor::new(
                "test_temp".to_string(), "Test Temperature".to_string(), "°C".to_string(),
                ValueConstraints::analog_with_thresholds(-40.0, 120.0, Some(-20.0), Some(0.0), Some(100.0), Some(110.0)),
                1.0
            )),
        );
        manager.add_analog_sensor_chain(analog_chain);
        
        // Test both sensors
        let high_beam_result = manager.read_digital_sensor(high_beam_input);
        let temp_result = manager.read_analog_sensor(temp_input);
        
        assert!(high_beam_result.is_ok(), "High beam sensor should work");
        assert!(temp_result.is_ok(), "Temperature sensor should work");
        
        let high_beam_sensor_value = high_beam_result.unwrap();
        let temp_sensor_value = temp_result.unwrap();
        
        println!("High beam active: {}", high_beam_sensor_value.is_active());
        println!("Temperature: {:.1}°C", temp_sensor_value.as_f32());
        
        // Validate ranges
        let temperature = temp_sensor_value.as_f32();
        assert!(temperature >= -40.0 && temperature <= 120.0, 
               "Temperature should be within sensor range");
        
        println!("✓ Multiple sensor chains test passed");
    }

    #[test]
    fn test_sensor_manager_nonexistent_chain() {
        println!("=== Testing Non-existent Sensor Chain ===");
        
        let mut manager = SensorManager::new();
        
        // Try to read from a sensor that wasn't configured
        let non_existent_input = HWInput::HwSpeed;
        let result = manager.read_digital_sensor(non_existent_input);
        
        assert!(result.is_err(), "Reading non-existent sensor should fail");
        
        let error_msg = result.unwrap_err();
        println!("Expected error: {}", error_msg);
        assert!(error_msg.contains("Digital sensor chain not found"), 
               "Error should indicate missing chain");
        
        println!("✓ Non-existent sensor chain test passed");
    }

    #[test] 
    fn test_sensor_manager_signal_processing_pipeline() {
        println!("=== Testing Signal Processing Pipeline ===");
        
        let mut manager = SensorManager::new();
        
        // Create a chain with multiple signal processors
        let turn_signal_input = HWInput::HwTurnSignal;
        
        // Add two debounce stages for extra filtering
        let debouncer1 = Box::new(DigitalSignalDebouncer::new(2, Duration::from_millis(25)));
        let debouncer2 = Box::new(DigitalSignalDebouncer::new(2, Duration::from_millis(25)));
        
        let chain = SensorDigitalInputChain::new(
            Box::new(TestDigitalDataProvider::new(turn_signal_input)),
            vec![debouncer1, debouncer2], // Multiple processors in sequence
            Box::new(GenericDigitalSensor::new("test_turn_signal".to_string(), "Test Turn Signal".to_string(),
                                              Level::High, ValueConstraints::digital_default())),
        );
        manager.add_digital_sensor_chain(chain);
        
        // Read the sensor to verify the pipeline works
        let result = manager.read_digital_sensor(turn_signal_input);
        assert!(result.is_ok(), "Signal processing pipeline should work");
        
        let sensor_value = result.unwrap();
        println!("Turn signal active (after processing): {}", sensor_value.is_active());
        
        println!("✓ Signal processing pipeline test passed");
    }
}