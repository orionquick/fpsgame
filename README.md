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
cargo run -- play       # walk it: click for mouse look, WASD, Shift sprint, Space jump, F fly, Esc
cargo test --workspace  # collision tests along 5th Ave, UTM conversion, LiDAR/footprint alignment
```

`compare` is the review image for the AI editor loop. Our render and the aerial photo share a 100 m labelled grid and the yellow play-area outline at 0.5 m per pixel, so differences can be pointed at by coordinate.

## Layout

| Crate | What it does |
|---|---|
| `crates/city` | Import (Overpass/OSM, USGS 3DEP LiDAR and elevation, NAIP aerial), the city model, and mesh generation: terrain, streets, sidewalks, rail, parks, extruded buildings, trees |
| `crates/render` | wgpu renderer: sun shadow map, forward pass, MSAA. Draws to a window or off-screen to PNG |
| `crates/app` | The `fps` binary: import / ortho / compare / shot / play, player collision |

## How the 3D comes from LiDAR

USGS 3DEP survey **CA_SanDiegoCo_D24** (flown 9–21 November 2024, published September 2026), four 1 km tiles, about 42 million points. `crates/city/src/lidar.rs` processes it.

- **Grids at 0.5 m:** bare ground (ground-class returns, holes filled), hard surfaces and canopy. The survey only classifies ground, water and noise, so the rest is split by return count: a pulse that hits a roof comes back once, one that hits a tree canopy usually several times.
- **Ground:** 2 m terrain from the LiDAR ground returns. Streets follow it, crown included.
- **Buildings:** each OSM footprint takes the median roof height measured inside it. Parts rising at least 4 m above that, covering at least 40 m², become stacked tiers traced from the grid (towers on podiums, the stepped ballpark bowl). Structures with no OSM footprint (newer buildings and canopies) are added from the grid.
- **Trees:** canopy peaks at least 6 m apart and 2 m clear of buildings, with a crown about 2.5 m across. Peaks over a roadway move to the curb.

Current import: 316 OSM buildings measured, 121 upper tiers, 132 structures OSM lacks, 2,279 trees. Only 5 buildings keep a guessed height.

## Data sources and attribution

- **Streets, footprints, trees, parks:** © OpenStreetMap contributors, under the [ODbL](https://opendatacommons.org/licenses/odbl/). `data/city/downtown_sd.json` is a derived database, so it is distributed under the ODbL.
- **LiDAR, ground and building heights:** USGS 3D Elevation Program, CA_SanDiegoCo_D24 (public domain). Elevation service as a fallback.
- **Aerial reference (not shipped):** USDA NAIP via USGS, 0.3 m (public domain). Cached in `data/raw/` for review only.
- No Google Maps, Street View or Photorealistic 3D Tiles data. Their terms forbid this use.
- No real names or logos on the ballpark or businesses.

## Known gaps (next up)

- **Massing, not facades:** buildings are stacked prisms with flat roofs at the median measured height, so pitched roofs, rooftop equipment and overhangs are approximated.
- **Trees** are low-poly placeholders scaled to measured height, and a few detections may be awnings or clutter. They don't collide yet.
- **The ballpark** is LiDAR tiers. It still needs a hand-built landmark model.
- **Street detail:** no curbs, lane markings or crosswalks, bridges sit at ground level, and street widths are heuristics (OSM lane counts with a 16 m downtown minimum).
- **Rendering:** placeholder flat-shaded materials. Ruinbound's post chain (TSR, exposure, bloom, tonemap) and PBR Megascans materials come in M1/M2.
- **Collision:** buildings are vertical prisms, roofs are walkable and trees don't collide. Rapier comes with real meshes.

## License

Code: MIT OR Apache-2.0. Map data: ODbL (see above).
