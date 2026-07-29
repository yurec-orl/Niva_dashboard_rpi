# Sensor Fusion Chain Design — Multi-Source Logical Sensors

## Problem
The current chain architecture binds exactly one `HWAnalogProvider` to one `AnalogSensor`
(`AnalogSensor::read(&mut self, input: u16)`), and one `HWDigitalProvider` to one
`DigitalSensor` analogously. That's sufficient for direct hardware-to-value pipelines, but
breaks down for a logical sensor that needs to combine two independently-sourced raw values
into one meaningful reading — e.g. a heading sensor that should read from a BNO085 IMU when
available (higher update rate, works while stationary) and fall back to GNSS dual-antenna
heading (drift-free, but needs an RTK heading fix and doesn't update while stationary) when
the IMU is absent or stale.

That arbitration — which source to trust right now — is a judgment call about data meaning,
not raw hardware access, so per this codebase's own layering (`sensor_manager.rs` module doc:
hardware providers return "unprocessed" data, logical sensors "convert processed signals to
meaningful values") it belongs at the logical sensor level. The current trait signatures make
that impossible: a logical sensor only ever sees one `u16` per read call.

## Alternatives considered

1. **Widen `AnalogSensor`/`SensorAnalogInputChain` to N providers**
   (`read(&mut self, inputs: &[u16])`, `hw_providers: Vec<...>`). Most general, but breaking:
   touches every existing `AnalogSensor` impl (`GenericAnalogSensor`, `EngineTemperatureSensor`,
   `UpsCurrentSensor`, `UpsChargeSensor`, `GnssAltitudeSensor`) and
   `SensorManager::read_analog_sensor`, for a capability only one sensor currently needs. Goes
   against this project's stated preference for not generalizing ahead of need (CLAUDE.md:
   "three similar lines is better than a premature abstraction").
2. **Do the arbitration in the hardware provider** (a `HeadingFusionProvider` holding both
   frames internally, returning one already-fused `u16`). Zero framework change, but puts a
   judgment call in the wrong layer — contradicts the raw-vs-logical split this codebase
   already establishes and enforces elsewhere (e.g. `AdcLinkStatusProvider` reports raw link
   staleness; deciding what that *means* for an alert happens in the Watchdog, not the
   provider).
3. **Add a narrow, parallel chain type for multi-source fusion** — chosen. Additive: doesn't
   touch any existing sensor, trait, or chain. Scoped to exactly the case that needs it.

## Chosen design: parallel fusion chain type

New trait, deliberately separate from `AnalogSensor` rather than a modification of it:

```rust
pub trait FusedAnalogSensor: Sensor {
    /// One entry per configured hardware provider, in the same order they were added to
    /// the chain. `None` means that provider's read failed or had nothing available this
    /// cycle — the fusion sensor decides what "unavailable" means for its own arbitration
    /// policy, rather than the whole chain read failing outright the way a plain
    /// `?`-propagated single-provider chain does today (see read_analog_sensor).
    fn read(&mut self, inputs: &[Option<u16>]) -> Result<&SensorValue, String>;
}
```

New chain type, registered in `SensorManager` alongside (not replacing) the existing
`analog_sensors: Vec<SensorAnalogInputChain>`:

```rust
pub struct SensorFusedAnalogInputChain {
    hw_providers: Vec<Box<dyn HWAnalogProvider + Send>>,
    sensor: Box<dyn FusedAnalogSensor + Send>,
}
```

`SensorManager` gains a parallel `fused_analog_sensors: Vec<SensorFusedAnalogInputChain>` list
and a `read_fused_analog_sensor` method that reads every provider independently (collecting
`Vec<Option<u16>>` — a failed read becomes `None`, not an early `?` bailout of the whole
chain), then calls `sensor.read(&values)`. Results land in the same
`sensor_values: HashMap<HWInput, SensorValue>` as everything else, so
`get_sensor_value`/`get_sensor_values()` — and therefore every downstream indicator — is
unaffected by how many raw providers fed a given `HWInput`.

### Why `Option<u16>` per provider, not a hard failure on any missing source
The whole point of fusion here is graceful fallback: BNO085 present → trust it; BNO085
absent/stale, GNSS heading present → fall back to GNSS; neither → `ValueData::Empty`. A
short-circuiting `?` (as `read_analog_sensor` uses today for single-provider chains) can only
express "proceed" or "abort," not "proceed with a degraded input."

## Concrete first use case: heading fusion (GNSS + BNO085)
- `HeadingFusionSensor: FusedAnalogSensor`, backed by a `SensorFusedAnalogInputChain` with two
  providers: `GnssChannelProvider::new(HWInput::HwGnssHeading, gnss_frame)` and a future BNO085
  orientation provider.
- Both encode heading the same way GNSS already does (`GNSS_HEADING_SCALE`, 0.1° resolution —
  see `hardware/hw_providers.rs`) so the fusion sensor decodes both sides identically.
- Feeds a new `HWInput::HwHeading` (distinct from `HwGnssHeading`, which stays as the raw
  GNSS-only reading for diagnostics/the terminal page) — the fused value is what a compass
  indicator would actually consume for display.

## Prerequisite, not yet in place
BNO085 would connect over I2C. `I2CProvider` (`hardware/hw_providers.rs`) is currently a dead
stub (`read_analog`/`read_digital` unconditionally return 0/`Level::Low`), already flagged in
CLAUDE.md's TODO as needing a decision — wire `hardware/gpio_input.rs`'s real I2C wrapper into
the provider traits, or drop the stub. That's a separate, blocking prerequisite for the
BNO085 side of this design, independent of the chain-type plumbing described above.

## Status
Design only — not implemented. The `SensorFusedAnalogInputChain`/`FusedAnalogSensor`
scaffolding could be added and validated with a GNSS-only single-source case (no real fusion
decision yet) ahead of the I2C/BNO085 work landing.

---
*Created: July 29, 2026*
