# OpenStreetMap Data Structure and Offline Navigation Feasibility

**Date**: October 5, 2026
**Status**: Research only — no map/tile/routing code exists in the repo yet.
**Question**: Can OSM data drive an offline moving map, offline route building, and real-time
turn-by-turn navigation on the Pi 4?

**Short answer**: Yes to all three. The moving map and the router are well-defined problems; the
turn-by-turn navigation state machine is where most of the unbounded work lives.

## 1. The OSM data model

Three primitives plus a free-form tag system. That is the entire model.

| Primitive | Contents | Notes |
|---|---|---|
| **Node** | 64-bit id, lat/lon (fixed-point 1e-7 deg), optional tags | Most nodes are untagged and exist only as way geometry. Tagged nodes are standalone features: `highway=traffic_signals`, `barrier=gate`, `amenity=fuel`. |
| **Way** | Ordered list of node *references* (max 2000), tags | A linestring. Closed if first ref == last ref, but whether a closed way is an area is decided by **tags, not geometry** (`building=yes` closed → polygon; `highway=footway` closed → loop path). |
| **Relation** | Ordered list of members (node/way/relation), each with a **role** string, plus tags | The `type=*` tag selects the interpretation. |

Relation types that matter here:

- `type=multipolygon` (`outer`/`inner` roles) — polygons with holes, and any area too large for a
  single 2000-node way. Required for correct rendering of forests, lakes, large buildings.
- `type=restriction` (`from`/`via`/`to` roles) — **turn restrictions**. Required for legal routing.
- `type=route` — numbered road routes; source of route shields / `ref` display.
- `type=boundary` — administrative areas.

### Tags

Arbitrary UTF-8 `key=value` pairs with **no schema and no validation**. Conventions live on the OSM
wiki, not in the data. Consequences to design around:

- Values are inconsistent (`maxspeed=60`, `60 km/h`, `RU:urban`) and frequently absent.
- Quality degrades sharply outside settlements. For this vehicle the interesting roads are
  `highway=track` with `tracktype=grade1..5`, `surface=*`, `smoothness=*`, `ford=yes` — exactly the
  tags most often missing or guessed.
- Any consumer needs a tag-normalisation layer with per-`highway`-class defaults.

### The structural fact that drives everything

**OSM contains no topology and no routing graph.** Connectivity is implicit, from exactly one rule:

> Two ways are connected if and only if they share a node id.

Geometric crossing means nothing. A bridge over a road crosses visually but shares no node, so it is
not connected — `bridge`/`tunnel`/`layer` tags describe the situation, but the *absence of a shared
node* is what encodes it. Conversely, a mapping error where two roads share a node creates a
junction that does not exist on the ground.

Every routing engine therefore begins with the same preprocessing pass:

1. Stream all ways tagged `highway=*`.
2. Count references per node; nodes referenced by more than one way are junctions.
3. Split ways at junction nodes; each resulting segment becomes a graph edge.
4. Apply `oneway`, `access`/`vehicle`/`motor_vehicle`, `junction=roundabout`, `barrier` nodes.
5. Resolve `type=restriction` relations into a turn table.

### Distribution formats

| Format | Use |
|---|---|
| `.osm` XML | Canonical interchange, verbose, ~10x larger than needed. Not useful here. |
| `.osm.pbf` | Protobuf, delta-encoded, zlib blocks. The format actually distributed. Planet ~80 GB; Geofabrik regional extracts are hundreds of MB. |
| MVT vector tiles in MBTiles / **PMTiles** | Runtime display format. |
| Raster tiles in MBTiles | Runtime display format, style baked in. |
| Engine-specific graphs (OSRM, Valhalla tiles) | Runtime routing format. |

**Raw PBF is a preprocessing input only.** It carries no spatial index, so any query is a full scan —
nothing on the Pi should parse it at runtime.

## 2. Moving map

Feasible. Two paths.

### Pre-rendered raster tiles

MBTiles is a SQLite file of PNG/WebP blobs keyed by z/x/y. Trivially simple: query, upload as a GL
texture, draw quads. Drawbacks are material for an MFD-style display:

- Style is baked at render time — no runtime restyling to the monochrome/green MFD look unless the
  whole tile set is rendered locally.
- Labels are baked into the pixels, so they rotate with the map in track-up mode.
- Zoom quantised to integer levels.
- Larger storage than vector for the same coverage.

### Vector tiles (recommended)

A single PMTiles file; decode MVT protobuf geometry per tile, tessellate, draw with project shaders.
Better fit for this codebase:

- Full runtime style control (MFD palette, line weights, per-mode layer visibility).
- Smooth zoom, and track-up rotation with upright labels.
- Text rendering already exists in `src/graphics/context.rs` (freetype).

Cost: the renderer is hand-written — polygon triangulation, line extrusion with joins/caps for
variable road widths, and label placement with collision detection (the fiddliest part).

Relevant Rust crates: `pmtiles`, `geozero`/`prost` for MVT decode, `lyon` or `earcutr` for
tessellation. `maplibre-rs` is wgpu-based and immature; it would fight the raw GLES/DRM/KMS stack
rather than help.

### Obtaining tiles

- `pmtiles extract --bbox=...` against the Protomaps daily planet PMTiles pulls a regional file over
  HTTP range requests without downloading the planet.
- Or run **planetiler** (Java, desktop) over a Geofabrik extract to emit exactly the layers wanted.

A few oblasts at z0–14 lands in the low hundreds of MB.

### Performance on the Pi 4

The GPU side is trivial for this workload. The cost is CPU-side tile decode plus tessellation, which
belongs on a worker thread feeding an LRU cache of tessellated geometry keyed by tile. The cache owns
pre-allocated VBOs and streams via `glBufferData` — per the V3D rule in `CLAUDE.md`, never
`glGenBuffers`/`glDeleteBuffers` in the render path.

Projection: tiles are Web Mercator. A moving map typically renders track-up, so the view transform
needs a rotation about the vehicle position; keeping a local tangent-plane transform for the visible
extent avoids Mercator scale artefacts at display scale.

## 3. Offline route building

Feasible, but only against a derived graph — never against raw OSM. Two options.

### Integrate an existing engine

| Engine | Fit |
|---|---|
| **Valhalla** | Best technical fit: tiled graph (loads only what it needs → low RAM), designed for offline/mobile, generates maneuvers *and* instruction narrative. Large C++ codebase behind FFI; awkward to cross-compile. |
| **OSRM** | Faster (contraction hierarchies) but wants the whole graph resident. Same C++/FFI weight. |
| **GraphHopper** | JVM. Not viable on this platform. |

### Build in Rust (recommended)

Split into two binaries.

**Desktop preprocessing tool** — `osmpbf`/`osmpbfreader` to stream the extract; split ways at shared
nodes; assign edge costs from `highway` class + `maxspeed` (with per-class defaults when absent) +
`surface`/`tracktype`; encode oneway direction; build a turn table from `type=restriction`
relations; build an R-tree or geohash index for nearest-edge lookup; serialise to a compact
mmap-able binary. Preserve `name`, `ref`, `destination`, `destination:ref` on edges — the navigation
layer needs them and they cannot be recovered later.

**On-Pi runtime** — bidirectional A* with a great-circle heuristic. For a regional graph and routes
up to a few hundred km this is single-digit to tens of milliseconds on a Pi 4. Contraction
hierarchies (`fast_paths`) only become necessary for instant cross-country results.

### Correctness traps

- **Turn restrictions require an edge-based graph or an explicit turn table.** A node-based graph
  cannot express "no left turn from A to C via B" at all.
- `access`-family tags, `barrier` nodes, ferries (`route=ferry`).
- Roundabouts (`junction=roundabout`) need exit counting, not just geometry.

## 4. Real-time turn-by-turn navigation

Feasible, and the largest chunk of work. None of it comes from OSM for free.

| Component | Notes |
|---|---|
| **Map matching** | Snap noisy GNSS to the graph. Matching against a *known route* is far easier than against the whole graph: project onto the route polyline and maintain a distance-along-route cursor. The UM982 supplies position and heading, and `src/hardware/heading_fusion_sensor.rs` already fuses heading — heading disambiguates parallel candidate edges. |
| **Off-route detection / reroute** | Threshold on lateral offset combined with heading disagreement, then re-run A* from the matched position. |
| **Maneuver generation** | Walk the route; at each junction compare outgoing to incoming bearing and classify turn/slight/keep/sharp/u-turn. Roundabouts need exit counting from connected branches; motorway exits come from `destination`/`destination:ref`. Consistently underestimated. |
| **Instruction text** | Russian phrase templates written directly in MFD abbreviation style — easier than localising an existing engine's narrative output. Needs `name`/`ref` preserved on edges. |
| **Trigger distances** | Speed-dependent announcement lookahead. |

Integration shape: a new page in the existing page framework (`src/page_framework/`), consuming the
existing GNSS provider and heading fusion sensor.

## 5. Caveats before committing

- **Updates are manual.** Offline means periodic re-extract and re-preprocess, or applying `.osc`
  diffs. Workable for a vehicle, but needs a deliberate workflow.
- **No elevation data in OSM.** Grade/terrain requires SRTM or Copernicus DEM — a separate pipeline.
- **Offline geocoding is a third subsystem.** Nominatim is far too heavy. A name+POI index built with
  the `fst` crate during preprocessing covers "nearest fuel" and street-name lookup at a fraction of
  the cost.
- **Licence**: ODbL. Personal use is unconstrained; anything published requires
  "© OpenStreetMap contributors", and derived databases are share-alike.
- **Field data quality.** Away from settlements, expect missing `maxspeed` and `surface`, and tracks
  that exist on the ground but not in the data (or the reverse). For this vehicle's use case this is
  the most consequential caveat.

## 6. Recommended stack

Geofabrik or Protomaps extract → one desktop Rust preprocessing binary emitting two artefacts:

1. a **PMTiles vector-tile file** for display, and
2. a **custom binary routing graph** (edges + turn table + spatial index + name/ref strings).

On the Pi: a new map/nav page; MVT decode and tessellation on a worker thread behind an LRU tile
cache; a hand-written GLES renderer for the MFD style; bidirectional A* for routing; hand-written
Russian maneuver templates.

Rough effort split: moving map ~40%, routing ~20%, turn-by-turn navigation logic ~40%.
