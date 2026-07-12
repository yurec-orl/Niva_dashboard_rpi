# Button Backlight Redesign — PWM via STM32

## Problem
Current button backlight design powers LEDs directly from 12V, which has two issues:
1. Buttons connect to the STM32 ADC module, which is powered from USB and has no 12V source available.
2. No brightness control — brightness is currently only a software render-time color multiplier (`apply_brightness` in `src/graphics/context.rs`), with no link to physical LEDs.

Goal: control button backlight brightness from the RPi, while minimizing cross-wiring between modules (button headers, LED power, and LED ground should all land on the STM32 board, not be split across modules).

## Proposed design

### Hardware
- Single PWM signal drives the entire backlight rail — brightness is one value for all buttons, not per-key, so one STM32 pin is sufficient.
- **PA10 (TIM1_CH3)** is free and PWM-capable. TIM1, TIM3, and TIM4 are entirely unused in the current firmware (only TIM2 is claimed, for the 50 Hz tick), so there's no pin/timer conflict.
- Signal chain: PA10 → gate resistor (~220Ω) → logic-level N-channel MOSFET (e.g. 2N7002/AO3400) → low-side switch for the LED backlight rail.
  - LED anodes tie to a low-voltage supply (5V from the Pi/UPS rail, or a dedicated regulated rail) through the existing per-LED/per-group current-limiting resistors.
  - LED cathodes (common return) go to the MOSFET drain; source to GND.
  - STM32 PWMs the gate at a few kHz (avoid visible flicker and audible whine) — TIM1 handles 1–20kHz easily.
- Rationale for MOSFET instead of driving LEDs directly from PA10: PA10 can only source/sink ~20-25mA, not enough for 8-16 button LEDs in parallel. The MOSFET is a current-capable switch; PA10 just toggles its gate.
- This removes the 12V dependency for LEDs (they move to the same low-voltage rail the STM32 already uses) and keeps all button wiring (signal, LED+, LED−) on the STM32 board.

### LED current-limiting resistor — 180Ω, 1/4W (one per LED)
- LEDs are amber/yellow, one resistor per LED (each LED in parallel to the MOSFET rail).
- Measured V_f via multimeter diode-test mode (~1mA test current): **1.817V**. Estimated V_f at the ~15-17mA operating point: ~2.0-2.05V (typical +0.15-0.25V rise from diode-test current to operating current).
- Supply: 5V, taken from the STM32 board's 5V pin (USB VBUS passthrough — see power budget note below; actual voltage under load is likely ~4.7-4.9V, not a clean 5.0V, and should be re-measured once the LED bank is wired in).
- R = (V_supply − V_f) / I_target ≈ (5.0 − 2.0) / 0.016 ≈ 180Ω (nearest standard E24 value).
- Resulting current at 180Ω: ~15.6-16.7mA across the expected V_f range — solidly daylight-visible while staying under the ~20mA continuous rating typical of standard 3mm/5mm indicator LEDs, with margin for static full-on (100% duty, pre-PWM) operation.
- Power dissipated per resistor: ~50mW — trivial for a 1/4W part.

### Two-way communication (RPi ↔ STM32)
Reuses the existing USB-CDC serial link — no new wiring needed. Also lays groundwork for future bidirectional OBD-II support over the same link (e.g. PID request/response).

- **RPi → STM32:** send a brightness command frame, e.g. `#B,<0-255>\n` — kept visually distinct from the existing `$...` telemetry frames so parsing can't confuse the two.
- **STM32:** add a `Serial.available()` read loop in `loop()` (parallel to the existing K-Line RX drain) that parses the brightness command and sets the TIM1 CH3 PWM compare value via `pwmWrite()`.

## Firmware changes needed (`stm32_adc_module/Niva_Dashboard_ADC_Module/src/main.cpp`)
1. Add `PIN_BTN_BACKLIGHT_PWM = PA10` and initialize a `HardwareTimer` on `TIM1` with a PWM channel on PA10 (mirrors the existing TIM2 setup pattern already in the file).
2. Add incoming-serial parsing in `loop()` for the brightness command.
3. Update the pin mapping comments and "Final Pin Assignment Summary" table — move PA10 out of "free" into backlight PWM.

## Rust app changes needed

### Serial write path
- `serialport` crate (used today) is inherently bidirectional (`Write` + `try_clone()`), but `ADCSerialReader` (`niva_dashboard/src/util/adc_serial_reader.rs`) currently wraps the port in a `BufReader` and never retains a writable handle.
- Fix: retain a `try_clone()`'d write handle alongside the existing read side — contained to this one file.
- The read loop already runs on its own blocking thread with `Arc<Mutex<...>>` shared state (`niva_dashboard/src/util/adc_data_provider.rs`) — the same pattern extends cleanly to a write path; no async runtime needed.

### Protocol
- There is currently no outbound framing/protocol at all — only inbound `$...` frame parsing exists (`ADCDataProvider::run()` in `adc_data_provider.rs`). A new command format (e.g. `#B,<value>\n`) needs to be designed, along with whatever ACK/response handling (if any) is wanted from the STM32 side.

### Brightness wiring
- Brightness today is purely a software color multiplier: `brightness: f32` field, `set_brightness`/`increase_brightness`/`decrease_brightness`/`apply_brightness` in `src/graphics/context.rs`, driven by `UIEvent::BrightnessUp/Down/SetBrightness` in `src/page_framework/events.rs` and `page_manager.rs`.
- Wiring physical backlight control means adding a call from these existing brightness handlers down to a new "write to STM32" API.

## Effort assessment
Small-to-medium change on both sides — no rearchitecting of the existing read pipeline or firmware structure. Main net-new work is the outbound protocol/framing (doesn't exist today) and the PWM init + command parsing on the firmware side.

## Power budget
Sourcing the LED bank from the STM32's 5V pin adds ~135mA (8 LEDs × ~17mA) on top of the STM32 board's own draw (~30-50mA) and shares headroom with the RPi's downstream USB power budget (~1.2-1.6A total depending on Pi model — verify against the actual target board, not just whichever doc is on hand). Total additional load (~400-420mA including the display) leaves ample margin either way and is not expected to be a constraint.

## Noise suppression
The STM32's 5V pin very likely also feeds its onboard 3.3V regulator, which serves as the ADC reference for the analog sensor channels (oil pressure, fuel, coolant, voltage). Once the backlight is PWMed (kHz-range switching of ~135mA), there's a real risk of switching noise coupling into ADC readings via the shared supply. Mitigations:
- Add a bulk capacitor (100-220µF) at the LED/MOSFET supply node to buffer the switching transient.
- Monitor sensor readings for jitter correlated with backlight brightness once PWM is live.
- If coupling proves significant, move the LED supply to a separate 5V source (e.g. directly off the UPS HAT rail) instead of sharing the STM32's own supply pin.

## Open decisions
- Exact command frame format and any ACK/response handling.
- MOSFET part number, gate resistor value, and PWM frequency.

---
*Created: July 4, 2026*
