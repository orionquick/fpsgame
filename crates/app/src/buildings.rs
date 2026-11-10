//! Buildings assembled entirely from Megascans modular-kit prefabs along their real footprints:
//! storefront ground floors, window floors, per-floor corner pieces, cornices, parapets and flat-roof tiles.
//! The extruded lidar volumes only remain as invisible collision.
use crate::library::{Library, Placement, GLASS};
use city::model::{Building, City};

#[derive(Clone)]
struct Piece {
    id: String,
    part: usize,
    w: f32,
    h: f32,
    d: f32,
    /// Has window openings that need glazing behind them.
    glazed: bool,
}

fn piece(lib: &Library, title: &str, part: usize, variant: u64, glazed: bool) -> Option<Piece> {
    let v: Vec<&assets::Asset> = lib.catalog.assets.iter().filter(|a| a.title == title && a.parts.len() > part).collect();
    let a = v.get((variant % v.len().max(1) as u64) as usize)?;
    let p = &a.parts[part];
    Some(Piece { id: a.id.clone(), part, w: p.max[0] - p.min[0], h: p.max[1] - p.min[1], d: p.max[2] - p.min[2], glazed })
}

fn hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
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

struct Style {
    ground: Vec<Piece>,
    upper: Vec<Piece>,
    ground_corner: Option<Piece>,
    upper_corner: Option<Piece>,
    cornice: Option<Piece>,
    cornice_corner: Option<Piece>,
    parapet: Option<Piece>,
    parapet_corner: Option<Piece>,
    balcony: Option<Piece>,
    /// Column width the facade grid aims for, metres.
    grid: f32,
}

fn style(lib: &Library, height: f64, seed: u64) -> Option<Style> {
    let g = |t: &str, p: usize, glazed: bool| piece(lib, t, p, seed, glazed);
    let ground = vec![
        g("Modular Building Door and Window", 0, true),
        g("Modular Building Door and Window", 0, true),
        g("Modular Building Ground Floor Kit", 1, false),
        g("Modular Building Door Kit", 1, true),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let s = if height > 50.0 {
        Style {
            ground,
            upper: vec![g("Urban Modular Building Window 01", 0, true); 7].into_iter().chain([g("Urban Modular Building Wall Set 01", 1, false)]).flatten().collect(),
            ground_corner: g("Modular Building Ground Floor Kit", 2, false),
            upper_corner: g("Urban Modular Building Wall Corner Set", 0, false),
            cornice: g("Modular Building 4th Floor Trim Kit", 0, false),
            cornice_corner: g("Modular Building 4th Floor Trim Kit", 3, false),
            parapet: g("Modular Building Roof Trim Kit", 3, false),
            parapet_corner: g("Modular Building Roof Trim Kit", 4, false),
            balcony: None,
            grid: 2.5,
        }
    } else if height > 22.0 {
        Style {
            ground,
            upper: vec![g("Modular Building 4th Floor Kit", 0, true)].into_iter().flatten().collect(),
            ground_corner: g("Modular Building Ground Floor Kit", 2, false),
            upper_corner: g("Modular Building 4th Floor Kit", 1, false),
            cornice: g("Modular Building 4th Floor Trim Kit", 0, false),
            cornice_corner: g("Modular Building 4th Floor Trim Kit", 3, false),
            parapet: g("Modular Building Roof Trim Kit", 3, false),
            parapet_corner: g("Modular Building Roof Trim Kit", 4, false),
            balcony: g("Modular Building Balcony", 0, false),
            grid: 3.0,
        }
    } else {
        Style {
            ground,
            upper: vec![piece(lib, "Modular Building Window", 0, seed, true), piece(lib, "Modular Building Window", 0, seed, true), piece(lib, "Modular Building Window", 0, seed >> 9, true)].into_iter().flatten().collect(),
            ground_corner: g("Modular Building Ground Floor Kit", 2, false),
            upper_corner: g("Modular Building 2nd Floor Kit", 3, false),
            cornice: g("Modular Building Trim Kit", 1, false),
            cornice_corner: g("Modular Building Trim Kit", 2, false),
            parapet: g("Modular Building Roof Trim Kit", 3, false),
            parapet_corner: g("Modular Building Roof Trim Kit", 4, false),
            balcony: g("Modular Building Balcony", 0, false),
            grid: 3.2,
        }
    };
    (!s.ground.is_empty() && !s.upper.is_empty()).then_some(s)
}

pub struct Stats {
    pub buildings: usize,
    pub pieces: usize,
    pub tris: u64,
}

/// Prefab placements for every building. `focus` is the area that gets full detail; distant buildings use
/// the same pieces on a coarser grid.
pub fn build(lib: &Library, c: &City, focus: [f64; 4]) -> (Vec<Placement>, Stats) {
    let mut out = vec![];
    let mut st = Stats { buildings: 0, pieces: 0, tris: 0 };
    let roof_tile = piece(lib, "Urban Modular Building Roof Set", 2, 0, false).or_else(|| piece(lib, "Modular Building Flat Roof Kit", 1, 0, false));
    let spans: std::collections::HashMap<&str, (f64, f64)> = c.buildings.iter().map(|b| (b.osm.as_str(), city::mesh::building_span(b, &c.terrain))).collect();
    for b in &c.buildings {
        let (y0, y1) = city::mesh::building_span(b, &c.terrain);
        // tiers start on top of the building they stand on
        let start = match &b.part_of {
            Some(p) => spans.get(p.as_str()).map_or(y0 + 0.5, |s| s.1),
            None => y0 + 0.5,
        };
        let height = y1 - start;
        if height < 2.4 || b.outer.len() < 3 {
            continue;
        }
        let seed = hash(b.osm.bytes().fold(0u64, |h, x| h.wrapping_mul(31).wrapping_add(x as u64)), 11);
        let total_h = y1 - y0;
        let Some(sty) = style(lib, total_h, seed) else { continue };
        let ctr = b.outer.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]).map(|v| v / b.outer.len() as f64);
        let dist = (ctr[0] - ctr[0].clamp(focus[0], focus[2])).hypot(ctr[1] - ctr[1].clamp(focus[1], focus[3]));
        // level of detail: coarser columns and fewer extras with distance
        let (lod, mesh_lod) = if dist < 120.0 { (1.0, 0u8) } else if dist < 320.0 { (1.25, 1) } else { (1.8, 2) };
        let n0 = out.len();
        build_one(b, &sty, start, height, b.part_of.is_none(), lod, seed, roof_tile.as_ref(), &mut out);
        // per-piece detail: distance to the focus area, and height above street level (nobody sees fine detail up high)
        for p in &mut out[n0..] {
            let d = (p.x - p.x.clamp(focus[0], focus[2])).hypot(p.z - p.z.clamp(focus[1], focus[3]));
            let by_dist = if d < 60.0 { 0u8 } else if d < 220.0 { 1 } else { 2 };
            let up = p.y - y0;
            let by_height = if up > 60.0 { 2u8 } else if up > 25.0 { 1 } else { 0 };
            p.lod = by_dist.max(by_height).max(mesh_lod.saturating_sub(1));
        }
        st.buildings += 1;
        st.pieces += out.len() - n0;
    }
    for p in &out {
        if let Some(a) = lib.catalog.get(&p.asset) {
            st.tris += a.parts.get(p.part).map_or(0, |q| (q.tris as f64 * [1.0, 0.25, 0.06][p.lod as usize]) as u64);
        }
    }
    (out, st)
}

fn place(out: &mut Vec<Placement>, pc: &Piece, x: f64, z: f64, y: f64, rot: f64, sx: f32, sy: f32) {
    out.push(Placement { asset: pc.id.clone(), part: pc.part, x, z, y, rot, scale: [sx, sy, 1.0], tint: [1.0, 1.0, 1.0], lod: 0 });
}

#[allow(clippy::too_many_arguments)]
fn build_one(b: &Building, sty: &Style, start: f64, height: f64, on_ground: bool, lod: f32, seed: u64, roof_tile: Option<&Piece>, out: &mut Vec<Placement>) {
    let ring = &b.outer;
    let cw = (0..ring.len()).map(|i| { let (a, q) = (ring[i], ring[(i + 1) % ring.len()]); a[0] * q[1] - q[0] * a[1] }).sum::<f64>() > 0.0;
    let ground_h = sty.ground[0].h as f64;
    let has_ground = on_ground && height > ground_h + 0.5;
    let up_h = sty.upper[0].h as f64 * lod as f64;
    let cornice_h = sty.cornice.as_ref().map_or(0.0, |p| p.h as f64);
    let body = height - if has_ground { ground_h } else { 0.0 } - cornice_h;
    let floors = (body / up_h).round().max(if body > 1.5 { 1.0 } else { 0.0 }) as usize;
    let fh = if floors > 0 { body / floors as f64 } else { 0.0 };
    let top = start + height - cornice_h;
    let grid = sty.grid * lod;

    let n = ring.len();
    for i in 0..n {
        let (a, q) = (ring[i], ring[(i + 1) % n]);
        let (dx, dz) = (q[0] - a[0], q[1] - a[1]);
        let len = dx.hypot(dz);
        if len < 1.2 {
            continue;
        }
        let (ux, uz) = (dx / len, dz / len);
        let (nx, nz) = if cw { (uz, -ux) } else { (-uz, ux) };
        let rot = nz.atan2(nx).to_degrees();
        let cols = ((len / grid as f64).round() as usize).max(1);
        let w = len / cols as f64;
        for k in 0..cols {
            let t = (k as f64 + 0.5) * w;
            let p = [a[0] + ux * t, a[1] + uz * t];
            let h = hash(seed, (i * 997 + k) as u64);
            let mut y = start;
            if has_ground {
                let g = &sty.ground[(h % sty.ground.len() as u64) as usize];
                let off = g.d as f64 * 0.5;
                place(out, g, p[0] - nx * off, p[1] - nz * off, y, rot, (w / g.w as f64) as f32, (ground_h / g.h as f64) as f32);
                if g.glazed {
                    glaze(out, p, nx, nz, rot, y, w, ground_h, g.d as f64);
                }
                y += ground_h;
            }
            for f in 0..floors {
                let u = &sty.upper[(hash(h, f as u64) % sty.upper.len() as u64) as usize];
                let off = u.d as f64 * 0.5;
                place(out, u, p[0] - nx * off, p[1] - nz * off, y, rot, (w / u.w as f64) as f32, (fh / u.h as f64) as f32);
                if u.glazed {
                    glaze(out, p, nx, nz, rot, y, w, fh, u.d as f64);
                }
                // balconies on some upper-floor columns (not on the top floor), stacked in vertical runs
                if let Some(bc) = &sty.balcony {
                    if lod < 1.2 && f + 1 < floors && hash(seed, (i * 31 + k) as u64) % 100 < 18 {
                        let sx = ((w * 0.9) / bc.w as f64).min(1.0) as f32;
                        let out_off = bc.d as f64 * 0.5 - 0.05;
                        place(out, bc, p[0] + nx * out_off, p[1] + nz * out_off, y, rot, sx, 1.0);
                    }
                }
                y += fh;
            }
            for (pc, py) in [(&sty.cornice, top), (&sty.parapet, start + height)] {
                if let Some(pc) = pc {
                    let off = pc.d as f64 * 0.5;
                    place(out, pc, p[0] - nx * off, p[1] - nz * off, py, rot, (w / pc.w as f64) as f32, 1.0);
                }
            }
        }
    }
    // corners: one piece per floor where the footprint turns
    for i in 0..n {
        let (p0, p1, p2) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
        let d0 = [p1[0] - p0[0], p1[1] - p0[1]];
        let d1 = [p2[0] - p1[0], p2[1] - p1[1]];
        let (l0, l1) = (d0[0].hypot(d0[1]), d1[0].hypot(d1[1]));
        if l0 < 1.0 || l1 < 1.0 {
            continue;
        }
        let turn = ((d0[0] * d1[1] - d0[1] * d1[0]) / (l0 * l1)).asin().abs();
        if turn < 0.4 {
            continue;
        }
        let bis = [d0[0] / l0 - d1[0] / l1, d0[1] / l0 - d1[1] / l1];
        let rot = bis[1].atan2(bis[0]).to_degrees();
        let mut y = start;
        if has_ground {
            if let Some(cp) = &sty.ground_corner {
                place(out, cp, p1[0], p1[1], y, rot, 1.0, (ground_h / cp.h as f64) as f32);
            }
            y += ground_h;
        }
        if let Some(cp) = &sty.upper_corner {
            for _ in 0..floors {
                place(out, cp, p1[0], p1[1], y, rot, 1.0, (fh / cp.h as f64) as f32);
                y += fh;
            }
        }
        if let Some(cp) = &sty.cornice_corner {
            place(out, cp, p1[0], p1[1], top, rot, 1.0, 1.0);
        }
        if let Some(cp) = &sty.parapet_corner {
            place(out, cp, p1[0], p1[1], start + height, rot, 1.0, 1.0);
        }
    }
    // flat roof tiles over the footprint
    if let Some(rt) = roof_tile {
        // 6 m tiles where they fit, then 3 m tiles to fill along the edges (grey: the kit's tile reads pink)
        let mut filled: Vec<[f64; 3]> = vec![];
        for s in [6.0 / rt.w, 3.0 / rt.w, 1.5 / rt.w] {
        let size = rt.w * s;
        let half = size as f64 * 0.5;
        let mn = ring.iter().fold([f64::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
        let mx = ring.iter().fold([f64::MIN; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
        let mut x = mn[0] + size as f64 * 0.5;
        while x < mx[0] {
            let mut z = mn[1] + size as f64 * 0.5;
            while z < mx[1] {
                // whole tile inside the footprint: no overhangs
                let inside = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0], [0.0, 0.0]].iter().all(|c: &[f64; 2]| point_in(ring, [x + c[0] * half, z + c[1] * half]));
                let free = !filled.iter().any(|f| (x - f[0]).abs() < (half + f[2]) - 0.01 && (z - f[1]).abs() < (half + f[2]) - 0.01);
                if inside && free {
                    filled.push([x, z, half]);
                    out.push(Placement { asset: rt.id.clone(), part: rt.part, x, z, y: start + height - 0.1, rot: 0.0, scale: [s, 1.0, s], tint: [0.62, 0.64, 0.64], lod: 0 });
                }
                z += size as f64;
            }
            x += size as f64;
        }
        }
    }
}

/// Dark glazing just behind a module's front face, seen through its window openings.
#[allow(clippy::too_many_arguments)]
fn glaze(out: &mut Vec<Placement>, p: [f64; 2], nx: f64, nz: f64, rot: f64, y: f64, w: f64, h: f64, d: f64) {
    // deep inside the module so its recessed frames and mullions stay in front of the glass
    let back = (d * 0.8).clamp(0.1, 1.0);
    // mostly dark panes; some with blinds, some showing a warm interior
    let r = hash((p[0] * 7.3 + y * 1.7) as i64 as u64, (p[1] * 3.1) as i64 as u64) % 100;
    let tint = if r < 60 { [0.03, 0.035, 0.04] } else if r < 82 { [0.055, 0.06, 0.065] } else if r < 93 { [0.11, 0.105, 0.095] } else { [0.17, 0.12, 0.07] };
    out.push(Placement { asset: GLASS.into(), part: 0, x: p[0] - nx * back, z: p[1] - nz * back, y: y + 0.05, rot, scale: [w as f32 * 0.98, (h - 0.1) as f32, 1.0], tint, lod: 0 });
}
