#!/usr/bin/env python3
"""Manual verification that the BNO085 (I2C addr 0x4B) responds and reports orientation.
Run, then rotate the board by hand and confirm the quaternion values change."""

import time

import board
import busio

import adafruit_bno08x
from adafruit_bno08x.i2c import BNO08X_I2C

BNO085_I2C_ADDRESS = 0x4B

i2c = busio.I2C(board.SCL, board.SDA, frequency=400000)
bno = BNO08X_I2C(i2c, address=BNO085_I2C_ADDRESS)

bno.enable_feature(adafruit_bno08x.BNO_REPORT_ROTATION_VECTOR)

print("Reading rotation vector. Rotate the board and watch the values change. Ctrl+C to stop.")
while True:
    try:
        quat_i, quat_j, quat_k, quat_real = bno.quaternion
        print(f"I: {quat_i:+.4f}  J: {quat_j:+.4f}  K: {quat_k:+.4f}  Real: {quat_real:+.4f}")
    except (KeyError, IndexError) as e:
        # Known adafruit-circuitpython-bno08x parsing bugs when polling without a HINT
        # pin: unrecognized report type (e.g. 0x7B) or out-of-range channel in its
        # sequence-number table. https://github.com/adafruit/Adafruit_CircuitPython_BNO08x/issues/16
        print(f"(skipped malformed/unrecognized packet: {type(e).__name__}: {e})")
    time.sleep(0.2)
