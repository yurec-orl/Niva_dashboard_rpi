//! Hand-rolled SHTP (Sensor Hub Transport Protocol) + SH-2 report parsing for the BNO085
//! IMU over I2C, via `rppal::i2c::I2c`. Decodes Rotation Vector, Game Rotation Vector,
//! Geomagnetic Rotation Vector, and Accelerometer.
//! Based on findings in research doc /home/user/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/BNO085_SHTP_PROTOCOL_RESEARCH.md

use rppal::gpio::{Gpio, InputPin, Trigger};

use std::collections::VecDeque;
use std::time::Duration;

/// I2C bus wired to the BNO085 breakout. Requires `dtparam=i2c_arm_baudrate=400000` in
/// `/boot/firmware/config.txt` — this chip's I2C interface was found to be unreliable at the
/// Pi's default 100 kHz (see research doc, "I2C clock speed").
pub const I2C_BUS: u8 = 1;
/// Confirmed via `i2cdetect` against the real board (the library default is 0x4A).
pub const BNO085_ADDR: u16 = 0x4B;
/// BCM pin wired to the breakout's INT (HINT) line — active low, asserted when a packet is
/// ready to read. Chosen clear of the I2C pins (GPIO2/3, shared with the UPS HAT) and of
/// GPIO14/15 (kept free for a possible future BOOT_UART capture, see PROJECT_CONTEXT.md).
///
/// Before this was wired, reads were attempted on a fixed 5ms timer regardless of whether the
/// sensor actually had anything pending — bench testing showed this reliably produced torn/
/// misaligned reads (garbage report IDs, 0xFF-padded tails) after only a few seconds, on both
/// this driver and the reference Adafruit CircuitPython driver, strongly suggesting a race
/// against the sensor's internal buffer swapping to a new report mid-read. Gating reads on
/// HINT means a read is only attempted once the sensor has said a packet is actually waiting,
/// which should substantially narrow that window — not a mathematical guarantee against every
/// possible race, but the standard recommended fix for this class of problem.
pub const HINT_PIN: u8 = 17;

// SHTP channels (SHTP spec §2.2, fixed assignment). Only the three actually used here.
const CHANNEL_EXECUTABLE: u8 = 1;
const CHANNEL_CONTROL: u8 = 2;
const CHANNEL_REPORTS: u8 = 3;
/// Total channel count (0-5) — any parsed header channel byte must be checked against this
/// before being trusted, per the root-cause lesson in the research doc ("Root cause of both
/// crashes hit during testing"): an out-of-range channel from a noisy/misaligned read must
/// never be used to index anything.
const CHANNEL_COUNT: u8 = 6;

// SH-2 report/command IDs actually used here (SH-2 reference manual §6.3.x).
const SH2_SET_FEATURE_COMMAND: u8 = 0xFD;
const SH2_PRODUCT_ID_REQUEST: u8 = 0xF9;
const SH2_PRODUCT_ID_RESPONSE: u8 = 0xF8;
pub const SH2_REPORT_ACCELEROMETER: u8 = 0x01;
pub const SH2_REPORT_ROTATION_VECTOR: u8 = 0x05;
/// Fuses gyro + accelerometer only, deliberately excludes the magnetometer — isolates gyro
/// heading tracking from magnetic distortion for the heading test's "which fusion input is
/// actually right" comparison (see the review discussion, memory/project notes on GNSS/BNO085
/// heading testing). Has no absolute reference, so unlike Rotation Vector it carries no
/// heading accuracy estimate.
pub const SH2_REPORT_GAME_ROTATION_VECTOR: u8 = 0x08;
/// Fuses accelerometer + magnetometer only, deliberately excludes the gyro — the magnetometer-
/// only counterpart to Game Rotation Vector, for the same isolation purpose. Same 14-byte
/// layout as Rotation Vector (it does have an absolute reference, hence a heading accuracy
/// estimate too).
pub const SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR: u8 = 0x09;
/// Prefixed onto every channel-3 batch ahead of the actual sensor report(s), even when only
/// one feature is enabled — confirmed against Adafruit's driver (`_BASE_TIMESTAMP = 0xFB`,
/// length 5: report ID + 4-byte time delta). Not mentioned explicitly in the research doc's
/// SHTP framing section, which describes a single report's own byte layout, not the batch
/// wrapper around it — missing this was the reason every report was silently discarded.
const SH2_REPORT_BASE_TIMESTAMP: u8 = 0xFB;
const SH2_REPORT_BASE_TIMESTAMP_LEN: usize = 5;

/// Upper bound on a single SHTP packet's declared length. Real traffic here (startup
/// advertisement, our two enabled reports) stays well under this; a header claiming more is
/// treated as a corrupted/garbage read (bus noise) rather than something to allocate for.
const MAX_PACKET_LEN: usize = 4096;

/// How many HINT-gated read attempts `init` will make while waiting for the Product ID
/// Response before giving up, and how long each attempt blocks waiting for HINT.
///
/// Deliberately kept a small total budget (MAX_INIT_ATTEMPTS * INIT_HINT_TIMEOUT = 1s) rather
/// than a generous one: a real successful init has been observed completing in low tens of
/// milliseconds, so 1s is already generous for the success path. Persistence for "the sensor
/// isn't back yet" belongs to the caller's reconnect loop (Bno085DataProvider's
/// RECONNECT_INTERVAL, which retries every 200ms indefinitely) — a single init() call sitting
/// here for a long timeout instead just adds dead time on top of that outer retry, which is
/// exactly what happened with the previous 50 * 200ms = 10s budget: one reconnect attempt
/// landing while the sensor was still mid-glitch blocked for the full 10s before the outer
/// loop got another chance, even though the sensor came back well before that.
const MAX_INIT_ATTEMPTS: u32 = 10;
const INIT_HINT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub enum Bno085Error {
    I2c(rppal::i2c::Error),
    Gpio(rppal::gpio::Error),
    /// `init` didn't see a Product ID Response within MAX_INIT_ATTEMPTS attempts.
    InitTimeout,
}

impl std::fmt::Display for Bno085Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I2c(e) => write!(f, "I2C error: {}", e),
            Self::Gpio(e) => write!(f, "GPIO error: {}", e),
            Self::InitTimeout => write!(f, "timed out waiting for Product ID Response"),
        }
    }
}

impl std::error::Error for Bno085Error {}

impl From<rppal::i2c::Error> for Bno085Error {
    fn from(e: rppal::i2c::Error) -> Self {
        Bno085Error::I2c(e)
    }
}

impl From<rppal::gpio::Error> for Bno085Error {
    fn from(e: rppal::gpio::Error) -> Self {
        Bno085Error::Gpio(e)
    }
}

/// Per-report SH-2 accuracy status (2-bit field common to every report's byte 2, bits 0-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accuracy {
    Unreliable,
    Low,
    Medium,
    High,
}

impl Accuracy {
    fn from_status_byte(status: u8) -> Self {
        match status & 0x03 {
            0 => Accuracy::Unreliable,
            1 => Accuracy::Low,
            2 => Accuracy::Medium,
            _ => Accuracy::High,
        }
    }
}

/// Decoded Rotation Vector report (SH-2 report ID 0x05): orientation quaternion plus a
/// heading accuracy estimate, still in the chip's own axis convention — no mounting-offset
/// correction applied here (see research doc, "Calibration" / Tare / Set Reorientation).
#[derive(Debug, Clone, Copy)]
pub struct RotationVectorReport {
    pub quat_i: f32,
    pub quat_j: f32,
    pub quat_k: f32,
    pub quat_real: f32,
    /// Radians (Q12), only meaningful for the heading (yaw) component.
    pub heading_accuracy_rad: f32,
    pub accuracy: Accuracy,
}

impl RotationVectorReport {
    /// Euler yaw/pitch/roll in radians, ZYX order, per the quaternion->Euler conversion
    /// derived in the research doc ("Report selection: Rotation Vector for heading and
    /// inclination"). `yaw` is magnetic heading, not true heading. Which axis is physically
    /// "roll" vs "pitch" depends on board mounting, not resolved here.
    pub fn euler_rad(&self) -> (f32, f32, f32) {
        quat_to_euler_rad(self.quat_i, self.quat_j, self.quat_k, self.quat_real)
    }
}

/// Decoded Game Rotation Vector report (SH-2 report ID 0x08): orientation quaternion from
/// gyro+accelerometer fusion only, no magnetometer input and no accuracy estimate (see
/// SH2_REPORT_GAME_ROTATION_VECTOR).
#[derive(Debug, Clone, Copy)]
pub struct GameRotationVectorReport {
    pub quat_i: f32,
    pub quat_j: f32,
    pub quat_k: f32,
    pub quat_real: f32,
}

impl GameRotationVectorReport {
    /// Same quaternion->Euler conversion as RotationVectorReport::euler_rad; `yaw` here is a
    /// gyro-integrated relative heading with no magnetometer or absolute reference at all, so
    /// it will drift over time and starts from an arbitrary origin at each sensor reset.
    pub fn euler_rad(&self) -> (f32, f32, f32) {
        quat_to_euler_rad(self.quat_i, self.quat_j, self.quat_k, self.quat_real)
    }
}

/// Shared quaternion->Euler (yaw/pitch/roll, ZYX order) conversion used by both
/// RotationVectorReport and GameRotationVectorReport — same math, just applied to quaternions
/// from different fusion inputs.
fn quat_to_euler_rad(i: f32, j: f32, k: f32, r: f32) -> (f32, f32, f32) {
    let yaw = (2.0 * (r * k + i * j)).atan2(1.0 - 2.0 * (j * j + k * k));
    let roll = (2.0 * (r * i + j * k)).atan2(1.0 - 2.0 * (i * i + j * j));
    let pitch = (2.0 * (r * j - k * i)).clamp(-1.0, 1.0).asin();
    (yaw, pitch, roll)
}

/// Decoded Accelerometer report (SH-2 report ID 0x01): instantaneous acceleration including
/// gravity, in m/s^2.
#[derive(Debug, Clone, Copy)]
pub struct AccelerometerReport {
    pub x_mps2: f32,
    pub y_mps2: f32,
    pub z_mps2: f32,
    pub accuracy: Accuracy,
}

/// A parsed SH-2 report from the reports channel.
#[derive(Debug, Clone, Copy)]
pub enum Bno085Report {
    RotationVector(RotationVectorReport),
    GameRotationVector(GameRotationVectorReport),
    /// Same wire layout and fields as RotationVector — distinguished only by which SH-2
    /// report ID it arrived on (0x09 vs 0x05), i.e. which sensors were actually fused.
    GeomagneticRotationVector(RotationVectorReport),
    Accelerometer(AccelerometerReport),
}

/// Result of one `poll()` call.
pub enum Bno085Event {
    /// HINT didn't assert within the caller's timeout — routine, just means nothing was
    /// pending yet.
    None,
    /// A recognized report arrived on the reports channel.
    Report(Bno085Report),
    /// The EXE channel's "reset complete" notification — the sensor actually reset, which
    /// silently clears its feature-enable state on the device side. The caller must re-issue
    /// Set Feature Command or no further reports will ever arrive (research doc, "Silent
    /// feature staleness").
    ResetComplete,
}

/// Owns the I2C connection and per-channel write sequence numbers for a single BNO085. Not
/// `Clone` / not thread-safe by itself — intended for exclusive use by one background thread
/// (see bno085_data_provider.rs), same shape as UpsI2CDataProvider's bare `I2c` handle.
pub struct Bno085 {
    i2c: rppal::i2c::I2c,
    hint: InputPin,
    write_seq: [u8; CHANNEL_COUNT as usize],
    /// Reports already decoded from a batch but not yet handed to the caller — a single
    /// channel-3 packet can carry more than one sensor report back-to-back (e.g. both
    /// Rotation Vector and Accelerometer landing in the same output cycle), but `poll()`
    /// returns one event at a time, so any extras from the last read wait here.
    pending: VecDeque<Bno085Report>,
}

impl Bno085 {
    pub fn open(bus: u8, address: u16, hint_pin: u8) -> Result<Self, Bno085Error> {
        let mut i2c = rppal::i2c::I2c::with_bus(bus)?;
        i2c.set_slave_address(address)?;

        // Pull-up + falling-edge: HINT idles high, the sensor pulls it low to signal a
        // packet is ready. No debounce — this is a clean digital assertion from the
        // sensor's own MCU, not a mechanical contact.
        let mut hint = Gpio::new()?.get(hint_pin)?.into_input_pullup();
        hint.set_interrupt(Trigger::FallingEdge, None)?;

        Ok(Bno085 { i2c, hint, write_seq: [0; CHANNEL_COUNT as usize], pending: VecDeque::new() })
    }

    /// Proven init sequence (research doc, "Startup handshake is simpler in practice than the
    /// spec implies"): send a Product ID Request, then read-and-discard whatever comes back
    /// until the Product ID Response shows up. No special-casing of the unsolicited startup
    /// advertisement needed — it resolves itself as just more discarded traffic.
    pub fn init(&mut self) -> Result<(), Bno085Error> {
        self.write_packet(CHANNEL_CONTROL, &[SH2_PRODUCT_ID_REQUEST, 0])?;

        for _ in 0..MAX_INIT_ATTEMPTS {
            if !self.wait_for_hint(INIT_HINT_TIMEOUT)? {
                continue;
            }
            if let Some((channel, cargo)) = self.read_raw_packet()? {
                if channel == CHANNEL_CONTROL && cargo.first() == Some(&SH2_PRODUCT_ID_RESPONSE) {
                    return Ok(());
                }
            }
        }
        Err(Bno085Error::InitTimeout)
    }

    /// Sends a Set Feature Command enabling `report_id` at `interval_us` microseconds between
    /// reports. Byte layout confirmed against real hardware (research doc, "Set Feature
    /// Command byte layout"). Change-sensitivity, batching, and sensor-specific config are
    /// left zero — nothing here needs them.
    pub fn enable_feature(&mut self, report_id: u8, interval_us: u32) -> Result<(), Bno085Error> {
        let mut cargo = [0u8; 17];
        cargo[0] = SH2_SET_FEATURE_COMMAND;
        cargo[1] = report_id;
        cargo[5..9].copy_from_slice(&interval_us.to_le_bytes());
        self.write_packet(CHANNEL_CONTROL, &cargo)
    }

    /// Blocks up to `timeout` waiting for HINT to indicate a packet is ready, then reads and
    /// classifies it. A read is only attempted once HINT actually asserts (or was already
    /// asserted when this call started) — no more blind polling on a fixed timer. Returns
    /// `Bno085Event::None` both when the timeout elapses with nothing pending and when a
    /// packet arrived but wasn't one this driver cares about, distinguished from a real error
    /// only by the `Result`.
    pub fn poll(&mut self, timeout: Duration) -> Result<Bno085Event, Bno085Error> {
        if let Some(report) = self.pending.pop_front() {
            return Ok(Bno085Event::Report(report));
        }

        if !self.wait_for_hint(timeout)? {
            return Ok(Bno085Event::None);
        }

        let Some((channel, cargo)) = self.read_raw_packet()? else {
            return Ok(Bno085Event::None);
        };

        if channel == CHANNEL_EXECUTABLE && !cargo.is_empty() {
            return Ok(Bno085Event::ResetComplete);
        }

        if channel != CHANNEL_REPORTS || cargo.is_empty() {
            return Ok(Bno085Event::None);
        }

        Self::parse_report_batch(&cargo, &mut self.pending);
        Ok(self.pending.pop_front().map(Bno085Event::Report).unwrap_or(Bno085Event::None))
    }

    /// Returns `true` once HINT is (or becomes, within `timeout`) asserted (active low). Checks
    /// the current level first rather than only arming the falling-edge interrupt, since HINT
    /// may already have been low before this call started watching for the edge (e.g. right
    /// after `enable_feature`, or when a batch left more than one report queued).
    fn wait_for_hint(&mut self, timeout: Duration) -> Result<bool, Bno085Error> {
        if self.hint.is_low() {
            return Ok(true);
        }
        Ok(self.hint.poll_interrupt(true, Some(timeout))?.is_some())
    }

    /// Walks a channel-3 cargo as a sequence of back-to-back sub-reports (SH-2's "batching":
    /// one Base Timestamp Reference followed by one or more sensor reports, all concatenated
    /// in a single SHTP packet), pushing every recognized sensor report onto `out`.
    ///
    /// An unrecognized report ID stops parsing the rest of this packet rather than guessing
    /// at its length — same "don't trust an unbounded/unknown field" caution as the
    /// channel-bounds check in read_raw_packet (research doc, "Root cause of both crashes hit
    /// during testing"). Whatever was already decoded earlier in the batch is kept.
    fn parse_report_batch(cargo: &[u8], out: &mut VecDeque<Bno085Report>) {
        let mut offset = 0;
        while offset < cargo.len() {
            let remaining = &cargo[offset..];
            let consumed = match remaining[0] {
                SH2_REPORT_BASE_TIMESTAMP if remaining.len() >= SH2_REPORT_BASE_TIMESTAMP_LEN => {
                    SH2_REPORT_BASE_TIMESTAMP_LEN
                }
                SH2_REPORT_ROTATION_VECTOR if remaining.len() >= 14 => {
                    out.push_back(Bno085Report::RotationVector(Self::parse_rotation_vector(remaining)));
                    14
                }
                SH2_REPORT_GAME_ROTATION_VECTOR if remaining.len() >= 12 => {
                    out.push_back(Bno085Report::GameRotationVector(Self::parse_game_rotation_vector(remaining)));
                    12
                }
                SH2_REPORT_GEOMAGNETIC_ROTATION_VECTOR if remaining.len() >= 14 => {
                    out.push_back(Bno085Report::GeomagneticRotationVector(Self::parse_rotation_vector(remaining)));
                    14
                }
                SH2_REPORT_ACCELEROMETER if remaining.len() >= 10 => {
                    out.push_back(Bno085Report::Accelerometer(Self::parse_accelerometer(remaining)));
                    10
                }
                _ => break,
            };
            offset += consumed;
        }
    }

    fn parse_rotation_vector(cargo: &[u8]) -> RotationVectorReport {
        RotationVectorReport {
            quat_i: q_to_f32(i16::from_le_bytes([cargo[4], cargo[5]]), 14),
            quat_j: q_to_f32(i16::from_le_bytes([cargo[6], cargo[7]]), 14),
            quat_k: q_to_f32(i16::from_le_bytes([cargo[8], cargo[9]]), 14),
            quat_real: q_to_f32(i16::from_le_bytes([cargo[10], cargo[11]]), 14),
            heading_accuracy_rad: q_to_f32(i16::from_le_bytes([cargo[12], cargo[13]]), 12),
            accuracy: Accuracy::from_status_byte(cargo[2]),
        }
    }

    fn parse_game_rotation_vector(cargo: &[u8]) -> GameRotationVectorReport {
        GameRotationVectorReport {
            quat_i: q_to_f32(i16::from_le_bytes([cargo[4], cargo[5]]), 14),
            quat_j: q_to_f32(i16::from_le_bytes([cargo[6], cargo[7]]), 14),
            quat_k: q_to_f32(i16::from_le_bytes([cargo[8], cargo[9]]), 14),
            quat_real: q_to_f32(i16::from_le_bytes([cargo[10], cargo[11]]), 14),
        }
    }

    fn parse_accelerometer(cargo: &[u8]) -> AccelerometerReport {
        AccelerometerReport {
            x_mps2: q_to_f32(i16::from_le_bytes([cargo[4], cargo[5]]), 8),
            y_mps2: q_to_f32(i16::from_le_bytes([cargo[6], cargo[7]]), 8),
            z_mps2: q_to_f32(i16::from_le_bytes([cargo[8], cargo[9]]), 8),
            accuracy: Accuracy::from_status_byte(cargo[2]),
        }
    }

    /// Two-transaction read strategy validated against real hardware (research doc, "I2C read
    /// strategy"): a 4-byte header-only read to learn the length, then a *second*, independent
    /// read of `length` bytes starting over from byte 0. SHTP over I2C forbids repeated
    /// starts, so the sensor just replays the same pending cargo on each new transaction until
    /// one read is long enough to fully drain it — no continuation-bit handling needed for
    /// packets this small.
    fn read_raw_packet(&mut self) -> Result<Option<(u8, Vec<u8>)>, Bno085Error> {
        let mut header = [0u8; 4];
        self.i2c.read(&mut header)?;

        let length = (u16::from_le_bytes([header[0], header[1]]) & 0x7FFF) as usize;
        if length <= 4 {
            return Ok(None); // null header — no cargo pending
        }
        if length > MAX_PACKET_LEN {
            log::warn!("BNO085: implausible packet length {}, discarding as garbage", length);
            return Ok(None);
        }

        let channel = header[2];
        if channel >= CHANNEL_COUNT {
            log::warn!("BNO085: packet header claims out-of-range channel {}, discarding", channel);
            return Ok(None);
        }

        let mut buf = vec![0u8; length];
        self.i2c.read(&mut buf)?;
        Ok(Some((channel, buf[4..].to_vec())))
    }

    fn write_packet(&mut self, channel: u8, cargo: &[u8]) -> Result<(), Bno085Error> {
        let seq = self.write_seq[channel as usize];
        self.write_seq[channel as usize] = seq.wrapping_add(1);

        let length = (cargo.len() + 4) as u16;
        let mut packet = Vec::with_capacity(cargo.len() + 4);
        packet.extend_from_slice(&length.to_le_bytes());
        packet.push(channel);
        packet.push(seq);
        packet.extend_from_slice(cargo);

        self.i2c.write(&packet)?;
        Ok(())
    }
}

/// "Q<n>" fixed point (SH-2 reference manual convention): divide the raw int16 by 2^n.
fn q_to_f32(raw: i16, q_point: u8) -> f32 {
    raw as f32 / (1u32 << q_point) as f32
}
