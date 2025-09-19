use rppal::gpio::Level;

use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueMetadata};
use crate::hardware::digital_signal_processing::{DigitalSignalProcessor, DigitalSignalProcessorPulsePerSecond};

// Used by all sensor types
pub trait Sensor {
    fn id(&self) -> &String;
    fn name(&self) -> &String;
    fn value(&self) -> Result<&SensorValue, String>;
    fn constraints(&self) -> &ValueConstraints;
    fn metadata(&self) -> &ValueMetadata;
}

// Digital sensor trait - represents on/off state based on active level
// Active level could be low in case of pull-up input configuration
pub trait DigitalSensor: Sensor {
    fn active_level(&self) -> Level;

    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: Level) -> Result<&SensorValue, String>;

    // Get last sensor value without modifying state
    fn value(&self) -> Result<&SensorValue, String>;
}

// Analog sensor trait - represents a numeric value based on raw input
// Value should be a processed input, e.g. voltage level converted to temperature
// All voltage divider calculations, pulse count to speed, and other 
// raw input conversion into meaningful values are done here
pub trait AnalogSensor: Sensor {

    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: u16) -> Result<&SensorValue, String>;

    // Get last sensor value without modifying state
    fn value(&self) -> Result<&SensorValue, String>;

    fn min_value(&self) -> f32 {
        0.0
    }

    fn max_value(&self) -> f32 {
        100.0
    }
}

pub struct GenericDigitalSensor {
    id: String,
    name: String,
    value: SensorValue,
    active_level: Level,
    constraints: ValueConstraints,
}

impl GenericDigitalSensor {
    pub fn new(id: String, name: String, active_level: Level,
               constraints: ValueConstraints) -> Self {
        GenericDigitalSensor { id, name, value: SensorValue::empty(),
                               active_level, constraints}
    }
}

impl Sensor for GenericDigitalSensor {
    fn id(&self) -> &String {
        &self.value.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.value.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.value.metadata
    }
}

impl DigitalSensor for GenericDigitalSensor {
    fn active_level(&self) -> Level {
        self.active_level
    }

    fn read(&mut self, input: Level) -> Result<&SensorValue, String> {
        self.value = SensorValue::digital(input == self.active_level, self.name.clone(), self.id.clone());
        Ok(&self.value)
    }

    fn value(&self) -> Result<&SensorValue, String> {
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
    pub fn new(constraints: ValueConstraints, metadata: ValueMetadata, 
               scale_factor: f32) -> Self {
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
}

impl AnalogSensor for GenericAnalogSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let value = (input as f32) * self.scale_factor;
        self.value = SensorValue::analog(value.clamp(self.min_value(), self.max_value()),
                                         self.min_value(), self.max_value(), 
                                         &self.metadata.unit,
                                         &self.metadata.label,
                                         &self.metadata.sensor_id);
        Ok(&self.value)
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

struct EngineTemperatureSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl EngineTemperatureSensor {
    fn new() -> Self {
        EngineTemperatureSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog_with_thresholds(
                0.0, 120.0,
                None, Some(100.0),
                None, Some(110.0),
            ),
            metadata: ValueMetadata {
                unit: "°C".to_string(),
                label: "ТЕМП".to_string(),
                sensor_id: "engine_temp".to_string(),
            },
        }
    }
}

impl Sensor for EngineTemperatureSensor {
    fn id(&self) -> &String {
        &self.value.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.value.metadata.label
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
}

impl AnalogSensor for EngineTemperatureSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        // Convert raw input (e.g. ADC value) to temperature
        // Placeholder conversion logic
        let temperature = (input as f32) * 0.1; // Example conversion
        self.value = SensorValue::analog(temperature.clamp(self.constraints.min_value, self.constraints.max_value),
                                         self.constraints.min_value, self.constraints.max_value,
                                         &self.metadata.unit, &self.metadata.label, &self.metadata.sensor_id);
        Ok(&self.value)
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.value)
    }
}

pub struct SpeedSensor {
    speed: SensorValue,
    pulse_counter: DigitalSignalProcessorPulsePerSecond,
    pulses_per_revolution: u32,
    wheel_circumference_m: f32,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl SpeedSensor {
    pub fn new() -> Self {
        // Physical parameters for 235/75/15 tire
        // Width: 235mm, Aspect ratio: 75%, Rim: 15 inches
        // Diameter = 15" (381mm) + 2 * (235mm * 0.75) = 733.5mm
        // Circumference = π * 733.5mm = 2.304 meters
        SpeedSensor {
            speed: SensorValue::empty(),
            pulse_counter: DigitalSignalProcessorPulsePerSecond::new(),
            pulses_per_revolution: 6, // 6 pulses per wheel rotation
            wheel_circumference_m: 2.304, // meters
            constraints: ValueConstraints::analog(0.0, 180.0),
            metadata: ValueMetadata {
                unit: "км/ч".to_string(),
                label: "СКОР".to_string(),
                sensor_id: "speed_sensor".to_string(),
            },
        }
    }
    
    /// Process a digital input pulse and return current speed
    pub fn process_pulse(&mut self, pulse: Level) -> f32 {
        // Process the pulse through the counter (using DigitalSignalProcessor trait)
        let _ = self.pulse_counter.read(pulse);
        
        // Get current pulses per second
        let pulses_per_second = self.pulse_counter.pulses_per_second();
        
        // Calculate and return speed
        self.speed = SensorValue::analog(self.calculate_speed_kmh(pulses_per_second),
            self.constraints.min_value.clone(), self.constraints.max_value.clone(),
            &self.metadata.unit, &self.metadata.label, &self.metadata.sensor_id);
        self.speed.as_f32()
    }
    
    /// Get current speed without processing new pulses
    pub fn current_speed_kmh(&mut self) -> f32 {
        let pulses_per_second = self.pulse_counter.pulses_per_second();
        self.calculate_speed_kmh(pulses_per_second)
    }
    
    /// Calculate speed in km/h from pulses per second
    pub fn calculate_speed_kmh(&self, pulses_per_second: f32) -> f32 {
        if pulses_per_second <= 0.0 {
            return 0.0;
        }
        
        // Revolutions per second = pulses_per_second / pulses_per_revolution
        let revolutions_per_second = pulses_per_second / self.pulses_per_revolution as f32;
        
        // Distance per second (m/s) = revolutions_per_second * wheel_circumference
        let meters_per_second = revolutions_per_second * self.wheel_circumference_m;
        
        // Convert m/s to km/h: multiply by 3.6
        meters_per_second * 3.6
    }
    
    /// Test function to verify speed calculations with pulse counting
    pub fn test_speed_calculations(&self) {
        println!("SpeedSensor Test Calculations:");
        println!("Wheel circumference: {:.3}m", self.wheel_circumference_m);
        println!("Pulses per revolution: {}", self.pulses_per_revolution);
        println!();
        
        // Test various pulse rates
        let test_cases = [
            (0.0, "Stopped"),
            (5.0, "Very slow"),
            (30.0, "City driving"), 
            (60.0, "Highway driving"),
            (100.0, "Fast highway"),
        ];
        
        for (pulses_per_sec, description) in test_cases {
            let speed = self.calculate_speed_kmh(pulses_per_sec);
            println!("{}: {:.1} pulses/sec = {:.1} km/h", description, pulses_per_sec, speed);
        }
        
        println!();
        println!("Note: In real usage, call process_pulse() with each Level::High/Low transition");
        println!("from the speed sensor hardware to count actual pulses over time.");
    }
    
    /// Test function demonstrating pulse processing
    pub fn test_pulse_processing(&mut self) {
        println!("Testing pulse processing...");
        
        // Simulate some pulses (this would normally come from GPIO)
        for i in 0..12 { // 12 pulses = 2 full revolutions
            let level = if i % 2 == 0 { Level::High } else { Level::Low };
            let current_speed = self.process_pulse(level);
            
            if i % 6 == 5 { // Every full revolution (6 pulses)
                println!("After {} pulses: {:.1} km/h", i + 1, current_speed);
            }
        }
    }
}

impl Sensor for SpeedSensor {
    fn id(&self) -> &String {
        &self.metadata.sensor_id
    }

    fn name(&self) -> &String {
        &self.metadata.label
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.speed)
    }

    fn constraints(&self) -> &ValueConstraints {
        &self.constraints
    }

    fn metadata(&self) -> &ValueMetadata {
        &self.metadata
    }
}

impl DigitalSensor for SpeedSensor {
    fn active_level(&self) -> Level {
        Level::High // Speed sensor pulses are active high
    }

    fn read(&mut self, input: Level) -> Result<&SensorValue, String> {
        self.process_pulse(input);
        Ok(&self.speed)
    }

    fn value(&self) -> Result<&SensorValue, String> {
        Ok(&self.speed)
    }
}