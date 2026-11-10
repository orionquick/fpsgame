//! Voxel flood-fill GI, CPU side: voxelizes the bespoke buildings and the ground around the player and injects
//! the light that seeds propagation (render::gi runs the propagation on the GPU).
//!
//! - Cells are solid when enough surface area passes through them (so thin mullions and glass leave windows open).
//! - Direct sky light is traced on the GPU through these voxels (render::gi's skylight pass).
//! - Solid cells facing an open neighbour reflect the sun (traced through the grid) and nearby lamps.
//!
//! Light then pours in through the window openings, bounces off sunlit floors and lamp-lit surfaces, and fades
//! into the depth of a room on its own.

use crate::bespoke::geom::{Key, Mat};
use glam::Vec3;
use render::gi::GI_DIM;
use std::collections::HashMap;

pub const CELL: f32 = 0.5;
const SUN_COL: Vec3 = Vec3::from_array(render::SUN_COL);
const GROUND_ALBEDO: Vec3 = Vec3::new(0.28, 0.27, 0.25);

pub struct Tri {
    pub p: [Vec3; 3],
    pub albedo: Vec3,
}

/// A city building as a solid prism (footprint, bottom, top): blocks sky and reflects sun onto the street.
pub struct Prism {
    pub ring: Vec<[f32; 2]>,
    pub y0: f32,
    pub y1: f32,
}

const CITY_ALBEDO: Vec3 = Vec3::new(0.36, 0.33, 0.3);

/// The city's buildings around the bespoke ones (which are voxelized from their real geometry).
pub fn prisms(c: &city::City) -> Vec<Prism> {
    c.buildings
        .iter()
        .filter(|b| !crate::bespoke::OSM_IDS.contains(&b.osm.as_str()) && b.outer.len() >= 3)
        .map(|b| {
            let base = b.base_y.unwrap_or_else(|| c.terrain.at(b.outer[0][0], b.outer[0][1])) as f32;
            Prism { ring: b.outer.iter().map(|p| [p[0] as f32, p[1] as f32]).collect(), y0: base + b.min_height as f32, y1: base + b.height as f32 }
        })
        .collect()
}

fn in_ring(ring: &[[f32; 2]], x: f32, z: f32) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub struct Field {
    pub origin: Vec3,
    pub vox: Vec<[u8; 4]>,
    pub emit: Vec<[f32; 4]>,
}

/// Mean linear albedo of a surface's base colour (cached on disk next to the BC texture cache).
fn surface_albedo(lib: &crate::library::Library, id: &str, cache: &mut HashMap<String, Vec3>) -> Vec3 {
    if let Some(&a) = cache.get(id) {
        return a;
    }
    let path = crate::root().join("data/assets/texcache").join(format!("{id}.avg"));
    let parsed = std::fs::read_to_string(&path).ok().and_then(|s| {
        let v: Vec<f32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
    });
    let a = parsed.unwrap_or_else(|| {
        let a = lib
            .catalog
            .get(id)
            .and_then(|asset| assets::load_material(&lib.inbox, asset, 32).ok())
            .map(|m| {
                let mut s = Vec3::ZERO;
                for p in m.base.pixels() {
                    s += Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32).map(|c| (c / 255.0).powf(2.2));
                }
                s / (m.base.width() * m.base.height()) as f32
            })
            .unwrap_or(Vec3::splat(0.4));
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, format!("{} {} {}", a.x, a.y, a.z));
        a
    });
    cache.insert(id.to_string(), a);
    a
}

/// The bespoke buildings' surfaces as albedo-tagged triangles (glass and light panels left out).
pub fn tris(lib: &crate::library::Library, c: &city::City) -> Vec<Tri> {
    let mut cache = HashMap::new();
    let mut out = vec![];
    for b in crate::bespoke::all(c) {
        for (key, (v, idx)) in &b.geo.groups {
            let albedo = match key_albedo(lib, key, &mut cache) {
                Some(a) => a,
                None => continue,
            };
            for t in idx.chunks_exact(3) {
                out.push(Tri { p: [0, 1, 2].map(|k| Vec3::from(v[t[k] as usize].pos)), albedo });
            }
        }
    }
    out
}

fn key_albedo(lib: &crate::library::Library, key: &Key, cache: &mut HashMap<String, Vec3>) -> Option<Vec3> {
    let tint = Vec3::from(key.tint());
    let a = match key.mat {
        Mat::Surface(id) => surface_albedo(lib, id, cache) * tint,
        Mat::Plain(_) => tint,
        Mat::Glass | Mat::Light | Mat::Screen(_) => return None,
    };
    Some(a.clamp(Vec3::splat(0.02), Vec3::splat(0.9)))
}

/// Voxelizes `tris` and the terrain in the GI volume around `center` and injects sky, sun and lamp light.
pub fn build(tris: &[Tri], city: &[Prism], terrain: impl Fn(f32, f32) -> f32, lights: &[render::PointLight], sun: Vec3, center: Vec3) -> Field {
    let [nx, ny, nz] = GI_DIM.map(|d| d as usize);
    let half = Vec3::new(nx as f32, 0.0, nz as f32) * CELL * 0.5;
    // inside (or near) a bespoke building that fits the volume: centre on the building so all of it is covered
    let (mut bmin, mut bmax) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for t in tris {
        for p in t.p {
            bmin = bmin.min(p);
            bmax = bmax.max(p);
        }
    }
    let center = if !tris.is_empty() && center.cmpge(bmin - 4.0).all() && center.cmple(bmax + 4.0).all() && (bmax - bmin).x < half.x * 2.0 - 4.0 && (bmax - bmin).z < half.z * 2.0 - 4.0 {
        let c = (bmin + bmax) * 0.5;
        Vec3::new(c.x, center.y, c.z)
    } else {
        center
    };
    let snap = |v: f32| (v / CELL).floor() * CELL;
    let origin = Vec3::new(snap(center.x - half.x), snap(terrain(center.x, center.z) - 2.0), snap(center.z - half.z));
    let top = origin.y + ny as f32 * CELL;
    let idx = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    let n = nx * ny * nz;
    let mut area = vec![0f32; n];
    let mut alb = vec![Vec3::ZERO; n];
    // surface area per column above the volume (roofs higher than its top still shade the sky)
    let mut above = vec![0f32; nx * nz];
    let step = CELL * 0.25;
    for t in tris {
        let (e1, e2) = (t.p[1] - t.p[0], t.p[2] - t.p[0]);
        let a = 0.5 * e1.cross(e2).length();
        if a < 1e-6 {
            continue;
        }
        let (mn, mx) = (t.p[0].min(t.p[1]).min(t.p[2]), t.p[0].max(t.p[1]).max(t.p[2]));
        if mx.x < origin.x || mx.z < origin.z || mn.x > origin.x + nx as f32 * CELL || mn.z > origin.z + nz as f32 * CELL || mx.y < origin.y {
            continue;
        }
        let edge = e1.length().max(e2.length()).max((t.p[2] - t.p[1]).length());
        let k = (edge / step).ceil().max(1.0) as usize;
        let sub = a / (k * k) as f32;
        let mut add = |u: f32, v: f32| {
            let p = t.p[0] + e1 * u + e2 * v;
            let c = (p - origin) / CELL;
            let (x, z) = (c.x.floor() as isize, c.z.floor() as isize);
            if x < 0 || z < 0 || x >= nx as isize || z >= nz as isize {
                return;
            }
            let y = c.y.floor() as isize;
            if y >= ny as isize {
                above[z as usize * nx + x as usize] += sub;
            } else if y >= 0 {
                let i = idx(x as usize, y as usize, z as usize);
                area[i] += sub;
                alb[i] += t.albedo * sub;
            }
        };
        let kf = k as f32;
        for i in 0..k {
            for j in 0..k - i {
                add((i as f32 + 1.0 / 3.0) / kf, (j as f32 + 1.0 / 3.0) / kf);
                if i + j + 1 < k {
                    add((i as f32 + 2.0 / 3.0) / kf, (j as f32 + 2.0 / 3.0) / kf);
                }
            }
        }
    }
    let face = CELL * CELL;
    // city buildings: solid prisms (their walls count as surface area so they block and reflect like geometry)
    let (gx1, gz1) = (origin.x + nx as f32 * CELL, origin.z + nz as f32 * CELL);
    for b in city {
        let (mut x0, mut z0, mut x1, mut z1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &b.ring {
            (x0, z0, x1, z1) = (x0.min(p[0]), z0.min(p[1]), x1.max(p[0]), z1.max(p[1]));
        }
        if x1 < origin.x || z1 < origin.z || x0 > gx1 || z0 > gz1 {
            continue;
        }
        let cx = |v: f32| (((v - origin.x) / CELL).floor().max(0.0) as usize).min(nx - 1);
        let cz = |v: f32| (((v - origin.z) / CELL).floor().max(0.0) as usize).min(nz - 1);
        for z in cz(z0)..=cz(z1) {
            for x in cx(x0)..=cx(x1) {
                if !in_ring(&b.ring, origin.x + (x as f32 + 0.5) * CELL, origin.z + (z as f32 + 0.5) * CELL) {
                    continue;
                }
                if b.y1 > top {
                    above[z * nx + x] += face;
                }
                for y in 0..ny {
                    let cy = origin.y + (y as f32 + 0.5) * CELL;
                    if cy >= b.y0 && cy <= b.y1 {
                        let i = idx(x, y, z);
                        area[i] += face;
                        alb[i] += CITY_ALBEDO * face;
                    }
                }
            }
        }
    }
    // sun rays stop at partly filled cells too (thin wall strips over windows, frames)
    let blocks: Vec<bool> = area.iter().map(|&a| a >= 0.25 * face).collect();
    let mut solid = vec![false; n];
    let mut albedo = vec![Vec3::ZERO; n];
    for z in 0..nz {
        for x in 0..nx {
            let ground = terrain(origin.x + (x as f32 + 0.5) * CELL, origin.z + (z as f32 + 0.5) * CELL);
            for y in 0..ny {
                let i = idx(x, y, z);
                if area[i] >= 0.7 * face {
                    solid[i] = true;
                    albedo[i] = alb[i] / area[i];
                } else if origin.y + (y as f32 + 0.5) * CELL < ground {
                    solid[i] = true;
                    albedo[i] = GROUND_ALBEDO;
                }
            }
        }
    }
    let mut emit = vec![[0f32; 4]; n];
    let covered_col: Vec<bool> = above.iter().map(|&a| a > 0.5 * face).collect();
    let inside = |p: Vec3| -> Option<usize> {
        let c = (p - origin) / CELL;
        if c.cmplt(Vec3::ZERO).any() || c.x >= nx as f32 || c.y >= ny as f32 || c.z >= nz as f32 {
            return None;
        }
        Some(idx(c.x as usize, c.y as usize, c.z as usize))
    };
    // true when nothing solid lies between `p` and the sun (leaving through the top under a roof counts as shade)
    let sunlit = |p: Vec3| -> bool {
        let mut q = p;
        loop {
            q += sun * (CELL * 0.25);
            match inside(q) {
                Some(i) if solid[i] || blocks[i] => return false,
                Some(_) => {}
                None => {
                    if q.y >= top {
                        let c = ((q - origin) / CELL).floor();
                        if c.x >= 0.0 && c.z >= 0.0 && (c.x as usize) < nx && (c.z as usize) < nz {
                            return !covered_col[c.z as usize * nx + c.x as usize];
                        }
                    }
                    return true;
                }
            }
        }
    };
    let visible = |a: Vec3, b: Vec3| -> bool {
        let d = b - a;
        let steps = (d.length() / (CELL * 0.5)).ceil() as usize;
        (1..steps).all(|s| inside(a + d * (s as f32 / steps as f32)).is_none_or(|i| !solid[i]))
    };
    let dirs = [(1i32, 0i32, 0i32), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let i = idx(x, y, z);
                if !solid[i] {
                    continue;
                }
                let centre = origin + (Vec3::new(x as f32, y as f32, z as f32) + 0.5) * CELL;
                let mut sun_e = 0f32;
                let mut lamp = Vec3::ZERO;
                let mut open = false;
                for &(dx, dy, dz) in &dirs {
                    let (qx, qy, qz) = (x as i32 + dx, y as i32 + dy, z as i32 + dz);
                    if qx < 0 || qy < 0 || qz < 0 || qx >= nx as i32 || qy >= ny as i32 || qz >= nz as i32 {
                        continue;
                    }
                    if solid[idx(qx as usize, qy as usize, qz as usize)] {
                        continue;
                    }
                    open = true;
                    let nf = Vec3::new(dx as f32, dy as f32, dz as f32);
                    let air = centre + nf * CELL;
                    let c = nf.dot(sun);
                    if c > sun_e && sunlit(air) {
                        sun_e = c;
                    }
                }
                if !open {
                    continue;
                }
                for l in lights {
                    let lp = Vec3::from(l.pos);
                    let d = lp.distance(centre);
                    if d > l.radius {
                        continue;
                    }
                    // aim at the open face nearest the lamp
                    let toward = (lp - centre).normalize_or_zero();
                    let probe = centre + toward * CELL * 0.75;
                    if !visible(probe, lp) {
                        continue;
                    }
                    let xr = d / l.radius;
                    // downlights light little above themselves (as in the shader)
                    let up = ((toward.y + 0.35) / 0.85).clamp(0.0, 1.0);
                    let down = 0.12 + 0.88 * up * up * (3.0 - 2.0 * up);
                    let fall = (1.0 - xr * xr).powi(2) / (1.0 + d * d) * (1.0 + (down - 1.0) * l._pad);
                    lamp += Vec3::from(l.color) * fall * 0.7;
                }
                let e = albedo[i] * (SUN_COL * sun_e + lamp);
                emit[i] = [e.x, e.y, e.z, 0.0];
            }
        }
    }
    let vox = (0..n)
        .map(|i| {
            let a = (albedo[i] * 255.0).round().clamp(Vec3::ZERO, Vec3::splat(255.0));
            [a.x as u8, a.y as u8, a.z as u8, if solid[i] { 255 } else { 0 }]
        })
        .collect();
    Field { origin, vox, emit }
}

impl Field {
    /// Debug image of the horizontal slice at height `y`: solid cells in their albedo, sky cells blue, lit
    /// surface cells by injected light.
    pub fn dump(&self, y: f32, path: &std::path::Path) {
        let [nx, ny, nz] = GI_DIM.map(|d| d as usize);
        let yy = (((y - self.origin.y) / CELL) as usize).min(ny - 1);
        let s = 6;
        let mut img = image::RgbImage::new((nx * s) as u32, (nz * s) as u32);
        for z in 0..nz {
            for x in 0..nx {
                let i = (z * ny + yy) * nx + x;
                let (v, e) = (self.vox[i], self.emit[i]);
                let c = if e[3] > 0.5 {
                    [90, 140, 220]
                } else if v[3] > 0 {
                    let k = |a: u8, b: f32| ((a as f32 * 0.6) + (b * 120.0)).min(255.0) as u8;
                    [k(v[0], e[0]), k(v[1], e[1]), k(v[2], e[2])]
                } else {
                    [20, 20, 20]
                };
                for dz in 0..s {
                    for dx in 0..s {
                        img.put_pixel((x * s + dx) as u32, (z * s + dz) as u32, image::Rgb(c));
                    }
                }
            }
        }
        let _ = img.save(path);
    }
}
