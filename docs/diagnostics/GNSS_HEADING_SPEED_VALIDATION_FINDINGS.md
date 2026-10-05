# GNSS/INS Heading and Speed Validation — First Real-Drive Log

*Session: 2026-10-05. First real car log captured via `util::nav_validation_log` (see
`niva_dashboard/src/util/nav_validation_log.rs`), `Logs/nav_validation.csv` — a single
UM982 + BNO085 + wheel-speed-sensor session, ~80 minutes, two trips with a stop between
them, Lviv, Ukraine. Analysis done offline on the dev machine, not on the Pi.*

> **Summary.** The logged trip structure, speed profile, and heading data all match the
> known-good reference (an actual drive the user made and could verify by memory). INS and
> GNSS course agree with each other on every real turn checked (0 direction disagreements
> across 2486 GNSS course updates). Cross-checking against OpenStreetMap road geometry
> confirms the fused heading output tracks real road direction to within ~1-4° on straight
> stretches; the handful of larger mismatches are explained either by genuine turn-settling
> lag (most cases) or by the map-matching tool picking the wrong road segment at a complex
> junction/parking area (a tooling limitation, not a sensor or fusion defect).

## 1. Trip structure

Timeline reconstructed from the log (local time):

| Segment | Start | End | Duration |
|---|---|---|---|
| Trip 1 | 12:48:19 | 13:23:36 | 35.3 min |
| Stop (logging gap — dashboard/Pi powered down) | 13:23:36 | 13:42:20 | 18.7 min |
| Trip 2 | 13:42:20 | 14:08:12 | 25.9 min |

- Both trip durations land inside the expected ~25-35 minute range.
- Trip 2 ends within **~1.0 m** of where Trip 1 started (49.842504/24.009595 vs
  49.842506/24.009609).
- Trip 1 ends and Trip 2 starts at the same location (49.8103, 24.0413), ~4.24 km
  straight-line from the start point — consistent with "drove out, stopped, drove the same
  route back."
- A small additional ~11s logging gap appears right before the main 18.7-minute one; harmless.

## 2. Speed profile

Checked against the expectation of "up to 30-40 km/h for most of the route, brief spikes to
50-60 km/h," using GNSS speed as ground truth and the wheel-pulse (logical) sensor as the
cross-check, restricted to actual driving time (stops excluded):

- ~88% of driving time ≤ 40 km/h.
- ~7% between 40-50 km/h, ~4-6% between 50-60 km/h.
- GNSS max speed 58.9 km/h (never exceeds 60); logical sensor max 63.2 km/h (same event,
  slightly hot at the top of its range — GPS position moves continuously across those rows,
  confirming it's a real excursion, not a glitch).
- 13 separate >40 km/h bursts, mostly 1-20s, two longer ones (53s, 107s) — consistent with
  mixed city/suburban driving, not sustained highway cruising.
- Two sensor-settle artifacts (ADC channel startup sweep, see `SETTLE_DEFAULT_SPEED_KMH` in
  `adc_data_provider.rs`) show the logical sensor reporting 65-180 km/h while GNSS speed is
  ~0 and position is frozen — at log start and right before the big logging gap. Real data,
  not a measurement, and already excluded from the stats above.

## 3. Heading smoothness (INS vs GNSS course)

Goal: check both heading sources change smoothly through curves, with no spikes or dropouts.

- **INS heading (raw BNO085 Game Rotation Vector):** 7286 consecutive-sample pairs checked
  (circular rate, deg/s). Zero isolated single-frame outliers (a spike defined as one
  sample's rate >2.5x both neighbors with no reversion). Every large rate value sits inside a
  multi-sample run that climbs/falls continuously in one direction — i.e. a real turn, not a
  glitch. Max turn rate observed: 52.3 deg/s at 13:06:57 (lat 49.834665, lon 24.016492),
  confirmed by the user as a real sharp turn made at that exact spot.
- **GNSS course (`gnss_course_deg`):** naive differencing flags up to 112 deg/s "spikes," but
  this is a receiver update-rate artifact: 29% of consecutive logged samples (while moving
  ≥10 km/h) repeat the previous sample's exact value — the receiver refreshes course at
  roughly half the ~2 Hz log rate, producing a staircase (flat, then a jump) rather than real
  noise.
- **Cross-check:** at all 2486 genuine GNSS course updates (speed ≥10 km/h on both sides),
  INS heading change over the same interval was compared for direction. **0 disagreements.**
  INS and GNSS course never point opposite ways during the same turn.

Conclusion: no real heading jank in either source. GNSS course just looks coarser on a raw
plot due to its lower effective update rate, not because it disagrees with the INS track.

## 4. OpenStreetMap road-direction cross-validation

### Why

The dashboard's own cross-validation (INS vs GNSS course agreeing) only proves the two
sensors agree with each other — not that either is actually right. A shared calibration or
mounting bias would pass that check undetected. Matching against OSM road geometry is an
independent, external reference neither sensor had any part in producing.

### Tool

Built as a separate, stdlib-only Python script (not part of the Rust dashboard):
`scripts/validate_heading_osm.py`. For each sampled moving point (speed ≥10 km/h, decimated
to one sample per 5s by default): fetches drivable OSM ways for the route's bounding box via
a single cached Overpass API call, projects the point onto the nearest road segment, and
compares that segment's bearing to `ins_heading_deg` / `fused_heading_deg` / `gnss_course_deg`
mod 180° (a road carries traffic both ways). Points within 6m of a segment endpoint are
flagged low-confidence (junction ambiguity) and excluded from headline stats. See the
script's own docstring for full usage.

### Results (259/375 samples matched confidently; 116 excluded as near-junction/no-road)

| Source | mean | median | max |
|---|---|---|---|
| Fused heading (dashboard output) | 4.3° | 1.2° | 72.5° |
| GNSS course | 2.8° | 1.0° | 79.5° |
| INS heading (raw) | 44.0° | 58.3° | 85.7° |

**Fused heading tracks the real road to within ~1-4° typically.** The INS row is not a fair
absolute comparison and should be disregarded for this check — `ins_heading_deg` is logged
raw (unanchored Game Rotation Vector), which by design has no absolute reference (see
`heading_fusion_sensor.rs`'s own doc comment) until combined with the fusion's current
`correction_offset_deg`. Its large mean/median here reflects that floating offset, not sensor
noise — confirmed in the deep-dive below, where raw INS independently shows a smooth,
correctly-sized turn that just sits on a different absolute scale than true north.

### Worst 15 fused-heading mismatches — manually checked against adjacent rows

11 of 15 are explained by **turn-settling lag**: the sample was taken mid-turn (heading still
sweeping), and the dashboard's heading settles onto the matched road's bearing within
1.5-9 seconds afterward, then holds there for 10-20+ seconds of straight cruising. Examples:
13:00:24 (settles in ~1.5s), 13:02:06 (~2s), 13:43:39 (~6s), 13:48:52 (~5.5s), 13:59:17
(~2.5s), among others. One pair (13:13:04/13:13:09) settles via a more complex two-stage
event — a continuing S-curve plus a GNSS re-anchor landing at the same time — but still
converges within ~4-9s.

4 of 15 do **not** settle toward the matched road at all — these are a different failure
mode, not sensor error:

- **13:07:32 (the single worst mismatch, 72.5°)**, lat=49.833719 lon=24.014911: fused heading
  settles to ~140-147° within ~1s and *holds there* for 20+ seconds of accelerating,
  confident cruising (14→18 km/h) — a stable, real reading. The matched OSM road bearing was
  232.4°, which it never approaches. GNSS course is identical to fused throughout this window
  (the fusion is directly tracking validated GNSS course tick-by-tick here), so it isn't "just
  GNSS" vs "just fused" — they're the same signal at this point. Raw INS, checked separately,
  shows a smooth, continuous ~91° turn over the same window with no jank, same direction and
  magnitude as the fused/course turn (~93°) — i.e. all three sensor-side signals agree with
  each other on what physically happened. The implied `fused - ins` offset only drifts ~8°
  across the turn (214.4° → 206.1°), confirming internal consistency. This location sits
  within a few meters of another flagged point 30s earlier (13:07:03) — both are inside one
  junction. Read together: the dashboard is very likely reporting the true heading correctly,
  and the map-matcher picked the wrong candidate road at that junction (the point was 5.3m
  from the segment — just outside the 6m endpoint-exclusion radius, despite effectively being
  inside a junction).
- **14:04:27, 14:04:32, 14:04:42** (all three within one continuous ~50s window,
  14:04:19-14:05:06): speed bounces 5-21 km/h with heading reversing direction repeatedly,
  never settling into a straight cruise. This window sits right at the destination — the car
  is evidently maneuvering into a tight yard/driveway, not driving a mapped road, so there's
  no stable "after" state to compare and the nearest OSM segment likely doesn't represent the
  actual path taken.

### Tooling caveats for next time

- Raising `--endpoint-radius` (currently 6m, try 10-12m) would likely auto-exclude the
  13:07:32-class junction mismatch.
- The final-approach parking cluster (14:04:xx) would need a different fix — e.g. excluding
  the last N seconds before a stop, or just expecting noise there — raising endpoint radius
  won't help since it's not a junction-proximity problem.
- Live Overpass API access was unreliable from the sandboxed dev-agent environment (quick
  504 → 406 responses, consistent with cloud-IP abuse protection on the public instance) but
  worked fine from the user's own machine. `--overpass-url` was added as an escape hatch for
  future runs if the default instance is ever unreachable.
- Road names in the first real run's console output appeared as mojibake (Cyrillic/Ukrainian
  names printed under the wrong codepage on Windows) — cosmetic only, not yet fixed.

## 5. Tooling built this session

- `niva_dashboard/src/util/nav_validation_log.rs` + wiring in `page_manager.rs`: a 2 Hz CSV
  logger (`Logs/nav_validation.csv`) recording INS heading, fused heading, GNSS
  lat/lon/course/speed, and the logical speed sensor — independent of the main
  `flexi_logger` event log, meant for exactly this kind of offline route validation.
- `scripts/validate_heading_osm.py`: the OSM cross-validation tool described in §4, following
  this repo's existing convention for standalone analysis scripts (stdlib-only Python, see
  `scripts/analyze_heading_log.py`).
