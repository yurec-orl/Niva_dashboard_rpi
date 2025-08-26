mod hardware;

use hardware::{GpioInput, GpioInputConfig, MultiGpioInput, Bias};
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
    
    // Example of multiple GPIO inputs
    println!("\n=== Multiple GPIO Inputs Example ===");
    match test_multiple_gpio_inputs() {
        Ok(_) => println!("Multiple GPIO test completed successfully"),
        Err(e) => println!("Multiple GPIO test failed: {}", e),
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

fn test_multiple_gpio_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let mut multi_input = MultiGpioInput::new();
    
    // Add multiple GPIO inputs (simulating dashboard buttons)
    let left_buttons = [2, 3, 4, 14];  // Left side buttons
    let right_buttons = [15, 18, 23, 24]; // Right side buttons
    
    println!("Configuring left side buttons...");
    for &pin in &left_buttons {
        let config = GpioInputConfig {
            pin_number: pin,
            bias: Bias::PullUp,
            active_low: true,
        };
        let index = multi_input.add_input(config)?;
        println!("  Added button on pin {} at index {}", pin, index);
    }
    
    println!("Configuring right side buttons...");
    for &pin in &right_buttons {
        let index = multi_input.add_input_pin(pin)?;
        println!("  Added button on pin {} at index {}", pin, index);
    }
    
    println!("\nTotal inputs configured: {}", multi_input.input_count());
    
    // Read all inputs for a few cycles
    println!("\nReading all inputs for 3 seconds...");
    for cycle in 0..30 {
        let raw_states = multi_input.read_all_raw();
        let logical_states = multi_input.read_all_logical();
        
        println!("Cycle {}: ", cycle + 1);
        for (i, (&raw, &logical)) in raw_states.iter().zip(logical_states.iter()).enumerate() {
            if let Some(pin_num) = multi_input.get_pin_number(i) {
                println!("  Pin {}: {} ({})", 
                        pin_num, 
                        raw, 
                        if logical { "PRESSED" } else { "RELEASED" });
            }
        }
        
        thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}
