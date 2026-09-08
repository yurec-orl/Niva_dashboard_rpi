# Sensor Calibration — Design (draft)

## Problem

`GenericAnalogSensor` (`hardware/sensors.rs`) converts raw ADC counts to a
physical value with a single linear `scale_factor`. `EngineTemperatureSensor`
is worse — a hardcoded placeholder (`input as f32 * 0.1`, comment "Placeholder
conversion"). Neither reflects the real senders' behavior: coolant temp, oil
pressure, and fuel level are all resistive senders with **non-linear**
resistance-vs-physical-quantity curves, read out through a fixed voltage
divider on the STM32 ADC module.

Goal: replace the placeholder/linear conversions with calibration curves
grounded in the actual senders' datasheet characteristics, and let those
curves be adjusted — ideally without a rebuild, ideally from the dashboard
itself — once the car exposes discrepancies a datasheet can't predict
(sender manufacturing tolerance, OEM wiring/gauge unknowns, sender aging).

## Circuit context (from `stm32_adc_module/WIRING.md`)

PA0/PA1/PA2 (oil pressure, fuel level, coolant temp) share one divider design:
R1 = 39 kΩ, R2 = 10 kΩ, targeting a 0–16V sensor-wire range down to 0–3.3V at
the ADC (ratio ×0.204). **These three inputs are tapped from the existing
instrument-cluster wiring**, in parallel with the OEM gauge — the sensor-wire
voltage this divider measures is therefore set by the sender's resistance
dividing against *whatever is on the OEM side*.

**OEM side identified: these are cross-coil (ratiometric) gauges, fed
directly off +12V, no separate regulator.** Confirmed by inspection: each
gauge is two coils sharing one supply node — one coil returns straight to
ground (fixed reference field), the other returns through the sender's
resistance to ground (variable field); needle angle follows the ratio between
the two coils' magnetic fields. No stabilizer relay, no additional circuitry.
This resolves what "the OEM side" is: **`R_series` is the variable coil's DC
winding resistance** — a fixed, physical, directly-measurable quantity (not
an unknowable black box), and there is no thermal/pulsed stabilizer in this
circuit for the earlier caveat below to appeal to.

**Consequence — two separate ones:**
- **Good news:** `R_series` is measurable, not merely inferable. With the
  sender disconnected and the circuit unpowered, a multimeter across the
  sender-wire connector pin and the shared +12V supply pin reads the variable
  coil's resistance directly — no need to solve for it indirectly from a live
  anchor point (see Conversion pipeline below).
- **Bad news, and this is the important one:** a ratiometric cross-coil gauge
  is itself insensitive to supply voltage — both coils' currents scale
  together with `V_supply`, so their *ratio* (and thus the needle) doesn't
  move when the alternator kicks in. **Our ADC tap doesn't get that
  cancellation.** It reads the absolute voltage at one node (between the
  variable coil and the sender), which is exactly the plain two-resistor
  divider modeled below — full supply-voltage sensitivity, none of the
  ratiometric gauge's immunity. The cross-coil design explains why the
  *needle* doesn't care about supply voltage; it does nothing to protect our
  single-point voltage tap. See Problem: floating supply voltage below —
  that section's linear model is the actual behavior to design for, not a
  pessimistic bound.

## Sensor specifications (reference data)

### 1. Engine coolant temperature — ТМ106 (analog thermistor, in dashboard use)

Range +30…+130 °C. Known for poor accuracy at the low end of its range.
Resistance in Ω, datasheet tolerance band (min…max) per point:

| °C  | Ω (min…max) |
|-----|-------------|
| 30  | 1350…1880   |
| 40  | 880…1220    |
| 50  | 585…820     |
| 60  | 405…560     |
| 70  | 280…390     |
| 80  | 214…268     |
| 90  | 155…196     |
| 100 | 115…145     |
| 110 | 87…109      |
| 120 | 66…84       |
| 130 | 51…65       |

### 2. Engine coolant temperature — ECU sender (analog thermistor, future use)

Not read by the dashboard today; recorded here for when it is. Wider range
(−40…+130 °C) and notably tighter tolerance than the ТМ106 — this is the
sensor to prefer if/when the dashboard reads the ECU's own coolant signal.

| °C   | Ω           |
|------|-------------|
| −40  | 100700      |
| −30  | 52700       |
| −20  | 28680       |
| −15  | 21450       |
| −10  | 16180       |
| −4   | 12300       |
| 0    | 9420        |
| 5    | 7280        |
| 10   | 5670        |
| 15   | 4450        |
| 20   | 3520        |
| 25   | 2796        |
| 30   | 2238        |
| 35   | 1802        |
| 40   | 1459        |
| 45   | 1188        |
| 50   | 973         |
| 60   | 667         |
| 70   | 467         |
| 80   | 332         |
| 90   | 241         |
| 100  | 177         |
| 128  | 76.7…85.1   |

### 3. Oil pressure — ММ393А (analog, resistive)

Range 0–8 kgf/cm². Resistance in Ω, tolerance band per point:

| kgf/cm² | Ω (min…max) |
|---------|-------------|
| 0       | 290…320     |
| 4       | 103…133     |
| 6       | 55…80       |
| 8       | 0…15        |

### 4. Fuel level — 2121-3827010-02 / 21213-3827010-01 (analog, resistive)

Two sender variants exist, differing in resistance range; **21213 is the
variant installed in this car**. Resistance in Ω, tolerance ± per point:

| Level | 21213 Ω (installed) | 2121 Ω (other variant, not installed) |
|-------|----------------------|----------------------------------------|
| Empty | 250 ± 12             | ~330…350 at empty                       |
| Half  | 66 ± 6               | —                                        |
| Full  | 20 ± 3               | —                                        |

(2121's full/half points weren't given — only that its empty-tank resistance
sits ~330–350 Ω, distinctly higher than 21213's 250 ± 12 Ω. Recorded here in
case the sender is ever swapped and this needs revisiting.)

## Decisions

- **Default curves: transcribed directly from the datasheet**, not computed
  through the divider or bench/field-measured up front. Since the curve is
  stored in Ω (below), a table entry *is* the datasheet value — no forward
  computation, no assumed `V_supply`, nothing to get arithmetically wrong
  between spec sheet and JSON.
- **Curve representation: piecewise-linear table over Ω, not raw ADC
  counts.** (Supersedes an earlier draft of this doc that stored `(raw,
  value, v_ref)` triples — see Runtime conversion below for why that turned
  out to be the wrong domain.) Reasons:
  - Matches the datasheet point-for-point — the JSON *is* the spec table,
    nothing derived.
  - **Voltage-invariant.** Ω is a property of the sender alone; it doesn't
    need a `v_ref` field per point the way a raw-count table did, because
    raw counts are meaningless without knowing what supply voltage produced
    them.
  - **Interpolates in the physically correct domain.** The datasheet's own
    non-linearity is defined in Ω; the mapping from Ω to raw ADC count is a
    *further* non-linear (ratiometric-divider) transform on top of that.
    Piecewise-linear interpolation between two known points gives a
    different curve shape depending which domain it's done in — interpolating
    directly on Ω is the more faithful approximation of the sender's real
    characteristic, since that's the domain the datasheet itself was
    tabulated in.
  - Needs no expression language, and a calibration-UI edit is still just
    "insert/adjust one point" — the datasheet tolerance bands (15–40% wide)
    already dwarf whatever smoothness a fitted curve would add.

## Conversion pipeline and R_series

Full path between sender resistance and raw ADC count, per the PA0/PA1/PA2
divider in `stm32_adc_module/WIRING.md` — invertible in either direction:

```
R_sender (Ω)
  ⇄ V_sensor_wire = V_supply × R_sender / (R_series + R_sender)
  ⇄ V_adc_pin     = V_sensor_wire × R2/(R1+R2)      [10/49, fixed, known]
  ⇄ raw           = V_adc_pin / V_ref_adc × 4095     [12-bit, V_ref_adc ≈ 3.3V]
```

`R1`/`R2` (39kΩ/10kΩ) are known. `R_series` is the cross-coil gauge's variable
coil winding resistance (see Circuit context above) — fixed and physical.
`V_supply` is not fixed at all — see Problem: floating supply voltage below.

Since curves are now stored in Ω, **this pipeline is only needed at runtime,
in the raw→Ω direction** (Runtime conversion section below) — building the
*default* curve needs none of it; the datasheet tables above are copied in
directly. The Ω→raw direction is still useful separately, for hardware
bring-up sanity checks and for `TestADCDataProvider`'s self-test simulation
to synthesize a plausible raw value for a given simulated physical value —
but it's no longer on the critical path for producing a calibration curve.

**`R_series` measured directly, per gauge** (multimeter across the shared
+12V supply pin and each gauge's own sender-wire pin, on a spare cluster
pulled from the car):

| Gauge | R_series (Ω) |
|---|---|
| Fuel level | 124.4 |
| Coolant temp | 110.4 |
| Oil pressure | 130.8 |

Measured on a **spare cluster, not the one actually installed** — treat these
as accurate for the installed unit only insofar as OEM coil-winding tolerance
between two units of the same part is tight, which is a reasonable assumption
but not a verified one. Cheap to re-check against the installed cluster later
if a computed curve turns out to disagree with a field calibration point.
This is now a **runtime constant consumed on every `read()` call** (Runtime
conversion below), not a one-time curve-generation input — a real advantage
if it later needs correcting: fix one number, and every reading improves,
rather than needing to regenerate a whole lookup table.

Measurement procedure (multimeter, battery disconnected, ignition off):
1. Pull the cluster, expose its harness connector.
2. Identify the shared +12V gauge-supply pin — continuity-check across the
   *cluster-side* connector (harness unplugged) to find the pin common to all
   three gauges' coils, or trace it to a known ignition-switched +12V wire.
3. Identify each gauge's sender-wire pin — same wire the STM32 module's
   PA0/PA1/PA2 divider already taps for that sensor.
4. Ω mode, one probe on the shared +12V pin, one on the sender-wire pin, per
   gauge. Battery must be disconnected — a resistance reading on a powered
   circuit is meaningless (and risks the meter).

## Problem: floating supply voltage

The curve itself is voltage-invariant (it's just Ω), but converting a *live
raw ADC reading* into Ω requires knowing the actual `V_supply` at that
instant. Get this wrong — e.g. hardcode a nominal voltage instead of reading
it live — and the inferred Ω (and hence the reported value) will be
systematically off, because the car's 12V rail is not fixed: it runs roughly
12.2V (moderately charged battery, engine off) to 14.5V (alternator
charging), a **+18.85%** swing.

### The swing is exact and R_series-independent for the raw signal itself

For a fixed sender resistance, node voltage (and therefore raw ADC count)
scales **exactly linearly** with `V_supply` — this falls straight out of the
divider equation in the Conversion pipeline section above, and holds
regardless of `R_series`'s value:

```
raw(R_sender, V_supply) = V_supply × [R_sender / (R_series + R_sender)] × (R2/(R1+R2)) × (4095/V_ref_adc)
```

Everything in brackets and after is fixed for a given `R_sender`; only
`V_supply` varies. So `raw(R, V2) / raw(R, V1) = V2 / V1` exactly — a 12.2V→
14.5V swing produces an **18.85% raw-count error**, full stop, independent of
`R_series`, as long as neither divider stage saturates or hits the Zener
clamp.

### But the error in the *inferred* Ω is worse, and depends on R_series

Inverting the ratiometric fraction, `f = R_sender/(R_series + R_sender)`,
using the wrong `V_supply` doesn't produce a merely-18.85%-off Ω value. If the
runtime assumed a fixed reference voltage `V_calibration` instead of the real
`V_actual`, the *apparent* fraction it would compute is `f_apparent = α ·
f_true` (`α = V_actual/V_calibration`) — and because `f` is a ratio, not a
percentage, converting that back into an apparent resistance
(`R_apparent = R_series · f_apparent/(1 − f_apparent)`) amplifies the error,
by an amount that depends on `R_series`.

Worked example using the **measured** `R_series` values, showing what a
naive fixed-12.2V assumption would infer if the real supply were 14.5V
(still a representative-point illustration — one sample resistance per
sensor, not a full curve):

| Sensor | True point | True Ω | R_series (measured) | Apparent Ω if V assumed 12.2V | Apparent reading | Error |
|---|---|---|---|---|---|---|
| Coolant temp (ТМ106) | 90 °C | 175.5 | 110.4 | 297.9 | ~74.0 °C | **−16.0 °C** |
| Oil pressure (ММ393А) | 4 kgf/cm² | 118 | 130.8 | 169.0 | ~2.91 kgf/cm² | **−1.09 kgf/cm²** |
| Fuel level (21213) | 50% | 66 | 124.4 | 87.2 | ~44.3% | **−5.7 pts** |

The oil pressure case is the one that matters most: a ~1.1 kgf/cm² swing is
large next to the "critical `<1 kgf/cm²` at idle" threshold already
documented for this sensor (`PROJECT_CONTEXT.md`) — a supply-voltage-driven
error in either direction could mask a real low-oil-pressure condition or
trigger a false alarm, depending on which way the engine happens to be
running relative to whatever voltage a naive implementation assumed.

**No stabilizer to hope for.** An earlier version of this doc floated the
possibility that a pulsed instrument voltage stabilizer might be absorbing
this swing before it reaches the gauges, which would make the real
sensitivity smaller than this linear model. Confirmed by inspection: these
are cross-coil gauges wired directly to +12V with no additional circuitry
(Circuit context above) — there is no stabilizer in this circuit to appeal
to. The gauge *needle* is voltage-insensitive for a different reason (it
compares two coils' currents, and both scale together), which is irrelevant
to a single-point voltage tap like ours. **The linear model in this section
is the actual behavior to design for, not a pessimistic upper bound.**

## Runtime conversion: raw → Ω → value

This is what `CalibratedVariableResistanceAnalogSensor::read()` actually does on every
tick — the reverse of the Conversion pipeline, using the *live* supply
voltage rather than an assumed one:

```
V_adc_pin     = raw / 4095 × V_ref_adc                    [V_ref_adc ≈ 3.3V]
V_sensor_wire = V_adc_pin × (R1+R2)/R2                     [4.9, fixed, known]
R_sender      = R_series × V_sensor_wire / (V_supply_now − V_sensor_wire)
```

then linearly interpolate `value` from the sensor's `(ohm, value)` curve
using `R_sender`, clamping past either end (same spirit as `ValueConstraints`
min/max clamping elsewhere) rather than extrapolating past datasheet-covered
territory.

- `R1`, `R2`, `V_ref_adc`, and the 4095 (12-bit) scale become explicit named
  runtime constants — following the precedent already set by `osc_page.rs`'s
  `OSC_DIVIDER_R1_OHM`/`OSC_DIVIDER_R2_OHM` and its own inverse-divider
  helper for the PA3 voltage channel, not a new pattern for this codebase.
- `R_series` is the per-sensor measured constant from the previous section.
- `V_supply_now` comes from the live `Hw12v` reading — see Cross-sensor
  dependency below for how it reaches `read()`. No new hardware needed
  (`HWInput::Hw12v` / `AdcChannel::Voltage12V`, per
  `stm32_adc_module/WIRING.md`'s PA3 divider, already exists) and no
  separate "correction factor" step — supplying the right `V_supply_now` is
  simply what makes the raw→Ω conversion correct in the first place, rather
  than a compensation bolted on afterward.
- **Fault handling: `read()` returns `Err`.** As `R_sender` (from the
  datasheet) approaches or exceeds `R_series`, `V_sensor_wire` approaches
  `V_supply_now` and the denominator shrinks — normal within these three
  sensors' real ranges, but a disconnected sender or ADC noise pushing
  `V_sensor_wire` at or above `V_supply_now` would blow the computed
  `R_sender` up to a huge or negative value. Rather than compute and clamp a
  nonsensical resistance, `read()` treats `(V_supply_now − V_sensor_wire)`
  falling at or below a small margin (a few mV, to absorb ADC noise near the
  boundary rather than triggering only on exact equality) as a fault and
  returns `Err(...)` — no signature change needed, since `AnalogSensor::read`
  already returns `Result<&SensorValue, String>`.

  This also means no new plumbing in `SensorManager`: `read_analog_sensor`
  already propagates a chain's `Err` without touching `sensor_values` for
  that tick, and `read_all_sensors` already treats one chain's failure as
  independent of every other chain's (`sensor_manager.rs`'s existing
  resilience, exercised by
  `test_read_all_sensors_one_failing_chain_does_not_block_others`). A fault
  on, say, the coolant sender simply leaves `HwCoolantTemp` absent from
  `sensor_values` for that tick — the same "no value this tick" behavior
  every other transient read failure already produces (a routine GNSS
  no-fix, an ADC link drop), not a new failure mode for callers to handle.

- **Low-side fault: `read()` also returns `Err` for an implausibly small
  `R_sender`.** If the PA0/PA1/PA2 divider's *input* wire (the tap back to the
  instrument cluster) is disconnected, R2 (10 kΩ) pulls the ADC pin to ~0 V.
  That decodes to a near-short sender resistance — and because low resistance
  means high temperature / pressure / level for all three senders, it would
  otherwise read a confident full-scale value. `read()` treats `R_sender`
  below the curve's lowest tabulated point × `SENDER_SHORT_FAULT_CURVE_FRACTION`
  (0.4), floored at `SENDER_SHORT_FAULT_MIN_OHM` (2 Ω), as this fault. The
  fraction keeps a margin under the lowest real datasheet point (coolant 51 Ω,
  fuel 17 Ω); only oil pressure's off-scale 0–15 Ω "8 kgf/cm²" corner is
  clipped, which is past the gauge's useful range anyway.

### Failure modes

| # | What's disconnected | ADC pin sits at | Decoded `R_sender` | Result |
|---|---|---|---|---|
| 1 | Sender itself (open sender / broken sender wire), gauge coil still fed | ≈ +12 V (via `R_series` coil, ~110–130 Ω, against the weak 49 kΩ tap divider) | huge | **Clamp to the high-Ω curve end → reads minimum**, matching a stock cross-coil gauge with an open sender. Not a fault. |
| 2 | Gauge +12 V feed | ≈ 0 V (node pulled down through `R_sender`) | ~0 | Fault. Caught by the high-side headroom check when losing that feed also drags `Hw12v` down (`headroom` goes negative); otherwise it looks like mode 3 and the low-side fault catches it. |
| 3 | The divider's input wire (tap to the cluster) | ≈ 0 V (R2 pulls the ADC pin down; nothing pulls it up) | ~0 | **Low-side fault → `Err`**, so the value drops to "no data" instead of pegging full-scale. |

Modes 2 and 3 recover automatically on reconnection (next tick converts
normally), same as every other transient read failure. Mode 1's deliberate
"reads minimum" behavior means it isn't visible as a fault — surfacing a
distinct "sender disconnected" indicator (driven by the `Err` of modes 2/3
plus a *sustained* high-Ω clamp for mode 1) is a follow-up, not part of this
iteration.

**Rejected: stabilizing the OEM supply in hardware.** Would remove the
software problem entirely, but puts a new component in series with the
actual stock gauges' power feed — a regression against this project's
existing passive-tap philosophy (voltage dividers read in parallel, never in
the gauge's power path; the Master warning light is wired straight to GPIO
specifically so it still works with no ADC link). A regulator failing open
would kill all three factory gauges, not just the Pi's copy of the readings
— software compensation fails soft by comparison. Also doesn't fully remove
residual error (a regulator has its own tolerance/ripple), unlike the exact
software relationship derived above.

## Sensor-side representation

New `AnalogSensor` impl, `CalibratedVariableResistanceAnalogSensor` (alongside
`GenericAnalogSensor`'s linear scale), holding:
- `curve: Vec<(f32, f32)>` — `(ohm, value)`, sorted ascending by `ohm`.
- `r_series_ohm: f32` — the measured constant from Conversion pipeline above.
- a handle to the live supply voltage (Cross-sensor dependency, below).
- `value_offset: f32` — a scalar added to the interpolated curve output, from
  the field calibration file (Calibration overlay file below). Defaults to
  `0.0` when no calibration has been captured.

`read()` performs the raw→Ω conversion (Runtime conversion above), finds the
bracketing pair in `curve`, linearly interpolates `value`, then adds
`value_offset` before the `ValueConstraints` min/max clamp. Adding the offset
at read time is equivalent to shifting every curve point's `value` by the same
delta, but keeps the stored `curve` a pristine copy of the datasheet so "reset
to default" is just dropping the offset.

### Cross-sensor dependency: getting the live 12V reading into read()

Unlike every other chain in scope, this sensor's `read()` needs a second live
input beyond its own raw channel — the current `Hw12v` reading, as
`V_supply_now` in the conversion above. `SensorManager`/the chain abstraction
assumes one hardware input feeds one sensor, so this can't be a
trait-signature parameter (would force every `AnalogSensor::read()`
implementor to accept a value it ignores) and can't be a handle back to
`SensorManager` (the sensor lives inside a chain the manager owns, so a live
reference back into `self.sensor_values` from inside a sensor
`SensorManager::read_all_sensors` is calling `&mut self` on isn't something
the borrow checker allows).

**Decision: a small shared cell, not a manager handle.**

- `Arc<AtomicU32>`, holding the current 12V reading as bits
  (`f32::to_bits`/`from_bits`) — `Arc`/atomic rather than `Rc<Cell<_>>`
  because `Box<dyn AnalogSensor + Send>` already requires the sensor to be
  `Send`.
- Created once during chain setup (`main.rs`/`sensor_config.rs`); cloned into
  the writer and into each `CalibratedVariableResistanceAnalogSensor::new(..., v_supply:
  Arc<AtomicU32>, r_series_ohm: f32, curve: Vec<(f32, f32)>, ...)`.
- **Writer:** a thin decorator wrapping the `Hw12v` chain's sensor (same
  pattern as the existing `decorator.rs`), publishing into the `Arc` as a
  side effect of its own `read()` — not `SensorManager` itself, so the
  manager and the `AnalogSensor` trait stay untouched.
- **Ordering:** doesn't matter. Whichever of `Hw12v` or the three calibrated
  sensors happens to run first within `read_all_sensors`'s `analog_sensors`
  loop, the calibrated ones see either this tick's or last tick's voltage —
  at most one tick (~20ms) stale, meaningless against how slowly supply
  voltage actually moves.
- **Startup/failure safety:** initialize the `Arc` to a documented nominal
  (12.2V) so ticks before `Hw12v`'s first successful read get a sane
  `V_supply_now`, not zero (which would make the conversion's denominator
  `V_supply_now − V_sensor_wire` degenerate). Only write on a *successful*
  `Hw12v` read, so an ADC link drop leaves the last-known-good voltage in
  place rather than corrupting every calibrated sensor's reading at once.

This also **updates a claim in `DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md`**: that
doc keeps `EngineTemperatureSensor` and friends hardcoded because "JSON isn't
going to carry that math without an embedded expression language." A
piecewise-linear table needs no expression language — it's just data — so
`kind: "calibrated_analog"` can be a fully data-driven sensor kind alongside
`generic_digital`/`generic_analog`, not a hardcoded exception. Coolant temp,
oil pressure, and fuel level all move to this kind; `SpeedSensor`/`TachoSensor`
(pulse-period math) and the GNSS/UPS sensors are unaffected — their conversion
really is arithmetic, not a lookup, so they stay hardcoded as that doc says.

### JSON shape (fits the existing `sensor_config.json` schema)

```jsonc
{
  "hw_input": "HwCoolantTemp",
  "provider": "adc",
  "analog_processors": [
    { "type": "moving_average", "window": 600 }
  ],
  "sensor": {
    "kind": "calibrated_analog",
    "id": "HwCoolantTemp",
    "name": "ТЕМП ОХЛ",
    "units": "°C",
    "r_series_ohm": 110.4,
    "curve": [
      { "ohm": 1615.0, "value": 30.0 },
      { "ohm": 1050.0, "value": 40.0 },
      { "ohm": 702.5,  "value": 50.0 },
      { "ohm": 482.5,  "value": 60.0 },
      { "ohm": 335.0,  "value": 70.0 },
      { "ohm": 241.0,  "value": 80.0 },
      { "ohm": 175.5,  "value": 90.0 },
      { "ohm": 130.0,  "value": 100.0 },
      { "ohm": 98.0,   "value": 110.0 },
      { "ohm": 75.0,   "value": 120.0 },
      { "ohm": 58.0,   "value": 130.0 }
    ],
    "constraints": { "min": 0.0, "max": 120.0, "warning_high": 105.0, "critical_high": 115.0 }
  }
}
```

`curve` values above are the ТМ106 table's per-point midpoints (min…max
averaged) — the direct transcription this design was meant to enable, not a
placeholder. `curve` needs at least 2 points; load fails otherwise, matching
the existing config loader's fail-fast stance
(`DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md`'s "bad config is a build-time-equivalent
mistake" precedent).

## Calibration overlay file (for the field UI)

**First iteration: a single global offset per sensor, not a multi-point
overlay.** A multi-point overlay that overrides only some datasheet nodes
leaves the rest pulling readings toward datasheet values, and puts slope
discontinuities wherever a corrected segment meets an uncorrected one — a
jagged curve, worst exactly in the coolant alert region (105–115 °C) that
can't be safely field-calibrated in the first place. Sender manufacturing
tolerance, aging, and a slightly-off `r_series_ohm` mostly behave like a
gain+offset on the nominal characteristic rather than per-point noise, so one
anchor measured at an easy operating point captures the dominant term. Whether
that's enough is a question for real-sensor testing, not up-front design.

The field calibration UI should **not** write directly into `sensor_config.json`
— that file is meant to be hand-authored/reviewed (it's the datasheet-derived
default, checked into the repo), and having a UI rewrite it risks losing
comments/formatting or corrupting unrelated entries. Instead:

- A separate `sensor_calibration.json`, keyed by sensor `id`, holding one
  record per calibrated sensor — the value pair from the capture moment:
  ```jsonc
  { "HwCoolantTemp": { "reported": 74.0, "true_value": 90.0 } }
  ```
  `reported` is what the sensor's curve output at that instant (the
  pre-calibration value the operator saw and adjusted); `true_value` is what
  they set it to. The applied correction is
  `value_offset = true_value − reported`, added to every subsequent reading.
- Stored as the value pair, not the bare offset, so the record is
  self-explanatory and re-derivable; stored in the **value domain, not raw ADC
  counts**, for the same reason the curve itself dropped `(raw, value, v_ref)`
  triples — a raw count is meaningless without the supply voltage that
  produced it, whereas `reported` is already voltage-normalized by the time
  `read()` computed it.
- One record per sensor. A second capture overwrites the first; corrections do
  not accumulate.
- Loaded after `sensor_config.json`; for each matching sensor `id`, sets that
  sensor's `value_offset`.
- Not checked into the repo (per-vehicle, per-harness data — `.gitignore`d
  like other machine-specific runtime state), but survives rebuilds/restarts
  since it's a plain file next to `sensor_config.json`.
- "Reset to datasheet default" — delete the file (or the sensor's key).

If real-sensor testing shows a single offset is insufficient — e.g. an error
that grows toward one end of the range, which an offset can't model — the next
step is a two-point capture (offset + scale) or the full multi-point overlay.
The per-sensor record generalizes from one pair to a list without a format
break.

## Field calibration UI sketch

Lives on the diagnostics page (`diag_page.rs`), which already shows live
sensor values — natural place to add a "calibrate" action rather than a new
page:

1. Operator selects a calibrated sensor (coolant temp / oil pressure / fuel
   level) and sees its current live reading.
2. Operator adjusts that displayed value up/down (via the existing
   button-driven input, no keyboard needed) until it matches the known true
   value for the current moment — see Anchor-point procedure per sensor for
   what that moment is per sensor.
3. Confirm records `{ reported: <value shown before adjustment>, true_value:
   <adjusted value> }` under that sensor's `id` in `sensor_calibration.json`,
   and sets the running sensor's `value_offset = true_value − reported`
   immediately — no restart, mirroring the existing hot-reload path
   (`util::shutdown::watch_for_config_update`) already used for
   `sensor_config.json` edits.

No `current_ohm` bookkeeping and no `Hw12v` snapshot are needed for the
first-iteration offset — the correction is applied in the value domain, after
raw→Ω→curve has already normalized for supply voltage. (A future multi-point
overlay working in the Ω domain would need that bookkeeping back; the first
iteration deliberately doesn't.)

## Anchor-point procedure per sensor

This section is about where a *known true value* comes from for each sensor —
the moment the operator can trust a reference reading and press Confirm.

**First iteration needs just one such moment per sensor** (Calibration overlay
file: a single global offset). Pick the easiest to hold steady:
- **Oil pressure** — the warm-idle point with the manometer teed in (lowest
  rpm to hold, and nearest the low-pressure alert that matters).
- **Fuel level** — the Full graduation (least slosh- and tilt-sensitive of the
  three points, most repeatable).
- **Coolant temperature** — the normal-operating 85–95 °C point, read against
  the ECU's own coolant value.

The multi-point tables below stay relevant only if real-sensor testing shows a
single offset isn't enough and the design moves to a two-point or full
multi-point overlay.

### Oil pressure — ММ393А

Reference tool: a mechanical **manometer** in place of (or teed into) the
stock sender's port. Two operating points:

| Point | Expected true pressure | How to hold it |
|---|---|---|
| Idle, warm | > 0.5 bar (≈ 0.5 kgf/cm²) | idle after warm-up |
| 3000 rpm, warm | 2.5–4.0 bar | hold a steady 3000 rpm |

- bar and kgf/cm² differ by ~2% (1 bar = 1.0197 kgf/cm²), well inside this
  sensor's datasheet tolerance — read the manometer in whatever unit it is
  marked and enter that value directly.
- The manometer and the electrical sender usually can't share one port. Either
  tee them so the sender stays live while the manometer reads (preferred —
  Confirm needs the sender's raw ADC value at the same instant), or, if teeing
  is impossible, characterise pressure-vs-rpm with the manometer first, then
  reconnect the sender and re-hit the same rpm points, using rpm as the proxy
  for the just-measured pressure.
- The pressure ranges above are acceptance bands, not the calibration input —
  enter the actual manometer reading at the instant of capture.

### Fuel level — 21213-3827010-01

No bench tool or drain-and-measure procedure is available. **The stock cluster
gauge is the reference**: with the car parked on level ground and the reading
settled (~1 min, for sender damping and fuel slosh), capture a point whenever
the OEM needle sits on a marked graduation.

- Capturing just the Empty and Full graduations is enough; the datasheet
  midpoint carries the rest.
- Expected accuracy is low — OEM cross-coil gauge tolerance plus tank-shape
  non-linearity — so this is a coarse correction of the datasheet curve, not a
  precise calibration.

### Engine coolant temperature — ТМ106

Reference: the **ECU's own coolant-temperature reading** (its tighter-tolerance
sender, section 2 above), read over the diagnostic port. Points of interest:

| Point | True temp | How to reach it |
|---|---|---|
| Cold soak | ambient | before first start of the day, coolant sits at outside-air temperature — enter a measured ambient |
| Normal operating | 85–95 °C | warmed up, thermostat open, fan cycling |
| Overheat / critical | 110–120 °C | opportunistic only — hard idle on a hot day, or a sustained climb; do not force it |

- The critical point matters most for the alert thresholds
  (`warning_high` / `critical_high` in the JSON) but is the hardest to reach
  safely. Take it if the car gets there on its own during testing; otherwise
  leave the datasheet curve covering that end.
- The cold-soak point is free and precise (ambient is easy to measure) and
  anchors the low end, which the ТМ106 is known to be worst at.

## Implementation status

Done (first iteration): the datasheet default curves, the runtime raw→Ω→value
conversion, the live-12 V cross-sensor dependency, and both-ended fault
detection (high-side headroom + low-side short) —
`CalibratedVariableResistanceAnalogSensor` and `SupplyVoltagePublisher` in
`hardware/sensors.rs`, a `calibrated_analog` sensor kind in
`hardware/sensor_config.rs`, and coolant/oil/fuel entries in
`sensor_config.json` (the hand-built `EngineTemperatureSensor` and its
placeholder conversion are removed). Deferred: `sensor_calibration.json`, the
value-offset overlay, the field UI (`value_offset` is carried as a `0.0`
constructor argument so that iteration only adds a loader), and the distinct
"sender disconnected" indicator (see Failure modes).

**Self-test sweep (revised).** `TestADCDataProvider::generate_channels` now
synthesizes each calibrated channel's raw count instead of sharing one 0–4095
ramp: it sweeps that sender's datasheet curve end to end in the Ω domain and
inverts the PA0/PA1/PA2 divider at a fixed mid-band supply, so the ДАВЛ МАСЛА /
УРОВ ТОПЛ / ТЕМП gauges trace their real calibrated ranges during the startup
animation rather than pegging at a curve end or tripping the headroom fault.
`Hw12v` is held at `SELF_TEST_SUPPLY_V` (≈13.5 V) for the sweep so the raw→Ω
inference stays exact — a moving supply would smear it through the `Hw12v`
moving-average. The Ω→raw inverse (`calibrated_sender_raw_from_ohm`) and the
`Hw12v` volts→raw inverse (`v12_raw_from_volts`) are `pub` in
`hardware/sensors.rs`, exposed for the self-test the same way
`speed_period_raw_from_kmh` already is; the sweep's per-sender curve endpoints
and `r_series` mirror `sensor_config.json` (a curve edit there must be mirrored
in the `SELF_TEST_*_OHM_SPAN` constants). The `hardware/sensors.rs` tests reuse
the same `calibrated_sender_raw_from_ohm` to drive a sensor from a known
resistance.

---
*Created: September 6, 2026*
*Revised: September 7, 2026 — curve representation changed from raw-ADC-based
`(raw, value, v_ref)` points to Ω-based `(ohm, value)` points; see Decisions.*
*Revised: September 8, 2026 — resolved the anchor-point procedure open question;
see Anchor-point procedure per sensor.*
*Revised: September 8, 2026 — first-iteration field calibration reduced to a
single global value offset per sensor (a `reported`/`true_value` pair applied
to the whole curve); multi-point overlay deferred pending real-sensor testing.*
*Revised: September 8, 2026 — default curves implemented
(`CalibratedVariableResistanceAnalogSensor`, `calibrated_analog` config kind);
`EngineTemperatureSensor` removed. See Implementation status.*
*Revised: September 8, 2026 — added a low-side (short/floating-input) fault to
`read()` and a Failure modes table; see Runtime conversion.*
*Revised: September 8, 2026 — self-test sweep now synthesizes per-sender raw
counts from the datasheet curves via `pub` Ω→raw / volts→raw inverses in
`hardware/sensors.rs` (supersedes the earlier "no shared helper needed"
resolution); see Implementation status.*
