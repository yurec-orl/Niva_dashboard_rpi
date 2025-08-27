mod hardware;
mod graphics;

use hardware::GpioInput;
use graphics::{run_opengl_test, run_dashboard_gauges_test, run_moving_needle_test, run_text_rendering_test, run_sdl2_gauges_test, GraphicsContext};
use std::thread;
use std::time::Duration;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    println!("Niva Dashboard - Raspberry Pi Version");
    println!("Available test modes:");
    println!("1. Basic OpenGL triangle test");
    println!("2. Simple moving needle test");
    println!("3. Multi-gauge dashboard test (OpenGL)");
    println!("4. Text rendering test with multiple fonts and sizes");
    println!("5. SDL2 high-level gauge rendering test");
    println!("6. GPIO input test");
    println!("7. Combined graphics test (shared context)");
    println!("Usage: cargo run -- [basic|needle|gauges|text|sdl2|gpio|all]");
    
    let test_name = if args.len() >= 2 {
        &args[1]
    } else {
        println!("\nNo test specified. Running all tests...");
        "all"
    };
    
    match test_name {
        "basic" => {
            println!("\n=== Basic OpenGL Triangle Test ===");
            run_graphics_test("Niva Dashboard - Basic Test", run_opengl_test);
        }
        "needle" => {
            println!("\n=== Simple Moving Needle Test ===");
            run_graphics_test("Niva Dashboard - Needle Test", run_moving_needle_test);
        }
        "gauges" => {
            println!("\n=== Multi-Gauge Dashboard Test ===");
            run_graphics_test("Niva Dashboard - Gauges Test", run_dashboard_gauges_test);
        }
        "text" => {
            println!("\n=== Text Rendering Test ===");
            run_graphics_test("Niva Dashboard - Text Test", run_text_rendering_test);
        }
        "sdl2" => {
            println!("\n=== SDL2 High-Level Gauge Rendering Test ===");
            run_graphics_test("Niva Dashboard - SDL2 Gauges", run_sdl2_gauges_test);
        }
        "gpio" => {
            println!("\n=== Single GPIO Input Example ===");
            match test_single_gpio_input() {
                Ok(_) => println!("Single GPIO test completed successfully"),
                Err(e) => println!("Single GPIO test failed: {}", e),
            }
        }
        "all" => {
            // Run all tests in sequence
            println!("\n=== Running All Tests ===");
            
            println!("\n=== Basic OpenGL Triangle Test ===");
            run_graphics_test("Niva Dashboard - Basic Test", run_opengl_test);
            
            println!("\n=== Simple Moving Needle Test ===");
            run_graphics_test("Niva Dashboard - Needle Test", run_moving_needle_test);
            
            println!("\n=== Multi-Gauge Dashboard Test ===");
            run_graphics_test("Niva Dashboard - Gauges Test", run_dashboard_gauges_test);
            
            println!("\n=== Text Rendering Test ===");
            run_graphics_test("Niva Dashboard - Text Test", run_text_rendering_test);
            
            println!("\n=== SDL2 High-Level Gauge Rendering Test ===");
            run_graphics_test("Niva Dashboard - SDL2 Gauges", run_sdl2_gauges_test);
            
            println!("\n=== Single GPIO Input Example ===");
            match test_single_gpio_input() {
                Ok(_) => println!("Single GPIO test completed successfully"),
                Err(e) => println!("Single GPIO test failed: {}", e),
            }
        }
        _ => {
            eprintln!("Unknown test: {}", test_name);
            eprintln!("Valid options: basic, needle, gauges, text, sdl2, gpio, all");
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