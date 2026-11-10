use crate::geo::{Frame, LatLonBox, HOME_PLATE, MARGIN_M, PLAY_BOX};
use crate::model::*;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const UA: &str = "fpsgame-city-import/0.1";
const OVERPASS: [&str; 3] = [
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
];
const DEM_URL: &str = "https://elevation.nationalmap.gov/arcgis/rest/services/3DEPElevation/ImageServer/exportImage";
const NAIP_URL: &str = "https://imagery.nationalmap.gov/arcgis/rest/services/USGSNAIPImagery/ImageServer/exportImage";

/// Metres per pixel of the cached aerial and DEM rasters.
pub const NAIP_MPP: f64 = 0.5;
const DEM_MPP: f64 = 2.0;

fn cached(path: &Path, fetch: impl FnOnce() -> Res<Vec<u8>>) -> Res<Vec<u8>> {
    if let Ok(b) = std::fs::read(path) {
        return Ok(b);
    }
    let b = fetch()?;
    std::fs::write(path, &b)?;
    Ok(b)
}

/// GET with retries: USGS servers answer 429/502 under load, and sometimes send a tiny HTML page instead.
fn get(url: &str) -> Res<Vec<u8>> {
    let mut last = String::new();
    for attempt in 0..6u64 {
        match ureq::get(url).header("User-Agent", UA).call() {
            Ok(mut r) => {
                let b = r.body_mut().with_config().limit(512 << 20).read_to_vec()?;
                if b.len() > 2048 || !b.starts_with(b"<") {
                    return Ok(b);
                }
                last = format!("{}-byte HTML page", b.len());
            }
            Err(e) => last = e.to_string(),
        }
        eprintln!("  retrying ({last})...");
        std::thread::sleep(std::time::Duration::from_secs(5 * (attempt + 1)));
    }
    Err(format!("{url}: {last}").into())
}

/// Cache key for raw downloads, so changing the play box never reuses data fetched for another area.
fn box_key(b: &LatLonBox) -> String {
    format!("{:.4}_{:.4}_{:.4}_{:.4}", b.w, b.s, b.e, b.n).replace('-', "m")
}

/// Cached aerial photo for an import box.
pub fn naip_name(b: &LatLonBox) -> String {
    format!("naip_{}.png", box_key(b))
}

fn raster_size(b: &LatLonBox, f: &Frame, mpp: f64) -> (usize, usize) {
    let r = b.xz_rect(f);
    (((r[2] - r[0]) / mpp).round() as usize, ((r[3] - r[1]) / mpp).round() as usize)
}

fn export_url(base: &str, b: &LatLonBox, w: usize, h: usize, extra: &str) -> String {
    format!("{base}?bbox={},{},{},{}&bboxSR=4326&imageSR=4326&size={w},{h}{extra}&f=image", b.w, b.s, b.e, b.n)
}

/// Fetches (or reuses cached) OSM, elevation and aerial data under `raw`, and builds the city model.
pub fn import(raw: &Path) -> Res<City> {
    std::fs::create_dir_all(raw)?;
    let frame = Frame::new(HOME_PLATE.0, HOME_PLATE.1);
    let ib = PLAY_BOX.grow_m(&frame, MARGIN_M);
    let key = box_key(&ib);

    let q = format!(
        "[out:json][timeout:180];(\
         way[\"building\"]({s},{w},{n},{e});\
         relation[\"building\"][\"type\"=\"multipolygon\"]({s},{w},{n},{e});\
         way[\"highway\"]({s},{w},{n},{e});\
         way[\"railway\"~\"^(rail|light_rail|tram)$\"]({s},{w},{n},{e});\
         node[\"natural\"=\"tree\"]({s},{w},{n},{e});\
         way[\"leisure\"~\"^(pitch|park|garden|playground|dog_park)$\"]({s},{w},{n},{e});\
         way[\"landuse\"~\"^(grass|recreation_ground)$\"]({s},{w},{n},{e});\
         way[\"amenity\"=\"parking\"][\"parking\"!~\"^(multi-storey|underground)$\"]({s},{w},{n},{e});\
         way[\"place\"=\"square\"]({s},{w},{n},{e});\
         way[\"highway\"=\"pedestrian\"][\"area\"=\"yes\"]({s},{w},{n},{e});\
         );out geom;",
        s = ib.s, w = ib.w, n = ib.n, e = ib.e
    );
    let osm = cached(&raw.join(format!("osm_{key}.json")), || {
        let mut last = String::new();
        for attempt in 0..6 {
            let url = OVERPASS[attempt % OVERPASS.len()];
            eprintln!("fetching OSM from {url}...");
            match ureq::post(url).header("User-Agent", UA).send_form([("data", q.as_str())]) {
                Ok(mut r) => return Ok(r.body_mut().with_config().limit(512 << 20).read_to_vec()?),
                Err(e) => last = e.to_string(),
            }
            std::thread::sleep(std::time::Duration::from_secs(3 + attempt as u64 * 2));
        }
        Err(format!("Overpass failed: {last}").into())
    })?;

    let (dw, dh) = raster_size(&ib, &frame, DEM_MPP);
    // fallback ground only: the lidar replaces it below
    let dem = cached(&raw.join(format!("dem_{key}.tif")), || {
        eprintln!("fetching 3DEP elevation {dw}x{dh}...");
        get(&export_url(DEM_URL, &ib, dw, dh, "&format=tiff&pixelType=F32&interpolation=RSP_BilinearInterpolation"))
    })
    .unwrap_or_else(|e| {
        eprintln!("  elevation service unavailable ({e}); using lidar ground only");
        vec![]
    });

    let (nw, nh) = raster_size(&ib, &frame, NAIP_MPP);
    cached(&raw.join(naip_name(&ib)), || {
        eprintln!("fetching NAIP aerial {nw}x{nh}...");
        get(&export_url(NAIP_URL, &ib, nw, nh, "&format=png"))
    })?;

    let rect = ib.xz_rect(&frame);
    let mut terrain = parse_dem(&dem, rect).unwrap_or_else(|_| Terrain::flat(rect));
    let origin_elev = terrain.at(0.0, 0.0);
    terrain.y.iter_mut().for_each(|v| *v -= origin_elev as f32);

    let mut city = City {
        frame,
        play_box: PLAY_BOX,
        import_box: ib,
        origin_elev,
        terrain,
        buildings: vec![],
        lines: vec![],
        areas: vec![],
        trees: vec![],
        tree_h: vec![],
        attribution: "Map data © OpenStreetMap contributors (ODbL). Elevation: USGS 3DEP. Imagery: USDA NAIP via USGS.".into(),
    };
    parse_osm(&serde_json::from_slice(&osm)?, &mut city);

    let ldir = raw.join("lidar");
    std::fs::create_dir_all(&ldir)?;
    for (t, p) in crate::lidar::TILES.iter().zip(crate::lidar::tile_paths(&ldir)) {
        cached(&p, || {
            eprintln!("fetching lidar tile {t} (USGS is slow, ~1 min per 10 MB)...");
            get(&crate::lidar::tile_url(t))
        })?;
    }
    eprintln!("rasterising lidar at {} m...", crate::lidar::RES);
    let g = crate::lidar::rasterize(&crate::lidar::tile_paths(&ldir), &frame, rect)?;
    let k = g.cell(0.0, 0.0).ok_or("home plate outside the lidar grid")?;
    city.origin_elev = g.ground[k] as f64;
    city.terrain = crate::lidar::terrain(&g, 2.0, city.origin_elev);
    let st = crate::lidar::apply(&g, &mut city.buildings, city.origin_elev);
    let lt = crate::lidar::trees(&g, &city.buildings);
    eprintln!("lidar: {} tree crowns (OSM had {})", lt.len(), city.trees.len());
    city.trees = lt.iter().map(|t| snap_off_roads(t.0, &city.lines)).collect();
    city.tree_h = lt.iter().map(|t| t.1).collect();
    eprintln!("lidar: {} buildings measured, {} kept OSM height (no returns), {} upper tiers, {} structures missing from OSM", st.lidar, st.kept_osm, st.parts, st.added);
    city.attribution = "Map data © OpenStreetMap contributors (ODbL). Elevation and building heights: USGS 3DEP lidar CA_SanDiegoCo_D24 (2024). Imagery: USDA NAIP via USGS.".into();
    Ok(city)
}

/// Crown peaks can overhang the street; moves a tree that lands on a roadway to just past the nearest curb.
fn snap_off_roads(p: [f64; 2], lines: &[Line]) -> [f64; 2] {
    let mut best: Option<(f64, [f64; 2], f64)> = None;
    for l in lines.iter().filter(|l| matches!(l.kind, LineKind::Road | LineKind::Service)) {
        for w in l.pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let l2 = dx * dx + dz * dz;
            if l2 < 1e-6 {
                continue;
            }
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0);
            let q = [a[0] + dx * t, a[1] + dz * t];
            let d = (p[0] - q[0]).hypot(p[1] - q[1]);
            if d < l.width * 0.5 && best.is_none_or(|b| d < b.0) {
                best = Some((d, q, l.width * 0.5));
            }
        }
    }
    let Some((d, q, half)) = best else { return p };
    let (nx, nz) = if d > 1e-3 { ((p[0] - q[0]) / d, (p[1] - q[1]) / d) } else { (1.0, 0.0) };
    [q[0] + nx * (half + 1.0), q[1] + nz * (half + 1.0)]
}

fn parse_dem(b: &[u8], rect: [f64; 4]) -> Res<Terrain> {
    let mut d = tiff::decoder::Decoder::new(std::io::Cursor::new(b))?;
    let (w, h) = d.dimensions()?;
    let y = match d.read_image()? {
        tiff::decoder::DecodingResult::F32(v) => v,
        _ => return Err("unexpected DEM pixel type".into()),
    };
    let good: Vec<f32> = y.iter().copied().filter(|v| v.is_finite() && *v > -100.0).collect();
    if good.is_empty() {
        return Ok(Terrain::flat(rect));
    }
    let fill = good.iter().sum::<f32>() / good.len() as f32;
    let y = y.into_iter().map(|v| if v.is_finite() && v > -100.0 { v } else { fill }).collect();
    Ok(Terrain { rect, w: w as usize, h: h as usize, y })
}

fn tags(e: &Value) -> HashMap<String, String> {
    e["tags"].as_object().map_or_else(HashMap::new, |m| {
        m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect()
    })
}

fn geom(f: &Frame, g: &Value) -> Vec<[f64; 2]> {
    g.as_array().map_or_else(Vec::new, |a| {
        a.iter().filter_map(|p| Some(f.to_xz(p["lat"].as_f64()?, p["lon"].as_f64()?))).collect()
    })
}

/// "12", "12 m", "40'", "40 ft" → metres.
fn parse_len(s: &str) -> Option<f64> {
    let t = s.trim().to_lowercase();
    let feet = t.ends_with('\'') || t.ends_with("ft") || t.contains("feet");
    let num: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let v: f64 = num.parse().ok()?;
    Some(if feet { v * 0.3048 } else { v })
}

fn closed(r: &[[f64; 2]]) -> bool {
    r.len() >= 4 && r[0] == r[r.len() - 1]
}

fn open_ring(mut r: Ring) -> Ring {
    if r.len() > 1 && r[0] == r[r.len() - 1] {
        r.pop();
    }
    r
}

/// Joins multipolygon member ways into closed rings.
fn assemble(mut parts: Vec<Vec<[f64; 2]>>) -> Vec<Ring> {
    let mut rings = vec![];
    while let Some(mut cur) = parts.pop() {
        while !closed(&cur) {
            let end = *cur.last().unwrap();
            let Some(i) = parts.iter().position(|p| p[0] == end || *p.last().unwrap() == end) else { break };
            let mut p = parts.swap_remove(i);
            if p[0] != end {
                p.reverse();
            }
            cur.extend_from_slice(&p[1..]);
        }
        if closed(&cur) {
            rings.push(open_ring(cur));
        }
    }
    rings
}

fn building_height(t: &HashMap<String, String>) -> (f64, f64, &'static str) {
    let min_h = t.get("min_height").and_then(|s| parse_len(s)).unwrap_or(0.0);
    if let Some(h) = t.get("height").and_then(|s| parse_len(s)) {
        return (h, min_h, "height");
    }
    if let Some(l) = t.get("building:levels").and_then(|s| s.trim().parse::<f64>().ok()) {
        let roof = t.get("roof:levels").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        return (1.0 + (l + roof) * 3.6, min_h, "levels");
    }
    let h = match t.get("building").map(String::as_str).unwrap_or("yes") {
        "house" | "shed" | "kiosk" | "roof" | "hut" => 5.0,
        "garage" | "garages" => 4.0,
        "stadium" => 30.0,
        "train_station" | "transportation" => 9.0,
        _ => 12.0,
    };
    (h, min_h, "default")
}

fn road_width(hw: &str, t: &HashMap<String, String>) -> Option<(LineKind, f64)> {
    if let Some(w) = t.get("width").and_then(|s| parse_len(s)) {
        let kind = match hw {
            "footway" | "path" | "pedestrian" | "steps" | "cycleway" | "corridor" => LineKind::Path,
            "service" => LineKind::Service,
            _ => LineKind::Road,
        };
        return Some((kind, w));
    }
    let lanes = t.get("lanes").and_then(|s| s.parse::<f64>().ok());
    Some(match hw {
        "motorway" | "trunk" => (LineKind::Road, lanes.unwrap_or(3.0) * 3.6 + 2.0),
        "motorway_link" | "trunk_link" => (LineKind::Road, lanes.unwrap_or(1.0) * 3.6 + 2.0),
        // downtown streets: travel lanes plus curbside parking/turn lanes; OSM lane counts miss those, so floor at the measured ~16 m curb-to-curb
        "primary" | "secondary" | "tertiary" => (LineKind::Road, (lanes.unwrap_or(3.0) * 3.3 + 5.0).max(16.0)),
        "primary_link" | "secondary_link" | "tertiary_link" => (LineKind::Road, lanes.unwrap_or(1.0) * 3.3 + 1.0),
        "residential" | "unclassified" | "living_street" => (LineKind::Road, (lanes.unwrap_or(2.0) * 3.2 + 5.0).max(14.0)),
        "service" => (LineKind::Service, if t.get("service").is_some_and(|s| s == "alley") { 5.0 } else { 4.5 }),
        "footway" => (LineKind::Path, if t.get("footway").is_some_and(|s| s == "sidewalk") { 3.0 } else { 2.5 }),
        "pedestrian" => (LineKind::Path, 6.0),
        "steps" | "cycleway" | "path" => (LineKind::Path, 2.0),
        _ => return None,
    })
}

fn parse_osm(v: &Value, c: &mut City) {
    let f = c.frame;
    let empty = vec![];
    for e in v["elements"].as_array().unwrap_or(&empty) {
        let t = tags(e);
        let typ = e["type"].as_str().unwrap_or("");
        let id = format!("{typ}/{}", e["id"]);
        let name = t.get("name").cloned();
        if typ == "node" {
            if t.get("natural").is_some_and(|s| s == "tree") {
                if let (Some(la), Some(lo)) = (e["lat"].as_f64(), e["lon"].as_f64()) {
                    c.trees.push(f.to_xz(la, lo));
                }
            }
            continue;
        }
        if t.contains_key("building") && t.get("building").is_some_and(|b| b != "no") {
            let (height, min_height, src) = building_height(&t);
            let kind = t["building"].clone();
            let mut push = |outer: Ring, holes: Vec<Ring>| {
                if outer.len() >= 3 {
                    c.buildings.push(Building { osm: id.clone(), outer, holes, height, min_height, height_src: src.into(), kind: kind.clone(), name: name.clone(), base_y: None, part_of: None });
                }
            };
            if typ == "way" {
                let g = geom(&f, &e["geometry"]);
                if closed(&g) {
                    push(open_ring(g), vec![]);
                }
            } else if typ == "relation" {
                let mem = e["members"].as_array().unwrap_or(&empty);
                let role = |r: &str| mem.iter().filter(|m| m["role"] == r && m["type"] == "way").map(|m| geom(&f, &m["geometry"])).filter(|g| g.len() >= 2).collect::<Vec<_>>();
                let holes = assemble(role("inner"));
                for o in assemble(role("outer")) {
                    push(o, holes.clone());
                }
            }
            continue;
        }
        if typ != "way" {
            continue;
        }
        let g = geom(&f, &e["geometry"]);
        if g.len() < 2 {
            continue;
        }
        let area_yes = t.get("area").is_some_and(|s| s == "yes");
        let area_kind = if t.get("leisure").is_some_and(|s| s == "pitch") {
            Some(AreaKind::Pitch)
        } else if t.contains_key("leisure") || t.contains_key("landuse") {
            Some(AreaKind::Grass)
        } else if t.get("amenity").is_some_and(|s| s == "parking") {
            Some(AreaKind::Parking)
        } else if t.get("place").is_some_and(|s| s == "square") || (t.get("highway").is_some_and(|s| s == "pedestrian") && area_yes) {
            Some(AreaKind::Plaza)
        } else {
            None
        };
        if let Some(kind) = area_kind {
            if closed(&g) {
                c.areas.push(Area { kind, outer: open_ring(g) });
            }
            continue;
        }
        if let Some(r) = t.get("railway") {
            c.lines.push(Line { kind: LineKind::Rail, tag: r.clone(), width: 3.2, pts: g, name });
            continue;
        }
        if let Some(hw) = t.get("highway") {
            if t.get("tunnel").is_some_and(|s| s != "no") || t.get("indoor").is_some_and(|s| s == "yes") || t.get("level").is_some_and(|s| s.starts_with('-')) {
                continue;
            }
            if let Some((kind, width)) = road_width(hw, &t) {
                c.lines.push(Line { kind, tag: hw.clone(), width, pts: g, name });
            }
        }
    }
}
