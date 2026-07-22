# Oscilloscope Page — Burst-Capture Redesign

## Problem
`osc_page.rs` currently exists as a UI stub (status text, channel toggles, `sample_rate: f32` field defaulting to 1000.0) with no real data path behind it. Since then, the ADC has become a separate STM32F103 module (`stm32_adc_module/`) sending one shared telemetry frame at a fixed 50 Hz (`TICK_HZ` in `main.cpp`), with each channel value already a 16-sample hardware average taken across that same 20 ms tick.

That rate is not viable for an actual oscilloscope. Alternator AC ripple — the thing you'd want to diagnose (failed diode, bad regulation) — sits roughly in the 400 Hz (idle) to a few kHz (high RPM) range depending on pole count and pulley ratio. A 50 Hz frame rate, further low-pass filtered by 16x on-tick averaging, aliases that completely; nothing above a couple Hz survives. The continuous telemetry stream can support a slow strip-chart of the 12V bus (regulator hunting, charging sag, battery-only vs. alternator-charging) but not ripple/diode waveform diagnostics.

Goal: a dedicated, on-demand burst-capture mode — the STM32 pauses its normal 50 Hz telemetry, captures one channel at a much higher rate into a local buffer, ships that buffer back as a one-shot "snapshot," then resumes normal telemetry. Not a continuously-refreshing scope; a manually-triggered capture, matching what's actually achievable over this link.

## Proposed design

### Capture parameters
- Target channel: PA3 (`ADC_IN3`), the existing 12V system voltage input — same rail the alternator/battery bus is already wired to, no new hardware.
- Sample rate: ~50 kSPS. Reasoning: to resolve ripple *shape* (not just detect its presence — shape is what distinguishes a healthy 6-pulse waveform from a diode dropout) want ~15-20 samples per ripple cycle; 50 kSPS gives that margin even at the top of the expected ripple range.
- Buffer size: 4096 samples → an ~82 ms capture window. Long enough to show 30+ cycles even at idle-speed ripple frequencies (~400 Hz), short enough that the capture-and-pause is imperceptible to the driver.
- Mechanism: hardware timer (TIM3 — free, same pattern as TIM2's existing 50 Hz tick) triggers the ADC via DMA into a RAM buffer with no per-sample CPU involvement. STM32F103's ADC is rated well past 50 kSPS single-channel, so this is comfortably within spec.

### Command/response protocol
- **Pi → STM32:** a command line down the existing USB-CDC serial link, e.g. `$OSCCAP\n`. Capture parameters (channel, rate, sample count) are compiled-in `#define`s rather than passed as arguments — matches how `TICK_HZ`/`ADC_OVERSAMPLE` are already fixed constants, and there's no need for a runtime-configurable capture for a single "check the alternator" action. Kept visually distinct from `$A0,A1,...` telemetry frames and from the `#B,<value>` brightness command proposed in `BUTTON_BACKLIGHT_DESIGN.md`, so all three can't be confused by the parser.
- **STM32, on receiving `$OSCCAP`:**
  1. Stop the normal tick-driven 50 Hz telemetry send.
  2. Run the DMA capture (blocking, ~82 ms). EXTI-driven tacho/speed counters are unaffected (interrupt-based, independent of `loop()`). K-Line RX draining and button-debounce polling pause for that window — acceptable for a manual, user-initiated capture.
  3. Resume normal 50 Hz telemetry.
  4. Stream the captured buffer back.
- **Framing the buffer back:** the existing Pi-side reader (`adc_serial_reader.rs`) reads line-by-line (`BufReader::read_line`), and the current `char frame[128]` is sized for one CSV telemetry line — not 4096 samples. Rather than changing that reader's fundamental shape, chunk the capture into many ASCII lines with a distinct prefix:
  - `$OSCD,<seq>,<v0>,<v1>,...,<v63>\n` — 64 lines of 64 samples each (tunable), sequence-numbered so the Pi side can detect drops/reorder.
  - `$OSCEND\n` — sentinel marking the capture complete.
  - Native USB-CDC throughput is well above the nominal 115200 baud setting (that number is largely a formality for a CDC-ACM virtual serial port), so ASCII-encoding ~20KB of samples isn't a real bottleneck for a one-shot transfer.

### Pi-side handling
- `ADCDataProvider`'s read loop (`adc_data_provider.rs`) currently assumes every line is a telemetry frame. It needs a branch: lines prefixed `$OSCD`/`$OSCEND` route into a new capture-buffer type (e.g. `OscBuffer`, `Arc<Mutex<Vec<u16>>>` alongside the existing `ADCFrame`) instead of overwriting the regular channel frame.
- Needs the same outbound-write capability described in `BUTTON_BACKLIGHT_DESIGN.md` (retaining a `try_clone()`'d write handle in `ADCSerialReader`, since today it's read-only) — the capture command and the brightness command would share that same new write path.
- `osc_page.rs`'s `is_running` field becomes "capture in flight" rather than "streaming." A capture is user-triggered (button press → send `$OSCCAP` → wait for `$OSCEND` → render), not continuous.
- The existing `trigger_level` field becomes a **software** trigger applied post-capture: scan the completed buffer for the first rising edge crossing that level and offset the displayed window there, so repeated captures line up visually instead of jittering frame to frame. No hardware triggering needed since the whole window is already captured before any analysis happens.

## Firmware changes needed (`stm32_adc_module/Niva_Dashboard_ADC_Module/src/main.cpp`)
1. Configure TIM3 + DMA for single-channel ADC capture on PA3 at ~50 kSPS into a 4096-sample buffer.
2. Add `$OSCCAP` command parsing (shares the incoming-serial read loop already proposed for the brightness command in `BUTTON_BACKLIGHT_DESIGN.md`).
3. Add the pause/resume around the normal 50 Hz tick send during a capture.
4. Add the chunked `$OSCD`/`$OSCEND` transmission path.

## Rust app changes needed
1. Outbound write path on `ADCSerialReader` (shared prerequisite with the backlight design — see `BUTTON_BACKLIGHT_DESIGN.md`).
2. `$OSCD`/`$OSCEND` parsing branch in `ADCDataProvider::run_loop`, feeding a new `OscBuffer` type.
3. `osc_page.rs`: replace the streaming-UI assumption with capture-triggered request/wait/render; add post-capture software trigger-edge search using the existing `trigger_level` field; render the waveform (currently a `// TODO: Render actual oscilloscope waveform` stub).

## Open decisions
- Exact sample rate / buffer size trade-off once real ripple waveforms can be measured on the bench (50 kSPS / 4096 samples above is a starting estimate, not measured).
- Whether to add a second capture channel (e.g. compare alternator output ripple against the coolant/oil channels) or keep it single-channel-only.
- Chunk size for `$OSCD` lines (64 samples/line above is arbitrary — tune against the 128-byte frame buffer constant already in `main.cpp`, which would need to grow or the chunk size shrink).
- Whether button/K-Line unresponsiveness during the ~82 ms capture window needs any handling, or is simply acceptable as-is.

---
*Created: July 22, 2026*
