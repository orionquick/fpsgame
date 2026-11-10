# fpsgame

An open-source first-person shooter set in downtown San Diego, built in Rust on wgpu. It targets macOS, Windows and Linux now, and Cairn OS later.

**Status: milestone 0.** The real street grid, building footprints and 2024 LiDAR heights are imported from public data, rendered top-down and at eye level, and walkable in a window. Materials are still flat placeholder colors.

## Map area

The ballpark and the blocks around it, inland (northeast) of the BNSF tracks: 4th Ave → 14th St, Imperial Ave → Market St. That's about 0.86 × 0.58 km, with a 120 m context margin imported around it. The box is `PLAY_BOX` in `crates/city/src/geo.rs`.

Coordinates are metres in a local frame with the origin at home plate: x = east, y = up, z = south (north is −z). Yaw 0 = north, 90 = east.

## Commands

```bash
cargo run --release -- import   # fetch OSM + USGS data (cached in data/raw/, ~180 MB of LiDAR), build data/city/downtown_sd.json
cargo run -- compare    # top-down render next to the NAIP aerial, same grid -> out/compare.png
cargo run -- ortho      # top-down render only -> out/ortho.png
cargo run -- shot --x -291.5 --z -60 --yaw 0 --pitch 6     # eye-height shot up 5th Ave -> out/shot.png
cargo run -- play --scene arena   # walk the demo arena: click for mouse look, WASD, Shift sprint, Space jump, F fly, Esc
cargo test --workspace  # editor ops, demo arena validity, collision, UTM conversion, LiDAR/footprint alignment
```

`compare` is the review image for the AI editor loop. Our render and the aerial photo share a 100 m labelled grid and the yellow play-area outline at 0.5 m per pixel, so differences can be pointed at by coordinate.

## Layout

| Crate | What it does |
|---|---|
| `crates/city` | Import (Overpass/OSM, USGS 3DEP LiDAR and elevation, NAIP aerial), the city model, and mesh generation: terrain, streets, sidewalks, rail, parks, extruded buildings, trees |
| `crates/render` | wgpu renderer: sun shadow map, forward pass, MSAA. Draws to a window or off-screen to PNG |
| `crates/editor` | Map editor core: scenes and their op log, gameplay validation, review overlays |
| `crates/app` | The `fps` binary: import / ortho / compare / shot / play, editor commands, the MCP server, player collision |

## Map editor (the AI loop)

A **scene** is gameplay content layered on the generated city: blockout objects, team spawns and a playable rectangle. Every edit is an **op**. The op log `data/scenes/NAME.ops.jsonl` is the source of truth (each line records who made it), and `NAME.json` is the replayed snapshot for readable diffs. Batches are atomic: if one op fails, nothing is written.

```bash
cargo run --release -- ops arena '[{"op":"line","kind":"barrier","from":[-125,-150],"to":[-110,-150]}]'
cargo run --release -- undo arena 1
cargo run --release -- validate arena   # JSON report
cargo run --release -- review arena     # out/arena_review.png
cargo run --release -- streets arena    # street names with centreline positions
```

- **Ops:** `place`, `line` (a row end to end, e.g. a barricade), `scatter` (seeded, avoids buildings and other objects), `move`, `delete`, `delete_rect`, `spawn`, `clear_spawns`, `bounds`.
- **Kinds:** barrier, car, bus, container, dumpster, crate, fence, box. These are blockout boxes until real assets arrive. Objects taller than 1.1 m block movement (the player's jump plus step clears anything lower), at least 1.6 m block sight, and at least 0.8 m count as cover.
- **Validation** runs on a 1 m grid over the bounds:
  - walkable area, reachability from spawns, and disconnected regions
  - cover within 3 m
  - the longest view from each spot, and the longest sightlines with their endpoints
  - spawn checks: spawns inside geometry, enemy spawns in view of each other, and walking distance to the centre for fairness
  - objects inside buildings, overlapping each other, or out of bounds
- **Review image:** on the left, the top-down render with object IDs, spawns and the longest sightlines; on the right, a heatmap of the longest view from each spot (green under 40 m, red over 150 m) with unreachable space in magenta.

**For Claude:** `.mcp.json` registers the `fps-editor` MCP server (`fps mcp`). Build first with `cargo build --release`, then restart Claude Code in this folder. It exposes these tools:

- `scene_status`, `streets`, `list_objects`
- `apply_ops`, `undo`
- `validate`, `render_review`, `render_shot`

Edit results come back with a validation summary, and renders come back as images.

**The demo scene `arena`** was built through this loop. It covers the two blocks north of the ballpark between J St and K St, from 7th Ave (team A) to North 10th Ave (team B):

- container and bus chicanes on J St and along the ballpark promenade
- cover in the plaza, plus parked cars, barriers and crates

Starting from the raw city, the views beyond 150 m went from 34% of spots to under 1%, cover from 23% to 51%, and the longest sightline from 361 m to 167 m (a plaza diagonal). Everything is reachable, the spawns are fair (217 m vs 191 m to the centre), and the validator reports no issues. A test keeps it that way.

## How the 3D comes from LiDAR

USGS 3DEP survey **CA_SanDiegoCo_D24** (flown 9–21 November 2024, published September 2026), four 1 km tiles, about 42 million points. `crates/city/src/lidar.rs` processes it.

- **Grids at 0.5 m:** bare ground (ground-class returns, holes filled), hard surfaces and canopy. The survey only classifies ground, water and noise, so the rest is split by return count: a pulse that hits a roof comes back once, one that hits a tree canopy usually several times.
- **Ground:** 2 m terrain from the LiDAR ground returns. Streets follow it, crown included.
- **Buildings:** each OSM footprint takes the median roof height measured inside it. Parts rising at least 4 m above that, covering at least 40 m², become stacked tiers traced from the grid (towers on podiums, the stepped ballpark bowl). Structures with no OSM footprint (newer buildings and canopies) are added from the grid.
- **Trees:** canopy peaks at least 6 m apart and 2 m clear of buildings, with a crown about 2.5 m across. Peaks over a roadway move to the curb.

Current import: 316 OSM buildings measured, 121 upper tiers, 132 structures OSM lacks, 2,279 trees. Only 5 buildings keep a guessed height.

## Hand-built buildings (one at a time)

The generic kit-of-parts assembler is being replaced building by building, starting with the blocks around the
ballpark. Each one is matched to street-level reference (looked at on screen only, nothing saved) and built in
`crates/app/src/bespoke/`:

- `geom.rs`: a mesh kit in a building-local frame: thick walls with real openings and reveals, boxes, sloped
  parapet pieces, planar metre UVs, and per-wall collision prisms.
- `plan.rs`: office floor plans from the window bays: perimeter offices (one per one or two bays), a corridor ring
  and cross corridor, central rooms, a lift lobby, and a core (switchback stair, lift, riser, restrooms) against the
  blind party wall.
- `mod.rs`: interior emitter (partitions with doors and open leaves, floor finishes by room use, dropped ceilings
  with light panels, slabs with stair and shaft holes, stairs, lift doors) and batching.
- `factory.rs`: the first one, the three-storey red-brick former factory facing the outfield park (OSM
  way/376480375): exact bay rhythm, multi-light steel sashes, glazed garage doors and storefronts, cast-stone pier
  caps and sills, the raised parapet with its sign panel and plaques, roof monitor; enterable on every floor.

The renderer draws glazing in a separate transparent pass (no shadow, so sun reaches interiors), emissive panels,
and an indoor lighting mode (dimmer sky light plus lamp light).

## Megascans prefab city

Everything visible in the city is built from Quixel Megascans assets downloaded from Fab: about 720 models and surfaces, kept out of git in `data/megascans_inbox/` because Megascans can't be redistributed.

- **Compiling the assets:** `cargo run --release -- assets` builds the catalog (`data/assets/catalog.json`) and the meshes (`data/assets/mesh/*.bin`).
  - Every glTF is converted to metres, Y-up, with simplified detail levels (~25% and ~6% of the triangles) made with meshoptimizer.
  - Textures load from the zips on demand.
- **Buildings, assembled from prefabs** (`crates/app/src/buildings.rs`): the modular kits are placed along each real footprint, floor by floor, with glass behind every window opening.
  - **Pieces:** storefront ground floors, window floors, balconies, per-floor corners, cornices, parapets and flat-roof tiles.
  - **Styles by height:** townhouses, mid-rises, and towers built from the Urban Modular panels.
  - **Fit:** piece widths and storey heights are scaled slightly so pieces fill each wall exactly.
  - **Collision only:** the extruded LiDAR volumes are no longer drawn and remain only for collision.
- **Streets:** cracked asphalt with Quixel decal road markings (double yellow, dashed lanes, crosswalks), plus continuous curbs.
  - **Sidewalk furniture:** parking meters, hydrants, bins, bollards, benches, signs.
  - **Palms:** at every LiDAR tree, built from Quixel palm trunks and fan-palm crowns.
  - **Editor objects:** these render as Quixel models: rusted car wrecks, shipping containers, concrete barriers, crates.
- **Rendering:**
  - textured PBR with alpha cutout for foliage and decals
  - a sky with sun disc, aerial haze
  - two shadow cascades
  - ACES tonemapping

## Data sources and attribution

- **Streets, footprints, trees, parks:** © OpenStreetMap contributors, under the [ODbL](https://opendatacommons.org/licenses/odbl/). `data/city/downtown_sd.json` is a derived database, so it is distributed under the ODbL.
- **LiDAR, ground and building heights:** USGS 3D Elevation Program, CA_SanDiegoCo_D24 (public domain). Elevation service as a fallback.
- **Aerial reference (not shipped):** USDA NAIP via USGS, 0.3 m (public domain). Cached in `data/raw/` for review only.
- No Google Maps, Street View or Photorealistic 3D Tiles data. Their terms forbid this use.
- No real names or logos on the ballpark or businesses.

## Known gaps (next up)

- **Kit look:** the modular kits are European, scanned in Gothenburg, so facades read as historic downtown rather than as San Diego's actual buildings, and the towers reuse the same kit families.
- **Trees** are low-poly placeholders scaled to measured height, and a few detections may be awnings or clutter. They don't collide yet.
- **The ballpark** is LiDAR tiers. It still needs a hand-built landmark model.
- **Street detail:** no curbs, lane markings or crosswalks, bridges sit at ground level, and street widths are heuristics (OSM lane counts with a 16 m downtown minimum).
- **Rendering:** no TSR, bloom or ambient occlusion pass yet. Ruinbound's post chain comes later. Play mode is heavy with this many prefabs, and needs streaming and per-frame level of detail.
- **Collision:** buildings and objects are vertical prisms, roofs are walkable and trees don't collide. Rapier comes with real meshes.
- **Editor:** no in-game editor UI yet (CLI and MCP only), no choke-point or flank metrics yet, and objects are boxes.

## License

Code: MIT OR Apache-2.0. Map data: ODbL (see above).
