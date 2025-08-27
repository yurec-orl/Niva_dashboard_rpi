mod hardware;
mod graphics;

use hardware::GpioInput;
use graphics::{run_opengl_test, run_dashboard_gauges_test, run_moving_needle_test};
use std::thread;
use std::time::Duration;

fn main() {
    println!("Niva Dashboard - Raspberry Pi Version");
    println!("Available test modes:");
    println!("1. Basic OpenGL triangle test");
    println!("2. Simple moving needle test");
    println!("3. Multi-gauge dashboard test");
    
    // For now, run all tests in sequence
    println!("\n=== Basic OpenGL Triangle Test ===");
    match run_opengl_test() {
        Ok(()) => println!("Basic graphics test completed successfully!"),
        Err(e) => eprintln!("Basic graphics test failed: {}", e),
    }
    
    println!("\n=== Simple Moving Needle Test ===");
    match run_moving_needle_test() {
        Ok(()) => println!("Moving needle test completed successfully!"),
        Err(e) => eprintln!("Moving needle test failed: {}", e),
    }
    
    println!("\n=== Multi-Gauge Dashboard Test ===");
    match run_dashboard_gauges_test() {
        Ok(()) => println!("Dashboard gauges test completed successfully!"),
        Err(e) => eprintln!("Dashboard gauges test failed: {}", e),
    }
    
    // Keep the GPIO test functionality available
    println!("\n=== Single GPIO Input Example ===");
    match test_single_gpio_input() {
        Ok(_) => println!("Single GPIO test completed successfully"),
        Err(e) => println!("Single GPIO test failed: {}", e),
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