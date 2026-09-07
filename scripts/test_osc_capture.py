#!/usr/bin/env python3
"""
test_osc_capture.py — Exercise the STM32 oscilloscope burst-capture path directly.

Sends "$OSCCAP" to the ADC module and reassembles the chunked
"$OSCD,<seq>,v0..v63" / "$OSCEND" response into one PA3 sample buffer, then
reports on it. Use to isolate whether "oscilloscope mode stopped working" is a
firmware/capture problem or a Pi-side (ADCDataProvider / OscPage) problem.

The dashboard must NOT be running — it holds the port open exclusively:
    sudo systemctl stop niva-dashboard
    python3 scripts/test_osc_capture.py [device]
    sudo systemctl start niva-dashboard

device defaults to /dev/niva_adc.

Firmware constants (stm32_adc_module/.../main.cpp), mirrored here:
  OSC_ADC_CHANNEL    = PA3 (ADC_IN3), same pin as telemetry A3 (12V bus)
  OSC_SAMPLE_RATE_HZ = 50000
  OSC_BUF_LEN        = 4096  (-> 64 chunks of 64 samples, ~82 ms window)
  ADC is 12-bit, right-aligned, Vref = 3.3V  => volts = raw / 4095 * 3.3
"""

import sys
import time
import statistics
import serial

DEVICE = sys.argv[1] if len(sys.argv) > 1 else "/dev/niva_adc"
BAUD = 115200

OSC_BUF_LEN = 4096
OSC_CHUNK_SAMPLES = 64
OSC_EXPECTED_CHUNKS = OSC_BUF_LEN // OSC_CHUNK_SAMPLES
OSC_SAMPLE_RATE_HZ = 50000
ADC_VREF = 3.3
ADC_FULL_SCALE = 4095

CAPTURE_TIMEOUT_S = 6.0


def raw_to_volts(raw):
    return raw / ADC_FULL_SCALE * ADC_VREF


def read_baseline_frames(port, n=10):
    """Grab a few normal telemetry frames; return list of A3 raw values."""
    a3 = []
    deadline = time.monotonic() + 3.0
    port.reset_input_buffer()
    port.readline()  # discard partial
    while len(a3) < n and time.monotonic() < deadline:
        line = port.readline().decode("ascii", errors="replace").strip()
        if not line.startswith("$") or line.startswith("$OSC") or line.startswith("$T"):
            continue
        parts = line[1:].split(",")
        if len(parts) != 24:
            continue
        try:
            a3.append(int(parts[3]))
        except ValueError:
            pass
    return a3


def capture(port):
    """Send $OSCCAP, collect chunks. Returns (samples_or_None, chunks_seen_set, extra_lines)."""
    chunks = [None] * OSC_EXPECTED_CHUNKS
    seen = set()
    extra = []
    got_end = False

    port.reset_input_buffer()
    try:
        port.write(b"$OSCCAP\n")
        port.flush()
    except serial.SerialTimeoutException:
        return None, seen, [("write-timeout", "STM32 not reading USB RX — command not delivered")], False

    start = time.monotonic()
    while time.monotonic() - start < CAPTURE_TIMEOUT_S:
        raw = port.readline()
        if not raw:
            continue
        line = raw.decode("ascii", errors="replace").strip()
        if line == "$OSCEND":
            got_end = True
            break
        if line.startswith("$OSCD,"):
            fields = line[len("$OSCD,"):].split(",")
            try:
                seq = int(fields[0])
                vals = [int(x) for x in fields[1:]]
            except ValueError:
                extra.append(("bad-oscd", line[:80]))
                continue
            if 0 <= seq < OSC_EXPECTED_CHUNKS and len(vals) == OSC_CHUNK_SAMPLES:
                chunks[seq] = vals
                seen.add(seq)
            else:
                extra.append(("bad-oscd-shape", f"seq={seq} n={len(vals)}"))
        elif line.startswith("$A") or line.startswith("$T"):
            extra.append(("telemetry", line[:60]))
        elif line:
            extra.append(("other", line[:80]))

    if not got_end and len(seen) < OSC_EXPECTED_CHUNKS:
        return None, seen, extra, got_end

    if len(seen) < OSC_EXPECTED_CHUNKS:
        return None, seen, extra, got_end

    samples = []
    for c in chunks:
        samples.extend(c)
    return samples, seen, extra, got_end


def ascii_plot(samples, width=72, height=14):
    lo, hi = min(samples), max(samples)
    span = max(hi - lo, 1)
    step = max(len(samples) // width, 1)
    cols = [samples[i] for i in range(0, len(samples), step)][:width]
    grid = [[" "] * len(cols) for _ in range(height)]
    for x, v in enumerate(cols):
        y = height - 1 - int((v - lo) / span * (height - 1))
        grid[y][x] = "*"
    out = []
    for y, row in enumerate(grid):
        val = hi - (y / (height - 1)) * span
        out.append(f"{val:6.0f} |" + "".join(row))
    out.append("       +" + "-" * len(cols))
    out.append(f"        0{' ' * (len(cols) - 10)}{len(samples)} samples")
    return "\n".join(out)


def main():
    print(f"Opening {DEVICE} @ {BAUD} (exclusive)\n")
    try:
        # write_timeout bounds the write: a firmware that isn't reading its USB RX
        # leaves the CDC OUT endpoint unserviced and tcdrain()/write() block forever.
        port = serial.Serial(DEVICE, BAUD, timeout=1.0, write_timeout=3.0, exclusive=True)
    except serial.SerialException as e:
        print(f"Error opening port: {e}")
        print("Is the dashboard still running? -> sudo systemctl stop niva-dashboard")
        sys.exit(1)

    with port:
        print("Baseline: reading normal telemetry frames (A3 = 12V-bus channel)...")
        a3 = read_baseline_frames(port)
        if a3:
            mean_a3 = statistics.mean(a3)
            print(f"  A3 raw over {len(a3)} frames: min={min(a3)} max={max(a3)} "
                  f"mean={mean_a3:.1f}  (~{raw_to_volts(mean_a3):.3f} V at the pin)")
        else:
            print("  No telemetry frames parsed — link may be down or firmware not sending.")
        print()

        print("Sending $OSCCAP ...")
        t0 = time.monotonic()
        samples, seen, extra, got_end = capture(port)
        dt = time.monotonic() - t0
        print(f"  capture round-trip: {dt * 1000:.0f} ms, "
              f"{len(seen)}/{OSC_EXPECTED_CHUNKS} chunks, $OSCEND={'yes' if got_end else 'NO'}")

        tel = sum(1 for k, _ in extra if k == "telemetry")
        oth = [v for k, v in extra if k not in ("telemetry",)]
        if tel:
            print(f"  ({tel} telemetry lines seen mid-capture — normal after resume)")
        for v in oth[:10]:
            print(f"  unexpected line: {v}")

        if samples is None:
            missing = sorted(set(range(OSC_EXPECTED_CHUNKS)) - seen)
            print("\nFAIL: incomplete capture.")
            if any(k == "write-timeout" for k, _ in extra):
                print("  write() to the STM32 timed out — the module is not reading its")
                print("  USB serial RX at all. The flashed firmware predates the $OSCCAP")
                print("  handler (poll_incoming_commands). Reflash from stm32_adc_module/.")
            elif not seen:
                print("  Zero $OSCD chunks received. Likely causes:")
                print("   - firmware on the STM32 predates the $OSCCAP command")
                print("   - command not reaching the MCU (RX path / wrong port)")
            elif missing:
                print(f"  Missing chunk seqs: {missing}")
            sys.exit(2)

        n = len(samples)
        lo, hi = min(samples), max(samples)
        mean = statistics.mean(samples)
        stdev = statistics.pstdev(samples)
        pkpk = hi - lo
        window_ms = n / OSC_SAMPLE_RATE_HZ * 1000
        print(f"\n{n} samples, {window_ms:.1f} ms window @ {OSC_SAMPLE_RATE_HZ} SPS")
        print(f"  raw:   min={lo}  max={hi}  mean={mean:.1f}  pk-pk={pkpk}  stdev={stdev:.1f}")
        print(f"  volts: min={raw_to_volts(lo):.3f}  max={raw_to_volts(hi):.3f}  "
              f"mean={raw_to_volts(mean):.3f}  pk-pk={raw_to_volts(pkpk):.3f}")
        print(f"\nfirst 32 raw: {samples[:32]}")
        print(f"last  32 raw: {samples[-32:]}")
        print()
        print(ascii_plot(samples))

        print("\nSanity check:")
        if lo == 0 and hi == 0:
            print("  ALL ZERO — DMA/ADC capture not running (timer TRGO, DMA clock, or channel cfg).")
        elif lo >= ADC_FULL_SCALE - 1:
            print("  RAILED HIGH — PA3 reading full-scale; check divider / pin mode / Vref.")
        elif pkpk <= 2:
            print(f"  Flat line at ~{raw_to_volts(mean):.3f} V. Expected ~2.2 V -> raw ~{int(2.2/ADC_VREF*ADC_FULL_SCALE)}.")
            print("  Flat is fine for a DC bench source; compare the mean above to your meter.")
        else:
            print(f"  Varying signal, ~{raw_to_volts(mean):.3f} V mean. Compare to meter (~2.2 V).")


if __name__ == "__main__":
    main()
