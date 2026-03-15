// ============================================================
// Niva Dashboard — STM32 ADC/Sensor Module
// ============================================================
//
// Replaces Arduino Mega as the sensor acquisition module.
// Reads analog sensors, digital indicators, pulse signals, and K-Line,
// then sends a unified data frame over USB-serial to Raspberry Pi at 50 Hz.
//
// Target MCU: STM32F103C8T6 ("Blue Pill"), 72 MHz, 3.3V logic
//
// Protocol (ASCII, same format as Arduino version):
//   "$A0,A1,A2,A3,TACHO,SPEED,D0..D9,B0..B7\n"
//   - A0..A3:  raw 12-bit analog values (0-4095)
//   - TACHO:   pulse count since last report (tachometer, 2 PPR)
//   - SPEED:   pulse count since last report (speed sensor, 4 PPR)
//   - D0..D9:  digital indicator states (0/1)
//   - B0..B7:  button states (0/1, 1 = pressed)
//
// NOTE: All 12V car signals MUST go through appropriate voltage dividers
//       or level shifters before reaching the 3.3V STM32 pins.
//       K-Line uses an L9637D adapter (12V↔5V) + BSS138 level shifter (5V↔3.3V).
//
// ============================================================
// Pin Mapping — STM32F103C8T6
// ============================================================
//
// === Analog Inputs (12-bit ADC, 0-3.3V, via voltage dividers) ===
//
//   PA0 (ADC1_CH0) — Oil pressure sensor (analog)
//   PA1 (ADC1_CH1) — Fuel level sensor (analog)
//   PA2 (ADC1_CH2) — Coolant temperature sensor (analog)
//   PA3 (ADC1_CH3) — 12V system voltage (analog, via divider)
//
// === Pulse/Counter Inputs (interrupt-capable) ===
//
//   PA8  (TIM1_CH1) — Tachometer signal, 2 pulses per revolution
//   PA9  (TIM1_CH2) — Speed sensor signal, 4 pulses per revolution
//
//   Using hardware timer input capture for jitter-free pulse counting.
//   5V sensor signals need a voltage divider or level shifter to 3.3V.
//
// === Digital Indicator Inputs ===
//
//   PB0  — Oil pressure low warning        (INPUT_PULLUP, active-low)
//   PB1  — Fuel low warning                (INPUT_PULLUP, active-low)
//   PB3  — Charging indicator              (INPUT_PULLUP, active-low)
//   PB4  — Exterior lights on              (INPUT, active-high)
//   PB5  — Brake fluid low                 (INPUT, active-high)
//   PB6  — Headlights on                   (INPUT, active-high)
//   PB7  — Turn signal on                  (INPUT, active-high)
//   PB8  — High beams on                   (INPUT, active-high)
//   PB9  — Parking brake on                (INPUT_PULLUP, active-low)
//   PA15 — Diff lock on                    (INPUT_PULLUP, active-low)
//
//   All 12V-level digital signals need external level conversion to 3.3V.
//
// === Dashboard Buttons (active-low, internal pull-up) ===
//
//   PB12 — Button 0 (left column, top)
//   PB13 — Button 1 (left column, 2nd)
//   PB14 — Button 2 (left column, 3rd)
//   PB15 — Button 3 (left column, bottom)
//   PA4  — Button 4 (right column, top)
//   PA5  — Button 5 (right column, 2nd)
//   PA6  — Button 6 (right column, 3rd)
//   PA7  — Button 7 (right column, bottom)
//
//   Directly connected to 3.3V logic — no level conversion needed.
//   Buttons short to GND when pressed; internal pull-ups are enabled.
//
// === K-Line Interface (OBD-II diagnostics, ISO 9141/14230) ===
//
//   PB10 (USART3_TX) — K-Line TX (to transceiver)
//   PB11 (USART3_RX) — K-Line RX (from transceiver)
//
//   Signal chain: K-Line bus (12V) ↔ L9637D adapter (5V) ↔ BSS138 shifter (3.3V) ↔ STM32
//
//   L9637D adapter: converts 12V K-Line bus to 5V UART.
//   BSS138 4-ch bidirectional level shifter: converts 5V ↔ 3.3V.
//
//   BSS138 board wiring:
//     HV  → 5V  (from L9637D adapter VCC)
//     LV  → 3.3V (from STM32 3.3V rail)
//     GND → common ground
//     HV1 → L9637D TX pin (5V UART out to K-Line bus)
//     LV1 → STM32 PB10 (USART3_TX)
//     HV2 → L9637D RX pin (5V UART in from K-Line bus)
//     LV2 → STM32 PB11 (USART3_RX)
//
//   USART3 configured at 10400 baud (ISO 9141-2 / KWP2000 slow init).
//
// === USB (data link to Raspberry Pi) ===
//
//   PA11 (USB_DM) — USB D-
//   PA12 (USB_DP) — USB D+
//
//   Native USB on STM32F103. Used for the main serial data stream.
//
// ============================================================
// Pin Conflict Resolution
// ============================================================
//
//   PA11 is shared between USART1_TX and USB_DM.
//   Resolution: K-Line uses USART3 (PB10/PB11) instead of USART1.
//
//   PB10 was originally Diff lock indicator.
//   Resolution: Diff lock moved to PA15.
//
// ============================================================
// Final Pin Assignment Summary
// ============================================================
//
//   Pin   | Function                | Type        | Notes
//   ------|-------------------------|-------------|---------------------------
//   PA0   | Oil pressure (analog)   | ADC_IN0     | Voltage divider from sensor
//   PA1   | Fuel level (analog)     | ADC_IN1     | Voltage divider from sensor
//   PA2   | Coolant temp (analog)   | ADC_IN2     | Voltage divider from sensor
//   PA3   | 12V voltage (analog)    | ADC_IN3     | Resistive divider 20V→3.3V
//   PA4   | Button 4                | GPIO IN PU  | Active-low, 3.3V direct
//   PA5   | Button 5                | GPIO IN PU  | Active-low, 3.3V direct
//   PA6   | Button 6                | GPIO IN PU  | Active-low, 3.3V direct
//   PA7   | Button 7                | GPIO IN PU  | Active-low, 3.3V direct
//   PA8   | Tachometer pulse        | TIM1_CH1    | Ext. level shift to 3.3V
//   PA9   | Speed sensor pulse      | TIM1_CH2    | Ext. level shift to 3.3V
//   PA10  | (free / future use)     |             |
//   PA11  | USB D-                  | USB         | To Raspberry Pi
//   PA12  | USB D+                  | USB         | To Raspberry Pi
//   PA15  | Diff lock indicator     | GPIO IN PU  | Active-low, level shifted
//   PB0   | Oil pressure warning    | GPIO IN PU  | Active-low, level shifted
//   PB1   | Fuel low warning        | GPIO IN PU  | Active-low, level shifted
//   PB3   | Charging indicator      | GPIO IN PU  | Active-low, level shifted
//   PB4   | Exterior lights         | GPIO IN     | Active-high, level shifted
//   PB5   | Brake fluid low         | GPIO IN     | Active-high, level shifted
//   PB6   | Headlights on           | GPIO IN     | Active-high, level shifted
//   PB7   | Turn signal on          | GPIO IN     | Active-high, level shifted
//   PB8   | High beams on           | GPIO IN     | Active-high, level shifted
//   PB9   | Parking brake on        | GPIO IN PU  | Active-low, level shifted
//   PB10  | K-Line TX (USART3_TX)   | UART TX     | Via L9637D + BSS138 shifter
//   PB11  | K-Line RX (USART3_RX)   | UART RX     | Via L9637D + BSS138 shifter
//   PB12  | Button 0                | GPIO IN PU  | Active-low, 3.3V direct
//   PB13  | Button 1                | GPIO IN PU  | Active-low, 3.3V direct
//   PB14  | Button 2                | GPIO IN PU  | Active-low, 3.3V direct
//   PB15  | Button 3                | GPIO IN PU  | Active-low, 3.3V direct
//
//   Reserved/used by system:
//   PA13  | SWDIO                   | Debug       | SWD programming
//   PA14  | SWCLK                   | Debug       | SWD programming
//   PB2   | BOOT1                   | Boot config | Tie to GND for normal boot
//   PC13  | On-board LED            | GPIO OUT    | Heartbeat / status blink
//   PC14  | OSC32_IN                | RTC crystal | (if 32kHz crystal fitted)
//   PC15  | OSC32_OUT               | RTC crystal | (if 32kHz crystal fitted)
//
// ============================================================
// Free pins (available for future expansion):
//   PA10, PB2 (if BOOT1 not needed at runtime)
// ============================================================

#include <Arduino.h>

// TODO: Implement STM32 firmware
// This file currently serves as pin mapping documentation.
// Full implementation will follow.
