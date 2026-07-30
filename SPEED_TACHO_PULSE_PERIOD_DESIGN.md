# Speed/Tacho Pulse-Period Measurement Design — replacing count-based rate

## Problem

`HwSpeed` (and `HwTacho`, though it doesn't need numeric precision — see Scope below) is
currently measured by having the STM32 ADC module count wheel-sensor pulses in a fixed 20ms
window (`TICK_HZ = 50`, hardware-timer-driven — see `stm32_adc_module/.../main.cpp`) and report
the accumulated integer count each report. At 6 pulses/revolution (`WIRING.md`), this gives a
km/h-per-raw-count granularity of:

```
km/h per raw count = TICK_HZ / PULSES_PER_REV * WHEEL_CIRCUMFERENCE_M * 3.6
                    = 50 / 6 * 2.304 * 3.6
                    = 69.12 km/h
```

That's coarse enough that no amount of Rust-side signal processing can fully hide it. Worked
example at a **perfectly steady 100 km/h**:

```
100 km/h = 27.778 m/s = 12.056 rev/s = 72.335 pulses/s (at 6 PPR)
raw count expected per 20ms tick = 72.335 * 0.02 = 1.4467   <- not an integer
```

Since the STM32 can only report a whole number, sustained 100 km/h isn't a clean `1,1,1,1,...`
stream — it's mostly `1`s with an extra `2` roughly every 2nd tick (that's what it takes for
an integer sequence to average to 1.4467 over time). This is not sensor noise; it's the
unavoidable consequence of measuring a continuous rate by counting discrete events in a fixed
window at a low pulse rate. Every time a `2` enters/leaves a 15-sample moving-average window,
the displayed value steps by `69.12 / 15 ≈ 4.6 km/h` — a real, visible jump, roughly 22 times a
second (versus roughly twice a second at the previously-assumed 4 PPR), **even under perfectly
steady real driving**, with the pipeline working exactly as designed.

Confirmed while investigating: there is **no smoothing at the render layer** either.
`NeedleIndicator::render` (`indicators/needle_indicator.rs`) computes the needle angle as a
pure, memoryless function of whatever `SensorValue` is current that frame — no lerp, no
easing, no stored previous angle. So any quantization/dithering in the measurement chain
propagates straight to the needle, frame by frame, with zero damping.

**Root cause**: a fixed-window *count* is the wrong measurement primitive for a low pulse-rate
signal (6 PPR) at a modest sample rate (50 Hz). Count-based quantization error is inversely
proportional to the averaging window size, which forces a hard tradeoff between resolution and
responsiveness that cannot be resolved downstream in software — a bigger window reduces the
step size but makes the gauge sluggish (window=100 for ~1 km/h resolution ≈ 2s of lag at the
real 50 Hz report rate). This is a genuine firmware/protocol-level limitation, not a Rust bug.

## Alternatives considered

1. **Keep count-based measurement, tune the window further.** Rejected as a real fix — just
   relocates the resolution/responsiveness tradeoff, doesn't remove it. (Already the current
   interim state; see Status below.)
2. **Add render-layer needle damping/easing only.** Would improve *visual* smoothness but not
   *measurement* accuracy — the underlying number would still hunt/step around the true value.
   Treats the symptom, not the cause. Worth doing independently at some point, but not a
   substitute for fixing the measurement itself.
3. **Measure inter-pulse period instead of per-window count (chosen).** The STM32 already runs
   a hardware timer (TIM2) for the 50Hz report tick. Capturing that timer's count at each pulse
   ISR (or using input-capture) and reporting the *period* between pulses instead of a count
   gives fine resolution even at low pulse rates, since time can be measured far more precisely
   than an integer count. At 100 km/h, the single inter-pulse period is ≈13.82ms; a timer with
   even modest clock resolution resolves that to a small fraction of a km/h, with no averaging
   window and no accuracy/responsiveness tradeoff.

## Chosen design: period-based pulse rate protocol

### Wire format (firmware change — not yet implemented, not this repo's Rust side)
Replace or augment the SPEED/TACHO channels with a period-based encoding. Left open for the
firmware design session, not dictated here:
- **(a) Raw timer-tick period since the previous pulse** (capture free-running timer count at
  each rising-edge ISR, report the delta from the previous capture). Simplest firmware change.
  Needs a defined "no new pulse since last report" encoding (e.g. 0 = idle/stopped) distinct
  from "very slow," and a channel wide enough not to wrap at low speed (long periods).
- **(b) Keep sending count-per-tick too, alongside period.** More wire complexity, but lets the
  Rust side pick whichever is more accurate across the full speed range (period dominates at
  low/moderate speed where counts are sparse; count aggregation is fine at high speed where
  many pulses land in one window anyway).

### Rust-side processing (period-based)
- Same `HWAnalogProvider`/`read_analog()` shape as today, just a different channel payload
  (period instead of count) — `ADCChannelProvider` itself shouldn't need to change.
- New conversion: `speed = (1 / period_seconds) / PULSES_PER_REV * WHEEL_CIRCUMFERENCE_M * 3.6`
  — inverting a period into a rate, instead of dividing an accumulated count by a fixed window.
  Needs explicit handling for "no recent pulse" (report 0, not a huge/infinite rate from a
  large period) — a staleness threshold, analogous to `ADCFrame::is_stale()` elsewhere in this
  codebase.
- Expected to be a localized change: swap the conversion math and the self-test data shape:
  `hardware/sensors.rs`'s `SPEED_*` constants and the `AnalogSignalProcessorScale` +
  `AnalogSignalProcessorMovingAverage` pipeline in `main.rs`'s `add_adc_sensor_chains` get
  replaced by whatever the period-based equivalent needs (likely still a moving average for
  jitter smoothing, but averaging *periods*, which are already fine-grained, not averaging
  *counts*, which are the actual source of the coarseness). The wider chain architecture
  (`SensorAnalogInputChain`, `GenericAnalogSensor`) is not expected to need changes.

## Scope

`HwTacho` (channel 4, 2 PPR) is explicitly **not** part of this — it only drives a boolean
"engine turning" indicator (`GenericDigitalSensor` via `read_digital`'s `value > 0` threshold),
which is correct for that purpose regardless of count-based coarseness. Only `HwSpeed`, which
needs numeric precision for a real km/h reading, is affected.

## Status

**Rust side done, simulated — STM32 firmware not yet changed.**

1. **[Done] Rust-side, simulated.** `hardware::sensors::SpeedSensor` now does the
   period-to-speed conversion directly (`speed = wheel_circumference_m / (period_s *
   pulses_per_rev) * 3.6`), with `raw == 0` reserved as the "no pulse observed" idle sentinel
   and a staleness timeout (`SPEED_STALE_PERIOD_MULTIPLIER`, 3x the last known period, floored
   at `SPEED_MIN_STALE_THRESHOLD` = 100ms) that decays the reading to 0 once a period value
   stops being refreshed by a new pulse for implausibly long — see the Rust-side processing
   section above. The old count-based interim approach (`AnalogSignalProcessorScale`,
   `SPEED_COUNT_PRESCALE`, `SPEED_SCALE_KMH_PER_COUNT`, the 15-sample moving average on raw
   counts) has been fully removed, not kept as a fallback path.

   `util::adc_data_provider::TestADCDataProvider`'s self-test sweep generates synthetic period
   data for HwSpeed via `hardware::sensors::speed_period_raw_from_kmh` — the exact inverse of
   `SpeedSensor`'s own conversion, so the self-test data and the real conversion it exercises
   can't silently drift apart. `SPEED_PERIOD_TIMER_HZ` (50kHz) is a placeholder pending the
   firmware wire-format decision below — chosen only so a u16 raw period doesn't wrap before
   ~1.1 km/h while still giving sub-km/h resolution at highway speed. Confirmed by
   `util::adc_data_provider::tests::self_test_speed_channel_tracks_envelope_target_speed`
   (drives the sweep's synthetic data through `SpeedSensor` end-to-end and checks the
   recovered speed tracks the intended envelope target) that the underlying period-to-speed
   conversion is correct and monotonic throughout the sweep, with no quantization artifacts.

   That conversion alone still looked visibly "steppy" on the self-test needle, though,
   because — unlike every other analog chain — HwSpeed initially had no signal processor at
   all between the raw channel and the sensor: at the sweep's fast rate of change (peaking
   near 200 km/h over a 500ms rise), each ~20ms raw update is itself a multi-km/h jump, and
   with nothing damping it frame-to-frame the needle visibly stair-steps instead of gliding
   the way the other gauges' 10-20-sample moving averages make their equally-fast self-test
   ramps look. `main.rs`'s `add_adc_sensor_chains` now runs the raw period through a small
   `AnalogSignalProcessorMovingAverage::new(5)` before `SpeedSensor` to restore that same
   smoothing — much smaller than the old 15-sample count average, since (a) a period sample
   doesn't need it for *accuracy* the way a count did, this is purely per-tick jitter
   smoothing, and (b) `SpeedSensor`'s idle/staleness logic reads the post-average value, so a
   larger window would perceptibly delay it from reaching `SPEED_PERIOD_IDLE_RAW` (0) once the
   vehicle genuinely stops. That delay isn't eliminated at window=5, just kept small; revisit
   if it proves too slow once real (noisier) firmware data exists.

2. **[Next] STM32 firmware change**, implementing whichever wire format is settled on from the
   options above (this also fixes `SPEED_PERIOD_TIMER_HZ`, currently just a placeholder),
   followed by switching the real `ADCChannelProvider`-based chain over from simulated to live
   data. No further Rust-side change is expected beyond that constant, since `SpeedSensor`
   already consumes a raw period value in the shape the firmware will produce.

---
*Created: July 30, 2026*
*Rust-side conversion implemented: July 30, 2026*
