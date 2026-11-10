//! Furniture for generated floor plans, from the Megascans office and dining sets.
//! Placement is by room use and orientation: perimeter offices face into the room with the window behind,
//! radiators sit under every window, meeting rooms get a table and chairs, the ground floor is a restaurant.
use super::geom::Geo;
use super::plan::{Plan, Rect, Use};
use crate::library::{Placement, ALL_PARTS};
use assets::Catalog;
use glam::Vec2;

// Megascans ids (Urban Office Equipment set, dining furniture, plants).
const DESKS: &[&str] = &["ygthdf3", "ygthdja", "yfpocft", "yfcmaaw", "yfuodjt", "yfwedgk", "yfclfi0"];
const OFFICE_CHAIRS: &[&str] = &["ye3kddb", "yfhpbgs", "yfikegk", "yfknefw", "yfrhfcw", "yfrhfgb"];
const GUEST_CHAIRS: &[&str] = &["yfinchh", "yfjjfhh"];
const CABINETS: &[&str] = &["yf1fafl", "yfoobem", "yfpodb2", "yfpocj3"];
const FLOOR_PLANTS: &[&str] = &["ph_potted_plant_01", "ph_potted_plant_02", "siumT", "wfypfebha"];
const DESK_CLUTTER: &[&str] = &["yfkmajq", "yfnjcbs", "yghlfey", "yghlabs", "uchmfdyia", "rhDrj"];
const RADIATOR: &str = "yf2fcdo";
const PINBOARD: &str = "ye4pcby";
const COUCHES: &[&str] = &["yfwecht", "yfweceo"];
const SHELF: &str = "yf5eajt";
const BOXES: &[&str] = &["tfpsec2pa", "uevjedeva"];
const MEETING_TABLES: &[&str] = &["viejdi1", "vghpabdga"];
const DINING_TABLES: &[&str] = &["ph_round_wooden_table_02", "vl0jbem", "ph_round_wooden_table_02"];
const DINING_CHAIRS: &[&str] = &["ukikajmaw", "ukipagtbw", "uknjbb2bw", "uknkaffaw", "uknqadhbw", "ukpgfbebw"];
const STOOLS: &[&str] = &["ph_bar_chair_round_01", "ph_metal_stool_02"];
const PENDANT: &str = "vgzledvaw";

/// Which way the furniture models face, as a turn from their +Z axis (degrees clockwise).
const FRONT: f64 = 0.0;

pub struct Furnisher<'a> {
    cat: &'a Catalog,
    pub out: Vec<Placement>,
    seed: u64,
    n: u64,
    /// Placements get interior lighting (false for the patio and street).
    pub indoor: bool,
}

impl<'a> Furnisher<'a> {
    pub fn new(cat: &'a Catalog, seed: u64) -> Self {
        Self { cat, out: vec![], seed, n: 0, indoor: true }
    }

    pub fn rand(&mut self) -> f32 {
        self.rnd()
    }

    fn rnd(&mut self) -> f32 {
        self.n += 1;
        let mut x = self.seed ^ self.n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 31;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 29;
        (x % 100_000) as f32 / 100_000.0
    }
    fn pick(&mut self, set: &[&'static str]) -> &'static str {
        let i = (self.rnd() * set.len() as f32) as usize;
        set[i.min(set.len() - 1)]
    }
    /// Plan size of a model turned to face `face` (x along its width, y its depth) and its height.
    fn size(&self, id: &str) -> Option<[f32; 3]> {
        self.cat.get(id).map(crate::library::extent)
    }

    /// Places a model centred at local `c` on level `y`, its front facing local direction `face`.
    /// `solid` adds a collision block of its footprint.
    pub fn put(&mut self, g: &mut Geo, id: &str, c: Vec2, y: f32, face: Vec2, solid: bool) -> Option<[f32; 3]> {
        let s = self.size(id)?;
        let [x, z] = g.world_xz(c);
        // the model's +Z turns to heading rot + 90, so to face `face` its X runs at heading(face) - 90
        let rot = g.heading(face) - 90.0 + FRONT;
        self.out.push(Placement { asset: id.to_string(), part: ALL_PARTS, x, z, y: g.world_y(y), rot, scale: [1.0; 3], tint: [1.0; 3], lod: 0, indoor: self.indoor });
        if solid {
            let f = face.normalize();
            let r = Vec2::new(-f.y, f.x);
            let (hw, hd) = (s[0] / 2.0, s[2] / 2.0);
            let o = c - r * hw - f * hd;
            g.block_box(glam::Vec3::new(o.x, 0.0, o.y), glam::Vec3::new(r.x, 0.0, r.y) * s[0], glam::Vec3::new(f.x, 0.0, f.y) * s[2], y, y + s[1]);
        }
        Some(s)
    }

    /// `put` with a uniform scale.
    pub fn put_scaled(&mut self, g: &mut Geo, id: &str, c: Vec2, y: f32, face: Vec2, scale: f32, solid: bool) {
        let n = self.out.len();
        self.put(g, id, c, y, face, solid);
        if let Some(p) = self.out.get_mut(n) {
            p.scale = [scale; 3];
        }
    }

    /// A model hung on a wall: `c` on the wall face, facing into the room, bottom at `y`.
    fn hang(&mut self, g: &mut Geo, id: &str, c: Vec2, y: f32, face: Vec2) {
        let d = self.size(id).map_or(0.05, |s| s[2]);
        self.put(g, id, c + face.normalize() * (d / 2.0 + 0.01), y, face, false);
    }
}

/// The exterior side of a perimeter room (unit vector pointing out through its windows), if any.
fn exterior(r: &Rect, w: f32, d: f32, t: f32) -> Option<Vec2> {
    let e = 0.05;
    if r.z0 < t + e {
        Some(Vec2::NEG_Y)
    } else if r.z1 > d - t - e {
        Some(Vec2::Y)
    } else if r.x0 < t + e {
        Some(Vec2::NEG_X)
    } else if r.x1 > w - t - e {
        Some(Vec2::X)
    } else {
        None
    }
}

/// Window centres along an exterior side, as plan points on the inner wall face.
pub struct Windows {
    pub north: Vec<f32>,
    pub south: Vec<f32>,
    pub west: Vec<f32>,
}

/// Furnishes one floor at level `y`. `w, d, t` are the plate size and exterior wall thickness.
pub fn floor(f: &mut Furnisher, g: &mut Geo, p: &Plan, y: f32, ceil: f32, w: f32, d: f32, t: f32, win: &Windows) {
    for room in &p.rooms {
        let r = room.r;
        match room.kind {
            Use::Office => {
                if let Some(out) = exterior(&r, w, d, t) {
                    perimeter_office(f, g, &r, y, out, win, t);
                } else {
                    open_office(f, g, &r, y);
                }
            }
            Use::Meeting => meeting(f, g, &r, y),
            Use::Storage => storage(f, g, &r, y),
            Use::Lobby => {
                let c = r.centre();
                let id = f.pick(COUCHES);
                f.put(g, id, Vec2::new(r.x0 + 0.5, c.y - 2.0), y, Vec2::X, true);
                let pl = f.pick(FLOOR_PLANTS);
                f.put(g, pl, Vec2::new(r.x0 + 0.45, r.z1 - 0.5), y, Vec2::X, false);
            }
            Use::Hall => restaurant(f, g, &r, y, ceil),
            _ => {}
        }
    }
}

fn perimeter_office(f: &mut Furnisher, g: &mut Geo, r: &Rect, y: f32, out: Vec2, win: &Windows, t: f32) {
    let inward = -out;
    let side = Vec2::new(-out.y, out.x); // along the window wall
    let c = r.centre();
    // distance from the window wall to the room centre line, and the room's extent along the wall
    let (depth, span) = if out.x != 0.0 { (r.w(), r.d()) } else { (r.d(), r.w()) };
    let wall = c + out * (depth / 2.0);
    // radiators under the windows that fall in this room
    let along: &[f32] = if out == Vec2::NEG_Y { &win.north } else if out == Vec2::Y { &win.south } else { &win.west };
    for &u in along {
        let p = if out.x != 0.0 { Vec2::new(wall.x, u) } else { Vec2::new(u, wall.y) };
        let rel = (p - c).dot(side);
        if rel.abs() < span / 2.0 - 0.5 {
            f.put(g, RADIATOR, p + inward * 0.14, y + 0.12, inward, false);
        }
    }
    // desk 1.7 m in from the window, the sitter facing into the room with the window behind
    let desk = f.pick(DESKS);
    let dpos = wall + inward * 1.75 + side * ((f.rnd() - 0.5) * (span - 2.4).max(0.0) * 0.6);
    if let Some(ds) = f.put(g, desk, dpos, y, out, true) {
        let chair = f.pick(OFFICE_CHAIRS);
        f.put(g, chair, dpos + out * (ds[2] / 2.0 + 0.45), y, inward, false);
        if depth > 3.8 {
            let guest = f.pick(GUEST_CHAIRS);
            f.put(g, guest, dpos + inward * (ds[2] / 2.0 + 0.55) + side * 0.35, y, out, false);
        }
        // a thing or two on the desk
        for k in 0..2 {
            if f.rnd() < 0.7 {
                let it = f.pick(DESK_CLUTTER);
                let off = side * ((k as f32 - 0.5) * ds[0] * 0.55);
                f.put(g, it, dpos + off, y + ds[1], out, false);
            }
        }
    }
    // cabinet on the far inner corner (doors are near the low end of the corridor wall), plant by the window
    let far = if out.x != 0.0 { if f.rnd() < 0.5 { 1.0 } else { -1.0 } } else { 1.0 };
    let cab = f.pick(CABINETS);
    if let Some(cs) = f.size(cab) {
        let pc = c + inward * (depth / 2.0 - cs[2] / 2.0 - 0.08) + side * far * (span / 2.0 - cs[0] / 2.0 - 0.1);
        f.put(g, cab, pc, y, out, true);
    }
    let pl = f.pick(FLOOR_PLANTS);
    f.put(g, pl, wall + inward * 0.55 - side * far * (span / 2.0 - 0.55), y, inward, false);
    // pinboard on a side wall
    if span > 3.0 {
        let sw = c + side * far * (span / 2.0);
        f.hang(g, PINBOARD, sw + inward * 0.4, y + 1.2, -side * far);
    }
    let _ = t;
}

fn open_office(f: &mut Furnisher, g: &mut Geo, r: &Rect, y: f32) {
    // pairs of desks back to back along the long axis
    let long_x = r.w() >= r.d();
    let (axis, across) = if long_x { (Vec2::X, Vec2::Y) } else { (Vec2::Y, Vec2::X) };
    let (len, wid) = if long_x { (r.w(), r.d()) } else { (r.d(), r.w()) };
    let c = r.centre();
    let n = ((len - 1.6) / 1.7).floor().max(1.0) as i32;
    let rows = if wid > 5.5 { 2 } else { 1 };
    for row in 0..rows {
        let off = if rows == 2 { (row as f32 - 0.5) * 2.9 } else { 0.0 };
        for i in 0..n {
            let a = (i as f32 - (n - 1) as f32 / 2.0) * 1.7;
            for (s, face) in [(1.0f32, across), (-1.0, -across)] {
                let desk = f.pick(DESKS);
                let ds = f.size(desk).unwrap_or([1.5, 0.75, 0.8]);
                let p = c + axis * a + across * (off + s * ds[2] / 2.0);
                f.put(g, desk, p, y, -face, true);
                let ch = f.pick(OFFICE_CHAIRS);
                f.put(g, ch, p + face * (ds[2] / 2.0 + 0.45), y, -face, false);
                if f.rnd() < 0.6 {
                    let it = f.pick(DESK_CLUTTER);
                    f.put(g, it, p, y + ds[1], face, false);
                }
            }
        }
    }
    let pl = f.pick(FLOOR_PLANTS);
    f.put(g, pl, Vec2::new(r.x1 - 0.5, r.z1 - 0.5), y, Vec2::X, false);
}

fn meeting(f: &mut Furnisher, g: &mut Geo, r: &Rect, y: f32) {
    let long_x = r.w() >= r.d();
    let axis = if long_x { Vec2::X } else { Vec2::Y };
    let across = Vec2::new(-axis.y, axis.x);
    let len = if long_x { r.w() } else { r.d() };
    let c = r.centre();
    let table = f.pick(MEETING_TABLES);
    let ts = f.size(table).unwrap_or([1.5, 0.75, 1.0]);
    let n = ((len - 2.4) / ts[0]).floor().clamp(1.0, 3.0) as i32;
    let chair = f.pick(DINING_CHAIRS);
    for i in 0..n {
        let a = (i as f32 - (n - 1) as f32 / 2.0) * ts[0];
        // tables placed with their length along the room
        f.put(g, table, c + axis * a, y, across, true);
        for s in [-1.0f32, 1.0] {
            for k in [-0.3f32, 0.3] {
                f.put(g, chair, c + axis * (a + k * ts[0]) + across * s * (ts[2] / 2.0 + 0.3), y, -across * s, false);
            }
        }
    }
    let half = n as f32 * ts[0] / 2.0;
    for s in [-1.0f32, 1.0] {
        f.put(g, chair, c + axis * s * (half + 0.35), y, -axis * s, false);
    }
    // pinboard on the end wall, a plant in a corner
    let end = c + axis * (len / 2.0);
    f.hang(g, PINBOARD, end, y + 1.1, -axis);
    let pl = f.pick(FLOOR_PLANTS);
    f.put(g, pl, c - axis * (len / 2.0 - 0.5) + across * ((if long_x { r.d() } else { r.w() }) / 2.0 - 0.5), y, axis, false);
}

fn storage(f: &mut Furnisher, g: &mut Geo, r: &Rect, y: f32) {
    let long_x = r.w() >= r.d();
    let axis = if long_x { Vec2::X } else { Vec2::Y };
    let across = Vec2::new(-axis.y, axis.x);
    let (len, wid) = if long_x { (r.w(), r.d()) } else { (r.d(), r.w()) };
    let c = r.centre();
    let ss = f.size(SHELF).unwrap_or([2.16, 1.31, 0.46]);
    let n = ((len - 0.6) / (ss[0] + 0.1)).floor().max(0.0) as i32;
    for i in 0..n {
        let a = (i as f32 - (n - 1) as f32 / 2.0) * (ss[0] + 0.1);
        f.put(g, SHELF, c + axis * a + across * (wid / 2.0 - ss[2] / 2.0 - 0.05), y, -across, true);
    }
    for _ in 0..(len * 1.5) as i32 {
        let b = f.pick(BOXES);
        let p = c + axis * ((f.rnd() - 0.5) * (len - 1.0)) - across * ((f.rnd() * 0.5) * (wid - 1.2));
        let face = Vec2::from_angle(f.rnd() * 6.28);
        f.put(g, b, p, y, face, false);
    }
}

fn restaurant(f: &mut Furnisher, g: &mut Geo, r: &Rect, y: f32, ceil: f32) {
    // tables for four on a grid, pendants over them; leave a 1.6 m aisle along the walls
    let (sx, sz) = (3.4f32, 3.1f32);
    let nx = ((r.w() - 3.2) / sx).floor().max(0.0) as i32;
    let nz = ((r.d() - 3.2) / sz).floor().max(0.0) as i32;
    if nx == 0 || nz == 0 {
        return;
    }
    let ox = r.x0 + (r.w() - (nx - 1) as f32 * sx) / 2.0;
    let oz = r.z0 + (r.d() - (nz - 1) as f32 * sz) / 2.0;
    let table = f.pick(DINING_TABLES);
    let chair = f.pick(DINING_CHAIRS);
    let ts = f.size(table).unwrap_or([1.3, 0.75, 0.75]);
    let ps = f.size(PENDANT).unwrap_or([0.4, 0.36, 0.4]);
    let cord = super::geom::Key::new(super::geom::Mat::Plain(80), [0.02, 0.02, 0.02]).inside();
    for i in 0..nx {
        for j in 0..nz {
            // stagger alternate rows so the room doesn't read as a grid
            let p = Vec2::new(ox + i as f32 * sx + if j % 2 == 1 { sx * 0.25 } else { -sx * 0.25 }, oz + j as f32 * sz);
            if p.x > r.x1 - 1.2 || p.x < r.x0 + 1.2 {
                continue;
            }
            // tables sit a little askew; chairs are pulled out by different amounts and turned
            let p = p + Vec2::new(f.rnd() - 0.5, f.rnd() - 0.5) * 0.5;
            let along = Vec2::X.rotate(Vec2::from_angle((f.rnd() - 0.5) * 0.35));
            let across = Vec2::new(-along.y, along.x);
            f.put(g, table, p, y, across, true);
            for s in [-1.0f32, 1.0] {
                for k in [-0.25f32, 0.25] {
                    if f.rnd() < 0.85 {
                        let pull = 0.25 + f.rnd() * f.rnd() * 0.45;
                        let face = (-across * s).rotate(Vec2::from_angle((f.rnd() - 0.5) * 0.9));
                        let pos = p + along * (k * ts[0] + (f.rnd() - 0.5) * 0.12) + across * s * (ts[2] / 2.0 + pull);
                        f.put(g, chair, pos, y, face, false);
                    }
                }
            }
            let hang = y + ceil - 0.9;
            f.put(g, PENDANT, p, hang, Vec2::Y, false);
            let top = hang + ps[1];
            g.cuboid(cord, glam::Vec3::new(p.x - 0.006, top, p.y - 0.006), glam::Vec3::new(p.x + 0.006, y + ceil, p.y + 0.006));
        }
    }
}

/// Bar counter with stools in front of it: `a`-`b` the counter's front edge (local), the bartender behind it.
pub fn bar(f: &mut Furnisher, g: &mut Geo, a: Vec2, b: Vec2, y: f32) {
    let wood = super::geom::Key::new(super::geom::Mat::Surface(super::m::WOOD), [0.55, 0.4, 0.3]).inside();
    let front = super::geom::Key::new(super::geom::Mat::Plain(140), [0.06, 0.12, 0.09]).inside();
    let d = b - a;
    let l = d.length();
    let dir = d / l;
    let back = Vec2::new(dir.y, -dir.x); // behind the counter
    g.beam(front, a + back * 0.3, b + back * 0.3, 0.3, y, y + 1.0);
    g.beam(wood, a + back * 0.28, b + back * 0.28, 0.36, y + 1.0, y + 1.06);
    let n = (l / 0.75).floor() as i32;
    for i in 0..n {
        let p = a + dir * ((i as f32 + 0.5) * l / n as f32) - back * 0.45;
        let s = f.pick(STOOLS);
        let jitter = Vec2::new(f.rnd() - 0.5, f.rnd() - 0.5) * 0.18;
        let face = back.rotate(Vec2::from_angle(f.rnd() * 6.28));
        f.put(g, s, p + jitter, y, face, false);
    }
}
