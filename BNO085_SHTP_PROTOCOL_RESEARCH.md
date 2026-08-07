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

Two failure modes seen during testing, useful as a checklist for any hand-rolled implementation:

- **`KeyError`/`IndexError` crashes in the Adafruit library** from unrecognized report IDs
  (e.g. `0x7B`) and out-of-range channel numbers on parsed packets — see "Implementation patterns"
  below for the root cause and how to avoid repeating it.
- **Transient `OSError: EIO`** from the kernel I2C layer, correlated with physically moving the
  board — consistent with a momentary loose connection (jumper/breadboard contact), not a bus
  lockup or dead device (device re-enumerated cleanly in `i2cdetect` immediately after, and no
  kernel-level bus-fault messages appeared in `dmesg`/`journalctl`). Any real driver needs
  retry/backoff around individual I2C transactions regardless of root cause — normal practice for
  I2C over unshielded wiring, not specific to this sensor.

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
