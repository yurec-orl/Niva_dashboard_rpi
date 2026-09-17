# BNO085 Connectivity Research

Hardware: BNO085 breakout wired over I2C, detected at address `0x4B` via `i2cdetect`.
No HINT/RST pins wired to Pi GPIO (SDA/SCL/VCC/GND only).

## Rust crate survey

The BNO08x family (BNO080/085/086) all run CEVA's SH-2 firmware and share one datasheet/protocol,
so BNO080-targeted crates work for the BNO085.

| Crate | Interface | embedded-hal | Status |
|---|---|---|---|
| [`bno080`](https://crates.io/crates/bno080) (tstellanova) | I2C + SPI | 0.2.3 | On crates.io, unmaintained since Apr 2020. No calibration/tare, no CI. |
| [`FrozenDroid/bno08x`](https://github.com/FrozenDroid/bno08x) | I2C + SPI | 1.0.0 | Fork of the above ported to eh 1.0. Git-only (not published), low activity, same gaps. |
| [`bno08x-rvc`](https://lib.rs/crates/bno08x-rvc) | UART only (RVC mode) | n/a | Simplified fused output (heading/roll/pitch) only; needs a mode-pin change on the module. Dormant since 2021. |
| [`bno08x-rs`](https://docs.rs/bno08x-rs) (EdgeFirstAI) | SPI only, via Linux `spidev`+`gpiod` directly (no embedded-hal) | n/a | Actively maintained (v2.0, Dec 2025), real CI. No I2C support; bypasses embedded-hal entirely (different integration shape than `rppal`-based hardware layer). |

Conclusion: no actively-maintained I2C option exists. `rppal` 0.22.1 supports both embedded-hal 0.2.7
and 1.0 for I2C (`hal`/`hal-unproven` features), so either `bno080` or the `FrozenDroid` fork would
plug into `rppal::i2c::I2c` directly if going the crate-dependency route — but expect to read/patch
the SHTP/report-parsing internals directly, not treat either as a black box.

Alternative: hand-roll a minimal SHTP-over-I2C reader against the protocol details below, decoding
only the reports actually needed (e.g. just Rotation Vector). Written directly against
`rppal::i2c::I2c` rather than through embedded-hal generics, since this runs on a Linux host, not
a `no_std` MCU target — sidesteps the embedded-hal version mismatch between the two crates above.

## Protocol layering

BNO08x sensors speak two stacked protocols:

1. **SHTP** (Sensor Hub Transport Protocol) — byte-framing/transport layer, protocol-agnostic
   (I2C/SPI/UART).
2. **SH-2** — the actual sensor report definitions (rotation vector, accel, etc.), carried as
   cargo inside SHTP.

Sources:
- [Sensor Hub Transport Protocol, CEVA doc 1000-3535 rev 1.10](https://docs.sparkfun.com/SparkFun_VR_IMU_Breakout_BNO086_QWIIC/assets/component_documentation/Sensor-Hub-Transport-Protocol.pdf)
- [SH-2 Reference Manual v1.2, Hillcrest/CEVA doc 1000-3625](https://cdn.sparkfun.com/assets/4/d/9/3/8/SH-2-Reference-Manual-v1.2.pdf)

### SHTP framing (transport layer)

Every transfer starts with a 4-byte header (SHTP spec §2.2.1):

| Byte | Field |
|---|---|
| 0 | Length LSB |
| 1 | Length MSB (bit 15 = continuation flag) |
| 2 | Channel |
| 3 | Sequence number (per-channel, per-direction) |

Fixed channels: 0 = command, 2 = control (feature enable/config), 3 = reports (normal sensor
data), 4 = wake reports, 5 = gyro-integrated RV.

I2C mechanics (§3.2, §3.4-3.5):
- No register address to write first — an I2C read is just "read N bytes from the device
  address." Repeated-start is not supported; every transfer ends with a STOP.
- The host doesn't know the payload length in advance: read the 4-byte header first, check the
  length field, then read the remainder (or read a generously-sized buffer in one shot and trust
  only the first `length` bytes).
- Reads are meant to happen only after the hub asserts HINT (interrupt GPIO, active low,
  separate from SDA/SCL, deasserted once the read begins). Without HINT wired, polling is safe
  per spec — reading with no cargo available just returns a zero-length "null" header — just
  higher latency than interrupt-driven reads.
- Writes are a plain I2C write of `[header][cargo bytes]`.

Startup handshake: immediately after power-on/reset, the hub sends its full advertisement
unsolicited on channel 0 and must not send anything else until that's been read. A minimal
client still has to read and discard (or parse) this before other traffic will flow correctly —
the step most likely to be silently botched in a hand-rolled implementation.

### SH-2 reports (report layer)

Enabling a sensor: send a **Set Feature Command** (report ID `0xFD`) on the control channel (2),
17-byte payload — report ID to enable (byte 1), feature flags (byte 2), change-sensitivity
(bytes 3-4), **report interval in microseconds, little-endian 32-bit** (bytes 5-8, e.g. `10000`
= 100 Hz), batch interval (9-12), sensor-specific config word (13-16).

Reading **Rotation Vector** (report ID `0x05`), delivered on the reports channel (3), 14-byte
payload:

```
0: Report ID (0x05)   1: seq num   2: status   3: delay
4-5:   quaternion i    (int16, Q14)
6-7:   quaternion j    (int16, Q14)
8-9:   quaternion k    (int16, Q14)
10-11: quaternion real (int16, Q14)
12-13: heading accuracy (int16, Q12, radians)
```

"Q14" means divide the raw int16 by `2^14` to get the float value.

Report ID `0x05` (full 9-axis rotation vector, needs magnetometer, gives absolute heading but
can be perturbed by magnetic interference) vs `0x08` (Game Rotation Vector, 6-axis, no mag,
drift-free relative orientation, no absolute heading) — pick deliberately depending on whether
absolute heading is needed.

### Magnetic compass azimuth

Needs a report that incorporates the magnetometer — not Game Rotation Vector (`0x08`), which
deliberately excludes it for drift-free short-term motion:

- **Rotation Vector (`0x05`)** — 9-axis (accel+gyro+mag), absolute heading referenced to magnetic
  north, includes a heading accuracy estimate. Standard choice.
- **Geomagnetic Rotation Vector (`0x09`)** — 6-axis (accel+mag, no gyro), also absolute heading
  with accuracy estimate; lower power and no gyro drift, but noisier short-term than `0x05`.

Both report bytes are a quaternion (Q14, same 14-byte layout as `0x05` above), not a direct
azimuth angle — extract yaw from the quaternion (`i`, `j`, `k`, `real` as decoded per the layout
above):

```
yaw = atan2(2*(real*k + i*j), 1 - 2*(j² + k²))
```

`yaw` is the compass azimuth in radians, relative to **magnetic** north (not true north) and
relative to the chip's own axis convention (right-handed, Z-up by default). If the board isn't
mounted flat with its silkscreen-defined forward axis aligned to the vehicle's forward axis, apply
a fixed offset correction or use the SH-2 "Set Reorientation" command to compensate.

**Design note**: the existing GNSS `UNIHEADING` parsing (dual-antenna GNSS) gives **true**
heading, while this gives **magnetic** heading — they won't agree unless magnetic declination for
the install location is applied. Decide explicitly how the two are meant to relate (e.g. shown as
separate sources, or reconciled via a declination constant) before feeding both into one compass
display.

### Report selection: Rotation Vector for heading and inclination

Decision: drive both the magnetic-heading fallback and the inclination (pitch/roll) indicator from
a single Rotation Vector (`0x05`) feature stream, rather than also running Game Rotation Vector
(`0x08`) in parallel.

Rationale:

- The quaternion already encodes full 3D orientation — pitch and roll are extractable from the same
  14-byte report used for yaw/heading, via the standard quaternion→Euler conversion (ZYX order,
  consistent with the yaw formula above):

  ```
  roll  = atan2(2*(real*i + j*k), 1 - 2*(i² + j²))
  pitch = asin(clamp(2*(real*j - k*i), -1, 1))   // clamp avoids NaN at ±90° (gimbal lock)
  yaw   = atan2(2*(real*k + i*j), 1 - 2*(j² + k²))   // as above
  ```

  Which physical axis is "roll" vs "pitch" depends on how the board is mounted relative to the
  vehicle (same caveat as the yaw/heading axis-alignment note above) — resolve via the same fixed
  offset or Set Reorientation command once mounting is finalized.

- In standard AHRS fusion design, the accelerometer alone resolves tilt (pitch/roll) against
  gravity; the magnetometer only supplies the remaining degree of freedom (yaw). So magnetic
  interference should degrade heading accuracy, not inclination accuracy — meaning there's no
  fusion-design reason to prefer Game Rotation Vector's pitch/roll over Rotation Vector's for the
  inclination indicator. **Caveat**: not independently verified against SH-2's actual (closed-box)
  internals — if inclination readings are ever observed to degrade specifically when the compass
  accuracy status is low, that would be a signal this assumption doesn't hold for this firmware.

- One report stream means one Set Feature Command, one parse path, and one staleness watchdog
  (per the "Silent feature staleness" lesson below) instead of two — matches the project's minimal
  hand-rolled-SHTP scope. Running Game RV in parallel would isolate inclination from any
  hypothetical magnetometer-related quaternion corruption, at the cost of doubling I2C report
  traffic and feature-management complexity on a bus already shown to have transient reliability
  issues (see "I2C clock speed" / "Error handling & recovery strategy" below) — not a free trade,
  so default to the single-stream approach unless the caveat above is observed in practice.

- Heading fusion with GPS: GNSS `UNIHEADING` (true heading) is authoritative when its solution
  status indicates a valid fix; BNO085's yaw is magnetic, so correct it to true heading by adding a
  fixed local declination constant (not the reverse) before comparing/switching. Switch on GNSS
  solution-status with hysteresis (require N consecutive good/bad fixes), not on speed/movement,
  since dual-antenna heading is valid even stationary. Expect a small residual disagreement between
  the two sources even after declination correction (compass drift, coarse declination constant) —
  worth a short blend/slew-limit at the switchover rather than an instant cut, to avoid a visible
  heading jump on the display.

### Minimal implementation scope estimate

An SHTP read/write pair over `rppal::i2c::I2c` (header parse + continuation handling), draining
the startup advertisement, sending one Set Feature Command, and matching on report ID 0x05 in
the read loop to decode 4 fixed-point int16s: roughly 150-300 lines.

## Python/CircuitPython verification path (before committing to a Rust implementation)

Adafruit maintains an actively-updated CircuitPython/Blinka driver
([`adafruit-circuitpython-bno08x`](https://github.com/adafruit/Adafruit_CircuitPython_BNO08x))
usable on Raspberry Pi via Blinka, useful for confirming the physical sensor and wiring work
before writing any Rust code.

- [Tutorial](https://learn.adafruit.com/adafruit-9-dof-orientation-imu-fusion-breakout-bno085/python-circuitpython)
  says `sudo pip3 install adafruit-circuitpython-bno08x` — fails on Debian 12 (PEP 668
  "externally managed environment"). Use a venv instead; no `sudo` needed since `/dev/i2c-1` is
  group `i2c` and this user is already in that group.
- `BNO08X_I2C` constructor: `__init__(self, i2c_bus, reset=None, address=_BNO08X_DEFAULT_ADDRESS,
  debug=False)` — default address is `0x4A`, but this board is at `0x4B`, so the test script must
  pass `address=0x4B` explicitly.
- `reset` defaults to `None` — no HINT/RST wiring required for basic operation, matching this
  board's actual wiring (Blinka polls rather than using a hardware interrupt).

### Hardware verification result

Ran `scripts/test_bno085.py` against the actual board: full SHTP handshake, feature enable, and
continuous Rotation Vector streaming all worked over the existing wiring (I2C only, no HINT/RST).
Quaternion output changed correctly in response to physically rotating the board. Confirms the
board, wiring, and I2C address (`0x4B`) are all good — the sensor itself is not in question for
any future implementation issues.

- **`KeyError`/`IndexError` crashes in the Adafruit library** from unrecognized report IDs
  (e.g. `0x7B`) and out-of-range channel numbers on parsed packets — see "Implementation patterns"
  below for the root cause and how to avoid repeating it.
- **Transient `OSError: EIO`** from the kernel I2C layer. Originally suspected as a momentary loose
  connection (jumper/breadboard contact) since it correlated with physically moving the board, and
  the device re-enumerated cleanly in `i2cdetect` immediately after with no kernel-level bus-fault
  messages in `dmesg`/`journalctl`. Clock speed clearly affects its frequency (much rarer at
  400 kHz than at 100 kHz) but doesn't eliminate it — treat both clock speed and physical wiring
  as contributing factors, and keep retry/backoff around individual I2C transactions regardless.
- **`RuntimeError('Unprocessable Batch bytes', ...)`** from the same root cause as the
  `KeyError`/`IndexError` cases: a legitimate SHTP packet the library's generic report-batch parser
  doesn't expect — in this case a 1-byte payload on the EXE channel (channel 1), which is the
  standard "reset complete" notification. Same fix class as the others (catch and skip), but with
  a sharper edge: see below.

### I2C clock speed

The Adafruit tutorial states the BNO085 "performs optimally" on the Pi at 400 kHz. This system's
`i2c-1` bus was running at the Pi's default 100 kHz (`/boot/firmware/config.txt` had
`dtparam=i2c_arm=on` but no baudrate override; confirmed via
`/sys/firmware/devicetree/base/soc/i2c@7e804000/clock-frequency` = `100000`). Note that Blinka
cannot change this at runtime — `busio.I2C(..., frequency=...)` is silently ignored on Linux
(`RuntimeWarning: I2C frequency is not settable in python, ignoring!`); the clock speed is fixed by
the device tree at boot.

Tried: added `dtparam=i2c_arm_baudrate=400000` to `/boot/firmware/config.txt`, rebooted. A couple
of early test runs at 400 kHz were stable, which initially looked like a fix. **That turned out to
be coincidental** — subsequent runs at the same 400 kHz setting failed just as badly as at
100 kHz, often running only a few seconds before errors and corrupt packets resumed. Retracting
the earlier conclusion that 400 kHz is meaningfully more reliable than 100 kHz: across the range
tested, clock speed alone does not reliably explain or fix the instability. Any implementation
still needs to tolerate transport/parsing errors as normal, expected operation regardless of bus
speed.

Note `i2c-1` is shared with the UPS HAT (INA219 at `0x43`, onboard MCU at `0x2D` — see
`ups_i2c_provider.rs`/`ups_monitor.rs`); no issues observed with the UPS HAT at 400 kHz in this
testing.

Tried: `dtparam=i2c_arm_baudrate=10000` (10 kHz) — the opposite direction from the Adafruit
tutorial's advice, but widens the BSC controller's clock-stretch timeout window instead of
narrowing it (see the 400 kHz result above, which narrows it and turned out not to help).
**Result**: continuous run of over an hour with no sensor resets and heading stable throughout —
the most reliable result observed across all clock speeds tested so far. Single long run, not yet
repeated.

### Error handling & recovery strategy in the test script

The three failure modes above needed two different recovery strategies, and conflating them caused
a bug worth recording:

- **Malformed/unrecognized packets** (`KeyError`/`IndexError`/`RuntimeError` from parsing) — caught
  and skipped; the next poll iteration reads a fresh packet, so no special recovery is needed
  beyond not crashing.
- **Transient I2C bus errors** (`OSError`/`EIO`) — caught, logged, short backoff, retry; escalated
  to a warning (not a crash) if it stops looking transient (20+ in a row).
- **Silent feature staleness** — the one that actually bit us: the EXE-channel "reset complete"
  notification means the sensor *actually reset*, which silently clears its feature-enable state
  on the device side. The Python exception gets caught and skipped same as any other malformed
  packet, execution continues without crashing, but no *new* Rotation Vector reports ever arrive
  again — `bno.quaternion` keeps returning the same cached last-known value forever, since nothing
  raises an error to signal it. Only a full script restart (which re-runs the feature-enable step)
  brought readings back to life.

  First fix attempt — re-enable the feature after N consecutive parse errors — was wrong: it
  requires a burst of errors to trigger, but the observed failure was a *single* `RuntimeError`
  followed by silence (no more exceptions, just frozen output), so the counter never reached
  threshold. The working fix instead detects staleness directly: track the last quaternion reading,
  and if it repeats bit-for-bit for ~15 consecutive polls, re-enable the feature. This works because
  live sensor output always has a small amount of noise/jitter even with the board sitting
  perfectly still (visible throughout every capture in this doc) — an exact repeat is a reliable
  signal that no new report arrived, regardless of whether any exception fired along the way.

  **Lesson for the Rust implementation**: don't assume "no error raised" means "data is fresh."
  A sensor-side reset can silently stop new reports without the transport layer ever surfacing a
  failure. Track data staleness (a timestamp or repeat-count on the decoded value) independently of
  error handling, and re-issue the Set Feature Command whenever a report is overdue — the same
  reasoning applies to a hand-rolled SHTP client, not just this Python library.

## Implementation patterns validated against real hardware

Extracted from reading `adafruit_bno08x/i2c.py` and `adafruit_bno08x/__init__.py` (installed at
`.venv-bno085-test/lib/python3.11/site-packages/adafruit_bno08x/`) after confirming they work
against this board — these are patterns proven against the real sensor, not just the datasheet.

**I2C read strategy**: two separate, fresh I2C read transactions per packet — first a 4-byte
header-only read to learn the length, then a *second* read of `length` bytes starting over from
byte 0 (header + cargo together, not just the remainder). Since SHTP over I2C forbids repeated
starts, the sensor just replays the same pending cargo from the start on each new transaction
until one read is long enough to fully drain it. No need to implement the spec's continuation-bit
logic for reports this small — size the buffer generously (they use 512 bytes, growing on demand)
and always re-read from the top.

**Startup handshake is simpler in practice than the spec implies**: rather than special-casing the
unsolicited channel-0 advertisement, a generic dispatch loop that reads whatever packet comes next
and silently drops it if it's on channel 0 (command) or 1 (executable) and doesn't match what's
being waited for handles it automatically — the advertisement resolves itself as the first thing
read and discarded, no bespoke first-boot logic needed. Proven init sequence: reset → send Product
ID Request (`0xF9`) on the control channel → loop reading/discarding packets until a Product ID
Response (`0xF8`) shows up.

**Set Feature Command byte layout, confirmed correct by testing**: `bytearray(17)`, byte 0 =
`0xFD`, byte 1 = feature/report ID, `report_interval` packed little-endian `u32` at offset 5,
sensor-specific config `u32` at offset 13 — matches the datasheet-derived layout above, now
cross-validated against a real device that actually started streaming.

**Report table cross-check** (`_AVAIL_SENSOR_REPORTS`, format `(Q-point scalar, component count,
byte length)`): confirms Rotation Vector = Q14, 4 components, 14-byte report, exactly matching
SH-2 manual Figure 82. Also gives Q-points/lengths for other reports if needed later without
re-deriving from the PDF: Accelerometer/Gravity/Linear Acceleration = Q8, 3 components, 10 bytes;
Gyroscope = Q9, 3 components, 10 bytes; Magnetometer = Q4, 3 components, 10 bytes; Geomagnetic
Rotation Vector = Q12, 4 components, 14 bytes; Game Rotation Vector = Q14, 4 components, 12 bytes
(no accuracy field, since it has no absolute heading to be inaccurate about).

**Root cause of both crashes hit during testing**: `self._sequence_number` is a fixed 6-element
list, one slot per channel 0-5. Any packet whose header decodes to a channel number ≥6 — garbage
from a noisy read, or parser misalignment after an unrecognized report ID threw off byte offsets —
causes an out-of-bounds list write. Lesson for the Rust implementation: validate the channel number
against the known range *before* using it to index anything, and treat an unrecognized report ID
as "skip these bytes" rather than a hard error. Both library bugs stemmed from trusting field
values without bounds-checking them, not from any I2C-level malfunction.

## Calibration

General SH-2 firmware behavior, from the SH-2 reference manual — **not yet exercised against this
hardware** (unlike the rest of this doc, which is backed by `scripts/test_bno085.py` runs against
the real board). Recorded here as a reference for whenever calibration behavior needs to be relied
on or debugged.

**Automatic, always-on**: accel/gyro/mag are continuously calibrated in the background by the
onboard Cortex-M0+ as the sensor experiences normal motion. There's no explicit "calibration mode"
to enter.

- **Magnetometer** converges faster with varied orientation (rotation through multiple axes). A
  yaw-only rotation (e.g. driving in a circle) only calibrates the horizontal-plane response, not
  the full 3D hard/soft-iron model — acceptable here since a dashboard-mounted sensor never sees
  other tilt angles anyway, and this is exactly the standard automotive "compass swing" procedure
  (calibrating against the vehicle's own magnetic distortion specifically at driving attitude).
- **Gyroscope** bias nulls out during detected stillness (parked/idling), not rotation — a 360°
  turn does nothing for it.
- **Accelerometer** wants varied tilt angles (6-position: flat, each side, inverted) to fully
  converge — essentially unachievable with a fixed dashboard mount. Expect it to stay at whatever
  calibration it had from the factory/last full calibration; no realistic in-vehicle fix if it
  degrades.

**Per-report accuracy status**: every SH-2 sensor report carries a 2-bit status field (0=unreliable,
1=low, 2=medium, 3=high accuracy). Check this before trusting a reading, same idea as checking a GPS
fix quality flag — most useful right after boot, before dynamic calibration has reconverged.

**Externally triggerable** (still driving the same internal algorithm — not a manual override of
calibration coefficients):

- **ME Calibration Command** — enable/disable dynamic calibration per sensor (accel/gyro/mag
  individually).
- **Save DCD (Dynamic Calibration Data)** — persists converged calibration to the chip's
  non-volatile storage, surviving power cycles. Worth issuing once accuracy status reaches "high" so
  the vehicle doesn't start every trip needing to reconverge from scratch.
- **Tare / Set Reorientation** — not sensor calibration; sets a reference orientation to correct for
  mounting-axis mismatch (board silkscreen forward ≠ vehicle forward/level). This is the mechanism
  to use for the axis-alignment correction flagged above (both for yaw/heading and for roll/pitch),
  rather than a manual offset computed in application code.

**Not supported**: no way to inject externally-computed hard-iron/soft-iron magnetometer
coefficients directly — the fusion algorithm is closed-box firmware, unlike a bare magnetometer chip
where the host computes and applies its own calibration matrix.

**Open questions for implementation**: when to issue Save DCD (once at first high-confidence
calibration? on every transition to high accuracy?), and how to detect "calibration degraded enough
to prompt the driver to do a compass swing" from the dashboard UI. Not yet designed.

## Field testing against the Rust implementation (bench, 2026-08-07)

Testing `Bno085DataProvider`/`Bno085` (bno085_data_provider.rs, bno085_protocol.rs) on the actual
board, on a table (not installed in the vehicle). Two symptom clusters observed: intermittent I2C
connection drops, and poor/degrading heading accuracy. Investigated together since they turned out
to be entangled. Original hypothesis going in ("the sensor unit is faulty or unreliable by design")
does not hold up cleanly against the evidence below — several more likely, and fixable, causes exist
— but isn't fully ruled out either.

### Confirmed from logs

- **Connection drops were initially thought to correlate with I2C clock speed** (see "I2C clock
  speed" above) — retracted: later testing at 400 kHz failed just as badly as at 100 kHz, so clock
  speed is not treated as a confirmed factor.
- **The sensor's internal fusion state does not survive a connection drop.** In every captured
  reconnect, the first fresh report after `connect_and_init()` shows `accuracy=Unreliable` and
  `heading_accuracy` at exactly ±180.0° (Q12's max-uncertainty value) — the firmware's own
  freshly-booted/unconverged signature — even when the drop happened with the board sitting
  completely still and no wires touched. If this were purely a Pi-side I2C controller glitch with
  the sensor's own MCU continuing to run uninterrupted, the fusion state should survive and resume
  near its pre-drop confidence, not reset to baseline. This is evidence the sensor's own MCU is
  actually resetting, not just that the bus transaction is failing.
- **Garbage `heading_accuracy` values appear around drops**: e.g. `±-449.5°`, `±-0.0°` — outside the
  field's valid range (heading uncertainty can't be negative or exceed ±180°) and a magnitude
  consistent with a torn/misaligned I2C read landing raw noise in bytes 12-13 of the Rotation Vector
  report, not a value the firmware would legitimately send. Points to a signal-integrity/bus event
  coinciding with the drop, not (only) a clean higher-level reset notification.
- **Heading accuracy also degrades during uninterrupted, no-reset operation.** Continuous stationary
  run, no reconnects logged: `heading_accuracy` went ±30.0° → ±35.0° → ±40.0° over 42 seconds
  (Medium status throughout), while the heading *value* itself stayed essentially flat (56.0° →
  55.8° → 55.7°). So the confidence estimate can erode even when nothing resets and the output value
  hasn't (yet) visibly degraded — the connection-drop/reset issue above does not explain this by
  itself.
- **Not correlated with physically moving/flexing the signal wires** in either failure mode — argues
  against a loose SDA/SCL mechanical connection as the (sole) cause of either symptom.

### Unconfirmed hypotheses (plausible, not yet tested)

- **Power delivery marginality** (insufficient decoupling at the breakout, thin/long jumper wire off
  the Pi's 3.3V GPIO pin, or a poor ground return) causing brief brownout resets of the sensor's own
  MCU. Would explain both the reset-signature-after-drop and the drop's lack of correlation with
  wire flexing. Not weakened by "everything else on the Pi (STM32, display) works fine" — those are
  digital loads on different rails/interfaces with different margins, not evidence the BNO085's
  supply is adequate.
- **Ambient magnetic interference from the project's own bench hardware** (Pi, STM32 over USB, GNSS
  receiver, display, any nearby PSUs) — not ruled out just because testing is on a table rather than
  in the vehicle; a bench full of running electronics isn't magnetically quiet either.
- **Thermal drift** in the magnetometer/hard-iron offset as the board or nearby components self-heat
  after power-on — could produce a slow accuracy-metric decay over the first minutes of operation
  independent of any interference or fault.
- **SH-2's confidence/covariance estimator may need periodic rotational stimulus** to hold its
  accuracy rating, and could decay on its own during a long stationary hold even with nothing
  actually wrong — speculative, no firmware documentation found confirming this behavior.
- Whether the reported heading *value* (not just the accuracy/confidence number) ever meaningfully
  drifts during an extended unconverged/low-accuracy period, or stays practically usable regardless
  of what the confidence metric says.

### Things to check next

1. Measure VCC at the BNO085's power pins with a multimeter/scope, ideally under bus activity; add
   local decoupling if the breakout's own isn't adequate; try powering from a separate bench supply
   instead of the Pi's 3.3V GPIO pin, and see if drop frequency changes.
2. Check `dmesg`/`journalctl` at the timestamp of a connection drop for kernel-level I2C bus-fault
   messages — presence/absence helps distinguish a kernel/controller-level fault from the sensor
   itself going silent.
3. Extended stationary run (10+ minutes), logging both heading value and accuracy/status throughout:
   does `heading_accuracy` plateau or climb indefinitely, and does the heading value itself ever
   start drifting once accuracy bottoms out, or stay put regardless.
4. Repeat the same extended stationary run in a maximally magnetically clean spot — a meter or more
   from the Pi, other project hardware, and any phone/laptop — and compare degradation rate.
5. Repeat after 10-15 minutes of powered warm-up before starting the clock, to test the thermal-drift
   hypothesis.
6. If a second BNO085 unit/breakout becomes available, run the identical stationary test side by
   side as a swap-test — the most direct way to isolate a genuinely marginal individual unit from an
   environmental or setup cause.
7. Not yet tried: a single retry of the failed I2C transaction before tearing down and reconnecting
   (`run_loop`'s `Err(e) => { ...; sensor = None; }` in bno085_data_provider.rs currently reconnects
   unconditionally on any I2C error) — would help distinguish a one-off transient bus error from an
   actual sensor-side reset, and avoid a full re-init cycle for the former.

### Reconnect signature refinement: clean reconnects don't reset the chip, crash-triggered ones do

A follow-up test ran four separate short (few-second) program launches spread over ~30 minutes,
each doing its own clean `open()` → `init()` → `enable_feature()` sequence. Across all three
resulting reconnects, `heading` stayed within about 1° (23.3° → 23.6° → 23.7° → 24.0°) and
`heading_accuracy` climbed *smoothly* across the reconnects rather than resetting
(±58.9° → ±112.8° → ±144.0° → ±176.4°, `Low` degrading to `Unreliable`) — i.e. a **graceful,
program-initiated reconnect does not perturb the chip's internal fusion state**. A later longer run
then hit an actual I2C error mid-stream and forced an unplanned reconnect; that one showed the full
reset signature again (heading jumped to 276.0°, unrelated to the ~24° it had been holding;
`heading_accuracy` snapped to exactly ±180.0°).

This refines (and partly corrects) the earlier reading of "reconnect ⇒ reset" from the first field
test session: it's not reconnecting itself that resets the chip, only whatever specifically happens
during the failure that forces an *unplanned* reconnect in an active-polling session. That points
more specifically at something occurring during sustained/active I2C communication as the crash
trigger — but is equally consistent with a Pi-side controller issue (e.g. clock-stretch mishandling
under sustained transaction volume corrupting a transfer badly enough to wedge/reset the sensor's I2C
peripheral) or a sensor-firmware-side issue that only surfaces after enough traffic — the two are
indistinguishable from application-level logs alone. Diagnostics that would actually separate them:
capturing the failing transaction with an I2C logic analyzer/sniffer, or reproducing the same
sustained-polling test from a different I2C master (rules the Pi's controller in or out directly).

### Extended-duration runs (2026-08-08): no crashes observed

Two further runs, both with the dashboard's `test=bno085` mode continuously polling orientation +
acceleration (same 50 Hz Rotation Vector + Accelerometer feature config as all prior tests), while a
second dashboard process ran concurrently in normal mode polling the UPS HAT (INA219 `0x43`, onboard
MCU `0x2D`) on the same shared `i2c-1` bus — active bus contention, not an idle bus:

- 30-minute run: clean throughout, no I2C errors, no reconnects, no reset-signature jumps.
- 1.5-hour run: same — clean throughout.

This complicates the "sustained active polling reliably triggers the crash-resets" reading from the
reconnect-signature test above: 1.5 hours of continuous 50 Hz polling with a second process
contending for the same bus produced zero crashes, while earlier sessions saw multiple crash-resets
within single-digit minutes of active polling. Two ways to reconcile, neither confirmed:

- The crash trigger may be a genuinely rare/probabilistic event (an occasional bus glitch, power
  transient, or similar) rather than something that reliably fires once enough I2C volume
  accumulates — a handful of short earlier runs catching several crashes and two much longer runs
  catching zero would be unremarkable for a low-probability, not volume-proportional, event.
- Something in the physical setup may differ between the crash-prone earlier tests and these clean
  runs (wiring reseated, different power source/routing, ambient conditions) that hasn't been pinned
  down yet.

Net effect: doesn't support "sustained active I2C traffic reliably triggers resets" as a simple
volume-based mechanism, and doesn't rule out the sensor/setup being marginal either — the crash-reset
cause still looks intermittent rather than cleanly duration- or load-triggered. More long runs, and
confirming whether anything physically changed between sessions, would help narrow this further.

### Clock speed identified as the likely differentiator (2026-08-08)

The one thing that changed between the crash-prone tests and the two crash-free extended runs above:
`/boot/firmware/config.txt` currently has `dtparam=i2c_arm_baudrate=10000` — 10 kHz, well below the
Pi's 100 kHz default and far below the 400 kHz set earlier in this doc's "I2C clock speed" section.
Confirmed via `cat`, system rebooted after setting it. The board was physically moved/rotated during
both extended runs, same handling as during the earlier crash-prone tests — so this isn't explained
by reduced physical disturbance either.

(`/sys/firmware/devicetree/base/soc/i2c@7e804000/clock-frequency` returned nothing via `cat` this
time — that path holds a raw big-endian 32-bit integer, not text, so a blank `cat` result isn't
necessarily meaningful either way; `od -An -tu4 <path>` would give a reliable readback if this needs
reconfirming.)

**Working hypothesis**: dropping the bus clock this far reduces or avoids a signal-integrity /
clock-stretch-timing-margin problem specific to this physical setup (wiring length/quality, pull-up
values, breadboard capacitance) that produces the chip-reset failure mode. Worth noting this looks
like a *different* failure mode from the torn/malformed-read parsing errors documented earlier
("Hardware verification result" / Python prototype phase) — those were traced to blind fixed-timer
polling racing the sensor's internal buffer swap, and were already independently fixed by adding
HINT-gated reads (bno085_protocol.rs). So the earlier "100 kHz produced more parse errors than
400 kHz" result doesn't contradict this: different bug, different code path (no HINT gating existed
yet), different symptom (garbled reports vs. full chip resets). A slower clock plausibly helps the
reset failure mode by giving more timing margin against a marginal physical layer, independent of
whether it was relevant to the older polling-race problem.

**Confirmed (2026-08-08)**: control test run — `dtparam=i2c_arm_baudrate` reverted to the 100 kHz
default, rebooted, same physical setup and handling (board moved/rotated during testing, as in every
prior run). The earlier crash/reset and error behavior came back. Clean A/B result: 10 kHz → zero
crashes across ~2 hours combined runtime; 100 kHz → crashes returned, nothing else changed. This
confirms I2C clock speed — not board/wiring handling, not sensor calibration or magnetic
environment — is the operative variable behind the connection-drop/reset failure mode.

**Verdict on the original "faulty sensor" hypothesis (connection-drop symptom specifically)**:
not supported. A defective individual unit would not be expected to reliably stop resetting purely
because the host talks to it slower. The clock-speed dependency points at a signal-integrity/timing-
margin limitation in this physical setup (wiring, pull-up values, bus capacitance, or a PHY-level
mismatch between this Pi's I2C controller and this sensor at standard speeds) — a solvable
configuration/wiring issue, not a hardware defect. The separate heading-accuracy-decays-while-
stationary behavior documented above is still open and not addressed by this finding — worth
revisiting once 10 kHz is settled as the operating speed, to see if extended clean runs (no resets)
also show accuracy eventually stabilizing/recovering, or continue degrading regardless of connection
stability.

**If confirmed**: check whether the UPS HAT (sharing `i2c-1`, INA219 `0x43` + onboard MCU `0x2D`)
tolerates 10 kHz without its own issues before committing to this as the permanent bus speed — no
problems were observed with it during the two extended runs, but that's the same
evidence-so-far-but-not-yet-stress-tested caveat as the BNO085 side of this finding.

This continues to weigh against "faulty/inaccurate by design" as the framing for the connection
issue specifically: a genuinely defective individual unit wouldn't be expected to reliably stop
resetting just because the bus clock dropped. A clock-speed-dependent fix points at a signal-timing
margin issue in this setup — wiring, pull-ups, bus capacitance, or a PHY-level compatibility
mismatch between this Pi's I2C controller and this sensor at higher speeds — which is a solvable
configuration/wiring issue, not a hardware-defect verdict.
