# Battery Monitor (Coulomb Counter) Design — Preliminary

Status: preliminary concept, nothing built or tested yet. Hardware choices below are candidates, not
final selections.

## Goal
Track the starter battery's (65 Ah lead-acid) charge in/out to estimate state of charge and expose
parked (key-off) current draw — including while the dashboard/Pi is off.

Current range to cover: parked draw of tens of mA up to ~300 A sustained (winch) and 200–400 A
cranking peaks. This sets the shunt at **500 A** (75 mV / 500 A, 0.15 mΩ, assumed below).

## Findings

### STM32F103 ADC is not suitable for direct shunt measurement
| Parameter | Value |
|---|---|
| Resolution | 12-bit over 0–3.3 V → ~0.8 mV/LSB |
| Reference | VREF+ internally tied to VDDA on the 48-pin package — no way to use a lower reference |
| Realistic ENOB | ~10–10.5 bits → 2–3 mV noise |
| Offset error | ~±1.5 LSB typ., plus INL/DNL |
| Inputs | Single-ended only, no PGA |

Even with a 75 mV / 100 A shunt (0.75 mΩ), 1 LSB ≈ 1.07 A; with the required 500 A shunt
(0.15 mΩ) it's ≈ 5.4 A. Parked draw (20–50 mA) on the 500 A shunt is 3–7.5 µV — far below one count.

Coulomb-counting specific problems:
- **Offset is integrated.** A 1 A offset = 24 Ah/day error. Oversampling improves resolution but
  not offset/INL, and zero current can't be guaranteed in the car to recalibrate.
- **Bidirectional current.** Charging current makes a low-side shunt voltage negative; the ADC
  can't read below 0 V without a level-shift.
- **No high-side option** — 12 V common mode is out of range.

An external current-sense amplifier (INA240/INA181 with mid-supply REF) would fix scaling and
bidirectionality, but offset drift still limits accuracy to ~10–50 mA — acceptable for an ammeter,
weak for a coulometer.

### INA228 is the preferred part
- 20-bit ΔΣ ADC, I2C, ±163.84 mV range (`ADCRANGE = 0`) at 312.5 nV/LSB → ~2.1 mA resolution with
  the 75 mV / 500 A shunt. The finer ±40.96 mV range (`ADCRANGE = 1`, ~0.5 mA/LSB) would clip at
  ~273 A, below winch current, so it's not usable.
- **Offset matters at this shunt value.** INA228 shunt offset is on the order of ±1 µV max (verify
  in datasheet) → up to ~±6.7 mA equivalent, i.e. up to ~0.16 Ah/day if integrated uncorrected.
  Small next to 65 Ah, but a sizeable fraction of a 20–50 mA parked-draw reading — see offset
  calibration in the software section.
- **Hardware charge accumulator** (`CHARGE` register) — integration happens on-chip, independent of
  whether the Pi is running.
- Common mode up to 85 V (high- or low-side), also measures bus voltage and die temperature.
- **Uses an external shunt.** The chip has no internal shunt; the shunt resistance is configured
  via the writable `SHUNT_CAL` register. (TI's INA780 is the integrated-shunt variant — limited to
  ~±78 A, unsuitable for cranking current.)

Alternatives considered: INA226 (16-bit, ~3.3 mA/LSB with this shunt, no accumulator — software
integration on the Pi), LTC2944 (dedicated coulomb counter, analog integrator).

### Adafruit INA228 breakout: onboard shunt must be removed
The breakout carries a 15 mΩ shunt between its VIN+/VIN− terminals. Per the Adafruit forum thread
"Adafruit INA228 external Shunt" (forums.adafruit.com, t=207502):
- Paralleling an external shunt with the onboard one is impractical — the onboard resistor plus
  connecting-wire resistance (~1 mΩ/ft for 10 AWG) share the current and add large, unstable error.
- Removing the onboard resistor eliminates both problems; the INA228 inputs draw negligible
  current, so sense-wire drop doesn't matter.
- Caveat from the thread: if the external shunt loses its connection, current takes the path
  through the sense wires and the chip, destroying it. See protection below.

### Power consumption is negligible
| Item | Current |
|---|---|
| INA228 VS, continuous conversion | ~640 µA typ. (~750 µA max) |
| VBUS input (~1 MΩ at 12 V) | ~10–15 µA |
| IN+/IN− bias | nA, negligible |
| Automotive LDO quiescent | ~3–15 µA (part-dependent) |

The low-power modes (shutdown, triggered one-shot) **can't be used** — the accumulator only
integrates in continuous conversion. Total ≈ 0.7 mA ≈ 0.5 Ah/month — smaller than lead-acid
self-discharge (~2–3 Ah/month for 65 Ah) and ~2–3 % of typical vehicle parked draw (15–36 Ah/month).
Not a practical concern.

### Ground disconnect switch interactions
Planned topology: `battery (−) → shunt → disconnect switch → chassis GND`.

- **Switch control wire.** The switch is purely mechanical (wired button pulses the coil, no
  receiver electronics), so standby draw is zero and only brief coil pulses bypass the shunt.
  Wiring its direct battery (−) lead to the battery terminal is fine. (If it were ever replaced by
  an RF-remote or non-latching type, that lead should move to the shunt/switch junction so its
  current is counted.)
- **Floating chassis with switch open.** Chassis is pulled toward +12 V through any load still
  connected to battery (+). If the INA228 were referenced to chassis/dashboard ground, its IN± pins
  (at battery −) would sit ~−12 V below its GND (abs. max −0.3 V), turning the chip's ESD diodes
  into a sneak ground path for the car's loads — destroying the chip and defeating the switch.
  → The INA228 must be referenced to battery (−) and its I2C link isolated (see design).
- Even with the switch closed, the two grounds differ by the shunt drop (up to ~75 mV at 500 A).
- The same concern applies to any other dashboard signal tapping the battery side of the switch
  (e.g. an STM32 battery-voltage input) — measure on the chassis side or isolate.

## Preliminary design

```
                 Battery (+) ──┬──────────────────────────────► vehicle loads
                               │
                     fuse + TVS/reverse diode
                               │
                     Automotive LDO 3.3 V (40 V in, µA Iq: TPS7B81 / TPS7B69 / LM2936)
                               │ VS
     ┌─────────────────────────┴──────────────┐          ┌───────────────┐
     │ INA228  (GND = battery −, battery side) │  I2C     │ I2C isolator  │  I2C    Raspberry Pi
     │   IN+ ◄─ 10 Ω ─ fuse ─┐                 ├─────────►│ ISO1541 /     ├────────► (shared bus
     │   IN− ◄─ 10 Ω ─ fuse ─┼─┐  (+ diff cap) │          │ ADuM1250      │          with UPS HAT)
     └─────────────────────── │─│──────────────┘          └───────────────┘
                              │ │                          side 1: LDO 3.3 V
                              │ │                          side 2: Pi 3.3 V
  Battery (−) ───────────[ SHUNT 75 mV/500 A ]─┬─────[ disconnect switch ]─── chassis GND
               Kelvin sense ┘ └ Kelvin sense   └─ (winch (−) here if the switch isn't rated for it)
```

Hardware notes:
- **Shunt sizing:** 500 A / 75 mV. Shunts are typically rated for continuous operation at ~2/3 of
  nominal (~330 A here), which covers sustained winching at ~300 A; cranking peaks (200–400 A,
  seconds) are within range. Dissipation at 300 A ≈ 13.5 W — mount with airflow, away from
  heat-sensitive parts. `ADCRANGE = 0` (±163.84 mV) leaves headroom above 500 A.
- **Winch return must go through the shunt.** Winch power cables are commonly bolted straight to
  the battery terminals; a winch (−) on the battery (−) post bypasses the shunt and makes all winch
  consumption invisible. Connect it to the load side of the shunt — to the shunt/switch junction if
  the disconnect switch isn't rated for winch current, or after the switch if it is.
  Winch (+) stays on battery (+) as usual.
- **Kelvin sense wiring** from the shunt's sense screws, never from the current lugs.
- **Sense-line protection:** small fuse (~100–250 mA) in each sense line, mounted at the shunt, plus
  ~10 Ω series resistors and a differential filter capacitor at the chip (keep resistors small —
  they form gain error with input bias current). Mount the shunt's load-side connection securely;
  a loose lug is the realistic way the "shunt disconnected" fault happens.
- **INA228 domain is always powered** from battery (+), so the accumulator keeps counting with the
  Pi off and the disconnect switch open.
- **Isolator's Pi side** is powered from the Pi's 3.3 V — only active when the Pi is on, which is
  the only time it's needed.
- Breakout vs. custom board: Adafruit breakout with the onboard shunt removed works; a small custom
  PCB (INA228 in 10-pin VSSOP + LDO + isolator) is the cleaner end state.

### Software (rough)
- New I2C hardware provider for the INA228 alongside the existing UPS I2C provider
  (`ups_i2c_provider.rs`), feeding the usual provider → processing → logical sensor chain.
- Init: write `SHUNT_CAL` from the measured shunt resistance, `ADCRANGE = 0`, continuous
  shunt+bus conversion. Must **not** reset the accumulator on dashboard start (`RSTACC`) — the
  count spans Pi power cycles.
- Logical sensors: battery current (signed), bus voltage, accumulated charge.
- Offset calibration: with the disconnect switch open, shunt current is truly zero (the switch is
  mechanical and the INA228/LDO are tapped on the battery side), so the reading then is pure offset.
  Store it and subtract offset × elapsed time from accumulated charge.
- SoC = stored reference + (current `CHARGE` − `CHARGE` at reference). The reference pair is
  persisted to disk by the Pi.
- Detect accumulator loss (INA228 lost power, e.g. battery disconnected/replaced) — e.g. via the
  power-on/reset state of config registers — and mark SoC as unknown until resynced.

## Open questions
- **SoC resync strategy.** Coulomb counting drifts (offset, charge efficiency of lead-acid
  ~85–95 %, Peukert effect). Candidate anchors: full-charge detection (high voltage + charging
  current tapered below a threshold), and resting open-circuit voltage after several hours of
  near-zero current mapped through an OCV→SoC table.
- **I2C address** — verify no conflict with the UPS HAT's devices (INA228 A0/A1 straps allow 16
  addresses).
- **Shunt choice** — 500 A / 75 mV assumed; confirm continuous rating/derating covers longest
  winch pulls. Measure actual resistance for `SHUNT_CAL` (cheap shunts are ±0.5–1 %).
- **Disconnect switch rating** vs. winch current — decides where the winch (−) attaches.
- **Measured INA228 offset** — whether real-world offset is small enough that the
  switch-open calibration is optional.
- **Self-consumption accounting** — LDO + INA228 tapped on the battery side of the shunt aren't
  counted; either subtract as a constant (~0.7 mA) or ignore.
- **UI** — which page shows battery current/SoC/parked-draw history.
