# STM32 ADC Module — Input Pins by Type

Wiring reference for the STM32F103C8T6 sensor acquisition module.
All 12V car signals require external level conversion to 3.3V.

## Protection notes

All voltage divider outputs use a **1N4728A** (DO-41, 3.3V, 1W Zener) to GND as
an overvoltage clamp. This is a standard through-hole Zener, widely available and
easy to hand-solder. The 1W rating handles automotive transients comfortably.

Analog filtering is handled in firmware via oversampling and moving averages
(see `analog_signal_processing.rs`). Hardware filter capacitors are omitted
except on pulse inputs, where they are required to suppress ringing that
firmware cannot correct after the fact.

All resistors are **1/4W carbon or metal film, through-hole** (axial, standard pitch).

---

## Analog inputs (voltage divider to 0–3.3V)

All analog sensors are tapped from existing car wiring (12V system).
Sensor wire voltages vary in the 0–12V range depending on sensor resistance
and the instrument cluster's series resistor. Design for 0–16V input range
to provide headroom for alternator voltage (14.4V) and minor transients.

Each analog input uses a resistive divider + Zener clamp.
No hardware filter capacitors — filtering is done in firmware.

```
Sensor wire ── R1 ──┬── PA_x (ADC input)
                     │
                    R2      D1 (3.3V Zener)
                     │        │
                    GND      GND
```

### PA0, PA1, PA2 — Sensor inputs (0–16V range)

Oil pressure, fuel level, and coolant temperature sensors.
Tapped from existing car instrument cluster wiring.
Target: 16V input → 3.3V at ADC pin.

| Component | Value | Notes |
|-----------|-------|-------|
| R1 (top)  | 39 kΩ | 1/4W through-hole axial |
| R2 (bottom) | 10 kΩ | 1/4W through-hole axial; Vout = 16V × 10k/(39k+10k) = 3.27V |
| D1 | 1N4728A (3.3V Zener, DO-41) | Overvoltage clamp to protect ADC |

Divider ratio: ×0.204. Normal 12V sensor range maps to 0–2.45V at ADC.
Divider impedance: ~7.96 kΩ (within STM32 ADC recommended ≤10 kΩ source).
At 12-bit resolution: ~0.81 mV/count → ~3.9 mV/count referred to input.

### PA3 — 12V system voltage (0–20V range)

Direct measurement of the car battery/alternator voltage.
Target: 20V input → 3.3V at ADC pin (allows detecting overvoltage/regulator failure).

| Component | Value | Notes |
|-----------|-------|-------|
| R1 (top)  | 51 kΩ | 1/4W through-hole axial; higher impedance acceptable for voltage sensing |
| R2 (bottom) | 10 kΩ | 1/4W through-hole axial; Vout = 20V × 10k/(51k+10k) = 3.28V |
| D1 | 1N4728A (3.3V Zener, DO-41) | Clamps load dump spikes |

Divider ratio: ×0.164. Normal 14V → 2.30V at ADC.
At 12-bit resolution: ~0.81 mV/count → ~4.9 mV/count referred to input.

---

## Pulse inputs (level shifted to 3.3V, timer input capture)

Tachometer and speed sensor output 0–12V square waves.
Use a voltage divider to bring the signal within 3.3V range, plus a Zener clamp.
A small filter capacitor is **required** here — it suppresses wire ringing and
ignition noise that would cause false edge counts on the hardware timer.
Software cannot correct spurious edges after they have already been counted.

```
Pulse sensor ── R1 ──┬── PA8/PA9 (TIM1 input)
                      │
                     R2      C1 (1nF)     D1 (3.3V Zener)
                      │        │              │
                     GND      GND            GND
```

| Component | Value | Notes |
|-----------|-------|-------|
| R1 (top)  | 10 kΩ | 1/4W through-hole axial; limits current during transients |
| R2 (bottom) | 3.9 kΩ | 1/4W through-hole axial; Vout = 12V × 3.9k/(10k+3.9k) = 3.37V |
| C1 | 1 nF ceramic | Radial, 2.54mm pitch; suppresses ringing, preserves pulse edges |
| D1 | 1N4728A (3.3V Zener, DO-41) | Clamps spikes from ignition noise |

| Pin | Signal |
|-----|--------|
| PA8 | Tachometer (2 PPR) |
| PA9 | Speed sensor (4 PPR) |

---

## Digital inputs, active-low via divider (12V→3.3V, INPUT_PULLUP)

These sensors idle at 12V (open) and short to GND when active.
The STM32 internal pull-up holds the pin high when the sensor side
is open-circuit (wire disconnected). The divider scales 12V down to 3.3V.

```
Car 12V line ── R1 ──┬── PB_x (GPIO input, pull-up enabled)
                      │
                     R2      D1 (3.3V Zener)
                      │        │
                     GND      GND
```

| Component | Value | Notes |
|-----------|-------|-------|
| R1 (top)  | 10 kΩ | 1/4W through-hole axial; current-limiting, also forms divider with R2 |
| R2 (bottom) | 3.9 kΩ | 1/4W through-hole axial; Vout = 12V × 3.9k/(10k+3.9k) = 3.37V → clamped to 3.3V by Zener |
| D1 | 1N4728A (3.3V Zener, DO-41) | Overvoltage clamp |

No filter cap needed — digital signals, read in software with debouncing.
When sensor shorts to GND: pin sees 0V (logic low, active state).
When sensor open: 12V through divider → ~3.3V (logic high, idle state).

| Pin  | Signal |
|------|--------|
| PB0  | Oil pressure low warning |
| PB1  | Fuel low warning |
| PB3  | Charging indicator |
| PB9  | Parking brake on |
| PA15 | Diff lock on |

---

## Digital inputs, active-high via divider (12V→3.3V, INPUT floating)

These signals are 0V when inactive and 12V when active.
Same divider as active-low, but no internal pull-up (pin floats when signal is 0V,
which reads as logic low). R2 provides a weak pull-down to ensure a clean 0V at rest.

```
Car 12V line ── R1 ──┬── PB_x (GPIO input, no pull-up)
                      │
                     R2      D1 (3.3V Zener)
                      │        │
                     GND      GND
```

| Component | Value | Notes |
|-----------|-------|-------|
| R1 (top)  | 10 kΩ | 1/4W through-hole axial; current-limiting + divider |
| R2 (bottom) | 3.9 kΩ | 1/4W through-hole axial; also acts as pull-down when signal is 0V |
| D1 | 1N4728A (3.3V Zener, DO-41) | Overvoltage clamp |

When signal is 12V: divider output ~3.3V → logic high.
When signal is 0V: R2 pulls pin to GND → logic low.

| Pin | Signal |
|-----|--------|
| PB4 | Exterior lights on |
| PB5 | Brake fluid low |
| PB6 | Headlights on |
| PB7 | Turn signal on |
| PB8 | High beams on |

---

## Buttons, 3.3V direct (INPUT_PULLUP, active-low, no divider)

Buttons connect directly to 3.3V STM32 pins — no voltage conversion needed.
Each button shorts to GND when pressed; internal pull-up holds pin high when released.
Debouncing is handled in firmware. No hardware capacitors needed.

Optional: add a **1N4148** (DO-35 through-hole) per pin for ESD protection
if buttons have long wire runs — anode to GND, cathode to pin.

```
STM32 pin (pull-up) ── Button ── GND
```

| Pin  | Signal |
|------|--------|
| PB12 | Button 0 (left top) |
| PB13 | Button 1 (left 2nd) |
| PB14 | Button 2 (left 3rd) |
| PB15 | Button 3 (left bottom) |
| PA4  | Button 4 (right top) |
| PA5  | Button 5 (right 2nd) |
| PA6  | Button 6 (right 3rd) |
| PA7  | Button 7 (right bottom) |

---

## UART via L9637D + BSS138 level shifter (K-Line)

No voltage dividers needed — the L9637D handles 12V↔5V,
and the BSS138 board handles 5V↔3.3V. See the main sketch file
for BSS138 board wiring details.

| Pin  | Signal |
|------|--------|
| PB10 | K-Line TX (USART3_TX) |
| PB11 | K-Line RX (USART3_RX) |

---

## Bill of Materials — Protection/Divider Components

| Component | Package | Quantity | Used for |
|-----------|---------|----------|----------|
| 39 kΩ resistor, 1/4W | Axial through-hole | 3 | Analog sensor divider R1 (PA0, PA1, PA2) |
| 51 kΩ resistor, 1/4W | Axial through-hole | 1 | 12V voltage divider R1 (PA3) |
| 10 kΩ resistor, 1/4W | Axial through-hole | 16 | Analog R2 (×4), pulse R1 (×2), digital R1 (×10) |
| 3.9 kΩ resistor, 1/4W | Axial through-hole | 12 | Pulse R2 (×2), digital R2 (×10) |
| 1N4728A Zener 3.3V, 1W | DO-41 through-hole | 16 | All divider outputs (4 analog + 2 pulse + 10 digital) |
| 1 nF ceramic capacitor | Radial, 2.54mm pitch | 2 | Pulse input ringing suppression (PA8, PA9) |
| 1N4148 signal diode (optional) | DO-35 through-hole | 8 | ESD protection for button pins |
