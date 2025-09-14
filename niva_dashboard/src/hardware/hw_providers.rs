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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
            let progress = ((elapsed.as_millis() % half_cycle.as_millis()) as f64 
                / half_cycle.as_millis() as f64);
            (1023.0 - (progress * 1023.0)) as u16
        };
        
        Ok(value)
    }
}

struct TestPulseDataProvider {
    input: HWInput,
    start_time: Instant,
}

impl TestPulseDataProvider {
    pub fn new(input: HWInput) -> Self {
        TestPulseDataProvider {
            input,
            start_time: Instant::now(),
        }
    }
    
    fn get_current_frequency(&self) -> f32 {
        let elapsed = self.start_time.elapsed();
        let cycle_duration = Duration::from_millis(5000); // 5 seconds total cycle
        let half_cycle = Duration::from_millis(2500); // 2.5 seconds per half
        let max_frequency = 83.3; // pulses per second at 100 km/h
        
        let cycle_position = elapsed.as_millis() % cycle_duration.as_millis();
        
        if cycle_position < half_cycle.as_millis() {
            // First half: gradually increasing (0 to 83.3 Hz)
            let progress = cycle_position as f32 / half_cycle.as_millis() as f32;
            progress * max_frequency
        } else {
            // Second half: gradually decreasing (83.3 to 0 Hz)
            let progress = (cycle_position - half_cycle.as_millis()) as f32 / half_cycle.as_millis() as f32;
            max_frequency * (1.0 - progress)
        }
    }
}

impl HWDigitalProvider for TestPulseDataProvider {
    fn input(&self) -> HWInput {
        self.input.clone()
    }
    fn read_digital(&self, input: HWInput) -> Result<Level, String> {
        let current_frequency = self.get_current_frequency();
        
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
        let max_frequency = 83.3;
        
        let cycle_time = elapsed_secs % cycle_duration_secs;
        let phase = if cycle_time < half_cycle_secs {
            // First half: frequency increases linearly from 0 to max
            // Integral of (max_freq * t / half_cycle) from 0 to cycle_time
            let progress = cycle_time / half_cycle_secs;
            0.5 * max_frequency * progress * progress * half_cycle_secs
        } else {
            // Second half: frequency decreases linearly from max to 0
            let t_in_second_half = cycle_time - half_cycle_secs;
            let progress = t_in_second_half / half_cycle_secs;
            // Add first half contribution + integral of decreasing frequency
            let first_half_phase = 0.5 * max_frequency * half_cycle_secs;
            let second_half_phase = max_frequency * t_in_second_half * (1.0 - 0.5 * progress);
            first_half_phase + second_half_phase
        };
        
        // Convert phase to digital state (square wave)
        let state = if (phase as u32) % 2 == 0 { Level::Low } else { Level::High };
        Ok(state)
    }
}