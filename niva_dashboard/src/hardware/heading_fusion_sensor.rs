#![allow(dead_code)]
//! Heading fusion: Game Rotation Vector (BNO085 gyro+accel, no magnetometer) as a continuously
//! -running relative tracker, anchored and periodically re-anchored by validated GNSS fixes
//! (`course_deg` while moving, `heading_deg` while stationary).
//!
//! Reads `GnssFrame`/`Bno085Frame` directly rather than going through the `HWAnalogProvider`/
//! `SensorFusedAnalogInputChain` machinery: the gating logic here needs several
//! concurrently-consistent fields per source (course_deg, heading_deg, heading_std_dev_deg,
//! speed_kmh -- all from one atomic `GnssFrame::fix()` snapshot) and produces multiple
//! independent outputs (heading, confidence, accuracy), none of which fit that chain type's
//! one-scalar-in/one-`SensorValue`-out shape.

use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueData, ValueMetadata};
use crate::util::bno085_data_provider::Bno085Frame;
use crate::util::gnss_data_provider::GnssFrame;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// GNSS course_deg is dominated by position noise below this speed -- not yet validated
/// against a real moving trial, just a starting point pending field data.
const MIN_SPEED_FOR_COURSE_KMH: f32 = 10.0;

/// GNSS heading_deg is only fed into the sustained-agreement check when its own reported
/// std-dev is at least this tight. Provisional.
const HEADING_STD_DEV_MAX_DEG: f32 = 30.0;

/// Sustained-agreement window: a GNSS candidate is only validated once at least
/// AGREEMENT_MIN_SAMPLES readings taken within this trailing window all agree with each other
/// to within AGREEMENT_TOLERANCE_DEG. Provisional.
const AGREEMENT_WINDOW: Duration = Duration::from_secs(5);
const AGREEMENT_MIN_SAMPLES: usize = 5;
const AGREEMENT_TOLERANCE_DEG: f32 = 15.0;

/// How fast a validated correction is allowed to visually ramp in, so a large correction
/// doesn't read as a needle glitch. Provisional.
const SLEW_RATE_DEG_PER_SEC: f32 = 30.0;

/// Below this circular distance from the last persisted value, a persist call is a no-op --
/// avoids a redundant SD write when a manual correction is immediately followed by shutdown.
const PERSIST_MIN_DELTA_DEG: f32 = 1.0;

/// BNO085 datasheet's Game Rotation Vector drift figure in pure-inertial operation (no
/// magnetometer, no external correction). Applied to `accuracy_deg` only while dead-reckoning
/// (see `degrade_accuracy`) -- a validated GNSS correction always resets accuracy outright
/// rather than accumulating against it.
const INERTIAL_DRIFT_DEG_PER_MIN: f32 = 0.5;

/// Below this raw Game RV delta between two ticks, the reading is treated as "didn't actually
/// move" rather than float noise -- empirically the BNO085 seems to re-calibrate itself with no
/// observed drift while fully stationary, so drift is only charged against `accuracy_deg` on a
/// tick where the raw heading genuinely changed.
const HEADING_STATIONARY_EPSILON_DEG: f32 = 0.01;

/// GNSS `course_deg` has no reported accuracy figure of its own (unlike `heading_deg`'s
/// `heading_std_dev_deg`) -- assumed accuracy for a course-validated anchor. Provisional; not
/// yet checked against a real moving trial.
const COURSE_ANCHOR_ACCURACY_DEG: f32 = 2.0;

/// Accuracy assigned to a manually-set anchor -- treated as a perfect reference point.
const MANUAL_ANCHOR_ACCURACY_DEG: f32 = 0.0;

/// Upper display clamp for `accuracy_deg` -- past this, the number stops being meaningful (the
/// heading itself is effectively random) so there's no point letting it grow unbounded across a
/// long uncorrected stretch. Not a claim about actual BNO085 drift bounds -- long-duration Game
/// RV drift is uncharacterized.
const MAX_DISPLAYED_ACCURACY_DEG: f32 = 90.0;

/// BNO085 Game Rotation Vector's first report(s) after a (re)connect can reflect a not-yet-
/// settled integration reference rather than the board's true current orientation -- observed
/// in practice as a persisted-prior anchor landing 100+ degrees off after a restart with the
/// board physically untouched (the offset math is otherwise self-canceling: it reproduces
/// `persisted_deg` exactly on the tick it's computed, so an unsettled `raw` bakes that
/// convergence error in as a permanent offset, since nothing else re-anchors PersistedPrior
/// until a validated GNSS fix arrives). Reuses the same sustained-agreement mechanism as the
/// GNSS gates (SustainedAgreement), just with a much shorter window since this is about sensor
/// settle time, not distrust of noisy external data. Provisional -- not yet checked against
/// how long a real settle actually takes.
const GAME_RV_SETTLE_WINDOW: Duration = Duration::from_millis(500);
const GAME_RV_SETTLE_MIN_SAMPLES: usize = 5;
const GAME_RV_SETTLE_TOLERANCE_DEG: f32 = 2.0;

/// Coded the same way `nmea::FixQuality::code()` is, so it can be surfaced as a plain analog
/// value (HWInput::HwHeadingConfidence) instead of growing SensorValue a dedicated field for
/// one consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingConfidence {
    /// No anchor obtained yet this boot -- no heading to display.
    Unknown,
    /// Anchor restored from the last shutdown's saved heading; not yet confirmed by a fresh
    /// validated GNSS fix this boot.
    PersistedPrior,
    /// Anchor was validated at some point, but no source is currently passing its gate.
    /// `HeadingFusionSensor::dead_reckoning_elapsed` reports how long.
    DeadReckoning,
    /// A currently-agreeing, validated fix just updated the anchor -- set the instant the fix
    /// validates, independent of whether the on-screen display has finished slewing to match.
    GnssCorrected,
    /// Anchor set by direct user input -- trusted equivalent to GnssCorrected for tracking,
    /// labeled separately for the UI.
    Manual,
}

impl HeadingConfidence {
    pub fn code(self) -> i32 {
        match self {
            HeadingConfidence::Unknown => 0,
            HeadingConfidence::PersistedPrior => 1,
            HeadingConfidence::DeadReckoning => 2,
            HeadingConfidence::GnssCorrected => 3,
            HeadingConfidence::Manual => 4,
        }
    }
}

/// Smallest circular distance between two headings, in [0, 180].
fn circular_diff_deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 { 360.0 - d } else { d }
}

/// Signed shortest rotation from `from` to `target`, in (-180, 180].
fn circular_signed_diff_deg(target: f32, from: f32) -> f32 {
    let mut d = (target - from).rem_euclid(360.0);
    if d > 180.0 { d -= 360.0; }
    d
}

/// Tracks whether a source's recent candidate readings agree with each other closely enough,
/// over a trailing time window, to be trusted. A single confidently-wrong reading (the failure
/// mode seen in the window-sill GNSS heading_deg trials) can't pass this on its own no matter
/// how tight its own reported accuracy claims to be.
struct SustainedAgreement {
    window: Duration,
    min_samples: usize,
    tolerance_deg: f32,
    samples: VecDeque<(Instant, f32)>,
}

impl SustainedAgreement {
    fn new(window: Duration, min_samples: usize, tolerance_deg: f32) -> Self {
        SustainedAgreement { window, min_samples, tolerance_deg, samples: VecDeque::new() }
    }

    /// Feeds one candidate reading that already passed this source's own eligibility gate
    /// (e.g. min-speed for course_deg, std-dev for heading_deg). Returns the candidate back
    /// once enough recent samples agree within tolerance, None otherwise.
    fn push(&mut self, now: Instant, value_deg: f32) -> Option<f32> {
        self.samples.push_back((now, value_deg));
        while let Some(&(t, _)) = self.samples.front() {
            if now.saturating_duration_since(t) > self.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }

        if self.samples.len() < self.min_samples {
            return None;
        }

        let mut max_spread = 0.0f32;
        for i in 0..self.samples.len() {
            for j in (i + 1)..self.samples.len() {
                let d = circular_diff_deg(self.samples[i].1, self.samples[j].1);
                max_spread = max_spread.max(d);
            }
        }

        if max_spread <= self.tolerance_deg {
            Some(value_deg)
        } else {
            None
        }
    }

    /// Called when the source fails its own eligibility gate this tick, so a gap in
    /// eligibility can't let older samples count toward agreement once it resumes.
    fn reset(&mut self) {
        self.samples.clear();
    }
}

/// One source's validated candidate, paired with the accuracy that comes with it -- returned by
/// `validated_gnss_correction` so `apply_correction` can reset `accuracy_deg` to the anchor's
/// own accuracy rather than whatever it had degraded to beforehand.
struct GnssAnchor {
    heading_deg: f32,
    accuracy_deg: f32,
}

/// Heading persisted across restarts. Loaded as `PersistedPrior`, never as `GnssCorrected` --
/// it goes through the same validation gate as any other anchor before being trusted again.
/// The timestamp is for UI/diagnostics only. `accuracy_deg` is `None` for files written before
/// this field existed (`#[serde(default)]`) or when nothing had ever anchored the sensor at
/// persist time -- `apply_persisted_prior_if_ready` treats that as 0.0 on load rather than
/// leaving the resumed anchor with no accuracy to degrade from.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct PersistedHeading {
    heading_deg: f32,
    #[serde(default)]
    accuracy_deg: Option<f32>,
    timestamp_unix_secs: u64,
}

/// `tick()`'s result: heading, confidence tier, and heading accuracy estimate, each ready to
/// push into `SensorManager` via `set_external_value`. Named fields instead of a tuple since a
/// third, unrelated-by-position value (`accuracy`) joined `heading`/`confidence` here.
pub struct HeadingFusionOutput {
    pub heading: SensorValue,
    pub confidence: SensorValue,
    pub accuracy: SensorValue,
    pub dead_reckoning_elapsed: SensorValue,
}

/// Combines a continuously-integrated BNO085 Game Rotation Vector (the always-running
/// backbone) with validated GNSS course/heading corrections into one heading estimate plus a
/// confidence tier. Not a `Sensor`/`FusedAnalogSensor` impl -- see the module doc for why;
/// `tick()` is called directly once per sensor-read cycle and its outputs are pushed into
/// `SensorManager` via `set_external_value`.
pub struct HeadingFusionSensor {
    gnss_frame: GnssFrame,
    bno_frame: Bno085Frame,

    confidence: HeadingConfidence,
    /// Offset such that the true current heading == `(correction_offset_deg + game_rv_deg) mod 360` --
    /// i.e. `anchor + (game_rv_now - game_rv_at_anchor)`, with this field holding the constant
    /// `anchor - game_rv_at_anchor` term. None until the first anchor (persisted or
    /// GNSS-validated) is established (`Unknown` tier). Updated instantly on every validated
    /// correction/manual input -- not itself slew-limited, so it (and `confidence`) always
    /// reflect the true current correction state. `display_offset_deg` is what actually ramps.
    correction_offset_deg: Option<f32>,
    /// Slew-rate-limited chase of `correction_offset_deg`, recombined with the (always
    /// instant, unfiltered) `game_rv_deg` at read time to produce `displayed_heading()`. Kept
    /// as a separate offset rather than filtering the combined heading directly so that real
    /// rotation (which shows up identically in both offsets and cancels out) is never
    /// rate-limited -- only the discontinuous jump introduced by a correction is smoothed.
    /// None exactly when `correction_offset_deg` is None.
    display_offset_deg: Option<f32>,
    /// Most recent Game RV wrapped heading reading, combined with `correction_offset_deg` at
    /// read time rather than integrated into a running sum. Recomputing from the raw sample
    /// every tick (instead of accumulating a per-tick delta) avoids compounding float rounding
    /// error over a long-running session, and avoids a stale-data gap (`is_stale()`) followed
    /// by >180 degrees of real rotation during the gap being silently aliased to a small delta
    /// in the wrong direction.
    game_rv_deg: Option<f32>,

    /// Set on every tick a validated correction is applied (anchor snap, or an in-progress
    /// slew step) -- NOT on the tick confidence drops to DeadReckoning. Read via
    /// `dead_reckoning_elapsed` as "time since the last correction".
    last_correction_instant: Option<Instant>,

    /// Estimated current heading accuracy, degrees. Only None while `correction_offset_deg` is
    /// also None (Unknown) -- there's no known accuracy figure to degrade from before the first
    /// anchor of any kind. A `PersistedPrior` anchor always gets a `Some` accuracy to resume
    /// degrading from (see `apply_persisted_prior_if_ready`): whatever was persisted alongside
    /// the heading, or 0.0 if the file predates that field / was written before the sensor was
    /// ever anchored -- never left at `None`, which would silently hide INS drift for the rest
    /// of the PersistedPrior stretch. Set/reset outright to the anchor's own accuracy on every
    /// validated GNSS correction or manual input; degraded by `degrade_accuracy` on ticks spent
    /// dead-reckoning instead.
    accuracy_deg: Option<f32>,

    course_agreement: SustainedAgreement,
    heading_agreement: SustainedAgreement,
    /// Settle check for the raw Game RV baseline used by `apply_persisted_prior_if_ready` --
    /// see GAME_RV_SETTLE_WINDOW's doc comment for why this exists. Not consulted by the GNSS
    /// anchor paths, which already have an effective warm-up delay from their own multi-second
    /// agreement windows.
    game_rv_settle: SustainedAgreement,

    /// Heading loaded from disk at construction, not yet folded into `correction_offset_deg`
    /// because that requires a Game RV baseline (BNO085 may not have reported yet).
    pending_persisted_deg: Option<f32>,
    /// Accuracy loaded alongside `pending_persisted_deg`, folded into `accuracy_deg` at the same
    /// moment. Independently `None` (persisted file predates this field, or nothing had ever
    /// anchored the sensor when it was written) even when `pending_persisted_deg` is `Some`.
    pending_persisted_accuracy_deg: Option<f32>,

    persist_path: PathBuf,
    /// Last value actually written to disk, for `persist_if_changed`'s dedup check. None
    /// means nothing has been persisted yet this run.
    last_persisted_deg: Option<f32>,

    last_tick_instant: Instant,

    heading_constraints: ValueConstraints,
    heading_metadata: ValueMetadata,
    confidence_metadata: ValueMetadata,
    accuracy_constraints: ValueConstraints,
    accuracy_metadata: ValueMetadata,
    dead_reckoning_elapsed_constraints: ValueConstraints,
    dead_reckoning_elapsed_metadata: ValueMetadata,
}

impl HeadingFusionSensor {
    pub fn new(gnss_frame: GnssFrame, bno_frame: Bno085Frame) -> Self {
        let persist_path = Self::default_persist_path();
        let pending_persisted = Self::load_persisted(&persist_path);
        if pending_persisted.is_some() {
            log::info!("Heading fusion: loaded persisted prior heading from {:?}", persist_path);
        }
        let pending_persisted_deg = pending_persisted.as_ref().map(|p| p.heading_deg);
        let pending_persisted_accuracy_deg = pending_persisted.and_then(|p| p.accuracy_deg);

        HeadingFusionSensor {
            gnss_frame,
            bno_frame,
            confidence: HeadingConfidence::Unknown,
            correction_offset_deg: None,
            display_offset_deg: None,
            game_rv_deg: None,
            last_correction_instant: None,
            accuracy_deg: None,
            course_agreement: SustainedAgreement::new(AGREEMENT_WINDOW, AGREEMENT_MIN_SAMPLES, AGREEMENT_TOLERANCE_DEG),
            heading_agreement: SustainedAgreement::new(AGREEMENT_WINDOW, AGREEMENT_MIN_SAMPLES, AGREEMENT_TOLERANCE_DEG),
            game_rv_settle: SustainedAgreement::new(GAME_RV_SETTLE_WINDOW, GAME_RV_SETTLE_MIN_SAMPLES, GAME_RV_SETTLE_TOLERANCE_DEG),
            pending_persisted_deg,
            pending_persisted_accuracy_deg,
            persist_path,
            last_persisted_deg: None,
            last_tick_instant: Instant::now(),
            heading_constraints: ValueConstraints::analog(0.0, 359.9),
            heading_metadata: ValueMetadata::new("\u{00b0}", "КУРС", "heading_fused"),
            confidence_metadata: ValueMetadata::new("", "СТАТУС КУРС", "heading_confidence"),
            accuracy_constraints: ValueConstraints::analog(0.0, MAX_DISPLAYED_ACCURACY_DEG),
            accuracy_metadata: ValueMetadata::new("\u{00b0}", "ТОЧН КУРС", "heading_accuracy"),
            dead_reckoning_elapsed_constraints: ValueConstraints::analog(0.0, f32::MAX),
            dead_reckoning_elapsed_metadata: ValueMetadata::new("с", "ВРЕМЯ СЧИСЛЕНИЯ КУРС", "dead_reckoning_elapsed"),
        }
    }

    fn default_persist_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        PathBuf::from(format!("{home}/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/State/heading.json"))
    }

    fn load_persisted(path: &std::path::Path) -> Option<PersistedHeading> {
        let contents = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&contents).ok()
    }

    /// Writes `heading_deg`/`accuracy_deg` to disk unless the heading is within
    /// `PERSIST_MIN_DELTA_DEG` of what's already there. Called on a manual correction and on
    /// drop (see `Drop` impl below) -- deliberately not on any timer or motion-state change.
    fn persist_if_changed(&mut self, heading_deg: f32, accuracy_deg: Option<f32>) {
        if let Some(last) = self.last_persisted_deg {
            if circular_diff_deg(heading_deg, last) < PERSIST_MIN_DELTA_DEG {
                return;
            }
        }

        let timestamp_unix_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let persisted = PersistedHeading { heading_deg, accuracy_deg, timestamp_unix_secs };
        let Ok(json) = serde_json::to_string(&persisted) else { return };
        if let Some(parent) = self.persist_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&self.persist_path, json) {
            Ok(()) => self.last_persisted_deg = Some(heading_deg),
            Err(e) => log::warn!("Heading fusion: failed to persist heading: {}", e),
        }
    }

    /// Directly sets the anchor from user input, equivalent to a validated GNSS correction
    /// for tracking purposes. No-op if the BNO085 hasn't produced a Game RV reading yet --
    /// there's no baseline to anchor from.
    pub fn set_manual_heading(&mut self, heading_deg: f32) {
        let Some(raw) = self.game_rv_deg else {
            log::warn!("Heading fusion: ignoring manual heading, no Game RV baseline yet");
            return;
        };
        let heading_deg = heading_deg.rem_euclid(360.0);
        let offset = heading_deg - raw;
        self.correction_offset_deg = Some(offset);
        // Bypass the display filter -- a manual correction is a deliberate, known-correct
        // user action, not something that should visibly ramp in like a GNSS re-anchor.
        self.display_offset_deg = Some(offset);
        self.confidence = HeadingConfidence::Manual;
        self.accuracy_deg = Some(MANUAL_ANCHOR_ACCURACY_DEG);
        self.last_correction_instant = Some(Instant::now());
        // Deliberate user action, not a hot path -- persist right away rather than waiting
        // for shutdown, so it survives an unclean exit before the next one.
        self.persist_if_changed(heading_deg, self.accuracy_deg);
    }

    /// How long the current DeadReckoning stretch has lasted, for UI display (e.g. dimming a
    /// needle further the longer it's been uncorrected). None outside DeadReckoning.
    pub fn dead_reckoning_elapsed(&self) -> Option<Duration> {
        if self.confidence != HeadingConfidence::DeadReckoning {
            return None;
        }
        self.last_correction_instant.map(|t| t.elapsed())
    }

    /// Advances the fusion state machine by one tick and returns the heading/confidence/
    /// accuracy outputs ready for SensorManager::set_external_value. Must be called at a
    /// roughly steady rate -- the slew rate, sustained-agreement window, and accuracy drift
    /// are all time-based.
    pub fn tick(&mut self) -> HeadingFusionOutput {
        self.tick_at(Instant::now())
    }

    /// `tick()`'s actual implementation, parametrized on "now" so tests can drive the
    /// slew/agreement timing deterministically instead of racing real wall-clock ticks.
    fn tick_at(&mut self, now: Instant) -> HeadingFusionOutput {
        let dt = now.saturating_duration_since(self.last_tick_instant);
        self.last_tick_instant = now;

        let game_rv_before = self.game_rv_deg;
        let validated_rv = self.integrate_game_rv();
        let settled_game_rv = self.settled_game_rv(now);
        self.apply_persisted_prior_if_ready(settled_game_rv);

        let validated_gnss = self.validated_gnss_correction(now);
        self.apply_correction(&validated_gnss, now);
        self.advance_display_offset(dt);

        if self.confidence == HeadingConfidence::GnssCorrected && validated_gnss.is_none() {
            // Downgrade confidence if we lose GNSS correction.
            self.confidence = HeadingConfidence::DeadReckoning;
        } else if self.confidence == HeadingConfidence::PersistedPrior && validated_rv.is_some() {
            // Upgrade confidence from PersistedPrior to DeadReckoning if we have a new RV reading.
            self.confidence = HeadingConfidence::DeadReckoning;
            self.last_correction_instant = Some(Instant::now());
        }

        if validated_gnss.is_none() {
            self.degrade_accuracy(game_rv_before, dt);
        }

        self.output()
    }

    /// Records this tick's raw Game RV reading. Runs unconditionally, independent of GNSS --
    /// this is the continuously-running backbone. `displayed_heading()`/`advance_display_offset`
    /// combine it with the correction offsets at read/slew time, so no rotation math happens
    /// here.
    fn integrate_game_rv(&mut self) -> Option<f32> {
        // Not bno_frame.is_stale() -- that's bumped by *any* of the BNO085's report types, so
        // it can read "fresh" before Game RV specifically has ever reported (see
        // Bno085Frame::game_orientation_last_update's doc comment). Using it here let a
        // phantom Default::default() game_orientation (heading_deg: 0.0) get treated as a real
        // reading.
        if self.bno_frame.game_orientation_is_stale() {
            return None;
        }

        self.game_rv_deg = Some(self.bno_frame.game_orientation().heading_deg);
        self.game_rv_deg
    }

    /// The heading actually exposed via `output`/persistence: `display_offset_deg` (which
    /// lags `correction_offset_deg` during a slew) combined with the current raw Game RV
    /// reading.
    fn displayed_heading(&self) -> Option<f32> {
        match (self.display_offset_deg, self.game_rv_deg) {
            (Some(offset), Some(raw)) => Some((offset + raw).rem_euclid(360.0)),
            _ => None,
        }
    }

    /// Feeds this tick's raw Game RV reading into `game_rv_settle` and returns it back once
    /// it's held steady for GAME_RV_SETTLE_WINDOW -- see that constant's doc comment. None
    /// while the reading is missing (BNO085 stale) or still settling; resets the tracker on a
    /// dropout so a later reconnect (which may itself carry a fresh, unsettled reference) has
    /// to settle again rather than trusting samples from before the gap.
    fn settled_game_rv(&mut self, now: Instant) -> Option<f32> {
        let Some(raw) = self.game_rv_deg else {
            self.game_rv_settle.reset();
            return None;
        };
        self.game_rv_settle.push(now, raw)
    }

    /// Folds a loaded-from-disk prior into `correction_offset_deg` and `accuracy_deg` the first
    /// tick a *settled* Game RV baseline becomes available -- see `settled_game_rv`/
    /// GAME_RV_SETTLE_WINDOW for why this waits rather than using the first raw reading.
    /// Deferred from `new()` because the BNO085 may not have reported (or settled) yet at
    /// construction time. `accuracy_deg` always becomes `Some` here, defaulting to 0.0 when
    /// nothing was persisted alongside the heading (a file predating that field, or written
    /// before the sensor was ever anchored) -- leaving it `None` instead would put a
    /// `PersistedPrior` anchor right back into the "nothing to degrade from" gap this exists to
    /// close, silently hiding INS drift until the next GNSS re-anchor.
    fn apply_persisted_prior_if_ready(&mut self, settled_game_rv: Option<f32>) {
        if self.correction_offset_deg.is_some() {
            return;
        }
        let Some(persisted_deg) = self.pending_persisted_deg else { return };
        let Some(raw) = settled_game_rv else { return };
        self.correction_offset_deg = Some(persisted_deg - raw);
        self.confidence = HeadingConfidence::PersistedPrior;
        self.accuracy_deg = Some(self.pending_persisted_accuracy_deg.unwrap_or(0.0));
        self.pending_persisted_deg = None;
        self.pending_persisted_accuracy_deg = None;
    }

    /// Reads one atomic GNSS fix snapshot and runs both sources through their own
    /// eligibility gate plus the shared sustained-agreement check. Prefers course_deg (the
    /// moving-car case) over heading_deg (the stationary case) when both happen to validate
    /// at once.
    fn validated_gnss_correction(&mut self, now: Instant) -> Option<GnssAnchor> {
        let fix = self.gnss_frame.fix();

        let course_eligible = fix.course_deg
            .filter(|_| fix.speed_kmh.unwrap_or(0.0) >= MIN_SPEED_FOR_COURSE_KMH);
        let validated_course = match course_eligible {
            Some(c) => self.course_agreement.push(now, c).map(|heading_deg| GnssAnchor {
                heading_deg,
                accuracy_deg: COURSE_ANCHOR_ACCURACY_DEG,
            }),
            None => { self.course_agreement.reset(); None }
        };

        let heading_eligible = fix.heading_deg
            .filter(|_| fix.heading_std_dev_deg.unwrap_or(f32::MAX) <= HEADING_STD_DEV_MAX_DEG);
        let validated_heading = match heading_eligible {
            Some(h) => self.heading_agreement.push(now, h).map(|heading_deg| GnssAnchor {
                heading_deg,
                // heading_eligible only passes above when heading_std_dev_deg itself was
                // Some(x) <= HEADING_STD_DEV_MAX_DEG (the None case unwraps to f32::MAX,
                // which always fails that filter) -- so this unwrap_or fallback is dead in
                // practice, kept only as a defensive default.
                accuracy_deg: fix.heading_std_dev_deg.unwrap_or(HEADING_STD_DEV_MAX_DEG),
            }),
            None => { self.heading_agreement.reset(); None }
        };

        validated_course.or(validated_heading)
    }

    /// Applies a validated correction, if any, instantly -- `correction_offset_deg`,
    /// `confidence`, and `accuracy_deg` always reflect the true current correction state with
    /// no convergence delay. `advance_display_offset` is what makes this look smooth on
    /// screen. `accuracy_deg` is reset outright to the anchor's own accuracy, not just kept
    /// from degrading further.
    fn apply_correction(&mut self, target: &Option<GnssAnchor>, now: Instant) {
        let Some(target) = target else { return };
        let Some(raw) = self.game_rv_deg else { return }; // no Game RV baseline yet -- wait for BNO085

        self.correction_offset_deg = Some(target.heading_deg - raw);
        self.confidence = HeadingConfidence::GnssCorrected;
        self.accuracy_deg = Some(target.accuracy_deg);
        self.last_correction_instant = Some(now);
    }

    /// Degrades `accuracy_deg` by the BNO085's pure-inertial drift rate for ticks spent
    /// dead-reckoning (no validated GNSS correction this tick), gated on the raw Game RV
    /// reading having actually moved since the last tick -- see `HEADING_STATIONARY_EPSILON_DEG`.
    /// Runs the same way whether the current anchor is `GnssCorrected` or a `PersistedPrior` --
    /// a None `accuracy_deg` (only possible pre-anchor, still `Unknown`) has nothing to degrade
    /// from and is left alone.
    fn degrade_accuracy(&mut self, game_rv_before: Option<f32>, dt: Duration) {
        let Some(accuracy_deg) = self.accuracy_deg else { return };
        let (Some(before), Some(after)) = (game_rv_before, self.game_rv_deg) else { return };
        if circular_diff_deg(before, after) < HEADING_STATIONARY_EPSILON_DEG {
            return;
        }
        self.accuracy_deg = Some(accuracy_deg + INERTIAL_DRIFT_DEG_PER_MIN * dt.as_secs_f32() / 60.0);
    }

    /// Ramps `display_offset_deg` one slew-rate-limited step toward `correction_offset_deg`,
    /// snapping instead if there's nothing to ramp from yet (the very first anchor -- nothing
    /// to glitch). Runs every tick regardless of whether a correction just landed, so a
    /// still-converging display keeps catching up smoothly. Operates on the offset rather than
    /// on `displayed_heading()` directly so real Game RV rotation -- applied to `game_rv_deg`
    /// unfiltered every tick -- is never itself rate-limited, only the correction jump is.
    fn advance_display_offset(&mut self, dt: Duration) {
        let Some(target_offset) = self.correction_offset_deg else { return };

        self.display_offset_deg = Some(match self.display_offset_deg {
            None => target_offset,
            Some(current_offset) => {
                let error = circular_signed_diff_deg(target_offset, current_offset);
                let max_step = SLEW_RATE_DEG_PER_SEC * dt.as_secs_f32();
                current_offset + error.clamp(-max_step, max_step)
            }
        });
    }

    fn output(&self) -> HeadingFusionOutput {
        let heading = match self.displayed_heading() {
            Some(heading_deg) => SensorValue::analog_with_constraints_and_metadata(
                heading_deg.clamp(self.heading_constraints.min_value, self.heading_constraints.max_value),
                self.heading_constraints.clone(),
                self.heading_metadata.clone(),
            ),
            None => SensorValue::new(ValueData::Empty, self.heading_constraints.clone(), self.heading_metadata.clone()),
        };

        let confidence = SensorValue::analog_with_constraints_and_metadata(
            self.confidence.code() as f32,
            ValueConstraints::analog(0.0, 4.0),
            self.confidence_metadata.clone(),
        );

        let accuracy = match self.accuracy_deg {
            Some(accuracy_deg) => SensorValue::analog_with_constraints_and_metadata(
                accuracy_deg.clamp(self.accuracy_constraints.min_value, self.accuracy_constraints.max_value),
                self.accuracy_constraints.clone(),
                self.accuracy_metadata.clone(),
            ),
            None => SensorValue::new(ValueData::Empty, self.accuracy_constraints.clone(), self.accuracy_metadata.clone()),
        };

        let dead_reckoning_elapsed = match self.dead_reckoning_elapsed() {
            Some(elapsed) => SensorValue::analog_with_constraints_and_metadata(
                elapsed.as_secs_f32(),
                self.dead_reckoning_elapsed_constraints.clone(),
                self.dead_reckoning_elapsed_metadata.clone(),
            ),
            None => SensorValue::new(ValueData::Empty, self.dead_reckoning_elapsed_constraints.clone(), self.dead_reckoning_elapsed_metadata.clone()),
        };

        HeadingFusionOutput { heading, confidence, accuracy, dead_reckoning_elapsed }
    }
}

/// Persists the last known heading on shutdown. Relies on normal Rust unwind-on-panic (no
/// `panic = "abort"` override in Cargo.toml) plus every process exit path in main.rs/
/// page_manager.rs returning normally rather than calling `std::process::exit` directly, so
/// this fires on a clean SIGTERM/SIGINT shutdown, a binary-update or SIGUSR1 restart, and a
/// panic unwind alike -- one hook instead of one persist call per exit path. Does not cover a
/// SIGKILL or power loss; that gap is accepted rather than worked around with periodic writes.
impl Drop for HeadingFusionSensor {
    fn drop(&mut self) {
        if let Some(heading_deg) = self.displayed_heading() {
            self.persist_if_changed(heading_deg, self.accuracy_deg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn new_sensor() -> (HeadingFusionSensor, GnssFrame, Bno085Frame) {
        let gnss_frame = GnssFrame::for_test();
        let bno_frame = Bno085Frame::for_test();
        let mut sensor = HeadingFusionSensor::new(gnss_frame.clone(), bno_frame.clone());
        // HeadingFusionSensor::new() just read the real default persist path -- redirect to a
        // harmless per-test temp path and discard whatever (if anything) it loaded, so tests
        // never depend on, or pollute, real persisted state from an actual dashboard run.
        let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        sensor.persist_path = std::env::temp_dir().join(format!("niva_heading_test_{}_{}.json", std::process::id(), test_id));
        sensor.pending_persisted_deg = None;
        (sensor, gnss_frame, bno_frame)
    }

    fn fix_with(course_deg: Option<f32>, heading_deg: Option<f32>, heading_std_dev_deg: Option<f32>, speed_kmh: Option<f32>) -> crate::util::nmea::GnssFix {
        crate::util::nmea::GnssFix {
            course_deg,
            heading_deg,
            heading_std_dev_deg,
            speed_kmh,
            ..Default::default()
        }
    }

    #[test]
    fn test_unknown_until_first_anchor() {
        let (mut sensor, _gnss, _bno) = new_sensor();
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick();
        assert_eq!(heading.value, ValueData::Empty, "no anchor yet -- no heading to display");
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::Unknown.code());
    }

    #[test]
    fn test_waits_for_game_rv_baseline_before_anchoring() {
        // Game RV never reported (bno frame stays stale) -- even a rock-solid validated GNSS
        // fix can't establish an anchor without a baseline to track deltas from.
        let (mut sensor, gnss, _bno) = new_sensor();
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            let heading = sensor.tick().heading;
            assert_eq!(heading.value, ValueData::Empty);
        }
    }

    #[test]
    fn test_first_validated_heading_snaps_no_slew() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);

        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick();
        assert!((heading.as_f32() - 90.0).abs() < 0.5, "expected ~90 deg, got {}", heading.as_f32());
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::GnssCorrected.code());
    }

    #[test]
    fn test_single_disagreeing_reading_does_not_validate() {
        // Mirrors the window-sill GNSS failure mode: a lone confidently-wrong sample must
        // not be enough on its own.
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);

        gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
        let heading = sensor.tick().heading;
        assert_eq!(heading.value, ValueData::Empty, "one sample must not validate on its own");
    }

    #[test]
    fn test_disagreeing_samples_never_validate() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);

        let mut heading_val = 90.0;
        for _ in 0..(AGREEMENT_MIN_SAMPLES + 3) {
            gnss.set_fix_for_test(fix_with(None, Some(heading_val), Some(0.5), None));
            sensor.tick();
            heading_val += 45.0; // wildly inconsistent between samples
        }
        let heading = sensor.tick().heading;
        assert_eq!(heading.value, ValueData::Empty, "disagreeing samples must never validate");
    }

    #[test]
    fn test_course_deg_gated_by_min_speed() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);

        for _ in 0..(AGREEMENT_MIN_SAMPLES + 2) {
            gnss.set_fix_for_test(fix_with(Some(180.0), None, None, Some(0.5))); // below MIN_SPEED_FOR_COURSE_KMH
            sensor.tick();
        }
        let heading = sensor.tick().heading;
        assert_eq!(heading.value, ValueData::Empty, "course_deg below min speed must never validate");
    }

    #[test]
    fn test_heading_deg_gated_by_std_dev() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);

        for _ in 0..(AGREEMENT_MIN_SAMPLES + 2) {
            gnss.set_fix_for_test(fix_with(None, Some(270.0), Some(50.0), None)); // above HEADING_STD_DEV_MAX_DEG
            sensor.tick();
        }
        let heading = sensor.tick().heading;
        assert_eq!(heading.value, ValueData::Empty, "heading_deg with a wide std-dev must never validate");
    }

    #[test]
    fn test_game_rv_dead_reckons_between_gnss_corrections() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        sensor.tick(); // settles the anchor at ~90

        // GNSS goes quiet; the car turns 30 degrees per the gyro alone.
        gnss.set_fix_for_test(fix_with(None, None, None, None));
        bno.set_game_heading_for_test(30.0);
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick();
        assert!((heading.as_f32() - 120.0).abs() < 0.5, "expected ~120 deg, got {}", heading.as_f32());
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());
    }

    #[test]
    fn test_large_correction_slews_rather_than_snaps() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(0.0), Some(0.5), None));
            sensor.tick();
        }
        sensor.tick(); // anchor settled at ~0

        // Drive ticks on a fabricated clock, past AGREEMENT_WINDOW, so the earlier
        // heading_deg=0 samples age out of the tracker before a big, sustained correction to
        // 90 degrees starts -- isolates the slew-rate behavior under test from the
        // sustained-agreement warm-up transient (covered separately by the "disagreeing
        // samples" tests).
        let mut t = Instant::now() + AGREEMENT_WINDOW + Duration::from_secs(1);
        let step_dt = Duration::from_millis(500); // 15 deg/tick at SLEW_RATE_DEG_PER_SEC=30
        let mut last_heading = 0.0;
        for _ in 0..(AGREEMENT_MIN_SAMPLES + 2) {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            t += step_dt;
            let heading = sensor.tick_at(t).heading;
            last_heading = heading.as_f32();
        }
        assert!(last_heading > 0.5 && last_heading < 89.0,
                "expected a partial ramp, not an instant snap, got {}", last_heading);
    }

    #[test]
    fn test_persisted_prior_switched_to_dead_reckoning() {
        let dir = std::env::temp_dir().join(format!("niva_heading_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heading.json");
        std::fs::write(&path, r#"{"heading_deg":123.0,"timestamp_unix_secs":0}"#).unwrap();

        let gnss_frame = GnssFrame::for_test();
        let bno_frame = Bno085Frame::for_test();
        let mut sensor = HeadingFusionSensor::new(gnss_frame, bno_frame.clone());
        sensor.persist_path = path.clone();
        let persisted = HeadingFusionSensor::load_persisted(&path);
        sensor.pending_persisted_deg = persisted.as_ref().map(|p| p.heading_deg);
        sensor.pending_persisted_accuracy_deg = persisted.and_then(|p| p.accuracy_deg);

        bno_frame.set_game_heading_for_test(123.0);
        // Game RV must hold steady for GAME_RV_SETTLE_MIN_SAMPLES ticks before the persisted
        // prior is trusted as a baseline -- see GAME_RV_SETTLE_WINDOW.
        for _ in 0..(GAME_RV_SETTLE_MIN_SAMPLES - 1) {
            sensor.tick();
        }
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick();
        assert!((heading.as_f32() - 123.0).abs() < 0.5);
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_persisted_prior_ignores_unsettled_first_game_rv_reading() {
        // Reproduces a real bug: rotate the board so the running session reads heading=100,
        // restart the program with the board physically untouched -- the persisted prior
        // should still read ~100 after restart, not whatever the BNO085's first, not-yet-
        // converged Game RV report happened to be.
        let dir = std::env::temp_dir().join(format!("niva_heading_test_settle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heading.json");
        std::fs::write(&path, r#"{"heading_deg":100.0,"timestamp_unix_secs":0}"#).unwrap();

        let gnss_frame = GnssFrame::for_test();
        let bno_frame = Bno085Frame::for_test();
        let mut sensor = HeadingFusionSensor::new(gnss_frame, bno_frame.clone());
        sensor.persist_path = path.clone();
        let persisted = HeadingFusionSensor::load_persisted(&path);
        sensor.pending_persisted_deg = persisted.as_ref().map(|p| p.heading_deg);
        sensor.pending_persisted_accuracy_deg = persisted.and_then(|p| p.accuracy_deg);

        let mut t = Instant::now();

        // First report after "reconnect" is a not-yet-converged transient, wildly different
        // from the board's true (unmoved) orientation.
        bno_frame.set_game_heading_for_test(0.0);
        assert_eq!(sensor.tick_at(t).heading.value, ValueData::Empty,
                   "must not anchor off a single unsettled reading");

        // It settles to its true, steady value -- advance the clock past GAME_RV_SETTLE_WINDOW
        // first so the stale transient sample ages out of the tracker instead of polluting the
        // settle check.
        t += GAME_RV_SETTLE_WINDOW + Duration::from_millis(1);
        bno_frame.set_game_heading_for_test(355.0);
        for _ in 0..(GAME_RV_SETTLE_MIN_SAMPLES - 1) {
            t += Duration::from_millis(20);
            sensor.tick_at(t);
        }
        t += Duration::from_millis(20);
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick_at(t);
        assert!((heading.as_f32() - 100.0).abs() < 0.5,
                 "expected persisted 100 deg anchored off the settled reading, got {}", heading.as_f32());
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_manual_heading_sets_manual_confidence() {
        let (mut sensor, _gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        sensor.tick();

        sensor.set_manual_heading(200.0);
        let HeadingFusionOutput { heading, confidence, .. } = sensor.tick();
        assert!((heading.as_f32() - 200.0).abs() < 0.5);
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::Manual.code());
    }

    #[test]
    fn test_manual_heading_persists_immediately() {
        let (mut sensor, _gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        sensor.tick();

        sensor.set_manual_heading(200.0);
        let persisted = HeadingFusionSensor::load_persisted(&sensor.persist_path)
            .expect("manual correction should be persisted right away, not deferred to shutdown");
        assert!((persisted.heading_deg - 200.0).abs() < 0.01);
        assert_eq!(persisted.accuracy_deg, Some(MANUAL_ANCHOR_ACCURACY_DEG),
                   "manual anchor's accuracy should be persisted alongside its heading");

        let _ = std::fs::remove_file(&sensor.persist_path);
    }

    #[test]
    fn test_drop_persists_tracked_heading() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        sensor.tick(); // anchor settles at ~90

        let path = sensor.persist_path.clone();
        assert_eq!(HeadingFusionSensor::load_persisted(&path), None,
                   "no persist trigger (manual correction or drop) has fired yet");

        drop(sensor);
        let persisted = HeadingFusionSensor::load_persisted(&path)
            .expect("drop should persist the tracked heading");
        assert!((persisted.heading_deg - 90.0).abs() < 0.5, "expected ~90 deg, got {}", persisted.heading_deg);
        assert_eq!(persisted.accuracy_deg, Some(0.5),
                   "the GNSS anchor's accuracy (the fix's heading_std_dev_deg) should be persisted alongside its heading");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_drop_without_anchor_does_not_persist() {
        let (sensor, _gnss, _bno) = new_sensor();
        let path = sensor.persist_path.clone();
        drop(sensor);
        assert_eq!(HeadingFusionSensor::load_persisted(&path), None,
                   "nothing to persist -- no anchor was ever established");
    }

    #[test]
    fn test_accuracy_unknown_before_first_anchor() {
        let (mut sensor, _gnss, _bno) = new_sensor();
        let output = sensor.tick();
        assert_eq!(output.accuracy.value, ValueData::Empty);
    }

    #[test]
    fn test_accuracy_defaults_to_zero_when_persisted_file_predates_accuracy_field() {
        // A file written before accuracy_deg existed on disk (or written when nothing had ever
        // anchored the sensor) has no accuracy on it -- rather than resuming with no accuracy to
        // degrade from (the reported gap), the sensor should start counting drift from 0.0.
        let dir = std::env::temp_dir().join(format!("niva_heading_test_no_accuracy_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heading.json");
        std::fs::write(&path, r#"{"heading_deg":123.0,"timestamp_unix_secs":0}"#).unwrap();

        let gnss_frame = GnssFrame::for_test();
        let bno_frame = Bno085Frame::for_test();
        let mut sensor = HeadingFusionSensor::new(gnss_frame, bno_frame.clone());
        sensor.persist_path = path.clone();
        let persisted = HeadingFusionSensor::load_persisted(&path);
        sensor.pending_persisted_deg = persisted.as_ref().map(|p| p.heading_deg);
        sensor.pending_persisted_accuracy_deg = persisted.and_then(|p| p.accuracy_deg);

        bno_frame.set_game_heading_for_test(123.0);
        for _ in 0..GAME_RV_SETTLE_MIN_SAMPLES {
            sensor.tick();
        }
        let output = sensor.tick();
        assert_eq!(output.confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());
        assert!((output.accuracy.as_f32() - 0.0).abs() < 0.01,
                 "no persisted accuracy on disk should default to 0.0, not stay unknown, got {}",
                 output.accuracy.as_f32());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_persisted_prior_resumes_degrading_from_its_persisted_accuracy() {
        // Reproduces the reported gap: a PersistedPrior anchor used to sit at accuracy_deg ==
        // None for its whole stretch (nothing was ever persisted to resume from), so no amount
        // of dead-reckoning drift ever showed up until the next GNSS re-anchor. It should
        // instead resume degrading from whatever accuracy was persisted alongside the heading.
        let dir = std::env::temp_dir().join(format!("niva_heading_test_resume_accuracy_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("heading.json");
        std::fs::write(&path, r#"{"heading_deg":123.0,"accuracy_deg":2.0,"timestamp_unix_secs":0}"#).unwrap();

        let gnss_frame = GnssFrame::for_test();
        let bno_frame = Bno085Frame::for_test();
        let mut sensor = HeadingFusionSensor::new(gnss_frame, bno_frame.clone());
        sensor.persist_path = path.clone();
        let persisted = HeadingFusionSensor::load_persisted(&path);
        sensor.pending_persisted_deg = persisted.as_ref().map(|p| p.heading_deg);
        sensor.pending_persisted_accuracy_deg = persisted.and_then(|p| p.accuracy_deg);

        bno_frame.set_game_heading_for_test(123.0);
        let mut t = Instant::now();
        for _ in 0..GAME_RV_SETTLE_MIN_SAMPLES {
            t += Duration::from_millis(20);
            sensor.tick_at(t);
        }
        let anchored = sensor.tick_at(t);
        assert_eq!(anchored.confidence.as_f32() as i32, HeadingConfidence::PersistedPrior.code());
        assert!((anchored.accuracy.as_f32() - 2.0).abs() < 0.01,
                 "expected the persisted accuracy (2.0) to be applied on load, got {}", anchored.accuracy.as_f32());

        // Board rotates for a full minute with no GNSS -- should cost one INERTIAL_DRIFT_DEG_PER_MIN,
        // same as it would from a GnssCorrected anchor.
        bno_frame.set_game_heading_for_test(133.0);
        t += Duration::from_secs(60);
        let output = sensor.tick_at(t);
        assert_eq!(output.confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());
        let expected = 2.0 + INERTIAL_DRIFT_DEG_PER_MIN;
        assert!((output.accuracy.as_f32() - expected).abs() < 0.05,
                 "expected accuracy ~{} deg after 1 min dead reckoning from a persisted prior, got {}",
                 expected, output.accuracy.as_f32());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_accuracy_resets_to_gnss_solution_accuracy_on_anchor() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        let HeadingFusionOutput { accuracy, confidence, .. } = sensor.tick();
        assert_eq!(confidence.as_f32() as i32, HeadingConfidence::GnssCorrected.code());
        assert!((accuracy.as_f32() - 0.5).abs() < 0.01,
                 "expected accuracy to take on the fix's heading_std_dev_deg (0.5), got {}", accuracy.as_f32());
    }

    #[test]
    fn test_accuracy_degrades_while_dead_reckoning_and_heading_changes() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        let accuracy_at_anchor = sensor.tick().accuracy.as_f32(); // ~0.5, anchor settled

        // GNSS goes quiet and the car turns per the gyro alone -- a full minute of pure
        // inertial operation should cost exactly one INERTIAL_DRIFT_DEG_PER_MIN.
        gnss.set_fix_for_test(fix_with(None, None, None, None));
        bno.set_game_heading_for_test(10.0);
        let t = Instant::now() + Duration::from_secs(60);
        let output = sensor.tick_at(t);
        assert_eq!(output.confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());
        let expected = accuracy_at_anchor + INERTIAL_DRIFT_DEG_PER_MIN;
        assert!((output.accuracy.as_f32() - expected).abs() < 0.05,
                 "expected accuracy ~{} deg after 1 min dead reckoning while rotating, got {} (was {} at anchor)",
                 expected, output.accuracy.as_f32(), accuracy_at_anchor);
    }

    #[test]
    fn test_accuracy_does_not_degrade_while_dead_reckoning_stationary() {
        let (mut sensor, gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        for _ in 0..AGREEMENT_MIN_SAMPLES {
            gnss.set_fix_for_test(fix_with(None, Some(90.0), Some(0.5), None));
            sensor.tick();
        }
        let accuracy_at_anchor = sensor.tick().accuracy.as_f32();

        // GNSS goes quiet but the raw Game RV reading doesn't move at all -- the BNO085
        // empirically re-calibrates itself with no observed drift while fully stationary, so
        // no time-based degradation should be charged no matter how long this stretch is.
        gnss.set_fix_for_test(fix_with(None, None, None, None));
        let t = Instant::now() + Duration::from_secs(600);
        let output = sensor.tick_at(t);
        assert_eq!(output.confidence.as_f32() as i32, HeadingConfidence::DeadReckoning.code());
        assert!((output.accuracy.as_f32() - accuracy_at_anchor).abs() < 0.01,
                 "accuracy should not degrade while stationary, was {} now {}",
                 accuracy_at_anchor, output.accuracy.as_f32());
    }

    #[test]
    fn test_manual_heading_sets_accuracy_zero() {
        let (mut sensor, _gnss, bno) = new_sensor();
        bno.set_game_heading_for_test(0.0);
        sensor.tick();

        sensor.set_manual_heading(200.0);
        let accuracy = sensor.tick().accuracy;
        assert!((accuracy.as_f32() - 0.0).abs() < 0.01,
                 "manual input is treated as a perfect reference, got {}", accuracy.as_f32());
    }
}
