use crate::util::serial_reader::{LineSerialReader, SerialReader};

use std::fmt;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long the background thread waits between attempts to (re)open the ADC serial
/// port after a failed or dropped connection.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

/// How long a frame can go without an update before the ADC link is considered down.
/// Shared by AdcLinkStatusProvider (drives the "ADC LINK" alert) and SensorManager
/// (suppresses "channel not in frame" read-error logging while the link is known down)
/// so the two stay in agreement about what counts as "down".
pub const ADC_LINK_MAX_AGE: Duration = Duration::from_millis(500);

/// USB hub location for the STM32 ADC module, as reported by `uhubctl` (see
/// PROJECT_CONTEXT.md "ADC module connectivity"). Hardware-specific — must be updated if
/// the module is rewired to a different hub.
///
/// The whole hub is power-cycled rather than just the module's own port: per-port power
/// switching on this hub (VIA Labs 2109:3431) is unreliable — the STM32 fails to
/// re-enumerate more often than not when only its port is cycled, even with correct sysfs
/// permissions. Cycling all ports on the hub together was confirmed reliable in testing and
/// is the only mechanism found to actually work. This also briefly drops power to whatever
/// else shares the hub (e.g. a wireless keyboard/mouse dongle used for dev/SSH access) —
/// harmless, since the dashboard's real input path is the GPIO-connected physical buttons,
/// not this hub.
const ADC_USB_HUB_LOCATION: &str = "1-1";

/// How long the ADC frame can go without a new sample, while a serial connection is open
/// and being read, before we conclude the STM32 itself is hung (not just the OS-level
/// serial link) and physically power-cycle its USB port. Deliberately much longer than
/// ADC_LINK_MAX_AGE (the UI-alert threshold) and RECONNECT_INTERVAL, so a routine
/// disconnect/reconnect never triggers a physical power cycle.
const HARD_RESET_STALE_THRESHOLD: Duration = Duration::from_secs(5);

/// Physical channel index within the STM32 ADC frame. After stripping the leading '$'
/// marker, the frame is a fixed sequence: A0-A3 (analog), TACHO/SPEED (raw inter-pulse
/// periods), D0-D9 (digital, STM32 pre-normalizes 1=active/0=inactive), then B0-B7
/// (physical MFD buttons) — see PROJECT_CONTEXT.md's "ADC module connectivity". D5 has no
/// sensor wired to it currently but the slot still exists in the frame, so it's kept here
/// rather than skipped, to avoid shifting every channel after it out of sync with the wire
/// layout.
///
/// Single source of truth for this layout: both the real wiring (main.rs's
/// add_adc_sensor_chains / setup_button_sensors) and the self-test synthetic frame
/// (TestADCDataProvider::generate_channels, below) index through this enum instead of
/// separately hand-maintained magic numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
#[allow(dead_code)] // D5: frame slot exists but no sensor is wired to it yet
pub enum AdcChannel {
    OilPressure = 0,
    FuelLevel = 1,
    EngineTemp = 2,
    Voltage12V = 3,
    Tacho = 4,
    Speed = 5,
    OilPressureLow = 6,
    FuelLow = 7,
    AlternatorCharging = 8,
    ExteriorLightsOn = 9,
    BrakeFluid = 10,
    HeadlightsOn = 11,
    TurnSignalOn = 12,
    HighBeamOn = 13,
    ParkBrakeOn = 14,
    CenterDiffLock = 15,
    B0 = 16,
    B1 = 17,
    B2 = 18,
    B3 = 19,
    B4 = 20,
    B5 = 21,
    B6 = 22,
    B7 = 23,
}

impl AdcChannel {
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Errors that can occur when starting the ADC data provider.
#[derive(Debug)]
pub enum AdcDataProviderError {
    AlreadyStarted,
    SpawnFailed(std::io::Error),
}

impl fmt::Display for AdcDataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "ADC data provider already started"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn thread: {}", err),
        }
    }
}

impl std::error::Error for AdcDataProviderError {}

/// A cloneable, thread-safe handle to the shared ADC frame.
/// Hardware providers hold this instead of the full ADCDataProvider so that
/// Arc<ADCFrame> does not drag in the non-Sync serial port fields.
#[derive(Clone)]
pub struct ADCFrame {
    data: Arc<Mutex<Vec<u16>>>,
    last_update: Arc<Mutex<Instant>>,
}

impl ADCFrame {
    fn new() -> Self {
        ADCFrame {
            data: Arc::new(Mutex::new(Vec::new())),
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn get_channel(&self, index: usize) -> Result<u16, String> {
        self.data.lock().unwrap()
            .get(index)
            .copied()
            .ok_or_else(|| format!("ADC channel {} not in frame", index))
    }

    /// Replaces the frame's channel data and marks it as freshly updated. Shared by
    /// ADCDataProvider's serial reader and TestADCDataProvider's synthetic writer — both
    /// populate an ADCFrame the same way, so ADCChannelProvider (and everything built on
    /// top of it) can't tell which one is currently the source.
    fn update(&self, values: Vec<u16>) {
        *self.data.lock().unwrap() = values;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    pub fn get_data(&self) -> Vec<u16> {
        self.data.lock().unwrap().clone()
    }

    /// Time elapsed since the last successfully parsed frame from the STM32 module.
    /// Used to detect a stalled/disconnected ADC link (see AdcLinkStatusProvider).
    pub fn last_update_age(&self) -> Duration {
        self.last_update.lock().unwrap().elapsed()
    }

    /// True if no frame has been received within ADC_LINK_MAX_AGE — i.e. the link is
    /// down, whether because the module was never connected or a live link dropped.
    pub fn is_stale(&self) -> bool {
        self.last_update_age() > ADC_LINK_MAX_AGE
    }
}

/// Owns the ADC serial connection's lifecycle within the background thread's read loop:
/// whether a reader currently exists, and whether the "port unavailable" warning has
/// already been logged for the current outage (so retries don't spam the log every
/// RECONNECT_INTERVAL). Purely local to that thread — never shared with ADCDataProvider
/// or the main thread, so no Arc/Mutex is needed here unlike ADCFrame/should_stop.
struct AdcConnection {
    reader: Option<LineSerialReader>,
    disconnect_logged: bool,
}

impl AdcConnection {
    fn new() -> Self {
        AdcConnection { reader: None, disconnect_logged: false }
    }

    /// (Re)opens the port if not already connected. Returns true once a live connection
    /// exists (whether it was already open or was just (re)established).
    fn ensure_connected(&mut self, port: &str, baud: u32) -> bool {
        if self.reader.is_some() {
            return true;
        }
        match LineSerialReader::try_new(port, baud) {
            Ok(opened) => {
                log::info!("ADC serial port '{}' (re)connected", port);
                self.disconnect_logged = false;
                self.reader = Some(opened);
                true
            }
            Err(_) => {
                if !self.disconnect_logged {
                    log::warn!(
                        "ADC serial port '{}' unavailable, retrying every {:?}",
                        port, RECONNECT_INTERVAL
                    );
                    self.disconnect_logged = true;
                }
                false
            }
        }
    }

    fn drop_connection(&mut self) {
        self.reader = None;
    }
}

/// Reads comma-separated ADC values from the serial port in a background thread,
/// keeping the latest frame available for reads by hardware providers via ADCFrame.
///
/// The background thread continuously overwrites the frame with each new parsed CSV line —
/// get_data and get_channel always return the most recent sample without consuming it.
///
/// The thread owns the full lifecycle of the serial connection, including the initial
/// open: it retries on RECONNECT_INTERVAL whenever there is no live connection, whether
/// because the port was never available or because a previously-live link dropped. This
/// means `run()` succeeds (and hardware providers get a usable ADCFrame) even if the STM32
/// module is not plugged in yet — AdcLinkStatusProvider's staleness check already treats
/// "never connected" and "not connected right now" identically, so no separate state is
/// needed here.
///
/// A dropped OS-level link (read error) is distinct from the STM32 firmware hanging while
/// the serial connection stays open — the latter never surfaces as a read error, just an
/// indefinitely stale frame. The thread also watches for this case and recovers it with a
/// physical USB power cycle (see HARD_RESET_STALE_THRESHOLD).
pub struct ADCDataProvider {
    port: String,
    baud: u32,
    should_stop: Arc<AtomicBool>,
    frame: ADCFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl ADCDataProvider {
    pub fn new(port: impl Into<String>, baud: u32) -> Self {
        ADCDataProvider {
            port: port.into(),
            baud,
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: ADCFrame::new(),
            thread: None,
        }
    }

    pub fn run(&mut self) -> Result<(), AdcDataProviderError> {
        if self.thread.is_some() {
            return Err(AdcDataProviderError::AlreadyStarted);
        }

        let port = self.port.clone();
        let baud = self.baud;
        let should_stop = Arc::clone(&self.should_stop);
        let frame = self.frame.clone();

        match std::thread::Builder::new()
            .name("adc-data-provider".into())
            .spawn(move || Self::run_loop(&port, baud, &should_stop, &frame)) {
            Ok(handle) => self.thread = Some(handle),
            Err(e) => return Err(AdcDataProviderError::SpawnFailed(e)),
        }

        Ok(())
    }

    /// Background thread body: (re)opens the serial port whenever there is no live
    /// connection, then reads frames until the link drops, looping back to reconnecting.
    /// Runs until `should_stop` is set.
    fn run_loop(port: &str, baud: u32, should_stop: &AtomicBool, frame: &ADCFrame) {
        let mut conn = AdcConnection::new();
        // A hub-wide power cycle is far more intrusive than a routine reconnect (it also
        // drops whatever else shares the hub), so it's attempted at most once per outage —
        // not retried on a timer. It only re-arms once real data proves the link is back.
        let mut reset_attempted = false;

        while !should_stop.load(Ordering::Relaxed) {
            if !conn.ensure_connected(port, baud) {
                Self::sleep_while_running(should_stop, RECONNECT_INTERVAL);
                continue;
            }

            match conn.reader.as_mut().unwrap().read_line() {
                Some(line) if !line.is_empty() => {
                    // Strip leading '$' frame marker before parsing channel values
                    let values: Vec<u16> = line
                        .trim_start_matches('$')
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                    if !values.is_empty() {
                        frame.update(values);
                        reset_attempted = false;
                    }
                }
                None => {
                    log::warn!("ADC serial link lost, attempting to reconnect");
                    conn.drop_connection();
                }
                _ => {
                    // Empty line (timeout) — keep polling, but watch for a connected-yet-dead
                    // link, which means the STM32 firmware itself is hung rather than the OS
                    // link being down (that case is already handled by the None arm above).
                    if frame.last_update_age() > HARD_RESET_STALE_THRESHOLD && !reset_attempted {
                        log::error!(
                            "ADC link unresponsive for over {:?}, power-cycling USB hub {}",
                            HARD_RESET_STALE_THRESHOLD, ADC_USB_HUB_LOCATION
                        );
                        conn.drop_connection();
                        match Self::power_cycle_adc_usb_port() {
                            Ok(()) => log::info!("ADC USB port power cycle succeeded"),
                            Err(e) => log::error!("ADC USB port power cycle failed: {}", e),
                        }
                        reset_attempted = true;
                    }
                }
            }
        }
    }

    /// Power-cycles every port on the STM32 module's USB hub via `uhubctl`, forcing a
    /// hardware power-on-reset. Requires root — a narrowly-scoped passwordless sudoers
    /// entry (see PROJECT_CONTEXT.md "ADC module connectivity") permits exactly this one
    /// command. Per-port cycling of just the module's own port was tested and found
    /// unreliable on this hub even with correct permissions (see ADC_USB_HUB_LOCATION);
    /// this exact invocation must match the sudoers entry verbatim or the sudo call fails.
    fn power_cycle_adc_usb_port() -> Result<(), String> {
        let status = std::process::Command::new("sudo")
            .args(["/usr/sbin/uhubctl", "-l", ADC_USB_HUB_LOCATION, "-a", "2"])
            .status()
            .map_err(|e| format!("failed to spawn uhubctl: {}", e))?;
        status.success().then_some(()).ok_or_else(|| format!("uhubctl exited with {}", status))
    }

    /// Sleeps for `duration`, checking `should_stop` in short increments so a stop
    /// request is picked up promptly instead of blocking for the full interval.
    fn sleep_while_running(should_stop: &AtomicBool, duration: Duration) {
        const POLL_INTERVAL: Duration = Duration::from_millis(100);
        let mut remaining = duration;
        while remaining > Duration::ZERO && !should_stop.load(Ordering::Relaxed) {
            let step = remaining.min(POLL_INTERVAL);
            thread::sleep(step);
            remaining -= step;
        }
    }

    pub fn stop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }

    /// Returns a cloneable handle to the shared frame for use by hardware providers.
    pub fn frame(&self) -> ADCFrame {
        self.frame.clone()
    }
}

impl Drop for ADCDataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// STM32 ADC channels are 12-bit.
const SELF_TEST_ADC_MAX_RAW: u16 = 4095;
/// Self-test sweep timing: rise fast, fall slower — long enough that debouncers (which need
/// several consistent samples) reliably latch both an active and an inactive state, short
/// enough not to delay real data past what's still a startup animation, not a wait.
const SELF_TEST_RISE: Duration = Duration::from_millis(500);
const SELF_TEST_FALL: Duration = Duration::from_millis(1500);
pub const SELF_TEST_DURATION: Duration = Duration::from_millis(2000);
/// How often the synthetic frame is refreshed. Comfortably faster than the 60Hz render loop
/// that drives SensorManager reads, so no single frame value is read as if it were stuck.
const SELF_TEST_TICK: Duration = Duration::from_millis(20);
/// Peak target speed for the HwSpeed channel's sweep — deliberately above the gauge's 180
/// km/h max (see hardware::sensors::SpeedSensor) so the sweep visibly reaches and clamps at
/// the top of the gauge rather than falling just short of it.
const SELF_TEST_SPEED_PEAK_KMH: f32 = 200.0;
/// Peak target rpm for the HwTacho channel's sweep -- comfortably below TachoSensor's 6000
/// rpm gauge max (unlike SELF_TEST_SPEED_PEAK_KMH, doesn't need to overshoot it: the point
/// here is just to exercise a realistic idle-to-redline sweep, not to test gauge clamping).
const SELF_TEST_TACHO_PEAK_RPM: f32 = 6000.0;

/// Populates an ADCFrame with synthetic values instead of reading the STM32 over serial —
/// mirrors ADCDataProvider's shape (owns an ADCFrame, updates it from a background thread) so
/// self-test can wire the exact same ADCChannelProvider / signal-processor / logical-sensor
/// chains as production (see main.rs's add_adc_sensor_chains), differing only in where the
/// raw channel bytes come from. Runs a fixed rise/fall sweep once and then stops updating;
/// the caller is expected to drop it (which stops the thread) once the real ADCDataProvider's
/// frame is ready to take over.
pub struct TestADCDataProvider {
    should_stop: Arc<AtomicBool>,
    frame: ADCFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestADCDataProvider {
    /// Starts generating synthetic frames immediately. The returned handle must be kept
    /// alive for the sweep to keep animating — dropping it stops the background thread.
    pub fn start() -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let frame = ADCFrame::new();
        let thread_should_stop = Arc::clone(&should_stop);
        let thread_frame = frame.clone();

        let thread = thread::Builder::new()
            .name("test-adc-data-provider".into())
            .spawn(move || Self::run_loop(&thread_should_stop, &thread_frame))
            .ok();

        TestADCDataProvider { should_stop, frame, thread }
    }

    /// Returns a cloneable handle to the shared frame, same as ADCDataProvider::frame().
    pub fn frame(&self) -> ADCFrame {
        self.frame.clone()
    }

    fn run_loop(should_stop: &AtomicBool, frame: &ADCFrame) {
        let start = Instant::now();
        while !should_stop.load(Ordering::Relaxed) {
            let elapsed = start.elapsed();
            if elapsed >= SELF_TEST_DURATION {
                break;
            }
            frame.update(Self::generate_channels(elapsed));
            thread::sleep(SELF_TEST_TICK);
        }
    }

    /// Triangular envelope: 0.0 -> 1.0 over the rise phase, back to 0.0 over the fall phase.
    fn envelope(elapsed: Duration) -> f32 {
        if elapsed < SELF_TEST_RISE {
            elapsed.as_secs_f32() / SELF_TEST_RISE.as_secs_f32()
        } else {
            let fall_elapsed = (elapsed - SELF_TEST_RISE).as_secs_f32();
            (1.0 - fall_elapsed / SELF_TEST_FALL.as_secs_f32()).max(0.0)
        }
    }

    /// Builds one synthetic STM32 frame (channels 0-15, matching the real layout used by
    /// main.rs's add_adc_sensor_chains: A0-A3, TACHO, SPEED, D0-D9). Button channels
    /// (16-23) are omitted — the button sensor manager always reads the real ADCFrame
    /// directly, never the self-test one.
    fn generate_channels(elapsed: Duration) -> Vec<u16> {
        let level = Self::envelope(elapsed);
        let analog_raw = (level * SELF_TEST_ADC_MAX_RAW as f32) as u16;
        // Digital channels latch active only past the envelope's midpoint, so debouncers see
        // a clean active period followed by a clean inactive one rather than chattering.
        let digital_raw: u16 = if level > 0.5 { 1 } else { 0 };
        // HwSpeed reports an inter-pulse period, not a count (see
        // SPEED_TACHO_PULSE_PERIOD_DESIGN.md) — encoded directly from the envelope's target
        // speed via SpeedSensor's own inverse conversion (speed_period_raw_from_kmh), so this
        // synthetic data and the real conversion it drives can't silently drift apart. Unlike
        // the count-based approach this replaced, no dithering trick is needed here: a period
        // already varies smoothly with speed, so the post-conversion reading tracks the
        // envelope directly.
        let speed_raw = crate::hardware::sensors::speed_period_raw_from_kmh(level * SELF_TEST_SPEED_PEAK_KMH);
        // HwTacho reports an inter-pulse period too (see hardware::sensors::TachoSensor),
        // same rationale as HwSpeed above -- driven through TachoSensor's own inverse
        // conversion so the synthetic data can't drift out of sync with the real one.
        let tacho_raw = crate::hardware::sensors::tacho_period_raw_from_rpm(level * SELF_TEST_TACHO_PEAK_RPM);

        let mut channels = vec![0u16; 16];
        channels[AdcChannel::OilPressure.index()] = analog_raw;  // HwOilPress
        channels[AdcChannel::FuelLevel.index()] = analog_raw;  // HwFuelLvl
        channels[AdcChannel::EngineTemp.index()] = analog_raw;  // HwEngineCoolantTemp
        channels[AdcChannel::Voltage12V.index()] = analog_raw;  // Hw12v
        channels[AdcChannel::Tacho.index()] = tacho_raw;   // HwTacho (raw inter-pulse period)
        channels[AdcChannel::Speed.index()] = speed_raw;   // HwSpeed (raw inter-pulse period)
        channels[AdcChannel::OilPressureLow.index()] = digital_raw; // HwOilPressLow
        channels[AdcChannel::FuelLow.index()] = digital_raw; // HwFuelLvlLow
        channels[AdcChannel::AlternatorCharging.index()] = digital_raw; // HwCharge
        channels[AdcChannel::ExteriorLightsOn.index()] = digital_raw; // HwExtLights / HwInstrIllum
        channels[AdcChannel::BrakeFluid.index()] = digital_raw; // HwBrakeFluidLvlLow
        // AdcChannel::D5 unused
        channels[AdcChannel::TurnSignalOn.index()] = digital_raw; // HwTurnSignal
        channels[AdcChannel::HighBeamOn.index()] = digital_raw; // HwHighBeam
        channels[AdcChannel::ParkBrakeOn.index()] = digital_raw; // HwParkBrake
        channels[AdcChannel::CenterDiffLock.index()] = digital_raw; // HwDiffLock
        channels
    }
}

impl Drop for TestADCDataProvider {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::hw_providers::{ADCChannelProvider, HWAnalogProvider, HWInput};
    use crate::hardware::analog_signal_processing::{AnalogSignalProcessor, AnalogSignalProcessorMovingAverage};
    use crate::hardware::sensors::{AnalogSensor, SpeedSensor, TachoSensor};

    /// Regression test for a bug where the self-test speed gauge never visibly moved.
    /// Drives the exact same pipeline main.rs's add_adc_sensor_chains wires up for HwSpeed
    /// (ADCChannelProvider::read_analog -> AnalogSignalProcessorMovingAverage -> SpeedSensor
    /// -- see SPEED_TACHO_PULSE_PERIOD_DESIGN.md) and asserts a nonzero km/h reading is
    /// observed before the sweep ends.
    #[test]
    fn self_test_speed_channel_produces_nonzero_reading_within_sweep() {
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        let adc_provider = ADCChannelProvider::new(HWInput::HwSpeed, frame);
        let mut moving_avg = AnalogSignalProcessorMovingAverage::new(5);
        let mut sensor = SpeedSensor::new();

        let start = Instant::now();
        let mut saw_nonzero_speed = false;
        while start.elapsed() < SELF_TEST_DURATION {
            if let Ok(raw) = adc_provider.read_analog(HWInput::HwSpeed) {
                let averaged = moving_avg.read(raw).unwrap();
                if sensor.read(averaged).unwrap().as_f32() > 0.0 {
                    saw_nonzero_speed = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_nonzero_speed, "self-test speed channel never produced a nonzero reading within SELF_TEST_DURATION");
    }

    /// End-to-end validation of the period-based redesign (see
    /// SPEED_TACHO_PULSE_PERIOD_DESIGN.md's Status section, step 1): generate_channels
    /// encodes HwSpeed via speed_period_raw_from_kmh(envelope * SELF_TEST_SPEED_PEAK_KMH),
    /// and SpeedSensor decodes it back via the inverse formula. This asserts the round trip
    /// actually recovers the intended speed while comfortably clear of the gauge's 0 floor
    /// and 180 clamp ceiling (both edges are dominated by rounding/staleness behavior that
    /// the other tests exercise directly, not by the conversion itself).
    ///
    /// Steps through generate_channels directly (fixed SELF_TEST_TICK increments) instead of
    /// racing TestADCDataProvider's background thread against wall-clock time: the latter
    /// made "target" (computed from the test's own Instant) and "raw" (last written by the
    /// thread up to one SELF_TEST_TICK ago) drift apart during the steepest part of the
    /// sweep -- at 200 km/h over a 500ms rise, one tick of lag alone is ~8 km/h, well past
    /// any reasonable tolerance. Calling generate_channels directly makes both sides of the
    /// comparison agree on the exact same `elapsed`, eliminating that timing race entirely.
    #[test]
    fn self_test_speed_channel_tracks_envelope_target_speed() {
        let mut sensor = SpeedSensor::new();

        let mut elapsed = Duration::ZERO;
        let mut checked_a_midrange_sample = false;
        while elapsed < SELF_TEST_DURATION {
            let channels = TestADCDataProvider::generate_channels(elapsed);
            let speed = sensor.read(channels[5]).unwrap().as_f32();
            let target = (TestADCDataProvider::envelope(elapsed) * SELF_TEST_SPEED_PEAK_KMH).min(180.0);

            if target > 20.0 && target < 170.0 {
                assert!((speed - target).abs() < 2.0,
                        "at elapsed={:?}, target={:.1} km/h but sensor read {:.1} km/h", elapsed, target, speed);
                checked_a_midrange_sample = true;
            }

            elapsed += SELF_TEST_TICK;
        }

        assert!(checked_a_midrange_sample, "sweep never passed through a mid-range speed to validate against");
    }

    /// Same regression as self_test_speed_channel_produces_nonzero_reading_within_sweep, for
    /// the HwTacho channel now that it's also period-based (see hardware::sensors::TachoSensor).
    #[test]
    fn self_test_tacho_channel_produces_nonzero_reading_within_sweep() {
        let provider = TestADCDataProvider::start();
        let frame = provider.frame();
        let adc_provider = ADCChannelProvider::new(HWInput::HwTacho, frame);
        let mut moving_avg = AnalogSignalProcessorMovingAverage::new(5);
        let mut sensor = TachoSensor::new();

        let start = Instant::now();
        let mut saw_nonzero_rpm = false;
        while start.elapsed() < SELF_TEST_DURATION {
            if let Ok(raw) = adc_provider.read_analog(HWInput::HwTacho) {
                let averaged = moving_avg.read(raw).unwrap();
                if sensor.read(averaged).unwrap().as_f32() > 0.0 {
                    saw_nonzero_rpm = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }

        assert!(saw_nonzero_rpm, "self-test tacho channel never produced a nonzero reading within SELF_TEST_DURATION");
    }

    /// Same round-trip validation as self_test_speed_channel_tracks_envelope_target_speed,
    /// for the HwTacho channel.
    #[test]
    fn self_test_tacho_channel_tracks_envelope_target_rpm() {
        let mut sensor = TachoSensor::new();

        let mut elapsed = Duration::ZERO;
        let mut checked_a_midrange_sample = false;
        while elapsed < SELF_TEST_DURATION {
            let channels = TestADCDataProvider::generate_channels(elapsed);
            let rpm = sensor.read(channels[4]).unwrap().as_f32();
            let target = (TestADCDataProvider::envelope(elapsed) * SELF_TEST_TACHO_PEAK_RPM).min(6000.0);

            if target > 500.0 && target < 5500.0 {
                assert!((rpm - target).abs() < 50.0,
                        "at elapsed={:?}, target={:.1} rpm but sensor read {:.1} rpm", elapsed, target, rpm);
                checked_a_midrange_sample = true;
            }

            elapsed += SELF_TEST_TICK;
        }

        assert!(checked_a_midrange_sample, "sweep never passed through a mid-range rpm to validate against");
    }
}
