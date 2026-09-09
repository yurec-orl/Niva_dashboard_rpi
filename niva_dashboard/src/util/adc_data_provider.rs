use crate::util::serial_reader::{LineSerialReader, SerialReader};

use std::collections::HashMap;
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

/// A DS18B20 address is considered stale after this long without a fresh `$T` reading for
/// it (see ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md and AdcTempFrame below). The `$T` cadence is
/// ~1 Hz, so this is ~5 missed cycles — deliberately far longer than ADC_LINK_MAX_AGE (the
/// telemetry-frame threshold) so a single dropped CRC never blanks the temperature display.
pub const TEMP_ADDR_MAX_AGE: Duration = Duration::from_secs(5);

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

/// How often the background thread re-sends `$VER` while the STM32 hasn't answered with its
/// firmware commit hash. One request per connection would suffice if replies were
/// guaranteed; retrying covers a dropped first reply without meaningfully adding to serial
/// traffic. Firmware predating the `$VER` command never replies at all — the diagnostics
/// page just shows "н/д" in that case.
const VERSION_REQUEST_INTERVAL: Duration = Duration::from_secs(10);

/// Wire parameters for the oscilloscope burst-capture protocol (see OSCILLOSCOPE_DESIGN.md
/// and stm32_adc_module's main.cpp OSC_* defines) — kept in sync with the firmware by hand,
/// since the two sides don't share a header.
pub const OSC_BUF_LEN: usize = 4096;
const OSC_CHUNK_SAMPLES: usize = 64;
const OSC_EXPECTED_CHUNKS: usize = OSC_BUF_LEN / OSC_CHUNK_SAMPLES;
/// Sample rate of the burst capture (50 kSPS, see OSCILLOSCOPE_DESIGN.md) — used by callers
/// to convert the buffer's sample index into elapsed time.
pub const OSC_SAMPLE_RATE_HZ: f64 = 50_000.0;
/// Bounds the whole request/response round trip: firmware's own DMA capture is bounded at
/// 150ms, plus time to ASCII-encode and transmit ~21KB back over the USB-CDC link. Generous
/// relative to both so a hung/missing STM32 is reported promptly rather than hanging forever.
const OSC_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

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

    /// Refreshes the "last updated" timestamp without altering channel data. Used while an
    /// oscilloscope capture has the STM32's normal telemetry paused, so the routine pause
    /// doesn't get mistaken by AdcLinkStatusProvider for a dropped link.
    fn touch(&self) {
        *self.last_update.lock().unwrap() = Instant::now();
    }
}

/// Cloneable, thread-safe holder for the STM32 firmware's commit hash, as returned in the
/// `$VER,<hash>` reply to a `$VER\n` request (see stm32_adc_module). The background thread
/// requests it once per serial connection and writes the answer here; the diagnostics page
/// reads it. `None` until the first reply arrives — or indefinitely, against firmware that
/// predates the command.
#[derive(Clone)]
pub struct AdcVersionFrame {
    hash: Arc<Mutex<Option<String>>>,
}

impl AdcVersionFrame {
    fn new() -> Self {
        AdcVersionFrame { hash: Arc::new(Mutex::new(None)) }
    }

    /// The reported firmware commit hash, or `None` if the STM32 hasn't answered a `$VER`
    /// request this session.
    pub fn get(&self) -> Option<String> {
        self.hash.lock().unwrap().clone()
    }

    fn set(&self, hash: String) {
        *self.hash.lock().unwrap() = Some(hash);
    }

    fn is_known(&self) -> bool {
        self.hash.lock().unwrap().is_some()
    }
}

/// One DS18B20 scratchpad reading: the raw temperature register in 1/16 °C (its native
/// signed format — negative values are real, e.g. -88 → -5.5 °C) plus when it last arrived.
#[derive(Clone, Copy)]
struct TempReading {
    raw_16ths: i16,
    updated: Instant,
}

/// Shared, thread-safe store for the STM32's `$T` one-wire temperature line (see
/// ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md and ONEWIRE_TEMP_SENSOR_DESIGN.md's wire protocol).
///
/// Kept separate from ADCFrame rather than merged into it: `$T` data is keyed by 64-bit ROM
/// address (a 16-char hex string) not by positional channel index, its values are signed,
/// and it needs a per-address timestamp for staleness that ADCFrame's single `last_update`
/// can't express. Same reasoning that keeps GnssFrame / Bno085Frame as their own handles.
///
/// The background thread (ADCDataProvider::run_loop, or TestADCDataProvider's synthetic
/// writer) owns the writer side; the sensor layer only reads, via OneWireTempChannelProvider.
#[derive(Clone)]
pub struct AdcTempFrame {
    readings: Arc<Mutex<HashMap<String, TempReading>>>,
    /// Bumped on every `$T` line, including the bare `$T\n` "bus alive, nothing found"
    /// keepalive — lets a consumer tell "one-wire bus reporting" from "no `$T` at all".
    last_line: Arc<Mutex<Instant>>,
}

impl AdcTempFrame {
    pub fn new() -> Self {
        AdcTempFrame {
            readings: Arc::new(Mutex::new(HashMap::new())),
            last_line: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Raw 1/16 °C register for `rom` if a reading has arrived within TEMP_ADDR_MAX_AGE,
    /// else None (never seen, or gone stale). Callers treat None as "sensor unavailable
    /// this cycle" — see OneWireTempChannelProvider.
    pub fn fresh_raw(&self, rom: &str) -> Option<i16> {
        self.readings.lock().unwrap().get(rom).and_then(|r| {
            (r.updated.elapsed() <= TEMP_ADDR_MAX_AGE).then_some(r.raw_16ths)
        })
    }

    /// Time since any `$T` line last arrived. Distinguishes "one-wire bus alive" from
    /// "no `$T` line at all" (firmware predates the feature / link down). Not yet consumed
    /// — for the ТЕМП page's future "bus down" vs. "all sensors stale" distinction.
    #[allow(dead_code)]
    pub fn bus_last_line_age(&self) -> Duration {
        self.last_line.lock().unwrap().elapsed()
    }

    /// Every ROM address seen this session, sorted. Not yet consumed — for the deferred
    /// commissioning view (reading opaque addresses off the running dashboard to build the
    /// address → logical-sensor map, see ONEWIRE_TEMP_SENSOR_DESIGN.md's open decision).
    #[allow(dead_code)]
    pub fn addresses(&self) -> Vec<String> {
        let mut addrs: Vec<String> = self.readings.lock().unwrap().keys().cloned().collect();
        addrs.sort();
        addrs
    }

    fn update_reading(&self, rom: &str, raw_16ths: i16) {
        self.readings.lock().unwrap().insert(
            rom.to_string(),
            TempReading { raw_16ths, updated: Instant::now() },
        );
    }

    fn touch_line(&self) {
        *self.last_line.lock().unwrap() = Instant::now();
    }
}

impl Default for AdcTempFrame {
    fn default() -> Self {
        Self::new()
    }
}

/// Outcome of the most recent (or in-progress) oscilloscope burst capture, shared between
/// ADCDataProvider's background thread (writer) and OscPage (reader) via OscFrame.
#[derive(Clone)]
pub enum OscCaptureState {
    /// No capture has been requested yet.
    Idle,
    /// A capture is currently in flight (request sent, waiting on chunks/$OSCEND).
    Capturing,
    /// Last capture completed successfully; carries the reassembled sample buffer.
    Done(Vec<u16>),
    /// Last capture failed (link lost, timeout, missing/malformed chunks).
    Failed(String),
}

/// A cloneable, thread-safe handle for requesting an oscilloscope burst capture and reading
/// back its result. Mirrors ADCFrame's shape (background thread owns the writer side, UI
/// thread only ever reads/requests) but models a one-shot request/response instead of a
/// continuously-refreshed frame.
#[derive(Clone)]
pub struct OscFrame {
    request_pending: Arc<AtomicBool>,
    state: Arc<Mutex<OscCaptureState>>,
}

impl OscFrame {
    fn new() -> Self {
        OscFrame {
            request_pending: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(OscCaptureState::Idle)),
        }
    }

    /// Requests a new capture. A no-op (from the caller's perspective) if a capture is
    /// already pending or in flight — the background thread only ever services one at a time.
    pub fn request_capture(&self) {
        self.request_pending.store(true, Ordering::SeqCst);
    }

    /// Current capture state, cloned out for the caller to inspect. `Vec<u16>` clones here
    /// are at most OSC_BUF_LEN elements (a few KB) — cheap enough to call every frame.
    pub fn state(&self) -> OscCaptureState {
        self.state.lock().unwrap().clone()
    }

    /// Consumes a pending request, if any. Called only from the background thread's read loop.
    fn take_request(&self) -> bool {
        self.request_pending.swap(false, Ordering::SeqCst)
    }

    fn set_state(&self, state: OscCaptureState) {
        *self.state.lock().unwrap() = state;
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
    temp_frame: AdcTempFrame,
    osc_frame: OscFrame,
    version_frame: AdcVersionFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl ADCDataProvider {
    pub fn new(port: impl Into<String>, baud: u32) -> Self {
        ADCDataProvider {
            port: port.into(),
            baud,
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: ADCFrame::new(),
            temp_frame: AdcTempFrame::new(),
            osc_frame: OscFrame::new(),
            version_frame: AdcVersionFrame::new(),
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
        let temp_frame = self.temp_frame.clone();
        let osc_frame = self.osc_frame.clone();
        let version_frame = self.version_frame.clone();

        match std::thread::Builder::new()
            .name("adc-data-provider".into())
            .spawn(move || Self::run_loop(&port, baud, &should_stop, &frame, &temp_frame, &osc_frame, &version_frame)) {
            Ok(handle) => self.thread = Some(handle),
            Err(e) => return Err(AdcDataProviderError::SpawnFailed(e)),
        }

        Ok(())
    }

    /// Background thread body: (re)opens the serial port whenever there is no live
    /// connection, then reads frames until the link drops, looping back to reconnecting.
    /// Runs until `should_stop` is set.
    fn run_loop(port: &str, baud: u32, should_stop: &AtomicBool, frame: &ADCFrame, temp_frame: &AdcTempFrame, osc_frame: &OscFrame, version_frame: &AdcVersionFrame) {
        let mut conn = AdcConnection::new();
        // A hub-wide power cycle is far more intrusive than a routine reconnect (it also
        // drops whatever else shares the hub), so it's attempted at most once per outage —
        // not retried on a timer. It only re-arms once real data proves the link is back.
        let mut reset_attempted = false;
        // Last time a `$VER` request went out; None re-arms an immediate request (on start
        // and after every reconnect). Stops once version_frame has an answer.
        let mut version_last_request: Option<Instant> = None;

        while !should_stop.load(Ordering::Relaxed) {
            if !conn.ensure_connected(port, baud) {
                Self::sleep_while_running(should_stop, RECONNECT_INTERVAL);
                continue;
            }

            if !version_frame.is_known()
                && version_last_request.map_or(true, |t| t.elapsed() >= VERSION_REQUEST_INTERVAL)
            {
                if let Some(reader) = conn.reader.as_mut() {
                    let _ = reader.write_line("$VER\n");
                }
                version_last_request = Some(Instant::now());
            }

            // Oscilloscope capture requests take priority over normal telemetry reads: the
            // STM32 pauses its own 50Hz tick for the duration, so there is nothing else
            // useful to read from the port until the capture finishes anyway.
            if osc_frame.take_request() {
                osc_frame.set_state(OscCaptureState::Capturing);
                let reader = conn.reader.as_mut().unwrap();
                match Self::perform_osc_capture(reader, frame) {
                    Ok(samples) => osc_frame.set_state(OscCaptureState::Done(samples)),
                    Err(e) => {
                        log::warn!("Oscilloscope capture failed: {}", e);
                        osc_frame.set_state(OscCaptureState::Failed(e));
                    }
                }
                continue;
            }

            match conn.reader.as_mut().unwrap().read_line() {
                Some(line) if !line.is_empty() => {
                    if let Some(rest) = line.strip_prefix("$T").filter(|r| r.is_empty() || r.starts_with(',')) {
                        // One-wire temperature line (see ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md):
                        // asynchronous, ~1 Hz, interleaved with the 50 Hz `$…` frames.
                        // Deliberately does NOT touch `frame`'s timestamp — a `$T` line says
                        // nothing about telemetry-frame liveness, and HARD_RESET_STALE_THRESHOLD
                        // must still fire if `$…` frames stop while only `$T` keeps arriving.
                        Self::parse_temp_line(rest, temp_frame);
                    } else if let Some(hash) = line.strip_prefix("$VER,") {
                        // Reply to the `$VER` request above. Intercepted here before the
                        // channel parse, which would otherwise pick the hash out as a
                        // bogus single-channel frame.
                        let hash = hash.trim();
                        if !hash.is_empty() && !version_frame.is_known() {
                            log::info!("STM32 ADC firmware version: {}", hash);
                            version_frame.set(hash.to_string());
                        }
                    } else {
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
                }
                None => {
                    log::warn!("ADC serial link lost, attempting to reconnect");
                    conn.drop_connection();
                    // Re-arm so the next connection re-requests the firmware version if we
                    // never got an answer on this one.
                    version_last_request = None;
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

    /// Parses the body of a `$T` line (everything after the `$T` marker — either empty for
    /// the bare `$T\n` keepalive, or `,<rom>:<raw>;<rom>:<raw>;...`) into `temp_frame`.
    /// A malformed pair (bad ROM length/charset, non-integer value) is skipped without
    /// dropping the well-formed pairs alongside it — the one-wire harness is noisy by
    /// design and the STM32 already omits any sensor that fails its CRC that cycle.
    fn parse_temp_line(rest: &str, temp_frame: &AdcTempFrame) {
        temp_frame.touch_line();
        let Some(pairs) = rest.strip_prefix(',') else { return }; // bare "$T" keepalive
        for pair in pairs.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            let Some((rom, raw)) = pair.split_once(':') else { continue };
            let rom = rom.trim();
            if rom.len() != 16 || !rom.bytes().all(|b| b.is_ascii_hexdigit()) {
                continue;
            }
            let Ok(raw_16ths) = raw.trim().parse::<i16>() else { continue };
            temp_frame.update_reading(rom, raw_16ths);
        }
    }

    /// Sends `$OSCCAP`, reassembles the chunked `$OSCD,<seq>,...`/`$OSCEND` response into one
    /// sample buffer, and returns it. Mirrors the standalone `osc_capture` test's logic (see
    /// test/run_test.rs::run_osc_capture_test — validated there against real hardware) but
    /// runs inside the provider's own connection/read loop instead of a throwaway one, so it
    /// can share the live port with normal telemetry reads.
    ///
    /// Touches `frame`'s last-update timestamp on every iteration of the wait: the STM32
    /// pauses its normal telemetry send for the whole capture window, which would otherwise
    /// read as a dropped ADC link (see ADCFrame::touch) and flash the link-down alert on
    /// every single capture.
    fn perform_osc_capture(reader: &mut LineSerialReader, frame: &ADCFrame) -> Result<Vec<u16>, String> {
        reader.write_line("$OSCCAP\n")?;

        let mut chunks: Vec<Option<Vec<u16>>> = vec![None; OSC_EXPECTED_CHUNKS];
        let start = Instant::now();
        let mut got_end = false;
        let mut got_ack = false;

        while start.elapsed() < OSC_CAPTURE_TIMEOUT {
            frame.touch();
            match reader.read_line() {
                Some(line) if !line.is_empty() => {
                    if line == "$OSCEND" {
                        got_end = true;
                        break;
                    } else if line == "$OSCACK" {
                        got_ack = true;
                    } else if let Some(rest) = line.strip_prefix("$OSCD,") {
                        let mut parts = rest.split(',');
                        if let Some(seq) = parts.next().and_then(|s| s.parse::<usize>().ok()).filter(|&s| s < OSC_EXPECTED_CHUNKS) {
                            let values: Vec<u16> = parts.filter_map(|s| s.parse().ok()).collect();
                            if values.len() == OSC_CHUNK_SAMPLES {
                                chunks[seq] = Some(values);
                            }
                        }
                    }
                    // Ignore other lines (e.g. a trailing normal telemetry frame that raced in).
                }
                Some(_) => {} // read timeout — keep polling
                None => return Err("ADC serial link lost during capture".to_string()),
            }
        }

        if !got_end {
            let received = chunks.iter().filter(|c| c.is_some()).count();
            if !got_ack && received == 0 {
                return Err("STM32 never acknowledged $OSCCAP (no $OSCACK, no data) — flashed \
                            firmware likely predates the oscilloscope handler, or the serial \
                            RX path is down".to_string());
            }
            return Err(format!("timed out waiting for $OSCEND ({} of {} chunks received)", received, OSC_EXPECTED_CHUNKS));
        }

        chunks.into_iter().collect::<Option<Vec<_>>>()
            .map(|chunks| chunks.into_iter().flatten().collect())
            .ok_or_else(|| "capture missing one or more chunks".to_string())
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

    /// Returns a cloneable handle to the shared one-wire temperature store (see
    /// OneWireTempChannelProvider). Populated from `$T` lines by the background thread.
    pub fn temp_frame(&self) -> AdcTempFrame {
        self.temp_frame.clone()
    }

    /// Returns a cloneable handle for requesting oscilloscope burst captures (see OscPage).
    pub fn osc_frame(&self) -> OscFrame {
        self.osc_frame.clone()
    }

    /// Returns a cloneable handle to the STM32 firmware commit hash (see DiagPage).
    pub fn version_frame(&self) -> AdcVersionFrame {
        self.version_frame.clone()
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
/// The DS18B20 ROM addresses on the current bench one-wire bus — must match
/// sensor_config.json's HwTempOut/HwTempInt entries so the config-built temperature chains
/// have data during the self-test sweep (same "keep synthetic + real in step" coupling as
/// speed_period_raw_from_kmh above; changing a ROM in the JSON means changing it here too).
const SELF_TEST_TEMP_ROMS: [&str; 2] = ["2854df6b000000d9", "28cb586a00000059"];
/// °C span of the temperature sweep (envelope 0 → 1). Crosses the warning thresholds in
/// sensor_config.json so the ТЕМП page and any temperature watchdog get exercised.
const SELF_TEST_TEMP_MIN_C: f32 = 10.0;
const SELF_TEST_TEMP_PEAK_C: f32 = 70.0;

/// PA0/PA1/PA2 resistive-sender self-test sweep. Each channel is swept end to end across its
/// datasheet resistance curve and run back through the real divider inverse
/// (hardware::sensors::calibrated_sender_raw_from_ohm), so ДАВЛ МАСЛА / УРОВ ТОПЛ / ТЕМП
/// trace their true calibrated ranges instead of pegging or tripping the headroom fault on a
/// shared 0–4095 ramp. The endpoint resistances and r_series mirror sensor_config.json's
/// `calibrated_analog` entries (same "keep synthetic and real in step" coupling as
/// SELF_TEST_TEMP_ROMS -- a curve edit there must be mirrored here).
///
/// Spans are ordered (Ω at zero reading, Ω at full-scale reading). All three curves are
/// falling — resistance drops as the measured quantity rises (sensor_config.json: 305 Ω→0
/// kgf vs 7.5 Ω→8 kgf, 250 Ω→0 % vs 20 Ω→100 %, 1615 Ω→30 °C vs 58 Ω→130 °C) — so the
/// high-Ω end is listed first, letting the 0→1 envelope drive the *displayed* value 0→full.
const SELF_TEST_OIL_R_SERIES_OHM: f32 = 130.8;
const SELF_TEST_OIL_OHM_SPAN: (f32, f32) = (305.0, 7.5);
const SELF_TEST_FUEL_R_SERIES_OHM: f32 = 124.4;
const SELF_TEST_FUEL_OHM_SPAN: (f32, f32) = (250.0, 20.0);
const SELF_TEST_COOLANT_R_SERIES_OHM: f32 = 110.4;
const SELF_TEST_COOLANT_OHM_SPAN: (f32, f32) = (702.5, 58.0);
/// System-voltage band Hw12v sweeps on the envelope (0→1→0), spanning the 10–16 V voltage
/// gauge face so БОРТ СЕТЬ visibly travels end to end. `supply_v()` returns the
/// instantaneous value; it is fed both to the Hw12v raw encoder and to every calibrated
/// sender's raw encoder below. Because the self-test chains run with their moving averages
/// bypassed (setup_self_test_sensors passes bypass_analog_filters), the sender chains
/// decode against this exact same supply each tick, so the swept rail doesn't distort
/// ДАВЛ МАСЛА / УРОВ ТОПЛ / ТЕМП. SELF_TEST_V12_TRIM mirrors sensor_config.json's Hw12v `trim`.
const SELF_TEST_SUPPLY_MIN_V: f32 = 10.0;
const SELF_TEST_SUPPLY_MAX_V: f32 = 16.0;
const SELF_TEST_V12_TRIM: f32 = 1.036;

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
    temp_frame: AdcTempFrame,
    thread: Option<thread::JoinHandle<()>>,
}

impl TestADCDataProvider {
    /// Starts generating synthetic frames immediately. The returned handle must be kept
    /// alive for the sweep to keep animating — dropping it stops the background thread.
    /// Test-only convenience; production uses deferred() + begin_sweep() to control when
    /// the sweep clock starts relative to the first rendered frame.
    #[cfg(test)]
    pub fn start() -> Self {
        let mut provider = Self::deferred();
        provider.begin_sweep();
        provider
    }

    /// Builds the provider and its synthetic frames without starting the sweep, so the
    /// sensor chains can take frame()/temp_frame() handles now while the sweep clock only
    /// starts at begin_sweep(). Startup wires the chains early but doesn't render for
    /// another ~1 s (page/indicator/GL/freetype setup); starting the clock here instead
    /// would burn the rise and part of the fall before the first frame is drawn, so the
    /// needles would only ever be seen travelling back down toward zero.
    pub fn deferred() -> Self {
        TestADCDataProvider {
            should_stop: Arc::new(AtomicBool::new(false)),
            frame: ADCFrame::new(),
            temp_frame: AdcTempFrame::new(),
            thread: None,
        }
    }

    /// Spawns the synthetic writer thread and starts the sweep clock. Call once, just
    /// before the render loop begins. No-op if the sweep is already running.
    pub fn begin_sweep(&mut self) {
        if self.thread.is_some() {
            return;
        }
        let thread_should_stop = Arc::clone(&self.should_stop);
        let thread_frame = self.frame.clone();
        let thread_temp_frame = self.temp_frame.clone();

        self.thread = thread::Builder::new()
            .name("test-adc-data-provider".into())
            .spawn(move || Self::run_loop(&thread_should_stop, &thread_frame, &thread_temp_frame))
            .ok();
    }

    /// Returns a cloneable handle to the shared frame, same as ADCDataProvider::frame().
    pub fn frame(&self) -> ADCFrame {
        self.frame.clone()
    }

    /// Returns a cloneable handle to the synthetic one-wire temperature store, same as
    /// ADCDataProvider::temp_frame().
    pub fn temp_frame(&self) -> AdcTempFrame {
        self.temp_frame.clone()
    }

    fn run_loop(should_stop: &AtomicBool, frame: &ADCFrame, temp_frame: &AdcTempFrame) {
        let start = Instant::now();
        while !should_stop.load(Ordering::Relaxed) {
            let elapsed = start.elapsed();
            if elapsed >= SELF_TEST_DURATION {
                break;
            }
            frame.update(Self::generate_channels(elapsed));
            temp_frame.touch_line();
            for (rom, raw_16ths) in Self::generate_temp_readings(elapsed) {
                temp_frame.update_reading(rom, raw_16ths);
            }
            thread::sleep(SELF_TEST_TICK);
        }
    }

    /// Synthetic `$T` readings for the bench ROMs, sweeping SELF_TEST_TEMP_MIN_C..PEAK_C off
    /// the same envelope as the analog channels. Quantised to 0.25 °C (4/16) steps to match
    /// the real 10-bit DS18B20 resolution; the second sensor trails the first by 0.5 °C so
    /// the two rows read differently.
    fn generate_temp_readings(elapsed: Duration) -> [(&'static str, i16); 2] {
        let level = Self::envelope(elapsed);
        let celsius = SELF_TEST_TEMP_MIN_C + level * (SELF_TEST_TEMP_PEAK_C - SELF_TEST_TEMP_MIN_C);
        let raw_16ths = ((celsius * 4.0).round() * 4.0) as i16;
        [
            (SELF_TEST_TEMP_ROMS[0], raw_16ths),
            (SELF_TEST_TEMP_ROMS[1], raw_16ths - 8),
        ]
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

    /// Instantaneous synthetic supply voltage: sweeps SELF_TEST_SUPPLY_MIN_V..MAX_V on the
    /// envelope. Drives both the Hw12v channel and (as the live supply) every calibrated
    /// sender's raw encoding, so all four stay mutually consistent tick to tick.
    fn supply_v(elapsed: Duration) -> f32 {
        SELF_TEST_SUPPLY_MIN_V
            + Self::envelope(elapsed) * (SELF_TEST_SUPPLY_MAX_V - SELF_TEST_SUPPLY_MIN_V)
    }

    /// Builds one synthetic STM32 frame (channels 0-15, matching the real layout used by
    /// main.rs's add_adc_sensor_chains: A0-A3, TACHO, SPEED, D0-D9). Button channels
    /// (16-23) are omitted — the button sensor manager always reads the real ADCFrame
    /// directly, never the self-test one.
    fn generate_channels(elapsed: Duration) -> Vec<u16> {
        let level = Self::envelope(elapsed);
        let supply_v = Self::supply_v(elapsed);
        // Digital channels latch active only past the envelope's midpoint, so debouncers see
        // a clean active period followed by a clean inactive one rather than chattering.
        let digital_raw: u16 = if level > 0.5 { 1 } else { 0 };
        // Resistive senders: sweep each datasheet curve end to end in the Ω domain, then
        // invert the PA0/PA1/PA2 divider at the instantaneous self-test supply so the
        // calibrated chains reproduce their sensor_config.json curves rather than
        // clamping/faulting on a shared ramp (SENSOR_CALIBRATION_DESIGN.md). Spans run
        // (Ω at zero, Ω at full) so the envelope's 0→1→0 drives the displayed reading
        // 0→full→0, not full→0→full.
        let sweep_ohm = |(ohm_at_zero, ohm_at_full): (f32, f32)|
            ohm_at_zero + (ohm_at_full - ohm_at_zero) * level;
        let sender_raw = |span, r_series| crate::hardware::sensors::calibrated_sender_raw_from_ohm(
            sweep_ohm(span), r_series, supply_v,
        );
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
        channels[AdcChannel::OilPressure.index()] = sender_raw(SELF_TEST_OIL_OHM_SPAN, SELF_TEST_OIL_R_SERIES_OHM);  // HwOilPress
        channels[AdcChannel::FuelLevel.index()] = sender_raw(SELF_TEST_FUEL_OHM_SPAN, SELF_TEST_FUEL_R_SERIES_OHM);  // HwFuelLvl
        channels[AdcChannel::EngineTemp.index()] = sender_raw(SELF_TEST_COOLANT_OHM_SPAN, SELF_TEST_COOLANT_R_SERIES_OHM);  // HwEngineCoolantTemp
        channels[AdcChannel::Voltage12V.index()] = crate::hardware::sensors::v12_raw_from_volts(supply_v, SELF_TEST_V12_TRIM);  // Hw12v
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
mod temp_frame_tests {
    use super::*;

    #[test]
    fn parses_a_multi_sensor_temp_line() {
        let f = AdcTempFrame::new();
        ADCDataProvider::parse_temp_line(",2854df6b000000d9:404;28cb586a00000059:408", &f);
        assert_eq!(f.fresh_raw("2854df6b000000d9"), Some(404));
        assert_eq!(f.fresh_raw("28cb586a00000059"), Some(408));
        assert_eq!(f.addresses(), vec!["2854df6b000000d9", "28cb586a00000059"]);
    }

    #[test]
    fn decodes_a_negative_raw_value() {
        let f = AdcTempFrame::new();
        ADCDataProvider::parse_temp_line(",2854df6b000000d9:-88", &f);
        assert_eq!(f.fresh_raw("2854df6b000000d9"), Some(-88)); // -5.5 °C
    }

    #[test]
    fn skips_a_malformed_pair_without_dropping_the_good_ones() {
        let f = AdcTempFrame::new();
        // bad ROM length, non-numeric value, missing colon — all skipped; the last is kept.
        ADCDataProvider::parse_temp_line(
            ",28ff:100;deadbeefdeadbeef:xx;garbage;28cb586a00000059:400",
            &f,
        );
        assert_eq!(f.fresh_raw("28cb586a00000059"), Some(400));
        assert_eq!(f.addresses(), vec!["28cb586a00000059"]);
    }

    #[test]
    fn bare_keepalive_bumps_the_bus_timestamp_but_adds_no_readings() {
        let f = AdcTempFrame::new();
        std::thread::sleep(Duration::from_millis(10));
        ADCDataProvider::parse_temp_line("", &f);
        assert!(f.bus_last_line_age() < Duration::from_millis(5));
        assert!(f.addresses().is_empty());
    }

    #[test]
    fn unknown_address_reads_as_none() {
        let f = AdcTempFrame::new();
        ADCDataProvider::parse_temp_line(",2854df6b000000d9:404", &f);
        assert_eq!(f.fresh_raw("0000000000000000"), None);
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

    /// The calibrated oil / fuel / coolant channels must sweep their configured resistance
    /// span end to end (SENSOR_CALIBRATION_DESIGN.md) without ever tripping
    /// CalibratedVariableResistanceAnalogSensor's headroom / short-circuit fault, and in the
    /// right direction: reading ~0 at the envelope's ends and ~100 at its peak. The shared
    /// supply cell is stepped to supply_v(elapsed) each tick, exactly as the live Hw12v
    /// chain's SupplyVoltagePublisher would, so the swept rail stays consistent with the raw.
    /// Each sensor is rebuilt here with a synthetic curve that maps the span's zero-reading Ω
    /// to 0 and its full-reading Ω to 100 (ascending in Ω, as interpolate_curve requires).
    #[test]
    fn self_test_calibrated_channels_sweep_their_full_resistance_span() {
        use crate::hardware::sensors::CalibratedVariableResistanceAnalogSensor;
        use crate::hardware::sensor_value::ValueConstraints;
        use std::sync::Arc;
        use std::sync::atomic::AtomicU32; // Ordering comes from `use super::*`

        let cases: [(usize, (f32, f32), f32); 3] = [
            (AdcChannel::OilPressure.index(), SELF_TEST_OIL_OHM_SPAN, SELF_TEST_OIL_R_SERIES_OHM),
            (AdcChannel::FuelLevel.index(), SELF_TEST_FUEL_OHM_SPAN, SELF_TEST_FUEL_R_SERIES_OHM),
            (AdcChannel::EngineTemp.index(), SELF_TEST_COOLANT_OHM_SPAN, SELF_TEST_COOLANT_R_SERIES_OHM),
        ];

        for (idx, (ohm_at_zero, ohm_at_full), r_series) in cases {
            let v_supply = Arc::new(AtomicU32::new(TestADCDataProvider::supply_v(Duration::ZERO).to_bits()));
            // Ascending-in-Ω curve (interpolate_curve requires it): the full-scale end is the
            // low-Ω end for all three senders, so it carries value 100 and the zero end 0.
            let mut sensor = CalibratedVariableResistanceAnalogSensor::new(
                "x".to_string(), "x".to_string(), "u".to_string(), r_series,
                vec![(ohm_at_full, 100.0), (ohm_at_zero, 0.0)], 0.0,
                ValueConstraints::analog(0.0, 100.0), v_supply.clone(),
            );

            let mut elapsed = Duration::ZERO;
            let (mut lo_seen, mut hi_seen) = (f32::MAX, f32::MIN);
            let mut first_seen = None;
            let mut last_seen = 0.0;
            while elapsed < SELF_TEST_DURATION {
                v_supply.store(TestADCDataProvider::supply_v(elapsed).to_bits(), Ordering::Relaxed);
                let raw = TestADCDataProvider::generate_channels(elapsed)[idx];
                match sensor.read(raw) {
                    Ok(sv) => {
                        let v = sv.as_f32();
                        first_seen.get_or_insert(v);
                        last_seen = v;
                        lo_seen = lo_seen.min(v);
                        hi_seen = hi_seen.max(v);
                    }
                    Err(e) => panic!("channel {idx} faulted at elapsed={elapsed:?}: {e}"),
                }
                elapsed += SELF_TEST_TICK;
            }

            assert!(lo_seen < 1.0, "channel {idx} sweep floor only reached {lo_seen:.2}, expected ~0");
            assert!(hi_seen > 99.0, "channel {idx} sweep peak only reached {hi_seen:.2}, expected ~100");
            // Direction: the envelope starts and ends near 0, so the reading must too — a
            // sweep that began near full scale would mean the Ω span is ordered backwards.
            // (The final tick lands a hair above 0 since it's not exactly at the envelope's
            // end, hence the looser bound there.)
            assert!(first_seen.unwrap() < 1.0, "channel {idx} started at {:.2}, expected ~0", first_seen.unwrap());
            assert!(last_seen < 5.0, "channel {idx} ended at {last_seen:.2}, expected ~0");
        }
    }
}
