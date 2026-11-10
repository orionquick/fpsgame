//! Hand-authored buildings, matched one at a time to street-level reference: real facade geometry
//! (openings, reveals, sashes, trim) and a generated interior (offices, corridors, core, stairs, lift).
pub mod factory;
pub mod furnish;
pub mod geom;
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
pub const OSM_IDS: &[&str] = &[factory::OSM];

/// All bespoke buildings in the city (only the ones already matched to reference).
pub fn all(c: &city::City) -> Vec<Building> {
    let cat = assets::Catalog::load(&crate::root().join("data/assets")).unwrap_or_default();
    factory::build(c, &cat).into_iter().collect()
}

pub const PARTITION_T: f32 = 0.12;
pub const CORE_T: f32 = 0.2;
pub const DOOR_H: f32 = 2.15;

fn k(mat: &'static str, tint: [f32; 3]) -> Key {
    Key::new(Mat::Surface(mat), tint).inside()
}

/// One floor of interior: partitions with doors, floor finishes, dropped ceiling with light panels, door leaves.
/// `y` is the finished floor level, `ceil` the ceiling height above it.
pub fn interior(g: &mut Geo, p: &Plan, y: f32, ceil: f32, seed: u64, ground: bool) {
    let white = [1.4, 1.4, 1.36];
    for (n, w) in p.walls.iter().enumerate() {
        let (mat, t, tint) = if w.core { (m::CORE_WALL, CORE_T, [1.05, 1.06, 1.08]) } else { (m::DRYWALL, PARTITION_T, white) };
        let key = k(mat, tint);
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
            Use::Hall => k(m::CONCRETE, [0.8, 0.78, 0.75]),
            Use::Storage | Use::Riser => k(m::CONCRETE, [0.7, 0.7, 0.7]),
            Use::Stair | Use::Elevator => continue,
        };
        let rr = r.r;
        g.quad(floor, Vec3::new(rr.x0, y, rr.z1), Vec3::X * rr.w(), Vec3::NEG_Z * rr.d());
        // ceiling (faces down)
        let ck = k(m::ACOUSTIC_CEILING, [1.0, 1.0, 1.0]);
        let _ = ground;
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

fn rnd(seed: u64, i: u64) -> f32 {
    let mut x = seed ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x51ab;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}

/// GPU batches for a building: one per material key, drawn with an identity instance.
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
    let ids: Vec<String> = g.groups.keys().filter_map(|k| if let Mat::Surface(id) = k.mat { Some(id.to_string()) } else { None }).collect();
    lib.preload(gpu, r, &ids);
    for (key, (v, idx)) in &g.groups {
        let tint = key.tint();
        let indoor = if key.indoor { [0.0, crate::library::INDOOR_AMBIENT, crate::library::INDOOR_LAMP, 0.0] } else { [0.0; 4] };
        let (mat, inst, transparent) = match key.mat {
            Mat::Surface(id) => {
                let tile = lib.catalog.get(id).and_then(|a| a.tile_m).map_or(2.0, |t| t[0].max(0.05));
                (lib.material(gpu, r, id), Instance::new(glam::Mat4::IDENTITY, tint, 1.0 / tile).with_params(indoor), false)
            }
            Mat::Plain(rough) => (plain.entry(rough).or_insert_with(|| Arc::new(Material::plain(gpu, r, rough))).clone(), Instance::new(glam::Mat4::IDENTITY, tint, 1.0).with_params(indoor), false),
            Mat::Glass => (lib.glass(gpu, r), Instance::new(glam::Mat4::IDENTITY, tint, 1.0), true),
            Mat::Light => (plain.entry(255).or_insert_with(|| Arc::new(Material::plain(gpu, r, 255))).clone(), Instance::new(glam::Mat4::IDENTITY, tint, 1.0).with_params([4.0, 1.2, 0.0, 0.0]), false),
        };
        if let Some(b) = Batch::new(gpu, v, idx, &[inst], mat) {
            out.push(if transparent { b.transparent() } else { b });
        }
    }
    out
}
