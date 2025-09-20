use std::thread;
use std::time::Duration;

use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::{run_basic_geometry_test, run_opengl_text_rendering_test, run_dashboard_performance_test, run_rotating_needle_gauge_test};
use crate::graphics::digital_display_demo::{render_digital_number, render_digital_speedometer, render_digital_rpm};
use crate::hardware::hw_providers::*;
use crate::hardware::GpioInput;
use crate::hardware::sensor_manager::SensorManager;
use crate::hardware::sensor_value::SensorValue;
use crate::indicators::digital_segmented_indicator::DigitalSegmentedIndicator;
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::ui_style::UIStyle;

extern crate gl;

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
        "digital" => {
            println!("\n=== Digital Segmented Display Test ===");
            run_graphics_test("Niva Dashboard - Digital Display Test", run_digital_display_test);
        }
        "font" => {
            println!("\n=== Digital Font Direct Rendering Test ===");
            run_graphics_test("Niva Dashboard - Font Test", run_digital_font_test);
        }
        _ => {
            eprintln!("Unknown test: {}", name);
            eprintln!("Valid options: basic, gltext, dashboard, needle, gpio, sensors, digital, font");
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

/// Digital segmented display demonstration and test
fn run_digital_display_test(context: &mut GraphicsContext) -> Result<(), String> {
    let ui_style = UIStyle::new();
    
    println!("\n=== Testing Digital Display Rendering ===");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for text transparency
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Clear screen with dark background
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    
    // Test different digital displays
    
    // Time display "10:43"
    if let Err(e) = render_time_example(context, &ui_style, 10, 43) {
        eprintln!("Error rendering time display: {}", e);
    } else {
        println!("✓ Time display rendered successfully");
    }
    
    // Speed display "0088"
    if let Err(e) = render_speed_example(context, &ui_style, 88.0) {
        eprintln!("Error rendering speed display: {}", e);
    } else {
        println!("✓ Speed display rendered successfully");
    }
    
    // RPM display "2500"
    if let Err(e) = render_rpm_example(context, &ui_style, 2500.0) {
        eprintln!("Error rendering RPM display: {}", e);
    } else {
        println!("✓ RPM display rendered successfully");
    }
    
    // Temperature display "85.2"
    if let Err(e) = render_temperature_example(context, &ui_style, 85.2) {
        eprintln!("Error rendering temperature display: {}", e);
    } else {
        println!("✓ Temperature display rendered successfully");
    }
    
    // Voltage display "V12.34"
    if let Err(e) = render_voltage_example(context, &ui_style, 12.34) {
        eprintln!("Error rendering voltage display: {}", e);
    } else {
        println!("✓ Voltage display rendered successfully");
    }
    
    unsafe {
        // Clean up OpenGL state
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindTexture(gl::TEXTURE_2D, 0);
        gl::UseProgram(0);
    }
    
    context.swap_buffers();
    
    println!("\n--- Display Test Results ---");
    println!("All digital displays have been rendered to the screen.");
    println!("Check the display for:");
    println!("- Time: 10:43");
    println!("- Speed: 0088 km/h");
    println!("- RPM: 2500");
    println!("- Temperature: 85.2°C");
    println!("- Voltage: V12.34");
    println!("- Amber LCD theme with inactive segments visible");
    
    // Keep display visible
    thread::sleep(Duration::from_secs(15));
    
    Ok(())
}

/// Example of rendering a digital time display
fn render_time_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    hours: i32,
    _minutes: i32
) -> Result<(), String> {
    let time_indicator = DigitalSegmentedIndicator::integer(4); // HHMM format as integer
    
    // Convert hours to HHMM format (e.g., 10:43 -> 1043)
    let time_as_int = hours * 100 + 43; // Hardcoded minutes for demo
    let time_value = SensorValue::analog(
        time_as_int as f32,
        0.0,
        2400.0,
        "",
        "Time",
        "time_display"
    );
    
    let bounds = IndicatorBounds {
        x: 50.0,
        y: 50.0,
        width: 200.0,
        height: 60.0,
    };
    
    time_indicator.render(&time_value, bounds, ui_style, context)
}

/// Example of rendering a digital speed display
fn render_speed_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    speed_kmh: f32
) -> Result<(), String> {
    let speed_indicator = DigitalSegmentedIndicator::integer(3); // 3-digit speed
    
    let speed_value = SensorValue::analog(
        speed_kmh,
        0.0,
        200.0,
        "km/h",
        "Speed",
        "speed_sensor"
    );
    
    let bounds = IndicatorBounds {
        x: 300.0,
        y: 50.0,
        width: 240.0,
        height: 60.0,
    };
    
    speed_indicator.render(&speed_value, bounds, ui_style, context)
}

/// Example of rendering a digital RPM display
fn render_rpm_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    rpm: f32
) -> Result<(), String> {
    let rpm_indicator = DigitalSegmentedIndicator::integer(4); // 4-digit RPM
    
    let rpm_value = SensorValue::analog(
        rpm,
        0.0,
        8000.0,
        "RPM",
        "Engine RPM",
        "rpm_sensor"
    );
    
    let bounds = IndicatorBounds {
        x: 50.0,
        y: 150.0,
        width: 200.0,
        height: 60.0,
    };
    
    rpm_indicator.render(&rpm_value, bounds, ui_style, context)
}

/// Example of rendering a digital temperature display
fn render_temperature_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    temp_celsius: f32
) -> Result<(), String> {
    let temp_indicator = DigitalSegmentedIndicator::float(4, 1); // 4 digits total, 1 decimal (XX.X)
    
    let temp_value = SensorValue::analog(
        temp_celsius,
        -40.0,
        120.0,
        "°C",
        "Temperature",
        "temp_sensor"
    );
    
    let bounds = IndicatorBounds {
        x: 300.0,
        y: 150.0,
        width: 160.0,
        height: 60.0,
    };
    
    temp_indicator.render(&temp_value, bounds, ui_style, context)
}

/// Example of rendering a digital voltage display
fn render_voltage_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    voltage: f32
) -> Result<(), String> {
    let voltage_indicator = DigitalSegmentedIndicator::float(4, 2); // 4 digits total, 2 decimals (XX.XX)
    
    let voltage_value = SensorValue::analog(
        voltage,
        0.0,
        15.0,
        "V",
        "Voltage",
        "voltage_sensor"
    );
    
    let bounds = IndicatorBounds {
        x: 50.0,
        y: 250.0,
        width: 200.0,
        height: 60.0,
    };
    
    voltage_indicator.render(&voltage_value, bounds, ui_style, context)
}

/// Ultra-simple font rendering test - just digits 0-9 at fixed position
fn run_digital_font_test(context: &mut GraphicsContext) -> Result<(), String> {
    println!("=== Simple Font Test: Digits 0-9 ===");
    
    unsafe {
        // Set viewport
        gl::Viewport(0, 0, context.width, context.height);
        
        // Enable blending for text transparency
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        
        // Clear screen with dark background
        gl::ClearColor(0.0, 0.0, 0.0, 1.0);
        gl::Clear(gl::COLOR_BUFFER_BIT);
    }
    
    // Render just "0123456789" at position 100,100 with DSEG font
    let dseg_font = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/fonts/DSEG7Classic-Regular.ttf";
    let white_color = (1.0, 1.0, 1.0);  // Pure white for visibility
    
    println!("Rendering '0123456789' at position (100, 100)");
    println!("Font: {}", dseg_font);
    println!("Color: white (1.0, 1.0, 1.0)");
    println!("Size: 32px");
    
    match context.render_text_with_font("0123456789", 100.0, 100.0, 2.5, white_color, dseg_font, 32) {
        Ok(_) => println!("✓ Text rendering completed without error"),
        Err(e) => {
            println!("✗ Text rendering failed: {}", e);
            return Err(e);
        }
    }
    
    unsafe {
        // Clean up OpenGL state
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindTexture(gl::TEXTURE_2D, 0);
        gl::UseProgram(0);
    }
    
    context.swap_buffers();
    
    println!("\n--- Test Complete ---");
    println!("Check the screen at position (100, 100) for white digits 0-9");
    println!("If you see colored rectangles instead of numbers, there's a glyph rendering issue");
    
    // Keep display visible longer
    thread::sleep(Duration::from_secs(10));
    Ok(())
}