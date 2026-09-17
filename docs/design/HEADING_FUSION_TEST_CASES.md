# Heading Fusion — Test Case Design

Test cases for the policy in `HEADING_FUSION_DESIGN.md`, written ahead of implementation so the
state machine, gates, and correction dynamics have a concrete pass/fail spec to build against
and check coverage against once `FusedAnalogSensor::read` (per `SENSOR_FUSION_CHAIN_DESIGN.md`)
exists. Distinct from `niva_dashboard/src/util/COMPASS_SENSORS_TESTING.md`, which measures raw
GNSS/BNO085 sensor accuracy in the field — these are unit-level tests of the *arbitration logic*
that consumes those readings, run against synthetic inputs, not live hardware.

## Assumed test surface

The design doc doesn't yet specify a call signature (it's blocked on the chain-type scaffolding
in `SENSOR_FUSION_CHAIN_DESIGN.md`). These cases assume something shaped like:

```rust
struct HeadingFusionState {
    confidence: Confidence,          // Unknown | PersistedPrior | DeadReckoning(Duration) | GnssCorrected | Manual
    anchor_heading_deg: f32,
    game_rv_at_anchor_deg: f32,
}

fn tick(
    &mut self,
    gnss: Option<GnssFix>,           // course_deg, heading_deg, heading_std_dev_deg, speed_kmh, fix_quality (nmea.rs)
    game_rv_heading_deg: Option<f32>,// Bno085Frame::game_orientation().heading_deg, None if stale/absent
    now: Instant,
) -> f32                             // tracked_heading_deg
```

**Testability requirement**: the four TBD constants from the design doc's "Open decisions"
(min-speed threshold, sustained-agreement window/tolerance, `heading_std_dev_deg` threshold,
slew rate) should be constructor/field-injectable, not hardcoded. That lets these tests assert
behavior with convenient round numbers independent of whatever the real-world-calibrated value
ends up being. Test IDs below use placeholder names (`MIN_SPEED`, `AGREE_N`, `AGREE_TOL`,
`STD_DEV_MAX`, `SLEW_RATE`) for whatever is injected.

## Open ambiguity found while designing these cases

The design doc's validation gate says sustained agreement means "N consecutive samples ... agreeing
within tolerance" but doesn't say agreeing *with what*. Two readings are consistent with the text
and produce different behavior:

- **(a) Self-consistency**: successive raw GNSS readings agree with each other (the reading has
  stopped moving/settling).
- **(b) Agreement with the current anchor**: the GNSS candidate agrees with `tracked_heading` as
  Game RV has been dead-reckoning it.

Interpretation (a) does **not** close the "confidently wrong" failure mode `COMPASS_SENSORS_TESTING.md`
documents twice (window-sill trials converging fast to a tight, stable, wrong value) — a
self-consistent-but-wrong reading passes it. Interpretation (b) closes that gap but starves the
very first anchor acquisition (`Unknown`/`PersistedPrior`), where there's no trustworthy
`tracked_heading` yet to compare against. Test cases `GATE-05`/`GATE-06` below pin down both
readings so whichever the implementation picks, there's a case that documents the choice — this
should be resolved during implementation, not guessed at here.

---

## A. Boot / initialization

| ID | Given | When | Then |
|----|-------|------|------|
| BOOT-01 | No persisted `(heading, timestamp)` on disk | First `tick()` | `confidence = Unknown`, no anchor, `tracked_heading` has no value to report |
| BOOT-02 | Persisted `(heading=200.0°, timestamp=1h ago)` exists | First `tick()`, Game RV present | `confidence = PersistedPrior`, `anchor_heading = 200.0°`, `game_rv_at_anchor` = that tick's Game RV reading |
| BOOT-03 | Persisted record exists, Game RV **not yet available** at boot | First `tick()` | Anchor not established until a Game RV reading arrives (no `game_rv_at_anchor` to pin); confidence stays `Unknown` until then, not `PersistedPrior` with a garbage baseline |
| BOOT-04 | Persisted record with an age of e.g. 30 days (stale by wall clock, not by design — see design doc's persistence section) | First `tick()` | Treated identically to a fresh persisted record: `confidence = PersistedPrior`. Age is UI-only; must not change gating |

## B. Manual input

| ID | Given | When | Then |
|----|-------|------|------|
| MAN-01 | `confidence = DeadReckoning`, arbitrary anchor | Manual heading input = 45.0° | `anchor_heading = 45.0°`, `game_rv_at_anchor` = current Game RV reading, `confidence = Manual`, snaps instantly (not slew-limited — design doc only specifies slew-limiting for GNSS corrections, not manual entry; confirm this reading during implementation) |
| MAN-02 | `confidence = GnssCorrected` | Manual heading input arrives | Overrides the GNSS-derived anchor; `confidence = Manual` |
| MAN-03 | `confidence = Manual`, no further input | A validated GNSS fix arrives afterward | Per design doc's `GnssCorrected`-tier re-anchoring rule, `Manual` is not listed as re-anchor-eligible in the transitions section — clarify during implementation whether a validated fix pulls `Manual` back to `GnssCorrected`, or whether `Manual` is sticky until the next manual/reboot event. Write this test once decided |

## C. Validation gate — `course_deg`

| ID | Given | When | Then |
|----|-------|------|------|
| GATE-01 | `speed_kmh = MIN_SPEED - 1` (just under threshold), `course_deg = Some(90.0)` | `tick()` | `course_deg` reading rejected outright (fails speed gate before sustained-agreement is even evaluated) |
| GATE-02 | `speed_kmh = MIN_SPEED` (boundary) | `tick()` | Confirm inclusive/exclusive boundary behavior once threshold is picked; document which |
| GATE-03 | `speed_kmh = MIN_SPEED + 5`, `course_deg` holds within `AGREE_TOL` for `AGREE_N - 1` consecutive samples, then one outlier sample | `tick()` × N | Sustained-agreement counter resets on the outlier; gate does not pass at sample N |
| GATE-04 | Same as GATE-03 but all `AGREE_N` samples agree | `tick()` × N | Gate passes on the Nth sample, not before |

## D. Validation gate — `heading_deg`

| ID | Given | When | Then |
|----|-------|------|------|
| GATE-05a | `heading_std_dev_deg = STD_DEV_MAX - 0.1` (passes std-dev sub-gate), value is stable and self-consistent across `AGREE_N` samples, but offset ~90° from current `tracked_heading` (models the window-sill "confidently wrong" case from `COMPASS_SENSORS_TESTING.md`) | `tick()` × N | **If interpretation (a):** gate passes, anchor jumps ~90° off. **If interpretation (b):** gate fails, anchor holds. Pick one per the ambiguity note above and assert it explicitly — this is the test that would have caught the window-sill failure mode had it existed pre-implementation |
| GATE-05b | Same as GATE-05a but this is the very first anchor (`confidence = Unknown`, no prior `tracked_heading` to compare against) | `tick()` × N | Confirms interpretation (b) has a defined bootstrap behavior (e.g. self-consistency-only when no anchor exists yet, escalating to interpretation (b) once one does) rather than permanently blocking first-anchor acquisition |
| GATE-06 | `heading_std_dev_deg` widens mid-sequence (models the hand-rotation transient from `COMPASS_SENSORS_TESTING.md`, std climbing into the tens/hundreds of degrees) | `tick()` sequence: tight → wide → tight | Reading rejected while std is wide, regardless of how tight the value itself looks; sustained-agreement counter does not carry across the wide stretch |
| GATE-07 | `heading_std_dev_deg = None` (field absent from fix) | `tick()` | Treated as gate failure, not as "no opinion" / pass-through |
| GATE-08 | `speed_kmh` high (car moving) and `heading_std_dev_deg` also passes | `tick()` | Confirm `heading_deg` is still eligible while moving (design doc says no minimum-speed requirement for it) — not just a stationary-only path |

## E. Anchor update / re-anchoring while `GnssCorrected`

| ID | Given | When | Then |
|----|-------|------|------|
| REANCHOR-01 | `confidence = GnssCorrected`, car moving | New validated `course_deg` fix arrives, differs slightly from current `tracked_heading` | Anchor + `game_rv_at_anchor` update immediately (per design doc: re-anchor on *every* new validated fix while already `GnssCorrected`, no additional settling wait) |
| REANCHOR-02 | `confidence = GnssCorrected` via `course_deg` (moving) | Car stops; `course_deg` fails speed gate but `heading_deg` starts passing its own gate | Re-anchoring source switches to `heading_deg` with no explicit mode-switch step or confidence drop in between (implicit handoff per design doc) |
| REANCHOR-03 | `confidence = GnssCorrected` | Both `course_deg` and `heading_deg` pass their gates simultaneously with **conflicting** values | Design doc doesn't specify tie-breaking between the two simultaneously-valid sources — needs a decision (e.g. prefer `course_deg` while moving) before this case can get a concrete `Then`; flag as a gap alongside the "Open decisions" list |

## F. Gate loss → stepping down to `DeadReckoning`

| ID | Given | When | Then |
|----|-------|------|------|
| DROP-01 | `confidence = GnssCorrected` | GNSS link lost entirely (no fix at all) | `confidence → DeadReckoning(0)`; `anchor_heading`/`game_rv_at_anchor` unchanged; `elapsed` starts at 0 and increases each subsequent tick |
| DROP-02 | `confidence = GnssCorrected` via `course_deg` | Car decelerates below `MIN_SPEED`, no `heading_deg` fix available to take over | `confidence → DeadReckoning(0)` (both sources now fail their gates) |
| DROP-03 | `confidence = GnssCorrected` via `heading_deg` | `heading_std_dev_deg` degrades past `STD_DEV_MAX` | `confidence → DeadReckoning(0)`, even though a `heading_deg` value is still being reported by the receiver |
| DROP-04 | `confidence = DeadReckoning(elapsed=5s)` | Another tick passes with still no valid fix | `elapsed` increases monotonically; `tracked_heading` keeps updating via Game RV integration off the unchanged anchor |
| DROP-05 | `confidence = DeadReckoning(elapsed=T)` | A validated fix arrives again | Per design doc's transition table: from `DeadReckoning` this goes through the slew-limited correction path (not an instant re-anchor like `REANCHOR-01`, which only applies from an already-`GnssCorrected` state) |

## G. Correction dynamics (slew limiting)

| ID | Given | When | Then |
|----|-------|------|------|
| SLEW-01 | `tracked_heading = 100°`, a validated fix at `150°` arrives, `SLEW_RATE` injected as e.g. 30°/s | Ticks advance over the correction | `tracked_heading` ramps 100° → 150° at ≤ `SLEW_RATE`, not an instant jump; confidence flips to `GnssCorrected` only once settled (per transition table: "once settled, anchor/`game_rv_at_anchor` update") |
| SLEW-02 | Mid-ramp from SLEW-01 | A genuine physical turn happens (Game RV shows real rotation) concurrently with the ramp | Design doc flags this as the reason no numeric rate is proposed yet — needs a case that distinguishes "ramp toward corrected value" from "vehicle actually turning" once a rate is chosen; write concretely once `SLEW_RATE` is picked, but the *shape* of this test (inject a Game RV delta during an active slew and check it isn't absorbed into or fights the ramp) should exist regardless of the final number |
| SLEW-03 | Correction target is behind by 350° vs. 10° (i.e., wraps through 0°/360°) | Ramp proceeds | Ramps the short way (10° gap), not the long way (350°) — a shortest-angular-path bug here would be an easy, silent mistake |

## H. Persistence

| ID | Given | When | Then |
|----|-------|------|------|
| PERSIST-01 | Anchor updates (any tier transition that touches anchor) | — | `(tracked_heading, timestamp)` written to disk at that point, not only at shutdown |
| PERSIST-02 | Process killed abruptly (models `earlyoom`, per CLAUDE.md TODO — dashboard binary deliberately unprotected) mid-session, no clean shutdown hook runs | Process restarts | Last periodically-persisted value is loaded as `PersistedPrior` — not lost, not treated as `Unknown` |
| PERSIST-03 | Persisted value exists from a *previous* boot's `GnssCorrected` state | Fresh boot | Loaded as `PersistedPrior`, **never** directly as `GnssCorrected` — must go through the validation gate again before being trusted (design doc is explicit about this) |
| PERSIST-04 | Vehicle started and driven before the dashboard process finishes booting (heading changed while the process was down) | Dashboard starts, loads stale `PersistedPrior`, Game RV starts dead-reckoning from it | First validated fix corrects it via the normal slew-limited ramp (SLEW-01), same code path as any other correction — no special-cased "cold start" branch should exist |

## I. Confidence tier / UI signal

| ID | Given | When | Then |
|----|-------|------|------|
| UI-01 | `confidence = DeadReckoning(elapsed)` | `elapsed` grows | Exposed confidence value reflects growing `elapsed` each tick (for e.g. UI dimming scaled by elapsed, per design doc) |
| UI-02 | `confidence = DeadReckoning(elapsed=large)` | A validated fix arrives and passes the gate | `elapsed` resets / tier changes to `GnssCorrected`— `elapsed` must not keep counting once corrected |
| UI-03 | Every tier (`Unknown`, `PersistedPrior`, `DeadReckoning`, `GnssCorrected`, `Manual`) | Read via whatever accessor the UI layer will use | Confidence is exposed as a distinct field from the numeric heading — never inferred by the UI from GNSS's own `Accuracy`/`heading_std_dev_deg`, which the design doc explicitly distrusts as a display signal |

## J. Robustness / edge inputs

| ID | Given | When | Then |
|----|-------|------|------|
| EDGE-01 | Game RV reading is stale (`Bno085Frame::is_stale()` true) or absent for one or more ticks | `tick()` | No panic; behavior should be defined (e.g. hold last `tracked_heading`, or a defined fallback) — `COMPASS_SENSORS_TESTING.md` found BNO085 frames stale on ~50% of poll ticks in every trial, so this is not a rare edge case, it's close to half of all ticks in practice |
| EDGE-02 | Game RV heading wraps 359° → 0° between two ticks (real rotation crossing north) | `tick()` | Delta computation (`game_rv_now − game_rv_at_anchor`) handles the wrap correctly, not a ~360° spurious jump |
| EDGE-03 | `anchor_heading + delta` exceeds 360° or goes negative | `tick()` | `tracked_heading` normalized back into `[0, 360)` |
| EDGE-04 | GNSS `fix_quality` is present but not `Gps`/`DGps`/etc. (e.g. `Invalid`, as seen transiently in `COMPASS_SENSORS_TESTING.md` trials during acquisition) even though `heading_deg`/`course_deg` fields are `Some` | `tick()` | Confirm whether `fix_quality` feeds the gate at all — design doc's validation gate section doesn't mention it, only speed and std-dev; if it should, that's a gap to raise, not assume |
| EDGE-05 | GNSS reports a value then briefly `None` then the same value again within one sustained-agreement window (transient signal loss mid-convergence, per the `gnss_rotation_3`/`gnss_rotation_4` blackout-then-snap pattern in `COMPASS_SENSORS_TESTING.md`) | `tick()` sequence | Confirm the `None` tick resets the sustained-agreement counter rather than being silently skipped/ignored, since a skip would let the two genuine (but blackout-bracketed) readings count as consecutive when they weren't |

---

## Coverage cross-reference

| Design doc transition (§Transitions) | Covered by |
|---|---|
| Boot, persisted prior exists | BOOT-02, BOOT-03, BOOT-04 |
| Boot, no persisted prior | BOOT-01 |
| Manual input, any time | MAN-01, MAN-02, MAN-03 |
| Validated fix from Unknown/PersistedPrior/DeadReckoning | GATE-05b, DROP-05, SLEW-01 |
| Validated fix while already GnssCorrected | REANCHOR-01, REANCHOR-02 |
| Both sources fail gate simultaneously | DROP-02 |
| GNSS becomes unavailable / fails gate | DROP-01, DROP-03, DROP-04 |
| Shutdown / periodic persistence tick | PERSIST-01, PERSIST-02 |

Gaps not yet resolvable into concrete `Then` clauses without an implementation decision:
**GATE-05/06 ambiguity** (self-consistency vs. anchor-agreement), **REANCHOR-03** (tie-break
between simultaneously-valid `course_deg`/`heading_deg`), **MAN-03** (does a validated fix pull
`Manual` back to `GnssCorrected`), **EDGE-04** (does `fix_quality` participate in the gate at all).
These four should be settled during implementation and this table updated to point at the
resulting concrete test, rather than the placeholder cases above.

---
*Created: August 9, 2026, alongside `HEADING_FUSION_DESIGN.md` (status: design only, not yet
implemented — these are the acceptance tests to write once it is).*
