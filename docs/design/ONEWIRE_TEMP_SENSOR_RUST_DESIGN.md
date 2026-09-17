# One-Wire Temperature Sensors — Rust / Dashboard Side

Companion to `ONEWIRE_TEMP_SENSOR_DESIGN.md` (firmware + wire protocol, **[Done]**). That
document deliberately deferred the Rust side; this is it.

Firmware recap (the contract this design consumes):

```
$T,<rom>:<raw>;<rom>:<raw>;...\n
```

- `<rom>` — 16 lowercase hex chars, the 8 ROM bytes in device order. Used **verbatim** as
  the map key; byte order is fixed.
- `<raw>` — signed decimal, DS18B20 temperature register in 1/16 °C. `°C = raw / 16.0`.
  Range at 10-bit resolution lands on 4-unit (`0.25 °C`) steps. Negative values occur
  (`-88` → −5.5 °C).
- A sensor failing CRC that cycle is **omitted** — no placeholder. Gaps are the
  dashboard's problem (per-address staleness).
- Zero sensors on the bus still emits `$T\n` every cycle ("bus alive, nothing found").
- Cadence ≈ 1 Hz (10-bit), interleaved between the 50 Hz `$…` telemetry frames.

## Scope

**Part 1 — [Done].** Wire reader, `AdcTempFrame`, two logical inputs (`HwTempOut` /
`HwTempInt`), `OneWireTempChannelProvider`, `OneWireTempSensor`, `sensor_config.json`
wiring, self-test coverage, log-spam suppression for a stale/absent DS18B20.

**Part 2 — [Basic done].** `ТЕМП` page (`page_framework/temp_page.rs`, `TEMP_PAGE_ID = 8`)
registered unconditionally, reached from `MainPage`'s `Left3` slot ("ТЕМП"). Renders
threshold-coloured text rows, "НЕТ СВЯЗИ" per stale row, "НЕТ ДАТЧИКОВ" when nothing has a
value. Richer indicator/decorator layout still open — see below.

Non-goals: the analog PA2 coolant sensor (`EngineTemperatureSensor`) is **untouched** and
stays a separate chain. Runtime hot-plug — the firmware discovers once at boot, so does the
dashboard.

---

## Part 1 — protocol reader → logical sensor

### 1. `$T` line parsing in `ADCDataProvider::run_loop`

Today the read loop strips a leading `$`, `split(',')`, `filter_map(parse::<u16>)`. A `$T`
line survives that untouched: no token parses as `u16`, `values.is_empty()`, frame not
updated (this is what makes the line inert on firmware that predates this feature).

Add an explicit branch **before** the generic parse, as a sibling of the existing
`Some(line)` arm's body:

```rust
Some(line) if !line.is_empty() => {
    if line == "$T" || line.starts_with("$T,") {
        Self::parse_temp_line(line, temp_frame);
    } else {
        // …existing generic CSV parse into `frame`…
    }
}
```

`parse_temp_line`:

- `line.strip_prefix("$T")` then `strip_prefix(',')` — bare `$T` (no comma) is the
  zero-sensor keepalive: bump the bus-level "last line" timestamp, touch no addresses,
  return.
- Split the remainder on `';'`; for each non-empty pair split once on `':'`.
- ROM half: must be exactly 16 ASCII hex chars (validate; a malformed pair is skipped, not
  fatal — the link is noisy by design). Kept as a lowercase `String`.
- Value half: `parse::<i16>()` (**not `u16`** — negatives are real).
- On a fully parsed line, replace/insert each `(rom, TempReading { raw_16ths, updated: now })`
  and bump the bus timestamp.

**Do not** call `frame.touch()` here. `$T` arriving says nothing about the `$…` telemetry
frame; keeping the two timestamps independent is what lets `HARD_RESET_STALE_THRESHOLD`
still fire the USB power-cycle recovery if *only* temp lines are getting through.

No Rust-side CRC or retry logic — the STM32 already drops bad scratchpad reads; per-address
staleness (below) absorbs the resulting gaps.

### 2. `AdcTempFrame`

A new cloneable handle in `adc_data_provider.rs`, shaped like `ADCFrame` /
`OscFrame` (background thread writes, UI thread reads), but address-keyed and signed rather
than a positional `Vec<u16>`:

```rust
#[derive(Clone, Copy)]
struct TempReading { raw_16ths: i16, updated: Instant }

#[derive(Clone)]
pub struct AdcTempFrame {
    readings: Arc<Mutex<HashMap<String, TempReading>>>, // key = 16-char lowercase ROM hex
    last_line: Arc<Mutex<Instant>>,                     // any $T line (incl. bare keepalive)
}
```

API:

| method | purpose |
|---|---|
| `fresh_raw(rom: &str) -> Option<i16>` | reading for `rom` if seen within `TEMP_ADDR_MAX_AGE`, else `None` |
| `bus_last_line_age() -> Duration` | drives an optional bus-health pseudo-sensor / the page's "no bus" state |
| `addresses() -> Vec<String>` | commissioning: list every ROM seen this session (for the ADC terminal page dump) |
| `update_reading` / `touch_line` | writer side, `pub(crate)`, called only from `run_loop` / the self-test writer |

Constants (in `adc_data_provider.rs`, next to `ADC_LINK_MAX_AGE`):

```rust
/// A DS18B20 address is considered stale after this long without a fresh $T reading for it.
/// ≈1 Hz cadence → this is ~5 missed cycles. Deliberately far longer than ADC_LINK_MAX_AGE
/// (500 ms, the telemetry-frame threshold): a single dropped CRC must not blank the display.
pub const TEMP_ADDR_MAX_AGE: Duration = Duration::from_secs(5);
```

`AdcTempFrame` is **not** merged into `ADCFrame`: different key model (ROM string vs.
positional index), different value type (`i16` vs. `u16`), and it needs per-key timestamps
that `ADCFrame`'s single `last_update` can't express. Same reasoning that keeps `GnssFrame`
/ `Bno085Frame` separate.

`ADCDataProvider` gains an `AdcTempFrame` field + `pub fn temp_frame(&self) -> AdcTempFrame`,
mirroring `frame()` / `osc_frame()`, and threads it into `run_loop`.

### 3. `HWInput` additions + the ROM → input map

Two new variants in `hw_providers.rs`:

```rust
// One-Wire (DS18B20) bus temperatures — see ONEWIRE_TEMP_SENSOR_RUST_DESIGN.md. Read by
// ROM address from AdcTempFrame, not by positional ADC channel, so adc_channel() returns
// None for these.
HwTempOut,   // outside air
HwTempInt,   // cabin / interior
```

Touch points (all already documented patterns):
- `config_name()` — add the two arms (compile-enforced exhaustive, so this *must* be done).
- `ALL` — add the two entries (manual list).
- `adc_channel()` — no change; falls through to `None`.

**The bus is discovery-driven; the dashboard's consumed set is not.** Logical temp readings
are a fixed enum, exactly like every other `HWInput` (the `SensorManager` value map,
`Watchdog`, page reads all key off it). Config maps a ROM string to one of these variants;
any address on the bus without a mapping is ignored. Adding oil / gearbox / etc. later =
new variant + new config entry.

Initial map (from the live bench bus, `$T,2854df6b000000d9:404;28cb586a00000059:408;`):

| ROM | input | label (RU) |
|---|---|---|
| `2854df6b000000d9` | `HwTempOut` | `НАРУЖ` |
| `28cb586a00000059` | `HwTempInt` | `САЛОН` |

Lives in `sensor_config.json` (see §6), not in code.

### 4. `OneWireTempChannelProvider`

New `HWAnalogProvider` in `hw_providers.rs`, one instance per logical temp input, modelled
on `GnssChannelProvider` (thin wrapper over a shared frame, resolves a named thing rather
than a channel index):

```rust
pub struct OneWireTempChannelProvider {
    input: HWInput,
    rom: String,          // 16-char lowercase hex, from config
    frame: AdcTempFrame,
}

impl HWAnalogProvider for OneWireTempChannelProvider {
    fn input(&self) -> HWInput { self.input }
    fn read_analog(&self, _input: HWInput) -> Result<u16, String> {
        self.frame.fresh_raw(&self.rom)
            .map(|raw| raw as u16)   // i16 bit-pattern through the u16 transport, see below
            .ok_or_else(|| format!("no fresh DS18B20 reading for {}", self.rom))
    }
}
```

The `raw as u16` reinterpret is the same trick `UPSDataProvider` → `UpsCurrentSensor` uses
for the INA219's signed current register: the `HWAnalogProvider` boundary is `u16`, so the
signed 16-bit value rides across as its bit pattern and `OneWireTempSensor` reverses it. A
missing/stale address returns `Err`, which `SensorManager::read_all_sensors` already
tolerates per-chain (like a GNSS field with no current fix) — the chain simply has no value
that cycle and the indicator renders stale.

### 5. `OneWireTempSensor`

New `AnalogSensor` in `sensors.rs`. A code struct holding the conversion, same category as
`GenericAnalogSensor` — it is *not* hardcoded into `main.rs` like `SpeedSensor`; it is
instantiated by the config loader (§6). It exists as its own type only because the
conversion is more than a scale factor:

```rust
impl AnalogSensor for OneWireTempSensor {
    fn read(&mut self, input: u16) -> Result<&SensorValue, String> {
        let celsius = (input as i16) as f32 / 16.0;   // undo the transport reinterpret
        self.value = SensorValue::analog_with_constraints_and_metadata(
            celsius.clamp(self.min_value(), self.max_value()),
            self.constraints.clone(), self.metadata.clone(),
        );
        Ok(&self.value)
    }
}
```

`new(id, name, constraints)` — unit is always `"°C"`, no scale argument (the `/ 16.0` is
intrinsic to the DS18B20 format). Constraints (min/max + warning/critical thresholds) come
from config so oil-temp vs. cabin-temp bands differ without new code.

### 6. `sensor_config.rs` — extend `load_chains` in place

`load_chains` is small; extend it rather than adding a parallel loader. Changes:

**`ChainConfig`** gains an optional address field (top-level, sibling of `hw_input` /
`provider`, since it parametrizes the *provider*, analogous to how the ADC provider derives
its channel from `HWInput`):

```rust
#[serde(default)]
rom: Option<String>,
```

**`build_chain`** — the `provider` guard currently hard-rejects anything but `"adc"`. Add
an `"adc_temp"` arm. It needs the `AdcTempFrame`, so `load_chains` / `build_chain` take one
more argument:

```rust
pub fn load_chains(path, group, frame: ADCFrame, temp_frame: AdcTempFrame, mgr) -> Result<(), String>
```

(Callers that have no real temp bus — none currently — would pass a detached
`AdcTempFrame::new()`; in practice `main.rs` always has one from `ADCDataProvider`.)

**`SensorConfig`** gains a variant:

```rust
OneWireTemp {
    id: String,
    name: String,
    constraints: ConstraintsConfig,   // Raw form; digital presets rejected as before
},
```

`build_chain` for an `adc_temp` + `one_wire_temp` entry:
- require `rom` present and 16 hex chars → else a fail-fast load error naming `hw_input`
  (consistent with the existing "unknown hw_input" / "processor kind mismatch" errors).
- reject `digital_processors` (same guard the analog branch already has).
- `analog_processors` still allowed — an optional `moving_average` / `dampener` for the
  ~1 Hz stream flows through the existing `SensorAnalogInputChain` unchanged.
- build `SensorAnalogInputChain::new(OneWireTempChannelProvider::new(input, rom, temp_frame),
  processors, OneWireTempSensor::new(id, name, constraints))`.

An `adc_temp` provider with a non-`one_wire_temp` sensor kind (or vice versa) is a
fail-fast mismatch error, mirroring the existing digital/analog cross-checks.

New JSON entries (`group: "sensor"`):

```json
{
  "group": "sensor", "hw_input": "HwTempOut", "provider": "adc_temp",
  "rom": "2854df6b000000d9",
  "analog_processors": [ { "type": "moving_average", "window": 4 } ],
  "sensor": {
    "kind": "one_wire_temp", "id": "HwTempOut", "name": "НАРУЖ",
    "constraints": { "min": -40.0, "max": 60.0, "warning_low": -25.0, "warning_high": 45.0 }
  }
},
{
  "group": "sensor", "hw_input": "HwTempInt", "provider": "adc_temp",
  "rom": "28cb586a00000059",
  "analog_processors": [ { "type": "moving_average", "window": 4 } ],
  "sensor": {
    "kind": "one_wire_temp", "id": "HwTempInt", "name": "САЛОН",
    "constraints": { "min": -40.0, "max": 80.0, "warning_high": 50.0 }
  }
}
```

`watch_for_updates` already re-runs the whole load on any `sensor_config.json` edit, so
retuning a threshold or swapping a ROM is a restart, not a rebuild — same as every other
chain.

### 7. Self-test coverage

`add_adc_sensor_chains` is shared between the real path and `setup_self_test_sensors`, and
it is where `load_chains` is called — so the temp chains are built in **both** paths
automatically once §6 lands. For them to show data during the ~2 s startup sweep,
`TestADCDataProvider` gains a synthetic `AdcTempFrame`:

- new `temp_frame: AdcTempFrame` field + `pub fn temp_frame(&self)`, mirroring `frame()`.
- its `run_loop` writes the two known bench ROMs each tick, driven off the existing
  triangular `envelope()` — e.g. `celsius = 15.0 + level * 60.0`, `raw_16ths = (celsius *
  16.0) as i16`, quantised to 4-unit steps to match the real 10-bit sensor. Reusing
  `envelope()` (rather than a second sweep shape) keeps this consistent with how the speed
  / tacho synthetic channels already work.
- `add_adc_sensor_chains` signature grows a `temp_frame: AdcTempFrame` param; both call
  sites (`setup_self_test_sensors` → `test_adc.temp_frame()`, `setup_sensors` →
  `adc_provider.temp_frame()`) updated.

### 8. `main.rs` wiring summary

- `setup_adc_data_provider` unchanged; `ADCDataProvider::temp_frame()` now available.
- `setup_sensors` passes `adc_temp_frame` into `add_adc_sensor_chains`.
- `setup_self_test_sensors` passes `test_adc.temp_frame()`.
- No new top-level chain literals in `main.rs` — everything goes through `load_chains`.
- Optional: a `HwTempBusLink` link-health chain (mirrors `HwAdcLink`) if a "bus wire cut
  but USB fine" alert is wanted. Deferred — total USB loss is already covered by
  `HwAdcLink`, and per-address staleness covers a single dead sensor.

### 9. Tests

- `adc_data_provider`: `parse_temp_line` — a well-formed multi-pair line populates the
  frame; a negative value decodes; a malformed pair is skipped without dropping the good
  ones; bare `$T` bumps the bus timestamp only; a `$T` line does **not** touch `ADCFrame`'s
  `last_update`.
- `AdcTempFrame`: `fresh_raw` returns `None` past `TEMP_ADDR_MAX_AGE`; `addresses()` lists
  everything seen.
- `sensor_config`: the two repo `HwTemp*` entries load (extend
  `repo_sensor_config_json_loads_successfully` with a `temp_frame` handle); missing/short
  `rom` is a load error; `adc_temp` + wrong sensor kind is a load error;
  `one_wire_temp` + `digital_processors` is a load error.
- `sensors`: `OneWireTempSensor` — `404` → 25.25 °C; `(-88_i16) as u16` → −5.5 °C; clamp
  at the configured max.
- End-to-end (mirrors `self_test_speed_channel_*`): drive `TestADCDataProvider`'s synthetic
  temp frame through `OneWireTempChannelProvider` → `OneWireTempSensor`, assert a nonzero /
  envelope-tracking °C reading within the sweep window.

---

## Part 2 — `ТЕМП` page (sketch)

Reachable from `MainPage`'s `Left3` slot, currently unbound in **both** the primary and
secondary button sets (primary uses Left1/Left2/Left4/Right3/Right4; secondary uses
Left1/Left2/Right1/Right2/Right4).

- `pub const TEMP_PAGE_ID: u32 = 8;` in `page_manager.rs`.
- New `page_framework/temp_page.rs`, a `Page` built on `PageBase` like `HorzPage` /
  `GnssPage`. Registered **unconditionally** in `PageManager::setup` (the `OscPage`
  leniency pattern) — with no bus / empty map it renders `НЕТ ДАТЧИКОВ` rather than being
  absent.
- `MainPage::setup_buttons` gains
  `PageButton::new(ButtonPosition::Left3, "ТЕМП", … SwitchToPage(TEMP_PAGE_ID))`.
- `render(&self, ctx, sensor_manager, ui_style)` reads `HwTempOut` / `HwTempInt` from
  `sensor_manager.get_sensor_values()` — identical access path to `MainPage`. A row per
  logical sensor: label, value, unit, threshold-coloured.
- **Staleness must read differently from a real 0 °C**: a missing map entry (chain returned
  `Err`) shows `—` / `НЕТ СВЯЗИ`, not `0.0 °C`.
- Indicator choice (bar vs. digital-segmented vs. plain text rows), decorators, and
  multi-sensor layout: **left open**, to be designed against the real installed sensor set
  once the address → sensor map is filled in at commissioning.

Commissioning aid (independent of the page): surface `$T` lines / `AdcTempFrame::addresses()`
on the existing ADC terminal page so ROM strings can be read off the running dashboard when
wiring real sensors ("warm one, see which address moves").

---

## Settled decisions

- Two initial logical inputs: `HwTempOut` = `2854df6b000000d9`, `HwTempInt` =
  `28cb586a00000059`. More added as variant + config entry.
- New sensor type is `OneWireTempSensor`. `EngineTemperatureSensor` (analog PA2 coolant)
  untouched.
- Config: extend `load_chains` in place — new `provider: "adc_temp"`, `kind:
  "one_wire_temp"`, top-level `rom`. Conversion (`i16 / 16`) in code; map + thresholds +
  processors in JSON.
- `AdcTempFrame` is a standalone handle off `ADCDataProvider`, not part of `ADCFrame`.
- `$T` parsing does not touch `ADCFrame::last_update`.
- Temp chains built via `load_chains` in the shared `add_adc_sensor_chains`;
  `TestADCDataProvider` grows a synthetic `AdcTempFrame` so self-test exercises them.

## Open items

- Real ROM → sensor map (oil / gearbox / coolant-secondary / …) — settled at commissioning,
  per `ONEWIRE_TEMP_SENSOR_DESIGN.md`'s open decision. Framework ships with the two bench
  entries.
- `ТЕМП` page indicator/decorator layout.
- Whether a `HwTempBusLink` health pseudo-sensor / alert is worth adding.

---
*Created: September 6, 2026*
