mod hardware;
mod graphics;
mod page_framework;
mod test;
mod indicators;
mod gauge_builders;

use crate::test::run_test::run_test;
use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::PageManager;
use crate::hardware::sensor_manager::{SensorManager, SensorDigitalInputChain, SensorAnalogInputChain};
use crate::hardware::hw_providers::*;
use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor, SpeedSensor};
use crate::hardware::sensor_value::ValueConstraints;
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
        Box::new(TestDigitalDataProvider::new(HWInput::HwBrakeFluidLvlLow)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwBrakeFluidLvlLow".to_string(), "Brake Fluid Level".to_string(),
                                           Level::Low, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(brake_fluid_chain);

    // Charge indicator sensor
    let charge_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwCharge)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwCharge".to_string(), "ЗАРЯД".to_string(),
                                           Level::Low, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(charge_chain);

    // Check engine sensor
    let check_engine_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwCheckEngine)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwCheckEngine".to_string(), "ПРОВЕРЬ ДВИГ".to_string(),
                                           Level::Low, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(check_engine_chain);

    // Differential lock sensor
    let diff_lock_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwDiffLock)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwDiffLock".to_string(), "БЛОК ДИФФ".to_string(),
                                           Level::Low, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(diff_lock_chain);

    // External lights sensor
    let ext_lights_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwExtLights)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwExtLights".to_string(), "ГАБАРИТ".to_string(),
                                           Level::Low, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(ext_lights_chain);

    // Fuel level low sensor
    let fuel_lvl_low_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwFuelLvlLow)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwFuelLvlLow".to_string(), "УРОВ ТОПЛ".to_string(),
                                           Level::Low, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(fuel_lvl_low_chain);

    // High beam sensor
    let high_beam_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwHighBeam)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwHighBeam".to_string(), "ДАЛЬНИЙ СВЕТ".to_string(),
                                           Level::Low, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(high_beam_chain);

    // Instrument illumination sensor
    let instr_illum_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwInstrIllum)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwInstrIllum".to_string(), "ОСВЕЩ".to_string(),
                                           Level::Low, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(instr_illum_chain);

    // Oil pressure low sensor
    let oil_press_low_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwOilPressLow)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwOilPressLow".to_string(), "ДАВЛ МАСЛА".to_string(),
                                           Level::Low, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(oil_press_low_chain);

    // Parking brake sensor
    let park_brake_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwParkBrake)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwParkBrake".to_string(), "СТОЯН ТОРМ".to_string(),
                                           Level::Low, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(park_brake_chain);

    // Speed sensor (active high, pulse-based)
    let speed_chain = SensorDigitalInputChain::new(
        Box::new(TestPulseDataProvider::new(HWInput::HwSpeed)),
        vec![], // No signal processors - SpeedSensor handles pulse processing internally
        Box::new(SpeedSensor::new()),
    );
    mgr.add_digital_sensor_chain(speed_chain);

    // Tachometer sensor (active high, pulse-based)
    let tacho_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwTacho)),
        vec![Box::new(DigitalSignalDebouncer::new(3, std::time::Duration::from_millis(10)))],
        Box::new(GenericDigitalSensor::new("HwTacho".to_string(), "ТАХОМЕТР".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(tacho_chain);

    // Turn signal sensor
    let turn_signal_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwTurnSignal)),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwTurnSignal".to_string(), "ИНД ПОВОР".to_string(),
                                           Level::Low, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(turn_signal_chain);

    // Analog sensor chains - using test data providers for development
    
    // 12V voltage sensor (0-20V range for full diagnostic capability)
    let voltage_12v_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::Hw12v)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new("Hw12v".to_string(), "БОРТ СЕТЬ".to_string(), "В".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 20.0, Some(11.0), Some(13.0), Some(14.7), Some(15.0)), 0.02)), // 0-20V range for diagnostic capability
    );
    mgr.add_analog_sensor_chain(voltage_12v_chain);

    // Fuel level sensor
    let fuel_level_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::HwFuelLvl)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(15))],
        Box::new(GenericAnalogSensor::new("HwFuelLvl".to_string(), "УРОВ ТОПЛ".to_string(), "%".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None), 0.1)), // Scale for percentage
    );
    mgr.add_analog_sensor_chain(fuel_level_chain);

    // Oil pressure sensor (0-8 kgf/cm² range)
    let oil_pressure_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::HwOilPress)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new("HwOilPress".to_string(), "ДАВЛ МАСЛА".to_string(), "кгс/см²".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 8.0, Some(0.5), Some(1.0), Some(7.0), Some(8.0)), 0.01)), // 0-8 kgf/cm² pressure range
    );
    mgr.add_analog_sensor_chain(oil_pressure_chain);

    // Engine temperature sensor (0-120°C range)
    let temperature_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::HwEngineCoolantTemp)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(20))],
        Box::new(GenericAnalogSensor::new("HwEngineCoolantTemp".to_string(), "ТЕМП ДВИГ".to_string(), "°C".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 120.0, Some(5.0), Some(10.0), Some(95.0), Some(105.0)), 0.1)), // 0-120°C engine temperature range
    );
    mgr.add_analog_sensor_chain(temperature_chain);

    print!("✓ Sensor manager initialized with digital and analog sensor chains\r\n");
    
    mgr
}

fn setup_ui_style() -> graphics::ui_style::UIStyle {
    let mut ui_style = graphics::ui_style::UIStyle::new();
    // ui_style.read_from_file("/etc/niva_dashboard/ui_style.json").unwrap_or_else(|e| {
    //     print!("Warning: Failed to read UI style config: {}\r\n", e);
    // });
    ui_style
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
    print!("7. Digital segmented display test\r\n");
    print!("Usage: cargo run -- [test={{basic|gltext|dashboard|needle|gpio|sensors|digital}}]\r\n");

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
    let sensors = setup_sensors();
    let ui_style = setup_ui_style();

    let mut mgr = PageManager::new(context, sensors, ui_style);

    mgr.setup().expect("Failed to setup page manager");

    match mgr.start() {
        Ok(()) => print!("Dashboard finished successfully!\r\n"),
        Err(e) => print!("Failed to start dashboard: {}\r\n", e),
    }
}