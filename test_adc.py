#!/usr/bin/env python3
"""
test_adc.py — Quick serial monitor for the STM32 ADC module.

Reads and pretty-prints the data frames sent by the STM32 at 50 Hz.

Frame format (ASCII, newline-terminated):
  $A0,A1,A2,A3,TACHO,SPEED,D0..D9,B0..B7\n

  A0  — Oil pressure    (raw 12-bit ADC, 0-4095)
  A1  — Fuel level      (raw 12-bit ADC, 0-4095)
  A2  — Coolant temp    (raw 12-bit ADC, 0-4095)
  A3  — 12V voltage     (raw 12-bit ADC, 0-4095)
  TACHO  — tachometer pulse count since last report (2 PPR)
  SPEED  — speed pulse count since last report (4 PPR)
  D0..D9 — digital indicator states (0/1)
  B0..B7 — button states (0/1, 1 = pressed)

Usage:
  python3 test_adc.py [device]

  device defaults to /dev/niva_adc; pass /dev/ttyACM0 etc. to override.
"""

import sys
import time
import serial

DEVICE   = sys.argv[1] if len(sys.argv) > 1 else "/dev/niva_adc"
BAUDRATE = 115200

DIGITAL_LABELS = [
    "oil_pres_warn",
    "fuel_low_warn",
    "charging",
    "ext_lights",
    "brake_fluid",
    "headlights",
    "turn_signal",
    "high_beams",
    "parking_brake",
    "diff_lock",
]

BUTTON_LABELS = [f"btn{i}" for i in range(8)]

def parse_frame(line: str):
    """Return a dict of parsed field values, or None if the frame is malformed."""
    line = line.strip()
    if not line.startswith("$"):
        return None
    parts = line[1:].split(",")
    # Frame has 4 analog + 2 pulse + 10 digital + 8 button = 24 fields
    if len(parts) != 24:
        return None
    try:
        values = [int(p) for p in parts]
    except ValueError:
        return None

    return {
        "A0_oil_pres":    values[0],
        "A1_fuel":        values[1],
        "A2_coolant":     values[2],
        "A3_voltage":     values[3],
        "tacho_pulses":   values[4],
        "speed_pulses":   values[5],
        "digital":        dict(zip(DIGITAL_LABELS, values[6:16])),
        "buttons":        dict(zip(BUTTON_LABELS, values[16:24])),
    }

def main():
    print(f"Opening {DEVICE} at {BAUDRATE} baud — Ctrl-C to quit\n")
    try:
        port = serial.Serial(DEVICE, BAUDRATE, timeout=2)
    except serial.SerialException as e:
        print(f"Error opening port: {e}")
        sys.exit(1)

    # Discard any partial frame at the start
    port.readline()

    frame_count  = 0
    error_count  = 0
    t_start      = time.monotonic()
    t_last_print = t_start

    try:
        while True:
            raw = port.readline()
            if not raw:
                print("Timeout — no data received")
                continue

            try:
                line = raw.decode("ascii", errors="replace")
            except Exception:
                error_count += 1
                continue

            data = parse_frame(line)
            if data is None:
                error_count += 1
                continue

            frame_count += 1
            now = time.monotonic()

            # Print a formatted summary once per second
            if now - t_last_print >= 1.0:
                elapsed = now - t_start
                fps     = frame_count / elapsed if elapsed > 0 else 0

                active_digital = [k for k, v in data["digital"].items() if v]
                active_buttons = [k for k, v in data["buttons"].items() if v]

                print(
                    f"\r\n"
                    f"  Elapsed: {elapsed:6.1f}s | Frames: {frame_count:6d} | FPS: {fps:4.1f} | Errors: {error_count}\n"
                    f"  Analog  A0(oil)={data['A0_oil_pres']:4d}  "
                    f"A1(fuel)={data['A1_fuel']:4d}  "
                    f"A2(temp)={data['A2_coolant']:4d}  "
                    f"A3(volt)={data['A3_voltage']:4d}\n"
                    f"  Pulses  tacho={data['tacho_pulses']:4d}  speed={data['speed_pulses']:4d}\n"
                    f"  Digital active: {active_digital or 'none'}\n"
                    f"  Buttons active: {active_buttons or 'none'}"
                )
                t_last_print = now

    except KeyboardInterrupt:
        pass
    finally:
        port.close()
        elapsed = time.monotonic() - t_start
        fps = frame_count / elapsed if elapsed > 0 else 0
        print(f"\n\nDone. {frame_count} frames in {elapsed:.1f}s ({fps:.1f} FPS), {error_count} errors.")

if __name__ == "__main__":
    main()
