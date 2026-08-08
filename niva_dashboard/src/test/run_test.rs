use std::thread;
use std::time::{Duration, Instant};

use crate::graphics::context::GraphicsContext;
use crate::graphics::opengl_test::{run_rotating_needle_gauge_test, run_indicator_zero_position_test, run_indicator_middle_position_test, run_indicator_max_position_test, run_fuel_level_grid_test, run_compass_test};
use crate::hardware::GpioInput;
use crate::hardware::sensor_value::SensorValue;
use crate::indicators::digital_segmented_indicator::DigitalSegmentedIndicator;
use crate::indicators::indicator::{Indicator, IndicatorBounds};
use crate::graphics::ui_style::UIStyle;
use crate::util::bno085_data_provider::Bno085DataProvider;
use crate::util::bno085_protocol::{
    SH2_REPORT_ACCELEROMETER, SH2_REPORT_GAME_ROTATION_VECTOR,
    SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR, SH2_REPORT_ROTATION_VECTOR,
};
use crate::util::gnss_data_provider::GnssDataProvider;
use crate::util::nmea::GnssFix;
use crate::util::shutdown;

extern crate gl;

pub fn run_test(name: &str) {
    match name {
        "needle" => {
            log::info!("\n=== Rotating Needle Gauge Test ===");
            run_graphics_test("Niva Dashboard - Needle Gauge Test", run_rotating_needle_gauge_test);
        }
        "gpio" => {
            log::info!("\n=== GPIO Input Test ===");
            match test_single_gpio_input() {
                Ok(()) => log::info!("GPIO test completed successfully!"),
                Err(e) => log::error!("GPIO test failed: {}", e),
            }
        }
        "digital" => {
            log::info!("\n=== Digital Segmented Display Test ===");
            run_graphics_test("Niva Dashboard - Digital Display Test", run_digital_display_test);
        }
        "ind_zero_pos" => {
            log::info!("\n=== Indicator Zero Position Test ===");
            run_graphics_test("Niva Dashboard - Zero Position Test", run_indicator_zero_position_test);
        }
        "ind_middle_pos" => {
            log::info!("\n=== Indicator Middle Position Test ===");
            run_graphics_test("Niva Dashboard - Middle Position Test", run_indicator_middle_position_test);
        }
        "ind_max_pos" => {
            log::info!("\n=== Indicator Maximum Position Test ===");
            run_graphics_test("Niva Dashboard - Maximum Position Test", run_indicator_max_position_test);
        }
        "fuel_grid" => {
            log::info!("\n=== Fuel Level Grid Stress Test ===");
            run_graphics_test("Niva Dashboard - Fuel Grid Stress Test", run_fuel_level_grid_test);
        }
        "compass" => {
            log::info!("\n=== Compass Indicator Test ===");
            run_graphics_test("Niva Dashboard - Compass Test", run_compass_test);
        }
        "bno085" => {
            log::info!("\n=== BNO085 IMU Test ===");
            run_bno085_test();
        }
        "heading" => {
            log::info!("\n=== GNSS/BNO085 Heading Accuracy Test ===");
            run_heading_test();
        }
        _ => {
            log::error!("Unknown test: {}", name);
            log::error!("Valid options: needle, gpio, digital, ind_zero_pos, ind_middle_pos, ind_max_pos, fuel_grid, compass, bno085, heading");
            log::error!("Note: SDL2-based tests (sdl2, advanced, etc.) are disabled after KMS/DRM migration");
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
                Ok(()) => log::info!("Graphics test completed successfully!"),
                Err(e) => log::error!("Graphics test failed: {}", e),
            }
        }
        Err(e) => log::error!("Failed to create graphics context: {}", e),
    }
}

fn test_single_gpio_input() -> Result<(), Box<dyn std::error::Error>> {
    // Create a GPIO input on pin 2 with default configuration (pull-up, active low)
    let gpio_input = GpioInput::new_with_pin(2)?;
    
    log::info!("Reading GPIO pin {} for 5 seconds...", gpio_input.pin_number());
    log::info!("Configuration: Active Low = {}", gpio_input.is_active_low());
    
    for i in 0..50 {
        let raw_state = gpio_input.read_raw();
        let logical_state = gpio_input.read_logical();
        
        log::info!("Sample {}: Raw = {}, Logical = {}", 
                i + 1, raw_state, if logical_state { "ACTIVE" } else { "INACTIVE" });
        
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}

/// Streams decoded BNO085 orientation (heading/pitch/roll) and instantaneous acceleration to
/// the console until interrupted (SIGINT/SIGTERM — install_signal_handlers() is already
/// active by the time run_test dispatches here, so Ctrl+C sets shutdown_requested() instead
/// of killing the process outright, letting the provider's Drop impl stop its thread cleanly).
fn run_bno085_test() {
    let mut provider = Bno085DataProvider::new(&[SH2_REPORT_ROTATION_VECTOR, SH2_REPORT_ACCELEROMETER]);
    if let Err(e) = provider.run() {
        log::error!("Failed to start BNO085 data provider: {}", e);
        return;
    }
    let frame = provider.frame();

    log::info!("Streaming BNO085 readings — press Ctrl+C to stop.");

    while !shutdown::shutdown_requested() {
        if frame.is_stale() {
            log::info!("BNO085: no data (not connected, or link stale)");
        } else {
            let o = frame.orientation();
            let a = frame.acceleration();
            log::info!(
                "orientation: heading={:6.1}\u{b0} (\u{b1}{:4.1}\u{b0})  pitch={:6.1}\u{b0}  roll={:6.1}\u{b0}  accuracy={:?}",
                o.heading_deg, o.heading_accuracy_deg, o.pitch_deg, o.roll_deg, o.accuracy
            );
            log::info!(
                "acceleration: x={:6.2}  y={:6.2}  z={:6.2} m/s\u{b2}  accuracy={:?}",
                a.x_mps2, a.y_mps2, a.z_mps2, a.accuracy
            );
        }
        thread::sleep(Duration::from_millis(200));
    }

    log::info!("BNO085 test stopped.");
}

/// How often the heading test polls both frames. Both GnssFrame and Bno085Frame just hold
/// whatever their own background thread last decoded — GNSS updates at #UNIHEADINGA's own
/// rate (much slower than this tick), BNO085 far faster — so polling faster than a source's
/// own update rate just re-reads the same cached value, which is harmless.
const HEADING_TEST_TICK: Duration = Duration::from_millis(200);

/// Delta (since the last logged snapshot) above which a change is logged on every tick, for as
/// long as it holds — dense sampling through a deliberate rotation.
const IMMEDIATE_THRESHOLD_DEG: f32 = 5.0;
/// Delta above which a change is logged at most every FAST_LOG_INTERVAL.
const FAST_THRESHOLD_DEG: f32 = 1.0;
const FAST_LOG_INTERVAL: Duration = Duration::from_secs(10);
/// Heartbeat interval when nothing is changing by even FAST_THRESHOLD_DEG.
const SLOW_LOG_INTERVAL: Duration = Duration::from_secs(30);

/// Smallest circular distance between two headings in [0, 360), result in [0, 180].
fn circular_delta(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// The four heading sources tracked by the test, as of one poll tick. `None` means "not
/// currently available" (stale link, no fix, or feature not yet reporting) rather than zero —
/// kept distinct from any numeric heading so a source appearing/disappearing is itself logged
/// as a change instead of being silently folded into "no change from 0°".
#[derive(Clone, Copy, Default)]
struct HeadingSnapshot {
    gnss_heading_deg: Option<f32>,
    rv_heading_deg: Option<f32>,
    game_heading_deg: Option<f32>,
    geo_heading_deg: Option<f32>,
}

impl HeadingSnapshot {
    /// Largest per-source delta against `prev`. A source flipping between `Some`/`None` counts
    /// as an unbounded change — that transition (acquiring or losing a fix/feature) is exactly
    /// what this test wants to catch immediately, not average away into the slow heartbeat.
    fn max_delta(&self, prev: &HeadingSnapshot) -> f32 {
        [
            Self::source_delta(prev.gnss_heading_deg, self.gnss_heading_deg),
            Self::source_delta(prev.rv_heading_deg, self.rv_heading_deg),
            Self::source_delta(prev.game_heading_deg, self.game_heading_deg),
            Self::source_delta(prev.geo_heading_deg, self.geo_heading_deg),
        ]
        .into_iter()
        .flatten()
        .fold(0.0, f32::max)
    }

    fn source_delta(prev: Option<f32>, cur: Option<f32>) -> Option<f32> {
        match (prev, cur) {
            (Some(p), Some(c)) => Some(circular_delta(p, c)),
            (None, None) => None,
            _ => Some(f32::INFINITY),
        }
    }
}

fn format_heading(value: Option<f32>) -> String {
    match value {
        Some(v) => format!("{:6.1}\u{b0}", v),
        None => "   --- ".to_string(),
    }
}

/// Streams timestamped heading + quality readings from GNSS (`#UNIHEADINGA`'s `heading_deg` —
/// the dual-antenna orientation solution, not `course_deg`) and all three BNO085 rotation
/// vector variants — full Rotation Vector (gyro+accel+mag), Game Rotation Vector (gyro+accel
/// only, isolates gyro heading tracking from magnetic distortion), and Geomagnetic Rotation
/// Vector (accel+mag only, isolates magnetometer heading tracking from gyro) — so each fusion
/// input's convergence, stationary stability, and relative-rotation accuracy can be assessed
/// independently rather than only ever seeing them blended together.
///
/// Logging cadence is adaptive: any source changing by more than IMMEDIATE_THRESHOLD_DEG since
/// the last logged snapshot logs on every tick for as long as that holds; above
/// FAST_THRESHOLD_DEG logs at most every FAST_LOG_INTERVAL; otherwise a SLOW_LOG_INTERVAL
/// heartbeat. Deltas are measured against the last *logged* snapshot, not the previous tick's
/// reading, so a steady sub-threshold drift still accumulates against a fixed anchor and
/// eventually crosses a threshold instead of perpetually resetting against a reading that just
/// moved along with it.
fn run_heading_test() {
    let mut gnss = GnssDataProvider::new("/dev/niva_gps", 115200);
    if let Err(e) = gnss.run() {
        log::error!("Failed to start GNSS data provider: {}", e);
        return;
    }
    let gnss_frame = gnss.frame();

    let mut bno = Bno085DataProvider::new(&[
        SH2_REPORT_ROTATION_VECTOR,
        SH2_REPORT_GAME_ROTATION_VECTOR,
        SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR,
    ]);
    if let Err(e) = bno.run() {
        log::error!("Failed to start BNO085 data provider: {}", e);
        return;
    }
    let bno_frame = bno.frame();

    log::info!("Streaming GNSS/BNO085 heading readings — press Ctrl+C to stop.");
    log::info!("GNSS=#UNIHEADINGA heading (std-dev, sat count, fix quality) | RV=full Rotation Vector | GAME=Game RV (gyro+accel only) | GEO=Geomagnetic RV (accel+mag only)");

    let start = Instant::now();
    let mut last_logged: Option<HeadingSnapshot> = None;
    let mut last_log_time = start;

    while !shutdown::shutdown_requested() {
        let fix: GnssFix = gnss_frame.fix();
        let gnss_available = !gnss_frame.is_stale() && fix.heading_deg.is_some();
        let bno_available = !bno_frame.is_stale();

        let rv = bno_frame.orientation();
        let game = bno_frame.game_orientation();
        let geo = bno_frame.geomagnetic_orientation();

        let current = HeadingSnapshot {
            gnss_heading_deg: if gnss_available { fix.heading_deg } else { None },
            rv_heading_deg: if bno_available { Some(rv.heading_deg) } else { None },
            game_heading_deg: if bno_available { Some(game.heading_deg) } else { None },
            geo_heading_deg: if bno_available { Some(geo.heading_deg) } else { None },
        };

        let should_log = match last_logged {
            None => true,
            Some(prev) => {
                let max_delta = current.max_delta(&prev);
                let elapsed = last_log_time.elapsed();
                max_delta > IMMEDIATE_THRESHOLD_DEG
                    || (max_delta > FAST_THRESHOLD_DEG && elapsed >= FAST_LOG_INTERVAL)
                    || elapsed >= SLOW_LOG_INTERVAL
            }
        };

        if should_log {
            log::info!(
                "[+{:8.1}s] GNSS={} (std={} sat={} fix={:?}) | RV={} ({:?}) | GAME={} | GEO={} ({:?})",
                start.elapsed().as_secs_f32(),
                format_heading(current.gnss_heading_deg),
                fix.heading_std_dev_deg.map(|v| format!("{:.2}\u{b0}", v)).unwrap_or_else(|| "--".to_string()),
                fix.heading_satellites.map(|v| v.to_string()).unwrap_or_else(|| "--".to_string()),
                fix.fix_quality,
                format_heading(current.rv_heading_deg),
                rv.accuracy,
                format_heading(current.game_heading_deg),
                format_heading(current.geo_heading_deg),
                geo.accuracy,
            );
            last_logged = Some(current);
            last_log_time = Instant::now();
        }

        thread::sleep(HEADING_TEST_TICK);
    }

    log::info!("Heading test stopped.");
}

/// Digital segmented display demonstration and test
fn run_digital_display_test(context: &mut GraphicsContext) -> Result<(), String> {
    let ui_style = UIStyle::new();
    
    log::info!("\n=== Testing Digital Display Rendering ===");
    
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
        log::error!("Error rendering time display: {}", e);
    } else {
        log::info!("✓ Time display rendered successfully");
    }
    
    // Speed display "088"
    if let Err(e) = render_speed_example(context, &ui_style, 88.0) {
        log::error!("Error rendering speed display: {}", e);
    } else {
        log::info!("✓ Speed display rendered successfully");
    }
    
    // RPM display "2500"
    if let Err(e) = render_rpm_example(context, &ui_style, 2500.0) {
        log::error!("Error rendering RPM display: {}", e);
    } else {
        log::info!("✓ RPM display rendered successfully");
    }
    
    // Temperature display "85.2"
    if let Err(e) = render_temperature_example(context, &ui_style, 85.2) {
        log::error!("Error rendering temperature display: {}", e);
    } else {
        log::info!("✓ Temperature display rendered successfully");
    }
    
    // Voltage display "V12.34"
    if let Err(e) = render_voltage_example(context, &ui_style, 12.34) {
        log::error!("Error rendering voltage display: {}", e);
    } else {
        log::info!("✓ Voltage display rendered successfully");
    }
    
    unsafe {
        // Clean up OpenGL state
        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindTexture(gl::TEXTURE_2D, 0);
        gl::UseProgram(0);
    }
    
    context.swap_buffers();
    
    log::info!("\n--- Display Test Results ---");
    log::info!("All digital displays have been rendered to the screen.");
    log::info!("Check the display for:");
    log::info!("- Time: 10:43");
    log::info!("- Speed: 0088 km/h");
    log::info!("- RPM: 2500");
    log::info!("- Temperature: 85.2°C");
    log::info!("- Voltage: V12.34");
    log::info!("- Amber LCD theme with inactive segments visible");
    
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
    // Create time indicator with inactive segments for realistic 7-segment look
    let time_indicator = DigitalSegmentedIndicator::integer(4)
        .with_inactive_segments(true);
    
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
        width: 208.0,
        height: 80.0,
    };

    time_indicator.render(&time_value, bounds, ui_style, context)
}

/// Example of rendering a digital speed display
fn render_speed_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    speed_kmh: f32
) -> Result<(), String> {
    // Use the speedometer preset with inactive segments
    let speed_indicator = DigitalSegmentedIndicator::integer(3)
        .with_inactive_segments(true);
    
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
        height: 80.0,
    };

    speed_indicator.render(&speed_value, bounds, ui_style, context)
}

/// Example of rendering a digital RPM display
fn render_rpm_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    rpm: f32
) -> Result<(), String> {
    // Use the tachometer preset with inactive segments
    let rpm_indicator = DigitalSegmentedIndicator::integer(4)
        .with_inactive_segments(true);
    
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
        width: 208.0,
        height: 80.0,
    };
    
    rpm_indicator.render(&rpm_value, bounds, ui_style, context)
}

/// Example of rendering a digital temperature display
fn render_temperature_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    temp_celsius: f32
) -> Result<(), String> {
    // Use the temperature preset with inactive segments
    let temp_indicator = DigitalSegmentedIndicator::float(4, 1)
        .with_inactive_segments(true);
    
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
        width: 164.0,
        height: 80.0,
    };

    temp_indicator.render(&temp_value, bounds, ui_style, context)
}

/// Example of rendering a digital voltage display
fn render_voltage_example(
    context: &mut GraphicsContext,
    ui_style: &UIStyle,
    voltage: f32
) -> Result<(), String> {
    // Use the voltage preset with inactive segments
    let voltage_indicator = DigitalSegmentedIndicator::float(5, 2)
        .with_inactive_segments(true);
    
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
        width: 208.0,
        height: 80.0,
    };

    voltage_indicator.render(&voltage_value, bounds, ui_style, context)
}