//! Converted-warehouse office fit-out, after the reference interiors: exposed heavy-timber structure (girders on the
//! pier lines, close joists, plank deck), timber columns, spiral ducts and sprinkler mains, rows of linear pendants,
//! polished concrete with carpet under open-plan desk benches, glass meeting rooms against the blind wall, a kitchen
//! by the core, and (ground floor) a café with a bar.
use super::furnish::Furnisher;
use super::geom::{Geo, Key, Mat};
use super::m;
use super::plan::{Core, Rect};
use glam::{Vec2, Vec3};

/// The floor plate and its structural grid.
pub struct Spec {
    pub w: f32,
    pub d: f32,
    pub t: f32,
    /// Girder lines (z), on the west front's piers.
    pub girders: Vec<f32>,
    /// Column lines (x).
    pub cols: Vec<f32>,
    pub core: Core,
}

const DECK: f32 = 0.1;
const JOIST_D: f32 = 0.3;
const GIRDER_D: f32 = 0.5;

fn ins(k: Key) -> Key {
    k.inside()
}
/// Old-growth heavy timber, darkened with age; grain stretched to suit big members.
fn timber() -> Key {
    ins(Key::new(Mat::Surface(TREATED), [0.42, 0.28, 0.19]).tile(0.9).rough(0.85))
}
/// Rough-sawn columns, darker and oilier than the beams (as in the photos).
fn column_timber() -> Key {
    ins(Key::new(Mat::Surface(TREATED), [0.24, 0.16, 0.115]).tile(1.1).rough(0.5))
}
/// Plank deck seen from below: dark aged boards (as in the photos).
fn deck() -> Key {
    ins(Key::new(Mat::Surface("pht_wood_planks"), [0.62, 0.46, 0.33]).tile(1.5).rough(1.0))
}
const OAK: &str = "rd5oqpp0";
const DECK_PLANKS: &str = "ul4vcieaw";
const TREATED: &str = "scoleyp0";
const VENEER: &str = "vlskbcz";
const TABLE_TOP: &str = "tdfhffuq";
const LEATHER: &str = "oizuetp0";
const SUEDE_BLACK: &str = "xjgheam";
/// Panel fabrics: charcoal, slate blue, navy, grey, twill.
const FABRICS: [&str; 5] = ["sjfvcbtc", "sjfvce3c", "sjfvcgnc", "sjfvdh0c", "xgbcediqx"];
fn white() -> Key {
    ins(Key::new(Mat::Plain(200), [0.82, 0.82, 0.8]))
}
fn black() -> Key {
    ins(Key::new(Mat::Plain(120), [0.025, 0.025, 0.028]))
}
fn galv() -> Key {
    ins(Key::new(Mat::Plain(55), [0.26, 0.27, 0.28]))
}

impl Spec {
    /// Height of the underside of the joists above a floor at `y` with storey height `h`.
    pub fn clear(h: f32) -> f32 {
        h - DECK - JOIST_D
    }
    fn in_core(&self, p: Vec2, pad: f32) -> bool {
        let c = self.core.r;
        p.x > c.x0 - pad && p.x < c.x1 + pad && p.y > c.z0 - pad && p.y < c.z1 + pad
    }
    pub fn columns(&self) -> Vec<Vec2> {
        let mut out = vec![];
        for &z in &self.girders {
            for &x in &self.cols {
                let p = Vec2::new(x, z);
                if !self.in_core(p, 0.6) {
                    out.push(p);
                }
            }
        }
        out
    }
}

/// The structure over a floor at `y` (storey `h`): deck, joists, girders, columns, ducts, sprinklers, lights.
/// `office_lights`: hang the linear LED pendants (the cafe floor uses warm pendants instead).
pub fn structure(g: &mut Geo, s: &Spec, y: f32, h: f32, seed: u64, office_lights: bool) {
    let (x0, x1, z0, z1) = (s.t, s.w - s.t, s.t, s.d - s.t);
    let top = y + h; // top of the deck above = next finished floor
    let deck_b = top - DECK;
    let joist_b = deck_b - JOIST_D;
    let girder_b = deck_b - GIRDER_D;
    // plank deck underside (faces down) and its top edge is the floor above
    // boards run along x (across the joists): texture V along x, face down
    let (pa, pb, pc, pd) = (Vec3::new(x0, deck_b, z0), Vec3::new(x1, deck_b, z0), Vec3::new(x1, deck_b, z1), Vec3::new(x0, deck_b, z1));
    g.poly(deck(), &[pa, pb, pc, pd], Vec3::NEG_Z, Vec3::X);
    // girders along x on the pier lines
    for &z in &s.girders {
        g.timber(timber(), Vec3::new(x0, girder_b, z - 0.16), Vec3::new(x1, deck_b, z + 0.16));
    }
    // joists along z between the girders (and the walls), every 0.6 m
    let mut lines = vec![z0];
    lines.extend(s.girders.iter().copied());
    lines.push(z1);
    let n = ((x1 - x0) / 0.6).floor() as i32;
    for k in 0..lines.len() - 1 {
        let (za, zb) = (lines[k] + if k == 0 { 0.0 } else { 0.16 }, lines[k + 1] - if k + 2 == lines.len() { 0.0 } else { 0.16 });
        for i in 0..=n {
            let x = x0 + 0.3 + i as f32 * (x1 - x0 - 0.6) / n as f32;
            g.timber(timber(), Vec3::new(x - 0.04, joist_b, za), Vec3::new(x + 0.04, deck_b, zb));
        }
    }
    // columns with steel caps
    let col_top = girder_b;
    for c in s.columns() {
        let r = 0.21;
        g.timber(column_timber(), Vec3::new(c.x - r, y, c.y - r), Vec3::new(c.x + r, col_top, c.y + r));
        g.block_aabb(Vec3::new(c.x - r, y, c.y - r), Vec3::new(c.x + r, col_top, c.y + r));
        // timber bolster where the column meets the girder
        g.timber(timber(), Vec3::new(c.x - 0.2, col_top - 0.3, c.y - 0.7), Vec3::new(c.x + 0.2, col_top + 0.001, c.y + 0.7));

    }
    // spiral supply ducts: a main along x near the middle, branches along z, hung under the girders
    let duct_y = girder_b - 0.38;
    let zc = s.core.r.z0 - 1.2;
    g.tube(galv(), Vec3::new(x0 + 1.0, duct_y, zc), Vec3::new(s.core.r.x0 - 0.3, duct_y, zc), 0.3, 18);
    for &x in s.cols.iter() {
        let bx = x - 1.8;
        if bx < x0 + 1.5 {
            continue;
        }
        g.tube(galv(), Vec3::new(bx, duct_y + 0.1, z0 + 2.0), Vec3::new(bx, duct_y + 0.1, z1 - 2.0), 0.18, 14);
        // registers along the branch
        let mut z = z0 + 3.0;
        while z < z1 - 2.5 {
            g.cuboid(galv(), Vec3::new(bx - 0.15, duct_y - 0.12, z - 0.15), Vec3::new(bx + 0.15, duct_y - 0.05, z + 0.15));
            z += 3.6;
        }
    }
    // hangers for the main duct
    let mut x = x0 + 2.0;
    while x < s.core.r.x0 - 0.5 {
        g.cuboid(black(), Vec3::new(x - 0.01, duct_y, zc - 0.01), Vec3::new(x + 0.01, girder_b, zc + 0.01));
        x += 2.4;
    }
    // sprinkler mains (red) and branch lines with heads
    let red = ins(Key::new(Mat::Plain(90), [0.06, 0.06, 0.065]));
    let sp_y = joist_b - 0.06;
    for &z in &s.girders {
        let zz = z + 1.6;
        g.tube(red, Vec3::new(x0 + 0.4, sp_y, zz), Vec3::new(x1 - 0.4, sp_y, zz), 0.035, 8);
        let mut x = x0 + 1.5;
        while x < x1 - 1.0 {
            g.cuboid(galv(), Vec3::new(x - 0.02, sp_y - 0.1, zz - 0.02), Vec3::new(x + 0.02, sp_y, zz + 0.02));
            x += 3.0;
        }
    }
    // linear LED pendants in rows across the bays (between girders), on two cables each
    let lamp_y = y + 2.75;
    let glow = Key::new(Mat::Light, [1.0, 0.95, 0.86]);
    for k in 0..if office_lights { lines.len() - 1 } else { 0 } {
        let zm = (lines[k] + lines[k + 1]) / 2.0;
        for row in [-1.0f32, 1.0] {
            let z = zm + row * (lines[k + 1] - lines[k]) * 0.24;
            let mut x = x0 + 1.2;
            while x + 1.4 < x1 - 0.8 {
                let p = Vec2::new(x + 0.7, z);
                if !s.in_core(p, 0.8) {
                    g.cuboid(black(), Vec3::new(x, lamp_y, z - 0.04), Vec3::new(x + 1.4, lamp_y + 0.06, z + 0.04));
                    g.quad(glow, Vec3::new(x + 0.02, lamp_y - 0.002, z - 0.03), Vec3::X * 1.36, Vec3::Z * 0.06);
                    g.light(Vec3::new(x + 0.7, lamp_y - 0.15, z), 4.5, [1.1, 1.02, 0.9]);
                    for cx in [x + 0.15, x + 1.25] {
                        g.cuboid(black(), Vec3::new(cx - 0.004, lamp_y + 0.06, z - 0.004), Vec3::new(cx + 0.004, joist_b, z + 0.004));
                    }
                }
                x += 1.4 + 0.9;
            }
        }
    }
    let _ = seed;
}

/// Floor finish: polished concrete everywhere outside the core, carpet tiles under `pads`.
pub fn floor(g: &mut Geo, s: &Spec, y: f32, pads: &[Rect]) {
    let carpet = ins(Key::new(Mat::Surface(m::CARPET), [0.62, 0.64, 0.68]));
    for r in pads {
        g.quad(carpet, Vec3::new(r.x0, y + 0.004, r.z1), Vec3::X * r.w(), Vec3::NEG_Z * r.d());
    }
    let _ = s;
}

/// A cluster of cubicles: two rows of `n` stations back to back along `axis`, centred at `c`.
/// Each station: L-shaped worksurface, fabric panels with a glass topper and veneer caps, a pedestal, one or two
/// monitors on arms, keyboard, phone, mug, papers, sometimes an overhead bin, and a task chair tucked in or pushed out.
/// The finish (worksurface, fabric, bins) varies per cluster. Returns the carpet pad under it.
pub fn cubicles(g: &mut Geo, f: &mut Furnisher, c: Vec2, axis: Vec2, n: i32, y: f32) -> Rect {
    let across = Vec2::new(-axis.y, axis.x);
    let (sw, sd) = (1.8f32, 1.75f32); // station width (along axis) and depth (from the spine)
    let len = n as f32 * sw;
    let v3 = |p: Vec2, h: f32| Vec3::new(p.x, h, p.y);
    let fabric = ins(Key::new(Mat::Surface(FABRICS[(f.rand() * 5.0) as usize % 5]), [1.0, 1.0, 1.0]).tile(0.6));
    let wood_top = f.rand() < 0.5;
    let top = if wood_top { ins(Key::new(Mat::Surface(TREATED), [0.82, 0.7, 0.58]).tile(1.0)) } else { ins(Key::new(Mat::Plain(140), [0.8, 0.79, 0.76])) };
    let veneer = ins(Key::new(Mat::Surface(VENEER), [1.0, 0.95, 0.9]));
    let alu = ins(Key::new(Mat::Plain(60), [0.5, 0.51, 0.52]));
    let ped = ins(Key::new(Mat::Plain(150), [0.62, 0.63, 0.64]));
    let glass = Key::new(Mat::Glass, [0.8, 0.84, 0.82]);
    let screen = ins(Key::new(Mat::Plain(25), [0.008, 0.009, 0.012]));
    let bins = f.rand() < 0.5;
    let (panel_h, fab_h) = (1.3f32, 1.02f32);
    // spine panel: fabric below, glass topper in an aluminium frame, veneer cap
    g.beam(fabric, c - axis * len / 2.0, c + axis * len / 2.0, 0.03, y, y + fab_h);
    let a0 = c - axis * len / 2.0;
    g.quad(glass, v3(a0, y + fab_h), Vec3::new(axis.x, 0.0, axis.y) * len, Vec3::Y * (panel_h - fab_h));
    g.beam(alu, c - axis * len / 2.0, c + axis * len / 2.0, 0.035, y + fab_h - 0.02, y + fab_h + 0.01);
    g.beam(veneer, c - axis * len / 2.0, c + axis * len / 2.0, 0.04, y + panel_h, y + panel_h + 0.03);
    g.block_box(v3(a0 - across * 0.04, 0.0), Vec3::new(axis.x, 0.0, axis.y) * len, Vec3::new(across.x, 0.0, across.y) * 0.08, y, y + panel_h);
    for side in [-1.0f32, 1.0] {
        let out = across * side;
        for i in 0..=n {
            // side panels between stations (veneer end panels at the cluster ends)
            let p = a0 + axis * (i as f32 * sw);
            let end = i == 0 || i == n;
            let k = if end { veneer } else { fabric };
            let h = if end { 1.1 } else { 1.05 };
            g.beam(k, p + out * 0.03, p + out * (sd - 0.1), if end { 0.03 } else { 0.025 }, y, y + h);
            g.beam(alu, p + out * 0.03, p + out * (sd - 0.1), 0.028, y + h, y + h + 0.015);
            g.block_box(v3(p - axis * 0.03, 0.0), Vec3::new(out.x, 0.0, out.y) * sd, Vec3::new(axis.x, 0.0, axis.y) * 0.06, y, y + h);
        }
        for i in 0..n {
            let s0 = a0 + axis * (i as f32 * sw + 0.03);
            let mid = s0 + axis * (sw / 2.0 - 0.03);
            let left = f.rand() < 0.5;
            // main worksurface along the spine and a return along one side panel
            let ws = s0 + out * 0.4;
            g.beam(top, ws, ws + axis * (sw - 0.06), 0.37, y + 0.71, y + 0.74);
            let (ra, rb) = if left { (s0 + axis * 0.32, s0 + axis * 0.32 + out * 1.2) } else { (s0 + axis * (sw - 0.38), s0 + axis * (sw - 0.38) + out * 1.2) };
            g.beam(top, ra + out * 0.75, rb, 0.3, y + 0.71, y + 0.74);
            g.block_box(v3(s0, 0.0), Vec3::new(axis.x, 0.0, axis.y) * (sw - 0.06), Vec3::new(out.x, 0.0, out.y) * 0.77, y, y + 0.74);
            // pedestal under the return
            let pp = (ra + rb) / 2.0 + out * 0.1;
            g.beam(ped, pp - out * 0.28, pp + out * 0.28, 0.2, y + 0.02, y + 0.62);
            g.beam(alu, pp - out * 0.28 + axis * 0.0, pp - out * 0.28 + axis * 0.0, 0.0, y, y);
            // overhead bin on the spine
            if bins {
                g.beam(top, s0 + out * 0.2 + axis * 0.2, s0 + out * 0.2 + axis * (sw - 0.3), 0.17, y + 1.32, y + 1.68);
                g.beam(alu, s0 + out * 0.37 + axis * 0.2, s0 + out * 0.37 + axis * (sw - 0.3), 0.004, y + 1.33, y + 1.67);
            }
            // monitors on an arm clamped to the spine
            let dual = f.rand() < 0.6;
            let base = mid + out * 0.12;
            g.tube(alu, v3(base, y + 0.74), v3(base, y + 1.12), 0.018, 6);
            let mons: Vec<f32> = if dual { vec![-0.31, 0.31] } else { vec![0.0] };
            let turn = (f.rand() - 0.5) * 0.25;
            for (j, &o) in mons.iter().enumerate() {
                let ang = if dual { if j == 0 { 0.18 } else { -0.18 } } else { 0.0 } + turn;
                let dirm = axis.rotate(Vec2::from_angle(ang * side));
                let pc = base + axis * o + out * (0.08 + o.abs() * 0.15);
                let w = if dual { 0.27 } else { 0.33 };
                g.beam(screen, pc - dirm * w, pc + dirm * w, 0.012, y + 0.92, y + 0.92 + w * 1.15);
            }
            // keyboard, mouse, phone, mug
            let kb = mid + out * 0.55 + axis * (f.rand() - 0.5) * 0.15;
            let kd = axis.rotate(Vec2::from_angle((f.rand() - 0.5) * 0.15));
            g.beam(ins(Key::new(Mat::Plain(170), [0.05, 0.05, 0.055])), kb - kd * 0.22, kb + kd * 0.22, 0.07, y + 0.74, y + 0.76);
            let ms = kb + axis * 0.32 + out * 0.03;
            g.beam(ins(Key::new(Mat::Plain(120), [0.04, 0.04, 0.045])), ms - out * 0.05, ms + out * 0.05, 0.03, y + 0.74, y + 0.775);
            let ph = mid - axis * 0.62 + out * 0.3;
            g.beam(ins(Key::new(Mat::Plain(120), [0.06, 0.06, 0.07])), ph - out * 0.1, ph + out * 0.1, 0.09, y + 0.74, y + 0.8);
            if f.rand() < 0.6 {
                let mug = mid + axis * (0.55 + f.rand() * 0.15) + out * (0.25 + f.rand() * 0.2);
                let col = [[0.8, 0.8, 0.78], [0.12, 0.25, 0.45], [0.6, 0.12, 0.08], [0.15, 0.15, 0.15]][(f.rand() * 4.0) as usize % 4];
                g.tube(ins(Key::new(Mat::Plain(90), col)), v3(mug, y + 0.74), v3(mug, y + 0.84), 0.04, 10);
            }
            if f.rand() < 0.35 {
                let id = ["ph_stationery_supplies", "ph_clipboard", "ph_desk_lamp_arm_01", "ph_potted_plant_04"][(f.rand() * 4.0) as usize % 4];
                let pa = s0 + axis * (if left { sw - 0.35 } else { 0.35 }) + out * 0.25;
                let dir = Vec2::from_angle(f.rand() * 6.28);
                f.put(g, id, pa, y + 0.74, dir, false);
            }
            if f.rand() < 0.5 {
                let id = ["yfkmajq", "yghlfey", "yfnjcbs"][(f.rand() * 3.0) as usize % 3];
                let pa = (ra + rb) / 2.0 + out * 0.2;
                let dir = out.rotate(Vec2::from_angle((f.rand() - 0.5) * 0.8));
                f.put(g, id, pa, y + 0.74, dir, false);
            }
            if f.rand() < 0.3 {
                let pa = s0 + axis * (if left { sw - 0.25 } else { 0.25 }) + out * 0.2;
                let id = ["uchmfdyia", "rhDrj"][(f.rand() * 2.0) as usize % 2];
                let dir = Vec2::from_angle(f.rand() * 6.28);
                f.put(g, id, pa, y + 0.74, dir, false);
            }
            // trash bin under the desk
            let tb = s0 + axis * (if left { sw - 0.3 } else { 0.3 }) + out * 0.25;
            g.tube(ins(Key::new(Mat::Plain(140), [0.08, 0.08, 0.08])), v3(tb, y), v3(tb, y + 0.32), 0.13, 10);
            // chair: tucked under or pushed back and turned
            let tucked = f.rand() < 0.4;
            let back = if tucked { 0.75 } else { 1.05 + f.rand() * 0.4 };
            let yaw = if tucked { (f.rand() - 0.5) * 0.25 } else { (f.rand() - 0.5) * 1.6 };
            let cp = mid + out * back + axis * (f.rand() - 0.5) * 0.35;
            let style = (f.rand() * 3.0) as u8;
            task_chair(g, cp, y, (-out).rotate(Vec2::from_angle(yaw)), style);
        }
    }
    let half = across * (sd + 0.6) + axis * (len / 2.0 + 0.3);
    let (p, q) = (c - half, c + half);
    Rect::new(p.x, p.y, q.x, q.y)
}

/// A conference table: thick reclaimed-wood top on black steel trestles.
pub fn conference_table(g: &mut Geo, c: Vec2, axis: Vec2, len: f32, wid: f32, y: f32) {
    let across = Vec2::new(-axis.y, axis.x);
    let topk = ins(Key::new(Mat::Surface(TABLE_TOP), [1.15, 1.0, 0.9]).tile(1.6));
    let (a, b) = (c - axis * len / 2.0, c + axis * len / 2.0);
    g.beam(topk, a, b, wid / 2.0, y + 0.69, y + 0.75);
    for s in [-0.38f32, 0.38] {
        let p = c + axis * len * s;
        g.beam(black(), p - across * (wid / 2.0 - 0.12), p + across * (wid / 2.0 - 0.12), 0.04, y + 0.62, y + 0.69);
        for t in [-1.0f32, 1.0] {
            let q = p + across * t * (wid / 2.0 - 0.15);
            g.beam(black(), q - axis * 0.03, q + axis * 0.03, 0.03, y, y + 0.69);
        }
    }
    g.block_box(Vec3::new(a.x, 0.0, a.y) - Vec3::new(across.x, 0.0, across.y) * (wid / 2.0), Vec3::new(axis.x, 0.0, axis.y) * len, Vec3::new(across.x, 0.0, across.y) * wid, y, y + 0.75);
}

/// Glass-walled room on `r` (glass on the sides listed by `open`: 0 west, 1 north, 2 east, 3 south), a door on the
/// first open side, drywall bulkhead above the glass up to the joists.
pub fn glass_room(g: &mut Geo, r: Rect, y: f32, clear: f32, open: &[u8]) {
    let glass = Key::new(Mat::Glass, [0.7, 0.74, 0.72]);
    let head = 2.75;
    for (n, &side) in open.iter().enumerate() {
        let (a, b) = match side {
            0 => (Vec2::new(r.x0, r.z1), Vec2::new(r.x0, r.z0)),
            1 => (Vec2::new(r.x0, r.z0), Vec2::new(r.x1, r.z0)),
            2 => (Vec2::new(r.x1, r.z0), Vec2::new(r.x1, r.z1)),
            _ => (Vec2::new(r.x1, r.z1), Vec2::new(r.x0, r.z1)),
        };
        let d = b - a;
        let l = d.length();
        let dir = d / l;
        let door = if n == 0 { Some((l * 0.75 - 0.5, l * 0.75 + 0.5)) } else { None };
        // glass panes between thin black mullions
        let panes = (l / 1.25).ceil() as i32;
        for i in 0..panes {
            let (u0, u1) = (l * i as f32 / panes as f32, l * (i + 1) as f32 / panes as f32);
            if let Some((d0, d1)) = door {
                if u1 > d0 && u0 < d1 {
                    continue;
                }
            }
            let p = a + dir * u0;
            g.quad(glass, Vec3::new(p.x, y, p.y), Vec3::new(dir.x, 0.0, dir.y) * (u1 - u0), Vec3::Y * head);
            g.beam(black(), p, p + dir * 0.04, 0.03, y, y + head);
        }
        if let Some((d0, d1)) = door {
            let (p0, p1) = (a + dir * d0, a + dir * d1);
            g.beam(black(), p0 - dir * 0.04, p0, 0.03, y, y + head);
            g.beam(black(), p1, p1 + dir * 0.04, 0.03, y, y + head);
            // glass door, ajar
            let leaf = dir.rotate(Vec2::from_angle(0.5));
            let q = p0 + leaf * 0.95;
            g.quad(glass, Vec3::new(p0.x, y, p0.y), Vec3::new(q.x - p0.x, 0.0, q.y - p0.y), Vec3::Y * 2.3);
            g.beam(black(), p0 + leaf * 0.85, p0 + leaf * 0.88, 0.02, y + 0.8, y + 1.3);
        }
        g.beam(black(), a, b, 0.04, y, y + 0.05);
        g.beam(black(), a, b, 0.04, y + head - 0.05, y + head);
        // bulkhead to the joists, and collision for the glass
        g.wall(white(), white(), a, b, y + head, y + clear, 0.12, &[], true);
        let solid: Vec<(f32, f32)> = match door {
            Some((d0, d1)) => vec![(0.0, d0), (d1, l)],
            None => vec![(0.0, l)],
        };
        for (u0, u1) in solid {
            let p = a + dir * u0;
            g.block_box(Vec3::new(p.x, 0.0, p.y), Vec3::new(dir.x, 0.0, dir.y) * (u1 - u0), Vec3::new(-dir.y, 0.0, dir.x) * 0.05, y, y + head);
        }
    }
}

/// Meeting room contents: table and chairs, a screen on the solid wall, pendants over the table.
pub fn meeting(g: &mut Geo, f: &mut Furnisher, r: Rect, y: f32, clear: f32, screen_wall: Vec2) {
    let c = r.centre();
    let long_x = r.w() >= r.d();
    let axis = if long_x { Vec2::X } else { Vec2::Y };
    let across = Vec2::new(-axis.y, axis.x);
    let len = if long_x { r.w() } else { r.d() };
    let tl = (len - 2.4).clamp(1.8, 5.0);
    let tw = 1.15;
    conference_table(g, c, axis, tl, tw, y);
    let n = ((tl - 0.3) / 0.75).floor() as i32;
    let style = 2 + (f.rand() < 0.5) as u8;
    for s in [-1.0f32, 1.0] {
        for i in 0..n {
            let a = (i as f32 - (n - 1) as f32 / 2.0) * (tl - 0.4) / (n - 1).max(1) as f32;
            let out = 0.75 + f.rand() * 0.35;
            let yaw = (f.rand() - 0.5) * 0.9;
            task_chair(g, c + axis * (a + (f.rand() - 0.5) * 0.15) + across * s * out, y, (-across * s).rotate(Vec2::from_angle(yaw)), style);
        }
    }
    task_chair(g, c + axis * (tl / 2.0 + 0.7), y, (-axis).rotate(Vec2::from_angle((f.rand() - 0.5) * 0.6)), style);
    // pendants over the table
    for k in [-0.3f32, 0.0, 0.3] {
        let p = c + axis * tl * k;
        let dir = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, "vgyide1aw", p, y + clear - 1.25, dir, false);
        g.cuboid(black(), Vec3::new(p.x - 0.004, y + clear - 0.97, p.y - 0.004), Vec3::new(p.x + 0.004, y + clear, p.y + 0.004));
    }
    // wall screen and a credenza under it
    let sw = c + screen_wall * ((if screen_wall.x != 0.0 { r.w() } else { r.d() }) / 2.0 - 0.06);
    let side = Vec2::new(-screen_wall.y, screen_wall.x);
    g.beam(ins(Key::new(Mat::Plain(25), [0.008, 0.008, 0.01])), sw - side * 0.85, sw + side * 0.85, 0.03, y + 1.05, y + 2.0);
    let cr = sw - screen_wall * 0.25;
    g.beam(ins(Key::new(Mat::Surface(VENEER), [1.0, 0.95, 0.9])), cr - side * 1.0, cr + side * 1.0, 0.22, y, y + 0.7);
    let dir = Vec2::from_angle(f.rand() * 6.28);
    f.put(g, "wfypfebha", c - axis * (len / 2.0 - 0.6) + across * ((if long_x { r.d() } else { r.w() }) / 2.0 - 0.6), y, dir, false);
}

/// Kitchen run against a wall: base cabinets, counter, upper cabinets, fridge; stools at an island.
pub fn kitchen(g: &mut Geo, f: &mut Furnisher, a: Vec2, b: Vec2, y: f32) {
    let d = b - a;
    let l = d.length();
    let dir = d / l;
    let out = Vec2::new(dir.y, -dir.x); // into the room
    let cab = ins(Key::new(Mat::Plain(160), [0.18, 0.2, 0.22]));
    let counter = ins(Key::new(Mat::Surface(m::CONCRETE), [0.85, 0.85, 0.83]));
    let line = |o: f32| (a + out * o, b + out * o);
    let (p, q) = line(0.31);
    g.beam(cab, p, q, 0.3, y, y + 0.88);
    g.beam(counter, p, q, 0.32, y + 0.88, y + 0.92);
    let (p, q) = line(0.18);
    g.beam(cab, p + dir * 0.6, q - dir * 0.9, 0.17, y + 1.5, y + 2.25);
    // fridge
    g.beam(ins(Key::new(Mat::Plain(60), [0.6, 0.61, 0.62])), b - dir * 0.85 + out * 0.36, b - dir * 0.05 + out * 0.36, 0.34, y, y + 1.9);
    g.block_box(Vec3::new(a.x, 0.0, a.y), Vec3::new(dir.x, 0.0, dir.y) * l, Vec3::new(out.x, 0.0, out.y) * 0.7, y, y + 0.92);
    // coffee maker on the counter, a pendant pair over the island
    f.put(g, "sf_383923d441", a + dir * (l * 0.3) + out * 0.3, y + 0.92, out, false);
    // island with stools
    let (p, q) = (a + out * 2.2 + dir * 0.8, b + out * 2.2 - dir * 1.4);
    g.beam(cab, p, q, 0.45, y, y + 0.9);
    g.beam(ins(Key::new(Mat::Surface(m::WOOD), [0.6, 0.45, 0.32])), p - dir * 0.1, q + dir * 0.1, 0.55, y + 0.9, y + 0.95);
    let il = (q - p).length();
    let mut s = 0.4;
    while s < il - 0.2 {
        let face = (-out).rotate(Vec2::from_angle(f.rand() * 6.28));
        let off = 0.8 + f.rand() * 0.15;
        f.put(g, "ph_bar_chair_round_01", p + dir * s + out * off, y, face, false);
        s += 0.65;
    }
}

/// Task chair: five-star base, gas column, seat, back, arms. `style` 0 black mesh, 1 grey fabric mid-back,
/// 2 black leather high-back, 3 tan leather conference chair.
pub fn task_chair(g: &mut Geo, c: Vec2, y: f32, face: Vec2, style: u8) {
    let f = face.normalize();
    let r = Vec2::new(-f.y, f.x);
    let k = black();
    let (seat_k, back_k, back_top) = match style % 4 {
        0 => (ins(Key::new(Mat::Surface(FABRICS[0]), [0.5, 0.5, 0.52]).tile(0.4)), ins(Key::new(Mat::Surface(SUEDE_BLACK), [1.0, 1.0, 1.0]).tile(0.2)), 1.1),
        1 => (ins(Key::new(Mat::Surface(FABRICS[3]), [1.0, 1.0, 1.0]).tile(0.4)), ins(Key::new(Mat::Surface(FABRICS[3]), [1.0, 1.0, 1.0]).tile(0.4)), 0.98),
        2 => (ins(Key::new(Mat::Surface(LEATHER), [0.2, 0.2, 0.22]).tile(0.3)), ins(Key::new(Mat::Surface(LEATHER), [0.2, 0.2, 0.22]).tile(0.3)), 1.18),
        _ => (ins(Key::new(Mat::Surface(LEATHER), [0.9, 0.85, 0.85]).tile(0.3)), ins(Key::new(Mat::Surface(LEATHER), [0.9, 0.85, 0.85]).tile(0.3)), 0.95),
    };
    let chrome = ins(Key::new(Mat::Plain(40), [0.35, 0.36, 0.37]));
    let spin = (c.x * 7.3 + c.y * 3.1).sin();
    for i in 0..5 {
        let a = Vec2::from_angle(i as f32 * std::f32::consts::TAU / 5.0 + spin);
        g.beam(if style == 3 { chrome } else { k }, c, c + a * 0.32, 0.018, y + 0.05, y + 0.09);
        let w = c + a * 0.31;
        g.tube(k, Vec3::new(w.x, y + 0.025, w.y) - Vec3::new(r.x, 0.0, r.y) * 0.02, Vec3::new(w.x, y + 0.025, w.y) + Vec3::new(r.x, 0.0, r.y) * 0.02, 0.025, 8);
    }
    g.tube(chrome, Vec3::new(c.x, y + 0.08, c.y), Vec3::new(c.x, y + 0.42, c.y), 0.022, 8);
    // seat pan with a cushion, back on a spine, arms
    g.beam(k, c - r * 0.24 + f * 0.02, c + r * 0.24 + f * 0.02, 0.23, y + 0.42, y + 0.46);
    g.beam(seat_k, c - r * 0.23 + f * 0.03, c + r * 0.23 + f * 0.03, 0.235, y + 0.46, y + 0.53);
    let back = c - f * 0.25;
    g.beam(k, back - f * 0.03, back, 0.03, y + 0.45, y + 0.62);
    // the back leans a little
    let lean = f * -0.06;
    g.beam(back_k, back - r * 0.22 - f * 0.03, back + r * 0.22 - f * 0.03, 0.035, y + 0.58, y + 0.8);
    g.beam(back_k, back - r * 0.21 - f * 0.03 + lean, back + r * 0.21 - f * 0.03 + lean, 0.035, y + 0.8, y + back_top);
    if style == 0 {
        g.beam(k, back - r * 0.23 - f * 0.03 + lean, back + r * 0.23 - f * 0.03 + lean, 0.04, y + back_top - 0.03, y + back_top + 0.01);
    }
    for s in [-1.0f32, 1.0] {
        let a = c + r * s * 0.26;
        g.beam(k, a - f * 0.05, a - f * 0.03, 0.015, y + 0.5, y + 0.68);
        g.beam(if style >= 2 { seat_k } else { k }, a - f * 0.2, a + f * 0.12, 0.03, y + 0.66, y + 0.7);
    }
}

/// The Office Props Pack (crazyshroomz, CC-BY) cubicle, reassembled from its pieces at their authored offsets.
/// Walls stand on the kit's -X and +Z sides; the CRT and beige keyboard are swapped for modern ones.
const KIT: &str = "sf_260ff7a37a";
/// Pieces that make up one cubicle (pack object indices), and the ones left out.
const KIT_SKIP: &[u32] = &[
    2,  // Object_4: beige tower (replaced)
    5,  // Object_7: beige keyboard (replaced)
    7,  // Object_9: CRT monitor (replaced)
    9,  // Object_11: beige desktop case (replaced)
    21, 22, 23, // water cooler and bottles (one per cluster instead)
    30, 31, 32, 34, // floor paper scatter
];

/// One cubicle from the pack at local `origin`, kit +Z along `z_dir`; adds modern screens and a varied chair.
pub fn kit_cubicle(g: &mut Geo, f: &mut Furnisher, origin: Vec2, y: f32, z_dir: Vec2) {
    let az = z_dir.normalize();
    let ax = Vec2::new(az.y, -az.x);
    for k in 0..36u32 {
        if KIT_SKIP.contains(&k) || k == 16 {
            continue;
        }
        let id = format!("{KIT}_{k:02}");
        let solid = matches!(k, 17 | 20 | 4); // desk, partitions, filing cabinet
        f.put_kit_piece(g, &id, origin, y, az, solid);
    }
    // the pack's desk surface is at 0.74 m: a flat-screen desktop set (monitor, keyboard, mouse) in the corner
    let at = |x: f32, z: f32| origin + ax * x + az * z;
    let turn = (f.rand() - 0.5) * 0.35;
    f.put(g, "sf_561abc2fc9", at(-0.7, 0.55), y + 0.74, ax.rotate(Vec2::from_angle(turn)), false);
    // chair pulled out at its own angle
    let chair = at(0.0, 0.3) + ax * (f.rand() * 0.5);
    let face = (-ax).rotate(Vec2::from_angle((f.rand() - 0.5) * 1.4));
    if f.rand() < 0.5 {
        f.put(g, "sf_675f34f730", chair, y, face, false);
    } else {
        let slide = f.rand() * 0.4;
        f.put_kit_piece(g, &format!("{KIT}_16"), origin + ax * slide, y, az, false);
    }
    if f.rand() < 0.4 {
        let id = ["sf_16c1a779bb_00", "sf_16c1a779bb_25", "sf_16c1a779bb_10"][(f.rand() * 3.0) as usize % 3];
        let dir = ax.rotate(Vec2::from_angle(f.rand() - 0.5));
        f.put(g, id, at(-0.9, 1.15), y + 0.74, dir, false);
    }
}

/// Four kit cubicles in a pinwheel around `c` with their partitions back to back; a water cooler at one corner.
pub fn kit_cluster(g: &mut Geo, f: &mut Furnisher, c: Vec2, y: f32) -> Rect {
    // the kit's walls face -X and +Z: for each quadrant pick the heading that turns both walls toward the centre
    let half = 1.16;
    for (sx, sz) in [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let toward = [Vec2::new(-sx, 0.0), Vec2::new(0.0, -sz)];
        let mut best = Vec2::Y;
        for az in [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y] {
            let ax = Vec2::new(az.y, -az.x);
            let walls = [-ax, az];
            if walls.iter().all(|w| toward.iter().any(|t| w.dot(*t) > 0.9)) {
                best = az;
            }
        }
        let ax = Vec2::new(best.y, -best.x);
        // kit wall planes sit at x = -1.15 and z = +1.34; put them on the cluster's centre lines
        let origin = c + Vec2::new(sx, sz) * 0.06 - (-ax) * 1.15 - best * 1.34 + (Vec2::new(sx, 0.0) + Vec2::new(0.0, sz)) * 0.0;
        let _ = half;
        kit_cubicle(g, f, origin, y, best);
    }
    let corner = c + Vec2::new(2.6, -2.4);
    f.put(g, &format!("{KIT}_21"), corner, y, Vec2::Y, true);
    let r = 2.7;
    Rect::new(c.x - r, c.y - r, c.x + r, c.y + r)
}

/// The ground-floor cafe, after industrial loft cafes: bistro tables with Tolix chairs, a reclaimed-wood communal
/// table, bar-light pendants (real lamps), a pub counter with stools and a back bar against the party wall,
/// espresso machines and bottles, a chalkboard by the door. `area`: the seating floor; `bar_x`: the back-bar wall.
pub fn cafe(g: &mut Geo, f: &mut Furnisher, area: Rect, bar: Rect, y: f32, clear: f32) {
    const CHAIRS: [&str; 2] = ["sf_0720ffc9b2", "sf_3fdcc5bc70"];
    const TABLE: &str = "sf_5b9c3706a6";
    const PENDANT: &str = "sf_2ba27702aa";
    let hang = |g: &mut Geo, f: &mut Furnisher, p: Vec2| {
        let h = f.bounds(PENDANT).map_or(1.25, |(a, b)| b[1] - a[1]);
        let top = y + clear;
        f.put(g, PENDANT, p, top - h, Vec2::Y, false);
        g.light(Vec3::new(p.x, top - h - 0.05, p.y), 5.0, [2.6, 1.7, 0.9]);
    };
    // communal table down the middle
    let c = area.centre();
    let axis = if area.d() > area.w() { Vec2::Y } else { Vec2::X };
    let across = Vec2::new(-axis.y, axis.x);
    let tl = 4.2;
    conference_table(g, c, axis, tl, 1.0, y);
    for s in [-1.0f32, 1.0] {
        for i in 0..5 {
            let a = (i as f32 - 2.0) * 0.82 + (f.rand() - 0.5) * 0.12;
            let pull = 0.6 + f.rand() * f.rand() * 0.45;
            let face = (-across * s).rotate(Vec2::from_angle((f.rand() - 0.5) * 0.8));
            let id = CHAIRS[(f.rand() * 2.0) as usize % 2];
            f.put(g, id, c + axis * a + across * s * pull, y, face, false);
        }
    }
    for k in [-0.33f32, 0.0, 0.33] {
        hang(g, f, c + axis * tl * k);
    }
    for k in 0..6 {
        let a = (k as f32 - 2.5) * 0.7;
        table_clutter(g, f, c + axis * a + across * (if k % 2 == 0 { 0.25 } else { -0.25 }), y + 0.75, 0.12);
    }
    // bistro tables for two to four in staggered rows either side
    let half_len = if axis == Vec2::Y { area.d() } else { area.w() } / 2.0 - 1.4;
    let half_wid = if axis == Vec2::Y { area.w() } else { area.d() } / 2.0;
    for s in [-1.0f32, 1.0] {
        for row in 0..3 {
            let off = 2.0 + row as f32 * 1.75;
            if off > half_wid - 0.9 {
                continue;
            }
            let n = (half_len * 2.0 / 2.0).floor() as i32;
            for i in 0..n {
                let a = -half_len + (i as f32 + 0.5 + if row == 1 { 0.5 } else { 0.0 }) * (half_len * 2.0 / n as f32);
                if a > half_len {
                    continue;
                }
                let p = c + axis * (a + (f.rand() - 0.5) * 0.3) + across * s * (off + (f.rand() - 0.5) * 0.25);
                let turn = Vec2::from_angle(f.rand() * 6.28);
                f.put(g, TABLE, p, y, turn, true);
                // darker oiled tops, as in the comparison photos
                if let Some(t) = f.out.last_mut() {
                    t.tint = [0.36, 0.29, 0.26];
                }
                table_clutter(g, f, p, y + 0.75, 0.3);
                let seats = 2 + (f.rand() * 2.2) as i32;
                let start = f.rand() * 6.28;
                for k in 0..seats {
                    if f.rand() < 0.12 {
                        continue;
                    }
                    let ang = start + k as f32 * std::f32::consts::TAU / seats as f32 + (f.rand() - 0.5) * 0.4;
                    let dir = Vec2::from_angle(ang);
                    let pull = 0.5 + f.rand() * f.rand() * 0.4;
                    let face = (-dir).rotate(Vec2::from_angle((f.rand() - 0.5) * 0.9));
                    let id = CHAIRS[(f.rand() * 2.0) as usize % 2];
                    f.put(g, id, p + dir * pull, y, face, false);
                }
                hang(g, f, p);
            }
        }
    }
    // the bar: back bar on the wall side of `bar`, pub counter in front, machines and bottles
    let back_x = bar.x1 - 0.3;
    let bc = Vec2::new(back_x, bar.centre().y);
    f.put(g, "sf_a1961b9e5a", bc, y, Vec2::NEG_X, true);
    let counter = Vec2::new(bar.x1 - 2.5, bar.centre().y);
    // the counter's stools are on its -Z side: turn that toward the room (west)
    f.put(g, "sf_e3fe539054", counter, y, Vec2::X, true);
    f.put(g, "sf_bab05bc780", counter + Vec2::new(0.35, -0.9), y + 1.1, Vec2::NEG_X, false);
    f.put(g, "sf_221f20f292", counter + Vec2::new(0.35, 0.6), y + 1.1, Vec2::NEG_X, false);
    // stock the back bar: rows of bottles and glasses from the glassware pack on its shelves
    let bottles: Vec<String> = (0..40).map(|k| format!("sf_1ea37815fb_{k:02}")).collect();
    for shelf in [0.9f32, 1.3, 1.7, 2.1] {
        let mut z = -2.1f32;
        while z < 2.1 {
            let id = &bottles[(f.rand() * bottles.len() as f32) as usize % bottles.len()];
            let turn = Vec2::from_angle(f.rand() * 6.28);
            let dx = -0.12 + (f.rand() - 0.5) * 0.08;
            f.put(g, id, bc + Vec2::new(dx, z), y + shelf, turn, false);
            z += 0.09 + f.rand() * 0.08;
        }
    }
    f.put(g, "sf_334377879c", bc + Vec2::new(-0.2, 1.4), y + 0.86, Vec2::NEG_X, false);
    for dz in [-1.2f32, 0.0, 1.2] {
        hang(g, f, counter + Vec2::new(-0.3, dz));
    }
    // grinders and cups on the counter
    f.put(g, "sf_2aff2683d3", counter + Vec2::new(0.4, -0.2), y + 1.1, Vec2::NEG_X, false);
    for k in 0..5 {
        let p = counter + Vec2::new(0.25 + (f.rand() - 0.5) * 0.2, 0.15 + k as f32 * 0.14);
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, "sf_c87d04d609", p, y + 1.1, turn, false);
    }
    // a chalkboard by the door, display shelves with plants and a framed poster wall on the core side
    f.put(g, "ph_standing_chalkboard_01", Vec2::new(area.x0 + 0.7, area.z1 - 0.9), y, Vec2::from_angle(0.4), false);
    f.put(g, "ph_wooden_display_shelves_01", Vec2::new(bar.x0 + 0.3, bar.z0 + 0.4), y, Vec2::Y, true);
    // plants: big ones along the window walls and corners, hanging pothos off the joists
    let big = ["sf_1ab9bf841d", "sf_161df9b2f7", "sf_efe25dacea", "sf_0ee05be48d"];
    let spots = [Vec2::new(area.x0 + 0.5, area.z0 + 0.6), Vec2::new(area.x0 + 0.5, area.centre().y), Vec2::new(area.x1 - 0.5, area.z1 - 0.6), Vec2::new(area.x1 - 0.5, area.z0 + 0.6), Vec2::new(bar.x0 + 0.5, bar.z1 - 0.5)];
    for (k, p) in spots.iter().enumerate() {
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, big[k % big.len()], *p, y, turn, false);
    }
    for k in 0..5 {
        let p = Vec2::new(area.x0 + 1.0 + f.rand() * 0.8, area.z0 + 2.0 + k as f32 * (area.d() - 4.0) / 4.0);
        let h = f.bounds("sf_9b961b9aab").map_or(1.0, |(a, b)| b[1] - a[1]);
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, "sf_9b961b9aab", p, y + clear - h, turn, false);
    }
}

/// Things on a cafe table: a cup or two, sometimes a laptop or a latte.
fn table_clutter(g: &mut Geo, f: &mut Furnisher, p: Vec2, top: f32, spread: f32) {
    let n = (f.rand() * 3.0) as i32;
    for _ in 0..n {
        let r = f.rand();
        let id = if r < 0.4 { "sf_c87d04d609" } else if r < 0.7 { "sf_294c5f6a0d" } else if r < 0.85 { "sf_9d2b96b5e3" } else { "sf_a651092825" };
        let off = Vec2::from_angle(f.rand() * 6.28) * (f.rand() * spread);
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, id, p + off, top, turn, false);
    }
}

/// Channel-tufted leather banquette along a wall from `a` to `b` (seat facing `face`), with two-top bistro tables
/// and Tolix chairs opposite, a pendant over each table.
pub fn banquette(g: &mut Geo, f: &mut Furnisher, a: Vec2, b: Vec2, face: Vec2, y: f32, clear: f32) {
    let d = b - a;
    let l = d.length();
    let dir = d / l;
    let face = face.normalize();
    let leather = ins(Key::new(Mat::Surface("pht_fabric_leather_02"), [1.0, 0.68, 0.48]).tile(0.5));
    let wood = ins(Key::new(Mat::Surface("pht_black_walnut_veneer_01"), [0.55, 0.45, 0.38]).tile(1.0));
    let v3 = |p: Vec2, h: f32| Vec3::new(p.x, h, p.y);
    // plinth, seat cushion, then vertical channels for the back
    g.beam(wood, a + face * 0.3, b + face * 0.3, 0.3, y, y + 0.32);
    g.beam(leather, a + face * 0.31, b + face * 0.31, 0.29, y + 0.32, y + 0.46);
    // upholstered back panel with plump vertical channels in front of it
    g.beam(leather, a + face * 0.06, b + face * 0.06, 0.05, y + 0.46, y + 1.14);
    let n = (l / 0.2).floor() as i32;
    for i in 0..n {
        let u = (i as f32 + 0.5) * l / n as f32;
        let p = a + dir * u + face * 0.16;
        g.tube(leather, v3(p, y + 0.5), v3(p - face * 0.05, y + 1.1), 0.1, 12);
    }
    g.beam(wood, a + face * 0.02, b + face * 0.02, 0.06, y + 1.14, y + 1.2);
    g.block_box(v3(a, 0.0), Vec3::new(dir.x, 0.0, dir.y) * l, Vec3::new(face.x, 0.0, face.y) * 0.62, y, y + 0.5);
    // two-tops along it
    let k = (l / 1.6).floor() as i32;
    for i in 0..k {
        let u = (i as f32 + 0.5) * l / k as f32;
        let t = a + dir * u + face * (0.62 + 0.45);
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, "sf_5b9c3706a6", t, y, turn, true);
        if let Some(p) = f.out.last_mut() {
            p.scale = [0.75; 3];
            p.tint = [0.36, 0.29, 0.26];
        }
        table_clutter(g, f, t, y + 0.56, 0.18);
        if f.rand() < 0.85 {
            let pull = 0.45 + f.rand() * 0.3;
            let chair_face = (-face).rotate(Vec2::from_angle((f.rand() - 0.5) * 0.9));
            let id = ["sf_0720ffc9b2", "sf_3fdcc5bc70"][(f.rand() * 2.0) as usize % 2];
            f.put(g, id, t + face * pull, y, -chair_face, false);
        }
        let h = f.bounds("sf_2ba27702aa").map_or(1.25, |(a, b)| b[1] - a[1]);
        f.put(g, "sf_2ba27702aa", t, y + clear - h, Vec2::Y, false);
        g.light(Vec3::new(t.x, y + clear - h - 0.05, t.y), 4.0, [2.2, 1.45, 0.8]);
    }
}

/// A timber window counter on black brackets along an opening (from `a` to `b`, the room on `face`), with stools.
pub fn window_counter(g: &mut Geo, f: &mut Furnisher, a: Vec2, b: Vec2, face: Vec2, y: f32) {
    let d = b - a;
    let l = d.length();
    let dir = d / l;
    let face = face.normalize();
    let top = ins(Key::new(Mat::Surface("pht_oak_veneer_01"), [0.7, 0.55, 0.42]).tile(1.8));
    g.beam(top, a + face * 0.25, b + face * 0.25, 0.22, y + 1.02, y + 1.07);
    let n = (l / 1.2).ceil() as i32;
    for i in 0..=n {
        let p = a + dir * (i as f32 * l / n as f32).clamp(0.05, l - 0.05) + face * 0.12;
        g.beam(black(), p, p + face * 0.3, 0.02, y + 0.95, y + 1.02);
        g.beam(black(), p, p + face * 0.04, 0.02, y, y + 1.02);
    }
    g.block_box(Vec3::new(a.x, 0.0, a.y), Vec3::new(dir.x, 0.0, dir.y) * l, Vec3::new(face.x, 0.0, face.y) * 0.47, y, y + 1.07);
    let m = (l / 0.65).floor() as i32;
    for i in 0..m {
        if f.rand() < 0.15 {
            continue;
        }
        let u = (i as f32 + 0.5) * l / m as f32;
        let p = a + dir * u + face * (0.75 + f.rand() * 0.2);
        let turn = Vec2::from_angle(f.rand() * 6.28);
        f.put(g, "sf_7e438f3903", p, y, turn, false);
        if f.rand() < 0.4 {
            let c = a + dir * u + face * 0.25;
            let turn = Vec2::from_angle(f.rand() * 6.28);
            let id = ["sf_c87d04d609", "sf_294c5f6a0d", "sf_a651092825"][(f.rand() * 3.0) as usize % 3];
            f.put(g, id, c, y + 1.07, turn, false);
        }
    }
}
