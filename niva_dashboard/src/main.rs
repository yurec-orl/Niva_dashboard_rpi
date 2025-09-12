mod hardware;
mod graphics;
mod page_framework;
mod test;
mod indicators;

use crate::test::run_test::run_test;
use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::PageManager;
use crate::hardware::sensor_manager::{SensorManager, SensorDigitalInputChain, SensorAnalogInputChain};
use crate::hardware::hw_providers::*;
use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor};
use rppal::gpio::Level;
use std::env;

fn setup_context() -> GraphicsContext {
    let context = GraphicsContext::new_dashboard("Niva Dashboard").expect("Failed to create graphics context");

    // Hide mouse cursor for dashboard application
    if let Err(e) = context.hide_cursor() {
        print!("Warning: Failed to hide cursor: {}\r\n", e);
    } else {
        print!("✓ Mouse cursor hidden for dashboard mode\r\n");
    }

    context
}

fn setup_sensors() -> SensorManager {
    let mut mgr = SensorManager::new();
    
    // Sensor value constraints:
    // - Engine Temperature: 5-100°C operational, 0-120°C dashboard range
    // - 12V System: 12-14.4V normal, 0-20V diagnostic range  
    // - Oil Pressure: 0-8 kgf/cm² (kilogram-force per square centimeter)
    // - Fuel Level: 0-100% of tank capacity
    
    // Digital sensor chains - using test data providers for development
    
    // Brake fluid level low sensor
    let brake_fluid_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_BRAKE_FLUID_LVL_LOW_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(brake_fluid_chain);

    // Charge indicator sensor
    let charge_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_CHARGE_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(charge_chain);

    // Differential lock sensor
    let diff_lock_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_DIFF_LOCK_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(diff_lock_chain);

    // External lights sensor
    let ext_lights_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_EXT_LIGHTS_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(ext_lights_chain);

    // Fuel level low sensor
    let fuel_lvl_low_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_FUEL_LVL_LOW_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(fuel_lvl_low_chain);

    // High beam sensor
    let high_beam_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_HIGH_BEAM_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(high_beam_chain);

    // Instrument illumination sensor
    let instr_illum_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_INSTR_ILLUM_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(instr_illum_chain);

    // Oil pressure low sensor
    let oil_press_low_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_OIL_PRESS_LOW_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(oil_press_low_chain);

    // Parking brake sensor
    let park_brake_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_PARK_BRAKE_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(park_brake_chain);

    // Speed sensor (active high, pulse-based)
    let speed_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_SPEED_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(3, std::time::Duration::from_millis(10)))],
        Box::new(GenericDigitalSensor::new(Level::High)),
    );
    mgr.add_digital_sensor_chain(speed_chain);

    // Tachometer sensor (active high, pulse-based)
    let tacho_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_TACHO_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(3, std::time::Duration::from_millis(10)))],
        Box::new(GenericDigitalSensor::new(Level::High)),
    );
    mgr.add_digital_sensor_chain(tacho_chain);

    // Turn signal sensor
    let turn_signal_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HW_TURN_SIGNAL_INPUT)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new(Level::Low)),
    );
    mgr.add_digital_sensor_chain(turn_signal_chain);

    // Analog sensor chains - using test data providers for development
    
    // 12V voltage sensor (0-20V range for full diagnostic capability)
    let voltage_12v_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HW_12V_INPUT)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new(0.0, 20.0, 0.01)), // 0-20V range for diagnostic capability
    );
    mgr.add_analog_sensor_chain(voltage_12v_chain);

    // Fuel level sensor
    let fuel_level_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HW_FUEL_LVL_INPUT)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(15))],
        Box::new(GenericAnalogSensor::new(0.0, 100.0, 0.1)), // Scale for percentage
    );
    mgr.add_analog_sensor_chain(fuel_level_chain);

    // Oil pressure sensor (0-8 kgf/cm² range)
    let oil_pressure_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HW_OIL_PRESS_INPUT)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new(0.0, 8.0, 0.01)), // 0-8 kgf/cm² pressure range
    );
    mgr.add_analog_sensor_chain(oil_pressure_chain);

    // Engine temperature sensor (0-120°C range)
    let temperature_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HW_TEMP_INPUT)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(20))],
        Box::new(GenericAnalogSensor::new(0.0, 120.0, 0.1)), // 0-120°C engine temperature range
    );
    mgr.add_analog_sensor_chain(temperature_chain);

    print!("✓ Sensor manager initialized with digital and analog sensor chains\r\n");
    
    mgr
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    print!("Niva Dashboard - Raspberry Pi Version (KMS/DRM Backend)\r\n");
    print!("Available test modes:\r\n");
    print!("1. Basic OpenGL triangle test\r\n");
    print!("2. OpenGL text rendering test with FreeType\r\n");
    print!("3. Dashboard performance test (9 animated gauges)\r\n");
    print!("4. Rotating needle gauge test (circular gauge with numbers)\r\n");
    print!("5. GPIO input test\r\n");
    print!("6. Sensor manager test\r\n");
    print!("Usage: cargo run -- [test={{basic|gltext|dashboard|needle|gpio|sensors}}]\r\n");

    for arg in args {
        let parm = arg.split("=").collect::<Vec<&str>>();
        if parm.len() == 2 {
            match parm[0] {
                "test" => {
                    run_test(parm[1]);
                    return;
                }
                _ => {
                    print!("Unknown argument: {}\r\n", parm[0]);
                }
            }
        }
    }

    let context = setup_context();
    let sensors = setup_sensors(); // Set up sensors but don't use them yet

    let mut mgr = PageManager::new(context, sensors);

    mgr.setup().expect("Failed to setup page manager");

    match mgr.start() {
        Ok(()) => print!("Dashboard finished successfully!\r\n"),
        Err(e) => print!("Failed to start dashboard: {}\r\n", e),
    }
}