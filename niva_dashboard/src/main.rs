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
use crate::alerts::alert_manager::AlertManager;
use crate::page_framework::input::{InputSource, PhysicalButtonInput, KeyboardInput};
use crate::hardware::sensor_manager::{SensorManager, SensorDigitalInputChain, SensorAnalogInputChain};
use crate::hardware::hw_providers::*;
use crate::hardware::digital_signal_processing::DigitalSignalDebouncer;
use crate::hardware::analog_signal_processing::AnalogSignalProcessorMovingAverage;
use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor, SpeedSensor, TachoSensor, EngineTemperatureSensor, GnssAltitudeSensor};
use crate::hardware::sensor_value::ValueConstraints;
use crate::hardware::heading_fusion_sensor;
use crate::util::adc_data_provider::{ADCDataProvider, ADCFrame, TestADCDataProvider, SELF_TEST_DURATION};
use crate::util::bno085_data_provider::{Bno085DataProvider, Bno085Frame};
use crate::util::bno085_protocol::{
    SH2_REPORT_ROTATION_VECTOR, SH2_REPORT_GAME_ROTATION_VECTOR,
    SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR, SH2_REPORT_ACCELEROMETER,
};
use crate::util::gnss_data_provider::{GnssDataProvider, GnssFrame};
use crate::util::logging::init_logging;
use crate::util::ups_monitor::UpsMonitor;
use crate::util::ups_i2c_provider::{UpsI2CDataProvider, UpsRawFrame};
use crate::hardware::sensors::{UpsCurrentSensor, UpsChargeSensor};
use rppal::gpio::Level;
use std::env;
use std::thread;

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

// Self-test sensor manager: wires the exact same ADC-backed chains as production
// (add_adc_sensor_chains) but points them at a TestADCDataProvider's synthetic frame instead
// of the real serial-fed one, so the startup self-test sweep exercises the same debounce/
// averaging/threshold code every real reading goes through — unlike two independently
// hand-written chain sets, this can't silently drift out of sync.
//
// Caller must keep the returned TestADCDataProvider alive for the sweep to animate —
// dropping it stops the synthetic writer thread.
fn setup_self_test_sensors() -> (SensorManager, TestADCDataProvider) {
    let mut mgr = SensorManager::new();
    let test_adc = TestADCDataProvider::start();
    add_adc_sensor_chains(&mut mgr, test_adc.frame());

    log::info!("✓ Self-test sensor manager initialized (synthetic ADC sweep)");

    (mgr, test_adc)
}

fn setup_sensors(adc: Option<ADCFrame>, ups: Option<UpsRawFrame>, gnss: Option<GnssFrame>, bno: Option<Bno085Frame>) -> (SensorManager, Option<heading_fusion_sensor::HeadingFusionSensor>) {
    let mut mgr = SensorManager::new();
    // Cloned before the GNSS scalar-chain block below consumes `gnss` -- needed again for the
    // heading fusion chain further down.
    let gnss_for_fusion = gnss.clone();
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

    // GNSS link-health chain — added unconditionally for the same reason as the ADC one
    // above: the receiver never having connected and a live link going stale should both
    // surface as HwGnssLink going active.
    let gnss_link_chain = SensorDigitalInputChain::new(
        Box::new(GnssLinkStatusProvider::new(gnss.clone())),
        vec![],
        Box::new(GenericDigitalSensor::new("HwGnssLink".to_string(), "GNSS LINK".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(gnss_link_chain);

    // BNO085 link-health chain — added unconditionally for the same reason as ADC/GNSS
    // above. Independent of the heading fusion chain below: this reports raw connectivity
    // (feeds the GNSS page's "ИНС" status box), while the fusion chain separately decides
    // what to do about a stale/missing BNO085 reading.
    let bno085_link_chain = SensorDigitalInputChain::new(
        Box::new(Bno085LinkStatusProvider::new(bno.clone())),
        vec![],
        Box::new(GenericDigitalSensor::new("HwBno085Link".to_string(), "ИНС LINK".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(bno085_link_chain);

    // GNSS scalar sensor chains — independent of the ADC link, same as UPS below.
    if let Some(gnss_frame) = gnss {
        let gnss_speed_chain = SensorAnalogInputChain::new(
            Box::new(GnssChannelProvider::new(HWInput::HwGnssSpeed, gnss_frame.clone())),
            vec![],
            Box::new(GenericAnalogSensor::new("gnss_speed".to_string(), "СКОР ГНСС".to_string(), "км/ч".to_string(),
                                              ValueConstraints::analog(0.0, 200.0), 1.0 / GNSS_SPEED_SCALE)),
        );
        mgr.add_analog_sensor_chain(gnss_speed_chain);

        let gnss_heading_chain = SensorAnalogInputChain::new(
            Box::new(GnssChannelProvider::new(HWInput::HwGnssMovingHeading, gnss_frame.clone())),
            vec![],
            Box::new(GenericAnalogSensor::new("gnss_heading".to_string(), "АЗИМУТ".to_string(), "°".to_string(),
                                              ValueConstraints::analog(0.0, 359.9), 1.0 / GNSS_HEADING_SCALE)),
        );
        mgr.add_analog_sensor_chain(gnss_heading_chain);

        let gnss_altitude_chain = SensorAnalogInputChain::new(
            Box::new(GnssChannelProvider::new(HWInput::HwGnssAltitude, gnss_frame.clone())),
            vec![],
            Box::new(GnssAltitudeSensor::new()),
        );
        mgr.add_analog_sensor_chain(gnss_altitude_chain);

        // Warning below 4 satellites: the minimum needed for a 3D fix, not a display
        // preference — below that, GNSS position/altitude can't be trusted regardless of
        // what fix_quality currently reports.
        let gnss_satellites_chain = SensorAnalogInputChain::new(
            Box::new(GnssChannelProvider::new(HWInput::HwGnssSatellites, gnss_frame.clone())),
            vec![],
            Box::new(GenericAnalogSensor::new("gnss_satellites".to_string(), "СПУТНИКИ".to_string(), "".to_string(),
                                              ValueConstraints::analog_with_thresholds(0.0, 99.0, None, Some(4.0), None, None), 1.0)),
        );
        mgr.add_analog_sensor_chain(gnss_satellites_chain);

        // Fix quality codes (see nmea::FixQuality) aren't linearly ordered by quality —
        // e.g. code 6 (Estimated) is worse than code 4 (RtkFixed) despite the higher
        // number — so no warning/critical thresholds are set here; this is a raw
        // informational readout, not a gauge value.
        let gnss_fix_quality_chain = SensorAnalogInputChain::new(
            Box::new(GnssChannelProvider::new(HWInput::HwGnssFixQuality, gnss_frame)),
            vec![],
            Box::new(GenericAnalogSensor::new("gnss_fix_quality".to_string(), "ТИП ФИКС".to_string(), "".to_string(),
                                              ValueConstraints::analog(0.0, 8.0), 1.0)),
        );
        mgr.add_analog_sensor_chain(gnss_fix_quality_chain);

        log::info!("✓ GNSS sensor chains added");
    } else {
        log::info!("GNSS data provider unavailable — GNSS sensor chains omitted");
    }

    // Heading fusion sensor — reads GnssFrame/Bno085Frame directly rather than through a
    // sensor chain (see hardware::heading_fusion_sensor, HEADING_FUSION_DESIGN.md); ticked
    // once per event-loop iteration by PageManager, independent of this SensorManager's
    // self-test/real handoff. Requires both sources' frames to exist (i.e. both background
    // threads spawned, regardless of whether either device is actually connected yet — see
    // Bno085ChannelProvider/GnssChannelProvider, which report per-read failures for "not
    // connected"/"no fix" independently of this).
    let heading_fusion = match (bno, gnss_for_fusion) {
        (Some(bno_frame), Some(gnss_frame)) => {
            log::info!("✓ Heading fusion sensor added (BNO085 + GNSS)");
            Some(heading_fusion_sensor::HeadingFusionSensor::new(gnss_frame, bno_frame))
        }
        _ => {
            log::info!("BNO085 or GNSS data provider unavailable — heading fusion sensor omitted");
            None
        }
    };

    // UPS sensor chains — added unconditionally alongside the ADC link chain, since the UPS
    // HAT is separate I2C hardware and its availability doesn't depend on the STM32 ADC link.
    if let Some(ups_frame) = ups {
        let ups_current_chain = SensorAnalogInputChain::new(
            Box::new(UPSDataProvider::new(HWInput::HwUPSCurrent, ups_frame.clone())),
            vec![],
            Box::new(UpsCurrentSensor::new()),
        );
        mgr.add_analog_sensor_chain(ups_current_chain);

        let ups_charge_chain = SensorAnalogInputChain::new(
            Box::new(UPSDataProvider::new(HWInput::HwUPSChargeState, ups_frame)),
            vec![],
            Box::new(UpsChargeSensor::new()),
        );
        mgr.add_analog_sensor_chain(ups_charge_chain);

        log::info!("✓ UPS sensor chains added");
    } else {
        log::info!("UPS I2C provider unavailable — UPS sensor chains omitted");
    }

    let Some(frame) = adc else {
        log::info!("ADC unavailable — real sensor set will be empty");
        return (mgr, heading_fusion);
    };

    add_adc_sensor_chains(&mut mgr, frame);
    log::info!("✓ Sensor manager initialized with ADC sensor chains");

    (mgr, heading_fusion)
}

// STM32 frame layout (after stripping '$'):
//   A0, A1, A2, A3, TACHO, SPEED, D0..D9, B0..B7
//
// All digital values are pre-normalized by STM32 (1=active, 0=inactive),
// so Level::High is the active level for every digital sensor here.
// Analog channels are 12-bit (0-4095); scale factors need calibration.
//
// Shared by setup_sensors (real, serial-fed ADCFrame) and setup_self_test_sensors
// (TestADCDataProvider's synthetic ADCFrame) — self-test exercises this exact wiring
// instead of a hand-duplicated copy, so the two can't silently drift apart.
fn add_adc_sensor_chains(mgr: &mut SensorManager, frame: ADCFrame) {
    // ---- Digital sensor chains ----

    let brake_fluid_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwBrakeFluidLvlLow, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwBrakeFluidLvlLow".to_string(), "Brake Fluid Level".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(brake_fluid_chain);

    let charge_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwCharge, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwCharge".to_string(), "ЗАРЯД".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(charge_chain);

    // HwCheckEngine: no STM32 input — omitted from real sensor set

    let diff_lock_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwDiffLock, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwDiffLock".to_string(), "БЛОК ДИФФ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(diff_lock_chain);

    let ext_lights_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwExtLights, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwExtLights".to_string(), "ГАБАРИТ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(ext_lights_chain);

    let fuel_lvl_low_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwFuelLvlLow, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwFuelLvlLow".to_string(), "УРОВ ТОПЛ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(fuel_lvl_low_chain);

    let high_beam_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwHighBeam, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwHighBeam".to_string(), "ДАЛЬНИЙ СВЕТ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(high_beam_chain);

    // D3 (ext lights / parking lights) also drives instrument illumination on Niva
    let instr_illum_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwInstrIllum, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwInstrIllum".to_string(), "ОСВЕЩ".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(instr_illum_chain);

    let oil_press_low_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwOilPressLow, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwOilPressLow".to_string(), "ДАВЛ МАСЛА".to_string(),
                                           Level::High, ValueConstraints::digital_critical())),
    );
    mgr.add_digital_sensor_chain(oil_press_low_chain);

    let park_brake_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwParkBrake, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwParkBrake".to_string(), "СТОЯН ТОРМ".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(park_brake_chain);

    let turn_signal_chain = SensorDigitalInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwTurnSignal, frame.clone())),
        vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(50)))],
        Box::new(GenericDigitalSensor::new("HwTurnSignal".to_string(), "ИНД ПОВОР".to_string(),
                                           Level::High, ValueConstraints::digital_default())),
    );
    mgr.add_digital_sensor_chain(turn_signal_chain);

    // ---- Analog sensor chains ----
    // Scale factors from test setup; calibration for 12-bit ADC range (0-4095) is pending.

    let voltage_12v_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::Hw12v, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(60))],
        Box::new(GenericAnalogSensor::new("Hw12v".to_string(), "БОРТ СЕТЬ".to_string(), "В".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 20.0, Some(11.0), Some(13.0), Some(14.7), Some(15.0)), 0.02)),
    );
    mgr.add_analog_sensor_chain(voltage_12v_chain);

    let fuel_level_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwFuelLvl, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(60*60))],         // Fuel level changes slowly - average over a minute (60 samples per second times 60 seconds)
        Box::new(GenericAnalogSensor::new("HwFuelLvl".to_string(), "УРОВ ТОПЛ".to_string(), "%".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 100.0, Some(10.0), Some(20.0), None, None), 0.1)),
    );
    mgr.add_analog_sensor_chain(fuel_level_chain);

    let oil_pressure_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwOilPress, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(60))],
        Box::new(GenericAnalogSensor::new("HwOilPress".to_string(), "ДАВЛ МАСЛА".to_string(), "кгс/см²".to_string(),
                                          ValueConstraints::analog_with_thresholds(0.0, 8.0, Some(0.5), Some(1.0), Some(7.0), Some(8.0)), 0.01)),
    );
    mgr.add_analog_sensor_chain(oil_pressure_chain);

    let temperature_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwEngineCoolantTemp, frame.clone())),
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(10*60))],      // Average over 10 seconds
        Box::new(EngineTemperatureSensor::new()),
    );
    mgr.add_analog_sensor_chain(temperature_chain);

    let speed_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwSpeed, frame.clone())),  // inter-pulse period, raw timer ticks
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(5))],
        Box::new(SpeedSensor::new()),
    );
    mgr.add_analog_sensor_chain(speed_chain);

    let tacho_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwTacho, frame.clone())),  // inter-pulse period, raw timer ticks
        vec![Box::new(AnalogSignalProcessorMovingAverage::new(5))],
        Box::new(TachoSensor::new()),
    );
    mgr.add_analog_sensor_chain(tacho_chain);
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
        HWInput::HwButton0,
        HWInput::HwButton1,
        HWInput::HwButton2,
        HWInput::HwButton3,
        HWInput::HwButton4,
        HWInput::HwButton5,
        HWInput::HwButton6,
        HWInput::HwButton7,
    ];

    for input in button_inputs {
        let chain = SensorDigitalInputChain::new(
            Box::new(ADCChannelProvider::new(input, frame.clone())),
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

fn setup_gnss_data_provider() -> Result<GnssDataProvider, String> {
    // "/dev/niva_gps" is the udev symlink for the GNSS receiver (UM982, enumerating as a
    // CH340 USB-serial adapter). Same reconnect-owning-thread pattern as the ADC provider:
    // this succeeds even if the receiver isn't plugged in yet.
    let mut provider = GnssDataProvider::new("/dev/niva_gps", 115200);
    provider.run().map_err(|e| e.to_string())?;
    Ok(provider)
}

fn setup_ups_i2c_provider() -> Result<UpsI2CDataProvider, String> {
    let mut provider = UpsI2CDataProvider::new();
    provider.run().map_err(|e| e.to_string())?;
    Ok(provider)
}

fn setup_bno085_data_provider() -> Result<Bno085DataProvider, String> {
    // BNO085 connects via I2C directly (owned by the provider's own background thread, no
    // serial device path) -- see bno085_data_provider.rs. Rotation Vector feeds the raw
    // diagnostic HwBno085Heading reading (Bno085ChannelProvider); Game Rotation Vector feeds
    // heading_fusion_sensor's continuously-integrated backbone (see HEADING_FUSION_DESIGN.md
    // -- the full Rotation Vector's magnetometer fusion was excluded from the fusion policy
    // itself as too error-prone in testing, but stays wired for the raw comparison reading).
    // Geomagnetic RV/Accelerometer are read directly from Bno085Frame by GnssPage's raw INS
    // diagnostics block (InfoBlocks::InsData) rather than through the HWInput/sensor-chain
    // pipeline -- same rationale as GnssPage reading GnssFrame's composite fields directly.
    let mut provider = Bno085DataProvider::new(&[
        SH2_REPORT_ROTATION_VECTOR, SH2_REPORT_GAME_ROTATION_VECTOR,
        SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR, SH2_REPORT_ACCELEROMETER,
    ]);
    provider.run().map_err(|e| e.to_string())?;
    Ok(provider)
}

fn show_help() {
    log::info!("Available test modes:");
    log::info!("1. Rotating needle gauge test (circular gauge with numbers)");
    log::info!("2. GPIO input test");
    log::info!("3. Digital segmented display test");
    log::info!("4. Indicator zero position test (needle and bar gauges at minimum)");
    log::info!("5. Indicator middle position test (needle and bar gauges at 50%)");
    log::info!("6. Indicator maximum position test (needle and bar gauges at maximum)");
    log::info!("7. Fuel level grid stress test");
    log::info!("8. Compass indicator test (synthetic GNSS heading sweep)");
    log::info!("9. BNO085 IMU test (streams heading/pitch/roll/acceleration to console)");
    log::info!("10. GNSS/BNO085 heading accuracy test (streams heading+accuracy from both to console)");
    log::info!("11. Oscilloscope burst capture test (sends $OSCCAP, validates the captured buffer)");
}

fn main() -> std::process::ExitCode {
    // Keep the handle alive for the whole process — dropping it early stops the logger's
    // background writer thread.
    let _logger_handle = init_logging();
    crate::util::shutdown::install_signal_handlers();
    crate::util::shutdown::watch_for_binary_update();

    let args: Vec<String> = env::args().collect();

    log::info!("Niva Dashboard - Raspberry Pi Version (KMS/DRM Backend)");
    log::info!("Usage: cargo run -- [help|test={{needle|gpio|digital|ind_zero_pos|ind_middle_pos|ind_max_pos|fuel_grid|compass|bno085|heading|osc_capture}}]");

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
    // so it keeps polling even if later setup steps fail.
    let ups_i2c_provider = match setup_ups_i2c_provider() {
        Ok(provider) => {
            log::info!("✓ UPS I2C provider started");
            Some(provider)
        }
        Err(e) => {
            log::warn!("UPS I2C provider unavailable: {}", e);
            None
        }
    };
    // Obtain a frame handle before moving ups_i2c_provider into the binding that keeps it alive.
    let ups_frame = ups_i2c_provider.as_ref().map(|p| p.frame());

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
    // Obtain frame handles before moving adc into setup_sensors
    let adc_frame = adc.as_ref().map(|p| p.frame());
    let osc_frame = adc.as_ref().map(|p| p.osc_frame());

    // Kept alive for the process lifetime, same as `adc` — its Drop impl stops the
    // background thread cleanly on shutdown.
    let gnss = match setup_gnss_data_provider() {
        Ok(provider) => {
            log::info!("✓ GNSS data provider started");
            Some(provider)
        }
        Err(e) => {
            log::info!("GNSS data provider unavailable: {}", e);
            None
        }
    };
    let gnss_frame = gnss.as_ref().map(|p| p.frame());

    // Kept alive for the process lifetime, same as `gnss` — its Drop impl stops the
    // background thread cleanly on shutdown.
    let bno085 = match setup_bno085_data_provider() {
        Ok(provider) => {
            log::info!("✓ BNO085 data provider started");
            Some(provider)
        }
        Err(e) => {
            log::info!("BNO085 data provider unavailable: {}", e);
            None
        }
    };
    let bno_frame = bno085.as_ref().map(|p| p.frame());

    // Temporary diagnostic: log raw ADC frame contents once a second to verify
    // the serial reader thread is actually receiving data from the STM32 module.
    // if let Some(frame) = adc_frame.clone() {
    //     thread::spawn(move || loop {
    //         log::info!("ADC frame: {:?}", frame.get_data());
    //         thread::sleep(Duration::from_secs(1));
    //     });
    // }

    let context = setup_context();
    let (self_test_sensors, test_adc_provider) = setup_self_test_sensors();
    let button_sensors = setup_button_sensors(adc_frame.clone());
    let input_sources = setup_input_sources(button_sensors);
    // Keep a handle for the ADC/GNSS diagnostic terminal pages before the sensor-chain
    // setup consumes the rest of their clones.
    let adc_frame_for_diag = adc_frame.clone();
    let gnss_frame_for_diag = gnss_frame.clone();
    let bno_frame_for_diag = bno_frame.clone();
    let (sensors, heading_fusion) = setup_sensors(adc_frame, ups_frame, gnss_frame, bno_frame);
    let ui_style = setup_ui_style();
    // Starts disabled: alerts (e.g. engine temp, oil pressure) must not fire against the
    // synthetic self-test sensor sweep. Enabled once the self-test sequence hands off to
    // the real sensor set (PageManager's UIEvent::SwitchSensorSet handler).
    let alert_manager = AlertManager::new(false, &ui_style);

    let mut mgr = PageManager::new(context, self_test_sensors, ui_style, input_sources, UpsMonitor::new(), adc_frame_for_diag, osc_frame, gnss_frame_for_diag, bno_frame_for_diag, alert_manager, heading_fusion);

    mgr.setup().expect("Failed to setup page manager");

    // Setup timer to switch the self-test sensor manager to the functional set once the
    // self-test sweep finishes. test_adc_provider is moved in so its synthetic writer
    // thread keeps animating the self-test sweep until the moment of the switch, then
    // stops (via Drop) as soon as this closure returns.
    let sender = mgr.get_smart_event_sender();
    let sensor_config_tx = mgr.get_sensor_config_tx();
    let thread_handle = thread::spawn(move || {
        thread::sleep(SELF_TEST_DURATION);
        log::info!("Switching sensor set...");
        drop(test_adc_provider);                    // Stop the self-test ADC sweep
        sensor_config_tx.send(sensors).ok();        // Send new sensor manager
        sender.send(UIEvent::SwitchSensorSet);      // Signal event handler to poll sensor_config channel
    });

    // Exit code doubles as a restart signal for the auto-start login script: a clean
    // exit (0) means the dashboard quit intentionally (e.g. 'q' for debugging) and should
    // not be relaunched, a non-zero code means it crashed and should be restarted after
    // a delay, and RESTART_EXIT_CODE means it quit because it was rebuilt (or a restart
    // was explicitly requested via SIGUSR1) and should be restarted immediately.
    let exit_code = match mgr.start() {
        Ok(()) if crate::util::shutdown::binary_updated() => {
            log::info!("Restarting to pick up newly built binary");
            std::process::ExitCode::from(crate::util::shutdown::RESTART_EXIT_CODE)
        }
        Ok(()) if crate::util::shutdown::restart_requested() => {
            log::info!("Restarting: requested via SIGUSR1");
            std::process::ExitCode::from(crate::util::shutdown::RESTART_EXIT_CODE)
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