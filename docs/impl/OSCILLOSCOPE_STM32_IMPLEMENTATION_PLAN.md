# Oscilloscope Feature — STM32 Implementation Plan

Implements the firmware side of `OSCILLOSCOPE_DESIGN.md` (burst-capture on PA3, ~50 kSPS, 4096-sample
buffer, `$OSCCAP` / `$OSCD` / `$OSCEND` protocol). Scope of this doc is `stm32_adc_module/Niva_Dashboard_ADC_Module/src/main.cpp`
only — Rust-side handling is out of scope (see the design doc's "Rust app changes needed").

## Resource allocation check

| Resource | Currently used by | Used by this feature | Conflict? |
|---|---|---|---|
| TIM2 | 50 Hz telemetry tick | — | no |
| TIM3 | free | ADC hardware trigger (TRGO on update) | no |
| TIM1, TIM4 | free (TIM1 earmarked for backlight PWM per `BUTTON_BACKLIGHT_DESIGN.md`) | not used | no |
| DMA1 Channel1 | unused (ADC1's fixed DMA channel on F103 — not remappable) | ADC1 sample stream | no — confirm at implementation time that the stm32duino core doesn't silently enable DMA for USART3 (K-Line) or USB CDC; neither should on F103 (USB uses dedicated FS peripheral hardware, not DMA1), but verify before assuming |
| ADC1 | `analogRead()` for PA0–PA3, software-triggered, oversampled ×16, called every tick | same peripheral, reconfigured to hardware-triggered continuous DMA on PA3 only, for the capture duration | **time-shared, not simultaneous** — see "ADC mode switch" below, this is the main risk area |
| PA3 | 12V voltage input, part of normal 4-channel telemetry | same pin, reused for high-rate capture | no new hardware; just can't be read via `analogRead()` while a capture is in flight (already true — telemetry is paused for that ~82 ms) |

No new pins, no new hardware. The only genuinely new resource claims are TIM3 and DMA1 Channel1, both currently free.

## Timer math (target 50 kSPS)

TIM3 on this core runs at 72 MHz (APB1 timer clock, doubled from the 36 MHz APB1 bus clock — standard
for STM32F103 general-purpose timers under the default 72 MHz sysclk config this project already uses).

- `PSC = 0`, `ARR = 1439` → period = 1440 counts / 72 MHz = 20.0 µs = exactly 50.000 kSPS.
- 4096 samples × 20 µs = 81.92 ms capture window, confirming the design doc's "~82 ms" figure.
- ADC1 conversion time budget: even at 12-bit resolution with a long sample-time setting (e.g. 55.5 or
  71.5 ADC cycles, for better noise averaging — there's no rate pressure at 50 kSPS), total conversion
  time is on the order of 1–6 µs, well inside the 20 µs trigger period. Prefer the longest sample-time
  setting that still fits, since accuracy costs nothing here.
- Confirm (don't assume) the core's existing ADC prescaler already satisfies the F103's ≤14 MHz ADC
  clock limit — `analogRead()` already relies on this today, so it should already be correct (likely
  ÷6 → 12 MHz off the 72 MHz APB2 clock); no change needed unless found otherwise.

TIM3 must be configured for **master mode, TRGO = update event** (`TIM3->CR2` `MMS = 010`, or
`HAL_TIMEx_MasterConfigSynchronization` with `TIM_TRGO_UPDATE`) and ADC1's external trigger selected as
TIM3 TRGO (`ADC_EXTERNALTRIGCONV_T3_TRGO`, `EXTSEL = 100`). The Arduino `HardwareTimer` wrapper (already
used for TIM2's tick) doesn't expose master-mode/TRGO configuration — use it only to get a clocked/enabled
TIM3 handle, then drop to HAL calls for the master-mode and ADC-trigger wiring. This mirrors how TIM2 is
already half-Arduino/half-bare in spirit, just one layer lower.

## New state and constants

```cpp
#define OSC_ADC_CHANNEL      ADC_CHANNEL_3     // PA3
#define OSC_SAMPLE_RATE_HZ   50000UL
#define OSC_BUF_LEN          4096
#define OSC_CHUNK_SAMPLES    64                // samples per $OSCD line (tunable, see below)

static uint16_t osc_buffer[OSC_BUF_LEN];        // ~8 KB static — fine on 20 KB RAM part
```

`osc_buffer` as a file-scope static array, not stack — 8 KB would blow the available stack on this MCU if
placed in `loop()`'s frame.

## Command parsing (shared infrastructure with the backlight design)

There is currently no inbound-serial handling at all in `main.cpp` — `Serial` is write-only today. Both
this feature's `$OSCCAP` and `BUTTON_BACKLIGHT_DESIGN.md`'s `#B,<value>` need the same non-blocking
line-accumulation loop, so build one shared reader rather than two:

```cpp
static char cmd_line[32];
static uint8_t cmd_len = 0;

// Call once per loop() iteration, before the tick_flag check — cheap when Serial is idle.
static void poll_incoming_commands() {
    while (Serial.available()) {
        char c = (char)Serial.read();
        if (c == '\n') {
            cmd_line[cmd_len] = '\0';
            dispatch_command(cmd_line);
            cmd_len = 0;
        } else if (cmd_len < sizeof(cmd_line) - 1) {
            cmd_line[cmd_len++] = c;
        } else {
            cmd_len = 0; // overlong line — drop and resync on next '\n'
        }
    }
}

static void dispatch_command(const char *line) {
    if (strcmp(line, "$OSCCAP") == 0) {
        run_oscilloscope_capture();
    }
    // else if (strncmp(line, "#B,", 3) == 0) { ... brightness, per BUTTON_BACKLIGHT_DESIGN.md ... }
}
```

32 bytes is enough for both `$OSCCAP` and `#B,255`; revisit if a future command needs more.

## Capture routine

Single blocking function, called synchronously from `dispatch_command()`. Blocking is acceptable per the
design doc (manual, user-triggered, ~82 ms).

```cpp
static void run_oscilloscope_capture() {
    // 1. Configure TIM3: PSC=0, ARR=1439, master mode TRGO=update. Do not start yet.
    // 2. Configure ADC1: single channel (OSC_ADC_CHANNEL), 12-bit, external trigger =
    //    TIM3 TRGO, DMA enabled, DMA in "normal" (one-shot) mode, NDTR = OSC_BUF_LEN,
    //    destination = osc_buffer, 16-bit peripheral/memory width.
    // 3. HAL_ADC_Start_DMA(&hadc1, (uint32_t*)osc_buffer, OSC_BUF_LEN);
    // 4. Start TIM3 counter — this begins triggering conversions.
    // 5. Poll DMA1 channel1 transfer-complete flag (DMA_FLAG_TC1) with a timeout guard
    //    (e.g. 150 ms — well above the expected 82 ms) rather than a fixed delay(), so a
    //    misconfiguration hangs a bounded time instead of forever.
    // 6. Stop TIM3, HAL_ADC_Stop_DMA(&hadc1).
    // 7. Restore ADC1 to the state analogRead() expects (software-triggered, no DMA,
    //    single-conversion mode) — see "ADC mode switch" below. This step is the one most
    //    likely to need iteration on real hardware.
    // 8. oscilloscope_send_buffer();
}
```

### ADC mode switch — the main risk area

`analogRead()` (stm32duino core) reinitializes ADC1's channel config and does a software-triggered
single conversion on every call — it does not assume any particular prior ADC state beyond the peripheral
being clocked and calibrated. That's good: it means a full `HAL_ADC_Init()`-equivalent reset back to
default (software trigger, no DMA, no continuous mode) after the capture should be enough to make
subsequent `analogRead()` calls on PA0–PA2 (and PA3 itself, next telemetry tick) behave exactly as before.

What needs verification on the bench, not just assumed from reading the HAL headers:
- That `HAL_ADC_Stop_DMA()` actually clears `CR2.DMA`/`CR2.EXTTRIG`/`CR2.CONT` as expected, or whether
  those need explicit clearing.
- That the first `analogRead()` call after a capture returns a sane value and not a stale/garbage one
  (a one-tick glitch on the telemetry channels right after a capture would be a minor but noticeable bug).
- ADC calibration (`ADC_StartCalibration`) is normally run once at boot by the core; confirm a capture
  doesn't require re-calibration afterward for accurate `analogRead()` values.

If in practice the restore proves fragile, the fallback is a full `HAL_ADC_DeInit()` +
re-run of whatever init sequence the core's `analogRead()` path relies on — more heavyweight but
unambiguous. Try the light restore first; only fall back if bench testing shows drift or glitches.

## Telemetry pause/resume

The design doc's step 1 ("stop the normal tick-driven 50 Hz telemetry send") doesn't need an explicit
"paused" flag or state machine — the capture call happens synchronously inside `dispatch_command()`,
called from `poll_incoming_commands()` at the top of `loop()`, *before* the existing
`if (!tick_flag) return;` gate. While `run_oscilloscope_capture()` blocks for ~82 ms:

- `tick_flag` will latch `true` (possibly several times, coalesced into one bool by TIM2's ISR) during
  the block. After the capture returns and the buffer is sent, `loop()` falls through to the normal
  tick-flag check on its next pass and sends exactly one telemetry frame — not a burst of 4 catch-up
  frames. No special handling needed; this falls out of `tick_flag` already being a single bool, not a
  counter.
- EXTI-driven tacho/speed pulse counters are unaffected (interrupt-based, independent of `loop()`), per
  the design doc.
- K-Line RX draining and button debounce polling pause for ~82 ms. At K-Line's 10400 baud (~1 byte/ms),
  that's up to ~18 bytes potentially lost into/past the 64-byte ring buffer during a capture — worth
  stating explicitly (the design doc leaves this as an open decision); given K-Line handling is still a
  stub with no consumer yet, treat as acceptable for now and revisit once K-Line parsing is real.
  Button input gains ~82 ms of extra worst-case latency on a press that lands mid-capture — not
  noticeable for a manual, occasional diagnostic action.

## Chunked transmission

```cpp
static void oscilloscope_send_buffer() {
    char osc_frame[OSC_CHUNK_SAMPLES * 5 + 16]; // "$OSCD,255," + up to 64×"4095," + "\n" + slack
    uint16_t seq = 0;
    for (uint16_t i = 0; i < OSC_BUF_LEN; i += OSC_CHUNK_SAMPLES, seq++) {
        int n = snprintf(osc_frame, sizeof(osc_frame), "$OSCD,%u", seq);
        for (uint16_t j = 0; j < OSC_CHUNK_SAMPLES; j++) {
            n += snprintf(osc_frame + n, sizeof(osc_frame) - n, ",%u", osc_buffer[i + j]);
        }
        snprintf(osc_frame + n, sizeof(osc_frame) - n, "\n");
        Serial.print(osc_frame);
    }
    Serial.print("$OSCEND\n");
}
```

Sizing decision (resolves one of the design doc's open items): give this its own buffer
(`osc_frame`, ~336 bytes for 64 samples/line) rather than growing or reusing the existing 128-byte
`frame[128]` telemetry buffer — that buffer is on `loop()`'s stack in the hot 50 Hz path and shouldn't
grow to accommodate a rare, large, one-shot transfer. `OSC_BUF_LEN / OSC_CHUNK_SAMPLES` = 64 lines total
at the proposed chunk size; adjust `OSC_CHUNK_SAMPLES` down (and the frame buffer with it) if bench testing
shows USB-CDC or Pi-side line-buffer issues at this line length, but there's no evidence yet that's needed
— native USB-CDC throughput is not the bottleneck the design doc already reasons through.

## File header / comment updates

- Add TIM3 (ADC capture trigger, TRGO on update, 50 kHz) and DMA1 Channel1 (ADC1 sample stream) to the
  "Final Pin Assignment Summary" / peripheral notes — these are timer/DMA resource claims, not new pins,
  so they belong near the existing TIM2 note rather than in the pin table itself.
- Note in the PA3 pin comment that it's time-shared between normal oversampled telemetry reads and
  capture-mode DMA streaming, mutually exclusive by construction (only one can be active at a time,
  enforced by the capture routine being synchronous/blocking).
- Update the protocol comment block at the top of the file (currently documents only the `$A0,...`
  telemetry line) to mention `$OSCCAP` / `$OSCD` / `$OSCEND` alongside it.

## Ordered task list

1. Add `poll_incoming_commands()` / `dispatch_command()` and wire into `loop()` before the tick-flag
   check. (Shared prerequisite with the backlight command — implement generically enough that adding
   `#B,<value>` later is a one-line addition to `dispatch_command()`, not a rewrite.)
2. Add oscilloscope constants and the static `osc_buffer`.
3. Implement `run_oscilloscope_capture()`: TIM3 master-mode config, ADC1 external-trigger + DMA config,
   start/poll-with-timeout/stop, ADC1 restore.
4. Implement `oscilloscope_send_buffer()`.
5. Update file header comments per above.
6. Bench validation (see below) before considering this done — this feature is unusually easy to get
   subtly wrong (ADC mode restore) in a way that only shows up as jittery telemetry *after* the first
   capture, not as a build or obvious runtime failure.

## Bench validation checklist

- Verify actual trigger rate: probe an otherwise-unused GPIO toggled once per TIM3 update (temporary
  debug instrumentation, remove before merging) or check captured-buffer timing indirectly via a known
  input frequency, to confirm 50.000 kSPS and not an off-by-one in `ARR`.
- Feed a known waveform (bench signal generator, or just toggle the 12V rail during engine idle) into the
  existing PA3 divider and confirm the captured shape is sane (not aliased, not clipped, not all-zero).
- Trigger several captures back-to-back and confirm normal telemetry (`A3` field, and ideally `A0`–`A2`
  too) reads correctly and without a value glitch on the frame immediately following each capture — this
  is the specific failure mode the "ADC mode switch" section above flags as the main risk.
- Confirm total capture-to-`$OSCEND` wall time roughly matches the ~82 ms capture window plus transmission
  time, with no unexpected stall (would indicate the DMA poll-with-timeout path is being hit).
- Send `$OSCCAP` while artificially loading K-Line RX and holding a button down, to sanity-check the
  pause/resume behavior described above doesn't do anything worse than the expected small data/latency hit.

## Open items carried over from the design doc (unchanged, still open)

- Exact sample rate / buffer size once real ripple waveforms are measured on the bench.
- Second capture channel, if ever wanted.
- Whether K-Line byte loss during a capture needs handling once K-Line parsing becomes real (currently a
  stub, so currently moot).

---
*Created: August 14, 2026*
