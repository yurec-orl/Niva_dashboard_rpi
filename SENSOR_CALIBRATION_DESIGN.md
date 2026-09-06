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
  anchor point (see Conversion pipeline below, revised).
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
  that section's worst-case linear model is now confirmed as *the* model,
  not a pessimistic bound.

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

- **Default curves: computed from datasheet + divider math**, not bench- or
  field-measured up front. Fastest path to a real (non-placeholder) curve;
  explicitly approximate, refined later via the calibration UI once the car
  exists to check readings against.
- **Curve representation: piecewise-linear point table**, not a fitted
  polynomial/Steinhart-Hart curve. Matches the datasheet tables point-for-point,
  needs no expression language, and a calibration-UI edit is just "insert/
  adjust one `(raw, value)` point" — the datasheet tolerance bands (15–40%
  wide) already dwarf whatever smoothness a curve fit would add.

## Conversion pipeline and R_series

Full path from sender resistance to raw ADC count, per the PA0/PA1/PA2 divider
in `stm32_adc_module/WIRING.md`:

```
R_sender (Ω, from datasheet table)
  → V_sensor_wire = V_supply × R_sender / (R_series + R_sender)
  → V_adc_pin     = V_sensor_wire × R2/(R1+R2)      [10/49, fixed, known]
  → raw           = V_adc_pin / V_ref × 4095         [12-bit, V_ref ≈ 3.3V]
```

`R1`/`R2` (39kΩ/10kΩ) are known. `R_series` is the cross-coil gauge's variable
coil winding resistance (see Circuit context above) — fixed and physical.
`V_supply` is not fixed at all — see Problem: floating supply voltage below.

**`R_series` measured directly, per gauge** (multimeter across the shared
+12V supply pin and each gauge's own sender-wire pin, on a spare cluster
pulled from the car — see procedure below the table):

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

With `R_series` now known:

1. Pick a nominal `V_supply` for the default curve — 12.2V ("moderately
   charged battery, engine off," the easiest state to reproduce) — and record
   it as that curve's `v_ref` (see Fix section below; this is exactly the
   value the runtime compensation needs to know).
2. Run the datasheet's Ω values through the now-fully-determined pipeline to
   generate the curve's `(raw, value)` points.

This produces a **curve accurate at one specific `V_supply`** — it says
nothing about behavior away from that reference voltage, which is what the
next section is about.

## Problem: floating supply voltage

The car's 12V rail is not fixed — it runs roughly 12.2V (moderately charged
battery, engine off) to 14.5V (alternator charging), a **+18.85%** swing.
Every calibration curve above is anchored to whatever `V_supply` happened to
be at the moment of its anchor measurement. If the real supply differs from
that anchor value while the engine is running, every reading built from these
three inputs is systematically off — and the size of that error is worse than
the raw voltage swing itself, for reasons below.

### The swing is exact and R_series-independent for the raw signal itself

For a fixed sender resistance, node voltage (and therefore raw ADC count)
scales **exactly linearly** with `V_supply` — this falls straight out of the
divider equation in the Conversion pipeline section above, and holds
regardless of `R_series`'s (unknown) value:

```
raw(R_sender, V_supply) = V_supply × [R_sender / (R_series + R_sender)] × (R2/(R1+R2)) × (4095/V_ref)
```

Everything in brackets and after is fixed for a given `R_sender`; only
`V_supply` varies. So `raw(R, V2) / raw(R, V1) = V2 / V1` exactly — a 12.2V→
14.5V swing produces an **18.85% raw-count error**, full stop, independent of
`R_series`, as long as neither divider stage saturates or hits the Zener
clamp.

### But the error in the *reported physical value* is worse, and depends on R_series

The calibration curve inverts a ratiometric fraction, `f = R_sender/(R_series
+ R_sender)`, back to a physical value assuming a fixed nominal `V_supply`.
When the real supply differs by factor `α = V_actual/V_calibration`, the
*apparent* fraction becomes `f_apparent = α · f_true` — and because `f` is a
ratio, not a percentage, converting that back into an apparent resistance
(`R_apparent = R_series · f_apparent/(1 − f_apparent)`) is **not** an 18.85%
error in Ω. The amplification depends on `R_series` — now measured (previous
section) rather than guessed.

Worked example using the **measured** `R_series` values, showing apparent
reading if calibrated at 12.2V but actually reading at 14.5V (still a
representative-point illustration — one sample resistance per sensor, not a
full curve — but no longer using a guessed `R_series`):

| Sensor | True point | True Ω | R_series (measured) | Apparent Ω at 14.5V | Apparent reading | Error |
|---|---|---|---|---|---|---|
| Coolant temp (ТМ106) | 90 °C | 175.5 | 110.4 | 297.9 | ~74.0 °C | **−16.0 °C** |
| Oil pressure (ММ393А) | 4 kgf/cm² | 118 | 130.8 | 169.0 | ~2.91 kgf/cm² | **−1.09 kgf/cm²** |
| Fuel level (21213) | 50% | 66 | 124.4 | 87.2 | ~44.3% | **−5.7 pts** |

The oil pressure case is the one that matters most: a ~1.1 kgf/cm² swing is
large next to the "critical `<1 kgf/cm²` at idle" threshold already
documented for this sensor (`PROJECT_CONTEXT.md`) — a supply-voltage-driven
error in either direction could mask a real low-oil-pressure condition or
trigger a false alarm, depending on which way the engine happens to be
running relative to the anchor voltage.

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

## Fix: compensate using the existing 12V channel

The `raw ∝ V_supply` relationship is exact and needs no knowledge of
`R_series` to *undo* — and the dashboard already measures system voltage
independently, on PA3 (`HWInput::Hw12v` / `AdcChannel::Voltage12V`, per
`stm32_adc_module/WIRING.md`'s PA3 divider). No new hardware is needed.

Before running a raw count from any calibrated-curve sensor (coolant temp,
oil pressure, fuel level) through its curve, scale it:

```
corrected_raw = raw_measured × (V_calibration_reference / V_measured_now)
```

- `V_measured_now` — read live from the `Hw12v` chain on every tick.
- `V_calibration_reference` — the system voltage recorded *at the time each
  curve's anchor point was captured* (bring-up procedure above, and later any
  field-UI-captured point), stored alongside the curve/points so the
  correction is anchored to the right reference, not a hardcoded constant.

This removes the error identified above **without ever needing to
characterize `R_series`** — it corrects the one thing that's exactly known
(the linear relationship to `V_supply`) rather than trying to model the
unknown OEM resistance more precisely. It should be treated as a required
step in the conversion pipeline for all three calibrated sensors, not an
optional refinement — implemented once, in the shared curve-lookup code
(`GenericCalibratedAnalogSensor::read`), not per-sensor.

Consequence for the calibration data format: each curve (default and
overlay) needs to record the reference voltage its points were taken at, not
just `(raw, value)`:

```jsonc
"curve": [
  { "raw": 3550, "value": 30.0, "v_ref": 12.2 },
  { "raw": 90,   "value": 130.0, "v_ref": 12.2 }
]
```

`v_ref` defaults to a documented nominal (e.g. 12.2V, "moderately charged
battery, engine off" — the easiest state to reliably reproduce when taking an
anchor reading) if omitted, so hand-authored default curves that predate this
field don't break.

## Sensor-side representation

New `AnalogSensor` impl, `GenericCalibratedAnalogSensor` (alongside
`GenericAnalogSensor`'s linear scale), holding a `Vec<(u16, f32, f32)>`
(raw, value, `v_ref`) sorted ascending by raw count. `read()` applies the
voltage-compensation fix above first, then finds the bracketing pair and
linearly interpolates; a raw value past either end clamps to that end's value
(same spirit as `ValueConstraints` min/max clamping elsewhere) rather than
extrapolating past datasheet-covered territory.

### Cross-sensor dependency: getting the live 12V reading into read()

Unlike every other chain in scope, this sensor's `read()` needs a second live
input beyond its own raw channel — the current `Hw12v` reading, for the
compensation step. `SensorManager`/the chain abstraction assumes one hardware
input feeds one sensor, so this can't be a trait-signature parameter (would
force every `AnalogSensor::read()` implementor to accept a value it ignores)
and can't be a handle back to `SensorManager` (the sensor lives inside a
chain the manager owns, so a live reference back into `self.sensor_values`
from inside a sensor `SensorManager::read_all_sensors` is calling `&mut self`
on isn't something the borrow checker allows).

**Decision: a small shared cell, not a manager handle.**

- `Arc<AtomicU32>`, holding the current 12V reading as bits
  (`f32::to_bits`/`from_bits`) — `Arc`/atomic rather than `Rc<Cell<_>>`
  because `Box<dyn AnalogSensor + Send>` already requires the sensor to be
  `Send`.
- Created once during chain setup (`main.rs`/`sensor_config.rs`); cloned into
  the writer and into each `GenericCalibratedAnalogSensor::new(..., v_ref:
  Arc<AtomicU32>)`.
- **Writer:** a thin decorator wrapping the `Hw12v` chain's sensor (same
  pattern as the existing `decorator.rs`), publishing into the `Arc` as a
  side effect of its own `read()` — not `SensorManager` itself, so the
  manager and the `AnalogSensor` trait stay untouched.
- **Ordering:** doesn't matter. Whichever of `Hw12v` or the three calibrated
  sensors happens to run first within `read_all_sensors`'s `analog_sensors`
  loop, the calibrated ones see either this tick's or last tick's voltage —
  at most one tick (~20ms) stale, meaningless against how slowly supply
  voltage actually moves.
- **Startup/failure safety:** initialize the `Arc` to the curve's own nominal
  `v_ref` (12.2V) so ticks before `Hw12v`'s first successful read get a
  neutral compensation factor of 1.0, not a divide-by-zero. Only write on a
  *successful* `Hw12v` read, so an ADC link drop leaves the last-known-good
  voltage in place rather than corrupting every calibrated sensor's reading
  at once.

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
    "curve": [
      { "raw": 3550, "value": 30.0, "v_ref": 12.2 },
      { "raw": 2500, "value": 40.0, "v_ref": 12.2 },
      { "raw": 1750, "value": 50.0, "v_ref": 12.2 },
      { "raw": 90,   "value": 130.0, "v_ref": 12.2 }
    ],
    "constraints": { "min": 0.0, "max": 120.0, "warning_high": 105.0, "critical_high": 115.0 }
  }
}
```

`curve` needs at least 2 points; load fails otherwise, matching the existing
config loader's fail-fast stance (`DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md`'s
"bad config is a build-time-equivalent mistake" precedent).

## Calibration overlay file (for the field UI)

The field calibration UI should **not** write directly into `sensor_config.json`
— that file is meant to be hand-authored/reviewed (it's the datasheet-derived
default, checked into the repo), and having a UI rewrite it risks losing
comments/formatting or corrupting unrelated entries. Instead:

- A separate `sensor_calibration.json`, keyed by sensor `id`, holding only
  calibration-point overrides:
  ```jsonc
  { "HwCoolantTemp": [ { "raw": 3480, "value": 30.0 } ] }
  ```
- Loaded after `sensor_config.json`; for each matching sensor `id`, its points
  are merged into the base curve — replacing any existing point at the same
  `raw` (within a small tolerance) and inserting new ones, then re-sorting.
- Not checked into the repo (per-vehicle, per-harness data — `.gitignore`d
  like other machine-specific runtime state), but survives rebuilds/restarts
  since it's a plain file next to `sensor_config.json`.
- Gives a trivial "reset to datasheet default" — delete the file.

## Field calibration UI sketch

Lives on the diagnostics page (`diag_page.rs`), which already shows live
sensor values — natural place to add a "capture calibration point" action
rather than a new page:

1. Operator selects a calibrated sensor (coolant temp / oil pressure / fuel
   level) and sees its current live raw count.
2. Operator enters the known true physical value for the current moment
   (typed via the existing button-driven input, no keyboard needed — same
   affordance style as other diag-page interactions).
3. Confirm writes/updates `(current_raw, entered_value, current_v_ref)` into
   `sensor_calibration.json` — `current_v_ref` read from the live `Hw12v`
   chain at the same instant, not asked of the operator — and applies it to
   the running sensor's curve immediately (no restart needed) — mirrors the
   existing hot-reload pattern (`util::shutdown::watch_for_config_update`)
   already used for `sensor_config.json` edits.

## Open questions

- Exact anchor-point procedure per sensor (what's the practical "known true
  value" moment for oil pressure and fuel level, which don't have as easy a
  reference as ambient-temp-at-cold-start) — needs deciding once the car is
  available to test against, not before.
- Whether `raw`-based curve points (this doc's choice, since that's what the
  sensor actually reads and what a live capture naturally produces) should
  also carry the corresponding Ω value as a comment/second field for
  traceability back to the datasheet table — leaning yes, as a non-functional
  `_ohm_reference` field for humans reading the JSON later, not consumed by
  the loader.

---
*Created: September 6, 2026*