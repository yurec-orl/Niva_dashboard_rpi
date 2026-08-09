//! Reads the BNO085 IMU in a background thread via the hand-rolled SHTP/SH-2 client in
//! bno085_protocol.rs, exposing decoded heading/pitch/roll (from Rotation Vector) and
//! instantaneous acceleration via Bno085Frame. Mirrors GnssDataProvider's shape
//! (gnss_data_provider.rs): a cloneable frame updated from one owned background thread, OS-
//! level reconnect on I2C errors (no hardware reset line to fall back on, same as GNSS).

use crate::util::bno085_protocol::{
    Accuracy, Bno085, Bno085Error, Bno085Event, Bno085Report, GameRotationVectorReport,
    RotationVectorReport, BNO085_ADDR, HINT_PIN, I2C_BUS,
};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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
}

impl Bno085Frame {
    fn new() -> Self {
        Bno085Frame {
            orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            game_orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            geomagnetic_orientation: Arc::new(Mutex::new(Bno085Orientation::default())),
            acceleration: Arc::new(Mutex::new(Bno085Acceleration::default())),
            last_update: Arc::new(Mutex::new(Instant::now() - READING_MAX_AGE)),
        }
    }

    /// Latest decoded Rotation Vector orientation (gyro+accel+magnetometer fusion, absolute
    /// magnetic heading). Zeroed/`accuracy: None` until the first report arrives — check
    /// `is_stale()` before trusting it.
    pub fn orientation(&self) -> Bno085Orientation {
        *self.orientation.lock().unwrap()
    }

    /// Latest decoded Game Rotation Vector orientation (gyro+accel fusion only, no
    /// magnetometer). `heading_accuracy_deg` is always 0.0 and `accuracy` always `None` — this
    /// report has no absolute reference to estimate accuracy against. Zeroed until the first
    /// report arrives — check `is_stale()` before trusting it.
    pub fn game_orientation(&self) -> Bno085Orientation {
        *self.game_orientation.lock().unwrap()
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
        Bno085DataProvider {
            features: features.to_vec(),
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: Bno085Frame::new(),
            thread: None,
        }
    }

    pub fn run(&mut self) -> Result<(), Bno085ProviderError> {
        if self.thread.is_some() {
            return Err(Bno085ProviderError::AlreadyStarted);
        }

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
}

impl Drop for Bno085DataProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
