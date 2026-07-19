use crate::util::adc_serial_reader::{ADCSerialReader, SerialReader};

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
    reader: Option<ADCSerialReader>,
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
        match ADCSerialReader::try_new(port, baud) {
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
                        *frame.data.lock().unwrap() = values;
                        *frame.last_update.lock().unwrap() = Instant::now();
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
