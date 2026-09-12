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
use crate::hardware::analog_signal_processing::{AnalogSignalProcessor, AnalogSignalProcessorMovingAverage};
use crate::hardware::sensors::{GenericDigitalSensor, GenericAnalogSensor, SpeedSensor, TachoSensor, GnssAltitudeSensor};
use crate::hardware::sensor_value::ValueConstraints;
use crate::hardware::heading_fusion_sensor;
use crate::util::adc_data_provider::{ADCDataProvider, ADCFrame, AdcTempFrame, TestADCDataProvider, SELF_TEST_DURATION};
use crate::util::bno085_data_provider::{Bno085DataProvider, Bno085Frame};
use crate::util::bno085_protocol::{
    SH2_REPORT_ROTATION_VECTOR, SH2_REPORT_GAME_ROTATION_VECTOR,
    SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR, SH2_REPORT_ACCELEROMETER,
};
use crate::util::gnss_data_provider::{GnssDataProvider, GnssFrame};
use crate::util::gnss_time_sync::GnssTimeSync;
use crate::util::logging::init_logging;
use crate::util::ups_monitor::UpsMonitor;
use crate::util::ups_i2c_provider::{UpsI2CDataProvider, UpsRawFrame};
use crate::hardware::sensors::{UpsCurrentSensor, UpsChargeSensor};
use crate::hardware::gpio_input::{GpioInput, GpioInputConfig, GpioOutput};
use rppal::gpio::{Level, Bias};
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

// Self-test sensor manager: wires the exact same ADC-backed chains as production
// (add_adc_sensor_chains) but points them at a TestADCDataProvider's synthetic frame instead
// of the real serial-fed one, so the startup self-test sweep exercises the same debounce/
// averaging/threshold code every real reading goes through — unlike two independently
// hand-written chain sets, this can't silently drift out of sync.
//
// Caller must keep the returned TestADCDataProvider alive for the sweep to animate —
// dropping it stops the synthetic writer thread. The sweep clock is not started here:
// the caller must call begin_sweep() once the render loop is about to start, otherwise
// the ~1 s of remaining startup (pages/indicators/GL) eats the rise-and-fall animation.
fn setup_self_test_sensors() -> Result<(SensorManager, TestADCDataProvider), String> {
    let mut mgr = SensorManager::new();
    let test_adc = TestADCDataProvider::deferred();
    // bypass_analog_filters: the production moving averages (coolant 600 ≈ 10 s, fuel 3600
    // ≈ 60 s) are tuned for driving-noise rejection and only smear the 2 s bench sweep — at
    // ~60 Hz reads they never fill, so they act as a cumulative mean that never tracks the
    // envelope. Drop them here so every needle follows the sweep directly.
    add_adc_sensor_chains(&mut mgr, test_adc.frame(), test_adc.temp_frame(), true)?;

    // Test sensor chain for the `СМОТРИ ЭКРАН` alert.
    let test_alert_link_chain = SensorDigitalInputChain::new(
        Box::new(TestDigitalDataProvider::new(HWInput::HwTestAlertInput).with_timeout(Duration::from_secs(30))),
        vec![],
        Box::new(GenericDigitalSensor::new("HwTestAlertInput".to_string(), "TEST ALERT".to_string(),
                                           Level::High, ValueConstraints::digital_warning())),
    );
    mgr.add_digital_sensor_chain(test_alert_link_chain);

    log::info!("✓ Self-test sensor manager initialized (synthetic ADC sweep)");

    Ok((mgr, test_adc))
}

fn setup_sensors(adc: Option<ADCFrame>, adc_temp: Option<AdcTempFrame>, ups: Option<UpsRawFrame>, gnss: Option<GnssFrame>, bno: Option<Bno085Frame>) -> Result<(SensorManager, Option<heading_fusion_sensor::HeadingFusionSensor>), String> {
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

    // Master warning button — GPIO27 (pull-up, active-low). Wired directly to the Pi's
    // GPIO rather than through the STM32 ADC module, so it still works while the ADC link
    // is down, and because the STM32 ran out of pins. Added unconditionally alongside the
    // UPS/GNSS/BNO085 chains above; the LED side (GPIO18) isn't wired yet.
    match GpioInput::new(GpioInputConfig { pin_number: 27, bias: Bias::PullUp, active_low: true }) {
        Ok(gpio) => {
            let master_warning_chain = SensorDigitalInputChain::new(
                Box::new(GPIOProvider::new(HWInput::HwMasterWarningBtn, gpio)),
                vec![Box::new(DigitalSignalDebouncer::new(5, std::time::Duration::from_millis(100)))],
                Box::new(GenericDigitalSensor::new("HwMasterWarningBtn".to_string(), "MASTER WARNING".to_string(),
                                                   Level::Low, ValueConstraints::digital_default())),
            );
            mgr.add_digital_sensor_chain(master_warning_chain);
            log::info!("✓ Master warning button chain added (GPIO27)");
        }
        Err(e) => log::warn!("Master warning button GPIO27 unavailable: {}", e),
    }

    let Some(frame) = adc else {
        log::info!("ADC unavailable — real sensor set will be empty");
        return Ok((mgr, heading_fusion));
    };

    // adc_temp comes from the same ADCDataProvider as `frame`, so it is Some whenever `frame`
    // is; fall back to a detached frame rather than unwrap so a future caller can't panic here.
    add_adc_sensor_chains(&mut mgr, frame, adc_temp.unwrap_or_default(), false)?;
    log::info!("✓ Sensor manager initialized with ADC sensor chains");

    Ok((mgr, heading_fusion))
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
//
// `bypass_analog_filters` drops every analog chain's moving-average/dampener stage. Only
// the self-test path passes true: those filters exist to reject driving noise over
// seconds-to-minutes and would just smear its 2 s sweep.
fn add_adc_sensor_chains(mgr: &mut SensorManager, frame: ADCFrame, temp_frame: AdcTempFrame, bypass_analog_filters: bool) -> Result<(), String> {
    // Generic digital/analog chains (brake fluid, charge, diff lock, ext lights, fuel
    // level/low, high beam, instrument illumination, oil pressure/low, park brake, turn
    // signal, 12V) plus the one-wire DS18B20 temperature chains (provider "adc_temp", see
    // ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md) are data-driven — see hardware::sensor_config and
    // DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md. A bad config file is surfaced to the caller (and
    // ultimately shown on screen by main's fallback loop), not panicked on.
    hardware::sensor_config::load_chains_with_options(
        &hardware::sensor_config::default_path(), "sensor",
        frame.clone(), Some(temp_frame), mgr, bypass_analog_filters,
    ).map_err(|e| format!("sensor_config.json: {}", e))?;

    // ---- Chains with real conversion math, out of scope for config (see design doc) ----
    // Coolant temp / oil pressure / fuel level are data-driven `calibrated_analog` chains
    // (datasheet resistance curves + live 12V supply) -- see hardware::sensor_config and
    // SENSOR_CALIBRATION_DESIGN.md. Only the pulse-period sensors stay hand-built here.

    let pulse_filters = || -> Vec<Box<dyn AnalogSignalProcessor + Send>> {
        if bypass_analog_filters { vec![] } else { vec![Box::new(AnalogSignalProcessorMovingAverage::new(5))] }
    };

    let speed_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwSpeed, frame.clone())),  // inter-pulse period, raw timer ticks
        pulse_filters(),
        Box::new(SpeedSensor::new()),
    );
    mgr.add_analog_sensor_chain(speed_chain);

    let tacho_chain = SensorAnalogInputChain::new(
        Box::new(ADCChannelProvider::new(HWInput::HwTacho, frame.clone())),  // inter-pulse period, raw timer ticks
        pulse_filters(),
        Box::new(TachoSensor::new()),
    );
    mgr.add_analog_sensor_chain(tacho_chain);

    Ok(())
}

// Physical MFD buttons (B0..B7), read from the same STM32 ADC frame as the sensors
// (indices 16-23, after A0-A3/TACHO/SPEED/D0-D9). Kept in its own SensorManager, separate
// from setup_sensors' self-test/functional swap, so buttons work from the very first frame.
// No debouncer — the STM32 already debounces buttons over 8 samples at 50Hz before
// setting B0..B7; can add one here later if that turns out to be insufficient.
fn setup_button_sensors(adc: Option<ADCFrame>) -> Result<SensorManager, String> {
    let mut mgr = SensorManager::new();
    // Lets adc_link_down() suppress "channel not in frame" log spam while the ADC
    // reconnect loop is doing its thing (see AdcDataProvider).
    mgr.set_adc_frame(adc.clone());

    let Some(frame) = adc else {
        log::info!("ADC unavailable — physical buttons will not respond");
        return Ok(mgr);
    };

    // Data-driven — see hardware::sensor_config and DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md.
    // No one-wire temperature entries in the "button" group, so no temp frame needed.
    hardware::sensor_config::load_chains(&hardware::sensor_config::default_path(), "button", frame, None, &mut mgr)
        .map_err(|e| format!("sensor_config.json: {}", e))?;

    log::info!("✓ Button sensor manager initialized");

    Ok(mgr)
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
    crate::util::shutdown::watch_for_updates(hardware::sensor_config::default_path());

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
    let adc_temp_frame = adc.as_ref().map(|p| p.temp_frame());
    let osc_frame = adc.as_ref().map(|p| p.osc_frame());
    let adc_version_frame = adc.as_ref().map(|p| p.version_frame());

    // Moved into PageManager below (unlike `adc`, which stays a process-lifetime local) --
    // PageManager pauses/resumes it to hand the GNSS frame off to a synthetic test writer on
    // UIEvent::NavToggleGnssTest (see toggle_gnss_test_mode).
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

    // Kept alive for the process lifetime -- background thread that steps CLOCK_REALTIME
    // from GNSS UTC once locked. No-op until the receiver has an active fix; see
    // gnss_time_sync.rs for why this is needed (no RTC, no NTP path once installed in a car).
    let _gnss_time_sync = gnss_frame.as_ref().map(|f| GnssTimeSync::start(f.clone()));

    // Moved into PageManager below, same as `gnss` — see that comment.
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

    let mut context = setup_context();
    // Keep a handle for the ADC/GNSS diagnostic terminal pages before the sensor-chain
    // setup consumes the rest of their clones.
    let adc_frame_for_diag = adc_frame.clone();
    let gnss_frame_for_diag = gnss_frame.clone();
    let bno_frame_for_diag = bno_frame.clone();

    // All three of these load sensor_config.json. A malformed or unreadable file used to
    // panic here (recoverable only by reading the logs); instead, show the error on screen
    // and idle until watch_for_updates sees the file fixed (or the binary rebuilt) and
    // triggers a restart.
    let sensor_setup = (|| -> Result<_, String> {
        let (self_test_sensors, test_adc_provider) = setup_self_test_sensors()?;
        let button_sensors = setup_button_sensors(adc_frame.clone())?;
        let (sensors, heading_fusion) = setup_sensors(adc_frame, adc_temp_frame, ups_frame, gnss_frame, bno_frame)?;
        Ok((self_test_sensors, test_adc_provider, button_sensors, sensors, heading_fusion))
    })();
    let (self_test_sensors, mut test_adc_provider, button_sensors, sensors, heading_fusion) = match sensor_setup {
        Ok(v) => v,
        Err(e) => return config_error_fallback_loop(&mut context, &e),
    };

    let input_sources = setup_input_sources(button_sensors);
    let ui_style = setup_ui_style();
    // Starts disabled: alerts (e.g. engine temp, oil pressure) must not fire against the
    // synthetic self-test sensor sweep. Enabled once the self-test sequence hands off to
    // the real sensor set (PageManager's UIEvent::SwitchSensorSet handler).
    let alert_manager = AlertManager::new(false, &ui_style);

    // Master warning LED — GPIO18, blinks at 2Hz while any alert is active (see
    // PageManager::event_loop). Pairs with the GPIO27 button chain in setup_sensors.
    let master_warning_led = match GpioOutput::new(18) {
        Ok(led) => Some(led),
        Err(e) => {
            log::warn!("Master warning LED GPIO18 unavailable: {}", e);
            None
        }
    };

    let mut mgr = PageManager::new(context, self_test_sensors, ui_style, input_sources, UpsMonitor::new(), adc_frame_for_diag, osc_frame, adc_version_frame, gnss_frame_for_diag, bno_frame_for_diag, gnss, bno085, alert_manager, heading_fusion, master_warning_led);

    mgr.setup().expect("Failed to setup page manager");

    // Start the synthetic sweep now, with the render loop about to begin, so the whole
    // 0 → max → 0 needle animation is on screen (setup above already wired the chains to
    // its frames). The swap timer below shares this origin.
    test_adc_provider.begin_sweep();

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
    // a delay, and RESTART_EXIT_CODE means it quit because it was rebuilt, its sensor
    // config was edited, or a restart was explicitly requested via SIGUSR1 — and should
    // be restarted immediately.
    let exit_code = match mgr.start() {
        Ok(()) if crate::util::shutdown::binary_updated() => {
            log::info!("Restarting to pick up newly built binary");
            std::process::ExitCode::from(crate::util::shutdown::RESTART_EXIT_CODE)
        }
        Ok(()) if crate::util::shutdown::config_updated() => {
            log::info!("Restarting to pick up updated sensor config");
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

// Shown instead of the dashboard when sensor_config.json fails to load. Renders the error
// and spins until watch_for_updates flags a config edit / binary rebuild / restart request
// (return RESTART_EXIT_CODE so the launcher relaunches immediately), or a shutdown signal
// arrives (return SUCCESS, same as a clean quit).
fn config_error_fallback_loop(context: &mut GraphicsContext, message: &str) -> std::process::ExitCode {
    let text = format!("Ошибка конфигурации: {}", message);
    log::error!("{}", text);

    let font = graphics::ui_style::DEFAULT_GLOBAL_FONT_PATH;
    let font_size: u32 = 24;
    let line_height = font_size as f32 * 1.6;
    let lines = wrap_text(&text, 80);

    loop {
        if crate::util::shutdown::shutdown_requested() {
            return std::process::ExitCode::SUCCESS;
        }
        if crate::util::shutdown::config_updated()
            || crate::util::shutdown::binary_updated()
            || crate::util::shutdown::restart_requested()
        {
            log::info!("Config error screen: update detected, restarting");
            return std::process::ExitCode::from(crate::util::shutdown::RESTART_EXIT_CODE);
        }

        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

        context.clear_screen();
        let block_top = context.height as f32 / 2.0 - (lines.len() as f32 * line_height) / 2.0;
        for (i, line) in lines.iter().enumerate() {
            let _ = context.render_text_with_font(
                line,
                10.0,
                block_top + i as f32 * line_height,
                1.0,
                (1.0, 1.0, 1.0),
                font,
                font_size,
            );
        }
        context.swap_buffers();

        thread::sleep(Duration::from_millis(100));
    }
}

// Greedy word wrap by character count (Unicode scalars, so Cyrillic wraps sanely). Words
// too long to fit a line on their own — an absolute config-file path being the usual case —
// are hard-broken into `max_chars`-wide pieces; the pieces after the first carry no leading
// space so the path isn't rendered with a gap in the middle.
fn wrap_text(s: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);

    // (piece, space_before): split on whitespace, then chop over-long tokens on char
    // boundaries so a multi-byte character is never cut.
    let mut tokens: Vec<(String, bool)> = Vec::new();
    for word in s.split_whitespace() {
        if word.chars().count() <= max_chars {
            tokens.push((word.to_string(), true));
        } else {
            let chars: Vec<char> = word.chars().collect();
            for (i, piece) in chars.chunks(max_chars).enumerate() {
                tokens.push((piece.iter().collect(), i == 0));
            }
        }
    }

    let mut lines = Vec::new();
    let mut current = String::new();
    for (token, space_before) in tokens {
        let sep = if !current.is_empty() && space_before { 1 } else { 0 };
        if !current.is_empty() && current.chars().count() + sep + token.chars().count() > max_chars {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() && space_before {
            current.push(' ');
        }
        current.push_str(&token);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::wrap_text;

    #[test]
    fn wraps_on_whitespace_and_keeps_words_intact() {
        assert_eq!(
            wrap_text("the quick brown fox jumps", 10),
            vec!["the quick", "brown fox", "jumps"]
        );
    }

    #[test]
    fn hard_breaks_a_long_unbroken_path_without_inserting_spaces() {
        let path = "/home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/niva_dashboard/sensor_config.json";
        let lines = wrap_text(path, 20);
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
        assert_eq!(lines.concat(), path);
    }

    #[test]
    fn hard_broken_piece_does_not_glue_to_preceding_word_with_a_space() {
        let lines = wrap_text("err: aaaaaaaaaaaaaaaaaaaaaaaa", 10);
        assert_eq!(lines, vec!["err:", "aaaaaaaaaa", "aaaaaaaaaa", "aaaa"]);
    }
}