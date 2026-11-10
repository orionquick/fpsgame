//! The three-storey red-brick former factory (now offices and a restaurant) facing the ballpark's outfield park,
//! OSM way/376480375, matched to street-level reference:
//! - west (park) front: corner piers, a narrow north bay then five wide bays; big multi-light steel sashes upstairs;
//!   glazed sectional garage doors and green storefronts below; cast-stone blocks capping the ground-floor piers;
//!   a raised parapet over the middle bays with a framed sign panel and two diamond plaques; corbelled brick band.
//! - south front (stadium side): six wide bays, flat parapet.
//! - north: two windowed bays at the west end, the rest is a party wall; east: party wall.
use super::geom::{wear, Geo, Hole, Key, Mat};
use super::plan::{self, Rect};
use super::{furnish, interior, lift_doors, loft, m, slab, stair, stair_hole, Building};
use glam::{Vec2, Vec3};

pub const OSM: &str = "way/376480375";

/// Interior face of the outer walls: Megascans old brick wall (aged, dusty, irregular).
const BRICK_IN: &str = "tc2jfcwn";
const T: f32 = 0.33; // exterior wall: 13 in. of brick
const LEVELS: [(f32, f32); 3] = [(0.0, 4.6), (4.6, 3.9), (8.5, 3.9)];
const ROOF: f32 = 12.4;
const PARAPET: f32 = 13.3;
const GROUND_HEAD: f32 = 3.5;
/// Upper window sill height and window height per upper floor.
const SILL: f32 = 0.85;
const WIN_H: [f32; 2] = [2.3, 2.45];

#[derive(Clone, Copy, PartialEq)]
enum Ground {
    Garage,
    Store,
    Entry,
}

struct Face {
    a: Vec2,
    b: Vec2,
    /// Window spans along the face (u from `a`).
    bays: Vec<(f32, f32)>,
    /// Ground-floor fill per bay; None = brick (no opening).
    ground: Vec<Option<Ground>>,
    /// Exposed span (decorated); outside it the wall is a plain party wall.
    exposed: (f32, f32),
    arched: Option<(f32, f32, f32)>,
}

impl Face {
    fn len(&self) -> f32 {
        (self.b - self.a).length()
    }
    fn dir(&self) -> Vec2 {
        (self.b - self.a) / self.len()
    }
    /// Outward normal in plan.
    fn out(&self) -> Vec2 {
        let d = self.dir();
        Vec2::new(-d.y, d.x)
    }
    /// Plan point at distance `u` along the face, `o` metres outside the outer face.
    fn at(&self, u: f32, o: f32) -> Vec2 {
        self.a + self.dir() * u + self.out() * o
    }
    /// Box aligned with the face: u0..u1 along, y0..y1, o0..o1 across (positive = outside).
    fn bx(&self, g: &mut Geo, k: Key, u0: f32, u1: f32, y0: f32, y1: f32, o0: f32, o1: f32) {
        let oc = (o0 + o1) / 2.0;
        g.beam(k, self.at(u0, oc), self.at(u1, oc), (o1 - o0).abs() / 2.0, y0, y1);
    }
    /// Pier spans between the bays (including the corners).
    fn piers(&self) -> Vec<(f32, f32)> {
        let mut out = vec![];
        let mut last = 0.0;
        for &(a, b) in &self.bays {
            out.push((last, a));
            last = b;
        }
        out.push((last, self.len()));
        out
    }
    fn parapet_top(&self, u: f32) -> f32 {
        match self.arched {
            Some((u0, u1, rise)) if u > u0 && u < u1 => {
                // flattened arch: curved shoulders rising into a level crown
                let t = (u - u0) / (u1 - u0);
                let s = (t.min(1.0 - t) / 0.3).min(1.0);
                PARAPET + rise * (1.0 - (1.0 - s).powi(2)).sqrt()
            }
            _ => PARAPET,
        }
    }
}

/// Bay spans: corner piers of `corner`, the given window widths, equal piers between.
fn bays(len: f32, corner: f32, widths: &[f32]) -> Vec<(f32, f32)> {
    let pier = (len - 2.0 * corner - widths.iter().sum::<f32>()) / (widths.len() - 1) as f32;
    let mut u = corner;
    widths
        .iter()
        .map(|&w| {
            let s = (u, u + w);
            u += w + pier;
            s
        })
        .collect()
}

pub fn build(c: &city::City, cat: &assets::Catalog) -> Option<Building> {
    let b = c.buildings.iter().find(|b| b.osm == OSM)?;
    // footprint corners: NW, SW, SE, NE (lidar-trimmed OSM polygon has an extra point on the north edge)
    let pts: Vec<Vec2> = b.outer.iter().map(|p| Vec2::new(p[0] as f32, p[1] as f32)).collect();
    let nw = *pts.iter().min_by(|a, b| (a.x + a.y).total_cmp(&(b.x + b.y)))?;
    let ne = *pts.iter().min_by(|a, b| (-a.x + a.y).total_cmp(&(-b.x + b.y)))?;
    let sw = *pts.iter().min_by(|a, b| (a.x - a.y).total_cmp(&(b.x - b.y)))?;
    let ex = (ne - nw).normalize();
    let w = (ne - nw).length();
    let d = (sw - nw).length();
    // ground floor a step above the highest ground under the footprint, so no terrain shows inside
    let ez = Vec2::new(-ex.y, ex.x);
    let mut base = f32::MIN;
    for i in 0..=12 {
        for j in 0..=12 {
            let p = nw + ex * (w * i as f32 / 12.0) + ez * (d * j as f32 / 12.0);
            base = base.max(c.terrain.at(p.x as f64, p.y as f64) as f32);
        }
    }
    let base = base + 0.12;
    let mut g = Geo::new(Vec3::new(nw.x, base, nw.y), Vec3::new(ex.x, 0.0, ex.y));

    let brick_id: &'static str = match std::env::var("FPS_BRICK").ok().as_deref() {
        Some(s) => Box::leak(s.to_string().into_boxed_str()),
        None => BRICK,
    };
    let tint = match brick_id {
        "ue3iaeifw" => [1.08, 0.92, 0.98],
        "udlmfczo" => [0.88, 0.6, 0.56],
        "scxriad" => [1.05, 1.05, 2.1],
        _ => BRICK_TINT,
    };
    let brick = Key::new(Mat::Surface(brick_id), tint);
    // converted loft: the outer walls are exposed brick inside too
    // inside: old, irregular, dusty red-brown brick with pale mortar (as in the interior photos)
    let brick_in = Key::new(Mat::Surface(BRICK_IN), [1.05, 1.0, 1.0]).inside().wear(wear::DUSTY);
    let stone = Key::new(Mat::Surface(m::CONCRETE), [0.86, 0.8, 0.72]);
    let plinth = Key::new(Mat::Surface(m::CONCRETE), [0.42, 0.38, 0.35]);
    let sill_in = Key::new(Mat::Plain(160), [0.75, 0.74, 0.71]).inside();
    let steel = Key::new(Mat::Plain(120), [0.03, 0.035, 0.035]).wear(wear::METAL);
    let green = Key::new(Mat::Plain(100), [0.05, 0.13, 0.09]).wear(wear::METAL);
    let alu = Key::new(Mat::Plain(90), [0.06, 0.065, 0.07]);
    let glass = Key::new(Mat::Glass, [0.55, 0.62, 0.6]);

    let v = Vec2::new;
    let faces = [
        // west: from the NW corner south; narrow bay at the north end
        Face {
            a: v(0.0, 0.0),
            b: v(0.0, d),
            bays: bays(d, 1.2, &[2.4, 3.6, 3.6, 3.6, 3.6, 3.6]),
            ground: vec![Some(Ground::Entry), Some(Ground::Store), Some(Ground::Garage), Some(Ground::Store), Some(Ground::Garage), Some(Ground::Store)],
            exposed: (0.0, d),
            arched: Some((3.4, d - 3.4, 1.5)),
        },
        // south: from SW east
        Face {
            a: v(0.0, d),
            b: v(w, d),
            bays: bays(w, 1.2, &[3.6; 6]),
            ground: vec![Some(Ground::Store), Some(Ground::Garage), Some(Ground::Store), Some(Ground::Garage), Some(Ground::Garage), Some(Ground::Entry)],
            exposed: (0.0, w),
            arched: None,
        },
        // east party wall
        Face { a: v(w, d), b: v(w, 0.0), bays: vec![], ground: vec![], exposed: (0.0, 0.0), arched: None },
        // north: from NE west; two bays at the exposed west end
        Face {
            a: v(w, 0.0),
            b: v(0.0, 0.0),
            bays: vec![(w - 1.2 - 3.0 - 1.4 - 3.0, w - 1.2 - 3.0 - 1.4), (w - 1.2 - 3.0, w - 1.2)],
            ground: vec![None, Some(Ground::Entry)],
            exposed: (w - 10.9, w),
            arched: None,
        },
    ];

    for f in &faces {
        let len = f.len();
        let mut holes = vec![];
        for (i, &(u0, u1)) in f.bays.iter().enumerate() {
            if f.ground[i].is_some() {
                holes.push(Hole { u0, u1, y0: 0.0, y1: GROUND_HEAD });
            }
            for (k, &(y, _)) in LEVELS[1..].iter().enumerate() {
                holes.push(Hole { u0, u1, y0: y + SILL, y1: y + SILL + WIN_H[k] });
            }
        }
        g.wall(brick, brick_in, f.a, f.b, -1.5, ROOF, T, &holes, false);
        // parapet: brick both sides, in short pieces so the arched centre reads as a curve
        let n = (len / 0.4).ceil() as usize;
        for i in 0..n {
            let (ua, ub) = (len * i as f32 / n as f32, len * (i + 1) as f32 / n as f32);
            let (ya, yb) = (f.parapet_top(ua), f.parapet_top(ub));
            g.wall_sloped(brick, brick, f.at(ua, 0.0), f.at(ub, 0.0), ROOF, ya, yb, T);
            // cast-stone coping, slightly proud on both sides
            g.wall_sloped(stone, stone, f.at(ua, 0.05), f.at(ub, 0.05), ya.min(yb) - 0.06, ya + 0.09, yb + 0.09, T + 0.1);
        }
        if f.exposed.1 - f.exposed.0 < 0.1 {
            continue;
        }
        let (e0, e1) = f.exposed;
        // corbelled band under the parapet and a soldier course at the second-floor line
        f.bx(&mut g, brick, e0, e1, 12.0, 12.25, 0.0, 0.06);
        f.bx(&mut g, brick, e0, e1, 11.95, 12.0, 0.0, 0.03);
        // piers: cast-stone caps level with the opening heads, a low stone plinth
        for (p0, p1) in f.piers() {
            if p1 <= e0 || p0 >= e1 || p1 - p0 < 0.05 {
                continue;
            }
            let (p0, p1) = (p0.max(e0), p1.min(e1));
            f.bx(&mut g, stone, p0 - 0.03, p1 + 0.03, GROUND_HEAD, GROUND_HEAD + 0.4, 0.0, 0.04);
            f.bx(&mut g, plinth, p0 - 0.01, p1 + 0.01, -1.5, 0.18, 0.0, 0.025);
        }
        // downspouts at the ends of the decorated span, gooseneck lamps over the ground-floor openings
        let pipe = Key::new(Mat::Plain(90), [0.1, 0.1, 0.1]);
        for u in [e0 + 0.35, e1 - 0.35] {
            let p = f.at(u, 0.12);
            g.tube(pipe, Vec3::new(p.x, 0.05, p.y), Vec3::new(p.x, ROOF - 0.2, p.y), 0.055, 10);
            f.bx(&mut g, pipe, u - 0.15, u + 0.15, ROOF - 0.5, ROOF - 0.1, 0.0, 0.25);
            f.bx(&mut g, pipe, u - 0.12, u + 0.12, 0.0, 0.25, 0.0, 0.22);
        }
        for (i, &(u0, u1)) in f.bays.iter().enumerate() {
            if f.ground[i].is_none() {
                continue;
            }
            let u = (u0 + u1) / 2.0;
            let (w0, w1) = (f.at(u, 0.0), f.at(u, 0.55));
            let y = GROUND_HEAD + 0.75;
            g.tube(pipe, Vec3::new(w0.x, y, w0.y), Vec3::new(w1.x, y, w1.y), 0.018, 6);
            g.tube(pipe, Vec3::new(w1.x, y, w1.y), Vec3::new(w1.x, y - 0.18, w1.y), 0.018, 6);
            g.tube(pipe, Vec3::new(w1.x, y - 0.32, w1.y), Vec3::new(w1.x, y - 0.18, w1.y), 0.2, 14);
            f.bx(&mut g, pipe, u - 0.08, u + 0.08, y - 0.1, y + 0.1, 0.0, 0.03);
        }
        // windows
        for (i, &(u0, u1)) in f.bays.iter().enumerate() {
            for (k, &(y, _)) in LEVELS[1..].iter().enumerate() {
                let (y0, y1) = (y + SILL, y + SILL + WIN_H[k]);
                let cols = if u1 - u0 > 3.0 { 8 } else { 5 };
                sash(&mut g, f, u0, u1, y0, y1, cols, 6, steel, glass);
                // stone sill outside, a painted timber stool board inside
                f.bx(&mut g, stone, u0 - 0.06, u1 + 0.06, y0 - 0.1, y0 + 0.01, -0.1, 0.06);
                f.bx(&mut g, sill_in, u0 - 0.05, u1 + 0.05, y0 - 0.005, y0 + 0.03, -T - 0.035, -0.13);
                f.bx(&mut g, sill_in, u0 - 0.05, u1 + 0.05, y0 - 0.12, y0 - 0.005, -T - 0.02, -T + 0.01);
            }
            match f.ground[i] {
                Some(Ground::Garage) => garage(&mut g, f, u0, u1, alu, glass),
                Some(Ground::Store) => storefront(&mut g, f, u0, u1, green, glass, false),
                Some(Ground::Entry) => storefront(&mut g, f, u0, u1, green, glass, true),
                None => {}
            }
        }
        // sign panel and plaques on the raised parapet
        if let Some((a0, a1, _)) = f.arched {
            let (s0, s1, y0, y1) = (a0 + 3.6, a1 - 3.6, 12.55, 13.55);
            for (ua, ub, ya, yb) in [(s0, s1, y0, y0 + 0.12), (s0, s1, y1 - 0.12, y1), (s0, s0 + 0.14, y0, y1), (s1 - 0.14, s1, y0, y1)] {
                f.bx(&mut g, brick, ua, ub, ya, yb, 0.0, 0.05);
            }
            for u in [s0 - 1.1, s1 + 1.1] {
                diamond(&mut g, f, u, (y0 + y1) / 2.0, 0.42, stone);
            }
            let paint = Key::new(Mat::Plain(220), [0.8, 0.76, 0.64]);
            sign(&mut g, f, "CANDY FACTORY", (s0 + s1) / 2.0, (y0 + y1) / 2.0, 0.105, paint);
        }
    }

    // floors
    let input = plan::Input {
        w,
        d,
        t: T,
        piers_n: faces[3].piers().iter().map(|p| w - (p.0 + p.1) / 2.0).collect(),
        piers_w: faces[0].piers().iter().map(|p| (p.0 + p.1) / 2.0).collect(),
        piers_s: faces[1].piers().iter().map(|p| (p.0 + p.1) / 2.0).collect(),
        seed: 376480375,
    };
    let inner = Rect::new(T, T, w - T, d - T);
    let ground = plan::ground(&input);
    let core = ground.core.clone();
    let win = furnish::Windows {
        north: faces[3].bays.iter().map(|b| w - (b.0 + b.1) / 2.0).collect(),
        south: faces[1].bays.iter().map(|b| (b.0 + b.1) / 2.0).collect(),
        west: faces[0].bays.iter().map(|b| (b.0 + b.1) / 2.0).collect(),
    };
    let mut fur = furnish::Furnisher::new(cat, input.seed);
    let _ = &win;
    // structural grid: girders on the west front's interior piers, three column lines across
    let piers = faces[0].piers();
    let spec = loft::Spec {
        w,
        d,
        t: T,
        girders: piers[1..piers.len() - 1].iter().map(|p| (p.0 + p.1) / 2.0).collect(),
        cols: vec![T + (w - 2.0 * T) * 0.25, T + (w - 2.0 * T) * 0.5, T + (w - 2.0 * T) * 0.75],
        core: core.clone(),
    };
    for (k, &(y, _)) in LEVELS.iter().enumerate() {
        let top = if k + 1 < LEVELS.len() { LEVELS[k + 1].0 } else { ROOF };
        let h = top - y;
        let clear = loft::Spec::clear(h);
        let holes = if k == 0 { vec![] } else { vec![stair_hole(core.stair), core.elevator] };
        slab(&mut g, inner, y, if k == 0 { 0.3 } else { 0.09 }, &holes);
        interior(&mut g, &ground, y, clear, 10 + k as u64, true, k == 0);
        loft::structure(&mut g, &spec, y, h, k as u64, k != 0 && !super::undecorated());
        wall_members(&mut g, &faces, &spec, y, top - loft::DECK, w);
        if k + 1 < LEVELS.len() {
            stair(&mut g, core.stair, y, LEVELS[k + 1].0 - y);
        }
        lift_doors(&mut g, core.elevator, y);
        // one reflection probe per floor: the open plate between the slab and the deck above
        g.probe(Vec3::new((T + core.r.x0) * 0.5, y + 1.7, d * 0.5), Vec3::new(T, y, T), Vec3::new(w - T, y + h - 0.1, d - T));
        if super::undecorated() {
            // the empty hall of the photos: metal dome high-bays down the bays, nothing else
            let mut zs = vec![T];
            zs.extend(spec.girders.iter().copied());
            zs.push(d - T);
            for j in 0..zs.len() - 1 {
                let zc = (zs[j] + zs[j + 1]) * 0.5;
                let mut x = T + 2.0;
                while x < core.r.x0 - 1.0 {
                    let p = v(x, zc);
                    let hh = fur.bounds("ph_hanging_industrial_lamp").map_or(1.36, |(a, b)| b[1] - a[1]);
                    fur.put(&mut g, "ph_hanging_industrial_lamp", p, y + clear - hh, v(0.0, 1.0), false);
                    g.light(Vec3::new(p.x, y + clear - hh - 0.1, p.y), 6.0, [2.4, 2.1, 1.7]);
                    x += 3.8;
                }
            }
            continue;
        }
        // zones kept free of desks: meeting rooms, kitchen, lift lobby
        let ne = Rect::new(w - T - 11.0, T, w - T, 6.0);
        let se = Rect::new(w - T - 9.0, d - T - 7.2, w - T, d - T);
        let kitchen = Rect::new(core.r.x0 - 0.5, 6.0, w - T, core.r.z0);
        let lobby = Rect::new(core.r.x0 - 2.6, core.r.z0 - 0.5, core.r.x0, core.r.z1 + 0.5);
        let mut reserved = vec![lobby];
        let mut pads = vec![];
        if k == 0 {
            // ground floor: cafe by the garage doors, bar in front of the core, lounge at the north entry
            let cafe = Rect::new(T + 1.7, T + 5.0, spec.cols[1] - 0.6, d - T - 1.0);
            // counters with stools behind the glazed garage doors on the park side (storefronts keep their doors clear)
            for (i, &(u0, u1)) in faces[0].bays.iter().enumerate() {
                if matches!(faces[0].ground[i], Some(Ground::Garage)) {
                    loft::window_counter(&mut g, &mut fur, v(T + 0.05, u0 + 0.2), v(T + 0.05, u1 - 0.2), v(1.0, 0.0), y);
                }
            }
            // a channel-tufted banquette along the party wall north of the cafe, two-tops in front
            loft::banquette(&mut g, &mut fur, v(10.0, T), v(core.r.x0 - 3.0, T), v(0.0, 1.0), y, clear);
            let bar = Rect::new(core.r.x0 - 1.0, core.r.z1 + 0.6, w - T, d - T - 0.6);
            loft::cafe(&mut g, &mut fur, cafe, bar, y, clear);
            // a poster wall on the timber-clad core facing the room
            fur.put(&mut g, "sf_2bc72a0509", v(core.r.x0 - 0.2, core.r.z0 + 6.8), y + 1.1, v(-1.0, 0.0), false);
            reserved.push(cafe);
            reserved.push(bar);
            fur.put(&mut g, "ph_sofa_03", v(4.6, 1.1), y, v(0.0, 1.0), true);
            fur.put(&mut g, "ph_modern_coffee_table_02", v(4.5, 2.6), y, v(0.2, 1.0), true);
            fur.put(&mut g, "ph_modern_arm_chair_01", v(2.4, 3.4), y, v(0.8, -0.6), true);
            fur.put(&mut g, "ph_modern_arm_chair_01", v(6.7, 3.5), y, v(-0.7, -0.7), true);
            fur.put(&mut g, "ph_potted_plant_01", v(1.0, 1.0), y, v(0.3, 1.0), false);
        } else {
            loft::glass_room(&mut g, ne, y, clear, &[3, 0]);
            loft::meeting(&mut g, &mut fur, ne, y, clear, v(1.0, 0.0));
            loft::glass_room(&mut g, se, y, clear, &[0, 1]);
            loft::meeting(&mut g, &mut fur, se, y, clear, v(1.0, 0.0));
            loft::kitchen(&mut g, &mut fur, v(core.r.x0, core.r.z0), v(w - T, core.r.z0), y);
            reserved.extend([ne, se, kitchen]);
        }
        // desk benches in the bays between girders and column lines, clear of everything reserved
        let mut zs = vec![T];
        zs.extend(spec.girders.iter().copied());
        zs.push(d - T);
        let mut xs = vec![T];
        xs.extend(spec.cols.iter().copied());
        xs.push(core.r.x0 - 2.6);
        for i in 0..xs.len() - 1 {
            for j in 0..zs.len() - 1 {
                let (xa, xb, za, zb) = (xs[i] + 0.5, xs[i + 1] - 0.5, zs[j], zs[j + 1]);
                let pairs = ((xb - xa - 1.0) / 1.8).floor() as i32;
                if pairs < 1 || zb - za < 4.2 {
                    continue;
                }
                let c = v((xa + xb) / 2.0, (za + zb) / 2.0);
                let half = v(pairs as f32 * 0.9 + 0.3, 2.3);
                let r = Rect::new(c.x - half.x, c.y - half.y, c.x + half.x, c.y + half.y);
                if reserved.iter().any(|q| q.x0 < r.x1 && q.x1 > r.x0 && q.z0 < r.z1 && q.z1 > r.z0) {
                    continue;
                }
                // one bay per floor is a lounge by the windows instead
                if i == 0 && j == 2 + k {
                    fur.put(&mut g, "sf_959bc21f62", c - v(0.0, 1.2), y, v(0.0, 1.0), true);
                    fur.put(&mut g, "sf_ddab2f2c25", c + v(1.3, 0.9), y, v(-0.7, -0.6), true);
                    fur.put(&mut g, "ph_modern_arm_chair_01", c + v(-1.3, 0.9), y, v(0.7, -0.6), true);
                    fur.put(&mut g, "ph_coffee_table_round_01", c, y, v(0.0, 1.0), true);
                    fur.put(&mut g, "ph_potted_plant_01", c + v(-2.2, -1.3), y, v(1.0, 0.3), false);
                    continue;
                }
                // the Office Props Pack cubicles where a full pinwheel fits, modelled benches elsewhere
                let kit_r = Rect::new(c.x - 2.75, c.y - 2.75, c.x + 2.75, c.y + 2.75);
                if k == 0 {
                    // the ground floor is the cafe: leftover bays get lounge seating, not desks
                    fur.put(&mut g, "sf_959bc21f62", c - v(0.0, 1.0), y, v(0.0, 1.0), true);
                    fur.put(&mut g, "ph_modern_arm_chair_01", c + v(1.2, 0.8), y, v(-0.7, -0.6), true);
                    fur.put(&mut g, "ph_coffee_table_round_01", c, y, v(0.0, 1.0), true);
                    continue;
                }
                if xb - xa >= 5.6 && !reserved.iter().any(|q| q.x0 < kit_r.x1 && q.x1 > kit_r.x0 && q.z0 < kit_r.z1 && q.z1 > kit_r.z0) {
                    pads.push(loft::kit_cluster(&mut g, &mut fur, c, y));
                } else {
                    pads.push(loft::cubicles(&mut g, &mut fur, c, v(1.0, 0.0), pairs, y));
                }
            }
        }
        loft::floor(&mut g, &spec, y, &pads);
        // framed prints, a clock and a fire alarm on the core's front wall
        let cx = core.r.x0 - 0.1;
        for (k2, z) in [core.r.z0 + 2.2, core.r.z0 + 6.6, core.r.z1 - 1.2].iter().enumerate() {
            let id = ["ph_hanging_picture_frame_01", "ph_hanging_picture_frame_02", "ph_hanging_picture_frame_03"][(k2 + k) % 3];
            fur.put(&mut g, id, v(cx - 0.02, *z), y + 1.3, v(-1.0, 0.0), false);
        }
        fur.put(&mut g, "ph_wall_clock", v(cx - 0.03, core.r.z0 + 4.4), y + 2.3, v(-1.0, 0.0), false);
        fur.put(&mut g, "ph_fire_alarm", v(cx - 0.02, core.r.z0 + 1.6), y + 1.4, v(-1.0, 0.0), false);
        // plants along the window walls
        for z in [7.0f32, 16.0, 25.0] {
            let turn = Vec2::from_angle(fur.rand() * 6.28);
            fur.put(&mut g, ["sikoN", "siumT", "wfypejzha"][(z as usize / 7) % 3], v(T + 0.55, z), y, turn, false);
        }
    }
    // patio on the park side: cafe tables outside the garage doors, a row of planters, potted palms at the entries
    fur.indoor = false;
    let west = &faces[0];
    for (i, &(u0, u1)) in west.bays.iter().enumerate() {
        let u = (u0 + u1) / 2.0;
        let at = |o: f32, du: f32| west.a + west.dir() * (u + du) + west.out() * o;
        match west.ground[i] {
            Some(Ground::Garage) | Some(Ground::Store) => {
                for du in [-0.9f32, 0.9] {
                    let c = at(2.3 + (fur.rand() - 0.5) * 0.4, du + (fur.rand() - 0.5) * 0.3);
                    let tdir = west.dir().rotate(Vec2::from_angle((fur.rand() - 0.5) * 1.2));
                    fur.put(&mut g, "ukphdffaw", c, 0.0, tdir, true);
                    let n2 = Vec2::new(-tdir.y, tdir.x);
                    for (k, side) in [tdir, -tdir, n2, -n2].iter().enumerate() {
                        if fur.rand() < 0.7 || k < 2 {
                            let pull = 0.5 + fur.rand() * 0.35;
                            let face = (-*side).rotate(Vec2::from_angle((fur.rand() - 0.5) * 1.0));
                            fur.put(&mut g, "uknkaffaw", c + *side * pull, 0.0, face, false);
                        }
                    }
                }
            }
            Some(Ground::Entry) => {
                for du in [-1.0f32, 1.0] {
                    fur.put(&mut g, "shqke", at(0.45, du * 1.35), 0.0, west.out(), false);
                }
            }
            None => {}
        }
        let c = at(4.0, 0.0);
        let turn = Vec2::from_angle(fur.rand() * 6.28);
        fur.put(&mut g, "vbljdakjw", c, 0.0, west.dir(), true);
        fur.put(&mut g, "wfypfa3ha", c, 0.4, turn, false);
    }
    // street trees (jacarandas, as on the real plaza) and pedestrian lamps
    for (k, u) in [5.0f32, 15.0, 25.0].iter().enumerate() {
        let p = west.a + west.dir() * *u + west.out() * 8.5;
        fur.put_scaled(&mut g, "ph_jacaranda_tree", p, -0.1, Vec2::from_angle(k as f32 * 2.1), 0.42, false);
        let l = west.a + west.dir() * (*u + 5.0) + west.out() * 6.2;
        fur.put(&mut g, "ph_street_lamp_01", l, 0.0, west.out(), true);
    }
    // plaza furniture: benches between the trees facing the front, bike racks and a newsbox by the north entry,
    // bins at the doors, a sidewalk clock at the corner (City Props Collection by TampaJoey)
    for u in [10.0f32, 20.0] {
        let p = west.a + west.dir() * u + west.out() * 7.6;
        fur.put(&mut g, "sf_32b8b82bb1_06", p, 0.0, -west.out(), true);
    }
    for (k, u) in [-1.6f32, -2.6, -3.6].iter().enumerate() {
        let p = west.a + west.dir() * (2.0 + k as f32 * 0.9) + west.out() * (5.0 - u * 0.0);
        fur.put(&mut g, "sf_2f52ea53c5_25", p, 0.0, west.dir().rotate(Vec2::from_angle(0.05 * k as f32)), true);
    }
    fur.put(&mut g, "sf_8975781045", west.a + west.dir() * 0.4 + west.out() * 1.8, 0.0, west.out(), true);
    for u in [3.1f32, 27.0] {
        fur.put(&mut g, "sf_32b8b82bb1_16", west.a + west.dir() * u + west.out() * 0.7, 0.0, west.out(), true);
    }
    fur.put(&mut g, "sf_2f52ea53c5_22", west.a + west.dir() * (d + 2.5) + west.out() * 5.0, 0.0, west.out(), true);
    for (k, (x, z)) in [(23.0f32, 4.0f32), (23.0, 8.0), (27.0, 4.0), (27.0, 8.0)].iter().enumerate() {
        fur.put(&mut g, "ph_exterior_aircon_unit", v(*x, *z + 14.0), ROOF + 0.02, if k % 2 == 0 { v(1.0, 0.0) } else { v(-1.0, 0.0) }, true);
    }
    fur.indoor = true;
    // roof: slab, gravel, a monitor skylight and two rooftop units
    slab(&mut g, inner, ROOF, 0.3, &[]);
    let gravel = Key::new(Mat::Surface(m::ROOF_GRAVEL), [0.75, 0.74, 0.72]);
    g.quad(gravel, Vec3::new(T, ROOF + 0.01, d - T), Vec3::X * (w - 2.0 * T), Vec3::NEG_Z * (d - 2.0 * T));
    let metal = Key::new(Mat::Plain(110), [0.42, 0.43, 0.44]);
    let (mx0, mz0, mx1, mz1) = (6.5, 10.5, 15.5, 19.5);
    g.solid(metal, Vec3::new(mx0, ROOF, mz0), Vec3::new(mx1, ROOF + 0.25, mz1));
    for (a, b) in [((mx0, mz0), (mx1, mz0)), ((mx1, mz0), (mx1, mz1)), ((mx1, mz1), (mx0, mz1)), ((mx0, mz1), (mx0, mz0))] {
        let (a, b) = (v(a.0, a.1), v(b.0, b.1));
        let dd = b - a;
        g.quad(glass, Vec3::new(a.x, ROOF + 0.25, a.y), Vec3::new(dd.x, 0.0, dd.y), Vec3::Y * 0.9);
        let l = dd.length();
        for s in 0..=((l / 1.5) as i32) {
            let p = a + dd * (s as f32 * 1.5 / l).min(1.0);
            g.cuboid(steel, Vec3::new(p.x - 0.04, ROOF + 0.25, p.y - 0.04), Vec3::new(p.x + 0.04, ROOF + 1.15, p.y + 0.04));
        }
    }
    g.solid(metal, Vec3::new(mx0 - 0.2, ROOF + 1.15, mz0 - 0.2), Vec3::new(mx1 + 0.2, ROOF + 1.3, mz1 + 0.2));

    Some(Building { osm: OSM, geo: g, furniture: fur.out })
}

/// Default facade brick (Megascans "Red Brick Wall") and tint matched to the reference's orange-red.
pub const BRICK: &str = "udlmfczo";
pub const BRICK_TINT: [f32; 3] = [0.84, 0.76, 0.72];

/// Industrial steel sash: heavy perimeter frame, thin glazing bars, clear lites, set back in the opening.
fn sash(g: &mut Geo, f: &Face, u0: f32, u1: f32, y0: f32, y1: f32, cols: usize, rows: usize, steel: Key, glass: Key) {
    let o = -0.1;
    let fr = 0.06;
    f.bx(g, steel, u0, u1, y0, y0 + fr, o - 0.04, o + 0.04);
    f.bx(g, steel, u0, u1, y1 - fr, y1, o - 0.04, o + 0.04);
    f.bx(g, steel, u0, u0 + fr, y0, y1, o - 0.04, o + 0.04);
    f.bx(g, steel, u1 - fr, u1, y0, y1, o - 0.04, o + 0.04);
    // window units split by heavy mullions, each a grid of small lites
    let units = if cols >= 8 { 2 } else { 1 };
    for c in 1..cols {
        let u = u0 + (u1 - u0) * c as f32 / cols as f32;
        let heavy = units > 1 && c * units % cols == 0;
        let wdt = if heavy { 0.07 } else { 0.022 };
        let dep = if heavy { 0.045 } else { 0.022 };
        f.bx(g, steel, u - wdt / 2.0, u + wdt / 2.0, y0, y1, o - dep, o + dep);
    }
    for r in 1..rows {
        let y = y0 + (y1 - y0) * r as f32 / rows as f32;
        f.bx(g, steel, u0, u1, y - 0.012, y + 0.012, o - 0.025, o + 0.025);
    }
    let p = f.at(u0, o);
    let dd = f.at(u1, o) - p;
    g.quad(glass, Vec3::new(p.x, y0, p.y), Vec3::new(dd.x, 0.0, dd.y), Vec3::Y * (y1 - y0));
}

/// Glazed sectional garage door (restaurant frontage): aluminium rails and stiles over clear panels.
fn garage(g: &mut Geo, f: &Face, u0: f32, u1: f32, alu: Key, glass: Key) {
    let o = -0.19;
    let (y0, y1) = (0.0, GROUND_HEAD - 0.05);
    // guide channels and header
    f.bx(g, alu, u0, u0 + 0.09, y0, GROUND_HEAD, o - 0.06, o + 0.06);
    f.bx(g, alu, u1 - 0.09, u1, y0, GROUND_HEAD, o - 0.06, o + 0.06);
    f.bx(g, alu, u0, u1, y1, GROUND_HEAD, o - 0.06, o + 0.06);
    let rows = 5;
    for r in 0..=rows {
        let y = y0 + (y1 - y0) * r as f32 / rows as f32;
        f.bx(g, alu, u0 + 0.09, u1 - 0.09, (y - 0.04).max(0.0), (y + 0.04).min(y1), o - 0.03, o + 0.03);
    }
    for c in 1..4 {
        let u = u0 + (u1 - u0) * c as f32 / 4.0;
        f.bx(g, alu, u - 0.035, u + 0.035, y0, y1, o - 0.03, o + 0.03);
    }
    let p = f.at(u0, o);
    let dd = f.at(u1, o) - p;
    g.quad(glass, Vec3::new(p.x, y0, p.y), Vec3::new(dd.x, 0.0, dd.y), Vec3::Y * (y1 - y0));
}

/// Storefront: painted steel mullions, transom, kick panels; a glazed door in the middle (`entry`: recessed pair).
fn storefront(g: &mut Geo, f: &Face, u0: f32, u1: f32, green: Key, glass: Key, entry: bool) {
    let o = if entry { -0.3 } else { -0.18 };
    let transom = 2.65;
    let wdt = u1 - u0;
    let (d0, d1) = if entry { (u0 + wdt * 0.2, u1 - wdt * 0.2) } else { (u0 + wdt / 2.0 - 0.55, u0 + wdt / 2.0 + 0.55) };
    let m = 0.07;
    for u in [u0, d0, d1, u1 - m] {
        let (a, b) = if u == d1 { (u, u + m) } else if u == d0 { (u - m, u) } else { (u, u + m) };
        f.bx(g, green, a, b, 0.0, GROUND_HEAD, o - 0.05, o + 0.05);
    }
    f.bx(g, green, u0, u1, transom - m / 2.0, transom + m / 2.0, o - 0.05, o + 0.05);
    // small-paned transom band
    let tn = ((u1 - u0) / 0.42).round().max(2.0) as i32;
    for i in 1..tn {
        let u = u0 + (u1 - u0) * i as f32 / tn as f32;
        f.bx(g, green, u - 0.015, u + 0.015, transom, GROUND_HEAD, o - 0.025, o + 0.025);
    }
    f.bx(g, green, u0, u1, (transom + GROUND_HEAD) / 2.0 - 0.015, (transom + GROUND_HEAD) / 2.0 + 0.015, o - 0.025, o + 0.025);
    f.bx(g, green, u0, u1, GROUND_HEAD - m, GROUND_HEAD, o - 0.05, o + 0.05);
    // kick panels under the side lites
    f.bx(g, green, u0 + m, d0 - m, 0.0, 0.4, o - 0.03, o + 0.03);
    f.bx(g, green, d1 + m, u1 - m, 0.0, 0.4, o - 0.03, o + 0.03);
    // door leaves: stiles and rails around a lite, a push bar
    let leaves: Vec<(f32, f32)> = if entry { vec![(d0, (d0 + d1) / 2.0), ((d0 + d1) / 2.0, d1)] } else { vec![(d0, d1)] };
    for (a, b) in leaves {
        f.bx(g, green, a, a + 0.1, 0.0, 2.3, o - 0.03, o + 0.03);
        f.bx(g, green, b - 0.1, b, 0.0, 2.3, o - 0.03, o + 0.03);
        f.bx(g, green, a, b, 0.0, 0.25, o - 0.03, o + 0.03);
        f.bx(g, green, a, b, 2.2, 2.3, o - 0.03, o + 0.03);
        f.bx(g, green, a + 0.15, b - 0.15, 1.0, 1.04, o + 0.03, o + 0.08);
    }
    f.bx(g, green, d0, d1, 2.3, 2.38, o - 0.05, o + 0.05);
    let p = f.at(u0, o);
    let dd = f.at(u1, o) - p;
    g.quad(glass, Vec3::new(p.x, 0.0, p.y), Vec3::new(dd.x, 0.0, dd.y), Vec3::Y * GROUND_HEAD);
    // the recess floor and soffit at an entry
    if entry {
        let stone = Key::new(Mat::Surface(m_concrete()), [0.85, 0.83, 0.8]);
        let a = f.at(u0, 0.0);
        let b = f.at(u0, o);
        let r = f.at(u1, 0.0) - a;
        let dep = b - a;
        g.quad(stone, Vec3::new(a.x, 0.005, a.y), Vec3::new(dep.x, 0.0, dep.y), Vec3::new(r.x, 0.0, r.y));
    }
}

/// Members against the inside of the brick, as in the interior photos: rough timber posts with corbels under the
/// girder ends (west and east walls), and the seismic retrofit such warehouses got: cast concrete pilasters on
/// the piers along the north and south walls and a concrete bond beam round the top of the walls.
fn wall_members(g: &mut Geo, faces: &[Face], spec: &loft::Spec, y: f32, deck_b: f32, w: f32) {
    let conc = Key::new(Mat::Surface(m::CONCRETE), [0.98, 0.96, 0.92]).inside().wear(wear::STONE);
    let girder_b = deck_b - loft::GIRDER_D + loft::DECK;
    // timber posts and corbels carrying the girder ends
    for &z in &spec.girders {
        for (x0, x1, into) in [(T, T + 0.28, 1.0f32), (w - T - 0.28, w - T, -1.0)] {
            let edge = if into > 0.0 { x0 } else { x1 };
            if spec.core.r.x0 - 0.3 < edge.max(x0) && edge.min(x1) < spec.core.r.x1 + 0.3 && z > spec.core.r.z0 - 0.4 && z < spec.core.r.z1 + 0.4 {
                continue;
            }
            g.timber(loft::column_timber(), Vec3::new(x0, y, z - 0.18), Vec3::new(x1, girder_b - 0.3, z + 0.18));
            let reach = edge + into * 0.55;
            g.timber(loft::timber(), Vec3::new(edge.min(reach), girder_b - 0.3, z - 0.21), Vec3::new(edge.max(reach), girder_b, z + 0.21));
        }
    }
    // pilasters at the column lines, moved onto the nearest pier so they never cover a window
    for (f, along_x) in [(&faces[1], false), (&faces[3], true)] {
        let piers = f.piers();
        for &x in &spec.cols {
            let mut u = if along_x { w - x } else { x };
            if f.bays.iter().any(|&(b0, b1)| u > b0 - 0.35 && u < b1 + 0.35) {
                u = piers.iter().map(|p| (p.0 + p.1) / 2.0).filter(|c| *c > 0.6 && *c < f.len() - 0.6).min_by(|a, b| (a - u).abs().total_cmp(&(b - u).abs())).unwrap_or(u);
            }
            f.bx(g, conc, u - 0.26, u + 0.26, y, deck_b - 0.6, -T - 0.2, -T);
        }
    }
    // bond beam round the walls just under the deck (kept out of the stair and lift core)
    for f in faces {
        let len = f.len();
        let mut spans = vec![(T, len - T)];
        // the core sits against the east party wall: leave that stretch out
        if f.bays.is_empty() && spec.core.r.x1 > w - T - 0.6 {
            let (c0, c1) = (f.len() - spec.core.r.z1, f.len() - spec.core.r.z0);
            spans = vec![(T, c0.max(T)), (c1.min(len - T), len - T)];
        }
        for (u0, u1) in spans {
            if u1 - u0 > 0.3 {
                f.bx(g, conc, u0, u1, deck_b - 0.6, deck_b, -T - 0.24, -T);
            }
        }
    }
}

fn m_concrete() -> &'static str {
    m::CONCRETE
}

/// Square cast-stone plaque set diagonally on the wall face.
fn diamond(g: &mut Geo, f: &Face, u: f32, y: f32, r: f32, k: Key) {
    let dep = 0.07;
    let pt = |du: f32, dy: f32, o: f32| {
        let p = f.at(u + du, o);
        Vec3::new(p.x, y + dy, p.y)
    };
    let right = {
        let d = f.dir();
        Vec3::new(d.x, 0.0, d.y)
    };
    let corners = [(0.0, -r), (r, 0.0), (0.0, r), (-r, 0.0)];
    let front: Vec<Vec3> = corners.iter().map(|&(a, b)| pt(a, b, dep)).collect();
    g.poly(k, &front, right, Vec3::Y);
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        let (p0, p1) = (pt(a.0, a.1, 0.0), pt(b.0, b.1, 0.0));
        let e = p1 - p0;
        let out = pt(0.0, 0.0, dep) - pt(0.0, 0.0, 0.0);
        g.quad(k, p0, e, out);
    }
}

/// Painted sign lettering from a 5x7 pixel font, centred at `u`, `y` on the face; `px` is the pixel size.
fn sign(g: &mut Geo, f: &Face, text: &str, u: f32, y: f32, px: f32, k: Key) {
    let glyph = |c: char| -> [&'static str; 7] {
        match c {
            'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
            'C' => [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
            'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
            'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
            'N' => ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
            'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
            'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
            'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
            'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
            _ => ["....."; 7],
        }
    };
    let n = text.chars().count() as f32;
    let width = (n * 6.0 - 1.0) * px;
    let u0 = u - width / 2.0;
    let top = y + 3.5 * px;
    for (i, ch) in text.chars().enumerate() {
        let rows = glyph(ch);
        for (r, row) in rows.iter().enumerate() {
            // merge horizontal runs of pixels into one quad
            let b = row.as_bytes();
            let mut c = 0;
            while c < 5 {
                if b[c] != b'#' {
                    c += 1;
                    continue;
                }
                let start = c;
                while c < 5 && b[c] == b'#' {
                    c += 1;
                }
                let ua = u0 + (i as f32 * 6.0 + start as f32) * px;
                let ub = u0 + (i as f32 * 6.0 + c as f32) * px;
                let yb = top - r as f32 * px;
                let p = f.at(ua, 0.056);
                let q = f.at(ub, 0.056);
                g.quad(k, Vec3::new(p.x, yb - px, p.y), Vec3::new(q.x - p.x, 0.0, q.y - p.y), Vec3::Y * px);
            }
        }
    }
}
