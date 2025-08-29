use std::thread;
use std::time::Duration;

use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::{run_basic_geometry_test, run_opengl_text_rendering_test};
use crate::hardware::GpioInput;

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
        "gpio" => {
            println!("\n=== GPIO Input Test ===");
            match test_single_gpio_input() {
                Ok(()) => println!("GPIO test completed successfully!"),
                Err(e) => eprintln!("GPIO test failed: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown test: {}", name);
            eprintln!("Valid options: basic, needle, gauges, text, sdl2, advanced, rotating, gpio, all");
            std::process::exit(1);
        }
    }
}

// Helper function to run graphics tests with shared context
fn run_graphics_test<F>(title: &str, test_func: F) 
where
    F: FnOnce(&GraphicsContext) -> Result<(), String>,
{
    match GraphicsContext::new_dashboard(title) {
        Ok(context) => {
            match test_func(&context) {
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