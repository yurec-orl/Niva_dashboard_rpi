use crate::util::serial_reader::{LineSerialReader, SerialReader};

use std::collections::VecDeque;
use std::fmt;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// How long the background thread waits between attempts to (re)open the GNSS serial
/// port after a failed or dropped connection.
const RECONNECT_INTERVAL: Duration = Duration::from_secs(2);

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

/// A cloneable, thread-safe handle to the shared GNSS line buffer. Unlike ADCFrame (which
/// always exposes the latest decoded sample), this is a queue of raw NMEA lines — the
/// diagnostic terminal page is a log-style view, not a "current value" reader.
#[derive(Clone)]
pub struct GnssFrame {
    lines: Arc<Mutex<VecDeque<String>>>,
}

impl GnssFrame {
    fn new() -> Self {
        GnssFrame {
            lines: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Removes and returns every line buffered since the last drain, oldest first.
    pub fn drain_lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().drain(..).collect()
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
/// buffering raw lines for display on the GNSS diagnostic terminal page via GnssFrame.
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
