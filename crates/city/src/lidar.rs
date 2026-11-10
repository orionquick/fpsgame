//! USGS 3DEP lidar (CA_SanDiegoCo_D24, flown Nov 2024): bare-earth ground, building roof heights and
//! vegetation heights rasterised onto a grid in the local frame, and per-building massing derived from them.
use crate::geo::Frame;
use crate::model::{Building, Ring, Terrain};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn std::error::Error>>;
type SRes<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub const TILES: [&str; 4] = ["11SMS484618", "11SMS484619", "11SMS485618", "11SMS485619"];
const TILE_URL: &str = "https://rockyweb.usgs.gov/vdelivery/Datasets/Staged/Elevation/LPC/Projects/CA_SanDiegoCo_D24/CA_SanDiegoCo_2_D24/LAZ/USGS_LPC_CA_SanDiegoCo_D24_";

/// Grid cell size, metres.
pub const RES: f64 = 0.5;

pub fn tile_paths(dir: &Path) -> Vec<PathBuf> {
    TILES.iter().map(|t| dir.join(format!("USGS_LPC_CA_SanDiegoCo_D24_{t}.laz"))).collect()
}

pub fn tile_url(t: &str) -> String {
    format!("{TILE_URL}{t}.laz")
}

/// Rasters over `rect` (min_x, min_z, max_x, max_z) at `RES`; heights are metres NAVD88, NaN where empty.
pub struct Grids {
    pub rect: [f64; 4],
    pub w: usize,
    pub h: usize,
    pub ground: Vec<f32>,
    pub roof: Vec<f32>,
    pub veg: Vec<f32>,
    /// Highest non-noise return of any class (catches unclassified structures like stadium stands).
    pub top: Vec<f32>,
}

impl Grids {
    pub fn cell(&self, x: f64, z: f64) -> Option<usize> {
        let (i, j) = (((x - self.rect[0]) / RES).floor(), ((z - self.rect[1]) / RES).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.w && (j as usize) < self.h).then(|| j as usize * self.w + i as usize)
    }
    pub fn center(&self, i: usize, j: usize) -> [f64; 2] {
        [self.rect[0] + (i as f64 + 0.5) * RES, self.rect[1] + (j as f64 + 0.5) * RES]
    }
}

/// UTM (northern hemisphere) to lat/lon on GRS80, Snyder's series. Sub-centimetre near the central meridian.
pub fn utm_to_latlon(e: f64, n: f64, zone: u32) -> (f64, f64) {
    let (a, f, k0) = (6_378_137.0f64, 1.0 / 298.257_222_101, 0.9996);
    let e2: f64 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);
    let x = e - 500_000.0;
    let m = n / k0;
    let mu = m / (a * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0));
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
    let (s, c, t) = (phi1.sin(), phi1.cos(), phi1.tan());
    let n1 = a / (1.0 - e2 * s * s).sqrt();
    let (t1, c1) = (t * t, ep2 * c * c);
    let r1 = a * (1.0 - e2) / (1.0 - e2 * s * s).powf(1.5);
    let d = x / (n1 * k0);
    let lat = phi1 - (n1 * t / r1) * (d * d / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0 + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1) * d.powi(6) / 720.0);
    let lon0 = (zone as f64 * 6.0 - 183.0).to_radians();
    let lon = lon0 + (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0 + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d.powi(5) / 120.0) / c;
    (lat.to_degrees(), lon.to_degrees())
}

/// Affine fit (E, N) → (x, z) over a tile from exact conversions; error is millimetres over a kilometre.
fn affine(frame: &Frame, zone: u32, e0: f64, n0: f64, span: f64) -> [f64; 6] {
    let p = |de: f64, dn: f64| {
        let (la, lo) = utm_to_latlon(e0 + de, n0 + dn, zone);
        frame.to_xz(la, lo)
    };
    let (o, ex, ny) = (p(0.0, 0.0), p(span, 0.0), p(0.0, span));
    [o[0], (ex[0] - o[0]) / span, (ny[0] - o[0]) / span, o[1], (ex[1] - o[1]) / span, (ny[1] - o[1]) / span]
}

/// Reads the WKT VLR: (UTM zone, horizontal metres-per-unit, vertical metres-per-unit).
fn crs(h: &las::Header) -> (u32, f64, f64, String) {
    let wkt = h.all_vlrs().find(|v| v.user_id == "LASF_Projection" && v.record_id == 2112).map(|v| String::from_utf8_lossy(&v.data).to_string()).unwrap_or_default();
    let zone = wkt.find("UTM zone ").and_then(|i| wkt[i + 9..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()).unwrap_or(11);
    let (horiz, vert) = wkt.split_once("VERT").unwrap_or((&wkt, ""));
    let unit = |s: &str| if s.contains("US survey foot") || s.contains("foot_us") { 1200.0 / 3937.0 } else if s.contains("foot") { 0.3048 } else { 1.0 };
    let summary = wkt.split('"').nth(1).unwrap_or("unknown CRS").to_string();
    (zone, unit(horiz), if vert.is_empty() { unit(horiz) } else { unit(vert) }, summary)
}

/// Rasterises every tile onto the grid. Tiles are decoded in parallel.
pub fn rasterize(paths: &[PathBuf], frame: &Frame, rect: [f64; 4]) -> Res<Grids> {
    let w = ((rect[2] - rect[0]) / RES).ceil() as usize;
    let h = ((rect[3] - rect[1]) / RES).ceil() as usize;
    let mut g = Grids { rect, w, h, ground: vec![f32::NAN; w * h], roof: vec![f32::NAN; w * h], veg: vec![f32::NAN; w * h], top: vec![f32::NAN; w * h] };
    let parts: Vec<SRes<(Vec<f32>, Vec<u32>, Vec<f32>, Vec<f32>, Vec<f32>)>> = std::thread::scope(|s| {
        let hs: Vec<_> = paths
            .iter()
            .map(|p| {
                s.spawn(move || -> SRes<_> {
                    let mut r = las::Reader::from_path(p)?;
                    let (zone, hu, vu, name) = crs(r.header());
                    let b = r.header().bounds();
                    eprintln!("  {}: {} points, {name}, xy x{hu:.4} z x{vu:.4}", p.file_name().unwrap().to_string_lossy(), r.header().number_of_points());
                    let (e0, n0) = (b.min.x * hu, b.min.y * hu);
                    let af = affine(frame, zone, e0, n0, 1000.0);
                    let (mut gs, mut gc) = (vec![0f32; w * h], vec![0u32; w * h]);
                    let (mut roof, mut veg, mut top) = (vec![f32::NAN; w * h], vec![f32::NAN; w * h], vec![f32::NAN; w * h]);
                    let max = |v: &mut f32, z: f32| if !(*v >= z) { *v = z };
                    loop {
                        let pd = r.read_points(2_000_000)?;
                        if pd.is_empty() {
                            break;
                        }
                        for pt in pd.points() {
                            let pt = pt?;
                            let (de, dn) = (pt.x * hu - e0, pt.y * hu - n0);
                            let x = af[0] + af[1] * de + af[2] * dn;
                            let zz = af[3] + af[4] * de + af[5] * dn;
                            let (i, j) = (((x - rect[0]) / RES).floor(), ((zz - rect[1]) / RES).floor());
                            if i < 0.0 || j < 0.0 || i as usize >= w || j as usize >= h || pt.is_withheld {
                                continue;
                            }
                            let k = j as usize * w + i as usize;
                            let z = (pt.z * vu) as f32;
                            use las::point::Classification as C;
                            // This survey classifies only ground/water/noise; everything else is "unclassified".
                            // Hard surfaces return a pulse once, canopy usually several times: split on that.
                            match pt.classification {
                                C::Ground => {
                                    gs[k] += z;
                                    gc[k] += 1;
                                }
                                C::Building => max(&mut roof[k], z),
                                C::LowVegetation | C::MediumVegetation | C::HighVegetation => max(&mut veg[k], z),
                                C::Unclassified | C::CreatedNeverClassified if pt.number_of_returns <= 1 => max(&mut roof[k], z),
                                C::Unclassified | C::CreatedNeverClassified => max(&mut veg[k], z),
                                _ => {}
                            }
                            if !matches!(pt.classification, C::LowPoint) && u8::from(pt.classification) != 18 {
                                max(&mut top[k], z);
                            }
                        }
                    }
                    Ok((gs, gc, roof, veg, top))
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut gsum = vec![0f32; w * h];
    let mut gcnt = vec![0u32; w * h];
    let merge = |dst: &mut [f32], src: &[f32]| dst.iter_mut().zip(src).for_each(|(d, s)| if s.is_finite() && !(*d >= *s) { *d = *s });
    for p in parts {
        let (gs, gc, roof, veg, top) = p.map_err(|e| e.to_string())?;
        gsum.iter_mut().zip(&gs).for_each(|(a, b)| *a += b);
        gcnt.iter_mut().zip(&gc).for_each(|(a, b)| *a += b);
        merge(&mut g.roof, &roof);
        merge(&mut g.veg, &veg);
        merge(&mut g.top, &top);
    }
    for k in 0..w * h {
        if gcnt[k] > 0 {
            g.ground[k] = gsum[k] / gcnt[k] as f32;
        }
    }
    fill_holes(&mut g.ground, w, h);
    Ok(g)
}

/// Push-pull hole filling: average down a pyramid until every cell is covered, then fill back up.
pub fn fill_holes(v: &mut [f32], w: usize, h: usize) {
    if v.iter().all(|x| x.is_finite()) || w < 2 || h < 2 {
        return;
    }
    let (w2, h2) = (w.div_ceil(2), h.div_ceil(2));
    let mut c = vec![f32::NAN; w2 * h2];
    for j in 0..h2 {
        for i in 0..w2 {
            let (mut s, mut n) = (0.0, 0);
            for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, z) = (i * 2 + dx, j * 2 + dz);
                if x < w && z < h && v[z * w + x].is_finite() {
                    s += v[z * w + x];
                    n += 1;
                }
            }
            if n > 0 {
                c[j * w2 + i] = s / n as f32;
            }
        }
    }
    fill_holes(&mut c, w2, h2);
    for j in 0..h {
        for i in 0..w {
            if !v[j * w + i].is_finite() {
                v[j * w + i] = c[(j / 2) * w2 + i / 2];
            }
        }
    }
}

/// Bare-earth terrain at `step` metres, relative to `origin_elev`.
pub fn terrain(g: &Grids, step: f64, origin_elev: f64) -> Terrain {
    let k = (step / RES).round().max(1.0) as usize;
    let (tw, th) = (g.w / k, g.h / k);
    let mut y = vec![0f32; tw * th];
    for j in 0..th {
        for i in 0..tw {
            let mut s = 0.0;
            for dz in 0..k {
                for dx in 0..k {
                    s += g.ground[(j * k + dz) * g.w + i * k + dx];
                }
            }
            y[j * tw + i] = s / (k * k) as f32 - origin_elev as f32;
        }
    }
    // cell centres: the terrain rect spans the first to the last sample
    let half = step * 0.5;
    Terrain { rect: [g.rect[0] + half, g.rect[1] + half, g.rect[0] + half + (tw - 1) as f64 * step, g.rect[1] + half + (th - 1) as f64 * step], w: tw, h: th, y }
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

/// Grid cells whose centres fall inside a polygon (with holes).
fn cells_in(g: &Grids, outer: &[[f64; 2]], holes: &[Ring]) -> Vec<usize> {
    let min = outer.iter().fold([f64::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
    let max = outer.iter().fold([f64::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
    let i0 = (((min[0] - g.rect[0]) / RES).floor().max(0.0)) as usize;
    let j0 = (((min[1] - g.rect[1]) / RES).floor().max(0.0)) as usize;
    let i1 = (((max[0] - g.rect[0]) / RES).ceil() as usize).min(g.w);
    let j1 = (((max[1] - g.rect[1]) / RES).ceil() as usize).min(g.h);
    let mut out = vec![];
    for j in j0..j1 {
        for i in i0..i1 {
            let c = g.center(i, j);
            if point_in(outer, c) && !holes.iter().any(|h| point_in(h, c)) {
                out.push(j * g.w + i);
            }
        }
    }
    out
}

fn percentile(v: &mut [f32], p: f64) -> f32 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Outer boundary loops of a cell set, traced along cell edges and simplified.
fn polygonize(g: &Grids, cells: &[usize]) -> Vec<Ring> {
    use std::collections::{HashMap, HashSet};
    let set: HashSet<usize> = cells.iter().copied().collect();
    let has = |i: i64, j: i64| i >= 0 && j >= 0 && (i as usize) < g.w && (j as usize) < g.h && set.contains(&(j as usize * g.w + i as usize));
    // directed boundary edges on the corner lattice, wound so the inside is on the right (clockwise, x right / z down)
    let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for &k in cells {
        let (i, j) = ((k % g.w) as i64, (k / g.w) as i64);
        if !has(i, j - 1) { next.entry((i, j)).or_default().push((i + 1, j)); }
        if !has(i + 1, j) { next.entry((i + 1, j)).or_default().push((i + 1, j + 1)); }
        if !has(i, j + 1) { next.entry((i + 1, j + 1)).or_default().push((i, j + 1)); }
        if !has(i - 1, j) { next.entry((i, j + 1)).or_default().push((i, j)); }
    }
    let mut loops = vec![];
    while let Some(&start) = next.keys().next() {
        let mut lp = vec![start];
        let mut cur = start;
        loop {
            let Some(v) = next.get_mut(&cur) else { break };
            let n = v.pop().unwrap();
            if v.is_empty() {
                next.remove(&cur);
            }
            if n == start {
                break;
            }
            lp.push(n);
            cur = n;
        }
        if lp.len() >= 4 {
            let ring: Ring = lp.iter().map(|&(i, j)| [g.rect[0] + i as f64 * RES, g.rect[1] + j as f64 * RES]).collect();
            let area: f64 = (0..ring.len()).map(|k| { let (a, b) = (ring[k], ring[(k + 1) % ring.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() * 0.5;
            if area > 0.0 {
                loops.push(simplify(&ring, RES * 2.5));
            }
        }
    }
    loops.into_iter().filter(|r| r.len() >= 3).collect()
}

/// Douglas-Peucker on a closed ring.
fn simplify(r: &[[f64; 2]], tol: f64) -> Ring {
    fn dp(p: &[[f64; 2]], tol: f64, out: &mut Vec<[f64; 2]>) {
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dz * dz).sqrt().max(1e-9);
        let (mut best, mut bi) = (0.0, 0);
        for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
            let d = ((q[0] - a[0]) * dz - (q[1] - a[1]) * dx).abs() / l;
            if d > best {
                best = d;
                bi = i;
            }
        }
        if best > tol {
            dp(&p[..=bi], tol, out);
            dp(&p[bi..], tol, out);
        } else {
            out.push(a);
        }
    }
    // split at the point farthest from the first so both halves are open chains
    let far = (1..r.len()).max_by(|&i, &j| {
        let d = |k: usize| (r[k][0] - r[0][0]).powi(2) + (r[k][1] - r[0][1]).powi(2);
        d(i).total_cmp(&d(j))
    }).unwrap_or(0);
    let mut a: Vec<[f64; 2]> = r[..=far].to_vec();
    let mut b: Vec<[f64; 2]> = r[far..].to_vec();
    b.push(r[0]);
    let mut out = vec![];
    if a.len() >= 2 { dp(&mut a, tol, &mut out); }
    if b.len() >= 2 { dp(&mut b, tol, &mut out); }
    out
}

fn components(g: &Grids, cells: &[usize]) -> Vec<Vec<usize>> {
    use std::collections::HashSet;
    let mut left: HashSet<usize> = cells.iter().copied().collect();
    let mut out = vec![];
    while let Some(&s) = left.iter().next() {
        left.remove(&s);
        let mut comp = vec![s];
        let mut stack = vec![s];
        while let Some(k) = stack.pop() {
            let (i, j) = (k % g.w, k / g.w);
            let nb = [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)];
            for (a, b) in nb {
                if a < g.w && b < g.h && left.remove(&(b * g.w + a)) {
                    comp.push(b * g.w + a);
                    stack.push(b * g.w + a);
                }
            }
        }
        out.push(comp);
    }
    out
}

/// Minimum height above ground for a lidar return to count as part of a structure, metres.
const MIN_STRUCT: f32 = 2.5;
/// A part must rise this far above its parent to become its own tier.
const TIER_STEP: f32 = 4.0;
const MIN_TIER_CELLS: usize = (40.0 / (RES * RES)) as usize;

/// Height above `base` per cell: building returns, or any return for cells with none (glass, stands).
fn rel_heights(g: &Grids, cells: &[usize], base: f64) -> Vec<(usize, f32)> {
    cells
        .iter()
        .filter_map(|&k| {
            let z = if g.roof[k].is_finite() { g.roof[k] } else { g.top[k] };
            let h = z - base as f32;
            (h.is_finite() && h > MIN_STRUCT).then_some((k, h))
        })
        .collect()
}

/// Stacked tiers inside `cells` above a parent of height `parent_h`: returns (ring, height) parts.
fn tiers(g: &Grids, hs: &[(usize, f32)], parent_h: f32, depth: u32, out: &mut Vec<(Ring, f32)>) {
    if depth > 3 {
        return;
    }
    let upper: Vec<usize> = hs.iter().filter(|(_, h)| *h > parent_h + TIER_STEP).map(|(k, _)| *k).collect();
    for comp in components(g, &upper) {
        if comp.len() < MIN_TIER_CELLS {
            continue;
        }
        let set: std::collections::HashSet<usize> = comp.iter().copied().collect();
        let sub: Vec<(usize, f32)> = hs.iter().filter(|(k, _)| set.contains(k)).copied().collect();
        let mut v: Vec<f32> = sub.iter().map(|x| x.1).collect();
        let h = percentile(&mut v, 0.5);
        for ring in polygonize(g, &comp) {
            out.push((ring, h));
        }
        tiers(g, &sub, h, depth + 1, out);
    }
}

/// Tree crowns from canopy returns: local maxima of vegetation height, at least 6 m apart (typical street-tree spacing), clear of buildings.
/// Pulses clipping a roof edge also return twice, so a crown must be mostly canopy across ~2.5 m with few
/// hard-surface returns, and at least 2 m from every footprint. Returns (position, height above ground).
pub fn trees(g: &Grids, buildings: &[Building]) -> Vec<([f64; 2], f64)> {
    let mut owned = vec![false; g.w * g.h];
    for b in buildings {
        for k in cells_in(g, &b.outer, &b.holes) {
            owned[k] = true;
        }
    }
    let clear = dilate(g, &owned, 2.0);
    let rel = |k: usize| g.veg[k] - g.ground[k];
    let hard = |k: usize| g.roof[k] - g.ground[k] > 2.0;
    let window = |k: usize, r: i64, f: &dyn Fn(usize) -> bool| {
        let (i, j) = ((k % g.w) as i64, (k / g.w) as i64);
        let mut n = 0;
        for dz in -r..=r {
            for dx in -r..=r {
                let (a, c) = (i + dx, j + dz);
                if a >= 0 && c >= 0 && (a as usize) < g.w && (c as usize) < g.h && f(c as usize * g.w + a as usize) {
                    n += 1;
                }
            }
        }
        n
    };
    let mut cand: Vec<(usize, f32)> = (0..g.w * g.h).filter(|&k| !clear[k] && rel(k) > 4.5 && rel(k) < 40.0).map(|k| (k, rel(k))).collect();
    cand.sort_by(|a, b| b.1.total_cmp(&a.1));
    let cell = 6.0;
    let mut hash: std::collections::HashMap<(i64, i64), Vec<[f64; 2]>> = std::collections::HashMap::new();
    let mut out = vec![];
    for (k, h) in cand {
        let p = g.center(k % g.w, k / g.w);
        let key = ((p[0] / cell).floor() as i64, (p[1] / cell).floor() as i64);
        if (-1..=1).any(|dx| (-1..=1).any(|dz| hash.get(&(key.0 + dx, key.1 + dz)).is_some_and(|v| v.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < cell)))) {
            continue;
        }
        let canopy = window(k, 2, &|q| rel(q) > 2.0);
        let solid = window(k, 2, &|q| hard(q) && !(rel(q) > 2.0));
        if canopy < 15 || solid > 6 {
            continue;
        }
        hash.entry(key).or_default().push(p);
        out.push((p, h as f64));
    }
    out
}

/// Cells within `m` metres of a set cell.
fn dilate(g: &Grids, set: &[bool], m: f64) -> Vec<bool> {
    let pad = (m / RES).round() as i64;
    let mut out = set.to_vec();
    for j in 0..g.h as i64 {
        for i in 0..g.w as i64 {
            if set[j as usize * g.w + i as usize] {
                for dz in -pad..=pad {
                    for dx in -pad..=pad {
                        let (a, c) = (i + dx, j + dz);
                        if a >= 0 && c >= 0 && (a as usize) < g.w && (c as usize) < g.h {
                            out[c as usize * g.w + a as usize] = true;
                        }
                    }
                }
            }
        }
    }
    out
}

pub struct Stats {
    pub lidar: usize,
    pub kept_osm: usize,
    pub parts: usize,
    pub added: usize,
}

/// Replaces building heights with lidar measurements, adds upper tiers as parts, and adds structures OSM lacks.
pub fn apply(g: &Grids, buildings: &mut Vec<Building>, origin_elev: f64) -> Stats {
    let mut st = Stats { lidar: 0, kept_osm: 0, parts: 0, added: 0 };
    let ground_at = |p: [f64; 2]| g.cell(p[0], p[1]).map_or(origin_elev, |k| g.ground[k] as f64);
    let mut owned = vec![false; g.w * g.h];
    let mut extra = vec![];
    for b in buildings.iter_mut() {
        let cells = cells_in(g, &b.outer, &b.holes);
        for &k in &cells {
            owned[k] = true;
        }
        if cells.len() < 8 {
            continue;
        }
        let base = b.outer.iter().map(|p| ground_at(*p)).fold(f64::INFINITY, f64::min);
        let hs = rel_heights(g, &cells, base);
        if (hs.len() as f64) < cells.len() as f64 * 0.3 {
            st.kept_osm += 1;
            continue;
        }
        let mut v: Vec<f32> = hs.iter().map(|x| x.1).collect();
        let main = percentile(&mut v, 0.5);
        b.height = main as f64;
        b.min_height = 0.0;
        b.base_y = Some(base - origin_elev);
        b.height_src = "lidar".into();
        st.lidar += 1;
        let mut parts = vec![];
        tiers(g, &hs, main, 0, &mut parts);
        for (ring, h) in parts {
            extra.push(Building { osm: format!("{}#tier", b.osm), outer: ring, holes: vec![], height: h as f64, min_height: 0.0, height_src: "lidar-tier".into(), kind: b.kind.clone(), name: None, base_y: Some(base - origin_elev), part_of: Some(b.osm.clone()) });
            st.parts += 1;
        }
    }
    // structures with no OSM footprint (newer buildings): building returns outside every footprint, dilated by 1 m
    let near = dilate(g, &owned, 1.0);
    let loose: Vec<usize> = (0..g.w * g.h).filter(|&k| !near[k] && g.roof[k].is_finite() && g.roof[k] - g.ground[k] > 4.0).collect();
    let mut n = 0;
    for comp in components(g, &loose) {
        if (comp.len() as f64) * RES * RES < 80.0 {
            continue;
        }
        let base = comp.iter().map(|&k| g.ground[k] as f64).fold(f64::INFINITY, f64::min);
        let hs = rel_heights(g, &comp, base);
        if hs.len() < MIN_TIER_CELLS {
            continue;
        }
        let mut v: Vec<f32> = hs.iter().map(|x| x.1).collect();
        let main = percentile(&mut v, 0.5);
        for ring in polygonize(g, &comp) {
            n += 1;
            let id = format!("lidar/{n}");
            extra.push(Building { osm: id.clone(), outer: ring, holes: vec![], height: main as f64, min_height: 0.0, height_src: "lidar-only".into(), kind: "yes".into(), name: None, base_y: Some(base - origin_elev), part_of: None });
            st.added += 1;
            let mut parts = vec![];
            tiers(g, &hs, main, 0, &mut parts);
            for (r, h) in parts {
                extra.push(Building { osm: format!("{id}#tier"), outer: r, holes: vec![], height: h as f64, min_height: 0.0, height_src: "lidar-tier".into(), kind: "yes".into(), name: None, base_y: Some(base - origin_elev), part_of: Some(id.clone()) });
                st.parts += 1;
            }
        }
    }
    buildings.extend(extra);
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utm_inverse_matches_a_known_point() {
        // the SW corner of tile 11SMS484618, per its USGS bounding box
        let (la, lo) = utm_to_latlon(484_000.0, 3_618_000.0, 11);
        assert!((la - 32.699616).abs() < 2e-5 && (lo + 117.170718).abs() < 2e-5, "{la} {lo}");
    }

    /// Roof returns from tile 484618 should land inside OSM footprints when the frame conversion is right.
    #[test]
    fn roofs_align_with_osm_footprints() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tile = root.join("data/raw/lidar/USGS_LPC_CA_SanDiegoCo_D24_11SMS484618.laz");
        let Ok(city) = crate::City::load(&root.join("data/city/downtown_sd.json")) else { return };
        if !tile.exists() {
            return;
        }
        let rect = [-500.0, -400.0, 0.0, 100.0];
        let g = rasterize(&[tile], &city.frame, rect).unwrap();
        let mut inside = vec![false; g.w * g.h];
        for b in &city.buildings {
            for k in cells_in(&g, &b.outer, &b.holes) {
                inside[k] = true;
            }
        }
        let roof: Vec<usize> = (0..g.w * g.h).filter(|&k| g.roof[k] - g.ground[k] > 6.0).collect();
        let hit = roof.iter().filter(|&&k| inside[k]).count() as f64 / roof.len().max(1) as f64;
        eprintln!("roof cells (>6 m) {} inside OSM footprints {:.1}%", roof.len(), hit * 100.0);
        assert!(roof.len() > 10_000 && hit > 0.8);
    }
}
