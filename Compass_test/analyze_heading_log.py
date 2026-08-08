#!/usr/bin/env python3
"""Analyze a `cargo run -- test=heading` log (see COMPASS_SENSORS_TESTING.md) — GNSS dual-antenna
heading convergence/stability, BNO085 RV/Game RV/Geomagnetic RV convergence and self-reported
accuracy, and BNO085 frame-staleness rate. Reads log lines produced by `run_heading_test` in
niva_dashboard/src/test/run_test.rs; if that log line format changes, LINE_PATTERN below needs
updating to match.

Usage: analyze_heading_log.py LOGFILE [--bucket-seconds 60] [--tail-minutes 5]
"""

import argparse
import math
import re
import statistics
import sys
from collections import defaultdict


def circular_mean(degs):
    """Mean of angles in degrees, correct across the 0/360 wrap (e.g. a heading sitting at
    359.9/0.1 alternately averages to ~0, not ~180 like a plain arithmetic mean would give)."""
    sin_sum = sum(math.sin(math.radians(d)) for d in degs)
    cos_sum = sum(math.cos(math.radians(d)) for d in degs)
    return math.degrees(math.atan2(sin_sum, cos_sum)) % 360


def circular_stdev(degs):
    n = len(degs)
    sin_sum = sum(math.sin(math.radians(d)) for d in degs)
    cos_sum = sum(math.cos(math.radians(d)) for d in degs)
    r = math.sqrt(sin_sum ** 2 + cos_sum ** 2) / n
    if r <= 0:
        return float('nan')
    return math.degrees(math.sqrt(-2 * math.log(r)))

LINE_PATTERN = re.compile(
    r'\[\+\s*([\d.]+)s\] GNSS=\s*(---|\d+\.\d+)°?\s*\(std=(--|\d+\.\d+)°?\s*sat=(--|\d+)\s*fix=(\S+)\)\s*\|\s*'
    r'RV=\s*(---|\d+\.\d+)°?\s*\((\S+)\)\s*\|\s*'
    r'GAME=\s*(---|\d+\.\d+)°?\s*\|\s*'
    r'GEO=\s*(---|\d+\.\d+)°?\s*\((\S+)\)'
)

FIELDS = ('t', 'gnss', 'std', 'sat', 'fix', 'rv', 'rv_acc', 'game', 'geo', 'geo_acc')


def parse_log(path):
    rows = []
    unparsed = 0
    with open(path, encoding='utf-8', errors='replace') as f:
        for line in f:
            if '[+' not in line:
                continue
            m = LINE_PATTERN.search(line)
            if not m:
                unparsed += 1
                continue
            values = list(m.groups())
            values[0] = float(values[0])
            rows.append(dict(zip(FIELDS, values)))
    return rows, unparsed


PLACEHOLDERS = ('---', '--')


def floats(rows, key, predicate=None):
    return [float(r[key]) for r in rows if r[key] not in PLACEHOLDERS and (predicate is None or predicate(r))]


def bucket_index(t, bucket_seconds):
    return int(t // bucket_seconds)


def print_gnss_section(rows, bucket_seconds, tail_minutes):
    print("\n=== GNSS dual-antenna heading (#UNIHEADINGA heading_deg) ===")
    vals = floats(rows, 'gnss')
    if not vals:
        print("  no GNSS fixes in this log")
        return
    duration = rows[-1]['t']
    print(f"  n={len(vals)}  range=[{min(vals):.1f}, {max(vals):.1f}]  span={max(vals) - min(vals):.1f} deg")

    tail_cut = duration - tail_minutes * 60
    tail_vals = floats(rows, 'gnss', lambda r: r['t'] > tail_cut)
    if len(tail_vals) > 1:
        print(f"  last {tail_minutes} min: n={len(tail_vals)}  range=[{min(tail_vals):.1f}, {max(tail_vals):.1f}]  "
              f"circ_mean={circular_mean(tail_vals):.1f}  circ_stdev={circular_stdev(tail_vals):.2f}")

    stds = floats(rows, 'std')
    if stds:
        print(f"  self-reported std: min={min(stds):.1f}  max={max(stds):.1f}  mean={statistics.mean(stds):.1f}")

    sats = [int(r['sat']) for r in rows if r['sat'] != '--']
    if sats:
        print(f"  satellite count range: {min(sats)}-{max(sats)}")

    fixes = sorted(set(r['fix'] for r in rows))
    print(f"  fix_quality values seen: {fixes}")

    stale = sum(1 for r in rows if r['gnss'] == '---')
    print(f"  stale/unavailable ticks: {stale}/{len(rows)}")

    print(f"\n  {bucket_seconds}s buckets (min/max heading, mean self-reported std):")
    buckets = defaultdict(list)
    for r in rows:
        if r['gnss'] != '---':
            buckets[bucket_index(r['t'], bucket_seconds)].append(r)
    for b in sorted(buckets):
        seg = buckets[b]
        g = [float(r['gnss']) for r in seg]
        s = [float(r['std']) for r in seg if r['std'] != '--']
        lo, hi = b * bucket_seconds, b * bucket_seconds + bucket_seconds
        std_str = f"{statistics.mean(s):5.1f}" if s else "  n/a"
        print(f"    t={lo:6d}-{hi:<6d}s  heading=[{min(g):6.1f}, {max(g):6.1f}]  std_mean={std_str}")


def print_bno_source(rows, label, key, acc_key, bucket_seconds):
    vals = [(r['t'], float(r[key])) for r in rows if r[key] != '---']
    print(f"\n--- {label} ---")
    if not vals:
        print("  no data")
        return
    print(f"  n={len(vals)}  first={vals[0][1]:.1f}  last={vals[-1][1]:.1f}  "
          f"min={min(v for _, v in vals):.1f}  max={max(v for _, v in vals):.1f}")

    if acc_key is not None:
        accs = sorted(set(r[acc_key] for r in rows))
        print(f"  accuracy values seen: {accs}")
    else:
        print("  accuracy: n/a (Game RV has no absolute reference, doc: heading_accuracy_deg always 0.0)")

    stale = sum(1 for r in rows if r[key] == '---')
    print(f"  stale ticks: {stale}/{len(rows)} ({100 * stale / len(rows):.0f}%)")

    buckets = defaultdict(list)
    for t, v in vals:
        buckets[bucket_index(t, bucket_seconds)].append(v)
    print(f"  {bucket_seconds}s bucket circular means:")
    for b in sorted(buckets):
        lo, hi = b * bucket_seconds, b * bucket_seconds + bucket_seconds
        print(f"    t={lo:6d}-{hi:<6d}s  mean={circular_mean(buckets[b]):6.1f}")


def print_staleness_by_bucket(rows, bucket_seconds):
    print(f"\n=== BNO085 frame staleness by {bucket_seconds}s bucket (RV/GAME/GEO share one flag) ===")
    buckets = defaultdict(lambda: [0, 0])
    for r in rows:
        b = bucket_index(r['t'], bucket_seconds)
        buckets[b][1] += 1
        if r['rv'] == '---':
            buckets[b][0] += 1
    for b in sorted(buckets):
        stale, total = buckets[b]
        lo, hi = b * bucket_seconds, b * bucket_seconds + bucket_seconds
        print(f"  t={lo:6d}-{hi:<6d}s  stale={stale:4d}/{total:<4d} ({100 * stale / total:.0f}%)")


def print_link_events(path):
    print("\n=== Link/error events (connect, reconnect, stale, WARN, ERROR) ===")
    pattern = re.compile(r'reconnect|disconnect|WARN|ERROR|connected|Failed', re.IGNORECASE)
    found = False
    with open(path, encoding='utf-8', errors='replace') as f:
        for line in f:
            if pattern.search(line) and '[+' not in line:
                print(f"  {line.rstrip()}")
                found = True
    if not found:
        print("  none")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('logfile')
    parser.add_argument('--bucket-seconds', type=int, default=60)
    parser.add_argument('--tail-minutes', type=float, default=5)
    args = parser.parse_args()

    rows, unparsed = parse_log(args.logfile)
    if not rows:
        print(f"No heading-test log lines matched in {args.logfile}", file=sys.stderr)
        sys.exit(1)

    duration = rows[-1]['t']
    print(f"Parsed {len(rows)} log lines ({unparsed} unparsed) spanning {duration:.1f}s ({duration / 60:.1f} min)")

    print_gnss_section(rows, args.bucket_seconds, args.tail_minutes)

    print("\n=== BNO085 rotation sources ===")
    print_bno_source(rows, "Rotation Vector (RV, gyro+accel+mag)", 'rv', 'rv_acc', args.bucket_seconds)
    print_bno_source(rows, "Game Rotation Vector (GAME, gyro+accel)", 'game', None, args.bucket_seconds)
    print_bno_source(rows, "Geomagnetic Rotation Vector (GEO, accel+mag)", 'geo', 'geo_acc', args.bucket_seconds)

    print_staleness_by_bucket(rows, args.bucket_seconds)
    print_link_events(args.logfile)


if __name__ == '__main__':
    main()
