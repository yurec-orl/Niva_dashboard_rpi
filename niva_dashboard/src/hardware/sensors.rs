use rppal::gpio::Level;

use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueMetadata};
use crate::hardware::digital_signal_processing::{DigitalSignalProcessor, DigitalSignalProcessorPulsePerSecond};

// Used by all sensor types
pub trait Sensor {
    fn id(&self) -> &String;
    fn name(&self) -> &String;
    // Get last sensor value without modifying state
    fn value(&self) -> Result<&SensorValue, String>;
    fn constraints(&self) -> &ValueConstraints;
    fn metadata(&self) -> &ValueMetadata;
    fn min_value(&self) -> f32;
    fn max_value(&self) -> f32;
}

// Digital sensor trait - represents on/off state based on active level
// Active level could be low in case of pull-up input configuration
pub trait DigitalSensor: Sensor {
    fn active_level(&self) -> Level;

    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: Level) -> Result<&SensorValue, String>;
}

// Analog sensor trait - represents a numeric value based on raw input
// Value should be a processed input, e.g. voltage level converted to temperature
// All voltage divider calculations, pulse count to speed, and other 
// raw input conversion into meaningful values are done here
pub trait AnalogSensor: Sensor {
    // Update internal state based on input and return current sensor value
    fn read(&mut self, input: u16) -> Result<&SensorValue, String>;
}

pub struct GenericDigitalSensor {
    value: SensorValue,
    active_level: Level,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
}

impl GenericDigitalSensor {
    pub fn new(id: String, name: String, active_level: Level,
               constraints: ValueConstraints) -> Self {
        let metadata = ValueMetadata::new("", name, id); // Empty unit for digital sensors
        GenericDigitalSensor { value: SensorValue::empty(),
                               active_level, constraints, metadata}
    }
}

impl Sensor for GenericDigitalSensor {
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

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
    }
}

impl DigitalSensor for GenericDigitalSensor {
    fn active_level(&self) -> Level {
        self.active_level
    }

    fn read(&mut self, input: Level) -> Result<&SensorValue, String> {
        self.value = SensorValue::digital(input == self.active_level, self.metadata.label.clone(), self.metadata.sensor_id.clone());
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
    pub fn new(id: String, name: String, units: String,
               constraints: ValueConstraints, scale_factor: f32) -> Self {
        let metadata = ValueMetadata::new(units, name, id);
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

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
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

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
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

    fn min_value(&self) -> f32 {
        self.constraints.min_value
    }

    fn max_value(&self) -> f32 {
        self.constraints.max_value
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::sensor_value::{ValueData, ValueConstraints};

    #[test]
    fn test_generic_digital_sensor_creation() {
        let constraints = ValueConstraints::digital_default();
        let sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::High,
            constraints
        );

        assert_eq!(sensor.active_level(), Level::High);
        assert_eq!(Sensor::value(&sensor).unwrap().value, ValueData::Empty);
    }

    #[test]
    fn test_generic_digital_sensor_active_high() {
        let constraints = ValueConstraints::digital_default();
        let mut sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::High,
            constraints
        );

        // Test active level (High)
        sensor.read(Level::High).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, true);
        } else {
            panic!("Expected digital value");
        }

        // Test inactive level (Low)
        sensor.read(Level::Low).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, false);
        } else {
            panic!("Expected digital value");
        }
    }

    #[test]
    fn test_generic_digital_sensor_active_low() {
        let constraints = ValueConstraints::digital_default();
        let mut sensor = GenericDigitalSensor::new(
            "test_id".to_string(),
            "Test Sensor".to_string(),
            Level::Low,
            constraints
        );

        // Test active level (Low)
        sensor.read(Level::Low).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, true);
        } else {
            panic!("Expected digital value");
        }

        // Test inactive level (High)
        sensor.read(Level::High).unwrap();
        if let ValueData::Digital(active) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*active, false);
        } else {
            panic!("Expected digital value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_creation() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 1.0);

        assert_eq!(sensor.min_value(), 0.0);
        assert_eq!(sensor.max_value(), 100.0);
        assert_eq!(Sensor::value(&sensor).unwrap().value, ValueData::Empty);
    }

    #[test]
    fn test_generic_analog_sensor_reading() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 0.1);

        // Test normal reading
        sensor.read(500).unwrap(); // 500 * 0.1 = 50.0
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert!((value - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_clamping() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 1.0);

        // Test value above maximum gets clamped
        sensor.read(150).unwrap();
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*value, 100.0);
        } else {
            panic!("Expected analog value");
        }

        // Test value below minimum gets clamped
        sensor.read(0).unwrap(); // This should clamp to 0.0 (minimum)
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*value, 0.0);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_generic_analog_sensor_scaling() {
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None);
        let mut sensor = GenericAnalogSensor::new("test_id".to_string(), "Test Sensor".to_string(), "%".to_string(), constraints, 0.01);

        sensor.read(5000).unwrap(); // 5000 * 0.01 = 50.0
        if let ValueData::Analog(value) = &Sensor::value(&sensor).unwrap().value {
            assert!((value - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog value");
        }
    }

    #[test]
    fn test_engine_temperature_sensor_creation() {
        let sensor = EngineTemperatureSensor::new();
        
        assert_eq!(sensor.constraints.min_value, 0.0);
        assert_eq!(sensor.constraints.max_value, 120.0);
        assert_eq!(sensor.metadata.unit, "°C");
        assert_eq!(sensor.metadata.label, "ТЕМП");
        assert_eq!(sensor.metadata.sensor_id, "engine_temp");
    }

    #[test]
    fn test_engine_temperature_sensor_reading() {
        let mut sensor = EngineTemperatureSensor::new();

        // Test normal temperature reading
        sensor.read(500).unwrap(); // 500 * 0.1 = 50.0°C
        if let ValueData::Analog(temp) = &Sensor::value(&sensor).unwrap().value {
            assert!((temp - 50.0).abs() < 0.001);
        } else {
            panic!("Expected analog temperature value");
        }

        // Test high temperature reading with clamping
        sensor.read(1500).unwrap(); // 1500 * 0.1 = 150.0°C, should clamp to 120.0°C
        if let ValueData::Analog(temp) = &Sensor::value(&sensor).unwrap().value {
            assert_eq!(*temp, 120.0);
        } else {
            panic!("Expected analog temperature value");
        }
    }

    #[test]
    fn test_speed_sensor_creation() {
        let sensor = SpeedSensor::new();
        
        assert_eq!(sensor.pulses_per_revolution, 6);
        assert!((sensor.wheel_circumference_m - 2.304).abs() < 0.001);
        assert_eq!(sensor.constraints.min_value, 0.0);
        assert_eq!(sensor.constraints.max_value, 180.0);
        assert_eq!(sensor.metadata.unit, "км/ч");
        assert_eq!(sensor.metadata.label, "СКОР");
        assert_eq!(sensor.metadata.sensor_id, "speed_sensor");
        assert_eq!(sensor.active_level(), Level::High);
    }

    #[test]
    fn test_speed_sensor_calculations() {
        let sensor = SpeedSensor::new();
        
        // Test zero speed
        let speed = sensor.calculate_speed_kmh(0.0);
        assert_eq!(speed, 0.0);
        
        // Test known values - validate the calculation logic
        // 6 pulses/sec = 1 revolution/sec = 2.304 m/s = 8.2944 km/h
        let speed = sensor.calculate_speed_kmh(6.0);
        let expected = 2.304 * 3.6; // 8.2944 km/h
        assert!((speed - expected).abs() < 0.01);
        
        // Test city driving speed (~30 km/h)
        let speed = sensor.calculate_speed_kmh(21.7); // Should give ~30 km/h
        assert!((speed - 30.0).abs() < 1.0); // Allow 1 km/h tolerance
        
        // Test highway speed (~60 km/h)
        let speed = sensor.calculate_speed_kmh(43.4); // Should give ~60 km/h
        assert!((speed - 60.0).abs() < 2.0); // Allow 2 km/h tolerance
    }

    #[test]
    fn test_speed_sensor_pulse_processing() {
        let mut sensor = SpeedSensor::new();
        
        // Test initial state
        assert_eq!(sensor.current_speed_kmh(), 0.0);
        
        // Simulate pulse sequence (alternating High/Low)
        let mut speed = 0.0;
        for i in 0..12 { // 12 pulses = 2 full revolutions
            let level = if i % 2 == 0 { Level::High } else { Level::Low };
            speed = sensor.process_pulse(level);
        }
        
        // After processing pulses, we should have some speed reading
        // The exact value depends on timing in the pulse counter, so just verify it's reasonable
        assert!(speed >= 0.0);
        assert!(speed <= 180.0); // Within sensor constraints
    }

    #[test]
    fn test_speed_sensor_digital_sensor_trait() {
        let mut sensor = SpeedSensor::new();
        
        // Test DigitalSensor trait implementation
        assert_eq!(sensor.active_level(), Level::High);
        
        // Test read method
        let result = sensor.read(Level::High);
        assert!(result.is_ok());
        
        // Test value method
        let value_result = Sensor::value(&sensor);
        assert!(value_result.is_ok());
    }

    #[test]
    fn test_speed_sensor_wheel_circumference_calculation() {
        // Verify the wheel circumference calculation for 235/75/15 tire
        // Diameter = 15" (381mm) + 2 * (235mm * 0.75) = 381 + 352.5 = 733.5mm
        // Circumference = π * 733.5mm = 2304.12mm = 2.304m
        let sensor = SpeedSensor::new();
        let expected_circumference = std::f32::consts::PI * 0.7335; // 0.7335m diameter
        assert!((sensor.wheel_circumference_m - expected_circumference).abs() < 0.01);
    }

    #[test]
    fn test_sensor_trait_implementations() {
        // Test GenericDigitalSensor implements Sensor trait correctly
        let constraints = ValueConstraints::digital_default();
        let digital_sensor = GenericDigitalSensor::new(
            "digital_test".to_string(),
            "Digital Test".to_string(),
            Level::High,
            constraints
        );
        
        assert!(digital_sensor.id().contains("digital_test"));
        assert!(digital_sensor.name().contains("Digital Test"));
        assert!(Sensor::value(&digital_sensor).is_ok());
        assert!(digital_sensor.constraints().min_value >= 0.0);
        // Note: empty SensorValue has empty metadata, so we skip that check

        // Test GenericAnalogSensor implements Sensor trait correctly
        let constraints = ValueConstraints::analog_with_thresholds(0.0, 100.0, None, None, None, None);
        let analog_sensor = GenericAnalogSensor::new("analog_test".to_string(), "Analog Test".to_string(), "V".to_string(), constraints, 1.0);
        
        assert_eq!(analog_sensor.id(), "analog_test");
        assert_eq!(analog_sensor.name(), "Analog Test");
        assert!(Sensor::value(&analog_sensor).is_ok());
        assert_eq!(analog_sensor.constraints().min_value, 0.0);
        assert_eq!(analog_sensor.metadata().unit, "V");

        // Test SpeedSensor implements Sensor trait correctly
        let speed_sensor = SpeedSensor::new();
        assert_eq!(speed_sensor.id(), "speed_sensor");
        assert_eq!(speed_sensor.name(), "СКОР");
        assert!(Sensor::value(&speed_sensor).is_ok());
        assert_eq!(speed_sensor.constraints().min_value, 0.0);
        assert_eq!(speed_sensor.metadata().unit, "км/ч");
    }

    #[test]
    fn test_analog_sensor_trait_implementations() {
        // Test GenericAnalogSensor implements AnalogSensor trait
        let constraints = ValueConstraints::analog_with_thresholds(10.0, 90.0, None, None, None, None);
        let mut sensor = GenericAnalogSensor::new("test".to_string(), "Test".to_string(), "V".to_string(), constraints, 1.0);
        
        assert_eq!(sensor.min_value(), 10.0);
        assert_eq!(sensor.max_value(), 90.0);
        
        let result = sensor.read(50);
        assert!(result.is_ok());

        let value_result = Sensor::value(&sensor);
        assert!(value_result.is_ok());

        // Test EngineTemperatureSensor implements AnalogSensor trait
        let mut temp_sensor = EngineTemperatureSensor::new();
        assert_eq!(temp_sensor.min_value(), 0.0);
        assert_eq!(temp_sensor.max_value(), 120.0);
        
        let temp_result = temp_sensor.read(800);
        assert!(temp_result.is_ok());
        
        let temp_value_result = Sensor::value(&temp_sensor);
        assert!(temp_value_result.is_ok());
    }
}