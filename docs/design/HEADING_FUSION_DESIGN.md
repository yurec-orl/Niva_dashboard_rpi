# Heading Fusion Design — GNSS Course + BNO085 Game Rotation Vector

## Problem
Goal: a heading that stays within a few degrees of true, including while stationary and/or
GPS-obstructed. Per `niva_dashboard/src/util/COMPASS_SENSORS_TESTING.md`'s empirical results, no
single available source meets this alone:

| Source                                  | Verdict                                                                                                  |
|------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| GNSS `course_deg` (movement vector)       | Architecturally reliable while moving, absent while stationary/GPS-denied. Not exercised by testing to date. |
| GNSS `heading_deg` (dual-antenna)         | Originally undiagnosable at window-sill siting: 2/5 stationary trials converged fast and confidently to headings 82–122° off, with `heading_std_dev_deg` just as tight as in the correct trials. Relocating the antennas to open siting (clear of nearby walls/corners) fixed this — 10/10 trials converged within seconds to 1° accuracy, and a later hand-rotation test showed `heading_std_dev_deg` correctly widening (into the tens/hundreds of degrees) during a bad transient rather than staying falsely tight. Usable as a correction source gated on `heading_std_dev_deg`, contingent on siting quality — see below. |
| BNO085 Rotation Vector / Geomagnetic RV   | Magnetometer-fused; 10–32°/leg error tracking a known 90° rotation at a relatively benign test location (window sill). Expected worse inside a steel car body. `Accuracy` enum actively misleading (flagged `High` on its worst leg, `Unreliable` on its best). |
| BNO085 Game Rotation Vector (gyro only)   | Most accurate relative tracker (0.85° mean / 1.3° max error over 90° legs), but no absolute reference — resets to an arbitrary ~0° at every power-on regardless of true orientation. Long-duration drift (>25 min) uncharacterized. |

## Design principle
Since no source is both absolute and trustworthy on a single reading, the fusion layer must:

1. Never treat a sensor's own confidence/accuracy signal as proof of correctness on its own — the
   BNO085 `Accuracy` enum failed this outright in testing (misleading in both directions), and
   GNSS `heading_std_dev_deg` failed it too at window-sill siting (falsely tight on wrong
   convergences) though it proved reliable at open siting (see `heading_deg` gate below). Still
   worth gating on sustained agreement rather than a single reading, since siting quality can't be
   guaranteed at every parking spot the car ends up at.
2. Gate every absolute correction behind an independent sustained-agreement check, not a single
   sample.
3. Keep one continuously-running relative tracker (Game RV) as the backbone at all times, so
   heading is never cold-started mid-drive.

## Core model
Game RV runs continuously from the first anchor obtained, for the life of the process:

```
tracked_heading = anchor_heading + (game_rv_now − game_rv_at_anchor)
```

"GPS available" vs. "GPS lost" changes only *which signal is allowed to update the anchor*, not
whether Game RV is integrating — there is no discrete "switch to INS mode" step, which would
otherwise leave a gap where the delta baseline is stale at the moment of the switch.

## Confidence tiers
A second, orthogonal piece of state — since nothing in the raw sensor data can carry it:

1. **Unknown** — no anchor obtained yet this boot. No heading to display.
2. **PersistedPrior** — anchor restored from the last shutdown's saved heading; not yet
   confirmed by a fresh validated GNSS fix (`course_deg` or `heading_deg`) this boot.
3. **DeadReckoning(elapsed)** — anchor was validated at some point, but neither `course_deg` nor
   `heading_deg` is currently available and passing its gate. `elapsed` = time since last
   correction.
4. **GnssCorrected** — a currently-agreeing, validated `course_deg` or `heading_deg` fix just
   updated the anchor.
5. **Manual** — anchor set by direct user input, treated as trusted (equivalent to
   `GnssCorrected` for tracking purposes, labeled separately for the UI).

## Accuracy estimate
A third, numeric piece of state — an estimated current heading error in degrees — orthogonal to
the confidence tier above (which says *how* the heading was last obtained, not *how wrong* it
might currently be).

- Reset outright (not blended toward) to the source's own accuracy on every anchor update:
  - `GnssCorrected` via `heading_deg` — the fix's own `heading_std_dev_deg`, already known tight
    enough to have passed the validation gate above.
  - `GnssCorrected` via `course_deg` — `course_deg` has no reported accuracy figure of its own
    (unlike `heading_deg`), so a fixed, provisional constant stands in (see Open decisions).
  - `Manual` — treated as a perfect reference, 0°, per the same "trusted equivalent to
    `GnssCorrected`" precedent as the confidence tier.
- Degrades while dead-reckoning (no validated correction lands on a given tick) at the BNO085
  datasheet's Game Rotation Vector drift rate — 0.5°/minute in pure-inertial operation — but only
  across a tick where the raw Game RV reading actually changed. Empirically the BNO085
  re-calibrates itself with no observed drift while genuinely stationary, so a car sitting
  through a GNSS outage doesn't accrue drift it never experienced.
- Left unset (no accuracy figure, same as `Unknown`) until the first anchor — including through
  `PersistedPrior`: only the heading and a timestamp are persisted, not an accuracy alongside it,
  so there's nothing to carry over from the previous run. Re-validating a persisted heading
  against live GNSS at boot is exactly what turns it into a known accuracy.

## Transitions

- **Boot**: if a persisted `(heading, timestamp)` exists, anchor := persisted heading,
  `game_rv_at_anchor` := first Game RV reading, confidence := `PersistedPrior`. Otherwise
  confidence := `Unknown`, no anchor, wait.
- **Manual input** (any time): anchor := input value, `game_rv_at_anchor` := current Game RV
  reading, confidence := `Manual`.
- **Validated GNSS fix** (`course_deg` while moving, or `heading_deg` while stationary — see gate
  below for each) arrives:
  - From `Unknown` / `PersistedPrior` / `DeadReckoning`: anchor/`game_rv_at_anchor` update and
    confidence := `GnssCorrected` immediately, with no wait for convergence -- confidence
    reflects whether a validated source is currently backing the anchor, not whether the
    on-screen display has finished catching up. The *displayed* heading is a separate,
    slew-limited chase of the anchor (see below) so the needle doesn't visibly jump.
  - While already `GnssCorrected`: keep re-anchoring on each new validated fix, so drift never
    gets the chance to accumulate while GNSS is good. A moving car re-anchors off `course_deg`;
    a car that stops re-anchors off `heading_deg` once it passes that source's own gate — the
    handoff between the two is implicit in which gate the current reading happens to pass, not a
    separate mode switch.
  - Both sources failing their gate simultaneously (no moving fix, no confident stationary
    fix) drops confidence to `DeadReckoning(0)`.
- **GNSS fix becomes unavailable, drops below the minimum-speed gate (`course_deg`), fails its
  std-dev threshold (`heading_deg`), or fails the sustained-agreement check**: confidence steps
  down from `GnssCorrected` to `DeadReckoning(0)`. Anchor and `game_rv_at_anchor` are unchanged —
  only the confidence label changes, and `elapsed` starts accumulating.
- **Shutdown, or periodic tick**: persist `(tracked_heading, timestamp)`.

## Validation gate (shared by every anchor update, initial or correcting)
- Minimum speed threshold on `course_deg` — GNSS course-over-ground is known to be dominated by
  position noise at low speed; a numeric threshold needs a dedicated moving trial (this doc's
  source testing only ever exercised stationary `heading_deg`, never `course_deg`).
- Sustained agreement — N consecutive samples (or T seconds) agreeing within tolerance, not a
  single reading. This directly targets the "confidently wrong" failure mode seen twice in the
  `heading_deg` stationary trials; the same discipline is applied to `course_deg` even though it
  hasn't yet been observed to fail the same way, since it hasn't been tested at all.
- `heading_deg` correction gate — additionally requires `heading_std_dev_deg` below a threshold
  (TBD, see Open decisions) before a reading is even eligible for the sustained-agreement check.
  This is the reported-accuracy gate that testing showed is meaningful at open antenna siting
  (`heading_std_dev_deg` widened into the tens/hundreds of degrees during a bad hand-rotation
  transient rather than staying falsely tight) — but the window-sill trials showed the same field
  can be falsely tight under bad siting, so sustained agreement is kept as a second, independent
  check rather than trusting the std-dev threshold alone. No minimum-speed requirement, unlike
  `course_deg` — `heading_deg` is valid while stationary, which is exactly the condition
  `course_deg` can't cover.

## Correction dynamics
The anchor itself updates instantly on a validated fix (see Transitions above) — but the
*displayed* heading is a separate value that slew-rate-limits its convergence toward the anchor,
not an instant snap, since an instant jump would read as a glitch on a needle-style heading
indicator. Real Game RV rotation is never subject to this limit, only the discontinuity a
correction introduces. The rate needs tuning against plausible real turn rates so a genuine fast
turn mid-ramp isn't misread as a slew artifact; no numeric rate is proposed here.

## Persistence
- Written on two triggers only: a manual heading correction (immediately — it's a deliberate,
  infrequent user action, not a hot path), and process shutdown (via `Drop for
  HeadingFusionSensor`, which fires on a clean SIGTERM/SIGINT exit, a binary-update or SIGUSR1
  restart, and a panic unwind alike). No periodic or motion-triggered writes: an earlier
  every-10s-while-anchored version put constant, mostly-redundant load on the SD card while
  driving without meaningfully improving crash coverage (a crash between writes could still
  lose up to a full interval of heading drift), and a proposed motion-stopped trigger was
  dropped as solving only the same narrow edge case (crash while stationary) that a periodic
  timer already covered.
- Consequently, a hard power loss, a SIGKILL (e.g. `earlyoom` configured to kill rather than
  send SIGTERM — see CLAUDE.md TODO, which deliberately doesn't protect the dashboard binary
  either way), or any other exit path that skips normal unwind loses whatever heading drift
  happened since the last manual correction. Accepted: the persisted value is only ever loaded
  as `PersistedPrior` and re-validated against live GNSS before being trusted (see below), so
  the cost of losing it is a slower re-anchor after restart, not a wrong reading.
- Loaded as `PersistedPrior`, never as `GnssCorrected` — it goes through the same validation gate
  as any other anchor before being trusted. This is what makes a car being started and turned
  before the dashboard finishes booting self-healing rather than a special case: Game RV
  dead-reckons through that gap on a possibly-wrong baseline, and the first validated GNSS fix
  (`course_deg` or `heading_deg`) corrects it via the normal slew-limited ramp, same as any other
  correction.
- The timestamp is for UI/diagnostics only (e.g. "last known heading, N hours old") — a stale
  persisted value is handled identically to a fresh one; age doesn't change the gating.

## UI-facing signal
Expose the confidence tier and the accuracy estimate (see above) alongside the numeric heading
(e.g. dimming or annotating a compass/HSI needle during `DeadReckoning`, scaled by `elapsed` or
by the accuracy figure), since neither underlying sensor's own accuracy field can be trusted as
that signal on its own.

## Deferred / explicitly excluded
- **BNO085 Rotation Vector / Geomagnetic RV**: excluded entirely — magnetometer-fused error was
  too large even at a relatively benign test location, and a car body is expected to be worse.
- **Long-duration Game RV drift**: uncharacterized past ~25 minutes. The `DeadReckoning(elapsed)`
  tier is meant to make an extended dead-reckoning stretch visible in the UI, not to solve
  unbounded drift — a long-duration trial is still needed before relying on this for e.g. an
  hour-long GPS-denied stretch.

## Relationship to `SENSOR_FUSION_CHAIN_DESIGN.md`
That doc's `HeadingFusionSensor` concrete use case sketched a simpler fallback policy ("trust
BNO085 when available, else GNSS heading"), built on a `FusedAnalogSensor`/
`SensorFusedAnalogInputChain` chain type (`Option<u16>`-per-provider in, one `SensorValue` out).
This design supersedes that policy, and outgrew that plumbing along with it: the
anchor/validate/correct/confidence logic above needs several concurrently-consistent fields
per source in one read (GNSS `course_deg`, `heading_deg`, `heading_std_dev_deg`, `speed_kmh` —
all from one atomic `GnssFrame::fix()` snapshot, not independently-read `u16` channels) and
produces multiple independent outputs (heading, confidence tier, accuracy estimate), none of
which the old chain type's shape supports. `HeadingFusionSensor` (`hardware/heading_fusion_sensor.rs`)
now holds `GnssFrame`/`Bno085Frame` directly instead — same precedent `hw_providers.rs` already
used for GNSS position/date-time ("composite values... read directly from GnssFrame... rather
than forced through the u16 HWAnalogProvider boundary") — and is ticked once per event-loop
iteration by `PageManager`, writing `HwHeading`, `HwHeadingConfidence`, and `HwHeadingAccuracy`
into `SensorManager` via `set_external_value` rather than through any chain. `SensorFusedAnalogInputChain`/
`FusedAnalogSensor` had no other consumer, so that scaffolding was deleted (see
SENSOR_FUSION_CHAIN_DESIGN.md's status note) rather than kept unused.

BNO085 connectivity itself no longer blocks on `I2CProvider`/the provider-trait stubs
mentioned in that doc — the BNO085 background thread (`util/bno085_data_provider.rs`) already
runs independently of the `HWAnalogProvider`/`HWDigitalProvider` chain framework, the same way
GNSS does, and `HeadingFusionSensor` reads its `Bno085Frame` directly.

## Open decisions / needs real-world numbers
- Minimum speed threshold and sustained-agreement window (sample count / duration / tolerance)
  for accepting a `course_deg` reading.
- `heading_std_dev_deg` threshold and sustained-agreement window for accepting a `heading_deg`
  reading. All supporting data so far (10/10 balcony convergences, informative std-dev widening
  in the hand-rotation retest) comes from one open-sky site; the window-sill failure mode was
  siting-dependent, so the threshold shouldn't be tuned tight enough to assume every parking spot
  the car sits in is equally open.
- Slew-rate limit for the correction ramp.
- Persistence write cadence.
- How confidence tier is rendered in the UI — display design, not fusion logic.
- Long-duration Game RV drift characterization.
- Assumed accuracy for a `course_deg`-sourced anchor (`course_deg` has no reported accuracy
  figure of its own, unlike `heading_deg`'s `heading_std_dev_deg`) — needs a real moving trial,
  same as the `course_deg` gate itself.
- The 0.5°/minute inertial drift rate used to degrade the accuracy estimate is the BNO085
  datasheet's spec figure, not yet checked against this project's own long-duration drift data
  (see the point above) — could be too optimistic or too conservative once that trial happens.
- How the accuracy estimate is rendered in the UI — display design, not fusion logic, same as
  the confidence tier.

## Status
Implemented in `hardware/heading_fusion_sensor.rs` (`HeadingFusionSensor`, ticked once per
event-loop iteration by `PageManager` — see "Relationship to SENSOR_FUSION_CHAIN_DESIGN.md"
above for the plumbing this ended up using instead of that doc's chain type). The mechanism
covers every transition in this doc: continuous Game RV integration, the course_deg/heading_deg
validation gates with sustained-agreement windows, slew-limited correction, the five confidence
tiers, the accuracy estimate (reset-on-anchor, drift-while-dead-reckoning), and disk persistence
(`HeadingFusionSensor::new`/`persist`).

Every numeric constant at the top of that file (`MIN_SPEED_FOR_COURSE_KMH`,
`HEADING_STD_DEV_MAX_DEG`, `AGREEMENT_WINDOW`/`AGREEMENT_MIN_SAMPLES`/`AGREEMENT_TOLERANCE_DEG`,
`SLEW_RATE_DEG_PER_SEC`, `PERSIST_MIN_DELTA_DEG`, `INERTIAL_DRIFT_DEG_PER_MIN`,
`COURSE_ANCHOR_ACCURACY_DEG`) is still a placeholder pending the real-world data listed in "Open
decisions" above — none of it has been validated against an actual moving trial or a
long-duration drift test yet. Also still open: how the confidence tier and accuracy estimate are
rendered in the UI — `HeadingFusionSensor` exposes them via `HwHeadingConfidence`/
`HwHeadingAccuracy` and `dead_reckoning_elapsed()`, but no indicator consumes any of the three
yet. `set_manual_heading` exists per the "Manual input" transition but has no UI entry point
wired to it yet either.

---
*Created: August 8, 2026*
