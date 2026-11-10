# fpsgame

An open-source first-person shooter set in downtown San Diego, built in Rust on wgpu. It targets macOS, Windows and Linux now, and Cairn OS later.

**Status: milestone 0.** The real street grid and building footprints are imported from public data, rendered top-down and at eye level, and walkable in a window. Everything is still grey boxes.

## Map area

The ballpark plus everything inland (northeast) of the BNSF tracks: 4th Ave → 14th St, Imperial Ave → Broadway. That's about 0.95 × 1.1 km, with a 120 m context margin imported around it.

Coordinates are metres in a local frame with the origin at home plate: x = east, y = up, z = south (north is −z). Yaw 0 = north, 90 = east.

## Commands

```bash
cargo run -- import     # fetch OSM + USGS data (cached in data/raw/), build data/city/downtown_sd.json
cargo run -- compare    # top-down render next to the NAIP aerial, same grid -> out/compare.png
cargo run -- ortho      # top-down render only -> out/ortho.png
cargo run -- shot --x -291.5 --z -480 --yaw 0 --pitch 5    # eye-height shot up 5th Ave -> out/shot.png
cargo run -- play       # walk it: click for mouse look, WASD, Shift sprint, Space jump, F fly, Esc
cargo test              # headless collision tests along 5th Ave
```

`compare` is the review image for the AI editor loop. Our render and the aerial photo share a 100 m labelled grid and the yellow play-area outline at 0.5 m per pixel, so differences can be pointed at by coordinate.

## Layout

| Crate | What it does |
|---|---|
| `crates/city` | Import (Overpass/OSM, USGS 3DEP elevation, NAIP aerial), the city model, and mesh generation: terrain, streets, sidewalks, rail, parks, extruded buildings, trees |
| `crates/render` | wgpu renderer: sun shadow map, forward pass, MSAA. Draws to a window or off-screen to PNG |
| `crates/app` | The `fps` binary: import / ortho / compare / shot / play, player collision |

## Data sources and attribution

- **Streets, footprints, trees, parks:** © OpenStreetMap contributors, under the [ODbL](https://opendatacommons.org/licenses/odbl/). `data/city/downtown_sd.json` is a derived database, so it is distributed under the ODbL.
- **Bare-earth elevation:** USGS 3D Elevation Program (public domain).
- **Aerial reference (not shipped):** USDA NAIP via USGS, 0.3 m (public domain). Cached in `data/raw/` for review only.
- No Google Maps, Street View or Photorealistic 3D Tiles data. Their terms forbid this use.
- No real names or logos on the ballpark or businesses.

## Known gaps (next up)

- **Building heights:** 394 of 562 buildings use a 12 m default because OSM has no height or floor count for them. Next: LiDAR (the only 3DEP coverage is a 2014 survey, so post-2014 towers need OSM floor counts, other datasets or manual values).
- **The ballpark** is a single extruded block. It needs a hand-built landmark model.
- **Street detail:** no curbs, lane markings or crosswalks, bridges sit at ground level, and street widths are heuristics (OSM lane counts with a 16 m downtown minimum).
- **Rendering:** placeholder flat-shaded materials. Ruinbound's post chain (TSR, exposure, bloom, tonemap) and PBR Megascans materials come in M1/M2.
- **Collision:** buildings are vertical prisms, roofs are walkable and trees don't collide. Rapier comes with real meshes.

## License

Code: MIT OR Apache-2.0. Map data: ODbL (see above).
