use std::thread;
use std::time::Duration;

use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::{run_basic_geometry_test, run_opengl_text_rendering_test, run_dashboard_performance_test, run_rotating_needle_gauge_test};
use crate::hardware::hw_providers::*;
use crate::hardware::GpioInput;
use crate::hardware::sensor_manager::SensorManager;

pub fn run_test(name: &str) {
    match name {
        "basic" => {
            println!("\n=== Basic OpenGL Triangle Test ===");
            run_graphics_test("Niva Dashboard - Basic Test", run_basic_geometry_test);
        }
        "gltext" => {
            println!("\n=== OpenGL Text Rendering Test ===");
            run_graphics_test("Niva Dashboard - Text Test", run_opengl_text_rendering_test);
        }
        "dashboard" => {
            println!("\n=== Dashboard Performance Test ===");
            run_graphics_test("Niva Dashboard - Performance Test", run_dashboard_performance_test);
        }
        "needle" => {
            println!("\n=== Rotating Needle Gauge Test ===");
            run_graphics_test("Niva Dashboard - Needle Gauge Test", run_rotating_needle_gauge_test);
        }
        "gpio" => {
            println!("\n=== GPIO Input Test ===");
            match test_single_gpio_input() {
                Ok(()) => println!("GPIO test completed successfully!"),
                Err(e) => eprintln!("GPIO test failed: {}", e),
            }
        }
        "sensors" => {
            println!("\n=== Sensor Manager Test ===");
            match test_sensor_manager() {
                Ok(()) => println!("Sensor manager test completed successfully!"),
                Err(e) => eprintln!("Sensor manager test failed: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown test: {}", name);
            eprintln!("Valid options: basic, gltext, dashboard, needle, gpio, sensors");
            eprintln!("Note: SDL2-based tests (sdl2, advanced, etc.) are disabled after KMS/DRM migration");
            std::process::exit(1);
        }
    }
}

// Helper function to run graphics tests with shared context
fn run_graphics_test<F>(title: &str, test_func: F) 
where
    F: FnOnce(&mut GraphicsContext) -> Result<(), String>,
{
    match GraphicsContext::new_dashboard(title) {
        Ok(mut context) => {
            match test_func(&mut context) {
                Ok(()) => println!("Graphics test completed successfully!"),
                Err(e) => eprintln!("Graphics test failed: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to create graphics context: {}", e),
    }
}

fn test_single_gpio_input() -> Result<(), Box<dyn std::error::Error>> {
    // Create a GPIO input on pin 2 with default configuration (pull-up, active low)
    let gpio_input = GpioInput::new_with_pin(2)?;
    
    println!("Reading GPIO pin {} for 5 seconds...", gpio_input.pin_number());
    println!("Configuration: Active Low = {}", gpio_input.is_active_low());
    
    for i in 0..50 {
        let raw_state = gpio_input.read_raw();
        let logical_state = gpio_input.read_logical();
        
        println!("Sample {}: Raw = {}, Logical = {}", 
                i + 1, raw_state, if logical_state { "ACTIVE" } else { "INACTIVE" });
        
        thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn test_sensor_manager() -> Result<(), Box<dyn std::error::Error>> {
    use crate::hardware::hw_providers::{TestDigitalDataProvider, TestAnalogDataProvider};
    use crate::hardware::sensor_manager::{SensorDigitalInputChain, SensorAnalogInputChain};
    use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
    use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
    use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor};
    use crate::hardware::sensor_value::ValueConstraints;
    use rppal::gpio::Level;
    use std::time::Duration;
    
    println!("Creating sensor manager for testing...");
    let mut manager = SensorManager::new();
    
    // Create digital sensor chain for park brake
    println!("Setting up digital sensor chain (park brake)...");
    let digital_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwParkBrake)),
        vec![Box::new(DigitalSignalDebouncer::new(2, Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("park_brake_test".to_string(), "СТОЯН ТОРМ".to_string(),
                                           Level::Low, ValueConstraints::digital_warning())),
    );
    manager.add_digital_sensor_chain(digital_chain);
    
    // Create analog sensor chain for fuel level
    println!("Setting up analog sensor chain (fuel level)...");
    let analog_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::HwFuelLvl)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(3))],
        Box::new(GenericAnalogSensor::new("fuel_test".to_string(), "УРОВ ТОПЛ".to_string(), "%".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None), 0.1)),
    );
    manager.add_analog_sensor_chain(analog_chain);
    
    // Test reading sensors multiple times
    for i in 1..=5 {
        println!("\n--- Reading cycle {} ---", i);
        
        // Read all sensors first
        match manager.read_all_sensors() {
            Ok(_) => {
                // Get sensor values
                let values = manager.get_sensor_values();

                // Display digital sensor values
                for (input, sensor_value) in values {
                    if *input == HWInput::HwParkBrake {
                        println!("Park brake: {} ({})", 
                                if sensor_value.is_active() { "ENGAGED" } else { "RELEASED" },
                                if sensor_value.is_warning() { "WARNING" } else if sensor_value.is_critical() { "CRITICAL" } else { "NORMAL" });
                        break;
                    }
                }

                // Display analog sensor values
                for (input, sensor_value) in values {
                    if *input == HWInput::HwFuelLvl {
                        let status = if sensor_value.is_critical() { " [CRITICAL]" } 
                                   else if sensor_value.is_warning() { " [WARNING]" } 
                                   else { "" };
                        println!("Fuel level: {:.1}%{}", sensor_value.as_f32(), status);
                        break;
                    }
                }
            },
            Err(e) => println!("Error reading sensors: {}", e),
        }
        
        thread::sleep(Duration::from_millis(500));
    }
    
    println!("\n✓ Sensor manager integration test completed");
    Ok(())
}