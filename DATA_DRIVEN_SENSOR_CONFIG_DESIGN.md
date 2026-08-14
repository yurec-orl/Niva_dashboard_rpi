# Data-Driven Sensor Chain Configuration — Implementation Plan

## Problem

`main.rs::add_adc_sensor_chains` (and, to a lesser extent, `setup_sensors`/
`setup_button_sensors`) hand-builds ~35 `SensorDigitalInputChain`/`SensorAnalogInputChain`
instances as inline Rust, most of them near-identical: an `HWInput`, a provider, a
debounce/averaging processor with hand-picked constants, and a `Generic*Sensor` with
inline `ValueConstraints`. Tuning a debounce count or a warning threshold means editing
and rebuilding the binary. This is also the root of the separate TODO item "sensor →
watchdog → alert construction is a multi-step process ... easy to mismatch one of the
parameters" — the chain-building boilerplate is where those mismatches happen.

Goal: move the *parameters* of chain construction into a JSON file, without trying to make
the whole sensor layer data-driven — some chains genuinely can't be, see Scope below.

## Scope

**In scope** — every chain in `add_adc_sensor_chains` plus the button chains in
`setup_button_sensors` (~27 chains total): all are `ADCChannelProvider` +
`Generic{Digital,Analog}Sensor`, differing only in `HWInput`, processor params, and
`ValueConstraints`/scale/name/units. These become fully data-driven.

**Out of scope, stays hardcoded:**
- **Custom conversion math**: `SpeedSensor`, `TachoSensor`, `EngineTemperatureSensor`,
  `GnssAltitudeSensor`, `UpsCurrentSensor`, `UpsChargeSensor` implement `AnalogSensor::read()`
  with real logic (inter-pulse-period conversion, calibration curves), not a linear scale.
  JSON isn't going to carry that math without an embedded expression language, which isn't
  worth building for six sensors. Their *processor* params (moving-average window etc.)
  are still worth pulling into config — see Stretch goal below.
- **Non-ADC providers**: `GnssChannelProvider`, `UPSDataProvider`, `Bno085ChannelProvider`,
  and the three `*LinkStatusProvider`s are constructed from live `GnssFrame`/`UpsRawFrame`/
  `Bno085Frame`/`Option<ADCFrame>` handles that only exist once their background thread has
  started. Chain *assembly* for these stays in `main.rs`; only their tunable numbers move
  to config, same as the custom sensors above.
- `HeadingFusionSensor` bypasses the chain abstraction entirely — untouched.

## JSON schema

One entry per chain, in a flat array (order doesn't matter — this replaces call-site
ordering with a `hw_input` key). Digital and analog chains share a shape; the sensor
sub-object's tag picks which.

```jsonc
[
  {
    "hw_input": "HwParkBrake",
    "provider": "adc",
    "digital_processors": [
      { "type": "debounce", "stable_count": 5, "stable_delay_ms": 50 }
    ],
    "sensor": {
      "kind": "generic_digital",
      "id": "HwParkBrake",
      "name": "СТОЯН ТОРМ",
      "active_level": "high",
      "constraints": { "digital_preset": "warning" }
    }
  },
  {
    "hw_input": "HwFuelLvl",
    "provider": "adc",
    "analog_processors": [
      { "type": "moving_average", "window": 3600 }
    ],
    "sensor": {
      "kind": "generic_analog",
      "id": "HwFuelLvl",
      "name": "УРОВ ТОПЛ",
      "units": "%",
      "scale": 0.1,
      "constraints": {
        "min": 0.0, "max": 100.0,
        "warning_low": 10.0, "critical_low": 20.0
      }
    }
  }
]
```

Field notes, chosen to mirror the existing Rust types 1:1 so the loader is a thin
translation, not a reinterpretation:

- `hw_input`: string name of an `HWInput` variant. Validated at load time against a manual
  string↔enum table (see below) — unknown names fail load with the offending string named
  in the error, not a silent skip.
- `provider`: `"adc"` for everything in scope. Kept as a field (not assumed) so the schema
  doesn't need reshaping when non-ADC providers are folded in later (Stretch goal).
- `digital_processors` / `analog_processors`: ordered arrays, applied in sequence — mirrors
  `Vec<Box<dyn ...Processor>>` today. Each entry's `type` selects the constructor:
  - digital: `"debounce"` → `DigitalSignalDebouncer::new(stable_count: u8, Duration::from_millis(stable_delay_ms))`
  - analog: `"moving_average"` → `AnalogSignalProcessorMovingAverage::new(window: usize)`,
    `"dampener"` → `AnalogSignalProcessorDampener::new(alpha: f32)`
- `sensor.kind`: `"generic_digital"` or `"generic_analog"` in scope (maps straight to
  `GenericDigitalSensor::new`/`GenericAnalogSensor::new`); the Stretch goal below adds more
  kinds without changing this field's role.
- `sensor.constraints`: either the raw six `ValueConstraints` fields (`min`, `max`,
  `critical_low`, `warning_low`, `warning_high`, `critical_high`, all but `min`/`max`
  optional) or a `digital_preset` string (`"default"` | `"warning"` | `"critical"`) that maps
  to `ValueConstraints::digital_default()/digital_warning()/digital_critical()` — keeps the
  common digital-indicator case as terse in JSON as it is in Rust today.
- `active_level`: `"high"` | `"low"` → `Level::High`/`Level::Low`.

## `HWInput` string mapping

`HWInput` is a plain closed enum (~35 variants, no `Display`/`FromStr`). Rather than pull
in `strum` for this, add a manual `HWInput::from_config_name(&str) -> Option<Self>` next to
the existing `adc_channel()` method in `hw_providers.rs` — same style as that match, one arm
per variant, compiler-enforced exhaustiveness via a trailing `_ => None` that we deliberately
*don't* add (use a `match` with every variant listed so adding a new `HWInput` variant
without updating this table is a compile error, not a silent gap).

## Module layout

New file: `src/hardware/sensor_config.rs`

```rust
#[derive(Deserialize)]
struct ChainConfig {
    hw_input: String,
    provider: String,
    #[serde(default)]
    digital_processors: Vec<DigitalProcessorConfig>,
    #[serde(default)]
    analog_processors: Vec<AnalogProcessorConfig>,
    sensor: SensorConfig,
}
// + DigitalProcessorConfig, AnalogProcessorConfig, SensorConfig, ConstraintsConfig
// as tagged enums/structs (serde(tag = "type") / serde(tag = "kind"))

pub fn load_adc_chains(path: &Path, frame: ADCFrame, mgr: &mut SensorManager) -> Result<(), String>
```

`load_adc_chains` parses the file, and for each entry:
1. Resolves `hw_input` via `HWInput::from_config_name`, erroring with the raw string on miss.
2. Builds `ADCChannelProvider::new(input, frame.clone())` (the only `provider` value in
   scope for now — non-`"adc"` values are a load error, not silently ignored).
3. Builds the processor `Vec<Box<dyn _>>` from `digital_processors`/`analog_processors`.
4. Builds the sensor from `sensor.kind` + fields.
5. Wraps into `SensorDigitalInputChain`/`SensorAnalogInputChain` (digital vs analog decided
   by which processor/sensor kind was present — a chain with both or neither is a load
   error) and calls `mgr.add_*_sensor_chain`.

`add_adc_sensor_chains` in `main.rs` shrinks to a call to `load_adc_chains` with a fixed
path; `setup_button_sensors`'s loop becomes 8 more entries in the same JSON file (or a
second file — see Open questions) instead of a second hand-written loop.

## Config file location

Runtime-loaded external file, not `include_str!`'d — the entire point is tuning without a
rebuild. Given `graphics/default_style.json` is on record (in this repo's TODO list) as a
config file that was added but never actually wired to a loader, the implementation is not
"done" until `add_adc_sensor_chains`/`setup_button_sensors` actually call `load_adc_chains`
and the old inline chain-construction code is deleted in the same change — not left as dead
parallel code.

Suggested path: alongside the binary / repo root, e.g. `sensor_config.json`, resolved the
same way other runtime paths in this project are (check `util::` for an existing
"config directory" convention before inventing a new one).

## Migration steps

1. Add `HWInput::from_config_name` in `hw_providers.rs` (mechanical, exhaustive match).
2. Add `hardware/sensor_config.rs` with the `Deserialize` structs and `load_adc_chains`,
   covering `generic_digital`/`generic_analog` + `debounce`/`moving_average`/`dampener` +
   `"adc"` provider only.
3. Write `sensor_config.json` by transcribing the current literal values out of
   `add_adc_sensor_chains` and `setup_button_sensors` — this step is a pure value copy, no
   behavior change, so it's the easiest point to verify nothing drifted (diff old vs. new
   `SensorValue`s at runtime, or eyeball a `git show` of the deleted block against the JSON).
4. Replace `add_adc_sensor_chains`'s body with the `load_adc_chains` call; delete the old
   inline chain literals. Fold `setup_button_sensors`'s loop into the same file/call.
5. `cargo test` (existing `sensor_manager.rs` unit tests are unaffected — they build chains
   directly, not through config) + a manual run comparing dashboard readings against a
   pre-change build for a few chains (fuel level, park brake, 12V) to catch transcription
   errors from step 3.
6. Update this repo's TODO list: mark the "data-driven sensor creation" item done for the
   ADC-backed subset, and note the Stretch goal below as the remaining piece.

## Stretch goal (separate follow-up, not this pass)

Extend `sensor.kind` to also cover `speed`, `tacho`, `engine_temp`, `gnss_altitude`,
`ups_current`, `ups_charge` — the loader would build the corresponding hardcoded sensor
struct (`SpeedSensor::new()` etc., which take no config today) but still read
`digital_processors`/`analog_processors`/`constraints` from JSON for them, same as the
generic kinds. This gets the moving-average windows for speed/tacho/temperature (currently
magic numbers in `main.rs`, e.g. `AnalogSignalProcessorMovingAverage::new(10*60)` for
coolant temp) under config too, without touching the conversion math itself. Needs the
`provider` field extended to `"gnss"`/`"ups"`/`"bno085"` and `load_*` functions taking the
relevant frame handle(s), mirroring `load_adc_chains`'s shape.

## Open questions for the user

- One JSON file for everything in scope (ADC digital + analog + buttons), or split
  buttons into their own file since they're loaded by a separate `SensorManager`
  (`setup_button_sensors`) with no `ADCFrame`-optionality branching in common with the
  main sensor set? Leaning toward one file, two lookups by a `role`/`group` field, to
  avoid maintaining two near-identical schemas.
- Fail-fast (panic / abort startup) vs. fail-soft (log + skip the one bad entry) on a
  malformed JSON entry. Existing code has no precedent for "sensor missing at startup"
  being non-fatal — recommend fail-fast, since a bad config here is a build-time-equivalent
  mistake, not a runtime hardware absence (which the `Option<ADCFrame>` path already
  handles separately).
