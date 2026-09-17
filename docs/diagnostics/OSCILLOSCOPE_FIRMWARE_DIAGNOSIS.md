# Oscilloscope Mode — USB Command-Path Diagnosis (RESOLVED)

*Sessions: 2026-09-07. Bench Pi + Windows dev box. STM32 ADC module ("Blue Pill",
STM32F103C8T6) on `/dev/niva_adc` (`ttyACM0`), behind USB hub `2109:3431` at `1-1`
(enumerates as `1-1.1`). PA3 fed from a bench supply (~2.2 V at the pin).*

> **RESOLVED (2026-09-07).** Adding **`-D USBD_CDC_USE_SINGLE_BUFFER`** to
> `platformio.ini` `build_flags` fixes it: after rebuild+flash, `printf` to the
> STM32 no longer blocks and oscilloscope mode works end-to-end.
>
> **Root cause:** the stm32duino CDC data-OUT endpoint is **double-buffered** on
> STM32F1 by default. On this build it comes up stuck in the HAL's own "Blocking
> State" (`DTOG_RX == SW_BUF`, both 0) — the USB peripheral NAKs every bulk-OUT
> packet even though `STAT_RX = VALID` and the CDC layer has a receive armed. No
> packet is ever accepted, so the ISR path that would unblock it never runs:
> permanent deadlock. `USBD_CDC_USE_SINGLE_BUFFER` switches EP1 OUT to
> single-buffered (plain `STAT_RX = VALID -> ACK`), sidestepping the whole
> `DTOG`/`SW_BUF` handshake.
>
> **What flipped it:** `$OSCCAP` receive *did* work end-to-end earlier — a real
> capture matched a signal generator. The framework files are byte-identical since
> February, yet **rebuilding the historically-good commit `f5d3f2a` now also
> fails.** Same source, different build environment => something in the build
> changed between then and now. The PlatformIO **Core CLI** self-update
> (2026-09-06) is the leading suspect (it can change optimization / flag
> composition / define ordering), but this was not proven — no "before" compiler
> command line was captured. Single-buffer is the robust fix regardless.
>
> **Superseded conclusions.** (1) "Stale firmware" — wrong: verified current build,
> running, endpoint armed. (2) "This board's USB RX hardware is damaged" — wrong: a
> brand-new STM32 behaves identically. (3) "CDC descriptor problem" — wrong: `lsusb
> -v` shows a correct bulk-OUT endpoint. (4) "This Pi's USB stack / OS" — wrong: a
> brand-new Pi 4, first boot, fails identically. (5) "Damaged hub / clone-batch
> clock fault" — not needed. (6) "Never actually tested before" — wrong, per the
> operator: a real waveform capture matched a signal generator.

## Symptom

Oscilloscope mode does not work. `$OSCCAP` — and every other command — sent from
the Pi never reaches the STM32. Telemetry (`$A…`, `$T…`) streams normally.

## How it was narrowed (pre-fix)

*The resolution is at the top of this file; this section is the elimination trail.*

**The STM32 module receives nothing over USB.** Device -> host (IN) transfers work
flawlessly; host -> device bulk DATA OUT transfers never arrive. Enumeration and
the DTR line-state change still succeed because those need only SETUP packets
(ACKed by the USB peripheral in hardware) and IN transfers — no host -> device
DATA OUT packet is involved in either.

Ruled out, each with evidence:

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
  way.** Built commit `f5d3f2a8bdd01451574f5d5052f371881a915113` ("Add osc mode
  test; fix STM32 osc firmware bug") — the revision whose OSC path was
  hardware-validated, predating the DS18B20 code — flashed it, `$OSCCAP` still no
  response. So the break is not in the current source, the DS18B20 additions, or
  the `$VER` changes. **Same source that worked in August, different result now =>
  the build environment changed** (see item 14).
- **Board hardware — a brand-new STM32.** Flashed the current firmware to an
  unused board, verified, power-cycled. Identical result: telemetry streams in
  fine, no command ever gets through, `write()` of one byte times out. Two
  independent boards failing the same way removes board-specific damage (and the
  reverse-polarity event) as the cause.
- **CDC descriptor.** `lsusb -v -d 0483:5740` shows a well-formed data endpoint:
  `bEndpointAddress 0x01` (EP 1 OUT), `bmAttributes 2` (Bulk), `wMaxPacketSize
  0x0040` (64). `cdc_acm` has a valid bulk-OUT pipe. Not a descriptor problem.
- **This Pi's USB stack / OS.** A **brand-new Pi 4, first boot**, gives the exact
  same failure. `uname -a` unchanged and no `linux-image` / `raspberrypi-kernel`
  entries in apt history on the original Pi — not a kernel regression either.

Everything individually swappable was swapped with no effect — which is the
signature of a **firmware build-config** fault, not hardware. The root-cause
section below pins it to the double-buffered CDC OUT endpoint; the hub and a
clone-batch clock fault are no longer needed to explain anything.

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

`EP1R` = `0x3101` before **and** after. Decoded: `CTR_RX` 0, `DTOG_RX` 0, `STAT_RX`
`11` (VALID), `EP_KIND` **1 (double-buffered)**, `DTOG_TX`/`SW_BUF` **0**, `STAT_TX`
`00` (DISABLED), `EA` 1. `CTR_RX` never sets, the queue stays empty — no packet is
ever accepted. See the root-cause section: `DTOG_RX == SW_BUF` on a double-buffered
OUT endpoint is the HAL's "Blocking State" — hardware NAKs regardless of
`STAT_RX = VALID`.

### usbmon — the OUT URB is submitted and NAKed forever

Setup: `stty … 115200 raw -echo -crtscts clocal -hupcl`; `exec 3<>/dev/ttyACM0`
(held open so `-hupcl` keeps DTR up); `sudo cat /sys/kernel/debug/usb/usbmon/1u | grep ':005:'`.

```
# printf 'hello\r' >&3   -- returns immediately
ffffff8083b69a80  2355089570 S Bo:1:005:1 -115 6 = 68656c6c 6f0d      # SUBMIT 6 B "hello\r" -> EP1 OUT
# ...228 s, no completion line...
# exec 3<&-   -- close
ffffff8083b699c0  2583520471 C Ii:1:005:3 -2:16 0                      # int-IN URB unlinked (-ENOENT)
ffffff8083b69a80  2583520765 C Bo:1:005:1 -2 0                         # OUT URB unlinked (-ENOENT), 0 bytes
```

The host submits the bulk-OUT URB, the device NAKs every token for 228 s, and the
URB only ends when `close()` cancels it — **0 bytes transferred**. Combined with
the SWD dump, both ends now agree: the STM32's EP1 OUT never accepts a packet.
The parallel session also confirmed `C Co:1:005:0 0 0` (EP0 control OK) and
continuous status-0 `Bi:1:005:2` reads (bulk IN OK).

### The `$VER` boot banner is never seen — expected, not a clue

`setup()` calls `Serial.print("$VER,…")` immediately after `Serial.begin()`,
before USB has enumerated; `USBSerial::write()` returns 0 while `CDC_connected()`
is false, so the boot banner is dropped every time (documented as best-effort). A
20 ms poller catching the first line as a data frame is expected. Its absence
never implied the wrong firmware — SWD confirmed the running image byte-for-byte.

The reliable path is the **`$VER` query**, which needs host -> device DATA. With
`USBD_CDC_USE_SINGLE_BUFFER` that now works — but check it correctly: have
`cat /dev/ttyACM0` running *first*, then `printf '$VER\n' > /dev/ttyACM0` from
another shell, and look for `$VER,<hash>` (one line in the 50 Hz stream; a
`grep` started after the reply races past it). Send a real `\n` — a bare `\r`
(as in the `printf 'hello\r'` used for usbmon) is never dispatched.

> For a firmware-identity signal that doesn't depend on getting the query timing
> right, emit `$VER` from `loop()` on a DTR rising edge (`Serial.dtr()` 0->1 —
> each time the host opens the port). TX-only, costs one dropped data frame per
> open. Not implemented.

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

Measured **1.487 kΩ** across the D+ pin (PA12) and the 3V3 pin on the **original
board** — right in the 1.5–1.7 kΩ target range for the "2 kΩ across the wrong
10 kΩ R10" bodge. That board's pull-up is intact, so "the bodge joint went open"
is ruled out for it. It still fails — so on this bench a correct ~1.5 kΩ D+
pull-up is not sufficient for host -> device DATA to work.

**Open question:** does the brand-new board have the bodge, or the stock (wrong)
10 kΩ R10? If it was tested un-bodged, its ~10 kΩ pull-up is a marginal-signaling
confound and it should be bodged and retested. Measure PA12 -> 3V3 on it too.

## Root cause: double-buffered CDC OUT endpoint stuck in "Blocking State"

Confirmed by the fix working (see below). Analysis:

Framework: `framework-arduinoststm32` 4.21200.0 (Arduino_Core_STM32 2.12.0).

- `libraries/USBDevice/src/usbd_ep_conf.c` — for the `USB` peripheral (STM32F1,
  not `USB_OTG_FS`), the CDC data-OUT endpoint is declared **`PCD_DBL_BUF`**
  unless `USBD_CDC_USE_SINGLE_BUFFER` is defined. This build does **not** define
  it, so EP1 OUT is double-buffered. Matches the `EP_KIND` bit in the SWD dump.
- `STM32F1xx_HAL_Driver/.../stm32f1xx_ll_usb.c` `USB_ActivateEndpoint`,
  double-buffer OUT branch: sets `STAT_RX = VALID`, `STAT_TX = DISABLED`, and
  **clears both `DTOG_RX` and `DTOG_TX` (SW_BUF) to 0**. That is exactly the state
  read over SWD (`EP1R = 0x3101`).
- Same file, `USB_EPStartXfer`, double-buffer bulk-OUT branch, explicitly names
  the condition:
  ```c
  /* Blocking State */
  if ( ((DTOG_RX!=0) && (DTOG_TX!=0)) || ((DTOG_RX==0) && (DTOG_TX==0)) )
      PCD_FREE_USER_BUFFER(USBx, ep->num, 0U);   /* toggles SW_BUF to unblock */
  ```
  i.e. `DTOG_RX == SW_BUF` => the peripheral NAKs. But that un-block runs **only
  on the "coming from ISR" path** (`ep->xfer_count != 0`). On the very first
  `HAL_PCD_EP_Receive`, `xfer_count == 0`, so it is skipped.

Result: after enumeration the endpoint sits at `DTOG_RX == SW_BUF == 0` = Blocking
State => NAKs the first OUT packet => `CTR_RX` never fires => the ISR that would
call `PCD_FREE_USER_BUFFER` never runs => permanent deadlock. Consistent with
*every* observation: `STAT_RX = VALID` yet perpetual NAK (usbmon), `CTR_RX` never
set and `ReceiveQueue` empty (SWD), `receivePended` stuck at 1, control + bulk-IN
unaffected, and the same failure on any board / Pi / cable because it is a
firmware build-config property.

### Fix — confirmed

`platformio.ini`:
```ini
build_flags =
    -DUSBCON
    -DUSBD_USE_CDC
    -DUSBD_CDC_USE_SINGLE_BUFFER
```
Switches EP1 OUT to `PCD_SNG_BUF` — the plain `STAT_RX = VALID -> ACK` model, no
`DTOG_RX`/`SW_BUF` handshake. **Result on hardware:** `printf` to the STM32 no
longer blocks, and oscilloscope mode works end-to-end (capture matches the input).

Not pursued, since single-buffer works and is a supported setting: patching the
HAL double-buffer init (`USB_ActivateEndpoint` seeding `SW_BUF != DTOG_RX`, or one
`PCD_FREE_USER_BUFFER` after the first `HAL_PCD_EP_Receive`) — that would mean
carrying a framework patch.

## Cross-check: the parallel session's usbmon conclusion

| Its claim | Verdict here |
|---|---|
| Enumeration / descriptors OK | ✅ matches (`lsusb -v`, dmesg) |
| Control transfers (EP0) OK | ✅ matches (`dtrState` toggled; `C Co:…:0 0 0`) |
| STM32 -> Pi bulk IN OK | ✅ matches (telemetry; status-0 `Bi` reads) |
| Pi -> STM32 bulk OUT dead, perpetual NAK, 0 bytes | ✅ matches usbmon + SWD |
| "The fix is on the MCU" | ✅ agree — firmware/framework side |
| "firmware isn't arming a receive buffer on EP 0x01" | ⚠️ **imprecise.** SWD shows `STAT_RX = VALID`, `receivePended = 1`, PMA addresses set — the receive *is* armed. It NAKs because it's a **double-buffered** endpoint sitting at `DTOG_RX == SW_BUF` (HAL "Blocking State"), not because no buffer was armed. |
| "Work the CDC_Receive_FS / USBD_CDC_ReceivePacket / IRQ-priority checklist" | ⚠️ partly misdirected. `CDC_Receive_FS` is CubeMX naming — not in this stm32duino stack. `USBD_CDC_ReceivePacket` already ran (`receivePended = 1`). USB IRQ priority is fine — EP0 + bulk-IN interrupts are serviced. The productive lever is the double-buffer config, i.e. `USBD_CDC_USE_SINGLE_BUFFER`. |

## History / timeline (from the bench operator)

1. OSC mode **worked end-to-end earlier** — commit `f5d3f2a` "Add osc mode test;
   fix STM32 osc firmware bug"; a real capture matched a signal-generator waveform
   exactly (operator-confirmed). Not exercised again until after DS18B20 support.
2. Unknown whether OSC was retested between its initial implementation and the
   DS18B20 flash.
3. PlatformIO auto-updated (Core CLI) before the DS18B20 build. Flashing threw new
   errors, worked around by reconnecting the ST-Link; upload reported success;
   DS18B20 then confirmed working from the Pi (2 sensors enumerated + data).
   `git show f19fd9e:…/main.cpp` — the DS18B20 commit — has
   `poll_incoming_commands()` intact as the first statement of `loop()`, so that
   image necessarily contains working `$OSCCAP` handling.
4. Shortly after, OSC retested — broken.
5. `$VER` diagnostic added and flashed (reported success). Commands still dead;
   `$VER` boot banner not seen (expected — see the note above).
6. **Reverse-polarity insertion event** at some point: an STM32 plugged in wrong,
   frying its 3.3 V regulator; the Pi's USB power briefly dropped (load switch
   tripped). The board was replaced. "Everything seemed to work" afterward — but
   only power and *read* paths (telemetry, GNSS) were exercised. Host -> device
   DATA OUT was never tested post-incident. Where this falls relative to items 3–5
   is not certain.
7. The first replacement STM32 does **not** enumerate on a Windows PC at all —
   unresolved; possibly a Windows-side cable/port issue. Re-test with the new
   board (item 8) before reading anything into it.
8. **Brand-new STM32**, current firmware, verified flash, power-cycled: identical
   failure — telemetry in fine, no command through, `write()` of one byte times
   out. Board-specific damage is out.
9. `lsusb -v -d 0483:5740`: bulk-OUT endpoint `0x01`, Bulk, `wMaxPacketSize 64` —
   descriptor is well-formed. Descriptor/endpoint theory is out.
10. **Brand-new Pi 4, first boot**, wired for this test: identical failure. This
    Pi's USB stack / OS state is out. `uname -a` and apt history on the original
    Pi show no kernel bump.
11. **usbmon**: the host submits the bulk-OUT URB (`S Bo:1:005:1 … 6 = 68656c6c
    6f0d`), gets no completion for 228 s, then `-ENOENT` on `close()`. Perpetual
    NAK, 0 bytes. Pi side fully cleared; fault is the STM32 EP1-OUT.
12. **Root cause identified** in framework source: the CDC OUT endpoint is
    double-buffered by default on STM32F1 and this build leaves it at
    `DTOG_RX == SW_BUF` (HAL "Blocking State") -> NAKs forever.
13. **Fixed.** `-DUSBD_CDC_USE_SINGLE_BUFFER` added to `build_flags`, rebuilt,
    flashed: `printf` to the STM32 no longer blocks and oscilloscope mode works
    end-to-end. (`$VER` query timing still to be re-checked — see the `$VER`
    note; boot banner remains best-effort/dropped.)
14. **Unresolved:** why double buffering worked in August and not now. Framework
    files are byte-identical since February, but rebuilding `f5d3f2a` today
    reproduces the failure — so the *build environment* changed. PlatformIO Core
    self-updated 2026-09-06; that is the suspect but was not proven (no
    before/after compiler command line captured).

## Firmware & tooling changes made during diagnosis

- **`-DUSBD_CDC_USE_SINGLE_BUFFER` in `platformio.ini` `build_flags` — the fix.**
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

## Follow-ups (optional)

- **Verify the `$VER` query** with the timing from the `$VER` note (cat first,
  then `printf '$VER\n'`, real newline). Not blocking — OSC mode is the thing
  that matters and it works.
- **`$VER` on DTR edge** — if a reliable firmware-ID line is wanted without the
  query-timing fuss, emit `$VER` from `loop()` on `Serial.dtr()` 0->1. ~6 lines,
  TX-only.
- **Pin the toolchain.** Since a build-environment change is what broke this,
  pin the platform and the packages in `platformio.ini`
  (`platform = ststm32@19.6.0`, `platform_packages = framework-arduinoststm32 @ 4.21200.0`)
  so a future auto-update can't silently regress it again.
- **Report upstream** (optional): stm32duino F1 double-buffered CDC OUT deadlocks
  on first packet with this HAL revision under the build conditions here;
  `USBD_CDC_USE_SINGLE_BUFFER` works around it.
- **Root cause of the *trigger* is still open** (item 14) — only pursue if it
  recurs.

### Done / ruled out during the hunt

- `lsusb -v` — bulk-OUT endpoint `0x01`, 64 B, well-formed.
- Two STM32 boards (one brand new), two Pis (one first-boot), two cables — same
  failure before the fix. No Pi kernel bump.
- `usbmon` — OUT URB submitted, NAKed forever, 0 bytes.
- SWD — `EP1R` double-buffered, `DTOG_RX == SW_BUF == 0`, `CTR_RX` never fires.
- D+ (PA12) -> 3V3 = **1.487 kΩ** on the original board — pull-up healthy.

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
