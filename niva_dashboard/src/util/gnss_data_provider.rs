use crate::util::nmea::{self, FixQuality, GnssFix};
use crate::util::serial_reader::{LineSerialReader, SerialReader};

use std::collections::VecDeque;
use std::fmt;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long the background thread waits between attempts to (re)open the GNSS serial
/// port after a failed or dropped connection.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

/// How long the GNSS link can go without a line (any line, not necessarily a recognized
/// sentence type) before it's considered down. Feeds GnssLinkStatusProvider (HwGnssLink).
/// Looser than ADC_LINK_MAX_AGE (500ms) since NMEA output rate is typically 1Hz, far slower
/// than the STM32's continuous CSV stream — this just needs to tolerate a couple of missed
/// update cycles before flagging a real problem.
pub const GNSS_LINK_MAX_AGE: Duration = Duration::from_secs(3);

/// Bound on buffered-but-not-yet-drained NMEA lines. A receiver at typical update rates
/// emits well under this many sentences between two polls of the diagnostic terminal page
/// (the only consumer); this just caps memory if that page is never opened.
const MAX_BUFFERED_LINES: usize = 500;

/// Errors that can occur when starting the GNSS data provider.
#[derive(Debug)]
pub enum GnssDataProviderError {
    AlreadyStarted,
    SpawnFailed(std::io::Error),
}

impl fmt::Display for GnssDataProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "GNSS data provider already started"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn thread: {}", err),
        }
    }
}

impl std::error::Error for GnssDataProviderError {}

/// A cloneable, thread-safe handle to the shared GNSS state. Combines two views onto the
/// same stream: a queue of raw NMEA lines (for the log-style diagnostic terminal page,
/// unlike ADCFrame which only ever exposes the latest decoded sample), and a `GnssFix`
/// accumulated from whichever sentences update it (for indicators/consumers that want
/// structured time/position/speed/heading rather than raw text).
#[derive(Clone)]
pub struct GnssFrame {
    lines: Arc<Mutex<VecDeque<String>>>,
    fix: Arc<Mutex<GnssFix>>,
    last_update: Arc<Mutex<Instant>>,
}

impl GnssFrame {
    fn new() -> Self {
        GnssFrame {
            lines: Arc::new(Mutex::new(VecDeque::new())),
            fix: Arc::new(Mutex::new(GnssFix::default())),
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Removes and returns every line buffered since the last drain, oldest first.
    pub fn drain_lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().drain(..).collect()
    }

    /// Snapshot of the latest accumulated fix (time/date/position/speed/heading/...).
    /// Individual fields are `None` until a sentence carrying them has been seen.
    pub fn fix(&self) -> GnssFix {
        *self.fix.lock().unwrap()
    }

    /// Replaces the frame's fix wholesale and marks it as freshly updated. Used by
    /// TestGnssDataProvider's synthetic writer, which produces a complete fix per tick rather
    /// than incrementally merging fields the way `nmea::update_from_sentence` does for real
    /// NMEA lines.
    fn set_fix(&self, fix: GnssFix) {
        *self.fix.lock().unwrap() = fix;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// True if no line has been received within GNSS_LINK_MAX_AGE — the OS/serial-level
    /// link is down, whether because the receiver was never connected or a live link
    /// dropped. Distinct from fix quality: this is about hearing from the receiver at all,
    /// not whether it currently has a position lock.
    pub fn is_stale(&self) -> bool {
        self.last_update.lock().unwrap().elapsed() > GNSS_LINK_MAX_AGE
    }
}

#[cfg(test)]
impl GnssFrame {
    /// Builds a frame with no background thread attached, for tests that need to inject
    /// exact fixes at exact moments (e.g. heading_fusion_sensor's state machine tests) rather
    /// than race a synthetic writer thread like TestGnssDataProvider's.
    pub(crate) fn for_test() -> Self {
        Self::new()
    }

    pub(crate) fn set_fix_for_test(&self, fix: GnssFix) {
        self.set_fix(fix);
    }
}

/// Owns the GNSS serial connection's lifecycle within the background thread's read loop,
/// mirroring AdcConnection (see adc_data_provider.rs) minus the STM32-specific hard-reset
/// path — a wedged GNSS receiver has no known equivalent recovery, so only OS-level
/// reconnect is handled here.
struct GnssConnection {
    reader: Option<LineSerialReader>,
    disconnect_logged: bool,
}

impl GnssConnection {
    fn new() -> Self {
        GnssConnection { reader: None, disconnect_logged: false }
    }

    fn ensure_connected(&mut self, port: &str, baud: u32) -> bool {
        if self.reader.is_some() {
            return true;
        }
        match LineSerialReader::try_new(port, baud) {
            Ok(opened) => {
                log::info!("GNSS serial port '{}' (re)connected", port);
                self.disconnect_logged = false;
                self.reader = Some(opened);
                true
            }
            Err(_) => {
                if !self.disconnect_logged {
                    log::warn!(
                        "GNSS serial port '{}' unavailable, retrying every {:?}",
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

/// Reads NMEA sentences from the GNSS receiver's serial port in a background thread,
/// buffering raw lines for the GNSS diagnostic terminal page and parsing them into a
/// structured GnssFix, both exposed via GnssFrame.
pub struct GnssDataProvider {
    port: String,
    baud: u32,
    should_stop: Arc<AtomicBool>,
    frame: GnssFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl GnssDataProvider {
    pub fn new(port: impl Into<String>, baud: u32) -> Self {
        GnssDataProvider {
            port: port.into(),
            baud,
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: GnssFrame::new(),
            thread: None,
        }
    }

    pub fn run(&mut self) -> Result<(), GnssDataProviderError> {
        if self.thread.is_some() {
            return Err(GnssDataProviderError::AlreadyStarted);
        }

        let port = self.port.clone();
        let baud = self.baud;
        let should_stop = Arc::clone(&self.should_stop);
        let frame = self.frame.clone();

        match std::thread::Builder::new()
            .name("gnss-data-provider".into())
            .spawn(move || Self::run_loop(&port, baud, &should_stop, &frame)) {
            Ok(handle) => self.thread = Some(handle),
            Err(e) => return Err(GnssDataProviderError::SpawnFailed(e)),
        }

        Ok(())
    }

    fn run_loop(port: &str, baud: u32, should_stop: &AtomicBool, frame: &GnssFrame) {
        let mut conn = GnssConnection::new();

        while !should_stop.load(Ordering::Relaxed) {
            if !conn.ensure_connected(port, baud) {
                Self::sleep_while_running(should_stop, RECONNECT_INTERVAL);
                continue;
            }

            match conn.reader.as_mut().unwrap().read_line() {
                Some(line) if !line.is_empty() => {
                    *frame.last_update.lock().unwrap() = Instant::now();
                    nmea::update_from_sentence(&mut frame.fix.lock().unwrap(), &line);

                    let mut lines = frame.lines.lock().unwrap();
                    if lines.len() >= MAX_BUFFERED_LINES {
                        lines.pop_front();
                    }
                    lines.push_back(line);
                }
                None => {
                    log::warn!("GNSS serial link lost, attempting to reconnect");
                    conn.drop_connection();
                }
                // Empty line (read timeout) — routine, just keep polling.
                _ => {}
            }
        }
    }

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

    /// Returns a cloneable handle to the shared line buffer for use by the terminal page.
    pub fn frame(&self) -> GnssFrame {
        self.frame.clone()
    }
}

impl Drop for GnssDataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Sweep period for the synthetic heading rotation — arbitrary but slow enough to watch the
/// compass tape scroll and confirm the 359°->0° wraparound, fast enough not to make a manual
/// test tedious.
const TEST_HEADING_ROTATION_PERIOD: Duration = Duration::from_secs(12);
/// How often the synthetic frame is refreshed — comfortably faster than the 60Hz render loop
/// that drives SensorManager reads, same rationale as TestADCDataProvider::SELF_TEST_TICK.
const TEST_TICK: Duration = Duration::from_millis(50);

/// Populates a GnssFrame with synthetic fix data instead of reading a real receiver over
/// serial — mirrors TestADCDataProvider's shape (see adc_data_provider.rs): owns a GnssFrame,
/// updates it from a background thread, so a test mode can exercise the same GnssFrame-
/// consuming code (GnssChannelProvider, CompassIndicator, GnssPage) as production, differing
/// only in where the fix comes from. Unlike TestADCDataProvider's single rise/fall sweep
/// (built for the real startup self-test animation), this runs indefinitely — it's only ever
/// used from a dedicated test mode rather than the app's normal startup path, so there's no
/// "hand off to the real provider" moment to sweep towards.
pub struct TestGnssDataProvider {
    should_stop: Arc<AtomicBool>,
    frame: GnssFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestGnssDataProvider {
    /// Starts generating synthetic fixes immediately. The returned handle must be kept alive
    /// for the sweep to keep animating — dropping it stops the background thread.
    pub fn start() -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let frame = GnssFrame::new();
        let thread_should_stop = Arc::clone(&should_stop);
        let thread_frame = frame.clone();

        let thread = thread::Builder::new()
            .name("test-gnss-data-provider".into())
            .spawn(move || Self::run_loop(&thread_should_stop, &thread_frame))
            .ok();

        TestGnssDataProvider { should_stop, frame, thread }
    }

    /// Returns a cloneable handle to the shared frame, same as GnssDataProvider::frame().
    pub fn frame(&self) -> GnssFrame {
        self.frame.clone()
    }

    fn run_loop(should_stop: &AtomicBool, frame: &GnssFrame) {
        let start = Instant::now();
        while !should_stop.load(Ordering::Relaxed) {
            let elapsed = start.elapsed().as_secs_f32();
            frame.set_fix(Self::generate_fix(elapsed));
            thread::sleep(TEST_TICK);
        }
    }

    /// Synthetic fix: heading sweeps continuously through the full 0-360° range (so a
    /// compass/heading-tape test can confirm both smooth rotation and the wraparound), speed
    /// oscillates gently, and the rest of the fields hold plausible fixed values so pages
    /// reading the full GnssFix (e.g. GnssPage) have something sensible to show too.
    fn generate_fix(elapsed: f32) -> GnssFix {
        let heading = (elapsed / TEST_HEADING_ROTATION_PERIOD.as_secs_f32() * 360.0).rem_euclid(360.0);
        GnssFix {
            time: None,
            date: None,
            latitude_deg: Some(55.751244),
            longitude_deg: Some(37.618423),
            altitude_m: Some(150.0),
            speed_kmh: Some(40.0 + 20.0 * (elapsed * 0.3).sin()),
            course_deg: Some(heading),
            heading_deg: Some(heading),
            pitch_deg: Some(0.0),
            heading_std_dev_deg: Some(10.5),
            pitch_std_dev_deg: Some(0.3),
            heading_satellites: Some(9),
            fix_quality: Some(FixQuality::Gps),
            satellites: Some(9),
            hdop: Some(1.1),
            status_active: Some(true),
        }
    }
}

impl Drop for TestGnssDataProvider {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
