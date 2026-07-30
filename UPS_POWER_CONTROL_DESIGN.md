# UPS HAT Auto Power-Off — Ignition-Controlled Design

## Problem
The UPS HAT (Waveshare UPS HAT (D), INA219 + onboard MCU at I2C `0x2D`) has no way to be told
"this power loss is long-term, fully power yourself down." `ups_monitor.rs` already detects
on-battery operation (INA219 current sign) and issues a graceful `sudo poweroff` after a
confirmation delay, arming the MCU's "boot when power applied" latch first. But once the Pi
halts, the HAT's MCU stays alive polling for mains return (that's what the boot-latch feature
requires) — a non-zero standby draw with nothing to stop it. The expected use pattern (car
parked, ignition off, for multiple days) turns that small standby draw into a real battery drain
over time.

The HAT has a physical 3-terminal slide switch that fully disconnects the battery-boost path —
but nothing today drives it automatically.

## Switch characterization
Probed with a continuity meter against the metal USB-port housing (chassis ground) in both
positions:

| Terminal                   | OFF position | ON position |
|----------------------------|--------------|-------------|
| 2 of 3 terminals           | grounded     | grounded    |
| 1 of 3 terminals ("sense") | **floating** | grounded    |

The two always-grounded terminals are the switch's frame/shield legs — not signal-carrying.
Only one terminal actually encodes switch position: floating = off, grounded = on. That terminal
is presumably read by the HAT's MCU (or directly gates the boost converter enable) through an
internal pull-up. Two mounting studs are purely mechanical.

This reduces the electrical problem to: **pull one pin to ground for "on," let it float for
"off."** No relay/SPDT hardware needed — a single small-signal N-channel MOSFET as an
open-drain pulldown replicates the switch exactly, including its own fail-safe: gate undriven →
high-impedance → pin floats → HAT reads "off," same as today's mechanical default.

## Proposed design
Ignition already feeds the UPS HAT's input (via TPS40057), so ignition-off already implies
mains-loss as seen by `ups_monitor.rs` — no separate ignition-sense wire into the Pi is needed.
The control board's only job is to hold the sense terminal grounded for a bounded window after
ignition drops, long enough for the Pi to finish its (now-shortened) software shutdown, then
release it — with zero components drawing current once released.

```
 Ignition (switched 12V)
        |
       [D1]  (blocking diode — stops C1 backfeeding the ignition line)
        |
        +--------------------+
        |                    |
       [C1]                 [R1]  (gate series resistor, limits charge inrush)
        |                    |
       GND                  [Q1 gate]
                              |
        +---------------------------+
        |                            |
       [R2]                    Q1 (N-MOSFET)
   (gate pulldown,           drain -> UPS sense terminal
    sets hold time            source -> GND
    together with C1)
        |
       GND
```

- **Ignition present:** C1 charges through D1; Q1's gate sits at ~ignition voltage minus one
  diode drop, well above any small-signal MOSFET's Vgs(th) (e.g. 2N7000, ~2.1V typ) — Q1 fully
  on, sense terminal held grounded, HAT stays in "on" state.
- **Ignition drops:** D1 blocks C1 from discharging back through the ignition wiring. C1
  discharges through R2 (the gate pulldown), so Q1 stays on for one RC-scaled window before its
  gate decays below threshold — hold time is tunable via R2·C1, independent of any software or
  microcontroller.
- **After the window elapses:** Q1 turns off, sense terminal floats, HAT reads "off" and fully
  disconnects the battery-boost path. Nothing in the circuit draws current from this point —
  true zero standby, unlike an always-on MCU-based controller.
- **Bonus:** a brief ignition dip (e.g. cranking) only partially discharges C1 before ignition
  returns and it recharges — the RC network naturally rides through short transients without any
  digital debounce logic.

### Physical install
Leave the existing slide switch in the **OFF** position (its contact then contributes nothing —
sense terminal floats through it) and tap Q1's drain onto the same sense pad in parallel. This
needs no desoldering of the switch, and doubles as a manual override: flipping the physical
switch to ON forces the sense terminal grounded regardless of what the control board is doing,
useful for bench work.

## Software change needed
`ups_monitor.rs`'s `ON_BATTERY_SHUTDOWN_DELAY` (currently 60s) exists to filter cranking-induced
current dips before the hardware ride-through above existed. With the RC network now absorbing
short transients at the switch level, this delay should shrink to just long enough to avoid
triggering `poweroff` on a real but brief dip — a few seconds, not 60. It must **not** go to
zero: a same-poll-interval shutdown would still fire on a 1-2s cranking dip, and even though the
capacitor hold means power isn't physically cut, the Pi would still halt and go through its
boot-on-power reboot cycle on every engine start.

The capacitor hold time (R2·C1) must comfortably exceed: shortened confirmation delay + actual
Pi shutdown duration, with margin for component tolerance (electrolytic capacitance drift is
typically ±20%, worse over automotive temperature range).

## Open decisions
- Confirm which of the 3 terminals is the sense pin with a labeled continuity test (this doc
  assumes it's identified, not yet pinned to a specific pin number/color).
- Measure actual Pi shutdown duration (`poweroff` issued → 5V rail current drops to near-zero)
  on the bench to set the confirmation delay and target hold time with real numbers instead of
  estimates.
- Final `ON_BATTERY_SHUTDOWN_DELAY` value (candidate: 3-5s).
- R1/R2/C1 values — target hold time should be roughly `confirmation delay + shutdown time +
  margin`; for the gate-decay time constant, time-to-threshold ≈ `R2 · C1 · ln(V_ignition / 2V)`.
  Watch for electrolytic self-leakage dominating the discharge if R2 is pushed into the MΩ range
  — may need a low-leakage cap or a more moderate R2/larger C1 pairing.
  **Bench-test starting point (not final):** C1 = 1000 µF / 25V, R2 = 10 kΩ (swap for a
  20-50 kΩ trimmer pot during bench tuning), R1 = 1 kΩ — gives t ≈ 18s nominal
  (`10kΩ · 1000µF · ln(12V/2V) ≈ 18s`), comfortably clear of the 3-5s confirmation delay plus an
  estimated few-second Pi shutdown, using round/on-hand part values. Retune once actual shutdown
  time is measured.
- D1/Q1 part selection — D1 needs to tolerate automotive line transients (general-purpose
  rectifier, e.g. 1N4001, rather than a small-signal diode); Q1 just needs Vgs(th) comfortably
  below ignition voltage minus the diode drop (most small-signal N-MOSFETs qualify).
- Whether to socket/breadboard this for bench validation of the hold timing before committing to
  a permanent install.

---
*Created: July 24, 2026*
