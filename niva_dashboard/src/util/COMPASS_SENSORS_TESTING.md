# GNSS / BNO085 Heading Accuracy and Precision Testing

## Goal

Assess, with numbers rather than the qualitative "approximately NNN degrees after approximately
several minutes" notes in `Compass_fusion_calibration_reseach`:

1. How accurate GNSS dual-antenna heading (`#UNIHEADINGA`'s `heading_deg`, via the UM982) is
   against a known true heading, and how stable it is once converged, while stationary.
2. How accurately and how quickly the BNO085 tracks a *relative* rotation, with the
   magnetometer and gyro contributions isolated from each other rather than only ever seen
   blended together in the fused Rotation Vector output.
3. How both systems react to a deliberate, known change in physical orientation.

Both sensors start from effectively random readings on power-up and converge (GNSS to an
absolute heading; BNO085 to internal consistency only, since it has no way to self-calibrate
without being moved). This test measures that convergence quantitatively, in place of the prior
manual/qualitative observation.

## What's measured

`heading_deg` is one field of `GnssFix` — not `course_deg` (direction of travel, only
meaningful while moving). `#UNIHEADINGA`'s heading is the dual-antenna orientation solution,
valid while stationary, and it's the only GNSS field under test here.

The BNO085 side runs three SH-2 report types simultaneously, decoded via
`bno085_data_provider::Bno085Frame`:

| Report                      | SH-2 ID | Fuses                       | Isolates                                      |
|-----------------------------|---------|-----------------------------|-----------------------------------------------|
| Rotation Vector             | 0x05    | gyro + accel + magnetometer | full fused heading (baseline)                 |
| Game Rotation Vector        | 0x08    | gyro + accel only           | gyro-only heading tracking, no magnetic input |
| Geomagnetic Rotation Vector | 0x09    | accel + magnetometer only   | magnetometer-only heading tracking, no gyro   |

Game Rotation Vector has no absolute reference, so it carries no accuracy estimate
(`heading_accuracy_deg: 0.0`, `accuracy: None` in `Bno085Orientation`) — that's expected, not a
fault. Running all three side by side is what lets gyro drift and magnetometer distortion be
told apart, instead of manually integrating raw gyro data to reinvent what the chip already
does internally (and more robustly — bias tracking, proper quaternion integration).

Accelerometer reporting is disabled for this test (not useful for heading assessment, and
cutting it reduces I2C/report traffic).

## Running the test

```
cargo run -- test=heading
```

Starts the real `GnssDataProvider` (`/dev/niva_gps`) and `Bno085DataProvider` (with Rotation
Vector + Game RV + Geomagnetic RV enabled), polls both at a fixed 200ms tick, and logs a
timestamped line to console/log file (`~/Work/Niva_Dashboard_Rpi/Niva_dashboard_rpi/Logs`,
per `src/util/logging.rs`) whenever the adaptive cadence below decides to. Stop with Ctrl+C.

Each log line reports, per source: current heading, plus that source's own quality signal
(GNSS: `heading_std_dev_deg`, `heading_satellites`, `fix_quality`; BNO085: the discrete
`Accuracy` enum and, where applicable, `heading_accuracy_deg`). These are not comparable
numbers across systems and are logged separately rather than collapsed into one "accuracy"
column.

### Logging cadence

Comparing each new reading against the last **logged** snapshot (not the previous raw reading)
so a steady sub-threshold drift still accumulates against a fixed anchor and eventually crosses
a threshold, rather than perpetually resetting against a reading that just moved along with it.
Any source flipping between available/unavailable (fix acquired/lost, link gone stale) counts
as an unbounded delta and forces an immediate log.

- max per-source delta since last log > 5° → log on every tick, for as long as this holds
  (dense sampling through a deliberate rotation).
- max per-source delta since last log > 1° → log at most every 10s.
- otherwise → log at least every 30s (heartbeat).

Implemented in `run_heading_test` (`src/test/run_test.rs`): `IMMEDIATE_THRESHOLD_DEG`,
`FAST_THRESHOLD_DEG` / `FAST_LOG_INTERVAL`, `SLOW_LOG_INTERVAL`.

## Ground truth

Repeated trials converging to the *same* value only demonstrates precision, not accuracy — the
prior manual trials in `Compass_fusion_calibration_reseach` include a run that converged fast
and confidently (< 1° reported accuracy) to a heading roughly 90° off. Before running trials,
establish one fixed reference heading for the antenna baseline / window sill orientation and
record it once. Every trial's converged heading gets compared against that reference, not just
against other trials.

Three independent measurements of the window sill baseline, all in close agreement:

| Method                                                              | Heading |
|---------------------------------------------------------------------|---------|
| Google Earth, parallel to building facade                           | 110°    |
| Google Earth, perpendicular to window glass → landmark (204°), −90° | 114°    |
| Phone compass (calibrated)                                          | 112°    |

**Reference value: 112° true heading.**

This is a **true** heading (geodetic), not magnetic — same reference frame as GNSS
`heading_deg`, which is derived purely from antenna positions with no magnetic sensor involved
([nmea.rs](nmea.rs), `heading_deg` doc comment). The two Google Earth measurements are true
heading by construction (computed from lat/lon). The phone compass reading is only valid to
mix into this average if the app was confirmed to be displaying true north (device-location-
corrected), not raw magnetic — verify this in the app's settings before trusting the agreement
between all three; local magnetic declination in this region is on the order of a dozen
degrees east (look up the exact current value for the actual coordinates — it drifts by
roughly a degree every few years — rather than assume a fixed number), so a genuinely magnetic
reading would be expected to disagree with the two Earth-based measurements by about that much,
not sit inside their 4° spread.

BNO085 Rotation Vector and Geomagnetic Rotation Vector headings are **magnetic**, not true
([bno085_protocol.rs](bno085_protocol.rs), `RotationVectorReport::euler_rad` doc comment) — before
comparing their logged heading against the 112° true reference, subtract local magnetic
declination first (magnetic ≈ true − declination, for easterly declination). A correctly
functioning magnetometer will read *low* relative to 112° by roughly the declination angle;
that offset is expected and is not sensor error. Game Rotation Vector has no magnetic input at
all and no absolute reference either way — it's never compared against 112°, only against its
own value at the start of a rotation (see Test procedure).

## Test procedure

### 1. Stationary convergence and stability

Run several repeated trials, both antennas and the BNO085 completely stationary, resetting both
sensors between trials:

- **GNSS reset**: physically unplug and replug the USB cable (`/dev/niva_gps`). A dashboard
  process restart alone is not a reset — the receiver's internal state (almanac, ambiguity
  resolution) may persist across that and is suspected to be part of why the prior manual
  trials showed such large run-to-run variance on identical antenna geometry.
- **BNO085 reset**: physically power cycle the sensor board. Restarting `test=heading` alone
  does *not* reset it — the driver never sends a reset command (`bno085_protocol.rs` only sends
  a Product ID Request on init and Set Feature Commands to enable reports; it only *listens*
  for the chip's own unsolicited reset-complete notification, never triggers one). The chip
  runs continuously off its own power rail regardless of whether a host process is attached, so
  restarting the test just reopens the I2C connection onto whatever internal fusion/gyro-bias
  state the chip already had — Rotation Vector and Geomagnetic RV pick up from that state, and
  Game Rotation Vector's origin is *not* re-randomized. A trial meant to test convergence from
  scratch needs an actual power cycle between runs, not just a process restart.

For each trial, record: time to converge (heading stable within some tolerance for a sustained
period), the converged value vs. the ground-truth reference, and post-convergence jitter.

GNSS heading is expected to be usable within **10–15 minutes**; longer than that is treated as
a fail for practical purposes, not just "slow."

BNO085's Rotation Vector/Geomagnetic RV headings are not expected to converge to anything
externally meaningful while sitting still without rotation (magnetometer calibration needs
motion) — this phase mainly characterizes noise floor and short-term stability at a fixed,
uncalibrated orientation, and confirms Game RV holds a constant heading (no gyro bias drift
visible over the trial's short duration) since it isn't referencing anything external at all.

Keep the physical environment (nearby metal, moved objects) fixed between trials — the window
sill location in the prior manual tests is a plausible source of magnetic distortion for the
magnetometer, and inconsistent surroundings would confound convergence-speed comparisons.

### 2. Relative rotation

**BNO085**: mounted in its 3D-printed fixture for repeatable, precise positioning. Reset, let
settle, rotate exactly 90° in the fixture, and log the resulting heading change on all three
BNO sources (Rotation Vector, Game RV, Geomagnetic RV). Compares each fusion input's tracked
rotation against the known 90° ground truth, both in magnitude and settling time.

**GNSS**: both antennas mounted on a shared ~1m rigid base, placed on the window sill. Reset,
let the heading solution converge, then physically rotate the entire base 180° in place on the
same sill (not a cable swap between antennas — that tests port/cable labeling, a different
thing from the solution itself) and log reconvergence. Small hand-placement deviation from
exactly 180° is expected and acceptable — record the setup, not a hand-measured exact angle.

## Results: Phase 1 stationary convergence (5 trials, 2026-08-08)

Window sill location, both antennas and the BNO085 stationary throughout, both sensors
power-cycled before each trial (GNSS: USB unplug/replug; BNO085: physical power cycle). Logs in
`Compass_test/test{1..5}.txt`; this table was produced with `Compass_test/analyze_heading_log.py
<log> --bucket-seconds 300 --tail-minutes 5` (also reports BNO085 accuracy-enum transitions,
staleness rate, and link/error events — re-run it on new logs rather than re-deriving these
numbers by hand).

| Trial | Duration | GNSS converged heading (last 5 min)        | vs. 112° truth | Time to std&lt;15° sustained | RV final | GEO final | GAME final |
|-------|----------|--------------------------------------------|----------------|------------------------------|----------|-----------|------------|
| test1 | 17.5 min | never converged (121–161°, still drifting) | —              | never                        | 31.7°    | 32.8°     | 2.0°       |
| test2 | 15.0 min | 234.3° ± 6.4°                              | off by ~122°   | 5.9 min                      | 32.4°    | 32.4°     | 0.0°       |
| test3 | 24.7 min | 111.6° ± 0.7°                              | off by 0.4°    | 4.6 min                      | 32.4°    | 33.2°     | 0.0°       |
| test4 | 11.7 min | 112.2° ± 1.6°                              | off by 0.2°    | 3.0 min                      | 30.6°    | 35.4°     | 0.3°       |
| test5 | 11.0 min | 193.9° ± 0.4°                              | off by ~82°    | 1.9 min                      | 37.2°    | 34.8°     | 4.7°       |

**GNSS heading: the outcome is qualitatively different each run, not just noisy around one
answer.** Across 5 independent power-cycles this produced three distinct outcome classes: failed
to converge at all (test1, still drifting after 17.5 minutes); converged fast and confidently to
a *wrong* heading (test2, ~6 min to tight std, final value 122° off; test5, ~2 min to tight std,
final value 82° off); and converged fast and accurately (test3 and test4, both within ~1° of the
112° true reference and within 0.6° of each other). `heading_std_dev_deg` was tight in all four
converging trials — wrong or right — so the receiver's own uncertainty estimate gives no signal
for whether the converged value is actually correct. This is the exact failure mode this doc's
Ground Truth section warned about (from the prior manual trials), now reproduced twice out of
five trials with the true reference known, so a single converged trial should not be trusted
without repetition. `fix_quality` never advanced past `Gps` in any trial (briefly `Invalid`
during acquisition in test3/test5, self-resolving in under a minute) — worth checking on the
UM982 config side, since never reaching a differential/RTK-resolved state for a moving-baseline
heading solution is a plausible root cause for both the non-convergence and the confidently-wrong
outcomes.

**BNO085 RV / Geomagnetic RV are far more repeatable across power cycles than GNSS, though this
phase still isn't an accuracy test for them.** RV settled in the 30–37° range and GEO in the
32–35° range across all 5 power cycles — a ~7° spread, versus GNSS's >120° spread over the same
trials. The tight clustering itself is informative even though comparing it to the 112° true
reference isn't valid yet (magnetometer needs motion to calibrate, per Ground Truth above): the
raw uncalibrated magnetometer reading is consistent for this fixed orientation/environment. The
accuracy self-report mismatch already seen in test1 held in every trial: RV's `Accuracy` never
left `Unreliable` even when dead stable, while GEO claimed `High` from its very first reading in
all 5 trials, before its value had finished settling.

**Game RV** converged near 0° in 4 of 5 trials (0.0–0.3°) and to 4.7° in test5 — consistent with
a fixed physical mounting producing a similar arbitrary origin each reset, not a real absolute
reference re-emerging.

**Confirmed across all 5 trials, not a test1-only artifact**: BNO085 frames were stale (no new
SH-2 report since the last 200ms poll tick) on ~49–50% of ticks in every trial, for the entire
trial duration in each case. Still unexplained — check the configured SH-2 report interval for
the three simultaneously-enabled reports against the 200ms poll tick.

Isolated events, seen once each, neither recurring nor visibly affecting the logged headings:
- test4: BNO085 driver logged `data stale despite no reported error, forcing reconnect` at
  t≈375s (mid-run); RV/GEO/GAME showed no discontinuity across the reconnect.
- test5: one `Serial read error: stream did not contain valid UTF-8` on the GNSS link at connect
  time, self-recovered (reconnected within the same tick, &lt;10ms).

## Results: BNO085 power-on orientation sensitivity (5 trials)

Not the Phase 2 relative-rotation procedure above (which rotates once, continuously, within a
single power-on) — a separate, ad hoc BNO085-only test: power off, rotate 90° in the 3D-printed
fixture (cumulative — each rotation is 90° from wherever the previous one ended, so Rotation 5's
physical orientation is 450° = 90° around from Rotation 1's, i.e. the same orientation), power
on, let the reading settle, record RV/GAME/GEO once:

|            | RV                  | GEO           | GAME   |
|------------|---------------------|---------------|--------|
| Rotation 1 | 317.3° (Unreliable) | 322.8° (High) | 354.4° |
| Rotation 2 | 279.2° (Unreliable) | 278.4° (High) | 0.6°   |
| Rotation 3 | 128.2° (Unreliable) | 133.5° (High) | 359.8° |
| Rotation 4 | 42.9° (Unreliable)  | 38.5° (High)  | 0.4°   |
| Rotation 5 | 330.8° (Unreliable) | 328.9° (High) | 1.9°   |

**Game RV initializes to ~0° regardless of actual physical orientation.** Across 5 power-ons at
orientations 90° apart from each other (spanning the full 360°), GAME settled within a 7.5° band
(354.4°–1.9°, treating the wrap as one cluster around 0°) every single time. This directly
confirms the "arbitrary origin" model this doc already assumed for Game RV, but now specifically
pins it down: the origin isn't arbitrary in the sense of "some unpredictable value" — it's
arbitrary in the sense of "whatever physical orientation the chip is in at power-on becomes
yaw≈0," independent of true heading. (The earlier 5-trial stationary set couldn't distinguish
this from "GAME just happens to be consistent because the fixture wasn't moved between trials" —
this test moved it and got the same ~0° result every time, ruling that out.)

**RV and GEO agree tightly with each other within any single trial** (within 0.8–5.5° every
time, expected since both derive from the same magnetometer, just fused with or without gyro)
**but neither tracks the intended 90° steps cleanly between trials.** Against a fixture-precise,
consistent-direction 90° rotation each step, the four consecutive deltas were:

| Step  | RV delta | GEO delta | vs. ideal 90° |
|-------|----------|-----------|---------------|
| R1→R2 | −38.1°   | −44.4°    | ~46–52° short |
| R2→R3 | −151.0°  | −144.9°   | ~55–61° over  |
| R3→R4 | −85.3°   | −95.0°    | within 5–10°  |
| R4→R5 | −72.1°   | −69.6°    | ~18–20° short |

All four deltas share the same sign (consistent rotation direction, as expected from a fixture),
and the R1→R2 and R2→R3 errors are opposite enough to mostly cancel: the combined R1→R3 delta
(two 90° steps, ideal 180°) comes out to 170.9° (RV) / 170.7° (GEO) — only ~9° off, despite each
individual step being 46–61° off. And Rotation 1 and Rotation 5 — the same intended physical
orientation, 360° apart — landed within 13.5° (RV) / 6.1° (GEO) of each other, tighter than any
of the intermediate consecutive-trial gaps. Taken together this looks like each power-cycle adds
its own several-tens-of-degrees offset (magnetometer/gyro fusion presumably starting from a
default bias estimate rather than a persisted calibration, per this doc's expectation that
BNO085 doesn't self-calibrate without motion) on top of an otherwise fairly repeatable raw
magnetic reading, rather than the rotation-tracking itself being fundamentally unreliable — but
this is one set of 5 trials, not confirmed. RV stayed `Unreliable` and GEO stayed `High` in every
trial, same self-report pattern as the Phase 1 stationary results above.

## Results: Phase 2 relative rotation, BNO085 (5 legs, in-function)

This one *is* the documented Phase 2 procedure for the BNO085 side above: single continuous
power-on (no reset between legs), rotated in the 3D-printed fixture, reading taken after each
rotation stabilizes. Four consecutive 90° legs sweep A→B→C→D→A (a full 360° loop), then a fifth
leg repeats the loop in reverse:

| Leg              | Nominal rotation | RV Δ (error vs 90°)                                     | GAME Δ (error vs 90°) | GEO Δ (error vs 90°) |
|------------------|------------------|---------------------------------------------------------|-----------------------|----------------------|
| T1 A→B           | 90°              | −89.0° (1.0°)                                           | −89.0° (1.0°)         | −62.9° (**27.1°**)   |
| T2 B→C           | 90°              | −58.0° (**32.0°**)                                      | −90.8° (0.8°)         | −82.6° (7.4°)        |
| T3 C→D           | 90°              | −99.6° (9.6°)                                           | −88.7° (1.3°)         | −94.7° (4.7°)        |
| T4 D→A           | 90°              | −90.2° (0.2°)                                           | −90.3° (0.3°)         | −89.1° (1.1°)        |
| T5 A→A (reverse) | 360°             | −1.5° (loop-closure, not a magnitude check — see below) | −1.5°                 | +0.8°                |

(Between-leg continuity is solid: the end-of-leg reading for one test and the start-of-leg
reading for the next agree within 0–0.4° every time, confirming "stabilize, then read" gave a
settled value and nothing moved between logging the previous leg's end and starting the next.)

**Game RV tracked every individual 90° leg far more accurately than RV or GEO**: mean absolute
error 0.85° (max 1.3°) across the four 90° legs, versus RV's 10.7° mean (max 32.0°) and GEO's
10.1° mean (max 27.1°). Since Game RV has no magnetometer input, this isolates the error source:
it's the magnetic sensing, not the rotation-tracking math or gyro integration, that's unreliable
here — consistent with this doc's existing suspicion of magnetic distortion at the test location
(see Ground Truth above), now showing up as *orientation-dependent* error (up to 27–32° on
specific legs) rather than a uniform offset that would cancel out of a delta.

**RV and GEO don't consistently agree with each other on which is closer to truth** — GEO was
worse on T1 (27.1° vs RV's 1.0°), RV was worse on T2 (32.0° vs GEO's 7.4°), GEO was slightly
better on T3, RV was slightly better on T4. No consistent ordering; both are simply unreliable
in different ways depending on physical orientation, and averaging them wouldn't obviously help.

**Self-reported accuracy actively misleads here, not just uninformatively**: GEO logged `High`
on T1, the leg it was off by 27.1° — its worst leg of the whole test. RV logged `Unreliable` on
T4, the leg it was accurate to within 0.2° — its best leg. The self-report enums didn't just fail
to add information; on both of this test's most informative legs, they pointed the wrong way.

**T5 (reverse 360°) is a loop-closure check, not a magnitude check** — a shortest-path delta
between start and end can't distinguish "rotated 360° and returned" from "didn't move," so the
near-zero deltas confirm repeatability of the reading for a revisited orientation, not that a
full 360° actually happened. Comparing T5 to the *first* time each source read position A
(T1's start: RV 29.6°, GAME 359.9°, GEO 30.6°) rather than the most recent one is more telling:
by T4's end / T5 (RV ≈51–53°, GAME ≈0–1.5°, GEO ≈61–62°), RV had drifted 23.5° and GEO 30.8° from
their very first A reading, while GAME drifted only 1.6°. One plausible read: the magnetometer
fusion filters (RV, GEO) were still adjusting their bias estimate over the course of the session
as rotational motion accumulated (this doc already notes the BNO085 can't self-calibrate the
magnetometer without motion), so "position A" reads differently early versus late in a session
even without a power cycle — though a hand-positioning imprecision returning to A (no fixture
detail recorded for whether A itself is keyed/indexed) can't be ruled out from this data alone.

## Notes / caveats

- `heading_deg` and `course_deg` are different fields with different validity conditions
  (`nmea.rs`) — this test only exercises `heading_deg`; don't conflate results here with
  course-over-ground behavior while driving.
- BNO085 accuracy self-reports (the `Accuracy` enum, `heading_accuracy_deg`) are not proof of
  correctness — treat them as one more logged signal, not a stopping criterion by themselves.
- Game Rotation Vector's heading has an arbitrary origin at each reset (no absolute reference)
  — only its *change* since reset is meaningful, not its absolute value.
