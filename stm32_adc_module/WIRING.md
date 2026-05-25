# STM32 ADC Module — Input Pins by Type

Wiring reference for the STM32F103C8T6 sensor acquisition module.
All 12V car signals require external level conversion to 3.3V.

---

## Power Supply

### Architecture

All system components are powered from a single regulated **5V rail** derived from the
car's ignition-switched 12V line via a **TPS40057-based synchronous buck converter**
(5V / 5A output).

```
Car battery ── F1 (5A fuse) ── D1 (SS34 Schottky) ──┬── TPS40057 buck converter (5V/5A)
                                                       │         │
                                            (permanent │)        │ 5V regulated
                                          for UPS charging       │
                                                       │    ┌────┴──────────────────────┐
                                                       │    │                           │
                                               ┌───────▼──────────┐         ┌───────────▼──────┐
                                               │  Pi UPS HAT      │         │  Display (USB)   │
                                               │  (5V USB input)  │         └──────────────────┘
                                               └───────┬──────────┘
                                                       │ powers
                                                ┌──────▼──────┐
                                                │ Raspberry Pi │
                                                │              ├──USB──► STM32 (data + VBUS)
                                                └─────────────┘
```

### Powered components

| Component         | Supply source              | Typical current |
|-------------------|----------------------------|-----------------|
| Raspberry Pi 4    | Pi UPS HAT (from 5V rail)  | 1.5–2.0 A       |
| Display (7" HDMI) | 5V rail (USB)              | 0.4–0.8 A       |
| STM32F103C8T6     | RPi USB port (VBUS)        | ~0.05 A         |
| UPS HAT overhead  | 5V rail                    | ~0.3 A          |
| **Total**         |                            | **~2.3–3.1 A**  |

The 5A converter rating provides ~2A of headroom for display backlight peaks and
cold-start current spikes.

**STM32 power note:** The STM32 is powered entirely through its USB data cable to the
Raspberry Pi. The RPi USB port supplies VBUS (5V), which feeds the Blue Pill's onboard
AMS1117-3.3V LDO. No separate regulator or power wire is needed for the STM32.

### TPS40057 converter

| Parameter       | Value                     |
|-----------------|---------------------------|
| Topology        | Synchronous buck (controller) |
| Input voltage   | Up to 40V (covers automotive load dumps) |
| Output voltage  | 5V                        |
| Output current  | 5A                        |
| Efficiency      | ~90%+ at typical load     |

### Input protection

```
Car 12V ── F1 (5A blade fuse) ── D1 (SS34 Schottky) ── TPS40057 VIN
```

| Component | Value / Part    | Purpose                                      |
|-----------|-----------------|----------------------------------------------|
| F1        | 5A automotive blade fuse | Protects car wiring from short circuit |
| D1        | SS34 Schottky diode     | Reverse polarity protection (~0.3V drop at 3A) |

No external TVS diode is required — the TPS40057's 40V-rated VIN handles
automotive load dump transients directly.

### Ignition vs. permanent 12V

- **Ignition-switched 12V** → TPS40057 VIN. System powers on/off with the car.
- **Permanent 12V** (optional) → Pi UPS HAT battery charging input directly.
  Keeps the UPS battery topped up and allows the RPi to complete a clean
  shutdown after ignition is cut, preventing SD card corruption.

Verify the TPS40057 module's **VREG pin** (gate driver bias, rated 4.5–13.2V) is
internally tied to VIN through a zener or LDO on your specific module, as is typical
for automotive-input designs.

---

## Protection notes

All voltage divider outputs use a **BZX55C3V6** (DO-35, 3.6V, 500mW Zener) to GND as
an overvoltage clamp. The 3.6V rating provides safe margin above the divider's normal
output (≤3.37V) while clamping before the STM32 absolute maximum input voltage (3.6V).
DO-35 glass package — same size as a 1N4148, easy to hand-solder.

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
                    R2      D1 (3.6V Zener)
                     │        │
                    GND      GND
```

### PA0, PA1, PA2 — Sensor inputs (0–16V range)

Oil pressure, fuel level, and coolant temperature sensors.
Tapped from existing car instrument cluster wiring.
Target: 16V input → 3.3V at ADC pin.

| Component   | Value                       | Notes                                                       |
|-------------|-----------------------------|-------------------------------------------------------------|
| R1 (top)    | 39 kΩ                       | 1/4W through-hole axial                                     |
| R2 (bottom) | 10 kΩ                       | 1/4W through-hole axial; Vout = 16V × 10k/(39k+10k) = 3.27V |
| D1          | BZX55C3V6 (3.6V Zener, DO-35) | Overvoltage clamp to protect ADC                            |

Divider ratio: ×0.204. Normal 12V sensor range maps to 0–2.45V at ADC.
Divider impedance: ~7.96 kΩ (within STM32 ADC recommended ≤10 kΩ source).
At 12-bit resolution: ~0.81 mV/count → ~3.9 mV/count referred to input.

### PA3 — 12V system voltage (0–20V range)

Direct measurement of the car battery/alternator voltage.
Target: 20V input → 3.3V at ADC pin (allows detecting overvoltage/regulator failure).

| Component   | Value                       | Notes                                                                    |
|-------------|-----------------------------|--------------------------------------------------------------------------|
| R1 (top)    | 51 kΩ                       | 1/4W through-hole axial; higher impedance acceptable for voltage sensing |
| R2 (bottom) | 10 kΩ                       | 1/4W through-hole axial; Vout = 20V × 10k/(51k+10k) = 3.28V              |
| D1          | BZX55C3V6 (3.6V Zener, DO-35) | Clamps load dump spikes                                                  |

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
                     R2      C1 (1nF)     D1 (3.6V Zener)
                      │        │              │
                     GND      GND            GND
```

| Component   | Value                       | Notes                                                           |
|-------------|-----------------------------|-----------------------------------------------------------------|
| R1 (top)    | 10 kΩ                       | 1/4W through-hole axial; limits current during transients       |
| R2 (bottom) | 3.9 kΩ                      | 1/4W through-hole axial; Vout = 12V × 3.9k/(10k+3.9k) = 3.37V   |
| C1          | 1 nF ceramic                | Radial, 2.54mm pitch; suppresses ringing, preserves pulse edges |
| D1          | BZX55C3V6 (3.6V Zener, DO-35) | Clamps spikes from ignition noise                               |

| Pin | Signal               |
|-----|----------------------|
| PA8 | Tachometer (2 PPR)   |
| PA9 | Speed sensor (4 PPR) |

---

## Digital inputs, active-low via divider (12V→3.3V, INPUT_PULLUP)

These sensors idle at 12V (open) and short to GND when active.
The STM32 internal pull-up holds the pin high when the sensor side
is open-circuit (wire disconnected). The divider scales 12V down to 3.3V.

```
Car 12V line ── R1 ──┬── PB_x (GPIO input, pull-up enabled)
                      │
                     R2      D1 (3.6V Zener)
                      │        │
                     GND      GND
```

| Component   | Value                       | Notes                                                                                    |
|-------------|-----------------------------|------------------------------------------------------------------------------------------|
| R1 (top)    | 10 kΩ                       | 1/4W through-hole axial; current-limiting, also forms divider with R2                    |
| R2 (bottom) | 3.9 kΩ                        | 1/4W through-hole axial; Vout = 12V × 3.9k/(10k+3.9k) = 3.37V → clamped to 3.6V by Zener |
| D1          | BZX55C3V6 (3.6V Zener, DO-35) | Overvoltage clamp                                                                        |

No filter cap needed — digital signals, read in software with debouncing.
When sensor shorts to GND: pin sees 0V (logic low, active state).
When sensor open: 12V through divider → ~3.3V (logic high, idle state).

| Pin  | Signal                   |
|------|--------------------------|
| PB0  | Oil pressure low warning |
| PB1  | Fuel low warning         |
| PB3  | Charging indicator       |
| PB9  | Parking brake on         |
| PA15 | Diff lock on             |

---

## Digital inputs, active-high via divider (12V→3.3V, INPUT floating)

These signals are 0V when inactive and 12V when active.
Same divider as active-low, but no internal pull-up (pin floats when signal is 0V,
which reads as logic low). R2 provides a weak pull-down to ensure a clean 0V at rest.

```
Car 12V line ── R1 ──┬── PB_x (GPIO input, no pull-up)
                      │
                     R2      D1 (3.6V Zener)
                      │        │
                     GND      GND
```

| Component   | Value                       | Notes                                                             |
|-------------|-----------------------------|-------------------------------------------------------------------|
| R1 (top)    | 10 kΩ                       | 1/4W through-hole axial; current-limiting + divider               |
| R2 (bottom) | 3.9 kΩ                      | 1/4W through-hole axial; also acts as pull-down when signal is 0V |
| D1          | BZX55C3V6 (3.6V Zener, DO-35) | Overvoltage clamp                                                 |

When signal is 12V: divider output ~3.3V → logic high.
When signal is 0V: R2 pulls pin to GND → logic low.

| Pin | Signal             |
|-----|--------------------|
| PB4 | Exterior lights on |
| PB5 | Brake fluid low    |
| PB6 | Headlights on      |
| PB7 | Turn signal on     |
| PB8 | High beams on      |

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

| Pin  | Signal                  |
|------|-------------------------|
| PB12 | Button 0 (left top)     |
| PB13 | Button 1 (left 2nd)     |
| PB14 | Button 2 (left 3rd)     |
| PB15 | Button 3 (left bottom)  |
| PA4  | Button 4 (right top)    |
| PA5  | Button 5 (right 2nd)    |
| PA6  | Button 6 (right 3rd)    |
| PA7  | Button 7 (right bottom) |

---

## UART via L9637D + BSS138 level shifter (K-Line)

No voltage dividers needed — the L9637D handles 12V↔5V,
and the BSS138 board handles 5V↔3.3V. See the main sketch file
for BSS138 board wiring details.

| Pin  | Signal                |
|------|-----------------------|
| PB10 | K-Line TX (USART3_TX) |
| PB11 | K-Line RX (USART3_RX) |

---

## Bill of Materials — Protection/Divider Components

| Component                      | Package              | Quantity | Used for                                              |
|--------------------------------|----------------------|----------|-------------------------------------------------------|
| 39 kΩ resistor, 1/4W           | Axial through-hole   | 3        | Analog sensor divider R1 (PA0, PA1, PA2)              |
| 51 kΩ resistor, 1/4W           | Axial through-hole   | 1        | 12V voltage divider R1 (PA3)                          |
| 10 kΩ resistor, 1/4W           | Axial through-hole   | 16       | Analog R2 (×4), pulse R1 (×2), digital R1 (×10)       |
| 3.9 kΩ resistor, 1/4W          | Axial through-hole   | 12       | Pulse R2 (×2), digital R2 (×10)                       |
| BZX55C3V6 Zener 3.6V, 500mW    | DO-35 through-hole   | 16       | All divider outputs (4 analog + 2 pulse + 10 digital) |
| 1 nF ceramic capacitor         | Radial, 2.54mm pitch | 2        | Pulse input ringing suppression (PA8, PA9)            |
| 1N4148 signal diode (optional) | DO-35 through-hole   | 8        | ESD protection for button pins                        |
