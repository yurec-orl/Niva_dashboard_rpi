mod hardware;
mod graphics;
mod page_framework;
mod test;
mod indicators;
mod indicator_builders;
mod alerts;
mod util;

use crate::test::run_test::run_test;
use crate::graphics::context::GraphicsContext;
use crate::page_framework::page_manager::PageManager;
use crate::page_framework::events::UIEvent;
use crate::page_framework::input::{InputSource, PhysicalButtonInput, KeyboardInput};
use crate::hardware::sensor_manager::{SensorManager, SensorDigitalInputChain, SensorAnalogInputChain};
use crate::hardware::hw_providers::*;
use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor, SpeedSensor, EngineTemperatureSensor};
use crate::hardware::sensor_value::ValueConstraints;
use crate::util::adc_data_provider::{ADCDataProvider, ADCFrame};
use crate::util::logging::init_logging;
use crate::util::ups_monitor::UpsMonitor;
use rppal::gpio::Level;
use std::env;
use std::thread;
use std::time::Duration;

fn setup_context() -> GraphicsContext {
    let context = GraphicsContext::new_dashboard("Niva Dashboard").expect("Failed to create graphics context");

    // Hide mouse cursor for dashboard application
    if let Err(e) = context.hide_cursor() {
        log::warn!("Warning: Failed to hide cursor: {}", e);
    } else {
        log::info!("✓ Mouse cursor hidden for dashboard mode");
    }

    context
}

fn setup_self_test_sensors() -> SensorManager {
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

    // Engine temperature sensor (0-130°C range)
    let temperature_chain = SensorAnalogInputChain::new(
        Box::new(TestAnalogDataProvider::new(HWInput::HwEngineCoolantTemp)),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(20))],
        Box::new(EngineTemperatureSensor::new()), // 0-130°C engine temperature range
    );
    mgr.add_analog_sensor_chain(temperature_chain);

    log::info!("✓ Sensor manager initialized with digital and analog sensor chains");
    
    mgr
}

fn setup_sensors(adc: Option<ADCFrame>) -> SensorManager {
    let mut mgr = SensorManager::new();
    // Lets adc_link_down() suppress "channel not in frame" log spam while the ADC
    // reconnect loop is doing its thing (see AdcDataProvider).
    mgr.set_adc_frame(adc.clone());

    // ADC link-health chain — added unconditionally (before the early return below) so
    // that both failure modes surface identically: the port never opening at startup
    // (adc is None) and a previously-live connection going stale (frame stops updating).
    let adc_link_chain = SensorDigitalInputChain::new(
        Box::new(AdcLinkStatusProvider::new(adc.clone())),
        vec![],
        Box::new(GenericDigitalSensor::new("HwAdcLink".to_string(), "ADC LINK".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(adc_link_chain);

    let Some(frame) = adc else {
        log::info!("ADC unavailable — real sensor set will be empty");
        return mgr;
    };

    // STM32 frame layout (after stripping '$'):
    //   A0, A1, A2, A3, TACHO, SPEED, D0..D9, B0..B7
    //
    // All digital values are pre-normalized by STM32 (1=active, 0=inactive),
    // so Level::High is the active level for every digital sensor here.
    // Analog channels are 12-bit (0-4095); scale factors need calibration.

    // ---- Digital sensor chains ----

    let brake_fluid_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwBrakeFluidLvlLow, 10, frame.clone())),  // D4
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwBrakeFluidLvlLow".to_string(), "Brake Fluid Level".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(brake_fluid_chain);

    let charge_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwCharge, 8, frame.clone())),  // D2
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwCharge".to_string(), "ЗАРЯД".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(charge_chain);

    // HwCheckEngine: no STM32 input — omitted from real sensor set

    let diff_lock_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwDiffLock, 15, frame.clone())),  // D9
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwDiffLock".to_string(), "БЛОК ДИФФ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(diff_lock_chain);

    let ext_lights_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwExtLights, 9, frame.clone())),  // D3
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwExtLights".to_string(), "ГАБАРИТ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(ext_lights_chain);

    let fuel_lvl_low_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwFuelLvlLow, 7, frame.clone())),  // D1
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwFuelLvlLow".to_string(), "УРОВ ТОПЛ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(fuel_lvl_low_chain);

    let high_beam_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwHighBeam, 13, frame.clone())),  // D7
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwHighBeam".to_string(), "ДАЛЬНИЙ СВЕТ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(high_beam_chain);

    // D3 (ext lights / parking lights) also drives instrument illumination on Niva
    let instr_illum_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwInstrIllum, 9, frame.clone())),  // D3
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwInstrIllum".to_string(), "ОСВЕЩ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(instr_illum_chain);

    let oil_press_low_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwOilPressLow, 6, frame.clone())),  // D0
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwOilPressLow".to_string(), "ДАВЛ МАСЛА".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(oil_press_low_chain);

    let park_brake_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwParkBrake, 14, frame.clone())),  // D8
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwParkBrake".to_string(), "СТОЯН ТОРМ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(park_brake_chain);

    let speed_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwSpeed, 5, frame.clone())),  // SPEED pulse count
        vec![],
        Box::new(SpeedSensor::new()),
    );
    mgr.add_digital_sensor_chain(speed_chain);

    let tacho_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwTacho, 4, frame.clone())),  // TACHO pulse count
        vec![Box::new(DigitalSignalDebouncer::new(3, std::time::Duration::from_millis(10)))],
        Box::new(GenericDigitalSensor::new("HwTacho".to_string(), "ТАХОМЕТР".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(tacho_chain);

    let turn_signal_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwTurnSignal, 12, frame.clone())),  // D6
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwTurnSignal".to_string(), "ИНД ПОВОР".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(turn_signal_chain);

    // ---- Analog sensor chains ----
    // Scale factors from test setup; calibration for 12-bit ADC range (0-4095) is pending.

    let voltage_12v_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::Hw12v, 3, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new("Hw12v".to_string(), "БОРТ СЕТЬ".to_string(), "В".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 20.0, Some(11.0), Some(13.0), Some(14.7), Some(15.0)), 0.02)),
    );
    mgr.add_analog_sensor_chain(voltage_12v_chain);

    let fuel_level_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwFuelLvl, 1, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(15))],
        Box::new(GenericAnalogSensor::new("HwFuelLvl".to_string(), "УРОВ ТОПЛ".to_string(), "%".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None), 0.1)),
    );
    mgr.add_analog_sensor_chain(fuel_level_chain);

    let oil_pressure_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwOilPress, 0, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10))],
        Box::new(GenericAnalogSensor::new("HwOilPress".to_string(), "ДАВЛ МАСЛА".to_string(), "кгс/см²".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 8.0, Some(0.5), Some(1.0), Some(7.0), Some(8.0)), 0.01)),
    );
    mgr.add_analog_sensor_chain(oil_pressure_chain);

    let temperature_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwEngineCoolantTemp, 2, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(20))],
        Box::new(EngineTemperatureSensor::new()),
    );
    mgr.add_analog_sensor_chain(temperature_chain);

    log::info!("✓ Sensor manager initialized with ADC sensor chains");

    mgr
}

// Physical MFD buttons (B0..B7), read from the same STM32 ADC frame as the sensors
// (indices 16-23, after A0-A3/TACHO/SPEED/D0-D9). Kept in its own SensorManager, separate
// from setup_sensors' self-test/functional swap, so buttons work from the very first frame.
// No debouncer — the STM32 already debounces buttons over 8 samples at 50Hz before
// setting B0..B7; can add one here later if that turns out to be insufficient.
fn setup_button_sensors(adc: Option<ADCFrame>) -> SensorManager {
    let mut mgr = SensorManager::new();
    // Lets adc_link_down() suppress "channel not in frame" log spam while the ADC
    // reconnect loop is doing its thing (see AdcDataProvider).
    mgr.set_adc_frame(adc.clone());

    let Some(frame) = adc else {
        log::info!("ADC unavailable — physical buttons will not respond");
        return mgr;
    };

    let button_inputs = [
        (HWInput::HwButton0, 16),
        (HWInput::HwButton1, 17),
        (HWInput::HwButton2, 18),
        (HWInput::HwButton3, 19),
        (HWInput::HwButton4, 20),
        (HWInput::HwButton5, 21),
        (HWInput::HwButton6, 22),
        (HWInput::HwButton7, 23),
    ];

    for (input, channel) in button_inputs {
        let chain = SensorDigitalInputChain::new(
            Box::new(ADCChannelProvider::new(input, channel, frame.clone())),
            vec![],
            Box::new(GenericDigitalSensor::new(format!("{:?}", input), format!("{:?}", input),
                                               Level::High, ValueConstraints::digital_default())),
        );
        mgr.add_digital_sensor_chain(chain);
    }

    log::info!("✓ Button sensor manager initialized");

    mgr
}

// Builds the input sources for page navigation: physical buttons (backed by the button
// sensor manager above) plus keyboard input for development/debugging on a TTY.
fn setup_input_sources(button_sensors: SensorManager) -> Vec<Box<dyn InputSource>> {
    let mut sources: Vec<Box<dyn InputSource>> = vec![Box::new(PhysicalButtonInput::new(button_sensors))];
    match KeyboardInput::try_new() {
        Ok(kb) => sources.push(Box::new(kb)),
        Err(e) => log::info!("Keyboard input unavailable (no TTY?): {}", e),
    }
    sources
}

fn setup_ui_style() -> graphics::ui_style::UIStyle {
    let ui_style = graphics::ui_style::UIStyle::new();
    // ui_style.read_from_file("/etc/niva_dashboard/ui_style.json").unwrap_or_else(|e| {
    //     print!("Warning: Failed to read UI style config: {}\r\n", e);
    // });
    ui_style
}

fn setup_adc_data_provider() -> Result<ADCDataProvider, std::string::String> {
    // "/dev/niva_adc" is the udev symlink for the STM32 ADC module. The provider's
    // background thread owns connecting (and reconnecting) to this port, so this succeeds
    // even if the device is not yet plugged in — the ADC link alert (AdcLinkStatusProvider)
    // covers "not connected" until the thread's retry loop picks the device up.
    let mut provider = ADCDataProvider::new("/dev/niva_adc", 115200);
    provider.run().map_err(|e| e.to_string())?;
    Ok(provider)
}

fn setup_ups_monitor() -> Result<UpsMonitor, String> {
    let mut monitor = UpsMonitor::new();
    monitor.run()?;
    Ok(monitor)
}

fn show_help() {
    log::info!("Available test modes:");
    log::info!("1. Basic OpenGL triangle test");
    log::info!("2. OpenGL text rendering test with FreeType");
    log::info!("3. Dashboard performance test (9 animated gauges)");
    log::info!("4. Rotating needle gauge test (circular gauge with numbers)");
    log::info!("5. GPIO input test");
    log::info!("6. Sensor manager test");
    log::info!("7. Digital segmented display test");
    log::info!("8. Indicator zero position test (needle and bar gauges at minimum)");
    log::info!("9. Indicator middle position test (needle and bar gauges at 50%)");
    log::info!("10. Indicator maximum position test (needle and bar gauges at maximum)");
}

fn main() -> std::process::ExitCode {
    // Keep the handle alive for the whole process — dropping it early stops the logger's
    // background writer thread.
    let _logger_handle = init_logging();
    crate::util::shutdown::install_signal_handlers();
    crate::util::shutdown::watch_for_binary_update();

    let args: Vec<String> = env::args().collect();

    log::info!("Niva Dashboard - Raspberry Pi Version (KMS/DRM Backend)");
    log::info!("Usage: cargo run -- [help|test={{basic|gltext|dashboard|needle|gpio|sensors|digital|ind_zero_pos|ind_middle_pos|ind_max_pos}}]");

    for arg in args {
        let parm = arg.split("=").collect::<Vec<&str>>();
        if parm.len() == 2 {
            match parm[0] {
                "test" => {
                    run_test(parm[1]);
                    return std::process::ExitCode::SUCCESS;
                }
                _ => {
                    log::warn!("Unknown argument: {}", parm[0]);
                }
            }
        } else {
            match arg.as_str() {
                "help" => {
                    show_help();
                    return std::process::ExitCode::SUCCESS;
                }
                _ => {
                    log::warn!("Unknown argument: {}", arg);
                }
            }
        }
    }

    // Kept alive for the process lifetime — its Drop impl stops the background thread
    // cleanly on shutdown. Started unconditionally and independently of graphics/sensors
    // so it keeps monitoring even if later setup steps fail.
    let _ups_monitor = match setup_ups_monitor() {
        Ok(monitor) => {
            log::info!("✓ UPS monitor started");
            Some(monitor)
        }
        Err(e) => {
            log::warn!("UPS monitor unavailable: {}", e);
            None
        }
    };

    let adc = match setup_adc_data_provider() {
        Ok(provider) => {
            log::info!("✓ ADC data provider started");
            Some(provider)
        }
        Err(e) => {
            log::info!("ADC data provider unavailable: {}", e);
            None
        }
    };
    // Obtain a frame handle before moving adc into setup_sensors
    let adc_frame = adc.as_ref().map(|p| p.frame());

    // Temporary diagnostic: log raw ADC frame contents once a second to verify
    // the serial reader thread is actually receiving data from the STM32 module.
    // if let Some(frame) = adc_frame.clone() {
    //     thread::spawn(move || loop {
    //         log::info!("ADC frame: {:?}", frame.get_data());
    //         thread::sleep(Duration::from_secs(1));
    //     });
    // }

    let context = setup_context();
    let self_test_sensors = setup_self_test_sensors();
    let button_sensors = setup_button_sensors(adc_frame.clone());
    let input_sources = setup_input_sources(button_sensors);
    let sensors = setup_sensors(adc_frame);
    let ui_style = setup_ui_style();

    let mut mgr = PageManager::new(context, self_test_sensors, ui_style, input_sources);

    mgr.setup().expect("Failed to setup page manager");

    // Setup timer to switch self-tests sensor manager to functional set after 5 seconds
    let sender = mgr.get_smart_event_sender();
    let sensor_config_tx = mgr.get_sensor_config_tx();
    let thread_handle = thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));      // Switch sensor manager after 5 seconds
        log::info!("Switching sensor set...");
        sensor_config_tx.send(sensors).ok();        // Send new sensor manager
        sender.send(UIEvent::SwitchSensorSet);      // Signal event handler to poll sensor_config channel
    });

    // Exit code doubles as a restart signal for the auto-start login script: a clean
    // exit (0) means the dashboard quit intentionally (e.g. 'q' for debugging) and should
    // not be relaunched, a non-zero code means it crashed and should be restarted after
    // a delay, and BINARY_UPDATED_EXIT_CODE means it quit because it was rebuilt and
    // should be restarted immediately with the new binary.
    let exit_code = match mgr.start() {
        Ok(()) if crate::util::shutdown::binary_updated() => {
            log::info!("Restarting to pick up newly built binary");
            std::process::ExitCode::from(crate::util::shutdown::BINARY_UPDATED_EXIT_CODE)
        }
        Ok(()) => {
            log::info!("Dashboard finished successfully!");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            log::error!("Failed to start dashboard: {}", e);
            std::process::ExitCode::FAILURE
        }
    };

    thread_handle.join().unwrap();

    exit_code
}