# Button Backlight — External Dimmer

## Problem
The original button backlight design powered the LEDs directly from 12V, with two issues:
1. Buttons connect to the STM32 ADC module, which is powered from USB and has no 12V source available.
2. No brightness control — screen brightness is a software render-time colour multiplier (`apply_brightness` in `src/graphics/context.rs`), with no link to the physical button LEDs.

An earlier revision of this document proposed solving both by PWMing the backlight rail from the STM32 (MOSFET low-side switch on PA10 / TIM1_CH3, driven by a `#B,<value>` serial command from the Pi). **That approach is dropped.** PA10 is committed to the DS18B20 one-wire temperature bus (see `ONEWIRE_TEMP_SENSOR_DESIGN.md`), and the backlight is instead handled by a **standalone external dimmer**, independent of the STM32 and the dashboard software.

## Decision
A commercial inline **PWM dimmer module** (rotary-knob or trimpot adjustable) sits on the button-LED supply. Brightness is set manually and is entirely decoupled from the software — screen brightness (`UIEvent::BrightnessUp/Down/SetBrightness`) and physical button-backlight brightness become two independent adjustments. This is acceptable: the backlight is a set-and-forget comfort setting, not something that needs to track the screen dimming automatically.

### Rationale
- **No firmware change, no serial protocol, no Rust change.** The `#B` command, the outbound serial write path, and the TIM1 PWM init all disappear from scope.
- **PA10 goes to the one-wire bus** instead of a backlight MOSFET — it is the last practical spare GPIO on the STM32 (only PB2/BOOT1 otherwise).
- **The ADC supply-noise risk is avoided entirely.** The earlier plan's kHz switching of ~135 mA shared the STM32's 5V pin (and hence its 3.3V ADC reference); an external dimmer on its own supply rail never touches that domain.

## Hardware

### Dimmer module and supply
- Off-the-shelf low-voltage PWM LED dimmer (the common potentiometer-knob type, rated well above the ~150 mA backlight draw).
- Fed from either the 12V accessory rail or a 5V rail — see Open decisions. The dimmer output is pulsed at its input voltage, so the LED current-limiting resistor is sized for whichever rail is chosen.
- All button wiring (switch signal, LED+, LED−) still lands on the STM32 board side for the switch contacts; the LED supply pair runs from the dimmer to the LED anodes' common rail, LED cathodes to the dimmer's switched return.

### LED current-limiting resistor (one per LED)
- LEDs are amber/yellow, one resistor per LED, all LEDs in parallel across the dimmed rail.
- Measured V_f (multimeter diode-test, ~1 mA): **1.817 V**. Estimated V_f at the ~15–17 mA operating point: **~2.0–2.05 V** (typical +0.15–0.25 V rise from diode-test current to operating current).
- `R = (V_rail − V_f) / I_target`, with `I_target ≈ 16 mA`:
  - **5V rail:** R ≈ (5.0 − 2.0) / 0.016 ≈ **180 Ω**, ~50 mW → 1/4 W is fine.
  - **12V rail:** R ≈ (12 − 2.0) / 0.016 ≈ **620 Ω**, ~160 mW → use 1/2 W for margin.
- Target current keeps the LEDs solidly daylight-visible while staying under the ~20 mA continuous rating of standard 3/5 mm indicator LEDs, with headroom for the dimmer at 100% duty.
- If the chosen dimmer module is constant-current rather than constant-voltage PWM, the per-LED resistors are omitted and the module's current setting is divided across the parallel LEDs instead — confirm the module type before ordering resistors.

## Power budget
- On a 12V feed: the backlight draw (~150 mA) is on the accessory rail, not the STM32/USB budget — no interaction with the Pi's downstream USB power limit.
- On a 5V feed shared with the STM32 board: ~135 mA (8 LEDs × ~17 mA) on top of the board's own ~30–50 mA. Still well within the Pi's USB budget, but prefer the 12V feed to keep it off the ADC supply domain.

## Consequences for other docs
- `ONEWIRE_TEMP_SENSOR_DESIGN.md` — PA10 is now unambiguously the one-wire bus pin; the pin-conflict caveat there is removed.
- `OSCILLOSCOPE_DESIGN.md` — the `$OSCCAP` outbound write path is no longer shared with a backlight command; it stands alone. (The oscilloscope feature is already built, so this is just a stale cross-reference to clean up.)

## Open decisions
- Dimmer module part and whether it is constant-voltage PWM (needs per-LED resistors) or constant-current (does not).
- Supply rail: 12V accessory (preferred — off the ADC domain) vs. 5V.
- Physical knob location — dash panel vs. inline/hidden trimpot set once.

---
*Created: July 4, 2026*
*Revised: August 30, 2026 — STM32 PWM approach dropped in favour of an external dimmer; PA10 committed to the one-wire temperature bus.*
