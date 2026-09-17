# K-Line Interface Design

## Problem
K-Line (ISO 9141 / ISO 14230 KWP2000) is a single-wire, open-drain, bidirectional bus idling at
battery voltage (~12V) through a pull-up. The STM32F103C8T6 ADC module's GPIOs are 3.3V logic.
Directly wiring K-Line to a 3.3V microcontroller pin is not viable, and a naive resistor divider
doesn't handle the open-drain TX direction (MCU needs to pull the line low, not just read it).

Two options were considered:

1. **Two-stage level shifter** (12V→5V→3.3V) between K-Line and the STM32 directly.
2. **Arduino Nano as protocol front-end**: Nano (5V logic) handles K-Line via a single 12V↔5V
   stage, then talks to the STM32 over UART.

## Nano-as-front-end: is 5V↔3.3V UART a problem?
Investigated as an alternative to the two-stage shifter, on the theory that a 5V↔3.3V UART hop
is a much simpler, better-understood problem than 12V↔3.3V. Findings:

- **STM32 TX (3.3V) → Nano RX**: works directly, no shifting. ATmega328 Vih at 5V supply is
  ~3.0V; 3.3V clears that with margin. Standard practice (same pattern as ESP32↔Arduino links).
- **Nano TX (5V) → STM32 RX**: needs care. Many STM32F103 GPIOs are 5V-tolerant ("FT" in the
  datasheet) when used as plain digital inputs — USART1_RX is typically one of them, but confirm
  against the exact pin before relying on it. Failing that, a 2-resistor divider (e.g. 1k/2k) on
  that one line resolves it trivially.
- Common ground required across all stages regardless of approach.

Conclusion: technically workable, but this approach still requires solving the 12V↔5V K-Line stage
*and* adds a second MCU (firmware, UART hop latency, extra power/failure domain) — worthwhile
only if there's a reason to offload K-Line protocol timing (e.g. ISO9141/KWP2000's 5-baud init)
onto the Nano specifically.

## Adopted approach: L9637D, single stage, no Nano
Before committing to either multi-stage option, checked whether a dedicated K-Line transceiver
IC could do the 12V→3.3V translation in one step. **L9637D** (STMicroelectronics, monolithic
ISO 9141 bus driver) does exactly this:

- **Vs** (battery-line pin): 4.5–36V operating (40V transient), reverse-supply protected to −24V.
  Connects to the car's 12V K-Line supply/battery line.
- **Vcc** (logic supply pin): **3–7V** — not 5V-only. Ties directly to the STM32's 3.3V rail.
- TX/RX logic thresholds are referenced to Vcc, so at Vcc=3.3V the TX/RX pins present 3.3V-level
  logic directly compatible with STM32 GPIOs/USART — no resistor dividers, no second MCU.

This eliminates both the two-stage shifter and the Nano front-end: one IC does the level
translation, wired straight into the STM32's USART.

**Wiring:**
```
Car K-Line (12V bus) ──── K pin (L9637D)
Car battery/ignition 12V ─ Vs pin (L9637D)
STM32 3.3V rail ────────── Vcc pin (L9637D)
STM32 USART_TX ─────────── TX pin (L9637D)
STM32 USART_RX ─────────── RX pin (L9637D)
Common GND ─────────────── GND pin (L9637D), STM32 GND, car chassis GND
```

**Open item:** exact TX input threshold / RX output swing numbers at Vcc=3.3V weren't confirmed
against the full ST datasheet (PDF fetch timed out during research) — worth a quick check against
STM32 GPIO Vih/Vil once wiring this up, though the Vcc=3–7V spec itself is solid from the
datasheet summary and Digi-Key's mirror.

## References
- [L9637 datasheet (ST)](https://www.st.com/resource/en/datasheet/l9637.pdf)
- [L9637D Datasheet mirror — Digi-Key](https://www.digikey.com/htmldatasheets/production/8727/0/0/1/l9637d.html)
- [L9637 product page — STMicroelectronics](https://www.st.com/en/automotive-analog-and-power/l9637.html)
