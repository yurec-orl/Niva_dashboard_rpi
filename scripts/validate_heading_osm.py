#!/usr/bin/env python3
"""validate_heading_osm.py — Cross-check a nav_validation.csv log's heading fields against
real road direction, using OpenStreetMap way geometry fetched from the Overpass API.

This is an independent, external check: the dashboard's own cross-validation (INS vs GNSS
course agreeing with each other) only shows the two sensors agree, not that either is actually
right -- a shared calibration/mounting bias would pass that check. Matching against OSM road
geometry instead compares heading to ground truth that neither sensor had any part in producing.

For each sampled log point: project it onto the nearest OSM road segment within
--max-match-dist, take that segment's bearing, and compare it to ins_heading_deg/
fused_heading_deg/gnss_course_deg mod 180 (a road carries traffic both ways, so a heading
pointing along the road in either direction counts as aligned). Points that land within
--endpoint-radius of a segment's end are flagged low-confidence and excluded from headline
stats -- that's a junction/intersection, where several roads meet within GPS's own error
radius and the "nearest segment" pick is unreliable (worst exactly at sharp turns, which is
also where you most want to check -- see the module's design discussion).

Network: a single Overpass API request per bounding box, cached to --cache-dir so repeat runs
don't re-hit the public server. Needs internet access.

Usage:
  validate_heading_osm.py LOGFILE [--min-speed 10] [--sample-seconds 5]
                          [--max-match-dist 20] [--endpoint-radius 6] [--worst 15]
                          [--cache-dir scripts/.osm_cache] [--refresh-cache]
                          [--at UNIX_TIMESTAMP] [--csv-out PATH]
"""

import argparse
import csv
import hashlib
import json
import math
import os
import statistics
import sys
import urllib.parse
import urllib.request
from collections import namedtuple
from datetime import datetime

EARTH_RADIUS_M = 6371000.0
OVERPASS_URL = "https://overpass-api.de/api/interpreter"

# Road classes a car could plausibly be on. Excludes footway/cycleway/path/steps/etc, which
# would otherwise out-match the real road at points near a sidewalk.
DRIVABLE_HIGHWAY = (
    "motorway", "trunk", "primary", "secondary", "tertiary", "unclassified",
    "residential", "living_street", "service",
    "motorway_link", "trunk_link", "primary_link", "secondary_link", "tertiary_link",
)

Segment = namedtuple("Segment", ["ax", "ay", "bx", "by", "alat", "alon", "blat", "blon",
                                  "bearing", "way_id", "name", "highway"])


def read_log(path):
    rows = []
    with open(path, newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            def g(key):
                v = row.get(key, "")
                return float(v) if v not in (None, "") else None
            rows.append({
                "t": float(row["unix_time_secs"]),
                "ins": g("ins_heading_deg"),
                "fused": g("fused_heading_deg"),
                "lat": g("lat"),
                "lon": g("lon"),
                "course": g("gnss_course_deg"),
                "gnss_speed": g("gnss_speed_kmh"),
                "logical_speed": g("logical_speed_kmh"),
            })
    return rows


def bbox_of(rows, pad_m=150.0):
    lats = [r["lat"] for r in rows if r["lat"] is not None]
    lons = [r["lon"] for r in rows if r["lon"] is not None]
    if not lats:
        raise ValueError("log has no GNSS fixes -- nothing to match against OSM")
    south, north = min(lats), max(lats)
    west, east = min(lons), max(lons)
    midlat = (south + north) / 2
    dlat = pad_m / 111320.0
    dlon = pad_m / (111320.0 * math.cos(math.radians(midlat)))
    return south - dlat, west - dlon, north + dlat, east + dlon


def cache_path(cache_dir, bbox):
    key = hashlib.sha1(("%.6f,%.6f,%.6f,%.6f" % bbox).encode()).hexdigest()[:16]
    return os.path.join(cache_dir, f"osm_{key}.json")


def fetch_osm_ways(bbox, cache_dir, overpass_url, refresh=False):
    os.makedirs(cache_dir, exist_ok=True)
    path = cache_path(cache_dir, bbox)
    if not refresh and os.path.exists(path):
        print(f"Using cached OSM data: {path}", file=sys.stderr)
        with open(path, encoding="utf-8") as f:
            return json.load(f)["elements"]

    south, west, north, east = bbox
    highway_re = "^(%s)$" % "|".join(DRIVABLE_HIGHWAY)
    query = (
        "[out:json][timeout:60];\n"
        f'(way["highway"~"{highway_re}"]({south:.6f},{west:.6f},{north:.6f},{east:.6f}););\n'
        "out geom;"
    )
    req = urllib.request.Request(
        overpass_url,
        data=urllib.parse.urlencode({"data": query}).encode(),
        headers={"User-Agent": "niva-dashboard-heading-validator/1.0 (offline analysis script)"},
    )
    print(f"Querying Overpass API ({overpass_url}) for bbox {bbox} ...", file=sys.stderr)
    with urllib.request.urlopen(req, timeout=90) as resp:
        data = json.load(resp)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f)
    return data["elements"]


def latlon_to_xy(lat, lon, ref_lat):
    """Local flat-earth projection, accurate enough over a route spanning a few km -- avoids
    pulling in a projection library for what's otherwise a stdlib-only script."""
    x = math.radians(lon) * math.cos(math.radians(ref_lat)) * EARTH_RADIUS_M
    y = math.radians(lat) * EARTH_RADIUS_M
    return x, y


def bearing_deg(lat1, lon1, lat2, lon2):
    """Initial geodesic bearing from point 1 to point 2, degrees true, 0-360."""
    phi1, phi2 = math.radians(lat1), math.radians(lat2)
    dlon = math.radians(lon2 - lon1)
    y = math.sin(dlon) * math.cos(phi2)
    x = math.cos(phi1) * math.sin(phi2) - math.sin(phi1) * math.cos(phi2) * math.cos(dlon)
    return math.degrees(math.atan2(y, x)) % 360.0


def build_segments(elements, ref_lat):
    segments = []
    for el in elements:
        if el.get("type") != "way" or "geometry" not in el:
            continue
        tags = el.get("tags", {})
        geom = el["geometry"]
        for a, b in zip(geom, geom[1:]):
            if a.get("lat") is None or b.get("lat") is None:
                continue
            if a["lat"] == b["lat"] and a["lon"] == b["lon"]:
                continue
            br = bearing_deg(a["lat"], a["lon"], b["lat"], b["lon"])
            ax, ay = latlon_to_xy(a["lat"], a["lon"], ref_lat)
            bx, by = latlon_to_xy(b["lat"], b["lon"], ref_lat)
            segments.append(Segment(ax, ay, bx, by, a["lat"], a["lon"], b["lat"], b["lon"],
                                     br, el["id"], tags.get("name", ""), tags.get("highway", "")))
    return segments


def closest_point_on_segment(px, py, seg):
    dx, dy = seg.bx - seg.ax, seg.by - seg.ay
    len2 = dx * dx + dy * dy
    if len2 == 0:
        t = 0.0
    else:
        t = ((px - seg.ax) * dx + (py - seg.ay) * dy) / len2
        t = max(0.0, min(1.0, t))
    cx, cy = seg.ax + t * dx, seg.ay + t * dy
    return math.hypot(px - cx, py - cy), t


def nearest_segment(px, py, segments):
    best, best_dist, best_t = None, None, None
    for seg in segments:
        dist, t = closest_point_on_segment(px, py, seg)
        if best_dist is None or dist < best_dist:
            best, best_dist, best_t = seg, dist, t
    return best, best_dist, best_t


def axis_diff(heading, bearing):
    """Angle between `heading` and the road axis defined by `bearing`, folded into [0,90] --
    a two-way road is "aligned" whether driven in the way's digitized direction or reverse."""
    d = abs(heading - bearing) % 180.0
    return min(d, 180.0 - d)


def endpoint_dist_m(px, py, seg, t):
    end_x, end_y = (seg.ax, seg.ay) if t <= 0.5 else (seg.bx, seg.by)
    return math.hypot(px - end_x, py - end_y)


def local_time(t):
    return datetime.fromtimestamp(t).strftime("%H:%M:%S")


def sample_rows(rows, min_speed, sample_seconds):
    """Decimates to moving samples with a real fix, at most one every `sample_seconds`."""
    out = []
    last_t = None
    for r in rows:
        if r["lat"] is None or r["gnss_speed"] is None or r["gnss_speed"] < min_speed:
            continue
        if last_t is not None and r["t"] - last_t < sample_seconds:
            continue
        out.append(r)
        last_t = r["t"]
    return out


def evaluate_point(r, segments, ref_lat, max_match_dist, endpoint_radius):
    px, py = latlon_to_xy(r["lat"], r["lon"], ref_lat)
    seg, dist, t = nearest_segment(px, py, segments)
    if seg is None or dist > max_match_dist:
        return {"row": r, "ok": False, "reason": f"no road within {max_match_dist:.0f}m"}
    if endpoint_dist_m(px, py, seg, t) < endpoint_radius:
        return {"row": r, "ok": False, "reason": "near a junction/endpoint", "seg": seg, "dist": dist}

    result = {"row": r, "ok": True, "seg": seg, "dist": dist}
    for key in ("ins", "fused", "course"):
        if r[key] is not None:
            result[key] = axis_diff(r[key], seg.bearing)
    return result


def print_summary(label, results, key):
    vals = [r[key] for r in results if r.get("ok") and key in r]
    if not vals:
        print(f"  {label}: no data")
        return
    print(f"  {label}: n={len(vals)}  mean={statistics.mean(vals):.1f} deg  "
          f"median={statistics.median(vals):.1f} deg  max={max(vals):.1f} deg")


def print_worst(results, key, label, n):
    confident = [r for r in results if r.get("ok") and key in r]
    confident.sort(key=lambda r: -r[key])
    print(f"\n  Worst {label} mismatches:")
    for r in confident[:n]:
        row, seg = r["row"], r["seg"]
        road = seg.name or f"({seg.highway}, unnamed)"
        print(f"    {local_time(row['t'])}  diff={r[key]:5.1f} deg  heading={row[key]:6.1f}  "
              f"road_bearing={seg.bearing:6.1f}  dist={r['dist']:4.1f}m  "
              f"lat={row['lat']:.6f} lon={row['lon']:.6f}  road={road}")


def drill_into(rows, segments, ref_lat, at, max_match_dist, endpoint_radius):
    r = min(rows, key=lambda row: abs(row["t"] - at))
    print(f"\n=== Nearest log row to t={at}: {local_time(r['t'])} "
          f"(unix {r['t']:.3f}, {abs(r['t'] - at):.2f}s away) ===")
    print(f"  lat={r['lat']}, lon={r['lon']}, ins={r['ins']}, fused={r['fused']}, "
          f"gnss_course={r['course']}, gnss_speed={r['gnss_speed']}")
    if r["lat"] is None:
        print("  no GNSS fix at this row -- can't match to OSM")
        return
    px, py = latlon_to_xy(r["lat"], r["lon"], ref_lat)
    ranked = sorted(
        ((closest_point_on_segment(px, py, s)[0], s, closest_point_on_segment(px, py, s)[1])
         for s in segments),
        key=lambda x: x[0],
    )[:5]
    print("  5 nearest road segments:")
    for dist, seg, t in ranked:
        near_end = endpoint_dist_m(px, py, seg, t) < endpoint_radius
        road = seg.name or f"({seg.highway}, unnamed)"
        flag = "  <-- near junction/endpoint" if near_end else ""
        print(f"    dist={dist:5.1f}m  bearing={seg.bearing:6.1f}  road={road}{flag}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("logfile")
    parser.add_argument("--min-speed", type=float, default=10.0,
                         help="km/h; below this, direction of travel is ill-defined (default 10, matches heading_fusion_sensor's MIN_SPEED_FOR_COURSE_KMH)")
    parser.add_argument("--sample-seconds", type=float, default=5.0)
    parser.add_argument("--max-match-dist", type=float, default=20.0, help="meters")
    parser.add_argument("--endpoint-radius", type=float, default=6.0, help="meters")
    parser.add_argument("--worst", type=int, default=15)
    parser.add_argument("--cache-dir", default=os.path.join(os.path.dirname(__file__), ".osm_cache"))
    parser.add_argument("--refresh-cache", action="store_true")
    parser.add_argument("--overpass-url", default=OVERPASS_URL,
                         help="override if the default public instance is unreachable/rate-limited; "
                              "e.g. https://overpass.kumi.systems/api/interpreter")
    parser.add_argument("--at", type=float, default=None, help="unix timestamp to drill into instead of running the full report")
    parser.add_argument("--csv-out", default=None)
    args = parser.parse_args()

    rows = read_log(args.logfile)
    print(f"Parsed {len(rows)} rows from {args.logfile}")

    bbox = bbox_of(rows)
    elements = fetch_osm_ways(bbox, args.cache_dir, args.overpass_url, args.refresh_cache)
    ref_lat = (bbox[0] + bbox[2]) / 2
    segments = build_segments(elements, ref_lat)
    print(f"Fetched {len(elements)} OSM ways -> {len(segments)} road segments in bbox {bbox}")

    if args.at is not None:
        drill_into(rows, segments, ref_lat, args.at, args.max_match_dist, args.endpoint_radius)
        return

    samples = sample_rows(rows, args.min_speed, args.sample_seconds)
    print(f"Evaluating {len(samples)} samples (speed>={args.min_speed} km/h, "
          f">= {args.sample_seconds}s apart)")

    results = [evaluate_point(r, segments, ref_lat, args.max_match_dist, args.endpoint_radius)
               for r in samples]
    excluded = [r for r in results if not r["ok"]]
    print(f"\n{len(results) - len(excluded)}/{len(results)} samples matched confidently "
          f"({len(excluded)} excluded: no nearby road or near a junction)")

    print("\n=== Heading vs road-axis alignment (mod 180, folded to [0,90]) ===")
    print_summary("INS heading", results, "ins")
    print_summary("Fused heading (dashboard output)", results, "fused")
    print_summary("GNSS course", results, "course")

    print_worst(results, "fused", "fused heading", args.worst)

    if args.csv_out:
        out_dir = os.path.dirname(args.csv_out)
        if out_dir:
            os.makedirs(out_dir, exist_ok=True)
        with open(args.csv_out, "w", newline="", encoding="utf-8") as f:
            w = csv.writer(f)
            w.writerow(["unix_time_secs", "lat", "lon", "ok", "reason", "road_bearing_deg",
                        "match_dist_m", "ins_diff_deg", "fused_diff_deg", "course_diff_deg"])
            for r in results:
                row = r["row"]
                seg = r.get("seg")
                w.writerow([row["t"], row["lat"], row["lon"], r["ok"], r.get("reason", ""),
                            seg.bearing if seg else "", r.get("dist", ""),
                            r.get("ins", ""), r.get("fused", ""), r.get("course", "")])
        print(f"\nWrote per-sample results to {args.csv_out}")


if __name__ == "__main__":
    main()
