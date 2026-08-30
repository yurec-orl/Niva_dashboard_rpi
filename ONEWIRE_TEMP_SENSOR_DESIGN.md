# One-Wire Temperature Sensors — DS18B20 Bus on the STM32 ADC Module

## Problem
The dashboard has one temperature input today: the analog coolant sensor on PA2 (`ADC_IN2`), converted from raw ADC counts on the Rust side. There is demand for several more temperature readings — oil, cabin, outside air, gearbox/transfer case — none of which have a dedicated analog channel, and the STM32F103 has no spare ADC pins (PA0–PA3 are all in use).

DS18B20 one-wire sensors solve the pin-count problem: any number of them share a single GPIO. The requirement is that **adding or removing a sensor must not require a firmware change** — the STM32 discovers whatever is on the bus, polls it, and reports each reading tagged with the sensor's 64-bit ROM address. The dashboard owns the mapping from address to logical sensor (coolant-secondary / oil / cabin / outside / …).

The analog coolant sensor stays as-is. The one-wire bus is strictly additive.

## Division of labour
- **STM32:** bus discovery (SEARCH ROM, once at startup), `CONVERT T` + scratchpad read, CRC validation, and emitting `address:value` pairs. No knowledge of what any sensor measures.
- **Dashboard:** map ROM address → logical sensor, per-address staleness, display/alert wiring. Ignores addresses it doesn't recognise on the main UI.

The Rust-side design is deliberately **out of scope for this document** — it will be specified separately, and folds into the data-driven sensor config work (`DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md`). What follows is firmware and wire protocol only.

## Wire protocol — the `$T` line

Temperatures are reported on their own tagged line, **not** appended to the 50 Hz `$…` telemetry frame:

```
$T,<rom>:<raw>;<rom>:<raw>;...\n
```

- **`<rom>`** — 16 lowercase hex characters: the 8 ROM bytes printed `%02x` in device order — byte 0 = family code (`28` for DS18B20), bytes 1–6 = 48-bit serial, byte 7 = ROM CRC. Emitted verbatim; the dashboard uses the full 16-char string as the map key, so byte order must not change.
- **`<raw>`** — signed decimal integer, the DS18B20 temperature register value in units of 1/16 °C (its native format, sign-extended from 12/11/10/9 bits per the configured resolution). The dashboard computes `°C = raw / 16.0`. Examples: `400` → +25.0 °C, `-88` → −5.5 °C. Sending the raw register (rather than a pre-scaled deci-/centi-°C value) keeps the MCU side conversion-free and lossless, and matches how the analog channels already hand raw counts to the dashboard.
- A sensor whose scratchpad **fails CRC on a given cycle is omitted** from that line — no placeholder value. The dashboard's per-address staleness handles the gap.
- **Zero sensors on the bus** → still emit `$T\n` every cycle. This lets the dashboard distinguish "bus alive, nothing found" from "firmware predates this feature / link down".

### Rationale for a separate line rather than an appended field
- DS18B20 12-bit conversion takes up to 750 ms, so new data arrives at roughly 1 Hz. Re-sending the same string inside every 20 ms telemetry frame would bloat the hot path for no benefit.
- The `$…` telemetry frame stays fixed-layout; the Rust side's positional CSV parser (`split(',')` → `filter_map(parse::<u16>)` → index by `AdcChannel`) is untouched. An embedded `:`/`;` field would only survive that parser by accident (non-numeric tokens are silently dropped) and only if it were the last field.
- Mirrors the existing `$OSCD` / `$OSCEND` tagged-line pattern. The current `ADCDataProvider::run_loop` already ignores unrecognised `$…` lines, so a `$T` line is safely inert until the Rust side explicitly handles it — backward compatible by construction.
- Decouples the one-wire cadence from the 50 Hz tick, which is what makes the non-blocking firmware state machine below feasible.

### Cadence
One `$T` line per convert→read cycle: ≈1 Hz at 10-bit resolution, ≈0.7 Hz at 12-bit. Lines are interleaved between normal telemetry frames. The Rust reader is line-oriented, so there is no framing conflict.

## Firmware design (`stm32_adc_module/Niva_Dashboard_ADC_Module/src/main.cpp`)

### Hardware
- **Bus pin: PA10.** Committed to this use — the button backlight, previously the other candidate for PA10, is now handled by an external dimmer with no STM32 pin (see `BUTTON_BACKLIGHT_DESIGN.md`). 1-Wire needs only an open-drain-capable GPIO, no timer.
- 4.7 kΩ pull-up from PA10 to 3.3 V.
- Sensors **externally powered (3-wire)**, not parasite. Parasite power forces a strong active pull-up and total bus silence for the whole conversion window, complicating the state machine for no real gain here.
- Automotive harness runs to the coolant-secondary / outside-air sensors are long and electrically noisy — use twisted or shielded pair, and treat CRC-retry as mandatory (below), not optional.

### Configuration constants
| define | value | purpose |
|---|---|---|
| `DS18B20_PIN` | `PA10` | bus GPIO |
| `MAX_DS18B20` | `10` | device table size; also sizes the `$T` transmit buffer |
| `DS18B20_RES_BITS` | `10` | 187.5 ms conversion; 0.25 °C resolution — ample for fluid/air temps. `12` is a one-line change if finer resolution is ever wanted. |
| family filter | `0x28` | only DS18B20; ignore any other 1-Wire device on the bus |

`$T` transmit buffer: `char temp_frame[MAX_DS18B20 * 24 + 16]` (≈ 256 B). Built incrementally with bounded `snprintf`, the same way `oscilloscope_send_buffer()` builds `$OSCD` lines — never a single unchecked format call.

### Non-blocking state machine
Ticked from the existing 50 Hz `loop()` (or driven off `millis()` timestamps). One transaction step per tick; never blocks.

1. **`OW_SEARCH`** (once, at startup) — run the SEARCH ROM algorithm **one device per tick** (each device is several bus resets plus 64 read-triplets — a few ms). Spreading it across ticks avoids stalling the telemetry frame during boot. CRC-check each returned ROM; keep `0x28`-family devices in the table. When the search completes, fall through to `OW_CONVERT` and never return here.
2. **`OW_CONVERT`** — bus reset, `SKIP ROM` (`0xCC`), `CONVERT T` (`0x44`). Record the start time.
3. **`OW_WAIT`** — no bus activity until the resolution's conversion time has elapsed (~7 ticks at 10-bit, ~38 at 12-bit).
4. **`OW_READ`** — **one sensor per tick**: `MATCH ROM` (`0x55`) + address, `READ SCRATCHPAD` (`0xBE`), 9 bytes, CRC8 over bytes 0–8. Good → store the raw 16-bit register value; bad → skip this sensor for this cycle.
5. After the last sensor: build the `$T` line from all sensors that read good this cycle, `Serial.print` it, return to `OW_CONVERT`.

No inbound command is involved — discovery and polling are autonomous.

### No runtime hot-plug
Discovery runs **once at startup**, not on a periodic re-scan. Rationale:
- The STM32 is USB-powered by the Pi and resets on every dashboard restart / power cycle, so a re-plugged or newly-added sensor is picked up on the next boot anyway.
- Removing then reinserting an *already-known* sensor needs no re-scan: its `MATCH ROM` reads simply fail (omitted from `$T`, dashboard staleness handles it) and resume on their own once it is back.
- Only a brand-new address appearing mid-run would be missed — not a real scenario for sensors installed once at commissioning.

Dropping the periodic re-scan also removes the rescan timer, the per-device miss counter, and the drop-after-N-misses logic — net simplification, not just a deferral.

### Interrupt interference
1-Wire read slots require the master to sample the bus ~15 µs into a ~60 µs slot. `tacho_isr` / `speed_isr` fire on EXTI0/EXTI1 at up to a few hundred Hz and, if one preempts mid-slot, can push the sample past its valid window and flip a bit.

Mitigation, in order:
1. **Rely on CRC8 + retry next cycle.** A corrupted scratchpad read is caught and that sensor is simply omitted from the current `$T` line; it reappears next cycle. For ~1 Hz fluid/air temperatures this is invisible.
2. If bench testing shows an unacceptable retry rate, **mask EXTI0/EXTI1 only during the ~1 ms scratchpad-read burst per sensor** — not across the whole transaction. That drops at most ~1 ms of tacho/speed edges per sensor per cycle, negligible for the inter-pulse-period measurement those ISRs perform.

Never wrap a whole transaction in `noInterrupts()` — it would corrupt the tacho/speed period accumulation.

### Interaction with `$OSCCAP`
`run_oscilloscope_capture()` blocks for ~82 ms, but only between `loop()` iterations, never mid-tick. A single sensor's per-tick scratchpad read therefore always completes atomically. A `CONVERT T` that is in flight when a capture runs simply experiences a longer effective `OW_WAIT`, which is harmless. No explicit coordination needed.

### Library
Use Paul Stoffregen's `OneWire` primitives directly (`reset`, `write`, `read`, `search`, `crc8`) and implement the async flow above. `DallasTemperature` is built around blocking `requestTemperatures()` and does not fit the state machine.

## Firmware changes needed
1. Add `DS18B20_PIN` and the config constants; update the pin-map comments and the "Final Pin Assignment Summary" table (PA10 moves from "reserved/planned" to active use).
2. Add the device table (`{ uint8_t rom[8]; int16_t raw; bool valid_this_cycle; }` × `MAX_DS18B20`, plus a count), populated once by `OW_SEARCH`.
3. Add the `OW_*` state machine, ticked from `loop()`.
4. Add the `$T` line builder + transmit, using bounded `snprintf`.
5. Update the protocol comment block at the top of `main.cpp` to document the `$T` line alongside the existing `$…` / `$OSCD` / `$OSCEND` descriptions.

## Rust app changes needed
Deferred — specified separately, alongside `DATA_DRIVEN_SENSOR_CONFIG_DESIGN.md`. In brief, the eventual work is: a `$T`-line parse branch in `ADCDataProvider::run_loop`, a per-address temperature store with per-address staleness on (or beside) `ADCFrame`, an address → logical-sensor map from config, "ignore unknown address" on the main UI, and a `$T` equivalent in `TestADCDataProvider` so self-test exercises the temp indicators.

## Settled decisions
- **Resolution: 10-bit** (0.25 °C, 187.5 ms conversion). Fixed via `DS18B20_RES_BITS`.
- **No signal processing on the STM32.** It emits raw register values only — no value-hold, hysteresis, smoothing, or unit conversion. All of that lives on the Rust side.

## Open decisions
- Physical commissioning: ROM addresses are opaque, so building the dashboard's address → sensor map needs a "warm one sensor, see which address moves" step. **The map itself is TBD and must be settled before Rust-side implementation.**

---
*Created: August 30, 2026*
