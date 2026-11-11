//! Hand-authored buildings, matched one at a time to street-level reference: real facade geometry
//! (openings, reveals, sashes, trim) and a generated interior (offices, corridors, core, stairs, lift).
pub mod factory;
pub mod furnish;
pub mod beerloft;
pub mod geom;
pub mod loft;
pub mod petco;
pub mod plan;

use geom::{Geo, Hole, Key, Mat};
use glam::{Vec2, Vec3};
use plan::{Plan, Rect, Use};
use render::{Batch, Gpu, Instance, Material, Renderer};
use std::collections::HashMap;
use std::sync::Arc;

/// Interior surfaces (Megascans ids).
pub mod m {
    pub const DRYWALL: &str = "sbqktep0";
    pub const CORE_WALL: &str = "sbqktep0";
    pub const PAINTED_BRICK: &str = "ubrjbcjmw";
    pub const CARPET: &str = "tesrd2l";
    pub const CORRIDOR_FLOOR: &str = "ue2iai3fw";
    pub const TILE_FLOOR: &str = "vlxjaed";
    pub const CONCRETE: &str = "tezmdjno";
    /// Poly Haven concrete_floor_02 (CC0)
    pub const POLISHED_CONCRETE: &str = "pht_concrete_floor_02";
    pub const CAST_CONCRETE: &str = "thfkebpfw";
    pub const ACOUSTIC_CEILING: &str = "sl4qlkd";
    pub const CONCRETE_CEILING: &str = "ub4ndfao";
    pub const WOOD: &str = "ucklejqdy";
    pub const ROOF_GRAVEL: &str = "ugzmbcrn";
}

/// Everything one bespoke building contributes: draw groups and collision.
pub struct Building {
    pub osm: &'static str,
    pub geo: Geo,
    /// Megascans furniture (instanced, lit as interior).
    pub furniture: Vec<crate::library::Placement>,
}

/// OSM ids built here instead of by the generic prefab assembler.
pub const OSM_IDS: &[&str] = &[factory::OSM, petco::OSM, petco::WESTERN_METAL];

/// Whether a city building is replaced by a bespoke one: listed ids, parts of them, and anything standing inside
/// the ballpark's outline or (lidar-derived only) within 30 m of it: its concourse blocks, gates, plaza canopies.
pub fn claimed(c: &city::City, b: &city::model::Building) -> bool {
    if OSM_IDS.contains(&b.osm.as_str()) || b.part_of.as_deref().is_some_and(|p| OSM_IDS.contains(&p)) {
        return true;
    }
    static RING: std::sync::OnceLock<Vec<[f64; 2]>> = std::sync::OnceLock::new();
    let ring = RING.get_or_init(|| c.buildings.iter().find(|x| x.osm == petco::OSM).map(|x| x.outer.clone()).unwrap_or_default());
    if ring.len() < 3 || b.outer.is_empty() {
        return false;
    }
    let n = b.outer.len() as f64;
    let p = [b.outer.iter().map(|q| q[0]).sum::<f64>() / n, b.outer.iter().map(|q| q[1]).sum::<f64>() / n];
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, q) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (q[1] > p[1]) && p[0] < (q[0] - a[0]) * (p[1] - a[1]) / (q[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    if inside {
        return true;
    }
    // lidar-derived blocks hugging the outline are its plaza structures and canopy, not buildings of their own
    b.osm.starts_with("lidar/")
        && ring.iter().zip(ring.iter().cycle().skip(1)).any(|(a, q)| {
            let (dx, dz) = (q[0] - a[0], q[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / (dx * dx + dz * dz).max(1e-9)).clamp(0.0, 1.0);
            let (cx, cz) = (a[0] + dx * t - p[0], a[1] + dz * t - p[1]);
            cx * cx + cz * cz < 30.0 * 30.0
        })
}

/// All bespoke buildings in the city (only the ones already matched to reference).
pub fn all(c: &city::City) -> Vec<Building> {
    let cat = assets::Catalog::load(&crate::root().join("data/assets")).unwrap_or_default();
    factory::build(c, &cat).into_iter().chain(petco::build(c, &cat)).collect()
}

/// World plan rings where hand-built ground replaces the city's (the ballpark's field).
pub fn ground_holes() -> Vec<Vec<[f64; 2]>> {
    vec![petco::field_ring(), petco::plaza_ring()]
}

pub const PARTITION_T: f32 = 0.12;
pub const CORE_T: f32 = 0.2;
pub const DOOR_H: f32 = 2.15;

fn k(mat: &'static str, tint: [f32; 3]) -> Key {
    Key::new(Mat::Surface(mat), tint).inside()
}

/// One floor of interior: partitions with doors, floor finishes, dropped ceiling with light panels, door leaves.
/// `y` is the finished floor level, `ceil` the ceiling height above it.
/// `loft`: open timber-ceilinged floor: painted core walls, polished concrete, ceilings only inside the core.
/// `wood_core`: clad the core in dark timber (the cafe floor), else painted.
/// `cut`: openings in this floor (a feature stair's well) that the floor finishes stop at.
pub fn interior(g: &mut Geo, p: &Plan, y: f32, ceil: f32, seed: u64, loft: bool, wood_core: bool, cut: &[Rect]) {
    let white = [1.4, 1.4, 1.36];
    for (n, w) in p.walls.iter().enumerate() {
        let (mat, t, tint) = if w.core { (m::CORE_WALL, CORE_T, [1.05, 1.06, 1.08]) } else { (m::DRYWALL, PARTITION_T, white) };
        let key = if loft && w.core && wood_core {
            Key::new(Mat::Surface("pht_dark_wooden_planks"), [1.1, 1.0, 0.95]).inside()
        } else if loft && w.core {
            Key::new(Mat::Plain(215), [0.8, 0.79, 0.76]).inside()
        } else {
            k(mat, tint)
        };
        let d = w.b - w.a;
        let l = d.length();
        if l < 0.05 {
            continue;
        }
        let dir = d / l;
        let nrm = Vec2::new(-dir.y, dir.x); // the wall function's outward side
        // centre the wall on its line: the outer face sits half a thickness out
        let off = nrm * (t / 2.0);
        let holes: Vec<Hole> = w.doors.iter().map(|&u| Hole { u0: u - plan::DOOR_W / 2.0, u1: u + plan::DOOR_W / 2.0, y0: y, y1: y + DOOR_H }).collect();
        let top = y + ceil;
        g.wall(key, key, w.a + off, w.b + off, y, top, t, &holes, true);
        // door leaves, mostly open, hinged on the u0 jamb, swinging to the outward side
        for (j, &u) in w.doors.iter().enumerate() {
            let r = rnd(seed ^ n as u64, j as u64);
            let ang = if r < 0.15 { 0.0 } else { (70.0 + 40.0 * r).to_radians() };
            let hinge = w.a + dir * (u - plan::DOOR_W / 2.0 + 0.03) + off;
            let leaf = dir * ang.cos() + nrm * ang.sin();
            let wood = Key::new(Mat::Plain(110), [0.36, 0.25, 0.16]).inside();
            g.beam(wood, hinge + nrm * 0.03, hinge + nrm * 0.03 + leaf * (plan::DOOR_W - 0.06), 0.022, y, y + DOOR_H - 0.02);
        }
    }
    let ce = Key::new(Mat::Light, [1.0, 0.97, 0.9]);
    for (n, r) in p.rooms.iter().enumerate() {
        let floor = match r.kind {
            Use::Office | Use::Meeting => k(m::CARPET, if r.kind == Use::Meeting { [0.95, 1.0, 1.1] } else { [1.1, 1.1, 1.12] }),
            Use::Corridor | Use::Lobby => k(m::CORRIDOR_FLOOR, [1.0, 1.0, 1.0]),
            Use::Restroom => k(m::TILE_FLOOR, [1.0, 1.0, 1.0]),
            Use::Hall => {
                stone_slabs(g, r.r, y, seed ^ n as u64, cut);
                Key::new(Mat::Plain(0), [0.0; 3])
            }
            Use::Storage | Use::Riser => k(m::CONCRETE, [0.7, 0.7, 0.7]),
            Use::Stair | Use::Elevator => continue,
        };
        let rr = r.r;
        if r.kind != Use::Hall {
            for q in minus(rr, cut) {
                g.quad(floor, Vec3::new(q.x0, y, q.z1), Vec3::X * q.w(), Vec3::NEG_Z * q.d());
            }
        }
        // ceiling (faces down); a loft only closes the rooms inside the core
        let closed = matches!(r.kind, Use::Restroom | Use::Riser | Use::Storage);
        if loft && !closed {
            continue;
        }
        let ck = if loft { Key::new(Mat::Plain(220), [0.82, 0.82, 0.8]).inside() } else { k(m::ACOUSTIC_CEILING, [1.0, 1.0, 1.0]) };
        g.quad(ck, Vec3::new(rr.x0, y + ceil, rr.z0), Vec3::X * rr.w(), Vec3::Z * rr.d());
        // light panels on a grid, kept off the walls
        if matches!(r.kind, Use::Riser) {
            continue;
        }
        let (sx, sz) = (2.4, 2.4);
        let nx = ((rr.w() - 1.2) / sx).floor().max(0.0) as i32 + 1;
        let nz = ((rr.d() - 1.2) / sz).floor().max(0.0) as i32 + 1;
        let (lw, ld) = if r.kind == Use::Corridor { (0.3, 1.2) } else { (0.6, 1.2) };
        let along_x = rr.w() > rr.d();
        let (lw, ld) = if along_x { (ld, lw) } else { (lw, ld) };
        let ox = rr.x0 + (rr.w() - (nx - 1) as f32 * sx) / 2.0;
        let oz = rr.z0 + (rr.d() - (nz - 1) as f32 * sz) / 2.0;
        for ix in 0..nx {
            for iz in 0..nz {
                let (cx, cz) = (ox + ix as f32 * sx, oz + iz as f32 * sz);
                if cx - lw / 2.0 < rr.x0 + 0.2 || cx + lw / 2.0 > rr.x1 - 0.2 || cz - ld / 2.0 < rr.z0 + 0.2 || cz + ld / 2.0 > rr.z1 - 0.2 {
                    continue;
                }
                g.quad(ce, Vec3::new(cx - lw / 2.0, y + ceil - 0.01, cz - ld / 2.0), Vec3::X * lw, Vec3::Z * ld);
            }
        }
        let _ = n;
    }
}

/// Floor slab under level `y` covering `area`, with holes (stair, lift shaft); collidable.
pub fn slab(g: &mut Geo, area: Rect, y: f32, thick: f32, holes: &[Rect]) {
    let key = k(m::CAST_CONCRETE, [0.8, 0.8, 0.8]);
    let mut xs = vec![area.x0, area.x1];
    let mut zs = vec![area.z0, area.z1];
    for h in holes {
        xs.extend([h.x0.clamp(area.x0, area.x1), h.x1.clamp(area.x0, area.x1)]);
        zs.extend([h.z0.clamp(area.z0, area.z1), h.z1.clamp(area.z0, area.z1)]);
    }
    for v in [&mut xs, &mut zs] {
        v.sort_by(f32::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    }
    for i in 0..xs.len() - 1 {
        for j in 0..zs.len() - 1 {
            let c = Vec2::new((xs[i] + xs[i + 1]) / 2.0, (zs[j] + zs[j + 1]) / 2.0);
            if holes.iter().any(|h| c.x > h.x0 && c.x < h.x1 && c.y > h.z0 && c.y < h.z1) {
                continue;
            }
            g.solid(key, Vec3::new(xs[i], y - thick, zs[j]), Vec3::new(xs[i + 1], y - 0.003, zs[j + 1]));
        }
    }
}

/// Region of a floor plate cut away for the stair flights from the floor below (the landing at the door stays).
pub fn stair_hole(s: Rect) -> Rect {
    Rect::new(s.x0 + STAIR_LANDING, s.z0, s.x1, s.z1)
}

pub const STAIR_LANDING: f32 = 1.3;

/// Switchback stair from level `y` up `h`: first flight eastwards on the north half, landing, back west.
pub fn stair(g: &mut Geo, s: Rect, y: f32, h: f32) {
    let key = k(m::CAST_CONCRETE, [0.85, 0.85, 0.85]);
    let rail = Key::new(Mat::Plain(90), [0.06, 0.06, 0.065]).inside();
    let l = STAIR_LANDING;
    let zm = (s.z0 + s.z1) / 2.0;
    let run = s.w() - 2.0 * l;
    let half = h / 2.0;
    let n = (half / 0.18).ceil() as usize;
    let (rise, tread) = (half / n as f32, run / n as f32);
    for i in 0..n {
        let top = y + rise * (i + 1) as f32;
        // flight 1: x from x0+l eastwards on z0..zm-0.1
        let xa = s.x0 + l + tread * i as f32;
        g.solid(key, Vec3::new(xa, y, s.z0 + 0.02), Vec3::new(xa + tread, top, zm - 0.1));
        // flight 2: x from x1-l westwards on zm+0.1..z1, starting at the landing
        let xb = s.x1 - l - tread * i as f32;
        g.solid(key, Vec3::new(xb - tread, y + half, zm + 0.1), Vec3::new(xb, top + half, s.z1 - 0.02));
    }
    // mid landing
    g.solid(key, Vec3::new(s.x1 - l, y + half - 0.25, s.z0 + 0.02), Vec3::new(s.x1 - 0.02, y + half, s.z1 - 0.02));
    // spine wall between the flights
    let wall = k(m::CORE_WALL, [0.9, 0.9, 0.88]);
    g.solid(wall, Vec3::new(s.x0 + l, y, zm - 0.1), Vec3::new(s.x1 - l, y + h, zm + 0.1));
    // handrails on the spine wall
    for (z, sgn) in [(zm - 0.14, 1.0f32), (zm + 0.14, -1.0)] {
        let (ya, yb) = if sgn > 0.0 { (y + 0.9, y + half + 0.9) } else { (y + h + 0.9, y + half + 0.9) };
        let a = Vec3::new(s.x0 + l, ya, z);
        let b = Vec3::new(s.x1 - l, yb, z);
        let d = b - a;
        let up = Vec3::Y * 0.05;
        g.quad(rail, a, d, up);
        g.quad(rail, a + up, d, Vec3::new(0.0, 0.0, 0.04 * sgn));
    }
}

/// Lift: shaft walls come from the core partitions; per floor a pair of brushed doors in the core front.
pub fn lift_doors(g: &mut Geo, e: Rect, y: f32) {
    let steel = Key::new(Mat::Plain(70), [0.62, 0.63, 0.64]).inside();
    let frame = Key::new(Mat::Plain(90), [0.35, 0.35, 0.36]).inside();
    let zc = e.centre().y;
    let x = e.x0 - CORE_T / 2.0 - 0.012;
    g.cuboid(steel, Vec3::new(x, y, zc - 0.5), Vec3::new(x + 0.02, y + 2.1, zc - 0.005));
    g.cuboid(steel, Vec3::new(x, y, zc + 0.005), Vec3::new(x + 0.02, y + 2.1, zc + 0.5));
    g.cuboid(frame, Vec3::new(x - 0.02, y + 2.1, zc - 0.62), Vec3::new(x + 0.02, y + 2.24, zc + 0.62));
    g.cuboid(frame, Vec3::new(x - 0.02, y, zc - 0.62), Vec3::new(x + 0.02, y + 2.1, zc - 0.5));
    g.cuboid(frame, Vec3::new(x - 0.02, y, zc + 0.5), Vec3::new(x + 0.02, y + 2.1, zc + 0.62));
    // the doors are closed: block the opening
    g.block_aabb(Vec3::new(x - 0.02, y, zc - 0.6), Vec3::new(x + 0.2, y + 2.2, zc + 0.6));
}

/// Honed concrete slabs on the building grid (the factory hall's floor): ~1.5 m stones with hairline light joints.
/// Each slab is its own quad inset over a pale grout plane, with its own patch of the texture, quarter-turn and
/// slight tint so the floor reads as laid stone rather than one printed sheet.
fn stone_slabs(g: &mut Geo, r: plan::Rect, y: f32, seed: u64, cut: &[Rect]) {
    const SLAB: f32 = 1.52;
    const JOINT: f32 = 0.0025;
    let grout = Key::new(Mat::Plain(200), [0.8, 0.79, 0.74]).inside();
    for q in minus(r, cut) {
        g.quad(grout, Vec3::new(q.x0, y - 0.002, q.z1), Vec3::X * q.w(), Vec3::NEG_Z * q.d());
    }
    // the texture averages ~0.13 (dark, warm); honed light concrete is ~0.35-0.4
    // cream-grey (as in the photos): the texture is warm brown, so lift green and blue a little less than red
    let tints = [[3.05, 3.3, 3.25], [3.2, 3.42, 3.35], [2.9, 3.15, 3.1], [3.12, 3.35, 3.22]];
    let nx = (r.w() / SLAB).ceil() as i32;
    let nz = (r.d() / SLAB).ceil() as i32;
    for i in 0..nx {
        for j in 0..nz {
            let x0 = r.x0 + i as f32 * SLAB;
            let z0 = r.z0 + j as f32 * SLAB;
            let x1 = (x0 + SLAB).min(r.x1);
            let z1 = (z0 + SLAB).min(r.z1);
            let h = (i as u64) << 20 | j as u64;
            let t = tints[(rnd(seed, h) * 4.0) as usize % 4];
            let key = Key::new(Mat::Surface("pht_brushed_concrete"), t).inside().tile(2.0).rough(0.5);
            let (a, b) = (x0 + if i > 0 { JOINT } else { 0.0 }, z0 + if j > 0 { JOINT } else { 0.0 });
            let (c, d) = (x1 - if x1 < r.x1 { JOINT } else { 0.0 }, z1 - if z1 < r.z1 { JOINT } else { 0.0 });
            let uv = [rnd(seed, h ^ 0xa1) * 7.0, rnd(seed, h ^ 0xb2) * 7.0];
            let turns = (rnd(seed, h ^ 0xc3) * 4.0) as u32;
            for q in minus(Rect::new(a, b, c, d), cut) {
                // a slab cut by an opening keeps its own patch of the texture
                let uq = [uv[0] + (q.x0 - a), uv[1] + (d - q.z1)];
                g.quad_uv(key, Vec3::new(q.x0, y, q.z1), Vec3::X * q.w(), Vec3::NEG_Z * q.d(), uq, turns);
            }
        }
    }
}

/// The parts of `r` outside every rectangle in `cut` (a grid split on the cut edges).
fn minus(r: Rect, cut: &[Rect]) -> Vec<Rect> {
    let hit = |c: &Rect| c.x0 < r.x1 && c.x1 > r.x0 && c.z0 < r.z1 && c.z1 > r.z0;
    if !cut.iter().any(hit) {
        return vec![r];
    }
    let mut xs = vec![r.x0, r.x1];
    let mut zs = vec![r.z0, r.z1];
    for c in cut.iter().filter(|c| hit(c)) {
        xs.extend([c.x0.clamp(r.x0, r.x1), c.x1.clamp(r.x0, r.x1)]);
        zs.extend([c.z0.clamp(r.z0, r.z1), c.z1.clamp(r.z0, r.z1)]);
    }
    for v in [&mut xs, &mut zs] {
        v.sort_by(f32::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    }
    let mut out = vec![];
    for i in 0..xs.len() - 1 {
        for j in 0..zs.len() - 1 {
            let (cx, cz) = ((xs[i] + xs[i + 1]) / 2.0, (zs[j] + zs[j + 1]) / 2.0);
            if !cut.iter().any(|c| cx > c.x0 && cx < c.x1 && cz > c.z0 && cz < c.z1) {
                out.push(Rect::new(xs[i], zs[j], xs[i + 1], zs[j + 1]));
            }
        }
    }
    out
}

pub(crate) fn rnd(seed: u64, i: u64) -> f32 {
    let mut x = seed ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x51ab;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}

/// GPU batches for a building: one per material key, drawn with an identity instance.
/// Every bespoke building's lamps.
pub fn all_lights(c: &city::City) -> Vec<render::PointLight> {
    all(c).into_iter().flat_map(|b| b.geo.lights).collect()
}

/// Every bespoke building's reflection probes (capture point, box min, box max).
pub fn all_probes(c: &city::City) -> Vec<[glam::Vec3; 3]> {
    all(c).into_iter().flat_map(|b| b.geo.probes).collect()
}

/// Undecorated mode (FPS_UNDECORATED): structure and finishes only, for judging materials against photos.
pub fn undecorated() -> bool {
    std::env::var("FPS_UNDECORATED").is_ok()
}

pub fn all_batches(gpu: &Gpu, r: &Renderer, lib: &mut crate::library::Library, c: &city::City) -> Vec<Batch> {
    let mut out = vec![];
    for b in all(c) {
        out.extend(batches(gpu, r, lib, &b.geo));
        if std::env::var("FPS_NO_FURNITURE").is_err() {
            let only: Option<Vec<String>> = std::env::var("FPS_FURN_ONLY").ok().map(|v| v.split(',').map(String::from).collect());
            let fur: Vec<crate::library::Placement> = b.furniture.into_iter().filter(|p| only.as_ref().is_none_or(|o| o.contains(&p.asset))).collect();
            out.extend(crate::library::prefab_batches(gpu, r, lib, &fur));
        }
    }
    out
}

pub fn batches(gpu: &Gpu, r: &Renderer, lib: &mut crate::library::Library, g: &Geo) -> Vec<Batch> {
    let mut out = vec![];
    let mut plain: HashMap<u8, Arc<Material>> = HashMap::new();
    let ids: Vec<String> = g.groups.keys().filter_map(|k| match k.mat {
        Mat::Surface(id) | Mat::Screen(id) => Some(id.to_string()),
        _ => None,
    }).collect();
    lib.preload(gpu, r, &ids);
    for (key, (v, idx)) in &g.groups {
        let tint = key.tint();
        let gloss = key.gloss as f32 / 100.0;
        // params.x carries the edge-wear kind in tenths (still "lit" mode 0 to the shader)
        let w = key.wear as f32 * 0.1;
        let indoor = if key.indoor { [w, crate::library::INDOOR_AMBIENT, crate::library::INDOOR_LAMP, gloss] } else { [w, 0.0, 0.0, gloss] };
        let (mat, inst, transparent) = match key.mat {
            Mat::Surface(id) => {
                let tile = if key.tile_cm > 0 { key.tile_cm as f32 / 100.0 } else { lib.catalog.get(id).and_then(|a| a.tile_m).map_or(2.0, |t| t[0].max(0.05)) };
                (lib.material(gpu, r, id), Instance::new(glam::Mat4::IDENTITY, tint, 1.0 / tile).with_params(indoor), false)
            }
            Mat::Plain(rough) => (plain.entry(rough).or_insert_with(|| Arc::new(Material::plain(gpu, r, rough))).clone(), Instance::new(glam::Mat4::IDENTITY, tint, 1.0).with_params(indoor), false),
            // glass clarity rides in params.y (Key::rough): base opacity, 0 = the default glazing
            // and its own warm glow in params.z (Key::tile, for lit glassware)
            Mat::Glass => (lib.glass(gpu, r), Instance::new(glam::Mat4::IDENTITY, tint, 1.0).with_params([0.0, gloss, key.tile_cm as f32 / 100.0, 0.0]), true),
            Mat::Screen(id) => (lib.material(gpu, r, id), Instance::new(glam::Mat4::IDENTITY, tint, 1.0 / if key.tile_cm > 0 { key.tile_cm as f32 / 100.0 } else { 1.0 }).with_params([4.0, 1.0, 0.0, 0.0]), false),
            Mat::Light => (plain.entry(255).or_insert_with(|| Arc::new(Material::plain(gpu, r, 255))).clone(), Instance::new(glam::Mat4::IDENTITY, tint, 1.0).with_params([4.0, 1.2, 0.0, 0.0]), false),
        };
        if let Some(b) = Batch::new(gpu, v, idx, &[inst], mat) {
            out.push(if transparent { b.transparent() } else { b });
        }
    }
    out
}
