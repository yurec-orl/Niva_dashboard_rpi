# Oscilloscope Mode — USB Command-Path Diagnosis

*Sessions: 2026-09-07. Bench Pi + Windows dev box. STM32 ADC module ("Blue Pill",
STM32F103C8T6) on `/dev/niva_adc` (`ttyACM0`), behind USB hub `2109:3431` at `1-1`
(enumerates as `1-1.1`). PA3 fed from a bench supply (~2.2 V at the pin).*

> **This supersedes the earlier conclusion in this file.** The first pass blamed
> stale firmware that "does not service its USB CDC OUT endpoint". Deeper testing
> shows the flashed firmware *is* the current build, *is* running, and has its
> bulk-OUT endpoint correctly armed and waiting. The fault is **hardware: this
> STM32 board's USB device-side receive (host -> device DATA) path is dead.**

## Symptom

Oscilloscope mode does not work. `$OSCCAP` — and every other command — sent from
the Pi never reaches the STM32. Telemetry (`$A…`, `$T…`) streams normally.

## Conclusion

**The STM32 module receives nothing over USB.** Device -> host (IN) transfers work
flawlessly; host -> device bulk DATA OUT transfers never arrive. Enumeration and
the DTR line-state change still succeed because those need only SETUP packets
(ACKed by the USB peripheral in hardware) and IN transfers — no host -> device
DATA OUT packet is involved in either.

Ruled out this session, each with evidence:

- **Pi USB port / hub / cable.** The UM982 GNSS on the *same hub* does full
  bidirectional command/response — `VERSIONA`, `MODE`, `CONFIG`, `UNLOGLIST` each
  answered in ~0.1 ms, `write()`+`flush()` ~0.1–0.2 ms, `close()` 0.00 s, no
  kernel messages. A second USB cable enumerates and streams but shows the
  identical send failure.
- **Firmware / flash.** The flashed image is byte-verified against the current
  `firmware.elf` (`** Verified OK ** / ** Resetting Target **` from the PlatformIO
  upload). That ELF contains `$VER,135f9ad`, `$OSCCAP` / `$OSCACK` / `$OSCD` /
  `$OSCEND`, and the full USB-CDC receive stack (`USBD_CDC_Receive`,
  `CDC_resume_receive`, `CDC_ReceiveQueue_*`, `USBSerial::read`/`available`).
  Halting via SWD puts PC at `0x080004ba`, inside that build's
  `CDC_ReceiveQueue_ReadSize` (`0x08000486`) — i.e. the app is running and idling
  in `poll_incoming_commands()`'s `while (Serial.available())` poll.
- **Framework / library regression.** `framework-arduinoststm32` 4.21200.0,
  `platform-ststm32` 19.6.0, the gcc toolchain, `tool-openocd`, and
  `tool-stm32duino` are all unchanged since Feb–May 2026 — `find … -newermt
  2026-06-01` over both package trees is empty; the USB-CDC sources are dated
  2026-02-03. Only the PlatformIO **Core CLI** (`~/.platformio/penv`) updated, on
  2026-09-06, and that does not change the compiled binary for a fixed
  platform+framework. This is the same framework that built the firmware which
  worked in August.
- **The historically-good firmware, rebuilt and reflashed now, fails the same
  way.** Checked out and built commit
  `f5d3f2a8bdd01451574f5d5052f371881a915113` ("Add osc mode test; fix STM32 osc
  firmware bug") — the exact revision whose OSC path was "validated against real
  hardware" — flashed it, and `$OSCCAP` still gets no response. So the break is
  not in the current source, the DS18B20 additions, or the `$VER` changes, and
  not in the toolchain: the same source that once worked no longer does on this
  board.

That leaves the STM32 board's own USB receive hardware.

## Positive evidence of the fault

### SWD register dump — target running, verified current image

Addresses are build-specific, taken from the flashed `firmware.elf` (`nm`).

| What | Address | Before Pi `printf` | A few s after |
|---|---|---|---|
| `ReceiveQueue.write / .read / .length` | `0x20000516` | `0000 / 0000 / 00c0` | `0000 / 0000 / 00c0` — **unchanged; 0 bytes ever queued** |
| `receivePended` | `0x2000012c` | `01` | `01` — **receive armed, never serviced** |
| `dtrState` | `0x20000455` | `00` | `01` — **`SET_CONTROL_LINE_STATE` (SETUP) got through** |
| `EP0R` | `0x40005c00` | `3220` (CONTROL, STAT_RX VALID) | `6220` (STAT_RX NAK after the DTR xfer) |
| `EP1R` | `0x40005c04` | `3101` | `3101` — **BULK, EA 1, STAT_RX = VALID (armed), never receives** |
| `EP2R` | `0x40005c08` | `0022` | `0022` — BULK IN, EA 2 (telemetry, works) |
| `EP3R` | `0x40005c0c` | `0623` | `0623` — INTERRUPT IN, EA 3 (CDC notifications) |
| `USB_ISTR` | `0x40005c44` | `0000` | `0002` |
| `USB_DADDR` | `0x40005c4c` | `0082` | `0082` — enabled, address 2 |

`EP1R` STAT_RX (bits [13:12]) = `11` (VALID) before **and** after: the bulk-OUT
endpoint is armed and ready. When the Pi writes 8 bytes, `CTR_RX` (bit 15) never
sets, STAT_RX stays VALID, and the receive queue stays empty. The packet never
reaches the peripheral.

### Side-by-side, same hub, same run

| | GNSS (CH340 bridge) | STM32 ADC (native USB-CDC) |
|---|---|---|
| Open | OK | OK |
| RX (device -> Pi) | clean NMEA stream | clean `$…` / `$T` stream |
| `write()` of 1 byte | ~0.1 ms, OK | 3 s timeout |
| `close()` after write | 0.00 s | 31.8 s (stuck TX drain) |
| Command -> response | every time | never |

### dmesg — clean enumeration

```
usb 1-1.1: new full-speed USB device number 28 using xhci_hcd
usb 1-1.1: New USB device found, idVendor=0483, idProduct=5740, bcdDevice= 2.00
usb 1-1.1: New USB device strings: Mfr=1, Product=2, SerialNumber=3
usb 1-1.1: Product: GENERIC_F103C8TX CDC in FS Mode
usb 1-1.1: Manufacturer: STMicroelectronics
cdc_acm 1-1.1:1.0: ttyACM0: USB ACM device
```

Full speed (bulk endpoints legal), no descriptor-read retries, no `-EPROTO` /
`-EILSEQ`, driver binds first try. A marginal D+ pull-up would litter dmesg with
retries — on the Pi it does not. The device's pull-up and IN path are fine; only
the *receive* side is broken.

### D+ pull-up resistance

Measured **1.487 kΩ** across the D+ pin (PA12) and the 3V3 pin — right in the
1.5–1.7 kΩ target range for the "2 kΩ across the wrong 10 kΩ R10" bodge. **The
pull-up is intact**, so "the bodge joint went open" is ruled out. Whatever is
wrong is deeper: the D+/D- signal path, series resistors, the connector, or the
USB peripheral silicon / bond wires themselves.

## History / timeline (from the bench operator)

1. OSC mode confirmed working earlier — commit `f5d3f2a` "Add osc mode test; fix
   STM32 osc firmware bug"; the Rust capture path comment says "validated against
   real hardware". Not exercised again until after DS18B20 support.
2. Unknown whether OSC was retested between its initial implementation and the
   DS18B20 flash.
3. PlatformIO auto-updated (Core CLI) before the DS18B20 build. Flashing threw new
   errors, worked around by reconnecting the ST-Link; upload reported success;
   DS18B20 then confirmed working from the Pi (2 sensors enumerated + data).
   `git show f19fd9e:…/main.cpp` — the DS18B20 commit — has
   `poll_incoming_commands()` intact as the first statement of `loop()`, so that
   image necessarily contains working `$OSCCAP` handling.
4. Shortly after, OSC retested — broken.
5. `$VER` diagnostic added and flashed (reported success). Pi sees no `$VER`
   banner; commands still dead.
6. **Reverse-polarity insertion event** at some point: an STM32 plugged in wrong,
   frying its 3.3 V regulator; the Pi's USB power briefly dropped (load switch
   tripped). The board was replaced. "Everything seemed to work" afterward — but
   only power and *read* paths (telemetry, GNSS) were exercised. Host -> device
   DATA OUT was never tested post-incident. Where this falls relative to items 3–5
   is not certain.
7. This STM32 does **not** enumerate on a Windows PC at all — unresolved, possibly
   a D+ pull-up right at the electrical margin, or a Windows-side cable/port
   issue. Consistent with a weak device-side USB front end.

## Firmware & tooling changes made during diagnosis (committed `0a9155a`)

- `$OSCACK\n` emitted on `$OSCCAP` receipt, before the blocking capture — lets the
  Pi distinguish "command not received" from "capture produced nothing".
- Command parser tolerates a trailing `\r` (CRLF senders).
- `osc_write_all()` retry wrapper for the buffer dump — rides out transient host
  read-stalls (USB-CDC `write()` bails when the host lags > 3 ms), bounded by
  `OSC_SEND_TIMEOUT_MS`.
- `$VER,<rev>` at boot and on a `$VER` query; `git_rev.py` PlatformIO pre-build
  hook injects `FW_GIT_REV` (short git hash, `-dirty` suffix, `unknown` fallback).
- Rust: `LineSerialReader::write_line()` now flushes; `perform_osc_capture` /
  `run_osc_capture_test` consume `$OSCACK` and report "STM32 never acknowledged
  $OSCCAP …" when neither ack nor data arrives.
- `scripts/test_osc_capture.py`: `write_timeout=3.0`, prints `$OSCACK=yes/NO`,
  three-way failure advice.
- `OSCILLOSCOPE_DESIGN.md` protocol section updated.

## Next step — decisive

**Flash the current firmware onto the spare known-good STM32 and test `$VER` /
`$OSCCAP` from the Pi.**

```
sudo systemctl stop niva-dashboard
python3 scripts/test_osc_capture.py       # expect $OSCACK=yes, 64 chunks, $OSCEND
printf '$VER\n' > /dev/niva_adc           # expect "$VER,<rev>" back
sudo systemctl start niva-dashboard
```

- Command gets a reply -> the current board's USB receive hardware is dead;
  replace it. Everything software-side is already correct and committed.
- Spare also fails -> an unaccounted-for systemic factor remains; re-open the
  Pi-side / hub investigation despite the GNSS control having passed.

### If the current board is confirmed faulty, before scrapping it

- D+ (PA12) -> 3V3 already measured at **1.487 kΩ** — pull-up healthy, not the
  problem (see above).
- Inspect / reflow D+ (PA12), D- (PA11), the USB connector, and any 22 Ω series
  resistors on the data lines.
- The reverse-insertion event is a plausible cause of USB-peripheral input damage
  even if this specific board was only briefly exposed. If reflow doesn't help,
  the USB peripheral's OUT/receive side is likely damaged in silicon — the board
  is scrap for USB-CDC use.

## Side notes

- **PA3 scale:** raw ~2276 => ~1.83 V at 3.3 V Vref vs. 2.2 V on the meter
  (~0.83× ratio). Unrelated to this fault; revisit the 20 V->3.3 V divider / VDDA
  reference separately.
- An early SWD halt once showed `pc: 0x1fffb16c` (system/reserved region) — an
  ST-Link reset-on-connect artifact; a proper `halt` gives `pc: 0x080004ba`,
  inside the application.
- Manual `openocd … -c "reset_config … connect_assert_srst"` failed
  (`jtag status contains invalid mode value - communication failure`) — the Blue
  Pill doesn't wire NRST to the SWD header. The PlatformIO extension upload (no
  SRST asserted) works and verifies.

## Reproduce the SWD probe

Target left running (`init`, no `halt`). Addresses are build-specific — re-derive
from `firmware.elf` with `nm` after any rebuild.

```
~/.platformio/packages/tool-openocd/bin/openocd \
  -f interface/stlink.cfg -c "transport select hla_swd" -f target/stm32f1x.cfg \
  -c "init" \
  -c "mdh 0x20000516 3" \   # ReceiveQueue: write, read, length
  -c "mdb 0x2000012c 1" \   # receivePended
  -c "mdb 0x20000455 1" \   # dtrState
  -c "mdh 0x40005c00 8" \   # USB EP0R..EP3R (+ reserved halfwords between)
  -c "mdh 0x40005c44 1" \   # USB_ISTR
  -c "mdh 0x40005c4c 1" \   # USB_DADDR
  -c "exit"
```

`EPnR` bit fields (16-bit): `CTR_RX` [15], `STAT_RX` [13:12]
(00 DISABLED / 01 STALL / 10 NAK / 11 VALID), `EP_TYPE` [10:9]
(00 BULK / 01 CONTROL / 10 ISO / 11 INTERRUPT), `CTR_TX` [7],
`STAT_TX` [5:4], `EA` [3:0].
