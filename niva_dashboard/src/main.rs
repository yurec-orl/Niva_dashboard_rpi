mod hardware;

use hardware::{GpioInput, GpioInputConfig, Bias};
use std::thread;
use std::time::Duration;

fn main() {
    println!("Niva Dashboard - GPIO Input Test");
    
    // Example of single GPIO input
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