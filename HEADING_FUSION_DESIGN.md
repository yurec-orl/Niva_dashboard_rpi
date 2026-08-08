# Heading Fusion Design — GNSS Course + BNO085 Game Rotation Vector

## Problem
Goal: a heading that stays within a few degrees of true, including while stationary and/or
GPS-obstructed. Per `niva_dashboard/src/util/COMPASS_SENSORS_TESTING.md`'s empirical results, no
single available source meets this alone:

| Source                                  | Verdict                                                                                                  |
|------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| GNSS `course_deg` (movement vector)       | Architecturally reliable while moving, absent while stationary/GPS-denied. Not exercised by testing to date. |
| GNSS `heading_deg` (dual-antenna)         | Undiagnosable: 2/5 stationary trials converged fast and confidently to headings 82–122° off, with `heading_std_dev_deg` just as tight as in the correct trials. No self-reported signal distinguishes right from wrong. |
| BNO085 Rotation Vector / Geomagnetic RV   | Magnetometer-fused; 10–32°/leg error tracking a known 90° rotation at a relatively benign test location (window sill). Expected worse inside a steel car body. `Accuracy` enum actively misleading (flagged `High` on its worst leg, `Unreliable` on its best). |
| BNO085 Game Rotation Vector (gyro only)   | Most accurate relative tracker (0.85° mean / 1.3° max error over 90° legs), but no absolute reference — resets to an arbitrary ~0° at every power-on regardless of true orientation. Long-duration drift (>25 min) uncharacterized. |

## Design principle
Since no source is both absolute and trustworthy on a single reading, the fusion layer must:

1. Never treat a sensor's own confidence/accuracy signal as proof of correctness (both GNSS
   `heading_std_dev_deg` and the BNO085 `Accuracy` enum failed this in testing).
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
   confirmed by a fresh validated GPS course fix this boot.
3. **DeadReckoning(elapsed)** — anchor was validated at some point, but GPS course is currently
   unavailable or failing the agreement check. `elapsed` = time since last correction.
4. **GpsCorrected** — a currently-agreeing, validated GPS course fix just updated the anchor.
5. **Manual** — anchor set by direct user input, treated as trusted (equivalent to
   `GpsCorrected` for tracking purposes, labeled separately for the UI).

## Transitions

- **Boot**: if a persisted `(heading, timestamp)` exists, anchor := persisted heading,
  `game_rv_at_anchor` := first Game RV reading, confidence := `PersistedPrior`. Otherwise
  confidence := `Unknown`, no anchor, wait.
- **Manual input** (any time): anchor := input value, `game_rv_at_anchor` := current Game RV
  reading, confidence := `Manual`.
- **Validated GPS course fix** (see gate below) arrives:
  - From `Unknown` / `PersistedPrior` / `DeadReckoning`: slew-limited correction (see below)
    toward the validated value; once settled, anchor/`game_rv_at_anchor` update, confidence :=
    `GpsCorrected`.
  - While already `GpsCorrected`: keep re-anchoring on each new validated fix, so drift never
    gets the chance to accumulate while GPS course is good.
- **GPS course becomes unavailable, drops below the minimum-speed gate, or fails the agreement
  check**: confidence steps down from `GpsCorrected` to `DeadReckoning(0)`. Anchor and
  `game_rv_at_anchor` are unchanged — only the confidence label changes, and `elapsed` starts
  accumulating.
- **Shutdown, or periodic tick**: persist `(tracked_heading, timestamp)`.

## Validation gate (shared by every anchor update, initial or correcting)
- Minimum speed threshold on `course_deg` — GNSS course-over-ground is known to be dominated by
  position noise at low speed; a numeric threshold needs a dedicated moving trial (this doc's
  source testing only ever exercised stationary `heading_deg`, never `course_deg`).
- Sustained agreement — N consecutive samples (or T seconds) agreeing within tolerance, not a
  single reading. This directly targets the "confidently wrong" failure mode seen twice in the
  `heading_deg` stationary trials; the same discipline is applied to `course_deg` even though it
  hasn't yet been observed to fail the same way, since it hasn't been tested at all.
- `heading_deg` is excluded from this gate entirely for now (see Deferred, below).

## Correction dynamics
Slew-rate-limited convergence toward a newly validated anchor, not an instant snap — an instant
jump would read as a glitch on a needle-style heading indicator. The rate needs tuning against
plausible real turn rates so a genuine fast turn mid-ramp isn't misread as a slew artifact; no
numeric rate is proposed here.

## Persistence
- Written periodically (e.g. on every anchor update, or every N seconds) — not only at clean
  shutdown. A hard power loss or the documented `earlyoom` kill-and-restart behavior (see
  CLAUDE.md TODO — deliberately not protecting the dashboard binary) can skip a clean-shutdown
  hook entirely.
- Loaded as `PersistedPrior`, never as `GpsCorrected` — it goes through the same validation gate
  as any other anchor before being trusted. This is what makes a car being started and turned
  before the dashboard finishes booting self-healing rather than a special case: Game RV
  dead-reckons through that gap on a possibly-wrong baseline, and the first validated GPS course
  fix corrects it via the normal slew-limited ramp, same as any other correction.
- The timestamp is for UI/diagnostics only (e.g. "last known heading, N hours old") — a stale
  persisted value is handled identically to a fresh one; age doesn't change the gating.

## UI-facing signal
Expose the confidence tier alongside the numeric heading (e.g. dimming or annotating a
compass/HSI needle during `DeadReckoning`, scaled by `elapsed`), since neither underlying
sensor's own accuracy field can be trusted as that signal.

## Deferred / explicitly excluded
- **`heading_deg`**: excluded from any role for now. Phase 1 testing showed its own uncertainty
  estimate doesn't correlate with correctness. Could re-enter later as a weak corroborating
  cross-check against `course_deg`, contingent on a dedicated moving `heading_deg`-vs-`course_deg`
  trial that hasn't been run.
- **BNO085 Rotation Vector / Geomagnetic RV**: excluded entirely — magnetometer-fused error was
  too large even at a relatively benign test location, and a car body is expected to be worse.
- **Long-duration Game RV drift**: uncharacterized past ~25 minutes. The `DeadReckoning(elapsed)`
  tier is meant to make an extended dead-reckoning stretch visible in the UI, not to solve
  unbounded drift — a long-duration trial is still needed before relying on this for e.g. an
  hour-long GPS-denied stretch.

## Relationship to `SENSOR_FUSION_CHAIN_DESIGN.md`
That doc's `HeadingFusionSensor` concrete use case sketches a simpler fallback policy ("trust
BNO085 when available, else GNSS heading"). This design supersedes that policy specifically — the
anchor/validate/correct logic above is what `FusedAnalogSensor::read` should implement once
built. The chain-type plumbing described there (`SensorFusedAnalogInputChain`,
`Option<u16>`-per-provider) is still the intended mechanism; only the arbitration policy changes.
Also still blocking, per that doc: `I2CProvider` (`hardware/hw_providers.rs`) is a dead stub, so
BNO085 isn't wired into the provider traits yet.

## Open decisions / needs real-world numbers
- Minimum speed threshold and sustained-agreement window (sample count / duration / tolerance)
  for accepting a `course_deg` reading.
- Slew-rate limit for the correction ramp.
- Persistence write cadence.
- How confidence tier is rendered in the UI — display design, not fusion logic.
- Long-duration Game RV drift characterization.

## Status
Design only — not implemented. Depends on `SENSOR_FUSION_CHAIN_DESIGN.md`'s chain-type
scaffolding (also not yet implemented) as the mechanism this policy would run inside.

---
*Created: August 8, 2026*
