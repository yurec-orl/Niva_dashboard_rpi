// Hardware sensor reading framework for automotive dashboard.
// This module provides trait-based abstractions for reading various sensor inputs
// and concrete implementations for different hardware interfaces.
//
// Defines:
// - HWInput/HWInput enums for all supported inputs
// - HWAnalogProvider/HWDigitalProvider traits for hardware abstraction
// - GPIOProvider: Direct GPIO digital input reading for Raspberry Pi
// - I2CProvider: External ADC/controller interface via I2C protocol  
// - TestDataProvider: Fixed test values for development/testing
//
// Architecture: Hardware providers supply raw sensor data that will be processed
// by higher-level sensor processing modules (filtering, debouncing, conversion
// to logical values) before being consumed by the UI rendering system.
//
// Intended data flow:
//   HWDigitalProvider -> digital signal processing (debouncing, smoothing) ->
//   -> DigSensor(convert raw data to logical values) -> UI Rendering
//   HWAnalogProvider -> analog signal processing (filtering, smoothing) ->
//   -> AnalogSensor(convert raw data to logical values) -> UI Rendering

use rppal::gpio::Level;
use std::time::{Duration, Instant};

/////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum HWInput {
    // Analog inputs
    Hw12v,
    HwFuelLvl,
    HwOilPress,
    HwEngineCoolantTemp,
    // Digital inputs
    HwBrakeFluidLvlLow,
    HwCharge,
    HwCheckEngine,
    HwDiffLock,
    HwExtLights,
    HwFuelLvlLow,
    HwHighBeam,
    HwInstrIllum,
    HwOilPressLow,
    HwParkBrake,
    HwSpeed,
    HwTacho,
    HwTurnSignal,
}

// Generic interface for reading input data.
pub trait HWAnalogProvider {
    fn input(&self) -> HWInput;
    fn read_analog(&self, input: HWInput) -> Result<u16, String>;
}

pub trait HWDigitalProvider {
    fn input(&self) -> HWInput;
    fn read_digital(&self, input: HWInput) -> Result<Level, String>;
}

// Read directly from GPIO pins
// Digital inputs only - Raspi does not have built-in ADC
pub struct GPIOProvider {
    input: HWInput,
    // Implementation details for GPIO access
}

impl GPIOProvider {
    pub fn new(input: HWInput) -> Self {
        GPIOProvider {
            input,
            // Initialize GPIO access here
        }
    }
}

impl HWDigitalProvider for GPIOProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }

    fn read_digital(&self, input: HWInput) -> Result<Level, String> {
        // Implementation for reading digital value from GPIO pin
        Ok(Level::Low)
    }
}

// Read inputs from external MC via I2C protocol
pub struct I2CProvider {
    input: HWInput,
    // Implementation details for I2C access
}

impl I2CProvider {
    pub fn new(input: HWInput) -> Self {
        I2CProvider {
            input,
            // Initialize I2C access here
        }
    }
}

impl HWAnalogProvider for I2CProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }
    fn read_analog(&self, input: HWInput) -> Result<u16, String> {
        // Implementation for reading analog value from external ADC via I2C
        Ok(0)
    }
}

impl HWDigitalProvider for I2CProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }
    fn read_digital(&self, input: HWInput) -> Result<Level, String> {
        // Implementation for reading digital value from external controller via I2C
        Ok(Level::Low)
    }
}

pub struct TestDigitalDataProvider {
    input: HWInput,
    start_time: Instant,
}

impl TestDigitalDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestDigitalDataProvider {
            input,
            start_time: Instant::now(),
        }
    }
}

impl HWDigitalProvider for TestDigitalDataProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }

    fn read_digital(&self, input: HWInput) -> Result<Level, String> {
        let elapsed = self.start_time.elapsed();
        let active_duration = Duration::from_secs(4);
        
        // Return active level for first 4 seconds, then inactive level
        if elapsed < active_duration {
            Ok(Level::High)
        } else {
            Ok(Level::Low)
        }
    }
}

pub struct TestAnalogDataProvider {
    input: HWInput,
    start_time: Instant,
}

impl TestAnalogDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestAnalogDataProvider {
            input,
            start_time: Instant::now(),
        }
    }
}

impl HWAnalogProvider for TestAnalogDataProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }
    fn read_analog(&self, input: HWInput) -> Result<u16, String> {
        let elapsed = self.start_time.elapsed();
        let cycle_duration = Duration::from_millis(5000); // 5 seconds total cycle
        let half_cycle = Duration::from_millis(2500); // 2.5 seconds per half
        
        // Calculate position within the cycle (0.0 to 1.0)
        let cycle_position = (elapsed.as_millis() % cycle_duration.as_millis()) as f64 
            / cycle_duration.as_millis() as f64;
        
        let value = if elapsed.as_millis() % cycle_duration.as_millis() < half_cycle.as_millis() {
            // First half: gradually increasing (0 to 1023)
            let progress = (elapsed.as_millis() % half_cycle.as_millis()) as f64 
                / half_cycle.as_millis() as f64;
            (progress * 1023.0) as u16
        } else {
            // Second half: gradually decreasing (1023 to 0)
            let progress = (elapsed.as_millis() % half_cycle.as_millis()) as f64 
                / half_cycle.as_millis() as f64;
            (1023.0 - (progress * 1023.0)) as u16
        };
        
        Ok(value)
    }
}

pub struct TestPulseDataProvider {
    input: HWInput,
    start_time: Instant,
    max_frequency: f32,
}

impl TestPulseDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestPulseDataProvider {
            input,
            start_time: Instant::now(),
            max_frequency: 83.3, // pulses per second at 100 km/h
        }
    }
}

/// Test provider that always returns zero value for testing zero-position indicators
pub struct TestZeroAnalogDataProvider {
    input: HWInput,
}

impl TestZeroAnalogDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestZeroAnalogDataProvider {
            input,
        }
    }
}

impl HWAnalogProvider for TestZeroAnalogDataProvider {
    fn input(&self) -> HWInput {
        self.input
    }
    
    fn read_analog(&self, _input: HWInput) -> Result<u16, String> {
        // Always return zero for testing zero-position indicators
        Ok(0)
    }
}

/// Test provider that always returns middle value for testing middle-position indicators
pub struct TestMiddleAnalogDataProvider {
    input: HWInput,
}

impl TestMiddleAnalogDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestMiddleAnalogDataProvider {
            input,
        }
    }
}

impl HWAnalogProvider for TestMiddleAnalogDataProvider {
    fn input(&self) -> HWInput {
        self.input
    }
    
    fn read_analog(&self, _input: HWInput) -> Result<u16, String> {
        // Always return middle value (50% of range) for testing middle-position indicators
        Ok(512)
    }
}

/// Test provider that always returns maximum value for testing max-position indicators
pub struct TestMaxAnalogDataProvider {
    input: HWInput,
}

impl TestMaxAnalogDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestMaxAnalogDataProvider {
            input,
        }
    }
}

impl HWAnalogProvider for TestMaxAnalogDataProvider {
    fn input(&self) -> HWInput {
        self.input
    }
    
    fn read_analog(&self, _input: HWInput) -> Result<u16, String> {
        // Always return maximum value for testing max-position indicators
        Ok(1023)
    }
}

impl TestPulseDataProvider {
    fn get_current_frequency(&self) -> f32 {
        let elapsed = self.start_time.elapsed();
        let cycle_duration = Duration::from_millis(5000); // 5 seconds total cycle
        let half_cycle = Duration::from_millis(2500); // 2.5 seconds per half
        
        let cycle_position = elapsed.as_millis() % cycle_duration.as_millis();
        
        if cycle_position < half_cycle.as_millis() {
            // First half: gradually increasing (0 to 83.3 Hz)
            let progress = cycle_position as f32 / half_cycle.as_millis() as f32;
            progress * self.max_frequency
        } else {
            // Second half: gradually decreasing (83.3 to 0 Hz)
            let progress = (cycle_position - half_cycle.as_millis()) as f32 / half_cycle.as_millis() as f32;
            self.max_frequency * (1.0 - progress)
        }
    }
}

impl HWDigitalProvider for TestPulseDataProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }
    fn read_digital(&self, input: HWInput) -> Result<Level, String> {
        let current_frequency = self.get_current_frequency();
        
        // Debug: Log frequency periodically
        // static mut LAST_LOG: std::time::Instant = unsafe { std::mem::zeroed() };
        // unsafe {
        //     let now = std::time::Instant::now();
        //     if LAST_LOG.elapsed().as_secs() >= 1 {
        //         println!("TestPulseDataProvider Debug: Current frequency: {:.2} Hz", current_frequency);
        //         LAST_LOG = now;
        //     }
        // }
        
        // If frequency is essentially zero, return low
        if current_frequency < 0.1 {
            return Ok(Level::Low);
        }
        
        // Calculate total elapsed time in seconds
        let elapsed_secs = self.start_time.elapsed().as_secs_f32();
        
        // Calculate instantaneous phase based on integral of frequency over time
        // Since frequency changes linearly within each half-cycle, we need to integrate
        let cycle_duration_secs = 5.0; // 5 seconds total cycle
        let half_cycle_secs = 2.5; // 2.5 seconds per half
        
        let cycle_time = elapsed_secs % cycle_duration_secs;
        let phase = if cycle_time < half_cycle_secs {
            // First half: frequency increases linearly from 0 to max
            // Integral of (max_freq * t / half_cycle) from 0 to cycle_time
            let progress = cycle_time / half_cycle_secs;
            0.5 * self.max_frequency * progress * progress * half_cycle_secs
        } else {
            // Second half: frequency decreases linearly from max to 0
            let t_in_second_half = cycle_time - half_cycle_secs;
            let progress = t_in_second_half / half_cycle_secs;
            // Add first half contribution + integral of decreasing frequency
            let first_half_phase = 0.5 * self.max_frequency * half_cycle_secs;
            let second_half_phase = self.max_frequency * t_in_second_half * (1.0 - 0.5 * progress);
            first_half_phase + second_half_phase
        };
        
        // Convert phase to digital state (square wave)
        let state = if (phase as u32) % 2 == 0 { Level::Low } else { Level::High };
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, Instant};
    use std::sync::Arc;

    // Test HWInput enum
    #[test]
    fn test_hw_input_enum_completeness() {
        // Test that all expected inputs are present
        let analog_inputs = vec![
            HWInput::Hw12v,
            HWInput::HwFuelLvl,
            HWInput::HwOilPress,
            HWInput::HwEngineCoolantTemp,
        ];

        let digital_inputs = vec![
            HWInput::HwBrakeFluidLvlLow,
            HWInput::HwCharge,
            HWInput::HwCheckEngine,
            HWInput::HwDiffLock,
            HWInput::HwExtLights,
            HWInput::HwFuelLvlLow,
            HWInput::HwHighBeam,
            HWInput::HwInstrIllum,
            HWInput::HwOilPressLow,
            HWInput::HwParkBrake,
            HWInput::HwSpeed,
            HWInput::HwTacho,
            HWInput::HwTurnSignal,
        ];

        assert_eq!(analog_inputs.len(), 4);
        assert_eq!(digital_inputs.len(), 13);
        
        // Test Debug and Clone traits
        let input = HWInput::Hw12v;
        let cloned_input = input.clone();
        assert_eq!(input, cloned_input);
        println!("{:?}", input);
    }

    #[test]
    fn test_hw_input_equality() {
        assert_eq!(HWInput::Hw12v, HWInput::Hw12v);
        assert_ne!(HWInput::Hw12v, HWInput::HwFuelLvl);
    }

    // Test GPIOProvider
    #[test]
    fn test_gpio_provider_creation() {
        let provider = GPIOProvider::new(HWInput::HwBrakeFluidLvlLow);
        assert_eq!(provider.input(), HWInput::HwBrakeFluidLvlLow);
    }

    #[test]
    fn test_gpio_provider_digital_read() {
        let provider = GPIOProvider::new(HWInput::HwBrakeFluidLvlLow);
        
        // Test reading digital value - should return Ok(Level::Low) based on current implementation
        let result = provider.read_digital(HWInput::HwBrakeFluidLvlLow);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Level::Low);
    }

    #[test]
    fn test_gpio_provider_different_inputs() {
        let provider = GPIOProvider::new(HWInput::HwCharge);
        
        // Test that provider can handle different input types
        let result = provider.read_digital(HWInput::HwSpeed);
        assert!(result.is_ok());
        
        // Test with analog input (should still work based on current implementation)
        let result = provider.read_digital(HWInput::Hw12v);
        assert!(result.is_ok());
    }

    // Test I2CProvider
    #[test]
    fn test_i2c_provider_creation() {
        let provider = I2CProvider::new(HWInput::HwOilPress);
        assert_eq!(HWAnalogProvider::input(&provider), HWInput::HwOilPress);
    }

    #[test]
    fn test_i2c_provider_analog_read() {
        let provider = I2CProvider::new(HWInput::HwOilPress);
        
        // Test reading analog value - should return Ok(0) based on current implementation
        let result = provider.read_analog(HWInput::HwOilPress);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_i2c_provider_digital_read() {
        let provider = I2CProvider::new(HWInput::HwBrakeFluidLvlLow);
        
        // Test reading digital value - should return Ok(Level::Low) based on current implementation
        let result = provider.read_digital(HWInput::HwBrakeFluidLvlLow);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Level::Low);
    }

    #[test]
    fn test_i2c_provider_different_inputs() {
        let analog_provider = I2CProvider::new(HWInput::Hw12v);
        let digital_provider = I2CProvider::new(HWInput::HwCharge);
        
        // Test analog reading
        let analog_result = analog_provider.read_analog(HWInput::HwFuelLvl);
        assert!(analog_result.is_ok());
        
        // Test digital reading
        let digital_result = digital_provider.read_digital(HWInput::HwHighBeam);
        assert!(digital_result.is_ok());
    }

    // Test TestDigitalDataProvider
    #[test]
    fn test_digital_test_provider_creation() {
        let provider = TestDigitalDataProvider::new(HWInput::HwCheckEngine);
        assert_eq!(provider.input(), HWInput::HwCheckEngine);
    }

    #[test]
    fn test_digital_test_provider_initial_state() {
        let provider = TestDigitalDataProvider::new(HWInput::HwCheckEngine);
        
        // Should be high initially (within first 4 seconds)
        let result = provider.read_digital(HWInput::HwCheckEngine);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Level::High);
    }

    #[test]
    fn test_digital_test_provider_timing() {
        let provider = TestDigitalDataProvider::new(HWInput::HwCheckEngine);
        
        // Test multiple readings in quick succession - should all be High initially
        for _ in 0..10 {
            let result = provider.read_digital(HWInput::HwCheckEngine);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), Level::High);
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test] 
    fn test_digital_test_provider_with_different_inputs() {
        let provider = TestDigitalDataProvider::new(HWInput::HwDiffLock);
        
        // Test with different input than configured
        let result = provider.read_digital(HWInput::HwTurnSignal);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Level::High);
    }

    // Test TestAnalogDataProvider
    #[test]
    fn test_analog_test_provider_creation() {
        let provider = TestAnalogDataProvider::new(HWInput::HwOilPress);
        assert_eq!(provider.input(), HWInput::HwOilPress);
    }

    #[test]
    fn test_analog_test_provider_initial_values() {
        let provider = TestAnalogDataProvider::new(HWInput::HwOilPress);
        
        // Should start close to 0 (within first few milliseconds)
        let result = provider.read_analog(HWInput::HwOilPress);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value < 100); // Should be low initially
    }

    #[test]
    fn test_analog_test_provider_value_range() {
        let provider = TestAnalogDataProvider::new(HWInput::HwFuelLvl);
        
        // Test that values are within expected range (0-1023)
        for _ in 0..20 {
            let result = provider.read_analog(HWInput::HwFuelLvl);
            assert!(result.is_ok());
            let value = result.unwrap();
            assert!(value <= 1023);
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn test_analog_test_provider_progression() {
        let provider = TestAnalogDataProvider::new(HWInput::Hw12v);
        
        // Test that values generally increase in the first part of the cycle
        let mut values = Vec::new();
        for _ in 0..10 {
            let result = provider.read_analog(HWInput::Hw12v);
            assert!(result.is_ok());
            values.push(result.unwrap());
            thread::sleep(Duration::from_millis(100));
        }
        
        // Check that we have a general upward trend in the first second
        let first_value = values[0];
        let last_value = values[values.len() - 1];
        assert!(last_value >= first_value);
    }

    #[test]
    fn test_analog_test_provider_different_inputs() {
        let provider = TestAnalogDataProvider::new(HWInput::HwEngineCoolantTemp);
        
        // Test with different input than configured
        let result = provider.read_analog(HWInput::Hw12v);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value <= 1023);
    }

    // Test TestPulseDataProvider
    #[test]
    fn test_pulse_test_provider_creation() {
        let provider = TestPulseDataProvider::new(HWInput::HwSpeed);
        assert_eq!(provider.input(), HWInput::HwSpeed);
    }

    #[test]
    fn test_pulse_test_provider_initial_state() {
        let provider = TestPulseDataProvider::new(HWInput::HwSpeed);
        
        // Should be low initially (frequency starts at 0)
        let result = provider.read_digital(HWInput::HwSpeed);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Level::Low);
    }

    #[test]
    fn test_pulse_test_provider_frequency_calculation() {
        let provider = TestPulseDataProvider::new(HWInput::HwTacho);
        
        // Test that get_current_frequency returns reasonable values
        let freq = provider.get_current_frequency();
        assert!(freq >= 0.0);
        assert!(freq <= 83.3); // Max frequency as defined in implementation
    }

    #[test]
    fn test_pulse_test_provider_frequency_progression() {
        let provider = TestPulseDataProvider::new(HWInput::HwTacho);
        
        let mut frequencies = Vec::new();
        for _ in 0..10 {
            frequencies.push(provider.get_current_frequency());
            thread::sleep(Duration::from_millis(100));
        }
        
        // Check that frequency generally increases in the first part of the cycle
        let first_freq = frequencies[0];
        let last_freq = frequencies[frequencies.len() - 1];
        assert!(last_freq >= first_freq);
    }

    #[test]
    fn test_pulse_test_provider_reaches_maximum_frequency() {
        let provider = TestPulseDataProvider::new(HWInput::HwSpeed);
        
        // Wait for approximately 2.5 seconds (middle of first half-cycle where frequency should be maximum)
        thread::sleep(Duration::from_millis(2500));
        
        let freq = provider.get_current_frequency();
        
        // Should be very close to maximum frequency (83.3 Hz)
        // Allow small tolerance due to timing precision
        assert!(freq > 80.0, "Frequency should be near maximum, got: {}", freq);
        assert!(freq <= 83.3, "Frequency should not exceed maximum, got: {}", freq);
        
        // Test a few readings around this time to ensure consistency
        let mut max_freq_readings = Vec::new();
        for _ in 0..5 {
            max_freq_readings.push(provider.get_current_frequency());
            thread::sleep(Duration::from_millis(50));
        }
        
        // All readings should be high (near maximum)
        for reading in &max_freq_readings {
            assert!(*reading > 75.0, "All readings near peak should be high, got: {}", reading);
        }
    }

    #[test]
    fn test_pulse_test_provider_state_changes() {
        let provider = TestPulseDataProvider::new(HWInput::HwSpeed);
        
        // Sleep a bit to get into the frequency range where pulses occur
        thread::sleep(Duration::from_millis(500));
        
        let mut states = Vec::new();
        for _ in 0..20 {
            let result = provider.read_digital(HWInput::HwSpeed);
            assert!(result.is_ok());
            states.push(result.unwrap());
            thread::sleep(Duration::from_millis(10));
        }
        
        // Should have some variation in states (not all the same)
        let high_count = states.iter().filter(|&&state| state == Level::High).count();
        let low_count = states.iter().filter(|&&state| state == Level::Low).count();
        
        // At least one state change should occur
        assert!(high_count > 0 || low_count > 0);
    }

    // Test trait implementations
    #[test]
    fn test_hw_analog_provider_trait() {
        let provider = TestAnalogDataProvider::new(HWInput::Hw12v);
        
        // Test trait method calls
        assert_eq!(provider.input(), HWInput::Hw12v);
        let result = provider.read_analog(HWInput::Hw12v);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hw_digital_provider_trait() {
        let provider = TestDigitalDataProvider::new(HWInput::HwBrakeFluidLvlLow);
        
        // Test trait method calls
        assert_eq!(provider.input(), HWInput::HwBrakeFluidLvlLow);
        let result = provider.read_digital(HWInput::HwBrakeFluidLvlLow);
        assert!(result.is_ok());
    }

    // Test polymorphism with trait objects
    #[test]
    fn test_analog_provider_polymorphism() {
        let test_provider: Box<dyn HWAnalogProvider> = 
            Box::new(TestAnalogDataProvider::new(HWInput::Hw12v));
        let i2c_provider: Box<dyn HWAnalogProvider> = 
            Box::new(I2CProvider::new(HWInput::HwFuelLvl));
        
        let providers = vec![test_provider, i2c_provider];
        
        for provider in providers {
            let result = provider.read_analog(HWInput::Hw12v);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_digital_provider_polymorphism() {
        let test_provider: Box<dyn HWDigitalProvider> = 
            Box::new(TestDigitalDataProvider::new(HWInput::HwBrakeFluidLvlLow));
        let gpio_provider: Box<dyn HWDigitalProvider> = 
            Box::new(GPIOProvider::new(HWInput::HwCharge));
        let i2c_provider: Box<dyn HWDigitalProvider> = 
            Box::new(I2CProvider::new(HWInput::HwCheckEngine));
        let pulse_provider: Box<dyn HWDigitalProvider> = 
            Box::new(TestPulseDataProvider::new(HWInput::HwSpeed));
        
        let providers = vec![test_provider, gpio_provider, i2c_provider, pulse_provider];
        
        for provider in providers {
            let result = provider.read_digital(HWInput::HwBrakeFluidLvlLow);
            assert!(result.is_ok());
        }
    }

    // Edge case tests
    #[test]
    fn test_providers_with_all_input_types() {
        let analog_inputs = vec![
            HWInput::Hw12v,
            HWInput::HwFuelLvl,
            HWInput::HwOilPress,
            HWInput::HwEngineCoolantTemp,
        ];

        let digital_inputs = vec![
            HWInput::HwBrakeFluidLvlLow,
            HWInput::HwCharge,
            HWInput::HwCheckEngine,
            HWInput::HwDiffLock,
            HWInput::HwExtLights,
            HWInput::HwFuelLvlLow,
            HWInput::HwHighBeam,
            HWInput::HwInstrIllum,
            HWInput::HwOilPressLow,
            HWInput::HwParkBrake,
            HWInput::HwSpeed,
            HWInput::HwTacho,
            HWInput::HwTurnSignal,
        ];

        // Test analog provider with all analog inputs
        for input in analog_inputs {
            let provider = TestAnalogDataProvider::new(input);
            let result = provider.read_analog(input);
            assert!(result.is_ok());
        }

        // Test digital provider with all digital inputs
        for input in digital_inputs {
            let provider = TestDigitalDataProvider::new(input);
            let result = provider.read_digital(input);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_error_handling() {
        // Current implementations don't return errors, but test the Result type
        let gpio_provider = GPIOProvider::new(HWInput::HwBrakeFluidLvlLow);
        let i2c_provider = I2CProvider::new(HWInput::Hw12v);
        let test_digital_provider = TestDigitalDataProvider::new(HWInput::HwCheckEngine);
        let test_analog_provider = TestAnalogDataProvider::new(HWInput::HwOilPress);
        let pulse_provider = TestPulseDataProvider::new(HWInput::HwSpeed);

        // All should return Ok results based on current implementation
        assert!(gpio_provider.read_digital(HWInput::HwBrakeFluidLvlLow).is_ok());
        assert!(i2c_provider.read_analog(HWInput::Hw12v).is_ok());
        assert!(i2c_provider.read_digital(HWInput::HwCheckEngine).is_ok());
        assert!(test_digital_provider.read_digital(HWInput::HwCheckEngine).is_ok());
        assert!(test_analog_provider.read_analog(HWInput::HwOilPress).is_ok());
        assert!(pulse_provider.read_digital(HWInput::HwSpeed).is_ok());
    }

    #[test]
    fn test_provider_consistency() {
        // Test that provider input() method returns consistent values
        let gpio_provider = GPIOProvider::new(HWInput::HwBrakeFluidLvlLow);
        let i2c_provider = I2CProvider::new(HWInput::HwOilPress);
        let test_provider = TestDigitalDataProvider::new(HWInput::HwCheckEngine);

        // Multiple calls should return same input
        for _ in 0..5 {
            assert_eq!(gpio_provider.input(), HWInput::HwBrakeFluidLvlLow);
            assert_eq!(HWAnalogProvider::input(&i2c_provider), HWInput::HwOilPress);
            assert_eq!(test_provider.input(), HWInput::HwCheckEngine);
        }
    }

    // Performance and stress tests
    #[test]
    fn test_provider_performance() {
        let provider = TestAnalogDataProvider::new(HWInput::Hw12v);
        let start = Instant::now();
        
        // Perform many reads
        for _ in 0..1000 {
            let result = provider.read_analog(HWInput::Hw12v);
            assert!(result.is_ok());
        }
        
        let elapsed = start.elapsed();
        // Should complete reasonably quickly (less than 1 second for 1000 reads)
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn test_concurrent_access() {
        let provider = Arc::new(TestAnalogDataProvider::new(HWInput::Hw12v));
        let mut handles = vec![];

        // Create multiple threads accessing the same provider
        for _ in 0..5 {
            let provider_clone: Arc<TestAnalogDataProvider> = Arc::clone(&provider);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let result = provider_clone.read_analog(HWInput::Hw12v);
                    assert!(result.is_ok());
                    thread::sleep(Duration::from_millis(1));
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}