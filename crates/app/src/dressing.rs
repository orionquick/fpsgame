//! Procedural street and facade dressing with Megascans prefabs: curbs, sidewalk furniture, and modular
//! window facades on street-facing walls. Deterministic, limited to an area (the scene bounds plus a margin).
use crate::library::{extent, Library, Placement};
use city::model::{City, LineKind};

fn hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

fn in_rect(p: [f64; 2], r: [f64; 4]) -> bool {
    p[0] >= r[0] && p[0] <= r[2] && p[1] >= r[1] && p[1] <= r[3]
}

struct Road {
    a: [f64; 2],
    b: [f64; 2],
    half: f64,
    line: usize,
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 < 1e-9 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dz * t)
}

fn point_in(r: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

pub fn dress(lib: &Library, c: &City, area: [f64; 4]) -> Vec<Placement> {
    let mut out = vec![];
    let grow = [area[0] - 30.0, area[1] - 30.0, area[2] + 30.0, area[3] + 30.0];
    let roads: Vec<Road> = c
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l.kind, LineKind::Road | LineKind::Service))
        .flat_map(|(i, l)| l.pts.windows(2).map(move |w| Road { a: w[0], b: w[1], half: l.width * 0.5, line: i }))
        .filter(|r| in_rect(r.a, grow) || in_rect(r.b, grow))
        .collect();
    let near_buildings: Vec<&city::model::Building> = c.buildings.iter().filter(|b| b.outer.iter().any(|p| in_rect(*p, grow))).collect();
    let in_building = |p: [f64; 2]| near_buildings.iter().any(|b| point_in(&b.outer, p));
    let on_other_road = |p: [f64; 2], line: usize, margin: f64| roads.iter().any(|r| r.line != line && seg_dist(p, r.a, r.b) < r.half + margin);

    let pick = |title: &str, k: u64| -> Option<&assets::Asset> {
        let v: Vec<&assets::Asset> = lib.catalog.assets.iter().filter(|a| a.title == title && !a.parts.is_empty()).collect();
        (!v.is_empty()).then(|| v[(k % v.len() as u64) as usize])
    };

    // plantings: boxwood and lady palms scattered on lawns and plazas
    let shrubs: Vec<&assets::Asset> = ["Boxwood", "Broadleaf Lady Palm"].iter().filter_map(|t| pick(t, 0)).collect();
    if !shrubs.is_empty() {
        for (ai, a) in c.areas.iter().enumerate() {
            if !matches!(a.kind, city::model::AreaKind::Grass | city::model::AreaKind::Plaza) || !a.outer.iter().any(|p| in_rect(*p, area)) {
                continue;
            }
            let mn = a.outer.iter().fold([f64::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
            let mx = a.outer.iter().fold([f64::MIN; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
            let step = if a.kind == city::model::AreaKind::Grass { 4.0 } else { 9.0 };
            let mut x = mn[0] + step * 0.5;
            while x < mx[0] {
                let mut z = mn[1] + step * 0.5;
                while z < mx[1] {
                    let h = hash(hash((x * 10.0) as i64 as u64, (z * 10.0) as i64 as u64), ai as u64);
                    let (jx, jz) = (((h % 1000) as f64 / 1000.0 - 0.5) * step * 0.8, (((h >> 10) % 1000) as f64 / 1000.0 - 0.5) * step * 0.8);
                    let p = [x + jx, z + jz];
                    if h % 100 < 45 && point_in(&a.outer, p) && !in_building(p) {
                        let s = shrubs[((h >> 20) % shrubs.len() as u64) as usize];
                        let sc = 0.8 + ((h >> 30) % 60) as f32 / 100.0;
                        out.push(Placement { asset: s.id.clone(), part: crate::library::ALL_PARTS, x: p[0], z: p[1], y: c.terrain.at(p[0], p[1]), rot: ((h >> 40) % 360) as f64, scale: [sc, sc, sc], tint: [1.0, 1.0, 1.0], lod: 0, indoor: false });
                    }
                    z += step;
                }
                x += step;
            }
        }
    }

    // curbs and sidewalk furniture along both edges of every road segment
    let curb = pick("Modular Curb Kit", 0);
    // US street furniture from Sketchfab (City Props Collection vol. 1/2 by TampaJoey, newspaper boxes by Keith Ito,
    // hydrant by Fayruso, trash can by Lyskilde): (asset ids, percent of 6 m slots)
    let furniture: [(&[&str], u64); 8] = [
        (&["sf_32b8b82bb1_15"], 20),
        (&["sf_32b8b82bb1_03", "sf_32b8b82bb1_16", "sf_2f52ea53c5_26", "sf_f61aeec14f"], 7),
        (&["sf_32b8b82bb1_14", "sf_d021f42ea8"], 4),
        (&["sf_8975781045"], 3),
        (&["sf_2f52ea53c5_25"], 3),
        (&["sf_32b8b82bb1_06", "sf_2f52ea53c5_02"], 2),
        (&["sf_32b8b82bb1_23"], 1),
        (&["sf_2f52ea53c5_10"], 1),
    ];
    let have = |id: &str| lib.catalog.get(id).is_some();
    for r in &roads {
        let (dx, dz) = (r.b[0] - r.a[0], r.b[1] - r.a[1]);
        let len = dx.hypot(dz);
        if len < 1.0 {
            continue;
        }
        let (ux, uz) = (dx / len, dz / len);
        let heading = ux.atan2(-uz).to_degrees();
        for side in [-1.0f64, 1.0] {
            let (nx, nz) = (-uz * side, ux * side);
            if let Some(cb) = curb {
                // the kit's longest part is the straight run; the others are ends and corners
                let long = |i: usize| { let p = &cb.parts[i]; (p.max[0] - p.min[0]).max(p.max[2] - p.min[2]) };
                // the kit's 0.7 m straight run (part 0); its other parts are ends and corners
                let straight = 0usize;
                let sp = &cb.parts[straight];
                // run the piece's long axis along the road (some pieces are modelled along Z)
                let turn = if sp.max[2] - sp.min[2] > sp.max[0] - sp.min[0] { 90.0 } else { 0.0 };
                // bevelled ends: overlap pieces so the run reads as one curb
                let step = (long(straight) as f64 * 0.82).max(0.3);
                let n = (len / step).floor() as usize;
                for k in 0..n {
                    let t = (k as f64 + 0.5) * step;
                    let p = [r.a[0] + ux * t + nx * (r.half + 0.12), r.a[1] + uz * t + nz * (r.half + 0.12)];
                    if !in_rect(p, area) || on_other_road(p, r.line, 0.3) || in_building(p) {
                        continue;
                    }
                    out.push(Placement { asset: cb.id.clone(), part: straight, x: p[0], z: p[1], y: c.terrain.at(p[0], p[1]), rot: heading + turn, scale: [1.0, 1.0, 1.0], tint: [1.0, 1.0, 1.0], lod: 0, indoor: false });
                }
            }
            if have("sf_6881aa1e84") {
                let n = (len / 32.0).floor() as usize;
                for k in 0..n {
                    let t = (k as f64 + if side > 0.0 { 0.25 } else { 0.75 }) * 32.0;
                    let p = [r.a[0] + ux * t + nx * (r.half + 0.6), r.a[1] + uz * t + nz * (r.half + 0.6)];
                    if !in_rect(p, area) || on_other_road(p, r.line, 4.0) || in_building(p) {
                        continue;
                    }
                    out.push(Placement { asset: "sf_6881aa1e84".into(), part: crate::library::ALL_PARTS, x: p[0], z: p[1], y: c.terrain.at(p[0], p[1]), rot: heading + 90.0 * side, scale: [1.0; 3], tint: [1.0; 3], lod: 0, indoor: false });
                }
            }
            let slots = (len / 6.0).floor() as usize;
            for k in 0..slots {
                let t = (k as f64 + 0.5) * 6.0;
                let p = [r.a[0] + ux * t + nx * (r.half + 0.9), r.a[1] + uz * t + nz * (r.half + 0.9)];
                if !in_rect(p, area) || on_other_road(p, r.line, 2.5) || in_building(p) {
                    continue;
                }
                let h = hash(hash(k as u64, r.line as u64), side.to_bits());
                let mut roll = h % 100;
                for (ids, pct) in furniture {
                    if roll < pct {
                        let id = ids[((h >> 8) % ids.len() as u64) as usize];
                        if have(id) {
                            // a few degrees off square, as installed
                            let jitter = ((h >> 20) % 17) as f64 - 8.0;
                            out.push(Placement { asset: id.to_string(), part: crate::library::ALL_PARTS, x: p[0], z: p[1], y: c.terrain.at(p[0], p[1]), rot: heading + jitter, scale: [1.0, 1.0, 1.0], tint: [1.0, 1.0, 1.0], lod: 0, indoor: false });
                        }
                        break;
                    }
                    roll -= pct;
                }
            }
        }
    }

    out
}

/// Street palms at every LiDAR tree position, built from Quixel parts: stacked "Palm Tree Trunk" segments up to
/// the measured height and a fan-palm crown. Near `area` they get the detailed crown; elsewhere a lighter one.
pub fn trees(lib: &Library, c: &City, area: [f64; 4]) -> Vec<Placement> {
    let find = |t: &str| lib.catalog.assets.iter().filter(|a| a.title == t && !a.parts.is_empty()).collect::<Vec<_>>();
    let trunks = find("Palm Tree Trunk");
    let rich = find("Chinese Fan Palm").into_iter().chain(find("Tropical Palm")).collect::<Vec<_>>();
    let light = find("Fan Palm");
    if trunks.is_empty() || (rich.is_empty() && light.is_empty()) {
        return vec![];
    }
    let near = [area[0] - 120.0, area[1] - 120.0, area[2] + 120.0, area[3] + 120.0];
    let mut out = vec![];
    for (i, p) in c.trees.iter().enumerate() {
        let h = c.tree_h.get(i).copied().unwrap_or(8.0).clamp(4.0, 22.0);
        let hs = hash(i as u64, 4242);
        let y0 = c.terrain.at(p[0], p[1]);
        let detailed = in_rect(*p, near);
        let crowns = if detailed && !rich.is_empty() { &rich } else if !light.is_empty() { &light } else { &rich };
        let crown = crowns[(hs % crowns.len() as u64) as usize];
        let ce = extent(crown);
        // crowns 4.5–6.5 m across
        let cs = (4.5 + ((hs >> 8) % 100) as f32 / 50.0) / ce[0].max(ce[2]).max(0.1);
        let crown_h = ce[1] as f64 * cs as f64;
        let trunk_h = (h - crown_h * 0.45).max(1.5);
        let t = trunks[((hs >> 16) % trunks.len() as u64) as usize];
        let seg = extent(t)[1] as f64;
        // one stretched segment: stacking shows each segment's flared cap
        let n = 1usize;
        let sy = (trunk_h / seg) as f32;
        let sw = 0.62f32;
        for k in 0..n {
            out.push(Placement { asset: t.id.clone(), part: 0, x: p[0], z: p[1], y: y0 + k as f64 * seg * sy as f64, rot: ((hs >> (k * 3)) % 360) as f64, scale: [sw, sy, sw], tint: [1.0, 1.0, 1.0], lod: 0, indoor: false });
        }
        out.push(Placement { asset: crown.id.clone(), part: crate::library::ALL_PARTS, x: p[0], z: p[1], y: y0 + trunk_h - crown_h * 0.25, rot: (hs % 360) as f64, scale: [cs, cs, cs], tint: [1.0, 1.0, 1.0], lod: 0, indoor: false });
    }
    out
}

/// Painted road markings from Quixel decals (real-world scale, alpha-cut): a double yellow centreline on wide
/// streets, dashed white lane lines, and crosswalks set back from intersections. Returns (asset id, mesh) strips.
pub fn markings(lib: &Library, c: &City, area: [f64; 4]) -> Vec<(String, Vec<render::TexVertex>, Vec<u32>)> {
    let decal = |t: &str| lib.catalog.assets.iter().find(|a| a.title == t && a.kind == assets::AssetKind::Surface).map(|a| (a.id.clone(), a.tile_m.unwrap_or([1.0, 1.0])));
    let (Some(yellow), Some(white), Some(cross)) = (decal("Yellow Road Line"), decal("White Road Line"), decal("Painted Pedestrian Crossing Lines")) else {
        if std::env::var("FPS_DEBUG").is_ok() {
            eprintln!("markings: decals missing {:?}", ["Yellow Road Line", "White Road Line", "Painted Pedestrian Crossing Lines"].map(|t| decal(t).is_some()));
        }
        return vec![];
    };
    let grow = [area[0] - 60.0, area[1] - 60.0, area[2] + 60.0, area[3] + 60.0];
    let roads: Vec<Road> = c
        .lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == LineKind::Road)
        .flat_map(|(i, l)| l.pts.windows(2).map(move |w| Road { a: w[0], b: w[1], half: l.width * 0.5, line: i }))
        .filter(|r| in_rect(r.a, grow) || in_rect(r.b, grow))
        .collect();
    let mut meshes: std::collections::HashMap<String, (Vec<render::TexVertex>, Vec<u32>)> = Default::default();
    // a strip from a to b, `w` wide, offset sideways by `off`; u runs along in tiles of `tile` metres
    let mut strip = |id: &str, a: [f64; 2], b: [f64; 2], off: f64, w: f64, tile: f64, across_u: bool| {
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dz);
        if len < 0.5 {
            return;
        }
        let (ux, uz) = (dx / len, dz / len);
        let (nx, nz) = (-uz, ux);
        let m = meshes.entry(id.to_string()).or_default();
        let steps = (len / 2.0).ceil().max(1.0) as usize;
        let base = m.0.len() as u32;
        for k in 0..=steps {
            let t = len * k as f64 / steps as f64;
            for side in [-0.5f64, 0.5] {
                let p = [a[0] + ux * t + nx * (off + side * w), a[1] + uz * t + nz * (off + side * w)];
                let y = c.terrain.at(p[0], p[1]) + 0.17;
                let (u, v) = if across_u { ((side + 0.5) * w / tile, t / tile) } else { (t / tile, side + 0.5) };
                m.0.push(render::TexVertex { pos: [p[0] as f32, y as f32, p[1] as f32], nrm: [0.0, 1.0, 0.0], uv: [u as f32, v as f32], tan: [ux as f32, 0.0, uz as f32, 1.0], edge: [0.0; 4] });
            }
        }
        for k in 0..steps as u32 {
            let i = base + k * 2;
            m.1.extend([i, i + 2, i + 1, i + 1, i + 2, i + 3]);
        }
    };
    let inter = |p: [f64; 2], line: usize| roads.iter().any(|r| r.line != line && seg_dist(p, r.a, r.b) < r.half + 1.0);
    for r in &roads {
        let (dx, dz) = (r.b[0] - r.a[0], r.b[1] - r.a[1]);
        let len = dx.hypot(dz);
        if len < 4.0 {
            continue;
        }
        let (ux, uz) = (dx / len, dz / len);
        // trim the run to stay out of intersections, leaving room for crosswalks
        let mut t0 = 0.0;
        while t0 < len && inter([r.a[0] + ux * t0, r.a[1] + uz * t0], r.line) {
            t0 += 0.5;
        }
        let mut t1 = len;
        while t1 > t0 && inter([r.a[0] + ux * t1, r.a[1] + uz * t1], r.line) {
            t1 -= 0.5;
        }
        if t1 - t0 < 6.0 {
            continue;
        }
        let at = |t: f64| [r.a[0] + ux * t, r.a[1] + uz * t];
        let cw = cross.1[0] as f64;
        // crosswalks at both ends where a street meets this one
        if t0 > 0.0 {
            let p = at(t0 + cw * 0.5 + 0.5);
            strip(&cross.0, [p[0] - uz * r.half * 0.92, p[1] + ux * r.half * 0.92], [p[0] + uz * r.half * 0.92, p[1] - ux * r.half * 0.92], 0.0, cw, cross.1[0] as f64, true);
        }
        if t1 < len {
            let p = at(t1 - cw * 0.5 - 0.5);
            strip(&cross.0, [p[0] - uz * r.half * 0.92, p[1] + ux * r.half * 0.92], [p[0] + uz * r.half * 0.92, p[1] - ux * r.half * 0.92], 0.0, cw, cross.1[0] as f64, true);
        }
        let (a, b) = (at(t0 + cw + 1.5), at(t1 - cw - 1.5));
        if seg_dist(a, b, b) < 3.0 {
            continue;
        }
        let ylw = yellow.1[0] as f64;
        let wht = white.1[0] as f64;
        if r.half * 2.0 >= 14.0 {
            strip(&yellow.0, a, b, 0.0, ylw, ylw, false);
            // lane lines a lane and a half out from the centre
            for side in [-1.0, 1.0] {
                strip(&white.0, a, b, side * 3.4, wht, wht, false);
            }
        } else {
            strip(&white.0, a, b, 0.0, wht, wht, false);
        }
    }
    if std::env::var("FPS_DEBUG").is_ok() {
        eprintln!("markings: {} road segments; {:?}", roads.len(), meshes.iter().map(|(k, m)| (k.clone(), m.1.len() / 3)).collect::<Vec<_>>());
    }
    meshes.into_iter().map(|(k, (v, i))| (k, v, i)).collect()
}
