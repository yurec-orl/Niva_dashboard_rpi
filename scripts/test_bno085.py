#!/usr/bin/env python3
"""Manual verification that the BNO085 (I2C addr 0x4B) responds and reports orientation.
Run, then rotate the board by hand and confirm the quaternion values change."""

import time

import board
import busio

import adafruit_bno08x
from adafruit_bno08x.i2c import BNO08X_I2C

BNO085_I2C_ADDRESS = 0x4B

# Roughly how many idle-loop iterations (at the 0.2s poll interval below) an exact-repeat
# reading is allowed to persist before we assume the sensor stopped producing new reports
# (e.g. a reset silently cleared its feature-enable state) and re-enable the feature.
STALE_READING_THRESHOLD = 15

i2c = busio.I2C(board.SCL, board.SDA, frequency=400000)
bno = BNO08X_I2C(i2c, address=BNO085_I2C_ADDRESS)

bno.enable_feature(adafruit_bno08x.BNO_REPORT_ROTATION_VECTOR)


def re_enable_feature(reason):
    print(f"{reason} — re-enabling rotation vector feature in case the sensor reset.")
    try:
        bno.enable_feature(adafruit_bno08x.BNO_REPORT_ROTATION_VECTOR)
    except (OSError, RuntimeError) as re_enable_error:
        print(f"(re-enable failed, will keep retrying: {re_enable_error})")


print("Reading rotation vector. Rotate the board and watch the values change. Ctrl+C to stop.")
consecutive_io_errors = 0
last_quaternion = None
stale_reads = 0
while True:
    try:
        quaternion = bno.quaternion
        quat_i, quat_j, quat_k, quat_real = quaternion
        print(f"I: {quat_i:+.4f}  J: {quat_j:+.4f}  K: {quat_k:+.4f}  Real: {quat_real:+.4f}")
        consecutive_io_errors = 0

        # Live sensor output always has a small amount of jitter, even at rest, so an
        # exact-repeat reading means no new report has actually arrived (e.g. after a
        # sensor reset silently cleared the feature-enable state), not that the board
        # is perfectly still.
        if quaternion == last_quaternion:
            stale_reads += 1
            if stale_reads >= STALE_READING_THRESHOLD:
                re_enable_feature(f"{stale_reads} consecutive identical readings")
                stale_reads = 0
        else:
            stale_reads = 0
        last_quaternion = quaternion
    except (KeyError, IndexError, RuntimeError) as e:
        # Known adafruit-circuitpython-bno08x parsing bugs when polling without a HINT pin:
        # unrecognized report type (e.g. 0x7B), out-of-range channel in its sequence-number
        # table, or unhandled non-report packets (e.g. the EXE channel's "reset complete"
        # notification) being fed through the sensor-report batch parser.
        # https://github.com/adafruit/Adafruit_CircuitPython_BNO08x/issues/16
        print(f"(skipped malformed/unrecognized packet: {type(e).__name__}: {e})")
        consecutive_io_errors = 0
    except OSError as e:
        # Transient I2C bus errors (e.g. EIO from a momentary loose connection). Back off
        # briefly and retry rather than dying, but surface it if it stops being transient.
        consecutive_io_errors += 1
        print(f"(I2C error, retrying: {e})")
        if consecutive_io_errors >= 20:
            print(
                f"{consecutive_io_errors} consecutive I2C errors — check wiring/connections, "
                "this no longer looks transient."
            )
        time.sleep(0.5)
        continue
    time.sleep(0.2)
