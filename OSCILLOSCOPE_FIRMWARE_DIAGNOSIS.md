# Oscilloscope Mode — Firmware Diagnosis

*Session date: 2026-09-07. Bench Pi, STM32 ADC module on `/dev/niva_adc` (`ttyACM0`).*

## Symptom

Oscilloscope mode stopped working. PA3 fed from a bench supply (~2.2 V at the pin
per multimeter).

## Conclusion

**The STM32 module is running old firmware that does not service its USB CDC OUT
endpoint.** The `$OSCCAP` capture command never reaches the MCU, so oscilloscope
mode cannot function. A reflash was attempted between test rounds but **did not
take** — post-reflash behavior is identical to pre-reflash.

The Pi-side path (`ADCDataProvider::perform_osc_capture`, `osc_page.rs`) was never
implicated.

## Evidence

Tested with `scripts/test_osc_capture.py` plus ad-hoc probes, dashboard stopped
(`sudo systemctl stop niva-dashboard` — it holds the port exclusively).

| Test | Result |
|---|---|
| Telemetry RX (`$A…` frames) | ✅ healthy, ~50–53 frames/s throughout |
| A3 (12V-bus) channel raw | ~2276 round 1, ~2510 round 2 (bench supply changed); stable ±5 LSB |
| `write("$OSCCAP\n")` from host | ❌ `write()` / `tcdrain()` blocks **forever** |
| `write("$VER\n")` from host | ❌ same infinite block, even for a single byte |
| Plain blocking write vs. `write_timeout` | Both hang — not a pyserial `select()` artifact |
| `$OSCD` / `$OSCEND` / `$OSCACK` response | ❌ none, ever |
| `$VER,<revision>` boot banner | ❌ not seen when opening the port ~0.9 s after a forced reboot |
| dmesg after each write attempt | no USB disconnect — MCU does **not** crash on RX, it just never drains EP1 OUT |

### Why the write hangs (mechanism)

- USB descriptor is fine: EP `0x01` BULK OUT is present
  (`lsusb -d 0483:5740 -v`).
- tty is clean: `-crtscts`, `clocal`, `-ixon -ixoff` — no flow-control stall.
- The device never calls `Serial.read()` / re-arms `USBD_CDC_ReceivePacket`, so
  after the first packet the CDC OUT endpoint stays NAKed. The host's write URB
  never completes → kernel `write()`/`tcdrain()` never returns. This is the exact
  "firmware that doesn't drain its RX buffer can lock up" failure noted in
  `.claude/CLAUDE.md` (ADC Module Connectivity).

### Firmware version check

- Freshly-booted module (forced via `sudo uhubctl -l 1-1 -a 2`, the documented
  hub power-cycle) emits **no `$VER` banner** catchable ~0.9 s post-boot.
- Repo `stm32_adc_module/Niva_Dashboard_ADC_Module/src/main.cpp` at the time of
  this session contains `$OSCCAP` handling (`poll_incoming_commands`,
  `dispatch_command`, `run_oscilloscope_capture`) but **no `$VER`** — `grep VER`
  finds only comments. So the `$VER` feature is either uncommitted or the push was
  incomplete.

## What to check on the flashing side

1. Upload actually completed and the board **reset/ran** afterward (ST-Link
   `program` without `reset run` leaves the old image executing). A hub
   power-cycle here already forced a real reboot — still old behavior — so a
   missing reset alone doesn't explain it.
2. Correct PlatformIO environment, `upload_protocol = stlink`, build from the
   updated source tree, no flash-write error / read-out protection in the log.
3. ST-Link wired to SWDIO/SWCLK on *this* board (not just powering it).
4. Commit/push the `$VER` change if it's meant to be part of "latest".

## Verifying the fix

With a build that services USB RX flashed:

```
sudo systemctl stop niva-dashboard
python3 scripts/test_osc_capture.py
sudo systemctl start niva-dashboard
```

A working module answers `$OSCACK` immediately (script distinguishes "command
never received" from "capture produced nothing"), then 64 `$OSCD` chunks and
`$OSCEND`, and the script prints raw/volt stats plus an ASCII plot of the PA3
window.

## Side notes

- **PA3 scale:** raw 2276 ⇒ ~1.83 V assuming 3.3 V Vref, vs. 2.2 V on the meter
  (fixed ~0.83× ratio). Unrelated to the oscilloscope fault; check meter probe
  point vs. the "20 V→3.3 V divider" on the pin, or the module's VDDA/Vref.
- `scripts/test_osc_capture.py` gained `write_timeout=3.0` and `$OSCACK`
  handling during this session so it fails fast instead of hanging on a
  non-draining module.
- Memory written: `project_stm32_firmware_stale_no_osccap`.
