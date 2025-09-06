use std::thread;
use std::time::Duration;

use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::{run_basic_geometry_test, run_opengl_text_rendering_test, run_dashboard_performance_test, run_rotating_needle_gauge_test};
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
    use crate::hardware::hw_providers::{TestDigitalDataProvider, TestAnalogDataProvider, HWDigitalInput, HWAnalogInput};
    use crate::hardware::sensor_manager::{SensorDigitalInputChain, SensorAnalogInputChain};
    use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
    use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
    use crate::hardware::sensors::{GenericDigitalSensor, AnalogSensor};
    use rppal::gpio::Level;
    use std::time::Duration;
    
    println!("Creating sensor manager for testing...");
    let mut manager = SensorManager::new();
    
    // Test fuel sensor implementation
    struct TestFuelSensor;
    impl AnalogSensor for TestFuelSensor {
        fn value(&self, input: u16) -> Result<f32, String> {
            let percentage = (input as f32 / 1023.0) * 100.0;
            Ok(percentage.clamp(0.0, 100.0))
        }
    }
    
    // Create digital sensor chain for park brake
    println!("Setting up digital sensor chain (park brake)...");
    let park_brake_input = HWDigitalInput::HwParkBrake(Level::Low);
    let digital_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(park_brake_input.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(2, Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    manager.add_digital_sensor_chain(digital_chain);
    
    // Create analog sensor chain for fuel level
    println!("Setting up analog sensor chain (fuel level)...");
    let fuel_input = HWAnalogInput::HwFuelLvl;
    let analog_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(fuel_input.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(3))],
        Box::new(TestFuelSensor),
    );
    manager.add_analog_sensor_chain(analog_chain);
    
    // Test reading sensors multiple times
    for i in 1..=5 {
        println!("\n--- Reading cycle {} ---", i);
        
        // Read digital sensor
        match manager.read_digital_sensor(park_brake_input.clone()) {
            Ok(brake_active) => println!("Park brake: {}", if brake_active { "ENGAGED" } else { "RELEASED" }),
            Err(e) => println!("Error reading park brake: {}", e),
        }
        
        // Read analog sensor  
        match manager.read_analog_sensor(fuel_input.clone()) {
            Ok(fuel_level) => println!("Fuel level: {:.1}%", fuel_level),
            Err(e) => println!("Error reading fuel level: {}", e),
        }
        
        thread::sleep(Duration::from_millis(500));
    }
    
    println!("\n✓ Sensor manager integration test completed");
    Ok(())
}