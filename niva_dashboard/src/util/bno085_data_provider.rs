//! Reads the BNO085 IMU in a background thread via the hand-rolled SHTP/SH-2 client in
//! bno085_protocol.rs, exposing decoded heading/pitch/roll (from Rotation Vector) and
//! instantaneous acceleration via Bno085Frame. Mirrors GnssDataProvider's shape
//! (gnss_data_provider.rs): a cloneable frame updated from one owned background thread, OS-
//! level reconnect on I2C errors (no hardware reset line to fall back on, same as GNSS).

use crate::util::bno085_protocol::{
    Accuracy, Bno085, Bno085Error, Bno085Event, Bno085Report, GameRotationVectorReport,
    RotationVectorReport, BNO085_ADDR, HINT_PIN, I2C_BUS,
};
use crate::util::config::Config;
use crate::util::link_status::LinkStatus;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Config section key the Game RV pitch/roll calibration (КАЛИБР) is persisted under.
const CALIBRATION_CONFIG_KEY: &str = "bno085_pitch_roll_calibration";

/// Correction added to the raw Game Rotation Vector pitch/roll so they read zero right after
/// КАЛИБР is pressed. Persisted via Config so it survives a restart instead of resetting to 0
/// -- previously lived page-locally in HorzPage and was lost on every restart.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
struct PitchRollCalibration {
    pitch_correction_deg: f32,
    roll_correction_deg: f32,
}

/// Report interval requested from the sensor for both enabled features. 50 Hz comfortably
/// exceeds what a dashboard display needs while staying well clear of the report rates where
/// the BNO085's I2C interface was observed to get less reliable (research doc, "I2C clock
/// speed" / "Error handling & recovery strategy").
const REPORT_INTERVAL_US: u32 = 20_000;

/// How long each HINT-gated poll blocks before giving up and looping back to re-check
/// should_stop/staleness. A few report periods at REPORT_INTERVAL_US — comfortably above
/// normal jitter so a healthy sensor rarely hits this timeout, but short enough that it isn't
/// the bottleneck on how quickly READING_MAX_AGE actually gets noticed (is_stale() is only
/// re-checked once per poll() return, so this timeout is the detection loop's real
/// granularity, independent of the READING_MAX_AGE threshold itself).
const HINT_POLL_TIMEOUT: Duration = Duration::from_millis(50);

/// How long the background thread waits between attempts to (re)open/reinit the sensor after
/// a failed or dropped I2C connection. Short — while the cable is still being finalized,
/// glitches from flexing/moving the wire are expected to be brief and self-clearing, so a
/// fast retry recovers almost as soon as the connection is good again, rather than sitting out
/// a long fixed backoff meant for "this device probably isn't coming back soon" (that
/// judgment call belongs on a stable cable, not this one). No known hard-reset recovery path
/// for this sensor (unlike the ADC module's USB hub power cycle), so OS-level reconnect is all
/// there is.
const RECONNECT_INTERVAL: Duration = Duration::from_millis(200);

/// A reading is treated as unavailable once it's older than this. ~7 report periods at
/// REPORT_INTERVAL_US — enough slack to absorb a couple of dropped/delayed reports without
/// falsely flagging routine jitter, while still catching a genuinely wedged/silently-reset
/// sensor (research doc, "Silent feature staleness": a reset can stop new reports without any
/// error ever surfacing) far faster than an arbitrary round-number timeout would.
const READING_MAX_AGE: Duration = Duration::from_millis(250);

/// Decoded orientation from the latest Rotation Vector report. `heading_deg` is magnetic
/// heading (not true heading — see research doc, "Design note" on reconciling with GNSS
/// UNIHEADING), wrapped to [0, 360). `pitch_deg`/`roll_deg` follow the raw Euler decomposition
/// range; which is physically "pitch" vs "roll" depends on board mounting, not corrected here.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bno085Orientation {
    pub heading_deg: f32,
    pub pitch_deg: f32,
    pub roll_deg: f32,
    pub heading_accuracy_deg: f32,
    pub accuracy: Option<Accuracy>,
}

/// Decoded instantaneous acceleration (including gravity) from the latest Accelerometer
/// report, in m/s^2, in the chip's own axis convention.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bno085Acceleration {
    pub x_mps2: f32,
    pub y_mps2: f32,
    pub z_mps2: f32,
    pub accuracy: Option<Accuracy>,
}

/// A cloneable, thread-safe handle to the BNO085's latest decoded orientation and
/// acceleration. Cheap to clone (Arc clone), same pattern as GnssFrame/UpsRawFrame.
#[derive(Clone)]
pub struct Bno085Frame {
    orientation: Arc<Mutex<Bno085Orientation>>,
    game_orientation: Arc<Mutex<Bno085Orientation>>,
    geomagnetic_orientation: Arc<Mutex<Bno085Orientation>>,
    acceleration: Arc<Mutex<Bno085Acceleration>>,
    last_update: Arc<Mutex<Instant>>,
    /// Separate from `last_update`: that shared timestamp is bumped by *any* of the four
    /// report types, so `is_stale()` alone can't tell "frame's alive" apart from "alive, but
    /// Game RV specifically hasn't reported yet" -- if a Rotation Vector or Accelerometer
    /// report happens to arrive first, `is_stale()` goes false while `game_orientation` is
    /// still sitting at `Default::default()` (heading_deg: 0.0). Consumers that need Game RV
    /// specifically (heading_fusion_sensor) must check `game_orientation_is_stale()` instead --
    /// observed in practice as a persisted-heading anchor locking onto a phantom 0.0 deg
    /// "reading" moments before the real first Game RV report landed.
    game_orientation_last_update: Arc<Mutex<Instant>>,
    /// (pitch, roll) correction added to the raw Game RV reading by `game_orientation()` --
    /// see `PitchRollCalibration`'s doc comment. Defaults to zero; set at construction via
    /// `with_calibration()` and updated by `calibrate_pitch_roll()`, which reports the new
    /// value to `calibration_persist` (if any) rather than touching Config itself.
    pitch_roll_correction_deg: Arc<Mutex<(f32, f32)>>,
    /// Invoked by `calibrate_pitch_roll()` with the freshly computed calibration so it can be
    /// persisted -- set via `with_calibration_persist()`. `None` for frames that don't own a
    /// persisted calibration (e.g. `for_test()`), in which case a calibration press just isn't
    /// saved.
    calibration_persist: Option<Arc<dyn Fn(PitchRollCalibration) + Send + Sync>>,
    /// Whether this frame is currently fed by a synthetic test source rather than the real
    /// sensor -- mirrors GnssFrame::test_mode (see its doc comment for why this is a shared,
    /// independently-settable flag rather than fixed at construction). No synthetic BNO085
    /// provider exists yet, so nothing sets this true today; status() and the flag are here so
    /// one can be added later (mirroring TestGnssDataProvider) without an API change.
    test_mode: Arc<AtomicBool>,
}

impl Bno085Frame {
    fn new() -> Self {
        Bno085Frame {
            orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            game_orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            geomagnetic_orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            acceleration: Arc::new(Mutex::new(Bno085Acceleration::default())),
            last_update: Arc::new(Mutex::new(Instant::now() - READING_MAX_AGE)),
            game_orientation_last_update: Arc::new(Mutex::new(Instant::now() - READING_MAX_AGE)),
            pitch_roll_correction_deg: Arc::new(Mutex::new((0.0, 0.0))),
            calibration_persist: None,
            test_mode: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Sets the pitch/roll correction applied by `game_orientation()`, without touching Config
    /// -- callers own loading the initial value.
    fn with_calibration(self, calibration: PitchRollCalibration) -> Self {
        *self.pitch_roll_correction_deg.lock().unwrap() = (calibration.pitch_correction_deg, calibration.roll_correction_deg);
        self
    }

    /// Registers a callback invoked by `calibrate_pitch_roll()` with the new calibration --
    /// callers own persisting it (or not).
    fn with_calibration_persist<F>(mut self, persist: F) -> Self
    where
        F: Fn(PitchRollCalibration) + Send + Sync + 'static,
    {
        self.calibration_persist = Some(Arc::new(persist));
        self
    }

    /// Latest decoded Rotation Vector orientation (gyro+accel+magnetometer fusion, absolute
    /// magnetic heading). Zeroed/`accuracy: None` until the first report arrives — check
    /// `is_stale()` before trusting it.
    pub fn orientation(&self) -> Bno085Orientation {
        *self.orientation.lock().unwrap()
    }

    /// Latest decoded Game Rotation Vector orientation (gyro+accel fusion only, no
    /// magnetometer), with the persisted КАЛИБР pitch/roll correction applied (heading is never
    /// corrected -- calibration only zeroes out mounting-induced pitch/roll error).
    /// `heading_accuracy_deg` is always 0.0 and `accuracy` always `None` — this report has no
    /// absolute reference to estimate accuracy against. Zeroed until the first report arrives —
    /// check `is_stale()` before trusting it.
    pub fn game_orientation(&self) -> Bno085Orientation {
        let mut o = *self.game_orientation.lock().unwrap();
        let (pitch_correction_deg, roll_correction_deg) = *self.pitch_roll_correction_deg.lock().unwrap();
        o.pitch_deg += pitch_correction_deg;
        o.roll_deg += roll_correction_deg;
        o
    }

    /// Handles a КАЛИБР press: sets the pitch/roll correction to minus the current raw
    /// reading, so `game_orientation()`'s pitch/roll read zero immediately, then hands it to
    /// `calibration_persist` (if set) -- survives a restart, unlike the correction previously
    /// living page-locally in HorzPage. No-op while stale -- no raw reading to zero out.
    pub fn calibrate_pitch_roll(&self) {
        if self.game_orientation_is_stale() {
            return;
        }
        let raw = *self.game_orientation.lock().unwrap();
        let calibration = PitchRollCalibration { pitch_correction_deg: -raw.pitch_deg, roll_correction_deg: -raw.roll_deg };
        *self.pitch_roll_correction_deg.lock().unwrap() = (calibration.pitch_correction_deg, calibration.roll_correction_deg);
        if let Some(persist) = &self.calibration_persist {
            persist(calibration);
        }
    }

    /// Latest decoded Geomagnetic Rotation Vector orientation (accel+magnetometer fusion only,
    /// no gyro). Zeroed/`accuracy: None` until the first report arrives — check `is_stale()`
    /// before trusting it.
    pub fn geomagnetic_orientation(&self) -> Bno085Orientation {
        *self.geomagnetic_orientation.lock().unwrap()
    }

    /// Latest decoded acceleration. Zeroed/`accuracy: None` until the first Accelerometer
    /// report arrives — check `is_stale()` before trusting it.
    pub fn acceleration(&self) -> Bno085Acceleration {
        *self.acceleration.lock().unwrap()
    }

    /// True if neither orientation nor acceleration has been updated within READING_MAX_AGE —
    /// covers both "never connected" and "connection silently died" the same way
    /// AdcFrame/GnssFrame's is_stale() does.
    pub fn is_stale(&self) -> bool {
        self.last_update.lock().unwrap().elapsed() > READING_MAX_AGE
    }

    /// Marks this frame as fed by a synthetic test source (or not) -- called by whichever
    /// provider owns writing to it, same pattern as GnssFrame::set_test_mode.
    pub(crate) fn set_test_mode(&self, active: bool) {
        self.test_mode.store(active, Ordering::Relaxed);
    }

    /// Clears orientation/acceleration back to their zeroed defaults and backdates every
    /// freshness timestamp so is_stale()/game_orientation_is_stale() read as stale immediately
    /// -- mirrors GnssFrame::reset() (see its doc comment for why this matters). Deliberately
    /// leaves `pitch_roll_correction_deg` untouched: that's the persisted КАЛИБР calibration,
    /// not a reading, and must survive a provider pause/resume cycle.
    fn reset(&self) {
        *self.orientation.lock().unwrap() = Bno085Orientation::default();
        *self.game_orientation.lock().unwrap() = Bno085Orientation::default();
        *self.geomagnetic_orientation.lock().unwrap() = Bno085Orientation::default();
        *self.acceleration.lock().unwrap() = Bno085Acceleration::default();
        let stale_instant = Instant::now() - READING_MAX_AGE;
        *self.last_update.lock().unwrap() = stale_instant;
        *self.game_orientation_last_update.lock().unwrap() = stale_instant;
    }

    /// Directly sets orientation/acceleration fields and freshness timestamps, bypassing the
    /// SH-2 report decoding the `set_*` methods below do for real hardware reports -- used by
    /// TestBno085DataProvider to write synthetic data into an existing (possibly long-lived,
    /// already widely cloned) frame the same way it writes real reports.
    pub(crate) fn set_synthetic_orientation(&self, o: Bno085Orientation) {
        *self.orientation.lock().unwrap() = o;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    pub(crate) fn set_synthetic_game_orientation(&self, o: Bno085Orientation) {
        *self.game_orientation.lock().unwrap() = o;
        *self.last_update.lock().unwrap() = Instant::now();
        *self.game_orientation_last_update.lock().unwrap() = Instant::now();
    }

    pub(crate) fn set_synthetic_geomagnetic_orientation(&self, o: Bno085Orientation) {
        *self.geomagnetic_orientation.lock().unwrap() = o;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    pub(crate) fn set_synthetic_acceleration(&self, a: Bno085Acceleration) {
        *self.acceleration.lock().unwrap() = a;
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// Link status for display: `Test` when fed by a synthetic provider (regardless of
    /// staleness), `NoData` when no report has arrived recently, `Ok` otherwise. Prefer this
    /// over `is_stale()` for anything shown to the user.
    pub fn status(&self) -> LinkStatus {
        if self.test_mode.load(Ordering::Relaxed) {
            LinkStatus::Test
        } else if self.is_stale() {
            LinkStatus::NoData
        } else {
            LinkStatus::Ok
        }
    }

    /// True if Game Rotation Vector specifically hasn't reported within READING_MAX_AGE --
    /// see `game_orientation_last_update`'s doc comment for why this must be checked instead
    /// of (not in addition to) `is_stale()` before trusting `game_orientation()`.
    pub fn game_orientation_is_stale(&self) -> bool {
        self.game_orientation_last_update.lock().unwrap().elapsed() > READING_MAX_AGE
    }

    /// Directly injects a Game RV heading for tests that need exact values at exact moments
    /// (e.g. heading_fusion_sensor's state machine tests), bypassing the quaternion decoding
    /// set_game_orientation does for real reports -- constructing a GameRotationVectorReport
    /// for an arbitrary target heading would just re-derive this same value the long way.
    #[cfg(test)]
    pub(crate) fn set_game_heading_for_test(&self, heading_deg: f32) {
        *self.game_orientation.lock().unwrap() = Bno085Orientation {
            heading_deg: heading_deg.rem_euclid(360.0),
            pitch_deg: 0.0,
            roll_deg: 0.0,
            heading_accuracy_deg: 0.0,
            accuracy: None,
        };
        *self.last_update.lock().unwrap() = Instant::now();
        *self.game_orientation_last_update.lock().unwrap() = Instant::now();
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::new()
    }

    fn set_orientation(&self, report: RotationVectorReport) {
        Self::store_rotation_vector(&self.orientation, report);
        *self.last_update.lock().unwrap() = Instant::now();
    }

    fn set_geomagnetic_orientation(&self, report: RotationVectorReport) {
        Self::store_rotation_vector(&self.geomagnetic_orientation, report);
        *self.last_update.lock().unwrap() = Instant::now();
    }

    fn store_rotation_vector(slot: &Mutex<Bno085Orientation>, report: RotationVectorReport) {
        let (yaw, pitch, roll) = report.euler_rad();
        *slot.lock().unwrap() = Bno085Orientation {
            heading_deg: yaw.to_degrees().rem_euclid(360.0),
            pitch_deg: pitch.to_degrees(),
            roll_deg: roll.to_degrees(),
            heading_accuracy_deg: report.heading_accuracy_rad.to_degrees(),
            accuracy: Some(report.accuracy),
        };
    }

    /// Game Rotation Vector has no absolute reference, so unlike the other two orientation
    /// sources it carries no accuracy estimate at all (heading_accuracy_deg: 0.0, accuracy:
    /// None) — not "unreliable", just not measured.
    fn set_game_orientation(&self, report: GameRotationVectorReport) {
        let (yaw, pitch, roll) = report.euler_rad();
        *self.game_orientation.lock().unwrap() = Bno085Orientation {
            heading_deg: yaw.to_degrees().rem_euclid(360.0),
            pitch_deg: pitch.to_degrees(),
            roll_deg: roll.to_degrees(),
            heading_accuracy_deg: 0.0,
            accuracy: None,
        };
        *self.last_update.lock().unwrap() = Instant::now();
        *self.game_orientation_last_update.lock().unwrap() = Instant::now();
    }

    fn set_acceleration(&self, report: crate::util::bno085_protocol::AccelerometerReport) {
        *self.acceleration.lock().unwrap() = Bno085Acceleration {
            x_mps2: report.x_mps2,
            y_mps2: report.y_mps2,
            z_mps2: report.z_mps2,
            accuracy: Some(report.accuracy),
        };
        *self.last_update.lock().unwrap() = Instant::now();
    }

    /// Marks the frame as freshly connected without touching the last decoded values —
    /// called right after a successful (re)connect so the stale last-update timestamp left
    /// over from before connecting (or from a prior dropped connection) doesn't immediately
    /// re-trip is_stale() before the first report has had a chance to arrive.
    fn mark_connected(&self) {
        *self.last_update.lock().unwrap() = Instant::now();
    }
}

/// Errors that can occur when starting the BNO085 data provider.
#[derive(Debug)]
pub enum Bno085ProviderError {
    AlreadyStarted,
    SpawnFailed(std::io::Error),
}

impl std::fmt::Display for Bno085ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "BNO085 data provider already started"),
            Self::SpawnFailed(err) => write!(f, "Failed to spawn thread: {}", err),
        }
    }
}

impl std::error::Error for Bno085ProviderError {}

/// Owns the BNO085's I2C connection lifecycle in a background thread: connect, run the SH-2
/// init handshake, enable Rotation Vector + Accelerometer, then poll and decode reports into
/// a shared Bno085Frame. Reconnects (and re-enables features) on I2C errors, on an explicit
/// EXE-channel reset notification, and as a staleness-timeout backstop in case that
/// notification itself is lost to bus noise — see the research doc's "Silent feature
/// staleness" lesson, which is exactly the failure mode the backstop guards against.
pub struct Bno085DataProvider {
    features: Vec<u8>,
    should_stop: Arc<AtomicBool>,
    frame: Bno085Frame,
    thread: Option<thread::JoinHandle<()>>,
}

impl Bno085DataProvider {
    /// `features` is the set of SH-2 report IDs to enable (and re-enable after a reset) —
    /// callers only get data for whichever of Bno085Frame's fields correspond to what's
    /// listed here. Different test modes want different subsets (e.g. the heading test has no
    /// use for Accelerometer and skips it to keep I2C/report traffic down).
    pub fn new(features: &[u8]) -> Self {
        let frame = Bno085Frame::new()
            .with_calibration(Self::load_calibration())
            .with_calibration_persist(|calibration| {
                let Ok(value) = serde_json::to_value(calibration) else { return };
                Config::load().set_section(CALIBRATION_CONFIG_KEY, value);
            });
        Bno085DataProvider {
            features: features.to_vec(),
            should_stop: Arc::new(AtomicBool::new(false)),
            frame,
            thread: None,
        }
    }

    fn load_calibration() -> PitchRollCalibration {
        let value = Config::load().section(CALIBRATION_CONFIG_KEY);
        serde_json::from_value(value).unwrap_or_default()
    }

    pub fn run(&mut self) -> Result<(), Bno085ProviderError> {
        if self.thread.is_some() {
            return Err(Bno085ProviderError::AlreadyStarted);
        }

        // No-op on a genuinely fresh frame (already default/stale), but required on `resume()`
        // after a `pause()` -- see Bno085Frame::reset()'s doc comment.
        self.frame.reset();

        let features = self.features.clone();
        let should_stop = Arc::clone(&self.should_stop);
        let frame = self.frame.clone();
        let handle = thread::Builder::new()
            .name("bno085-data-provider".into())
            .spawn(move || Self::run_loop(&features, &should_stop, &frame))
            .map_err(Bno085ProviderError::SpawnFailed)?;
        self.thread = Some(handle);
        Ok(())
    }

    /// Returns a cloneable handle to the shared frame for use by hardware providers.
    pub fn frame(&self) -> Bno085Frame {
        self.frame.clone()
    }

    fn run_loop(features: &[u8], should_stop: &AtomicBool, frame: &Bno085Frame) {
        let mut sensor: Option<Bno085> = None;

        while !should_stop.load(Ordering::Relaxed) {
            if sensor.is_none() {
                match Self::connect_and_init(features) {
                    Ok(s) => {
                        log::info!("BNO085 connected, features {:?} enabled", features);
                        frame.mark_connected();
                        sensor = Some(s);
                    }
                    Err(e) => {
                        log::warn!("BNO085 unavailable ({}), retrying in {:?}", e, RECONNECT_INTERVAL);
                        Self::sleep_while_running(should_stop, RECONNECT_INTERVAL);
                        continue;
                    }
                }
            }

            let s = sensor.as_mut().unwrap();
            match s.poll(HINT_POLL_TIMEOUT) {
                Ok(Bno085Event::Report(Bno085Report::RotationVector(r))) => frame.set_orientation(r),
                Ok(Bno085Event::Report(Bno085Report::GameRotationVector(r))) => frame.set_game_orientation(r),
                Ok(Bno085Event::Report(Bno085Report::GeomagneticRotationVector(r))) => frame.set_geomagnetic_orientation(r),
                Ok(Bno085Event::Report(Bno085Report::Accelerometer(a))) => frame.set_acceleration(a),
                Ok(Bno085Event::None) => {}
                Ok(Bno085Event::ResetComplete) => {
                    log::warn!("BNO085 reported reset complete, re-enabling features");
                    if let Err(e) = Self::enable_features(s, features) {
                        log::warn!("BNO085 failed to re-enable features after reset: {}", e);
                        sensor = None;
                    }
                }
                Err(e) => {
                    log::warn!("BNO085 I2C error ({}), reconnecting", e);
                    sensor = None;
                }
            }

            if sensor.is_some() && frame.is_stale() {
                log::warn!("BNO085 data stale despite no reported error, forcing reconnect");
                sensor = None;
            }
        }
    }

    fn connect_and_init(features: &[u8]) -> Result<Bno085, Bno085Error> {
        let mut sensor = Bno085::open(I2C_BUS, BNO085_ADDR, HINT_PIN)?;
        sensor.init()?;
        Self::enable_features(&mut sensor, features)?;
        Ok(sensor)
    }

    fn enable_features(sensor: &mut Bno085, features: &[u8]) -> Result<(), Bno085Error> {
        for &feature in features {
            sensor.enable_feature(feature, REPORT_INTERVAL_US)?;
        }
        Ok(())
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

    /// Stops the background thread and blocks until it exits, then resets internal state so a
    /// later `run()` (see `resume()`) can restart it -- mirrors GnssDataProvider::pause(), used
    /// to hand the BNO085 frame off to a synthetic test writer without discarding the frame.
    pub fn pause(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.should_stop.store(false, Ordering::SeqCst);
    }

    /// Restarts the background thread after `pause()`. Just `run()` under another name.
    pub fn resume(&mut self) -> Result<(), Bno085ProviderError> {
        self.run()
    }
}

impl Drop for Bno085DataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Sweep period for the synthetic heading rotation, matching TestGnssDataProvider's so the
/// GNSS page's compass and the BNO085 INS readings rotate in visual sync during test mode.
const TEST_HEADING_ROTATION_PERIOD: Duration = Duration::from_secs(12);
/// How often the synthetic frame is refreshed -- same rationale as TestGnssDataProvider::TEST_TICK.
const TEST_TICK: Duration = Duration::from_millis(50);
/// Amplitude of the gentle synthetic pitch/roll oscillation, degrees.
const TEST_PITCH_ROLL_AMPLITUDE_DEG: f32 = 8.0;
/// Standard gravity, m/s^2 -- synthetic acceleration's Z axis, matching a level, stationary
/// mounting (chip's native unit, same as real Accelerometer reports).
const TEST_STANDARD_GRAVITY_MPS2: f32 = 9.80665;

/// Populates a Bno085Frame with synthetic orientation/acceleration data instead of reading the
/// real I2C sensor -- mirrors TestGnssDataProvider's shape (see gnss_data_provider.rs): writes
/// into an existing, already-shared frame rather than a disposable one of its own, so every
/// consumer (GnssPage's INS block, heading_fusion_sensor, HorzPage) sees test data the same way
/// it would see real hardware. Heading tracks TestGnssDataProvider's sweep so the fused heading
/// output stays self-consistent (both sources agree) instead of the fusion logic fighting two
/// independently-moving "sensors".
pub struct TestBno085DataProvider {
    should_stop: Arc<AtomicBool>,
    frame: Bno085Frame,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestBno085DataProvider {
    /// Starts generating synthetic reports immediately into an existing Bno085Frame. The
    /// returned handle must be kept alive for the sweep to keep animating -- dropping it stops
    /// the background thread and clears the frame's test_mode flag.
    pub fn start_on(frame: Bno085Frame) -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        frame.set_test_mode(true);
        let thread_should_stop = Arc::clone(&should_stop);
        let thread_frame = frame.clone();

        let thread = thread::Builder::new()
            .name("test-bno085-data-provider".into())
            .spawn(move || Self::run_loop(&thread_should_stop, &thread_frame))
            .ok();

        TestBno085DataProvider { should_stop, frame, thread }
    }

    fn run_loop(should_stop: &AtomicBool, frame: &Bno085Frame) {
        let start = Instant::now();
        while !should_stop.load(Ordering::Relaxed) {
            let elapsed = start.elapsed().as_secs_f32();
            let o = Self::generate_orientation(elapsed);
            frame.set_synthetic_orientation(o);
            frame.set_synthetic_game_orientation(o);
            frame.set_synthetic_geomagnetic_orientation(o);
            frame.set_synthetic_acceleration(Self::generate_acceleration(elapsed));
            thread::sleep(TEST_TICK);
        }
    }

    /// Heading sweeps through the full 0-360° range in sync with TestGnssDataProvider's own
    /// sweep; pitch/roll oscillate gently so HorzPage has something to visibly animate too.
    fn generate_orientation(elapsed: f32) -> Bno085Orientation {
        let heading_deg = (elapsed / TEST_HEADING_ROTATION_PERIOD.as_secs_f32() * 360.0).rem_euclid(360.0);
        Bno085Orientation {
            heading_deg,
            pitch_deg: TEST_PITCH_ROLL_AMPLITUDE_DEG * (elapsed * 0.2).sin(),
            roll_deg: TEST_PITCH_ROLL_AMPLITUDE_DEG * (elapsed * 0.15).cos(),
            heading_accuracy_deg: 2.0,
            accuracy: Some(Accuracy::High),
        }
    }

    /// Roughly 1g on Z (level, stationary orientation) with a small wobble on X/Y so the
    /// InfoBlocks::InsData block doesn't show a perfectly static reading.
    fn generate_acceleration(elapsed: f32) -> Bno085Acceleration {
        Bno085Acceleration {
            x_mps2: 0.3 * (elapsed * 0.4).sin(),
            y_mps2: 0.3 * (elapsed * 0.37).cos(),
            z_mps2: TEST_STANDARD_GRAVITY_MPS2,
            accuracy: Some(Accuracy::High),
        }
    }
}

impl Drop for TestBno085DataProvider {
    fn drop(&mut self) {
        self.should_stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Mirrors TestGnssDataProvider's Drop -- required so status() reverts once the real
        // provider resumes writing to this (shared) frame.
        self.frame.set_test_mode(false);
    }
}
