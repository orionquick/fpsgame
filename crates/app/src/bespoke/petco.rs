//! The ballpark (Petco Park, OSM relation/1779874), built from measurement rather than guesswork:
//! - home plate and the field axis from the aerial (the 13 ft dirt circle and the mound, confirmed by a 0.28 m lidar
//!   bump 18.1 m north of it): the axis points due north (0.6 degrees east);
//! - the outfield wall traced on the aerial (track edge) and in the lidar (the wall step), checked against the published
//!   distances: 336 ft at the left-field pole, 396 ft to centre, 322 ft at the right-field pole; wall 4-7 ft;
//! - the front of the stands (foul territory) from the OSM pitch outline, the backstop moved out to the aerial's edge;
//! - every tier from lidar cross-sections taken all round the bowl: the lower bowl (front ~0.5 m, back ~4.5 m), the
//!   terrace face (~11 m), the upper deck (21 -> 30+ m), the white canopy (40.5 m) and the light towers (52-65 m);
//! - materials and details from reference photos: navy seats, pale concrete risers with galvanized aisle rails, blue
//!   padded walls with LED ribbons, "Padres gold" sandstone and stucco, white-painted steel, yellow foul poles.
//! Rows of seats run parallel to a smoothed, convex guide line around the field (the true front line has two jogs),
//! so their pitch is constant; each tier's extent comes from the measured lines along the guide's normals.
use super::furnish::Furnisher;
use super::geom::{wear, Geo, Key, Mat};
use super::Building;
use glam::{Vec2, Vec3};

pub const OSM: &str = "relation/1779874";
/// The Western Metal Supply Co. building (its own OSM way), built here as part of the ballpark.
pub const WESTERN_METAL: &str = "way/769800588";
/// Home plate (world x, z): the centre of the dirt circle on the aerial.
const HOME: Vec2 = Vec2::new(-2.1, 8.1);
/// Seat model (folding seat with arms, 0.49 m wide) and the navy it is painted.
const SEAT: &str = "sf_6bbe4c85d2_03";
const NAVY: [f32; 3] = [0.035, 0.055, 0.13];

/// Plan direction of a compass bearing (degrees clockwise from north) in local x-east / z-south.
fn dir(b: f32) -> Vec2 {
    let t = b.to_radians();
    Vec2::new(t.sin(), -t.cos())
}

/// Outfield wall: bearing from home (deg) -> distance (m), traced (aerial track edge + lidar step).
const WALL: [(f32, f32); 37] = [
    (-45.0, 101.4), (-42.5, 102.0), (-40.0, 102.5), (-37.5, 103.25), (-35.0, 103.75), (-32.5, 105.0), (-30.0, 105.75), (-27.5, 107.0),
    (-25.0, 108.75), (-22.5, 111.5), (-20.0, 113.25), (-17.5, 116.0), (-15.0, 119.0), (-12.5, 119.5), (-10.0, 119.75), (-7.5, 119.75),
    (-5.0, 120.0), (-2.5, 120.3), (0.0, 120.7), (2.5, 120.5), (5.0, 120.3), (7.5, 120.0), (10.0, 119.5), (12.5, 118.7), (15.0, 117.8),
    (17.5, 117.0), (20.0, 115.0), (22.5, 112.5), (25.0, 110.75), (27.5, 109.25), (30.0, 108.0), (32.5, 107.0), (35.0, 105.5),
    (37.5, 104.0), (40.0, 102.5), (42.5, 99.5), (45.0, 97.0),
];

/// Lidar-measured tier lines, bearing from home (deg) -> distance (m): the terrace face (first > 9 m), the upper
/// deck's front (first > 19 m), the canopy's front (first > 38 m). Outliers (light towers, people) smoothed out.
const TERRACE: [(f32, f32); 53] = [
    (50.0, 98.5), (55.0, 92.5), (60.0, 87.5), (65.0, 80.5), (70.0, 71.5), (75.0, 65.5), (80.0, 60.5), (85.0, 56.0), (90.0, 51.8),
    (95.0, 48.0), (100.0, 44.5), (105.0, 42.5), (110.0, 40.5), (115.0, 39.0), (120.0, 37.5), (125.0, 37.0), (130.0, 36.5), (135.0, 36.5),
    (140.0, 36.5), (145.0, 37.0), (150.0, 37.0), (155.0, 38.0), (160.0, 39.0), (165.0, 39.0), (170.0, 39.0), (175.0, 39.0), (180.0, 39.0),
    (185.0, 39.0), (190.0, 38.8), (195.0, 38.6), (200.0, 38.0), (205.0, 36.0), (210.0, 35.5), (215.0, 35.0), (220.0, 35.0), (225.0, 35.0),
    (230.0, 35.0), (235.0, 35.2), (240.0, 36.0), (245.0, 37.0), (250.0, 38.5), (255.0, 40.0), (260.0, 43.0), (265.0, 46.0), (270.0, 51.0),
    (275.0, 54.5), (280.0, 58.5), (285.0, 64.5), (290.0, 71.0), (295.0, 78.0), (300.0, 84.0), (305.0, 88.0), (310.0, 94.0),
];
const UPPER: [(f32, f32); 53] = [
    (50.0, 104.0), (55.0, 97.5), (60.0, 92.0), (65.0, 87.0), (70.0, 78.5), (75.0, 71.5), (80.0, 66.5), (85.0, 61.5), (90.0, 57.8),
    (95.0, 55.0), (100.0, 51.5), (105.0, 49.0), (110.0, 46.5), (115.0, 45.0), (120.0, 43.5), (125.0, 43.0), (130.0, 43.0), (135.0, 42.5),
    (140.0, 42.5), (145.0, 43.5), (150.0, 43.8), (155.0, 43.8), (160.0, 44.0), (165.0, 43.8), (170.0, 44.0), (175.0, 44.0), (180.0, 43.8),
    (185.0, 44.0), (190.0, 43.6), (195.0, 43.5), (200.0, 43.2), (205.0, 43.0), (210.0, 43.0), (215.0, 42.5), (220.0, 41.0), (225.0, 40.7),
    (230.0, 41.0), (235.0, 41.0), (240.0, 42.0), (245.0, 42.5), (250.0, 45.5), (255.0, 47.5), (260.0, 49.5), (265.0, 53.0), (270.0, 56.5),
    (275.0, 60.0), (280.0, 65.0), (285.0, 71.0), (290.0, 78.0), (295.0, 84.0), (300.0, 89.0), (305.0, 93.5), (310.0, 99.5),
];
const CANOPY: [(f32, f32); 51] = [
    (55.0, 114.0), (60.0, 108.0), (65.0, 103.0), (70.0, 97.0), (75.0, 89.5), (80.0, 83.0), (85.0, 77.5), (90.0, 73.5), (95.0, 70.0),
    (100.0, 66.5), (105.0, 63.5), (110.0, 61.5), (115.0, 59.0), (120.0, 57.5), (125.0, 56.5), (130.0, 56.0), (135.0, 55.5), (140.0, 55.5),
    (145.0, 55.5), (150.0, 55.5), (155.0, 55.5), (160.0, 55.0), (165.0, 55.0), (170.0, 55.0), (175.0, 55.5), (180.0, 55.0), (185.0, 55.0),
    (190.0, 54.5), (195.0, 54.5), (200.0, 54.5), (205.0, 55.0), (210.0, 54.0), (215.0, 54.0), (220.0, 54.5), (225.0, 54.0), (230.0, 54.0),
    (235.0, 55.0), (240.0, 56.0), (245.0, 57.5), (250.0, 59.5), (255.0, 61.5), (260.0, 64.0), (265.0, 68.0), (270.0, 71.5), (275.0, 76.5),
    (280.0, 82.5), (285.0, 90.0), (290.0, 95.5), (295.0, 99.0), (300.0, 103.5), (305.0, 110.0),
];

/// Front of the stands in foul territory (world x, z), right-field pole round behind home to the left-field pole:
/// the OSM pitch outline with the backstop moved 1.5 m out (the aerial's edge) and the poles at the traced wall.
const FRONT: [(f32, f32); 21] = [
    (66.5, -60.5), (59.3, -53.0), (43.2, -37.0), (41.9, -35.0), (26.2, -6.7), (23.4, -4.2), (2.6, 17.2), (0.7, 18.3), (-1.2, 18.6),
    (-3.4, 18.6), (-5.3, 18.1), (-7.2, 16.5), (-23.9, -1.3), (-28.6, -6.2), (-46.9, -33.6), (-46.7, -39.1), (-60.4, -53.5), (-69.4, -62.3),
    (-74.2, -63.0), (-74.2, -63.0), (-74.2, -63.0),
];

fn interp(t: &[(f32, f32)], x: f32) -> f32 {
    if x <= t[0].0 {
        return t[0].1;
    }
    for w in t.windows(2) {
        if x <= w[1].0 {
            let f = (x - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * f;
        }
    }
    t[t.len() - 1].1
}

/// A measured polar line as a local plan polyline (0.5 degree steps).
fn polar_line(t: &[(f32, f32)]) -> Vec<Vec2> {
    let (a, b) = (t[0].0, t[t.len() - 1].0);
    let n = ((b - a) / 0.5) as usize;
    (0..=n).map(|i| {
        let th = a + (b - a) * i as f32 / n as f32;
        dir(th) * interp(t, th)
    })
    .collect()
}

/// Distance along the ray o + d*t to the polyline, if it is crossed.
fn ray_hit(o: Vec2, d: Vec2, line: &[Vec2]) -> Option<f32> {
    let mut best: Option<f32> = None;
    for w in line.windows(2) {
        let (a, e) = (w[0], w[1] - w[0]);
        let den = d.x * e.y - d.y * e.x;
        if den.abs() < 1e-9 {
            continue;
        }
        let ao = a - o;
        let t = (ao.x * e.y - ao.y * e.x) / den;
        let u = (ao.x * d.y - ao.y * d.x) / den;
        if t > 0.0 && (0.0..=1.0).contains(&u) {
            best = Some(best.map_or(t, |b: f32| b.min(t)));
        }
    }
    best
}

fn point_in(r: &[Vec2], p: Vec2) -> bool {
    let mut c = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

/// Distance from p to the polyline.
fn dist_to(line: &[Vec2], p: Vec2) -> f32 {
    line.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let e = b - a;
            let t = ((p - a).dot(e) / e.length_squared().max(1e-9)).clamp(0.0, 1.0);
            (a + e * t - p).length()
        })
        .fold(f32::MAX, f32::min)
}

/// Resample a polyline every `step` metres.
fn resample(pts: &[Vec2], step: f32) -> Vec<Vec2> {
    let mut out = vec![pts[0]];
    let mut carry = 0.0;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let l = (b - a).length();
        let mut t = step - carry;
        while t <= l {
            out.push(a + (b - a) * (t / l));
            t += step;
        }
        carry = l - (t - step);
    }
    if (*out.last().unwrap() - pts[pts.len() - 1]).length() > step * 0.3 {
        out.push(pts[pts.len() - 1]);
    }
    out
}

fn chaikin(pts: &[Vec2], iters: usize) -> Vec<Vec2> {
    let mut p = pts.to_vec();
    for _ in 0..iters {
        let mut q = vec![p[0]];
        for w in p.windows(2) {
            q.push(w[0] * 0.75 + w[1] * 0.25);
            q.push(w[0] * 0.25 + w[1] * 0.75);
        }
        q.push(*p.last().unwrap());
        p = q;
    }
    p
}

/// Convex chain (seen from the field) through an open polyline that wraps the field: drops the dents.
fn convex_chain(pts: &[Vec2], inside: Vec2) -> Vec<Vec2> {
    let mut out: Vec<Vec2> = vec![];
    for &p in pts {
        out.push(p);
        while out.len() >= 3 {
            let n = out.len();
            let (a, b, c) = (out[n - 3], out[n - 2], out[n - 1]);
            // b is a dent if it lies on the field's side of the chord a-c
            let side = |q: Vec2| (c - a).perp_dot(q - a);
            if side(b) * side(inside) > 0.0 {
                out.remove(n - 2);
            } else {
                break;
            }
        }
    }
    out
}

/// Materials.
struct Mats {
    grass: Key,
    grass_dark: Key,
    dirt: Key,
    track: Key,
    chalk: Key,
    rubber: Key,
    concrete: Key,
    riser: Key,
    stucco: Key,
    sandstone: Key,
    steel: Key,
    navy: Key,
    pad: Key,
    led: Key,
    glass: Key,
    yellow: Key,
    rail: Key,
    roof: Key,
    /// Painted white structural steel (the exposed frame), and the pale underside of the upper deck.
    white: Key,
    soffit: Key,
    letters: Key,
    interior: Key,
    /// Warm light strips, and the decks' glass guards.
    led_warm: Key,
    guard: Key,
}

fn mats() -> Mats {
    let s = |id: &'static str, t: [f32; 3]| Key::new(Mat::Surface(id), t);
    Mats {
        grass: s("sfklehwa", [0.62, 0.92, 0.5]).tile(2.0).rough(0.85),
        grass_dark: s("sfklehwa", [0.5, 0.78, 0.42]).tile(2.0).rough(0.85),
        dirt: s("pht_baseball_playground", [0.92, 0.58, 0.4]).tile(3.0),
        track: s("pht_red_laterite_soil_stones", [0.95, 0.78, 0.68]).tile(2.0),
        chalk: Key::new(Mat::Plain(200), [0.92, 0.92, 0.9]),
        rubber: Key::new(Mat::Plain(160), [0.95, 0.95, 0.94]),
        concrete: s("pht_smooth_concrete_floor", [1.25, 1.22, 1.15]).tile(2.0).wear(wear::STONE),
        riser: s("pht_smooth_concrete_floor", [1.05, 1.02, 0.97]).tile(2.0),
        stucco: s("tjeneevfw", [1.1, 0.92, 0.7]).tile(2.0),
        sandstone: s("pt_ashlar_sandstone", [1.0, 1.0, 1.0]).tile(3.0),
        steel: Key::new(Mat::Plain(110), [0.86, 0.87, 0.87]).wear(wear::METAL),
        navy: Key::new(Mat::Plain(150), [0.035, 0.055, 0.12]),
        pad: Key::new(Mat::Plain(190), [0.03, 0.06, 0.15]),
        led: Key::new(Mat::Light, [0.25, 0.45, 1.1]),
        glass: Key::new(Mat::Glass, [0.55, 0.62, 0.66]),
        yellow: Key::new(Mat::Plain(120), [0.95, 0.72, 0.05]),
        rail: Key::new(Mat::Plain(70), [0.58, 0.6, 0.6]).wear(wear::METAL),
        roof: Key::new(Mat::Plain(140), [0.8, 0.81, 0.8]),
        white: Key::new(Mat::Plain(90), [0.93, 0.93, 0.91]).wear(wear::METAL),
        soffit: Key::new(Mat::Plain(170), [0.26, 0.26, 0.26]),
        letters: Key::new(Mat::Plain(80), [0.3, 0.02, 0.03]),
        interior: s("tjeneevfw", [0.8, 0.64, 0.48]).tile(2.0),
        led_warm: Key::new(Mat::Light, [1.2, 1.05, 0.8]),
        guard: Key::new(Mat::Glass, [0.18, 0.22, 0.24]),
    }
}

/// One station on the guide line: position, outward normal, and the measured tier offsets along that normal.
#[derive(Clone, Copy)]
struct Station {
    p: Vec2,
    n: Vec2,
    /// Offsets along the normal: true front line (wall), lower bowl back, terrace face, upper deck front, canopy front.
    front: f32,
    lb_end: f32,
    terrace: f32,
    upper: f32,
    canopy: f32,
    /// Bearing from home of the station.
    bearing: f32,
}

const LB_PITCH: f32 = 0.85;
const TERR_PITCH: f32 = 0.85;
const UP_PITCH: f32 = 0.8;
const UP_RISE: f32 = 0.56;
const TERR_RISE: f32 = 0.5;
const CONCOURSE_Y: f32 = 4.6;
const TERR_Y: f32 = 11.6;
const UP_Y: f32 = 21.8;
const CANOPY_DEPTH: f32 = 12.0;
/// Spacing of the white steel rakers round the back of the bowl.
const BAY: f32 = 9.5;
/// The home plate gate's exposed frame (from the photos, scaled by people on its decks and the canopy height): the
/// frame's plane out from the upper deck's back, its bay, the upper concourse floor, the lettering's top, and the
/// truss chords (letter truss, roof truss top under the canopy).
const FRAME_OFF: f32 = 0.85;
const GZ_BAY: f32 = 13.4;
const GZ_UPPER_CONCOURSE: f32 = 22.5;
const GZ_LETTER_TOP: f32 = 36.4;
/// The underside of the top rows' box edge, over the posts above the letter truss.
const GZ_BOX_BOTTOM: f32 = 35.0;
/// How far the top rows overhang the frame on their angled struts, and the upper deck's rake length (front row to
/// back) behind the home gate, for the raker trusses under it.
const GZ_OVERHANG: f32 = 0.9;
const GZ_RAKE: f32 = 21.9;
const UP_ROWS: f32 = 27.0;

/// The field's boundary (world plan), 2 m out under the stands' front and the wall: the city's ground stops here.
pub fn field_ring() -> Vec<[f64; 2]> {
    let mut ring: Vec<Vec2> = FRONT.iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect();
    ring.dedup_by(|a, b| (*a - *b).length() < 0.01);
    ring.extend(WALL.iter().map(|&(b, d)| dir(b) * d));
    let c = Vec2::new(0.0, -50.0);
    ring.iter().map(|p| {
        let q = *p + (*p - c).normalize() * 2.0 + HOME;
        [q.x as f64, q.y as f64]
    })
    .collect()
}

pub fn build(c: &city::City, cat: &assets::Catalog) -> Option<Building> {
    c.buildings.iter().find(|b| b.osm == OSM)?;
    let mut g = Geo::new(Vec3::new(HOME.x, 0.0, HOME.y), Vec3::X);
    let mut f = Furnisher::new(cat, 0x9e7c0);
    f.indoor = false;
    let m = mats();
    let front: Vec<Vec2> = FRONT.iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect();
    let wall: Vec<Vec2> = WALL.iter().map(|&(b, d)| dir(b) * d).collect();
    field(&mut g, &m, &front, &wall);
    outfield_wall(&mut g, &m);
    foul_poles(&mut g, &m);
    let st = stations(&front);
    if std::env::var("FPS_PETCO_DEBUG").is_ok() {
        let b: Vec<String> = st.iter().step_by(10).map(|s| format!("{:.0}@({:.0},{:.0}) f{:.1}", s.bearing, s.p.x, s.p.y, s.front)).collect();
        eprintln!("stations {}: {}", st.len(), b.join(" "));
    }
    let cores = cores(c, &st);
    bowl(&mut g, &mut f, &m, &st, &front, &cores);
    for c in &cores {
        if std::env::var("FPS_PETCO_DEBUG").is_ok() {
            let w = |q: Vec2| ((q.x + HOME.x) as i32, (q.y + HOME.y) as i32);
            eprintln!("core street {:?} bridge {:?} y{:.1} top {:?} y{:.1} sides {} {}", w(c.at(-CORE_HALF + LAND / 2.0, c.r0 + CORE_DEPTH + 1.5)), w(c.at(c.terrace_side * (CORE_HALF - LAND / 2.0), c.t_back)), c.t_tread, w(c.at(c.top_side * (CORE_HALF - LAND / 2.0), c.up_back)), c.up_tread, c.terrace_side, c.top_side);
        }
        stair_core(&mut g, &m, c);
    }
    home_plaza(&mut g, &mut f, &m, c, &st);
    outfield(&mut g, &mut f, &m);
    backstop_net(&mut g, &m, &st);
    for b in [95.0f32, 265.0] {
        disc(&mut g, m.dirt, dir(b) * 9.5, 1.52, 0.02);
    }
    western_metal(&mut g, &m, c);
    for t in TOWERS {
        tower(&mut g, &m, t);
    }
    board(&mut g, &m, Vec2::new(-99.7, -42.8) - HOME, Vec2::X, 27.0, 33.0, 49.5, true);
    board(&mut g, &m, Vec2::new(99.1, -43.7) - HOME, Vec2::NEG_X, 27.0, 38.0, 47.5, false);
    Some(Building { osm: OSM, geo: g, furniture: f.out })
}

// ---------------------------------------------------------------- field

/// The playing surface: grass mowed in diagonal checks, the warning track round the whole boundary, the skinned
/// infield (MLB geometry: 90 ft square, 95 ft arc about the rubber, 13 ft plate circle, 18 ft mound), base paths,
/// cutouts, chalk lines, batter's and catcher's boxes, bases, plate and rubber.
fn field(g: &mut Geo, m: &Mats, front: &[Vec2], wall: &[Vec2]) {
    // the boundary: the stands' front from the right-field pole round to the left, then the wall back across
    let mut bound: Vec<Vec2> = front.to_vec();
    bound.dedup_by(|a, b| (*a - *b).length() < 0.01);
    bound.extend(wall.iter().copied());
    let fw = |k: Key| k;
    // grass checks: 15 ft squares turned 45 degrees to the field axis
    let cell = 4.572;
    let (u, v) = (dir(45.0), dir(-45.0));
    for i in -40i32..40 {
        for j in -40i32..40 {
            let o = u * (i as f32 * cell) + v * (j as f32 * cell);
            let ctr = o + (u + v) * (cell / 2.0);
            if !point_in(&bound, ctr) && dist_to(&bound, ctr) > cell {
                continue;
            }
            // 2024 pattern: straight diagonal stripes
            let k = if i.rem_euclid(2) == 0 { m.grass } else { m.grass_dark };
            g.quad(fw(k), Vec3::new(o.x, 0.0, o.y) + Vec3::new(v.x, 0.0, v.y) * cell, Vec3::new(u.x, 0.0, u.y) * cell, Vec3::new(-v.x, 0.0, -v.y) * cell);
        }
    }
    // warning track: 15 ft inside the boundary all round
    let b2 = resample(&bound, 1.0);
    let ctr = Vec2::new(0.0, -50.0);
    for w in b2.windows(2) {
        let (a, b) = (w[0], w[1]);
        let e = (b - a).normalize_or_zero();
        let mut nin = Vec2::new(-e.y, e.x);
        if nin.dot(ctr - (a + b) / 2.0) < 0.0 {
            nin = -nin;
        }
        let d = 4.6;
        let pts = [a, b, b + nin * d, a + nin * d].map(|p| Vec3::new(p.x, 0.006, p.y));
        poly_up(g, m.track, &pts);
    }
    // the infield skin: home, out the lines to the 95 ft arc about the rubber, round it
    let rubber = Vec2::new(0.0, -18.44);
    let arc_r = 28.96;
    let mut skin = vec![Vec2::ZERO];
    for k in 0..=60 {
        let b = -45.0 + 90.0 * k as f32 / 60.0;
        // the arc point along bearing b from home, at radius arc_r about the rubber
        let d = dir(b);
        let q = d.dot(rubber);
        let t = q + (q * q - rubber.length_squared() + arc_r * arc_r).sqrt();
        skin.push(d * t);
    }
    fan(g, m.dirt, &skin, 0.012);
    // infield grass: the square inset 3 ft from the base lines
    let s = 27.432;
    let (b1, b2s, b3) = (dir(45.0) * s, Vec2::new(0.0, -38.795), dir(-45.0) * s);
    let inset = |p: Vec2, k: f32| p + (Vec2::new(0.0, -19.4) - p).normalize() * k;
    let sq = [inset(Vec2::ZERO, 1.3), inset(b1, 1.3), inset(b2s, 1.3), inset(b3, 1.3)];
    poly_up(g, m.grass, &sq.map(|p| Vec3::new(p.x, 0.018, p.y)));
    // base cutouts (13 ft), the plate circle (13 ft), the mound (18 ft) on top
    for (p, r) in [(b1, 3.96), (b2s, 3.96), (b3, 3.96), (Vec2::ZERO, 3.96)] {
        disc(g, m.dirt, p, r, 0.024);
    }
    // base paths: 3 ft either side of the lines from the plate circle to the arc
    for b in [45.0f32, -45.0] {
        let d = dir(b);
        let side = Vec2::new(-d.y, d.x);
        let (a, e) = (d * 3.9, d * 28.0);
        let pts = [a - side * 0.91, e - side * 0.91, e + side * 0.91, a + side * 0.91].map(|p| Vec3::new(p.x, 0.022, p.y));
        poly_up(g, m.dirt, &pts);
    }
    mound(g, m, Vec2::new(0.0, -17.98));
    // chalk: foul lines out to the wall, batter's boxes, catcher's box
    for b in [45.0f32, -45.0] {
        let d = dir(b);
        let side = Vec2::new(-d.y, d.x);
        let end = interp(&WALL, b) - 0.3;
        let pts = [d * 0.3 - side * 0.05, d * end - side * 0.05, d * end + side * 0.05, d * 0.3 + side * 0.05].map(|p| Vec3::new(p.x, 0.03, p.y));
        poly_up(g, m.chalk, &pts);
    }
    for sx in [-1.0f32, 1.0] {
        // batter's box 4 x 6 ft, 6 in from the plate, centred on the plate's middle
        let (x0, x1) = (sx * 0.368, sx * (0.368 + 1.219));
        rect_line(g, m.chalk, Vec2::new(x0.min(x1), -0.93), Vec2::new(x0.max(x1), 0.9), 0.03);
    }
    rect_line(g, m.chalk, Vec2::new(-0.54, 0.5), Vec2::new(0.54, 2.9), 0.03);
    // coaches' boxes: 20 x 10 ft, 15 ft beyond the base lines
    for (b, base) in [(45.0f32, b1), (-45.0, b3)] {
        let d = dir(b);
        let out = Vec2::new(d.x.signum(), 0.0);
        let c0 = base + Vec2::new(0.0, 4.5) + out * 4.6;
        rect_line(g, m.chalk, c0 - Vec2::new(1.5, 3.0), c0 + Vec2::new(1.5, 3.0), 0.028);
    }
    // bases (18 in) and home plate (17 in pentagon)
    for (p, b) in [(b1, 45.0f32), (b2s, 0.0), (b3, -45.0)] {
        let d = dir(b) * 0.229;
        let r = Vec2::new(-d.y, d.x);
        let q = [p - d - r, p - d + r, p + d + r, p + d - r];
        let _ = b;
        let top = 0.09;
        for k in 0..4 {
            let (a, c) = (q[k], q[(k + 1) % 4]);
            g.quad(m.rubber, Vec3::new(a.x, 0.024, a.y), Vec3::new(c.x - a.x, 0.0, c.y - a.y), Vec3::Y * (top - 0.024));
        }
        poly_up(g, m.rubber, &q.map(|v| Vec3::new(v.x, top, v.y)));
    }
    let plate = [Vec2::new(-0.216, -0.216), Vec2::new(0.216, -0.216), Vec2::new(0.216, 0.0), Vec2::new(0.0, 0.216), Vec2::new(-0.216, 0.0)];
    fan(g, m.rubber, &plate, 0.032);
}

/// The pitcher's mound: 18 ft across, 10 in high, a flat top round the 24 x 6 in rubber, sloping to the skin.
fn mound(g: &mut Geo, m: &Mats, c: Vec2) {
    let (r0, r1, h) = (0.9, 2.74, 0.254);
    let n = 40;
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
        let (d0, d1) = (Vec2::new(a0.cos(), a0.sin()), Vec2::new(a1.cos(), a1.sin()));
        let p = |d: Vec2, r: f32, y: f32| Vec3::new(c.x + d.x * r, y, c.y + d.y * r);
        g.poly(m.dirt, &[p(d0, r1, 0.024), p(d0, r0, h), p(d1, r0, h), p(d1, r1, 0.024)], Vec3::X, Vec3::Z);
        g.poly(m.dirt, &[p(d0, r0, h), Vec3::new(c.x, h, c.y), p(d1, r0, h)], Vec3::X, Vec3::Z);
    }
    let rb = c + Vec2::new(0.0, -0.46);
    g.cuboid(m.rubber, Vec3::new(rb.x - 0.305, h, rb.y - 0.076), Vec3::new(rb.x + 0.305, h + 0.02, rb.y + 0.076));
}

/// Upward-facing flat polygon (any winding).
fn poly_up(g: &mut Geo, k: Key, pts: &[Vec3]) {
    let a = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
    let mut v = pts.to_vec();
    if a.y < 0.0 {
        v.reverse();
    }
    // right = x, up = -z (so the normal right x up points +y)
    g.poly(k, &v, Vec3::X, Vec3::NEG_Z);
}

/// Downward-facing flat polygon (any winding).
fn poly_down(g: &mut Geo, k: Key, pts: &[Vec3]) {
    let a = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
    let mut v = pts.to_vec();
    if a.y > 0.0 {
        v.reverse();
    }
    g.poly(k, &v, Vec3::X, Vec3::Z);
}

/// Flat fan from the first point (a star-shaped polygon).
fn fan(g: &mut Geo, k: Key, pts: &[Vec2], y: f32) {
    let c = pts[0];
    for w in pts[1..].windows(2) {
        poly_up(g, k, &[Vec3::new(c.x, y, c.y), Vec3::new(w[0].x, y, w[0].y), Vec3::new(w[1].x, y, w[1].y)]);
    }
}

fn disc(g: &mut Geo, k: Key, c: Vec2, r: f32, y: f32) {
    let n = 40;
    let mut pts = vec![c];
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        pts.push(c + Vec2::new(a.cos(), a.sin()) * r);
    }
    fan(g, k, &pts, y);
}

fn rect_line(g: &mut Geo, k: Key, a: Vec2, b: Vec2, y: f32) {
    let w = 0.05;
    for (p, q) in [(Vec2::new(a.x, a.y), Vec2::new(b.x, a.y)), (Vec2::new(b.x, a.y), Vec2::new(b.x, b.y)), (Vec2::new(b.x, b.y), Vec2::new(a.x, b.y)), (Vec2::new(a.x, b.y), Vec2::new(a.x, a.y))] {
        let e = (q - p).normalize();
        let s = Vec2::new(-e.y, e.x) * w;
        poly_up(g, k, &[p - s, q - s, q + s, p + s].map(|v| Vec3::new(v.x, y, v.y)));
    }
}

// ---------------------------------------------------------------- outfield wall, poles

/// Wall height by bearing: ~4 ft in the left-field corner rising to 7 ft by left-centre, ~7.5 ft round right field.
fn wall_h(b: f32) -> f32 {
    if b < -30.0 {
        1.3
    } else if b < -22.0 {
        1.3 + (b + 30.0) / 8.0 * 0.8
    } else if b < 15.0 {
        2.1
    } else {
        2.3
    }
}

/// The outfield wall: blue padding on the field side with a yellow top line, concrete behind; collidable.
fn outfield_wall(g: &mut Geo, m: &Mats) {
    let step = 1.0;
    let n = (90.0 / step) as i32;
    for i in 0..n {
        let (ba, bb) = (-45.0 + i as f32 * step, -45.0 + (i + 1) as f32 * step);
        let (a, b) = (dir(ba) * interp(&WALL, ba), dir(bb) * interp(&WALL, bb));
        let (ha, hb) = (wall_h(ba), wall_h(bb));
        let e = (b - a).normalize();
        let out = Vec2::new(e.y, -e.x) * if Vec2::new(e.y, -e.x).dot(a) > 0.0 { 1.0 } else { -1.0 };
        let t = 0.45;
        let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
        // padded face (towards home)
        g.poly(m.pad, &[v(b, 0.0), v(a, 0.0), v(a, ha), v(b, hb)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        // yellow line along the top
        g.poly(m.yellow, &[v(a, ha), v(a + out * t, ha), v(b + out * t, hb), v(b, hb)], Vec3::new(e.x, 0.0, e.y), Vec3::new(out.x, 0.0, out.y));
        g.poly(m.concrete, &[v(a + out * t, 0.0), v(b + out * t, 0.0), v(b + out * t, hb), v(a + out * t, ha)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        g.block_ring(&[a, b, b + out * t, a + out * t], 0.0, ha.min(hb));
    }
}

/// Yellow foul poles with their fair-side screens: the left one on the Western Metal Supply building's corner.
fn foul_poles(g: &mut Geo, m: &Mats) {
    for b in [45.0f32, -45.0] {
        let p = dir(b) * interp(&WALL, b);
        let h = 28.0;
        g.tube(m.yellow, Vec3::new(p.x, 0.0, p.y), Vec3::new(p.x, h, p.y), 0.16, 10);
        // the screen: on the fair side of the pole
        let fair = dir(if b > 0.0 { b - 90.0 } else { b + 90.0 });
        let side = Vec2::new(fair.x, fair.y) * 0.75;
        for (ka, kb) in [(Vec3::Y * 3.0, Vec3::Y * (h - 0.5))] {
            let o = Vec3::new(p.x, 0.0, p.y);
            g.quad(m.yellow, o + ka, Vec3::new(side.x, 0.0, side.y), kb - ka);
            g.quad(m.yellow, o + ka + Vec3::new(side.x, 0.0, side.y), Vec3::new(-side.x, 0.0, -side.y), kb - ka);
        }
        g.block_aabb(Vec3::new(p.x - 0.2, 0.0, p.y - 0.2), Vec3::new(p.x + 0.2, h, p.y + 0.2));
    }
}

// ---------------------------------------------------------------- the bowl

/// Stations every ~1.2 m along the convex guide line, with the tier offsets measured along their normals.
fn stations(front: &[Vec2]) -> Vec<Station> {
    let inside = Vec2::new(0.0, -50.0);
    let mut pts = front.to_vec();
    pts.dedup_by(|a, b| (*a - *b).length() < 0.01);
    let hull = convex_chain(&pts, inside);
    let guide = resample(&chaikin(&resample(&hull, 2.0), 3), 1.2);
    let lines = [polar_line(&TERRACE), polar_line(&UPPER), polar_line(&CANOPY)];
    let mut out = vec![];
    for k in 0..guide.len() {
        let p = guide[k];
        let (a, b) = (guide[k.saturating_sub(1)], guide[(k + 1).min(guide.len() - 1)]);
        let e = (b - a).normalize_or_zero();
        let mut n = Vec2::new(-e.y, e.x);
        if n.dot(p - inside) < 0.0 {
            n = -n;
        }
        let fr = ray_hit(p - n * 30.0, n, &pts).map_or(0.0, |t| (t - 30.0).max(0.0));
        let hit = |l: &Vec<Vec2>| ray_hit(p, n, l);
        let (Some(t), Some(u)) = (hit(&lines[0]), hit(&lines[1])) else { continue };
        let cpy = hit(&lines[2]).unwrap_or(u + 11.0);
        let bearing = p.x.atan2(-p.y).to_degrees().rem_euclid(360.0);
        out.push(Station { p, n, front: fr, lb_end: t - 0.8, terrace: t, upper: u.max(t + 3.0), canopy: cpy.max(u + 6.0), bearing });
    }
    // light smoothing of the measured offsets along the bowl
    let sm: Vec<Station> = (0..out.len())
        .map(|i| {
            let mut s = out[i];
            let win: Vec<&Station> = out[i.saturating_sub(3)..(i + 4).min(out.len())].iter().collect();
            let avg = |f: fn(&Station) -> f32| win.iter().map(|w| f(w)).sum::<f32>() / win.len() as f32;
            s.terrace = avg(|w| w.terrace);
            s.lb_end = avg(|w| w.lb_end);
            s.upper = avg(|w| w.upper);
            s.canopy = avg(|w| w.canopy);
            s
        })
        .collect();
    sm
}

/// Tread height of the lower bowl at offset `n` (rows rise slowly at the front, faster near the top).
fn lb_y(n: f32, end: f32) -> f32 {
    let t = ((n - 0.8) / (end - 0.8)).clamp(0.0, 1.0);
    0.45 + (CONCOURSE_Y - 0.45) * t.powf(1.35)
}

/// The seating bowl from foul pole to foul pole: the lower bowl's concrete steps and seats behind a padded front
/// wall, the concourse under the terrace with suite glazing, the terrace, the upper deck with stepped aisles and
/// rails, its soffit, the white steel canopy with lights, and the sandstone back wall.
fn bowl(g: &mut Geo, f: &mut Furnisher, m: &Mats, st: &[Station], front: &[Vec2], cores: &[Core]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    // aisles: every 11.5 m along the guide, 1.3 m wide
    let mut arc = vec![0.0f32];
    for w in st.windows(2) {
        arc.push(arc.last().unwrap() + (w[1].p - w[0].p).length());
    }
    let aisle = |s: f32| {
        let k = (s / 11.5).round();
        (s - k * 11.5).abs() < 0.65
    };
    let mut back_arc = 0.0f32;
    let mut gate_line: Vec<(Vec2, Vec2)> = vec![];
    let mut gz_line: Vec<(Vec2, Vec2)> = vec![];
    let mut prev_col: Option<Vec2> = None;
    for i in 0..st.len() - 1 {
        let (a, b) = (st[i], st[i + 1]);
        let mid_s = (arc[i] + arc[i + 1]) / 2.0;
        let in_aisle = aisle(mid_s);
        // --- front wall (true front line): padded, 1.1 m; in the dugout spans the dugout's roof edge instead
        let (fa, fb) = (at(&a, a.front), at(&b, b.front));
        let e = (fb - fa).normalize_or_zero();
        let dug = dugout_at(a.bearing) && dugout_at(b.bearing);
        if dug {
            dugout(g, m, &a, &b);
        } else {
            g.poly(m.pad, &[v(fb, 0.0), v(fa, 0.0), v(fa, 1.1), v(fb, 1.1)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        }
        g.poly(m.concrete, &[v(fa, 1.1), v(fb, 1.1), v(fb + b.n * 0.3, 1.1), v(fa + a.n * 0.3, 1.1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y.cross(Vec3::new(e.x, 0.0, e.y)));
        g.block_ring(&[fa, fb, fb + b.n * 0.3, fa + a.n * 0.3], 0.0, 1.1);
        // --- lower bowl: treads and risers from the front line to the back
        let rows_a = ((a.lb_end - 0.8) / LB_PITCH).floor() as i32;
        let rows = rows_a.max(1);
        for j in 0..rows {
            let n0 = 0.8 + j as f32 * LB_PITCH;
            let n1 = n0 + LB_PITCH;
            if n1 < a.front.min(b.front) || (dug && n0 < a.front + DUG_DEPTH) {
                continue;
            }
            let (n0a, n0b) = (n0.max(a.front + 0.3), n0.max(b.front + 0.3));
            let (ya, yb) = (lb_y(n0, a.lb_end), lb_y(n0, b.lb_end));
            let y_next_a = lb_y(n1, a.lb_end);
            let y_next_b = lb_y(n1, b.lb_end);
            // tread
            g.poly(m.concrete, &[v(at(&a, n0a), ya), v(at(&a, n1), ya), v(at(&b, n1), yb), v(at(&b, n0b), yb)], Vec3::new(a.n.x, 0.0, a.n.y), Vec3::new(e.x, 0.0, e.y));
            // riser up to the next row
            g.poly(m.riser, &[v(at(&a, n1), ya), v(at(&b, n1), yb), v(at(&b, n1), y_next_b), v(at(&a, n1), y_next_a)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            g.block_ring(&[at(&a, n0a), at(&a, n1), at(&b, n1), at(&b, n0b)], 0.0, ya.min(yb));
            // seats, facing the field
            if !in_aisle && n0 > a.front + 0.9 {
                seat_row(g, f, at(&a, n0 + 0.42), at(&b, n0 + 0.42), (ya + yb) / 2.0, -(a.n + b.n).normalize());
            }
        }
        let (lba, lbb) = (0.8 + rows as f32 * LB_PITCH, 0.8 + rows as f32 * LB_PITCH);
        // --- concourse floor under the terrace (lower bowl back to the suites)
        let (ca, cb) = (lba, lbb);
        let (sa, sb) = (a.terrace + 3.0, b.terrace + 3.0);
        g.poly(m.concrete, &[v(at(&a, ca), CONCOURSE_Y), v(at(&a, sa + 6.0), CONCOURSE_Y), v(at(&b, sb + 6.0), CONCOURSE_Y), v(at(&b, cb), CONCOURSE_Y)], Vec3::new(a.n.x, 0.0, a.n.y), Vec3::new(e.x, 0.0, e.y));
        g.block_ring(&[at(&a, ca), at(&a, sa + 6.0), at(&b, sb + 6.0), at(&b, cb)], 0.0, CONCOURSE_Y);
        // rail along the top of the lower bowl
        g.tube(m.rail, v(at(&a, ca), CONCOURSE_Y + 1.0), v(at(&b, cb), CONCOURSE_Y + 1.0), 0.03, 6);
        // suite glazing under the terrace
        g.poly(m.glass, &[v(at(&a, sa), CONCOURSE_Y), v(at(&b, sb), CONCOURSE_Y), v(at(&b, sb), 8.6), v(at(&a, sa), 8.6)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        g.block_ring(&[at(&a, sa), at(&b, sb), at(&b, sb + 0.2), at(&a, sa + 0.2)], CONCOURSE_Y, 8.6);
        // terrace soffit over the concourse
        g.poly(m.stucco, &[v(at(&a, a.terrace), 8.6), v(at(&b, b.terrace), 8.6), v(at(&b, sb + 6.0), 8.6), v(at(&a, sa + 6.0), 8.6)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, 0.0, a.n.y));
        // --- terrace facia with an LED ribbon
        let (ta, tb) = (at(&a, a.terrace), at(&b, b.terrace));
        g.poly(m.navy, &[v(tb, 8.6), v(ta, 8.6), v(ta, TERR_Y + 0.2), v(tb, TERR_Y + 0.2)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.poly(m.led, &[v(tb - b.n * 0.02, 10.4), v(ta - a.n * 0.02, 10.4), v(ta - a.n * 0.02, 11.2), v(tb - b.n * 0.02, 11.2)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        // --- terrace rows: 10, under the upper deck's overhang
        let t_rows = (((a.upper + 3.5) - (a.terrace + 0.3)) / TERR_PITCH).floor().clamp(4.0, 12.0) as i32;
        for j in 0..t_rows {
            let n0 = a.terrace + 0.3 + j as f32 * TERR_PITCH;
            let (n0a, n0b) = (n0, b.terrace + 0.3 + j as f32 * TERR_PITCH);
            let y = TERR_Y + j as f32 * TERR_RISE;
            step_row(g, m, &a, &b, n0a, n0b, TERR_PITCH, y, TERR_RISE, in_aisle);
            if !in_aisle {
                seat_row(g, f, at(&a, n0a + 0.42), at(&b, n0b + 0.42), y, -(a.n + b.n).normalize());
            }
        }
        let t_back_a = a.terrace + 0.3 + t_rows as f32 * TERR_PITCH;
        let t_back_b = b.terrace + 0.3 + t_rows as f32 * TERR_PITCH;
        let t_top = TERR_Y + t_rows as f32 * TERR_RISE;
        // club-level back wall behind the terrace, up to the upper deck's soffit (open where a stair core's bridge
        // comes through)
        let (ca, cb) = (at(&a, t_back_a), at(&b, t_back_b));
        let (ca1, cb1) = (at(&a, t_back_a + 0.3), at(&b, t_back_b + 0.3));
        for (f0, f1, hole) in runs(|f| cores.iter().any(|c| c.door(ca.lerp(cb, f), c.terrace_side))) {
            if hole {
                continue;
            }
            let (p0, p1) = (ca.lerp(cb, f0), ca.lerp(cb, f1));
            g.poly(m.stucco, &[v(p1, t_top), v(p0, t_top), v(p0, UP_Y - 1.0), v(p1, UP_Y - 1.0)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
            g.block_ring(&[p0, p1, ca1.lerp(cb1, f1), ca1.lerp(cb1, f0)], t_top - 0.1, UP_Y);
        }
        // --- upper deck: facia, rows with stepped aisles, rails
        let (ua, ub) = (at(&a, a.upper), at(&b, b.upper));
        g.poly(m.navy, &[v(ub, UP_Y - 2.3), v(ua, UP_Y - 2.3), v(ua, UP_Y + 0.25), v(ub, UP_Y + 0.25)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.poly(m.led, &[v(ub - b.n * 0.02, UP_Y - 1.5), v(ua - a.n * 0.02, UP_Y - 1.5), v(ua - a.n * 0.02, UP_Y - 0.7), v(ub - b.n * 0.02, UP_Y - 0.7)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.tube(m.rail, v(ua + a.n * 0.15, UP_Y + 1.0), v(ub + b.n * 0.15, UP_Y + 1.0), 0.03, 6);
        let u_rows = UP_ROWS as i32;
        for j in 0..u_rows {
            let n0a = a.upper + 0.3 + j as f32 * UP_PITCH;
            let n0b = b.upper + 0.3 + j as f32 * UP_PITCH;
            let y = UP_Y + j as f32 * UP_RISE;
            step_row(g, m, &a, &b, n0a, n0b, UP_PITCH, y, UP_RISE, in_aisle);
            if !in_aisle {
                seat_row(g, f, at(&a, n0a + 0.4), at(&b, n0b + 0.4), y, -(a.n + b.n).normalize());
            }
        }
        if in_aisle {
            // centre rail down the aisle
            let (p0, p1) = (at(&a, a.upper + 0.3), at(&a, a.upper + 0.3 + (u_rows as f32 - 1.0) * UP_PITCH));
            g.tube(m.rail, v(p0, UP_Y + 0.9), v(p1, UP_Y + (u_rows as f32 - 1.0) * UP_RISE + 0.9), 0.025, 6);
        }
        let up_back_a = a.upper + 0.3 + u_rows as f32 * UP_PITCH;
        let up_back_b = b.upper + 0.3 + u_rows as f32 * UP_PITCH;
        let up_top = UP_Y + u_rows as f32 * UP_RISE;
        // soffit under the upper deck (from the facia's foot back and up, parallel to the rake)
        if home_gate(a.bearing) && home_gate(b.bearing) {
            let under_rows = Key::new(Mat::Plain(215), [0.42, 0.43, 0.44]);
            // behind the open frame the precast rows show from below as a staircase: each row's underside, and a
            // step up at its outer edge to the next
            for j in 0..u_rows {
                let (n0a, n0b) = (a.upper + 0.3 + j as f32 * UP_PITCH, b.upper + 0.3 + j as f32 * UP_PITCH);
                let (n1a, n1b) = (n0a + UP_PITCH, n0b + UP_PITCH);
                let y = UP_Y + j as f32 * UP_RISE - 0.55;
                poly_down(g, under_rows, &[v(at(&a, n0a), y), v(at(&b, n0b), y), v(at(&b, n1b), y), v(at(&a, n1a), y)]);
                if j + 1 < u_rows {
                    g.poly(under_rows, &[v(at(&a, n1a), y), v(at(&b, n1b), y), v(at(&b, n1b), y + UP_RISE), v(at(&a, n1a), y + UP_RISE)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                }
            }
        } else {
            g.poly(m.soffit, &[v(ua, UP_Y - 2.3), v(ub, UP_Y - 2.3), v(at(&b, up_back_b), up_top - 2.3), v(at(&a, up_back_a), up_top - 2.3)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, -0.6, a.n.y).normalize());
        }
        // back rail and the sandstone back wall down to the ground
        // the back: a stucco parapet band behind the top rows only; below it the white steel frame stands open
        // over sandstone concourse blocks at street level (as in the exterior photos); a doorway where a stair core
        // reaches the top row
        let (wa, wb) = (at(&a, up_back_a + 0.2), at(&b, up_back_b + 0.2));
        let band = up_top - 3.0;
        // the home plate gate: the frame stands fully exposed, concourse decks stacked behind it
        let gz = home_gate(a.bearing) && home_gate(b.bearing);
        let band_mat = if gz { m.white } else { m.stucco };
        if gz && std::env::var("FPS_GZ_DEBUG").is_ok() {
            eprintln!("gz {:.1} t_tread {:.2} up_back {:.1} upper {:.1} t_back {:.1}", a.bearing, t_top - TERR_RISE, up_back_a, a.upper, t_back_a);
        }
        if gz {
            // the letters stand proud of the frame
            gate_line.push((wa + a.n * (FRAME_OFF + GZ_OVERHANG), a.n));
            gate_line.push((wb + b.n * (FRAME_OFF + GZ_OVERHANG), b.n));
            gz_line.push((at(&a, up_back_a + FRAME_OFF), a.n));
            gz_line.push((at(&b, up_back_b + FRAME_OFF), b.n));
            let t_tread = t_top - TERR_RISE;
            let edge = 0.35;
            let at_gate_stair = GATES.iter().any(|&gb| (a.bearing - gb).abs() < 2.2);
            // the upper deck's soffit height at offset r (it rises with the rake from the front to the back)
            let soffit = |s: &Station, ub: f32, r: f32| (UP_Y - 2.3) + (up_top - UP_Y) * ((r - s.upper) / (ub - s.upper)).clamp(0.0, 1.0);
            // decks (height, inner offset at a and b, outer offset at a and b, fascia depth): a mezzanine over the
            // street-level wall, set back; the terrace concourse level with the terrace's top row; the upper
            // concourse, out from under the low end of the soffit; each out to the frame
            let mut decks = vec![];
            if !at_gate_stair {
                decks.push((CONCOURSE_Y, a.terrace + 9.0, b.terrace + 9.0, up_back_a + edge, up_back_b + edge, 0.8));
                // a strip of lights under its edge
                let (la, lb) = (at(&a, up_back_a + edge - 0.6), at(&b, up_back_b + edge - 0.6));
                poly_down(g, m.led_warm, &[v(la, CONCOURSE_Y - 0.82), v(lb, CONCOURSE_Y - 0.82), v(lb + b.n * 0.25, CONCOURSE_Y - 0.82), v(la + a.n * 0.25, CONCOURSE_Y - 0.82)]);
            }
            decks.push((t_tread + 0.25, t_back_a + 0.3, t_back_b + 0.3, up_back_a + edge, up_back_b + edge, 1.35));
            let (ri_a, ri_b) = (a.upper + 0.36 * (up_back_a - a.upper), b.upper + 0.36 * (up_back_b - b.upper));
            decks.push((GZ_UPPER_CONCOURSE, ri_a, ri_b, up_back_a + edge, up_back_b + edge, 1.4));
            for &(y, ra_in, rb_in, ra_out, rb_out, fascia) in &decks {
                let (ia, ib, oa, ob) = (at(&a, ra_in), at(&b, rb_in), at(&a, ra_out), at(&b, rb_out));
                let q = [ia, ib, ob, oa];
                poly_up(g, m.concrete, &q.map(|p| v(p, y)));
                g.poly(m.soffit, &[v(oa, y - 0.35), v(ob, y - 0.35), v(ib, y - 0.35), v(ia, y - 0.35)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, 0.0, a.n.y));
                g.block_ring(&q, y - 0.35, y);
                // white fascia, and the rail along the deck edge (none where a stair core's bridge meets it)
                g.poly(m.white, &[v(oa, y - fascia), v(ob, y - fascia), v(ob, y + 0.1), v(oa, y + 0.1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                if !cores.iter().any(|c| c.covers((oa + ob) / 2.0)) && !point_in(&bridge_ring(), (oa + ob) / 2.0) {
                    g.tube(m.rail, v(oa, y + 1.05), v(ob, y + 1.05), 0.03, 5);
                    g.poly(m.guard, &[v(oa, y + 0.05), v(ob, y + 0.05), v(ob, y + 1.0), v(oa, y + 1.0)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                    g.block_ring(&[oa, ob, ob + b.n * 0.1, oa + a.n * 0.1], y, y + 1.05);
                }
                // the concourse's back wall (pale plaster, lit) up to the soffit or the deck above
                let above = decks.iter().map(|d| d.0).filter(|&d| d > y + 0.5).fold(f32::MAX, f32::min);
                let top_y = (soffit(&a, up_back_a, ra_in) - 0.05).min(above - 0.35);
                if top_y > y + 0.5 && top_y < 60.0 {
                    g.poly(m.interior, &[v(ia, y), v(ib, y), v(ib, top_y), v(ia, top_y)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                    if i % 4 == 0 {
                        g.light(v(ia.lerp(oa, 0.5), top_y - 0.4), 7.0, [1.1, 1.0, 0.85]);
                    }
                }
            }
            // the top row's walk out to the frame: a strip, the white fascia under it, and the rail behind the letters
            let top_row = UP_Y + (UP_ROWS - 1.0) * UP_RISE;
            let (ba, bb, oa, ob) = (at(&a, up_back_a), at(&b, up_back_b), at(&a, up_back_a + edge), at(&b, up_back_b + edge));
            poly_up(g, m.concrete, &[v(ba, top_row), v(bb, top_row), v(ob, top_row), v(oa, top_row)]);
            g.poly(m.white, &[v(oa, GZ_BOX_BOTTOM), v(ob, GZ_BOX_BOTTOM), v(ob, top_row + 0.1), v(oa, top_row + 0.1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        }
        let (ra, rb) = (at(&a, up_back_a), at(&b, up_back_b));
        let (wa1, wb1) = (wa + a.n * 0.4, wb + b.n * 0.4);
        for (f0, f1, door) in runs(|f| cores.iter().any(|c| c.door(wa.lerp(wb, f), c.top_side))) {
            if gz {
                break;
            }
            let (p0, p1) = (wa.lerp(wb, f0), wa.lerp(wb, f1));
            let (q0, q1) = (wa1.lerp(wb1, f0), wa1.lerp(wb1, f1));
            let h = if door { up_top - 0.5 } else { up_top + 1.4 };
            g.poly(band_mat, &[v(p0, band), v(p1, band), v(p1, h), v(p0, h)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            g.poly(band_mat, &[v(p1, band), v(p0, band), v(p0, h), v(p1, h)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
            g.block_ring(&[p0, p1, q1, q0], band, h);
            if !door {
                g.tube(m.rail, v(ra.lerp(rb, f0), up_top + 1.1), v(ra.lerp(rb, f1), up_top + 1.1), 0.03, 6);
            }
        }
        // the white steel frame: a raker every BAY metres along the back (stations spread far apart out here behind
        // home), Λ braces between them and a mid girder
        let seg = (wb - wa).length();
        let eb = (wb - wa).normalize_or_zero();
        let mut k = (back_arc / BAY).ceil();
        while k * BAY < back_arc + seg {
            let f = (k * BAY - back_arc) / seg.max(1e-3);
            k += 1.0;
            let (wc, uc) = (wa.lerp(wb, f), ua.lerp(ub, f));
            let nc = a.n.lerp(b.n, f).normalize();
            let back_c = at(&a, up_back_a).lerp(at(&b, up_back_b), f);
            let terr_c = at(&a, t_back_a + 0.2).lerp(at(&b, t_back_b + 0.2), f);
            if cores.iter().any(|c| c.covers(wc)) {
                prev_col = None;
                continue;
            }
            if !gz {
                g.member(m.steel, v(uc, UP_Y - 3.0), v(back_c, up_top - 3.0), 0.5, 0.45, Vec3::new(eb.x, 0.0, eb.y));
            }
            if gz {
                prev_col = Some(wc);
                continue;
            }
            let nv = Vec3::new(nc.x, 0.0, nc.y);
            g.member(m.steel, v(wc, 0.0), v(wc, band), 0.6, 0.6, nv);
            g.member(m.steel, v(wc, band * 0.45), v(terr_c, t_top - 0.5), 0.35, 0.35, Vec3::new(eb.x, 0.0, eb.y));
            g.block_ring(&[wc - eb * 0.35 - nc * 0.35, wc + eb * 0.35 - nc * 0.35, wc + eb * 0.35 + nc * 0.35, wc - eb * 0.35 + nc * 0.35], 0.0, band);
            if let Some(pc) = prev_col {
                let mid = (pc + wc) / 2.0;
                let lo = 14.0;
                g.member(m.steel, v(pc, lo), v(mid, band - 1.2), 0.24, 0.24, nv);
                g.member(m.steel, v(wc, lo), v(mid, band - 1.2), 0.24, 0.24, nv);
                g.member(m.steel, v(pc, (lo + band) / 2.0), v(wc, (lo + band) / 2.0), 0.28, 0.28, nv);
            }
            prev_col = Some(wc);
        }
        back_arc += seg;
        // a white horizontal girder along the back at the upper concourse level
        if !gz { g.poly(m.steel, &[v(wa, band - 1.2), v(wb, band - 1.2), v(wb, band), v(wa, band)], Vec3::new(e.x, 0.0, e.y), Vec3::Y); }
        // sandstone concourse blocks at street level, stepping in height along the bowl
        let blk = 9.0 + 5.0 * (((arc[i] / 23.0).floor() as i32).rem_euclid(3) as f32);
        let (ga, gb) = (at(&a, up_back_a - 6.0), at(&b, up_back_b - 6.0));
        let (oa, ob) = (wa + a.n * 4.0, wb + b.n * 4.0);
        let pieces = runs(|f| cores.iter().any(|c| c.covers(oa.lerp(ob, f))));
        if pieces.len() > 1 || pieces[0].2 {
            // a stair core stands here: blocks either side of it only
            for (f0, f1, hole) in pieces {
                if hole {
                    continue;
                }
                let (pa, pb, qa, qb) = (oa.lerp(ob, f0), oa.lerp(ob, f1), ga.lerp(gb, f0), ga.lerp(gb, f1));
                wall_poly(g, m.sandstone, &[v(pa, 0.0), v(pb, 0.0), v(pb, blk), v(pa, blk)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                poly_up(g, m.concrete, &[v(qa, blk), v(qb, blk), v(pb, blk), v(pa, blk)]);
                g.block_ring(&[qa, qb, pb, pa], 0.0, blk);
            }
        } else if GATES.iter().any(|&gb| (a.bearing - gb).abs() < 2.2) {
            // a gate: the blocks open and a stair climbs from the sidewalk to the concourse under the decks
            gate_stair(g, m, &a, &b, up_back_a + 4.0, a.terrace + 9.0);
        } else if gz {
            // under the decks at the home plate gate: an open arcade, a sandstone wall set well back
            let (pa, pb) = (at(&a, up_back_a - 4.0), at(&b, up_back_b - 4.0));
            wall_poly(g, m.sandstone, &[v(pa, 0.0), v(pb, 0.0), v(pb, CONCOURSE_Y - 0.35), v(pa, CONCOURSE_Y - 0.35)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            g.block_ring(&[ga, gb, pb, pa], 0.0, CONCOURSE_Y - 0.35);
        } else {
            wall_poly(g, m.sandstone, &[v(oa, 0.0), v(ob, 0.0), v(ob, blk), v(oa, blk)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            poly_up(g, m.concrete, &[v(ga, blk), v(gb, blk), v(ob, blk), v(oa, blk)]);
            g.block_ring(&[ga, gb, ob, oa], 0.0, blk);
        }
        // --- canopy: white steel over the back rows (55-305 degrees)
        if a.bearing > 55.0 && a.bearing < 305.0 {
            let cy = 39.4;
            let (c0a, c0b) = (at(&a, a.canopy), at(&b, b.canopy));
            let (c1a, c1b) = (at(&a, a.canopy + CANOPY_DEPTH), at(&b, b.canopy + CANOPY_DEPTH));
            poly_up(g, m.roof, &[v(c0a, cy + 0.6), v(c0b, cy + 0.6), v(c1b, cy + 1.1), v(c1a, cy + 1.1)]);
            g.poly(m.steel, &[v(c0b, cy - 0.6), v(c0a, cy - 0.6), v(c0a, cy + 0.6), v(c0b, cy + 0.6)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
            // underside (white) and a truss every few stations
            g.poly(m.steel, &[v(c0b, cy - 0.6), v(c1b, cy - 0.2), v(c1a, cy - 0.2), v(c0a, cy - 0.6)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, 0.0, a.n.y));
            if i % 6 == 0 {
                truss(g, m, c0a, c1a, cy - 0.6, up_top + 1.4);
                // floodlights along the canopy's front edge
                g.cuboid(m.led, v(c0a - a.n * 0.3, cy - 0.95) - Vec3::splat(0.25), v(c0a - a.n * 0.3, cy - 0.95) + Vec3::splat(0.25));
            }
        }
    }
    home_gate_front(g, m, &gate_line);
    gate_frame(g, m, &gz_line, cores);
    footbridge(g, m, &gz_line);
    home_towers(g, m);
    sw_offices(g, m);
    east_building(g, m);
    // end walls closing the bowl at both foul poles
    for s in [st[0], st[st.len() - 1]] {
        let e = s.n.perp();
        let pts = [s.p + s.n * s.front, s.p + s.n * (s.upper + 0.3 + UP_ROWS * UP_PITCH + 0.6)];
        let top = UP_Y + UP_ROWS * UP_RISE + 1.4;
        wall_poly(g, m.sandstone, &[v(pts[0], 0.0), v(pts[1], 0.0), v(pts[1], top), v(pts[0], top * 0.25)], Vec3::new(s.n.x, 0.0, s.n.y), Vec3::Y);
        wall_poly(g, m.sandstone, &[v(pts[1], 0.0), v(pts[0], 0.0), v(pts[0], top * 0.25), v(pts[1], top)], Vec3::new(-s.n.x, 0.0, -s.n.y), Vec3::Y);
        let _ = e;
        let _ = front;
    }
}

/// One row of steps (tread + riser) between stations a and b; in an aisle the riser is split in two half steps.
fn step_row(g: &mut Geo, m: &Mats, a: &Station, b: &Station, n0a: f32, n0b: f32, pitch: f32, y: f32, rise: f32, in_aisle: bool) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    let e = (b.p - a.p).normalize_or_zero();
    let ev = Vec3::new(e.x, 0.0, e.y);
    let (n1a, n1b) = (n0a + pitch, n0b + pitch);
    if in_aisle {
        let (ma, mb) = (n0a + pitch / 2.0, n0b + pitch / 2.0);
        for (p0a, p0b, p1a, p1b, yy) in [(n0a, n0b, ma, mb, y), (ma, mb, n1a, n1b, y + rise / 2.0)] {
            g.poly(m.concrete, &[v(at(a, p0a), yy), v(at(a, p1a), yy), v(at(b, p1b), yy), v(at(b, p0b), yy)], Vec3::new(a.n.x, 0.0, a.n.y), ev);
            g.poly(m.riser, &[v(at(a, p1a), yy), v(at(b, p1b), yy), v(at(b, p1b), yy + rise / 2.0), v(at(a, p1a), yy + rise / 2.0)], ev, Vec3::Y);
            g.block_ring(&[at(a, p0a), at(a, p1a), at(b, p1b), at(b, p0b)], yy - 1.0, yy);
        }
    } else {
        g.poly(m.concrete, &[v(at(a, n0a), y), v(at(a, n1a), y), v(at(b, n1b), y), v(at(b, n0b), y)], Vec3::new(a.n.x, 0.0, a.n.y), ev);
        g.poly(m.riser, &[v(at(a, n1a), y), v(at(b, n1b), y), v(at(b, n1b), y + rise), v(at(a, n1a), y + rise)], ev, Vec3::Y);
        g.block_ring(&[at(a, n0a), at(a, n1a), at(b, n1b), at(b, n0b)], y - 1.0, y);
    }
}

/// Seats every 0.5 m along a row segment (centred so neighbouring segments line up), facing `face`.
fn seat_row(g: &mut Geo, f: &mut Furnisher, a: Vec2, b: Vec2, y: f32, face: Vec2) {
    let l = (b - a).length();
    let n = (l / 0.5).round() as i32;
    for k in 0..n {
        let t = (k as f32 + 0.5) / n as f32;
        f.put_tint(g, SEAT, a + (b - a) * t, y, face, NAVY);
    }
}

/// A canopy truss: a white steel rafter from the canopy's front to its back with a mast down to the back wall.
fn truss(g: &mut Geo, m: &Mats, front: Vec2, back: Vec2, y: f32, wall_top: f32) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    g.tube(m.steel, v(front, y), v(back, y + 0.4), 0.12, 6);
    g.tube(m.steel, v(front, y - 1.4), v(back, y - 0.6), 0.09, 6);
    let n = 6;
    for k in 0..n {
        let (t0, t1) = (k as f32 / n as f32, (k + 1) as f32 / n as f32);
        let (p0, p1) = (front.lerp(back, t0), front.lerp(back, t1));
        g.tube(m.steel, v(p0, y - 1.4 + 0.8 * t0), v(p1, y + 0.4 * t1), 0.05, 4);
    }
    g.tube(m.steel, v(back, wall_top), v(back, y + 0.4), 0.18, 8);
}

// ---------------------------------------------------------------- outfield

/// Stations along the outfield wall between two bearings, normals pointing away from home.
fn wall_stations(b0: f32, b1: f32) -> Vec<Station> {
    let n = ((b1 - b0) / 0.6).ceil() as i32;
    (0..=n)
        .map(|k| {
            let b = b0 + (b1 - b0) * k as f32 / n as f32;
            let p = dir(b) * interp(&WALL, b);
            let (pa, pb) = (dir(b - 0.3) * interp(&WALL, b - 0.3), dir(b + 0.3) * interp(&WALL, b + 0.3));
            let e = (pb - pa).normalize();
            let mut nn = Vec2::new(-e.y, e.x);
            if nn.dot(p) < 0.0 {
                nn = -nn;
            }
            Station { p, n: nn, front: 0.0, lb_end: 0.0, terrace: 0.0, upper: 0.0, canopy: 0.0, bearing: b }
        })
        .collect()
}

/// One tier of outfield seating: rows from offset n0 to n1 behind the wall, rising y0 -> y1; a facia below its
/// front, a soffit under it if it stands high, and a back wall.
fn tier(g: &mut Geo, f: &mut Furnisher, m: &Mats, st: &[Station], n0: f32, n1: f32, y0: f32, y1: f32, facia_from: f32) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    let rows = ((n1 - n0) / LB_PITCH).floor().max(1.0) as i32;
    let rise = (y1 - y0) / rows as f32;
    let mut arc = 0.0f32;
    for i in 0..st.len() - 1 {
        let (a, b) = (st[i], st[i + 1]);
        let e = (b.p - a.p).normalize_or_zero();
        let ev = Vec3::new(e.x, 0.0, e.y);
        arc += (b.p - a.p).length();
        let in_aisle = ((arc / 11.5).round() * 11.5 - arc).abs() < 0.65;
        for j in 0..rows {
            let n = n0 + j as f32 * LB_PITCH;
            let y = y0 + j as f32 * rise;
            step_row(g, m, &a, &b, n, n, LB_PITCH, y, rise, in_aisle && rise > 0.3);
            if !in_aisle {
                seat_row(g, f, at(&a, n + 0.42), at(&b, n + 0.42), y, -(a.n + b.n).normalize());
            }
        }
        // facia under the front row and the rail on it
        let (fa, fb) = (at(&a, n0), at(&b, n0));
        g.poly(m.navy, &[v(fb, facia_from), v(fa, facia_from), v(fa, y0), v(fb, y0)], -ev, Vec3::Y);
        g.tube(m.rail, v(fa, y0 + 1.0), v(fb, y0 + 1.0), 0.03, 6);
        // soffit under the tier and the back wall
        let (ba, bb) = (at(&a, n0 + rows as f32 * LB_PITCH), at(&b, n0 + rows as f32 * LB_PITCH));
        if facia_from > 1.0 {
            g.poly(m.stucco, &[v(fa, facia_from), v(fb, facia_from), v(bb, y1 - 1.5), v(ba, y1 - 1.5)], ev, Vec3::new(a.n.x, 0.0, a.n.y));
        }
        wall_poly(g, m.sandstone, &[v(ba, 0.0), v(bb, 0.0), v(bb, y1 + 1.4), v(ba, y1 + 1.4)], ev, Vec3::Y);
        g.poly(m.stucco, &[v(bb, 0.0), v(ba, 0.0), v(ba, y1 + 1.4), v(bb, y1 + 1.4)], -ev, Vec3::Y);
        g.block_ring(&[ba, bb, bb + b.n * 0.4, ba + a.n * 0.4], 0.0, y1 + 1.4);
    }
}

/// Beyond the outfield wall (all from the lidar): the right-field stands (two tiers), the left-field stands over a
/// low terrace, the bullpens in left-centre, the raised centre-field terrace and the batter's eye.
fn outfield(g: &mut Geo, f: &mut Furnisher, m: &Mats) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    // right field: lower rows from the wall (1.6 -> 7.7 m over 20 m), the upper deck from 24 m out (14.5 -> 24 m)
    let rf = wall_stations(18.0, 44.5);
    tier(g, f, m, &rf, 1.2, 21.0, 1.6, 7.7, 0.0);
    tier(g, f, m, &rf, 24.0, 42.0, 14.5, 24.0, 12.0);
    // left field: a low terrace (2 m) behind the wall, then rows from 6 m out climbing 8 -> 14 m
    let lf = wall_stations(-39.0, -16.0);
    for w in lf.windows(2) {
        let (a, b) = (w[0], w[1]);
        poly_up(g, m.concrete, &[v(a.p + a.n * 0.45, 2.0), v(b.p + b.n * 0.45, 2.0), v(b.p + b.n * 6.0, 2.0), v(a.p + a.n * 6.0, 2.0)]);
        g.block_ring(&[a.p + a.n * 0.45, b.p + b.n * 0.45, b.p + b.n * 6.0, a.p + a.n * 6.0], 0.0, 2.0);
        g.tube(m.rail, v(a.p + a.n * 0.5, 3.0), v(b.p + b.n * 0.5, 3.0), 0.03, 6);
    }
    tier(g, f, m, &lf, 6.2, 18.0, 8.0, 13.6, 2.0);
    // the bullpens in left-centre: field level, two mounds side by side, a fence behind the wall
    let bp = wall_stations(-15.5, -5.5);
    for w in bp.windows(2) {
        let (a, b) = (w[0], w[1]);
        poly_up(g, m.grass, &[v(a.p + a.n * 0.45, 0.01), v(b.p + b.n * 0.45, 0.01), v(b.p + b.n * 18.0, 0.01), v(a.p + a.n * 18.0, 0.01)]);
        // retaining wall up to the terrace at the back
        let e = (b.p - a.p).normalize();
        wall_poly(g, m.sandstone, &[v(a.p + a.n * 18.0, 0.0), v(b.p + b.n * 18.0, 0.0), v(b.p + b.n * 18.0, 3.5), v(a.p + a.n * 18.0, 3.5)], -Vec3::new(e.x, 0.0, e.y), Vec3::Y);
    }
    for (bb, k) in [(-13.0f32, 0), (-8.0, 1)] {
        let p = dir(bb) * (interp(&WALL, bb) + 9.0);
        let _ = k;
        disc(g, m.dirt, p, 2.7, 0.03);
        mound(g, m, p);
    }
    // centre-field terrace at 3.5 m (grass) from 6 m behind the wall, with the batter's eye on it
    let cf = wall_stations(-5.5, 17.0);
    for w in cf.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n0 = if a.bearing < 6.0 { 6.0 } else { 13.0 };
        poly_up(g, m.grass_dark, &[v(a.p + a.n * n0, 3.5), v(b.p + b.n * n0, 3.5), v(b.p + b.n * 45.0, 3.5), v(a.p + a.n * 45.0, 3.5)]);
        let e = (b.p - a.p).normalize();
        wall_poly(g, m.sandstone, &[v(b.p + b.n * n0, 0.0), v(a.p + a.n * n0, 0.0), v(a.p + a.n * n0, 3.5), v(b.p + b.n * n0, 3.5)], -Vec3::new(e.x, 0.0, e.y) * -1.0, Vec3::Y);
        poly_up(g, m.grass_dark, &[v(a.p + a.n * 0.45, 0.01), v(b.p + b.n * 0.45, 0.01), v(b.p + b.n * n0, 0.01), v(a.p + a.n * n0, 0.01)]);
        g.block_ring(&[a.p + a.n * n0, b.p + b.n * n0, b.p + b.n * 45.0, a.p + a.n * 45.0], 0.0, 3.5);
    }
    // the batter's eye: a dark box 13.5 m high with a white pavilion roof above (bearings -4.5..4.5, 12-30 m out)
    let eye = Key::new(Mat::Plain(200), [0.05, 0.055, 0.06]);
    let (b0, b1) = (-4.5f32, 4.5);
    let p = |b: f32, n: f32| dir(b) * (interp(&WALL, b) + n);
    let c = [p(b0, 12.0), p(b1, 12.0), p(b1, 30.0), p(b0, 30.0)];
    for k in 0..4 {
        let (a, b) = (c[k], c[(k + 1) % 4]);
        let e = (b - a).normalize();
        g.poly(eye, &[v(a, 3.5), v(b, 3.5), v(b, 13.5), v(a, 13.5)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
    }
    poly_up(g, eye, &c.map(|q| v(q, 13.5)));
    g.block_ring(&c, 0.0, 13.5);
    let r = [p(b0 - 0.5, 10.0), p(b1 + 0.5, 10.0), p(b1 + 0.5, 32.0), p(b0 - 0.5, 32.0)];
    poly_up(g, m.roof, &r.map(|q| v(q, 17.3)));
    for q in r {
        g.tube(m.steel, v(q, 13.5), v(q, 17.3), 0.15, 8);
    }
}

// ---------------------------------------------------------------- Western Metal Supply Co., towers, boards

/// The 1909 Western Metal Supply Co. building in the left-field corner (OSM footprint, 23 m to the parapet):
/// red brick, four tall storeys of windows, a painted name band, seating balconies on the field sides, rooftop
/// bleachers under a white canopy. The left-field pole stands at its south-east corner.
fn western_metal(g: &mut Geo, m: &Mats, _c: &city::City) {
    let brick = Key::new(Mat::Surface("ufmhehts"), [0.95, 0.85, 0.8]).tile(2.0).wear(wear::STONE);
    let trim = Key::new(Mat::Plain(160), [0.82, 0.78, 0.7]);
    let win = Key::new(Mat::Glass, [0.25, 0.3, 0.32]);
    let (x0, x1, z0, z1) = (-105.4 - HOME.x, -74.1 - HOME.x, -94.2 - HOME.y, -62.1 - HOME.y);
    let h = 23.1;
    let corners = [Vec2::new(x0, z1), Vec2::new(x1, z1), Vec2::new(x1, z0), Vec2::new(x0, z0)];
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    for k in 0..4 {
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let e = (b - a).normalize();
        let out = Vec2::new(e.y, -e.x);
        let len = (b - a).length();
        // wall, then windows: 4 storeys x bays of 3.9 m, recessed glass with stone sills
        g.poly(brick, &[v(a, 0.0), v(b, 0.0), v(b, h), v(a, h)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        let bays = (len / 3.9).floor() as i32;
        let bw = len / bays as f32;
        for fl in 0..4 {
            let y0 = 1.6 + fl as f32 * 5.0;
            for bi in 0..bays {
                if bi == 0 || bi == bays - 1 {
                    continue; // brick corner piers
                }
                let p0 = a + e * (bi as f32 * bw + 0.7) + out * 0.02;
                let wv = e * (bw - 1.4);
                g.quad(win, v(p0, y0), Vec3::new(wv.x, 0.0, wv.y), Vec3::Y * 3.2);
                g.quad(trim, v(p0 - e * 0.1 + out * 0.03, y0 - 0.25), Vec3::new(wv.x + e.x * 0.2, 0.0, wv.y + e.y * 0.2), Vec3::Y * 0.25);
            }
        }
        // painted name band under the parapet (field sides)
        if k == 0 || k == 1 {
            g.quad(trim, v(a + out * 0.02 + e * 1.0, h - 2.6), Vec3::new(e.x, 0.0, e.y) * (len - 2.0), Vec3::Y * 1.5);
        }
        g.quad(trim, v(a + out * 0.03, h - 0.4), Vec3::new(e.x, 0.0, e.y) * len, Vec3::Y * 0.4);
    }
    poly_up(g, m.concrete, &corners.map(|p| v(p, h)));
    g.block_ring(&corners, 0.0, h);
    // rooftop canopy over the bleachers
    let r = [Vec2::new(x0 + 3.0, z1 - 3.0), Vec2::new(x1 - 2.0, z1 - 3.0), Vec2::new(x1 - 2.0, z0 + 10.0), Vec2::new(x0 + 3.0, z0 + 10.0)];
    poly_up(g, m.roof, &r.map(|p| v(p, h + 7.4)));
    for p in r {
        g.tube(m.steel, v(p, h), v(p, h + 7.4), 0.14, 8);
    }
    // field-side balconies with a rail on each upper floor
    for fl in 1..4 {
        let y = 1.2 + fl as f32 * 5.0;
        let (a, b) = (corners[1], corners[2]);
        g.cuboid(m.concrete, v(a, y - 0.25) + Vec3::new(0.0, 0.0, 0.5), v(b, y) + Vec3::new(1.6, 0.0, -0.5));
        g.tube(m.rail, v(a + Vec2::new(1.6, 0.5), y + 1.0), v(b + Vec2::new(1.6, -0.5), y + 1.0), 0.03, 6);
    }
}

/// Light towers (world centre, lidar footprint half-size, top height): sandstone shafts with white lattice crowns
/// carrying the lamp banks, aimed at the field.
const TOWERS: [(f32, f32, f32, f32); 6] = [
    (67.1, 21.2, 5.5, 64.6),
    (-67.9, 22.8, 5.5, 65.0),
    (53.2, 42.3, 4.0, 54.0),
    (-54.6, 42.8, 4.0, 54.3),
    (87.0, -109.8, 4.5, 58.5),
    (-58.3, -108.7, 4.0, 66.0),
];

fn tower(g: &mut Geo, m: &Mats, t: (f32, f32, f32, f32)) {
    let c = Vec2::new(t.0, t.1) - HOME;
    let (r, top) = (t.2, t.3);
    let shaft_top = top - 15.0;
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    // the shafts are pale precast, not sandstone (photos)
    g.solid(pale_precast(), v(c - Vec2::splat(r * 0.8), 0.0), v(c + Vec2::splat(r * 0.8), shaft_top));
    // lattice crown: four legs and rings
    let aim = (Vec2::new(0.0, -50.0) - c).normalize();
    let side = aim.perp();
    let legs = [c + side * r - aim * r * 0.5, c - side * r - aim * r * 0.5, c - side * r + aim * r * 0.6, c + side * r + aim * r * 0.6];
    for p in legs {
        g.tube(m.steel, v(p, shaft_top), v(p + aim * 0.8, top - 1.0), 0.16, 6);
    }
    for k in 0..4 {
        let y = shaft_top + 3.0 + k as f32 * 3.0;
        for i in 0..4 {
            g.tube(m.steel, v(legs[i], y), v(legs[(i + 1) % 4], y + 1.5), 0.07, 4);
        }
    }
    // the lamp bank: a frame facing the field with rows of lamps (lit)
    let face = c + aim * (r * 0.6 + 0.8);
    let bw = r * 2.0;
    g.quad(m.steel, v(face - side * bw / 2.0 + aim * 0.3, top - 7.5), Vec3::new(side.x, 0.0, side.y) * bw, Vec3::Y * 7.0);
    for row in 0..5 {
        for col in 0..((bw / 1.1) as i32) {
            let p = face - side * (bw / 2.0 - 0.55 - col as f32 * 1.1) + aim * 0.4;
            let y = top - 7.0 + row as f32 * 1.35;
            g.cuboid(m.led, v(p, y) - Vec3::splat(0.33), v(p, y) + Vec3::splat(0.33));
        }
    }
}

/// A video board on a white steel frame: the screen faces `face`; the left-field one has a name sign on top.
fn board(g: &mut Geo, m: &Mats, c: Vec2, face: Vec2, w: f32, y0: f32, y1: f32, sign: bool) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let side = Vec2::new(-face.y, face.x);
    let a = c - side * (w / 2.0);
    let screen = Key::new(Mat::Screen("scr_ballgame"), [1.0, 1.0, 1.0]).tile(w * 0.94);
    // dark cabinet, then the picture on its face
    g.solid(m.navy, v(c - side * (w / 2.0) - face * 1.5, y0) .min(v(c + side * (w / 2.0), y1)), v(c - side * (w / 2.0) - face * 1.5, y0).max(v(c + side * (w / 2.0), y1)));
    let inset = 0.6;
    g.decal(screen, v(a + side * inset + face * 0.02, y0 + inset), Vec3::new(side.x, 0.0, side.y) * (w - 2.0 * inset), Vec3::Y * (y1 - y0 - 2.0 * inset), w * 0.94);
    // legs down to the stands' back
    for t in [0.1f32, 0.5, 0.9] {
        let p = a + side * (w * t) - face * 0.8;
        g.tube(m.steel, v(p, 14.0), v(p, y0), 0.35, 8);
    }
    if sign {
        let k = Key::new(Mat::Light, [1.4, 1.4, 1.35]);
        g.quad(m.navy, v(c - side * 9.0 + face * 0.05, y1 + 0.3), Vec3::new(side.x, 0.0, side.y) * 18.0, Vec3::Y * 3.0);
        g.quad(k, v(c - side * 8.0 + face * 0.08, y1 + 1.1), Vec3::new(side.x, 0.0, side.y) * 16.0, Vec3::Y * 1.4);
    }
}

// ---------------------------------------------------------------- dugouts, netting

/// The dugouts (station bearings from home): first-base side (visitors) and third-base side (home), ~30 m long and
/// centred off the bases, where the aerial shows the breaks in the front rows.
const DUGOUTS: [(f32, f32); 2] = [(57.0, 80.0), (280.0, 303.0)];
const DUG_DEPTH: f32 = 4.2;

fn dugout_at(b: f32) -> bool {
    DUGOUTS.iter().any(|&(a, c)| b > a && b < c)
}

/// One segment of a dugout: sunk 1.2 m with a bench along its back wall, its roof at 1.1 m (the front rows start
/// above it), a padded roof edge and a rail along the front; closed to players (a block over the pit).
fn dugout(g: &mut Geo, m: &Mats, a: &Station, b: &Station) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    let e = (b.p - a.p).normalize_or_zero();
    let ev = Vec3::new(e.x, 0.0, e.y);
    let (f0a, f0b) = (at(a, a.front), at(b, b.front));
    let (bka, bkb) = (at(a, a.front + DUG_DEPTH), at(b, b.front + DUG_DEPTH));
    let floor = -1.2;
    poly_up(g, m.concrete, &[v(f0a, floor), v(f0b, floor), v(bkb, floor), v(bka, floor)]);
    // the field-side lip down into the pit, the back wall, the bench
    g.poly(m.pad, &[v(f0b, floor), v(f0a, floor), v(f0a, 0.0), v(f0b, 0.0)], ev, Vec3::Y);
    g.poly(m.navy, &[v(bka, floor), v(bkb, floor), v(bkb, 1.1), v(bka, 1.1)], -ev, Vec3::Y);
    let (s0a, s0b) = (at(a, a.front + DUG_DEPTH - 0.55), at(b, b.front + DUG_DEPTH - 0.55));
    g.poly(m.navy, &[v(s0a, floor + 0.45), v(s0b, floor + 0.45), v(bkb, floor + 0.45), v(bka, floor + 0.45)], ev, Vec3::new(a.n.x, 0.0, a.n.y));
    // roof slab with a padded edge facing the field
    let (r0a, r0b) = (at(a, a.front + 1.0), at(b, b.front + 1.0));
    poly_up(g, m.concrete, &[v(r0a, 1.15), v(r0b, 1.15), v(bkb, 1.15), v(bka, 1.15)]);
    g.poly(m.riser, &[v(bka, 1.0), v(bkb, 1.0), v(r0b, 1.0), v(r0a, 1.0)], ev, -Vec3::new(a.n.x, 0.0, a.n.y));
    g.poly(m.pad, &[v(r0b, 0.75), v(r0a, 0.75), v(r0a, 1.15), v(r0b, 1.15)], -ev, Vec3::Y);
    g.tube(m.rail, v(f0a + a.n * 0.2, 0.95), v(f0b + b.n * 0.2, 0.95), 0.035, 6);
    g.block_ring(&[f0a, f0b, bkb, bka], 0.0, 1.15);
}

/// Netting behind home plate from dugout to dugout: steel posts and cables with a fine, nearly clear net.
fn backstop_net(g: &mut Geo, m: &Mats, st: &[Station]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let net = Key::new(Mat::Glass, [0.25, 0.25, 0.25]).rough(0.07);
    let span: Vec<&Station> = st.iter().filter(|s| s.bearing > DUGOUTS[0].1 && s.bearing < DUGOUTS[1].0).collect();
    let h = 9.0;
    for (k, w) in span.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let (pa, pb) = (a.p + a.n * (a.front + 0.15), b.p + b.n * (b.front + 0.15));
        let e = (pb - pa).normalize_or_zero();
        g.poly(net, &[v(pa, 1.1), v(pb, 1.1), v(pb, h), v(pa, h)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        if k % 9 == 0 {
            g.tube(m.rail, v(pa, 1.1), v(pa, h + 0.3), 0.06, 6);
        }
    }
    if let (Some(a), Some(b)) = (span.first(), span.last()) {
        let _ = (a, b);
    }
    let pts: Vec<Vec2> = span.iter().map(|s| s.p + s.n * (s.front + 0.15)).collect();
    for w in pts.windows(2) {
        g.tube(m.rail, v(w[0], h), v(w[1], h), 0.02, 4);
    }
}

/// Gates (bearing from home): the home plate gate behind home and one each side.
const GATES: [f32; 3] = [172.0, 120.0, 240.0];

/// A stair from the street (at offset `n_out`) up to the concourse floor (at `n_in`), between stations a and b.
fn gate_stair(g: &mut Geo, m: &Mats, a: &Station, b: &Station, n_out: f32, n_in: f32) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    let e = (b.p - a.p).normalize_or_zero();
    let ev = Vec3::new(e.x, 0.0, e.y);
    let steps = 30;
    let rise = CONCOURSE_Y / steps as f32;
    let run = (n_out - n_in) / steps as f32;
    for k in 0..steps {
        let (n0, n1) = (n_out - k as f32 * run, n_out - (k + 1) as f32 * run);
        let y = (k + 1) as f32 * rise;
        poly_up(g, m.concrete, &[v(at(a, n0), y), v(at(b, n0), y), v(at(b, n1), y), v(at(a, n1), y)]);
        g.poly(m.riser, &[v(at(a, n0), y - rise), v(at(b, n0), y - rise), v(at(b, n0), y), v(at(a, n0), y)], ev, Vec3::Y);
        g.block_ring(&[at(a, n0), at(b, n0), at(b, n1), at(a, n1)], 0.0, y);
    }
}

// ---------------------------------------------------------------- stair cores

/// A stair core behind the bowl: a sandstone tower whose switchback stair climbs from the street to a bridge onto the
/// terrace's top row and on to the upper deck's back row, both through doorways.
struct Core {
    /// Reference point on the guide, tangent and outward normal.
    p: Vec2,
    t: Vec2,
    n: Vec2,
    /// Inner face of the tower (offset along n), the terrace's back and its top row's height, the upper deck's back
    /// and its top row's height.
    r0: f32,
    t_back: f32,
    t_tread: f32,
    up_back: f32,
    up_tread: f32,
    /// Street level at the door.
    base: f32,
    /// Which end of the tower (-1 along -t, +1 along +t) each landing is at.
    terrace_side: f32,
    top_side: f32,
}

const CORE_HALF: f32 = 3.9;
const CORE_DEPTH: f32 = 3.4;
const LANE: f32 = 1.6;
const LAND: f32 = 1.8;
const FLIGHT_H: f32 = 2.6;

impl Core {
    fn local(&self, q: Vec2) -> (f32, f32) {
        ((q - self.p).dot(self.t), (q - self.p).dot(self.n))
    }
    /// Whether a wall at `q` is in this core's doorway at landing end `side`.
    fn door(&self, q: Vec2, side: f32) -> bool {
        let (t, r) = self.local(q);
        let (t0, t1) = if side < 0.0 { (-CORE_HALF, -CORE_HALF + LAND) } else { (CORE_HALF - LAND, CORE_HALF) };
        t > t0 - 0.7 && t < t1 + 0.7 && r > self.t_back - 2.0 && r < self.r0 + 1.0
    }
    /// Whether the tower stands over `q` (the street-level blocks give way).
    fn covers(&self, q: Vec2) -> bool {
        let (t, r) = self.local(q);
        t.abs() < CORE_HALF + 1.0 && r > self.r0 - 8.0 && r < self.r0 + CORE_DEPTH + 6.0
    }
    fn at(&self, t: f32, r: f32) -> Vec2 {
        self.p + self.t * t + self.n * r
    }
    /// Flights to rise from y0 to y1 (each ~2.6 m) and the end the last one lands at, given the end it starts at.
    fn flights(y0: f32, y1: f32) -> i32 {
        ((y1 - y0) / FLIGHT_H).round().max(1.0) as i32
    }
}

/// Walking routes up each stair core (world plan points), each with the floor height expected at its end points:
/// outside the street door (ground), on the terrace's top row past the bridge, and on the upper deck's back row.
pub fn core_routes(city: &city::City) -> Vec<Vec<([f32; 2], Option<f32>)>> {
    let front: Vec<Vec2> = FRONT.iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect();
    let st = stations(&front);
    let w = |q: Vec2| [q.x + HOME.x, q.y + HOME.y];
    cores(city, &st).into_iter().map(|c| {
        let y_terr = c.t_tread + 0.25;
        let n1 = Core::flights(c.base, y_terr);
        let n2 = Core::flights(y_terr, c.up_tread + 0.2);
        let (ta, tb) = (-(CORE_HALF - LAND / 2.0), CORE_HALF - LAND / 2.0);
        let (la, lb) = (c.r0 + LANE / 2.0, c.r0 + CORE_DEPTH - LANE / 2.0);
        let mut r = vec![(w(c.at(ta, c.r0 + CORE_DEPTH + 2.5)), None), (w(c.at(ta, lb)), None)];
        let mut climb = |r: &mut Vec<([f32; 2], Option<f32>)>, n: i32| {
            for k in 0..n {
                if k % 2 == 0 {
                    r.push((w(c.at(ta, la)), None));
                    r.push((w(c.at(tb, la)), None));
                } else {
                    r.push((w(c.at(tb, lb)), None));
                    r.push((w(c.at(ta, lb)), None));
                }
            }
        };
        climb(&mut r, n1);
        r.push((w(c.at(ta, c.r0 - 1.0)), None));
        r.push((w(c.at(ta, c.t_back - 0.45)), Some(c.t_tread)));
        r.push((w(c.at(ta, c.r0 - 1.0)), None));
        r.push((w(c.at(ta, lb)), None));
        climb(&mut r, n2);
        r.push((w(c.at(ta, c.r0 - 1.0)), None));
        r.push((w(c.at(ta, c.up_back - 0.4)), Some(c.up_tread)));
        r
    })
    .collect()
}

/// The stair cores, one a little clockwise of each gate, standing on the street.
fn cores(city: &city::City, st: &[Station]) -> Vec<Core> {
    GATES.iter().filter_map(|&gb| core_at(st, gb + if gb == 172.0 { 36.0 } else { 6.0 })).map(|mut c| {
        // the ground floor is a level slab at the highest street point round the footprint
        let pts = [(-CORE_HALF, c.r0), (CORE_HALF, c.r0), (CORE_HALF, c.r0 + CORE_DEPTH + 1.0), (-CORE_HALF, c.r0 + CORE_DEPTH + 1.0), (0.0, c.r0 + CORE_DEPTH / 2.0)];
        c.base = pts.iter().map(|&(t, r)| {
            let q = c.at(t, r) + HOME;
            city.terrain.at(q.x as f64, q.y as f64) as f32
        })
        .fold(f32::MIN, f32::max) + 0.05;
        let side = |k: i32| if k % 2 == 0 { -1.0 } else { 1.0 };
        let n1 = Core::flights(c.base, c.t_tread + 0.25);
        let n2 = Core::flights(c.t_tread + 0.25, c.up_tread + 0.2);
        c.terrace_side = side(n1);
        c.top_side = side(n1 + n2);
        c
    })
    .collect()
}

fn core_at(st: &[Station], bearing: f32) -> Option<Core> {
    let s = *st.iter().min_by(|a, b| (a.bearing - bearing).abs().total_cmp(&(b.bearing - bearing).abs()))?;
    let t_rows = (((s.upper + 3.5) - (s.terrace + 0.3)) / TERR_PITCH).floor().clamp(4.0, 12.0);
    let t_back = s.terrace + 0.3 + t_rows * TERR_PITCH;
    let t_tread = TERR_Y + (t_rows - 1.0) * TERR_RISE;
    let up_back = s.upper + 0.3 + UP_ROWS * UP_PITCH;
    let up_tread = UP_Y + (UP_ROWS - 1.0) * UP_RISE;
    // flight k goes +t if k is even: an even count lands back at the start end (-t)
    let n1 = Core::flights(0.0, t_tread + 0.25);
    let n2 = Core::flights(t_tread + 0.25, up_tread + 0.2);
    let side = |k: i32| if k % 2 == 0 { -1.0 } else { 1.0 };
    Some(Core { p: s.p, t: s.n.perp(), n: s.n, r0: up_back + 0.7, t_back, t_tread, up_back, up_tread, base: 0.0, terrace_side: side(n1), top_side: side(n1 + n2) })
}

fn stair_core(g: &mut Geo, m: &Mats, c: &Core) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let y_terr = c.t_tread + 0.25;
    let y_top = c.up_tread + 0.2;
    let n1 = Core::flights(c.base, y_terr);
    let n2 = Core::flights(y_terr, y_top);
    let run = 2.0 * CORE_HALF - 2.0 * LAND;
    // the ground floor, and a threshold out to the street
    let gq = [c.at(-CORE_HALF, c.r0), c.at(CORE_HALF, c.r0), c.at(CORE_HALF, c.r0 + CORE_DEPTH + 0.3), c.at(-CORE_HALF, c.r0 + CORE_DEPTH + 0.3)];
    poly_up(g, m.concrete, &gq.map(|p| v(p, c.base)));
    g.block_ring(&gq, c.base - 1.0, c.base);
    let mut y = c.base;
    let mut k = 0;
    for (count, y_end) in [(n1, y_terr), (n2, y_top)] {
        let h = (y_end - y) / count as f32;
        for _ in 0..count {
            let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
            let (r_a, r_b) = if k % 2 == 0 { (c.r0, c.r0 + LANE) } else { (c.r0 + CORE_DEPTH - LANE, c.r0 + CORE_DEPTH) };
            let steps = (h / 0.18).ceil() as i32;
            let (rise, tread) = (h / steps as f32, run / steps as f32);
            let t_start = -dir * (CORE_HALF - LAND);
            for j in 0..steps {
                let (t0, t1) = (t_start + dir * j as f32 * tread, t_start + dir * (j + 1) as f32 * tread);
                let top = y + (j + 1) as f32 * rise;
                let q = [c.at(t0, r_a), c.at(t1, r_a), c.at(t1, r_b), c.at(t0, r_b)];
                poly_up(g, m.concrete, &q.map(|p| v(p, top)));
                let (p0, p1) = (c.at(t0, r_a), c.at(t0, r_b));
                let e = (p1 - p0).normalize();
                g.quad(m.riser, v(p0, top - rise), Vec3::new(e.x, 0.0, e.y) * LANE, Vec3::Y * rise);
                g.block_ring(&q, top - 0.5, top);
            }
            y += h;
            k += 1;
            // landing at the far end, across both lanes
            let (t0, t1) = if dir > 0.0 { (CORE_HALF - LAND, CORE_HALF) } else { (-CORE_HALF, -CORE_HALF + LAND) };
            let q = [c.at(t0, c.r0), c.at(t1, c.r0), c.at(t1, c.r0 + CORE_DEPTH), c.at(t0, c.r0 + CORE_DEPTH)];
            poly_up(g, m.concrete, &q.map(|p| v(p, y)));
            g.block_ring(&q, y - 0.3, y);
            // a lamp on each landing
            let ctr = c.at((t0 + t1) / 2.0, c.r0 + CORE_DEPTH / 2.0);
            g.light(v(ctr, y + 2.4), 6.0, [1.2, 1.1, 0.95]);
        }
    }
    // the bridge to the terrace's top row, under the upper deck
    let (t0, t1) = if c.terrace_side < 0.0 { (-CORE_HALF, -CORE_HALF + LAND) } else { (CORE_HALF - LAND, CORE_HALF) };
    let bq = [c.at(t0, c.t_back - 0.5), c.at(t1, c.t_back - 0.5), c.at(t1, c.r0), c.at(t0, c.r0)];
    poly_up(g, m.concrete, &bq.map(|p| v(p, y_terr)));
    g.block_ring(&bq, y_terr - 0.35, y_terr);
    for tt in [t0, t1] {
        g.tube(m.rail, v(c.at(tt, c.t_back), y_terr + 1.05), v(c.at(tt, c.r0), y_terr + 1.05), 0.03, 6);
        g.block_ring(&[c.at(tt - 0.05, c.t_back), c.at(tt + 0.05, c.t_back), c.at(tt + 0.05, c.r0), c.at(tt - 0.05, c.r0)], y_terr, y_terr + 1.1);
    }
    // the top landing reaches over the parapet line onto the upper deck's back row
    let (t0, t1) = if c.top_side < 0.0 { (-CORE_HALF, -CORE_HALF + LAND) } else { (CORE_HALF - LAND, CORE_HALF) };
    // (the rows are chords between stations, so it laps well over the top row)
    let tq = [c.at(t0, c.up_back - 0.6), c.at(t1, c.up_back - 0.6), c.at(t1, c.r0), c.at(t0, c.r0)];
    poly_up(g, m.concrete, &tq.map(|p| v(p, y_top)));
    g.block_ring(&tq, y_top - 0.35, y_top);
    // the tower: sandstone walls round it with doorways (street, bridge, top), and a roof
    let top = y_top + 3.2;
    let corners = [c.at(-CORE_HALF - 0.3, c.r0 - 0.3), c.at(CORE_HALF + 0.3, c.r0 - 0.3), c.at(CORE_HALF + 0.3, c.r0 + CORE_DEPTH + 0.3), c.at(-CORE_HALF - 0.3, c.r0 + CORE_DEPTH + 0.3)];
    let door_t = |side: f32| if side < 0.0 { (0.3, 0.3 + LAND) } else { (2.0 * CORE_HALF + 0.6 - 0.3 - LAND, 2.0 * CORE_HALF + 0.6 - 0.3) };
    // inner wall (corners[0] -> corners[1]): doors to the bridge and the top landing
    let y0 = c.base - 0.5;
    walled(g, m.sandstone, corners[0], corners[1], y0, top, &[(door_t(c.terrace_side), (y_terr, y_terr + 2.5)), (door_t(c.top_side), (y_top, y_top + 2.5))]);
    walled(g, m.sandstone, corners[1], corners[2], y0, top, &[]);
    // outer wall (corners[2] -> corners[3], running -t): the street door at the start end (-t)
    let len = 2.0 * CORE_HALF + 0.6;
    walled(g, m.sandstone, corners[2], corners[3], y0, top, &[((len - 0.3 - LAND, len - 0.3), (y0, c.base + 2.6))]);
    walled(g, m.sandstone, corners[3], corners[0], y0, top, &[]);
    poly_up(g, m.concrete, &corners.map(|p| v(p, top)));
}

/// A straight 0.3 m wall from a to b (its outside to the right of a->b seen from above), with rectangular doorways
/// given as (along-wall span, height span); drawn both sides, collidable except in the doorways.
fn walled(g: &mut Geo, k: Key, a: Vec2, b: Vec2, y0: f32, y1: f32, doors: &[((f32, f32), (f32, f32))]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (b - a).length();
    let e = (b - a) / len;
    let out = Vec2::new(-e.y, e.x) * -1.0;
    // cut the wall into vertical strips at the door edges, then each strip into solid spans
    let mut cuts = vec![0.0, len];
    for &((u0, u1), _) in doors {
        cuts.push(u0.clamp(0.0, len));
        cuts.push(u1.clamp(0.0, len));
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-3);
    for w in cuts.windows(2) {
        let (u0, u1) = (w[0], w[1]);
        let mid = (u0 + u1) / 2.0;
        let mut spans = vec![(y0, y1)];
        for &((d0, d1), (h0, h1)) in doors {
            if mid > d0 && mid < d1 {
                spans = spans.into_iter().flat_map(|(s0, s1)| {
                    let mut out = vec![];
                    if h0 > s0 { out.push((s0, h0.min(s1))); }
                    if h1 < s1 { out.push((h1.max(s0), s1)); }
                    out
                }).collect();
            }
        }
        let (p0, p1) = (a + e * u0, a + e * u1);
        for (s0, s1) in spans {
            if s1 - s0 < 0.05 {
                continue;
            }
            wall_poly(g, k, &[v(p0, s0), v(p1, s0), v(p1, s1), v(p0, s1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            wall_poly(g, k, &[v(p1 + out * 0.3, s0), v(p0 + out * 0.3, s0), v(p0 + out * 0.3, s1), v(p1 + out * 0.3, s1)], -Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            g.block_ring(&[p0, p1, p1 + out * 0.3, p0 + out * 0.3], s0, s1);
        }
    }
}

/// Splits a wall segment (fractions 0..1 along it) into runs that are or are not cut by `hole`, sampled finely.
fn runs(hole: impl Fn(f32) -> bool) -> Vec<(f32, f32, bool)> {
    const N: usize = 32;
    let mut out: Vec<(f32, f32, bool)> = vec![];
    for k in 0..N {
        let (f0, f1) = (k as f32 / N as f32, (k + 1) as f32 / N as f32);
        let h = hole((f0 + f1) / 2.0);
        match out.last_mut() {
            Some(l) if l.2 == h => l.1 = f1,
            _ => out.push((f0, f1, h)),
        }
    }
    out
}

// ---------------------------------------------------------------- home plate gate

/// The bearings (from home) where the frame stands exposed behind home plate.
fn home_gate(b: f32) -> bool {
    // from the tower's side round to where the east building stands in front of the bowl (photos)
    (134.0..212.0).contains(&b)
}

/// The home plate gate's street face: lettering on the white top band. `line` is the outer face of the top band (point, outward normal) station by station.
fn home_gate_front(g: &mut Geo, m: &Mats, line: &[(Vec2, Vec2)]) {
    if line.len() < 2 {
        return;
    }
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let mut pts: Vec<(Vec2, Vec2)> = vec![line[0]];
    for &q in &line[1..] {
        if (q.0 - pts.last().unwrap().0).length() > 0.05 {
            pts.push(q);
        }
    }
    // read left to right from the street, i.e. against the station order
    pts.reverse();
    let mut s = vec![0.0f32];
    for w in pts.windows(2) {
        s.push(s.last().unwrap() + (w[1].0 - w[0].0).length());
    }
    let total = *s.last().unwrap();
    let place = |u: f32| -> (Vec2, Vec2, Vec2) {
        let i = s.partition_point(|&x| x < u).clamp(1, s.len() - 1);
        let f = ((u - s[i - 1]) / (s[i] - s[i - 1]).max(1e-4)).clamp(0.0, 1.0);
        let p = pts[i - 1].0.lerp(pts[i].0, f);
        let n = pts[i - 1].1.lerp(pts[i].1, f).normalize();
        let e = (pts[i].0 - pts[i - 1].0).normalize();
        (p, n, e)
    };
    // the top band's y: the upper deck's back row height, the band spanning up_top-3 .. up_top+1.4
    let px = 0.43;
    let text = "BALLPARK";
    let glyph = |c: char| -> [&'static str; 7] {
        match c {
            'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
            'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
            'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
            'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
            'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
            'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
            _ => ["....."; 7],
        }
    };
    let n_ch = text.chars().count() as f32;
    let width = (n_ch * 6.0 - 1.0) * px * 1.15;
    // centred east of the link building, as in the photos (bearing ~167)
    let centre = (0..=200).map(|k| total * k as f32 / 200.0).min_by(|&a, &b| {
        let ba = place(a).0.x.atan2(-place(a).0.y).to_degrees().rem_euclid(360.0);
        let bb = place(b).0.x.atan2(-place(b).0.y).to_degrees().rem_euclid(360.0);
        (ba - 167.0).abs().total_cmp(&(bb - 167.0).abs())
    }).unwrap_or(total / 2.0);
    let u0 = centre - width / 2.0;
    let top = GZ_LETTER_TOP;
    for (ci, ch) in text.chars().enumerate() {
        for (r, row) in glyph(ch).iter().enumerate() {
            let b = row.as_bytes();
            let mut c = 0;
            while c < 5 {
                if b[c] != b'#' {
                    c += 1;
                    continue;
                }
                let st = c;
                while c < 5 && b[c] == b'#' {
                    c += 1;
                }
                let ua = u0 + (ci as f32 * 6.0 + st as f32) * px * 1.15;
                let ub = u0 + (ci as f32 * 6.0 + c as f32) * px * 1.15;
                let (pa, na, _) = place(ua);
                let (pb, nb, _) = place(ub);
                let yb = top - (r + 1) as f32 * px;
                let e = (pb - pa).normalize();
                // red outline (the letters' returns) behind, white face in front, as on the building
                let (wa, wb) = (pa - e * 0.1 + na * 0.12, pb + e * 0.1 + nb * 0.12);
                g.poly(m.letters, &[v(wa, yb - 0.1), v(wb, yb - 0.1), v(wb, yb + px + 0.1), v(wa, yb + px + 0.1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                let (ra, rb) = (pa + na * 0.2, pb + nb * 0.2);
                g.poly(m.white, &[v(ra, yb), v(rb, yb), v(rb, yb + px), v(ra, yb + px)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            }
        }
    }
}

/// The footbridge from the terrace concourse out to the tower (satellite): its tower end on the tower's north face,
/// and a point toward the bowl along its axis (world); it starts where the axis crosses the frame.
const BRIDGE_TOWER: (f32, f32) = (-11.0, 103.4);
const BRIDGE_BOWL: (f32, f32) = (1.9, 73.4);
const BRIDGE_W: f32 = 4.4;

/// The bridge's plan (local), from well inside the frame out to the tower: what the frame, rails and paving keep
/// clear of.
fn bridge_ring() -> Vec<Vec2> {
    let (t, b) = (Vec2::new(BRIDGE_TOWER.0, BRIDGE_TOWER.1) - HOME, Vec2::new(BRIDGE_BOWL.0, BRIDGE_BOWL.1) - HOME);
    let e = (b - t).normalize();
    let n = e.perp() * (BRIDGE_W / 2.0 + 0.3);
    let a = b + e * 4.0;
    vec![t - n, a - n, a + n, t + n]
}

/// The tall tower at the link's far end (42 m with its mesh lantern), world plan from the lidar.
fn tower_ring() -> Vec<Vec2> {
    // turned about 32 degrees with the street, as the lidar and the map's building outline both show
    [(-15.0, 101.0), (0.0, 110.0), (-6.0, 121.0), (-21.0, 112.0)].iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect()
}

/// The lantern's footprint on the tower (map outline of its upper part; the lidar puts its top at 42 m).
fn lantern_ring() -> Vec<Vec2> {
    [(-11.0, 108.0), (-2.0, 113.0), (-6.0, 121.0), (-16.0, 115.0)].iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect()
}

/// A ring pushed out by `d` along its corner bisectors (convex rings).
fn grow(r: &[Vec2], d: f32) -> Vec<Vec2> {
    let c = r.iter().copied().sum::<Vec2>() / r.len() as f32;
    r.iter().map(|&p| p + (p - c).normalize() * d).collect()
}

/// The link building and the lantern tower: sandstone cladding with full-height glass slots, flat roofs.
fn home_towers(g: &mut Geo, m: &Mats) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let glass = Key::new(Mat::Glass, [0.32, 0.38, 0.42]);
    let mull = Key::new(Mat::Plain(90), [0.8, 0.8, 0.78]);
    let clad = |g: &mut Geo, ring: &[Vec2], h: f32, slots: &[(usize, f32)]| {
        let c = ring.iter().copied().sum::<Vec2>() / ring.len() as f32;
        // make the winding outward-facing whatever order the ring is in
        let area: f32 = (0..ring.len()).map(|i| ring[i].perp_dot(ring[(i + 1) % ring.len()])).sum();
        let pts: Vec<Vec2> = if area < 0.0 { ring.to_vec() } else { ring.iter().rev().copied().collect() };
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let e = (b - a).normalize();
            let mut out = Vec2::new(e.y, -e.x);
            if out.dot((a + b) / 2.0 - c) < 0.0 {
                out = -out;
            }
            wall_poly(g, m.sandstone, &[v(a, 0.0), v(b, 0.0), v(b, h), v(a, h)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            // parapet cap
            g.quad(m.concrete, v(a + out * 0.05, h), Vec3::new(b.x - a.x, 0.0, b.y - a.y), Vec3::new(-out.x, 0.0, -out.y) * 0.4);
            for &(side, at) in slots {
                if side != i {
                    continue;
                }
                // a recessed vertical glass slot 0.8 m wide with mullions every 1.5 m
                let len = (b - a).length();
                let p0 = a + e * (len * at - 0.4) + out * 0.03;
                g.quad(glass, v(p0, 1.5), Vec3::new(e.x, 0.0, e.y) * 0.8, Vec3::Y * (h - 3.0));
                let mut y = 1.5;
                while y <= h - 1.5 {
                    g.quad(mull, v(p0 + out * 0.01, y), Vec3::new(e.x, 0.0, e.y) * 0.8, Vec3::Y * 0.06);
                    y += 1.5;
                }
                for sx in [0.0, 0.76] {
                    g.quad(mull, v(p0 + e * sx + out * 0.01, 1.5), Vec3::new(e.x, 0.0, e.y) * 0.04, Vec3::Y * (h - 3.0));
                }
            }
        }
        poly_up(g, m.concrete, &pts.iter().map(|&p| v(p, h - 0.4)).collect::<Vec<_>>());
        g.block_ring(&pts, 0.0, h);
    };
    let tr = tower_ring();
    clad(g, &tr, 32.4, &[(1, 0.5), (2, 0.3)]);
    // the lantern: a dark steel mesh cage on the tower's upper part
    let mesh = Key::new(Mat::Plain(100), [0.16, 0.16, 0.15]).wear(wear::METAL);
    let (y0, y1) = (32.4, 42.0);
    let tr = lantern_ring();
    clad(g, &tr, 33.4, &[]);
    for i in 0..4 {
        let (a, b) = (tr[i], tr[(i + 1) % 4]);
        let len = (b - a).length();
        let n = (len / 0.7).round() as i32;
        for k in 0..=n {
            let p = a.lerp(b, k as f32 / n as f32);
            g.tube(mesh, v(p, y0), v(p, y1), 0.05, 4);
        }
        let mut y = y0;
        while y <= y1 + 0.01 {
            g.tube(mesh, v(a, y), v(b, y), 0.05, 4);
            y += 0.7;
        }
    }
    poly_up(g, mesh, &tr.iter().map(|&p| v(p, y1)).collect::<Vec<_>>());
}

// ---------------------------------------------------------------- home plate plaza

/// Palm planter rows across the home plate plaza, world (x, z) of their palms (the LiDAR trees the street dressing
/// plants), west to east. Each row is a long raised bed; the gate fence steps from row to row.
const PLAZA_ROWS: [&[(f32, f32)]; 6] = [
    &[(11.1, 108.8), (11.6, 116.3), (10.6, 125.3), (11.6, 131.3)],
    &[(17.1, 101.3), (18.6, 107.8), (21.6, 113.3), (22.6, 120.8)],
    &[(25.6, 94.3), (27.1, 100.3), (28.1, 106.3), (31.6, 111.3)],
    &[(31.6, 86.3), (33.6, 92.3), (38.1, 96.3), (38.6, 102.8)],
    &[(38.6, 79.8), (42.6, 84.3), (43.1, 90.3), (45.6, 97.3)],
    &[(45.6, 78.8), (50.6, 83.8), (51.6, 90.3)],
];

/// The fence's runs between rows (satellite): (row it leaves, z there, z where it reaches the next row) - it runs
/// east across the plaza, then north along the next row's bed to the next run.
const FENCE_RUNS: [(usize, f32, f32); 5] = [(0, 115.5, 113.5), (1, 109.0, 106.5), (2, 100.5, 98.0), (3, 91.5, 89.0), (4, 84.0, 82.5)];

/// World plan of the home plate plaza where the hand-laid paving replaces the city's ground, paths and lawns: from
/// under the bowl's back out to 10.5 m from the street's centreline, east of the tower's garden.
pub fn plaza_ring() -> Vec<[f64; 2]> {
    vec![[-1.0, 64.0], [60.0, 48.0], [95.0, 52.0], [93.7, 58.5], [29.7, 120.5], [12.0, 136.2], [2.0, 146.0], [-14.0, 137.0], [-20.0, 126.0], [-16.0, 118.0], [-3.0, 104.0]]
}

/// A planter row as a line x = a + b z (world), fitted through its palms, with its z extent.
fn plaza_row(i: usize) -> (f32, f32, f32, f32) {
    let r = PLAZA_ROWS[i];
    let n = r.len() as f32;
    let (mz, mx) = (r.iter().map(|p| p.1).sum::<f32>() / n, r.iter().map(|p| p.0).sum::<f32>() / n);
    let (szz, szx) = r.iter().fold((0.0, 0.0), |(a, b), p| (a + (p.1 - mz) * (p.1 - mz), b + (p.1 - mz) * (p.0 - mx)));
    let b = szx / szz.max(1e-3);
    let z0 = r.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let z1 = r.iter().map(|p| p.1).fold(f32::MIN, f32::max);
    (mx - b * mz, b, z0 - 3.0, z1 + 3.0)
}

/// The home plate plaza in front of the gate: pale broom-finished concrete slabs from the bowl to the street, a
/// grass strip at the curb, black bollards, the long raised palm beds, and the tall galvanised gate fence that
/// steps across the plaza from bed to bed with rolling gates (open on game days).
fn home_plaza(g: &mut Geo, f: &mut Furnisher, m: &Mats, city: &city::City, st: &[Station]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let w = |x: f32, z: f32| Vec2::new(x, z) - HOME;
    let terr = |p: Vec2| -> f32 {
        let q = p + HOME;
        city.terrain.at(q.x as f64, q.y as f64) as f32
    };
    // paving height: the terrain smoothed over ~8 m, never below the terrain itself
    let ground = |p: Vec2| -> f32 {
        let mut s = 0.0;
        for i in -2..=2 {
            for j in -2..=2 {
                s += terr(p + Vec2::new(i as f32 * 2.0, j as f32 * 2.0));
            }
        }
        // the terrain mesh is a 2 m grid with steps: clear its highest nearby point
        let mut hi = f32::MIN;
        for i in -1..=1 {
            for j in -1..=1 {
                hi = hi.max(terr(p + Vec2::new(i as f32 * 1.6, j as f32 * 1.6)));
            }
        }
        (s / 25.0).max(hi) + 0.04
    };
    let paving = Key::new(Mat::Surface("vlzlajnfw"), [0.64, 0.62, 0.58]).tile(3.0).rough(0.8);
    let joint = Key::new(Mat::Plain(200), [0.16, 0.155, 0.15]);
    let curb = Key::new(Mat::Surface("sg0tbcya"), [0.88, 0.85, 0.8]).tile(2.0).wear(wear::STONE);
    let grass = Key::new(Mat::Surface("sfklehwa"), [0.55, 0.78, 0.42]).tile(2.0).rough(0.85);
    let gravel = Key::new(Mat::Surface("ugzmbcrn"), [0.62, 0.5, 0.4]).tile(2.0);
    let galv = Key::new(Mat::Plain(150), [0.2, 0.21, 0.21]);
    let black = Key::new(Mat::Plain(120), [0.03, 0.03, 0.03]).wear(wear::METAL);

    // the street along the plaza's south-east side (centreline, 16 m wide)
    let road: Vec<Vec2> = city.lines.iter().filter(|l| matches!(l.kind, city::model::LineKind::Road)).flat_map(|l| {
        l.pts.windows(2).map(|s| (Vec2::new(s[0][0] as f32, s[0][1] as f32) - HOME, Vec2::new(s[1][0] as f32, s[1][1] as f32) - HOME, l.width as f32 * 0.5)).collect::<Vec<_>>()
    }).filter(|(a, b, _)| (a + HOME).distance(Vec2::new(30.0, 110.0)) < 120.0 || (b + HOME).distance(Vec2::new(30.0, 110.0)) < 120.0).flat_map(|(a, b, h)| [a, b, Vec2::new(h, 0.0)]).collect();
    let road_d = |p: Vec2| -> (f32, f32) {
        // (distance from the nearest centreline, that road's half width)
        let mut best = (f32::MAX, 8.0);
        for c in road.chunks(3) {
            let (a, b, h) = (c[0], c[1], c[2].x);
            let e = b - a;
            let t = ((p - a).dot(e) / e.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let d = (p - (a + e * t)).length();
            if d < best.0 {
                best = (d, h);
            }
        }
        best
    };
    let others: Vec<Vec<Vec2>> = city.buildings.iter().filter(|b| b.osm != OSM && !b.osm.starts_with(OSM)).map(|b| b.outer.iter().map(|p| Vec2::new(p[0] as f32, p[1] as f32) - HOME).collect()).collect();
    let lawns: Vec<Vec<Vec2>> = city.areas.iter().filter(|a| matches!(a.kind, city::model::AreaKind::Grass)).map(|a| a.outer.iter().map(|p| Vec2::new(p[0] as f32, p[1] as f32) - HOME).collect()).collect();
    // outside the bowl's street-level face: the arcade wall at the home gate, the concourse blocks elsewhere
    let outside_bowl = |p: Vec2| -> bool {
        let b = p.x.atan2(-p.y).to_degrees().rem_euclid(360.0);
        let s = st.iter().min_by(|a, c| (a.bearing - b).abs().total_cmp(&(c.bearing - b).abs())).unwrap();
        let up_back = s.upper + 0.3 + UP_ROWS * UP_PITCH;
        let face = if home_gate(s.bearing) { up_back - 4.0 } else { up_back + 4.2 };
        (p - s.p).dot(s.n) > face + 0.05
    };
    let tower = tower_ring();
    let beds: Vec<Vec<Vec2>> = (0..PLAZA_ROWS.len()).map(|i| {
        let (a, b, z0, z1) = plaza_row(i);
        let hw = if i == 5 { 1.8 } else { 0.9 };
        vec![w(a + b * z0 - hw, z0), w(a + b * z0 + hw, z0), w(a + b * z1 + hw, z1), w(a + b * z1 - hw, z1)]
    }).collect();

    // --- paving: 3 m slabs square to the street, each showing a different part of the scan; grass at the curb
    let ax = Vec2::new(0.714, -0.700).normalize();
    let az = Vec2::new(-ax.y, ax.x);
    let o = w(30.0, 110.0);
    let (cell, slab) = (1.5f32, 2i32);
    for i in -50i32..50 {
        for j in -40i32..40 {
            let at = |di: f32, dj: f32| o + ax * ((i as f32 + di) * cell) + az * ((j as f32 + dj) * cell);
            let c = at(0.5, 0.5);
            let cw = c + HOME;
            if cw.x < -24.0 || cw.x > 78.0 || cw.y < 56.0 || cw.y > 152.0 {
                continue;
            }
            let (rd, rh) = road_d(c);
            if rd < rh + 0.35 || !outside_bowl(c) || point_in(&tower, c) || beds.iter().any(|r| point_in(r, c)) {
                continue;
            }
            if others.iter().any(|r| point_in(r, c)) {
                continue;
            }
            // lawns inside the plaza (the tower's garden) stay grass; the plaza's own strip beds are the palm beds
            let lawn = lawns.iter().any(|r| point_in(r, c));
            let pts = [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)].map(|p| v(p, ground(p)));
            let (si, sj) = (i.div_euclid(slab), j.div_euclid(slab));
            let h = (si.wrapping_mul(73_856_093) ^ sj.wrapping_mul(19_349_663)) as u32;
            let uv = [(h % 7) as f32 * 3.0 + 0.0, ((h >> 4) % 5) as f32 * 3.0];
            let (k, uv) = if rd < rh + 2.3 || lawn { (grass, [0.0, 0.0]) } else { (paving, uv) };
            // joints on the slab grid: the texture's joint lies on its tile edge, so slabs start on tile edges
            g.poly_uv(k, &[pts[0], pts[3], pts[2], pts[1]], Vec3::new(ax.x, 0.0, ax.y), Vec3::new(-az.x, 0.0, -az.y), uv);
            // saw-cut joints along the slab edges
            if k == paving {
                let jw = 0.018;
                if i.rem_euclid(slab) == 0 {
                    let (p0, p1) = (at(0.0, 0.0), at(0.0, 1.0));
                    let (y0, y1) = (ground(p0) + 0.004, ground(p1) + 0.004);
                    g.poly(joint, &[v(p0 - ax * jw, y0), v(p0 + ax * jw, y0), v(p1 + ax * jw, y1), v(p1 - ax * jw, y1)].iter().rev().copied().collect::<Vec<_>>(), Vec3::new(ax.x, 0.0, ax.y), Vec3::new(-az.x, 0.0, -az.y));
                }
                if j.rem_euclid(slab) == 0 {
                    let (p0, p1) = (at(0.0, 0.0), at(1.0, 0.0));
                    let (y0, y1) = (ground(p0) + 0.004, ground(p1) + 0.004);
                    g.poly(joint, &[v(p0 - az * jw, y0), v(p1 - az * jw, y1), v(p1 + az * jw, y1), v(p0 + az * jw, y0)].iter().rev().copied().collect::<Vec<_>>(), Vec3::new(ax.x, 0.0, ax.y), Vec3::new(-az.x, 0.0, -az.y));
                }
            }
        }
    }

    // --- bollards along the outer walk, 3.5 m in from the grass
    let mut s = 0.0f32;
    for c in road.chunks(3) {
        let (a, b, h) = (c[0], c[1], c[2].x);
        let e = (b - a).normalize_or_zero();
        let mut nrm = Vec2::new(-e.y, e.x);
        if nrm.dot(w(20.0, 100.0) - a) < 0.0 {
            nrm = -nrm;
        }
        let len = (b - a).length();
        while s < len {
            let p = a + e * s + nrm * (h + 5.8);
            let pw = p + HOME;
            if pw.x > 2.0 && pw.x < 64.0 && pw.y > 70.0 && pw.y < 140.0 && outside_bowl(p) && road_d(p).0 > h + 5.0 {
                let y = ground(p);
                g.tube(black, v(p, y), v(p, y + 0.95), 0.1, 8);
                g.tube(black, v(p, y + 0.95), v(p, y + 1.0), 0.07, 8);
                g.block_box(Vec3::new(p.x - 0.1, 0.0, p.y - 0.1), Vec3::X * 0.2, Vec3::Z * 0.2, y, y + 1.0);
            }
            s += 2.4;
        }
        s -= len;
    }

    // --- palm beds: a raised concrete curb round decomposed granite, planted
    let plants: [(&str, f32); 5] = [("sf_cab98668fa", 0.85), ("sf_10ec8061cc", 1.3), ("wfypfa3ha", 0.6), ("shqke", 1.1), ("sf_8dd8f987f9", 0.42)];
    for (i, r) in beds.iter().enumerate() {
        let (_, _, z0, z1) = plaza_row(i);
        let y = r.iter().map(|&p| terr(p)).fold(f32::MIN, f32::max) + 0.03;
        let top = y + 0.42;
        // the walls go down to the lowest ground round the bed, so it never floats where the plaza falls away
        let foot = r.iter().map(|&p| terr(p)).fold(f32::MAX, f32::min) - 0.4;
        for k in 0..4 {
            let (a, b) = (r[k], r[(k + 1) % 4]);
            let e = (b - a).normalize();
            let out = Vec2::new(e.y, -e.x);
            let out = if out.dot(a - (r[0] + r[2]) / 2.0) < 0.0 { -out } else { out };
            g.poly(curb, &[v(a, foot), v(b, foot), v(b, top), v(a, top)], Vec3::new(e.x, 0.0, e.y) * if out.dot(Vec2::new(-e.y, e.x)) > 0.0 { -1.0 } else { 1.0 }, Vec3::Y);
            poly_up(g, curb, &[v(a, top), v(b, top), v(b - out * 0.18, top), v(a - out * 0.18, top)]);
        }
        poly_up(g, gravel, &r.iter().map(|&p| v(p, top - 0.1)).collect::<Vec<_>>());
        g.block_ring(r, foot, top);
        // plants down the bed, clear of the palms; flowers at the outer (south) end
        let (a, b, _, _) = plaza_row(i);
        let len = z1 - z0;
        let n = (len / 0.95) as i32;
        for k in 0..n {
            let z = z0 + 0.5 + k as f32 * len / n as f32;
            if PLAZA_ROWS[i].iter().any(|p| (p.1 - z).abs() < 0.8) {
                continue;
            }
            let side = if k % 2 == 0 { -0.35 } else { 0.35 };
            let p = w(a + b * z + side, z);
            let south = z > z1 - len * 0.3;
            let pick = if south { 4 } else { (f.rand() * 4.0) as usize % 4 };
            let (id, hgt) = plants[pick];
            if let Some((mn, mx)) = f.bounds(id) {
                let sc = hgt * (0.8 + 0.4 * f.rand()) / (mx[1] - mn[1]).max(1e-4);
                let ang = f.rand() * std::f32::consts::TAU;
                f.put_scaled(g, id, p, top - 0.1, Vec2::new(ang.cos(), ang.sin()), sc, false);
            }
        }
    }

    // --- the gate fence: west run along the tower's south side, then runs east and legs north up each bed
    let row_at = |i: usize, z: f32| -> Vec2 {
        let (a, b, _, _) = plaza_row(i);
        w(a + b * z + 0.5, z)
    };
    let mut path: Vec<(Vec2, Vec2, Option<(f32, f32)>)> = vec![];
    path.push((w(-11.0, 118.4), w(-11.0, 124.0), None));
    path.push((w(-11.0, 124.0), row_at(0, 124.0), Some((0.45, 0.62))));
    let mut prev = row_at(0, 124.0);
    for (k, &(ri, za, zb)) in FENCE_RUNS.iter().enumerate() {
        let (pa, pb) = (row_at(ri, za), row_at(ri + 1, zb));
        path.push((prev, pa, None));
        let open = if k == 4 { None } else { Some((0.35, 0.35 + 3.8 / (pb - pa).length())) };
        path.push((pa, pb, open));
        prev = pb;
    }
    path.push((prev, row_at(5, 77.5), None));
    for (a, b, open) in path {
        fence_run(g, galv, curb, a, b, open, &ground);
    }
}

/// One straight run of the gate fence from `a` to `b`: 3 m flat-bar pickets at 115 mm, square posts, two rails, on
/// a low concrete curb. `open` (fractions along the run) is a rolled-back gate: the gap, and its leaf parked
/// behind the fence on the plaza side.
fn fence_run(g: &mut Geo, galv: Key, curb: Key, a: Vec2, b: Vec2, open: Option<(f32, f32)>, ground: &dyn Fn(Vec2) -> f32) {
    // the runs across the plaza are all rolling-gate leaves (kick plates); the legs along the beds are fixed fence
    let leaves = open.is_some();
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (b - a).length();
    if len < 0.2 {
        return;
    }
    let e = (b - a) / len;
    let nrm = Vec2::new(-e.y, e.x);
    let h = 3.0;
    let pieces: Vec<(f32, f32, bool)> = match open {
        Some((o0, o1)) => vec![(0.0, o0, false), (o0, o1.min(1.0), true), (o1.min(1.0), 1.0, false)],
        None => vec![(0.0, 1.0, false)],
    };
    let panel = |g: &mut Geo, p: Vec2, q: Vec2, frame: bool| {
        let l = (q - p).length();
        if l < 0.05 {
            return;
        }
        let e = (q - p) / l;
        let n = Vec2::new(-e.y, e.x);
        let y0 = ground(p).min(ground(q));
        // curb
        let c = 0.13;
        g.poly(curb, &[v(p - n * c, y0 - 0.2), v(q - n * c, y0 - 0.2), v(q - n * c, y0 + 0.12), v(p - n * c, y0 + 0.12)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        g.poly(curb, &[v(q + n * c, y0 - 0.2), v(p + n * c, y0 - 0.2), v(p + n * c, y0 + 0.12), v(q + n * c, y0 + 0.12)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        poly_up(g, curb, &[v(p - n * c, y0 + 0.12), v(q - n * c, y0 + 0.12), v(q + n * c, y0 + 0.12), v(p + n * c, y0 + 0.12)]);
        let base = y0 + 0.12;
        // pickets: 50 x 12 mm flat bars
        let count = (l / 0.115).floor() as i32;
        let pad = (l - count as f32 * 0.115) / 2.0;
        for k in 0..=count {
            let s = pad + k as f32 * 0.115;
            let pc = p + e * s;
            let (hw, hd) = (0.025, 0.007);
            let c4 = [pc - e * hw - n * hd, pc + e * hw - n * hd, pc + e * hw + n * hd, pc - e * hw + n * hd];
            for i in 0..4 {
                let (u, t) = (c4[i], c4[(i + 1) % 4]);
                let d = (t - u).normalize();
                g.poly(galv, &[v(u, base), v(t, base), v(t, base + h - 0.1), v(u, base + h - 0.1)], Vec3::new(d.x, 0.0, d.y), Vec3::Y);
            }
            poly_up(g, galv, &c4.map(|q| v(q, base + h - 0.1)));
        }
        // rails: rectangular tubes on the plaza side, low and high
        for (y, t) in [(0.3, 0.06), (h - 0.55, 0.06)] {
            let (r0, r1) = (n * 0.008, n * 0.06);
            g.poly(galv, &[v(p + r1, base + y), v(q + r1, base + y), v(q + r1, base + y + t), v(p + r1, base + y + t)], Vec3::new(e.x, 0.0, e.y) * -1.0, Vec3::Y);
            g.poly(galv, &[v(q - r1, base + y), v(p - r1, base + y), v(p - r1, base + y + t), v(q - r1, base + y + t)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            poly_up(g, galv, &[v(p - r1, base + y + t), v(q - r1, base + y + t), v(q + r1, base + y + t), v(p + r1, base + y + t)]);
            let _ = r0;
        }
        if frame || leaves {
            // a gate leaf: heavy tube frame and a solid kick plate
            for (u, t) in [(p, q)] {
                g.tube(galv, v(u, base + 0.05), v(t, base + 0.05), 0.05, 4);
                g.tube(galv, v(u, base + h - 0.15), v(t, base + h - 0.15), 0.05, 4);
            }
            for u in [p, q] {
                g.tube(galv, v(u, base), v(u, base + h - 0.1), 0.05, 4);
            }
            for side in [-1.0f32, 1.0] {
                let o = n * 0.03 * side;
                g.quad(galv, v(if side > 0.0 { q + o } else { p + o }, base + 0.08), Vec3::new(e.x, 0.0, e.y) * (if side > 0.0 { -l } else { l }), Vec3::Y * 0.32);
            }
        }
        g.block_ring(&[p - n * 0.1, q - n * 0.1, q + n * 0.1, p + n * 0.1], y0 - 0.2, base + h);
    };
    for (f0, f1, gap) in pieces {
        let (p, q) = (a + e * (len * f0), a + e * (len * f1));
        if gap {
            // the leaf rolled back behind the next fixed stretch, on the plaza side (north)
            let back = if nrm.y < 0.0 { nrm } else { -nrm };
            let l = (q - p).length();
            let (lp, lq) = (q + e * 0.1 + back * 0.35, q + e * (l + 0.1) + back * 0.35);
            if f1 < 0.999 {
                panel(g, lp, lq, true);
            }
            // gate posts and the floor track across the opening
            let y = ground(p).min(ground(q));
            g.quad(galv, v(p - nrm * 0.04, y + 0.005), Vec3::new(e.x, 0.0, e.y) * l, Vec3::new(nrm.x, 0.0, nrm.y) * 0.08);
            continue;
        }
        panel(g, p, q, false);
        // square posts every 3 m and at the ends
        let l = (q - p).length();
        let np = (l / 3.0).ceil().max(1.0) as i32;
        for k in 0..=np {
            let pc = p + e * (l * k as f32 / np as f32);
            let y = ground(pc);
            g.cuboid(galv, Vec3::new(pc.x - 0.065, y, pc.y - 0.065), Vec3::new(pc.x + 0.065, y + 3.25, pc.y + 0.065));
        }
    }
}

// ---------------------------------------------------------------- sandstone cladding

/// Panel tones of the cladding, measured on the tower in the photos: k-means of panel colours relative to the wall's
/// mean (linear), with how often each occurs. The photo is hazy, so `wall_poly` widens the hue spread a little.
const ASHLAR_TONES: [([f32; 3], u32); 7] = [
    ([0.935, 0.897, 0.867], 119),
    ([1.054, 1.044, 0.984], 114),
    ([1.145, 1.197, 1.130], 86),
    ([1.041, 1.070, 1.150], 81),
    ([0.825, 0.758, 0.741], 79),
    ([0.925, 0.933, 1.052], 77),
    ([1.187, 1.276, 1.323], 41),
];
/// Cladding courses, as on the building (and in the scan).
const COURSE: f32 = 0.6;

/// The scanned panels' windows in the colour-neutral sandstone (metres in its 3 m tile), written by `fps proctex`.
/// Slivers the scan's joints split off are left out.
fn ashlar_windows() -> &'static [[f32; 4]] {
    static W: std::sync::OnceLock<Vec<[f32; 4]>> = std::sync::OnceLock::new();
    W.get_or_init(|| {
        std::fs::read_to_string(crate::root().join("data/megascans_inbox/procedural/pt_ashlar_panels.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<[f32; 4]>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|w| w[2] - w[0] > 0.5)
            .collect()
    })
}

fn mix(a: u64, b: u64, c: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ c.wrapping_mul(0x1656_67B1_9E37_79F9);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 29)
}

/// Course heights of the cladding, bottom up, repeating: measured on a rectified street photo of the east
/// building's face (joints at 0.30, 0.45-0.47 and 0.57-0.60 m spacings in an irregular sequence).
const COURSES: [f32; 12] = [0.45, 0.6, 0.3, 0.45, 0.3, 0.6, 0.6, 0.3, 0.6, 0.3, 0.45, 0.3];

/// The cladding's overall colour (linear albedo), set against sunlit and shaded photos.
const STONE_TONE: [f32; 3] = [0.95, 0.7, 0.43];

/// A wall face. Sandstone is laid stone by stone as on the building: uneven courses on a grid shared by every wall,
/// stones of varying length (some tall ones split into two), each a different piece of a scanned cleft sandstone
/// face (its own grain, normals and roughness; sometimes turned over) with its own tone, set in recessed dark
/// joints. Anything else, or a face that is not upright, is a plain polygon.
fn wall_poly(g: &mut Geo, k: Key, pts: &[Vec3], right: Vec3, up: Vec3) {
    if k != mats().sandstone || up.y < 0.99 || right.y.abs() > 0.01 {
        g.poly(k, pts, right, up);
        return;
    }
    let right = Vec3::new(right.x, 0.0, right.z).normalize();
    let n = right.cross(Vec3::Y);
    let d = pts.iter().map(|p| p.dot(n)).sum::<f32>() / pts.len() as f32;
    let q: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.dot(right), p.y)).collect();
    let area: f32 = (0..q.len()).map(|i| q[i].perp_dot(q[(i + 1) % q.len()])).sum();
    let ccw = area > 0.0;
    let (umin, umax) = q.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (ymin, ymax) = q.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    // the joints: a dark backing just behind the stones
    let joint = Key::new(Mat::Plain(230), [0.24, 0.18, 0.125]);
    let back: Vec<Vec3> = pts.iter().map(|&p| p - n * 0.012).collect();
    g.poly(joint, &back, right, Vec3::Y);
    // one bond per wall plane
    let seed = mix((right.x * 997.0).round() as i64 as u64, (right.z * 997.0).round() as i64 as u64, (d * 7.0).round() as i64 as u64);
    let rnd = |a: i64, b: i64, c: u64| (mix(seed, a as u64, b as u64 ^ c.wrapping_mul(0x9E37)) % 100_000) as f32 / 100_000.0;
    let total: u32 = ASHLAR_TONES.iter().map(|t| t.1).sum();
    let cycle: f32 = COURSES.iter().sum();
    let gap = 0.007;
    let mut emit = |g: &mut Geo, a: f32, b: f32, c0: f32, c1: f32, id: (i64, i64, u64)| {
        // the stone's rectangle, less the joint, clipped to the face
        let mut poly = vec![Vec2::new(a + gap, c0 + gap), Vec2::new(b - gap, c0 + gap), Vec2::new(b - gap, c1 - gap), Vec2::new(a + gap, c1 - gap)];
        for i in 0..q.len() {
            let (e0, e1) = if ccw { (q[i], q[(i + 1) % q.len()]) } else { (q[(i + 1) % q.len()], q[i]) };
            let inside = |p: Vec2| (e1 - e0).perp_dot(p - e0) >= -1e-5;
            let mut out = vec![];
            for m in 0..poly.len() {
                let (p0, p1) = (poly[m], poly[(m + 1) % poly.len()]);
                let (i0, i1) = (inside(p0), inside(p1));
                if i0 {
                    out.push(p0);
                }
                if i0 != i1 {
                    let (s0, s1) = ((e1 - e0).perp_dot(p0 - e0), (e1 - e0).perp_dot(p1 - e0));
                    out.push(p0 + (p1 - p0) * (s0 / (s0 - s1)));
                }
            }
            poly = out;
            if poly.len() < 3 {
                return;
            }
        }
        let h = mix(seed ^ id.2, id.0 as u64, id.1 as u64);
        let turned = h & 1 == 1;
        let mut pick = (mix(h, 11, 13) % total as u64) as u32;
        let mut ti = 0;
        for (i, t) in ASHLAR_TONES.iter().enumerate() {
            if pick < t.1 {
                ti = i;
                break;
            }
            pick -= t.1;
        }
        let t = ASHLAR_TONES[ti].0;
        let l = (t[0] + t[1] + t[2]) / 3.0;
        // weathering: the bottom course a little darker
        let dim = if c0 < 0.5 { 0.9 } else { 1.0 };
        // stone-to-stone spread as measured on the sunlit face: about 6-10% in brightness, a little in hue
        let lv = 1.0 + (l - 1.0) * 0.6;
        let tint = [0, 1, 2].map(|c| (lv + (t[c] - l) * 0.35) * dim * STONE_TONE[c]);
        let key = Key::new(Mat::Surface("pt_cleft_sandstone"), tint).tile(2.0).rough(1.15 + 0.04 * ti as f32).wear(wear::STONE);
        // a piece of the 2 m scan at true scale, anywhere in it
        let (ou, ov) = ((mix(h, 3, 5) % 2000) as f32 / 1000.0, (mix(h, 5, 7) % 2000) as f32 / 1000.0);
        let uvs: Vec<[f32; 2]> = poly
            .iter()
            .map(|p| {
                let (mut s, mut tt) = (p.x - a, p.y - c0);
                if turned {
                    s = (b - a) - s;
                    tt = (c1 - c0) - tt;
                }
                [ou + s, ov - tt]
            })
            .collect();
        let p3: Vec<Vec3> = poly.iter().map(|p| right * p.x + Vec3::Y * p.y + n * d).collect();
        if ccw {
            g.poly_mapped(key, &p3, &uvs, right, Vec3::Y, turned);
        } else {
            let (r3, ruv): (Vec<Vec3>, Vec<[f32; 2]>) = (p3.iter().rev().copied().collect(), uvs.iter().rev().copied().collect());
            g.poly_mapped(key, &r3, &ruv, right, Vec3::Y, turned);
        }
    };
    // courses by absolute height, so they line up round every corner
    let mut kc = (ymin / cycle).floor() as i64 * COURSES.len() as i64;
    let mut c0 = (ymin / cycle).floor() * cycle;
    while c0 < ymax {
        let hgt = COURSES[kc.rem_euclid(COURSES.len() as i64) as usize];
        let c1 = c0 + hgt;
        if c1 > ymin + 1e-3 {
            // stone lengths: longer in the tall courses; cut on a jittered grid so both ends of a wall agree
            let cell = if hgt > 0.55 { 1.15 } else if hgt > 0.4 { 0.95 } else { 0.8 };
            let off = rnd(kc, -1, 1) * cell;
            let edge = |j: i64| off + j as f32 * cell + (rnd(kc, j, 2) - 0.5) * cell * 0.55;
            let (j0, j1) = (((umin - off) / cell).floor() as i64 - 1, ((umax - off) / cell).ceil() as i64 + 1);
            for j in j0..j1 {
                let (a, b) = (edge(j), edge(j + 1));
                if b <= umin || a >= umax {
                    continue;
                }
                // now and then a tall stone is two stacked
                if hgt > 0.55 && rnd(kc, j, 3) < 0.16 {
                    let mid = c0 + hgt * (0.42 + 0.16 * rnd(kc, j, 4));
                    emit(g, a, b, c0, mid, (kc, j, 1));
                    emit(g, a, b, mid, c1, (kc, j, 2));
                } else {
                    emit(g, a, b, c0, c1, (kc, j, 0));
                }
            }
        }
        c0 = c1;
        kc += 1;
    }
}

/// The home plate gate's exposed white steel frame, member for member as in the photos. Square columns a bay apart
/// (one between the words of the lettering). At every level a V springs from each column's foot up to the truss
/// under the deck above. Six-panel trusses run under each deck, behind the lettering and under the roof, their
/// diagonals leaning in to the bay's middle (/ / / \ \ \), with verticals between; a shallow zig-zag runs under the
/// letter truss. All members are rectangular sections. `line`: the frame's plane, (point, outward normal) by station.
fn gate_frame(g: &mut Geo, m: &Mats, line: &[(Vec2, Vec2)], cores: &[Core]) {
    let mut pts: Vec<(Vec2, Vec2)> = vec![];
    for &q in line {
        if pts.last().map_or(true, |l: &(Vec2, Vec2)| (q.0 - l.0).length() > 0.05) {
            pts.push(q);
        }
    }
    if pts.len() < 2 {
        return;
    }
    let mut s = vec![0.0f32];
    for w in pts.windows(2) {
        s.push(s.last().unwrap() + (w[1].0 - w[0].0).length());
    }
    let total = *s.last().unwrap();
    let place = |u: f32| -> (Vec2, Vec2) {
        let i = s.partition_point(|&x| x < u).clamp(1, s.len() - 1);
        let f = ((u - s[i - 1]) / (s[i] - s[i - 1]).max(1e-4)).clamp(0.0, 1.0);
        (pts[i - 1].0.lerp(pts[i].0, f), pts[i - 1].1.lerp(pts[i].1, f).normalize())
    };
    let bearing = |p: Vec2| p.x.atan2(-p.y).to_degrees().rem_euclid(360.0);
    // a column at the lettering's centre, then every bay either way
    let centre = (0..=400).map(|k| total * k as f32 / 400.0).min_by(|&a, &b| (bearing(place(a).0) - 167.0).abs().total_cmp(&(bearing(place(b).0) - 167.0).abs())).unwrap();
    let mut cols: Vec<f32> = vec![];
    let mut u = centre - (centre / GZ_BAY).floor() * GZ_BAY;
    while u <= total + 1e-3 {
        cols.push(u);
        u += GZ_BAY;
    }
    // the ends of the run take a column too, so the frame closes there
    if cols.first().map_or(true, |&c| c > 2.0) {
        cols.insert(0, 0.0);
    }
    if cols.last().map_or(true, |&c| c < total - 2.0) {
        cols.push(total);
    }
    if std::env::var("FPS_GZ_DEBUG").is_ok() {
        let w: Vec<String> = cols.iter().map(|&c| { let (p, n) = place(c); format!("({:.0},{:.0} n {:.2},{:.2})", p.x + HOME.x, p.y + HOME.y, n.x, n.y) }).collect();
        eprintln!("frame cols: {}", w.join(" "));
    }
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let k = m.white;
    let blocked = |p: Vec2| cores.iter().any(|c| c.covers(p));
    let d1 = TERR_Y + 9.0 * TERR_RISE + 0.25;
    let top_row = UP_Y + (UP_ROWS - 1.0) * UP_RISE;
    let roof = 39.2;
    // truss bands (bottom chord, top chord, diagonal pattern: true = six panels leaning in with verticals, false = a
    // twelve-panel zig-zag) and V levels (foot, head, reach of each arm as a fraction of the bay)
    // (bottom chord, top chord, panels whose diagonal is a big arm running through the band)
    // (bottom chord, top chord, panels whose diagonal rises from a big arm's head: a heavier member, as in the photos)
    let bands: [(f32, f32, &[usize]); 4] = [(13.0, d1 - 1.35, &[2, 3]), (18.9, GZ_UPPER_CONCOURSE - 1.4, &[1, 4]), (31.5, 33.4, &[2, 3]), (top_row + 1.15, roof, &[])];
    // the big arms, left half of the bay (mirrored for the right): (fraction, height) to (fraction, height), section.
    // From the photos: at the bottom, arms from near the bay's middle under the terrace deck down through the truss
    // to the street-level deck's edge by the columns, crossed by legs from each column's foot of the truss; under
    // the upper concourse, arms from the terrace deck by the column up through the truss to the fascia; the tall
    // bay's from the upper concourse by the column to the letter truss. Where an arm crosses a truss it is that
    // panel's diagonal.
    let mid_lo = 18.9;
    // each reaches the floor of the level below; steep (about 60 degrees), not 45
    // every arm hangs from the bottom chord of the truss above its level down to that level's floor; the trusses
    // keep their own diagonals
    // at the bottom a leg from each column's top corner runs down to meet the big arm (a Y, not a cross)
    let big = ((2.0f32 / 6.0, 13.0f32), (0.0f32, CONCOURSE_Y));
    let leg_dir = (0.36f32, CONCOURSE_Y - 13.0);
    let leg_end = {
        // solve (0, 13) + t * leg_dir = big.0 + s * (big.1 - big.0)
        let (ax, ay) = (big.1 .0 - big.0 .0, big.1 .1 - big.0 .1);
        let den = leg_dir.0 * ay - leg_dir.1 * ax;
        let t = ((big.0 .0) * ay - (big.0 .1 - 13.0) * ax) / den;
        (leg_dir.0 * t, 13.0 + leg_dir.1 * t)
    };
    let arms = [
        (big.0 .0, big.0 .1, big.1 .0, big.1 .1, 0.6),
        (0.0, 13.0, leg_end.0, leg_end.1, 0.42),
        (1.0 / 6.0, mid_lo, 0.0, d1, 0.5),
        (2.0 / 6.0, 31.5, 0.0, GZ_UPPER_CONCOURSE, 0.55),
    ];
    for w in cols.windows(2) {
        let ((pa, na), (pb, nb)) = (place(w[0]), place(w[1]));
        if (pb - pa).length() < 1.0 {
            continue;
        }
        let nrm = (na + nb).normalize();
        let dn = Vec3::new(nrm.x, 0.0, nrm.y);
        let at = |f: f32| pa.lerp(pb, f);
        let br = bridge_ring();
        let mut member = |g: &mut Geo, p: Vec2, yp: f32, q: Vec2, yq: f32, wdt: f32, dep: f32| {
            if blocked(p) || blocked(q) || blocked((p + q) / 2.0) {
                return;
            }
            // the bridge's passage through the frame (deck to its top beams) stays clear: members are cut back to
            // its sides, not dropped
            let inside = |t: f32| {
                let y = yp + (yq - yp) * t;
                point_in(&br, p.lerp(q, t)) && y > d1 - 0.4 && y < d1 + 3.9
            };
            const N: usize = 48;
            let mut t0: Option<f32> = None;
            for i in 0..=N {
                let t = i as f32 / N as f32;
                let out = i < N && !inside((t + 0.5 / N as f32).min(1.0));
                match (t0, out) {
                    (None, true) => t0 = Some(t),
                    (Some(a0), false) => {
                        let (pa0, pa1) = (p.lerp(q, a0), p.lerp(q, t));
                        let (ya0, ya1) = (yp + (yq - yp) * a0, yp + (yq - yp) * t);
                        g.member(k, v(pa0, ya0), v(pa1, ya1), wdt, dep, dn);
                        t0 = None;
                    }
                    _ => {}
                }
            }
        };
        for &(y0, y1, arm_panels) in &bands {
            // chords in front of the deck fascias
            member(g, pa, y0, pb, y0, 0.45, 0.4);
            member(g, pa, y1, pb, y1, 0.45, 0.4);
            for i in 0..6 {
                let (f0, f1) = (i as f32 / 6.0, (i + 1) as f32 / 6.0);
                if i > 0 {
                    member(g, at(f0), y0, at(f0), y1, 0.26, 0.26);
                }
                let sz = if arm_panels.contains(&i) { 0.46 } else { 0.26 };
                if i < 3 {
                    member(g, at(f0), y0, at(f1), y1, sz, sz);
                } else {
                    member(g, at(f0), y1, at(f1), y0, sz, sz);
                }
            }
        }
        for &(f0, y0, f1, y1, size) in &arms {
            member(g, at(f0), y0, at(f1), y1, size, size * 0.9);
            member(g, at(1.0 - f0), y0, at(1.0 - f1), y1, size, size * 0.9);
        }
        // the top rows overhang the frame: their walk out to a white box edge with the rail, carried on angled struts
        // from the letter truss's foot at every panel point, with outriggers under the box
        let (fa, fb) = (pa + na * GZ_OVERHANG, pb + nb * GZ_OVERHANG);
        let (ia, ib) = (pa - na * 0.55, pb - nb * 0.55);
        let under = GZ_BOX_BOTTOM;
        // posts from the letter truss up to the box (none at the bay's middle, where the truss's diagonals meet),
        // and the box's bottom chord
        for f in [1.0 / 6.0, 2.0 / 6.0, 4.0 / 6.0, 5.0 / 6.0] {
            member(g, at(f), 33.4, at(f), under, 0.3, 0.3);
        }
        member(g, pa, under, pb, under, 0.4, 0.4);

        // (cut round a stair core standing in the frame: its landing takes over there)
        for (f0, f1, hole) in runs(|f| blocked(pa.lerp(pb, f)) || blocked(fa.lerp(fb, f))) {
            if hole {
                continue;
            }
            let (ia, ib, fa, fb) = (ia.lerp(ib, f0), ia.lerp(ib, f1), fa.lerp(fb, f0), fa.lerp(fb, f1));
            let ev = Vec3::new((pb - pa).normalize().x, 0.0, (pb - pa).normalize().y);
            poly_up(g, m.concrete, &[v(ia, top_row), v(ib, top_row), v(fb, top_row), v(fa, top_row)]);
            g.poly(k, &[v(fa, under), v(fb, under), v(fb, top_row + 0.15), v(fa, top_row + 0.15)], ev, Vec3::Y);
            g.poly(m.soffit, &[v(fb, under), v(fa, under), v(ia, under), v(ib, under)], ev, Vec3::new(-nrm.x, 0.0, -nrm.y));
            g.block_ring(&[ia, ib, fb, fa], under, top_row);
            for hh in [0.5, 1.05] {
                g.tube(m.rail, v(fa, top_row + hh), v(fb, top_row + hh), 0.025, 5);
            }
            g.block_ring(&[fa - nrm * 0.1, fb - nrm * 0.1, fb, fa], top_row, top_row + 1.05);
        }
        for i in 0..=6 {
            let f = i as f32 / 6.0;
            let p = at(f);
            let side = Vec3::new((pb - pa).normalize().x, 0.0, (pb - pa).normalize().y);
            if !blocked(p) {
                let front = fa.lerp(fb, f);
                g.member(k, v(p, 33.4), v(front, under), 0.28, 0.26, side);
                g.member(k, v(p - nrm * 0.4, under - 0.2), v(front, under - 0.2), 0.3, 0.26, side);
            }
        }
        // under the upper deck, against the rows' stepped underside: a raker truss at each tall column (chords down
        // the rake from the top rows' back to the upper concourse, Warren webs between, a post at the back), and in
        // each bay a horizontal across the underside with knee braces into the rakers at its ends and a V from the
        // top rows down to its middle. r: metres in from the upper deck's back (the frame stands FRAME_OFF out).
        let side = Vec3::new((pb - pa).normalize().x, 0.0, (pb - pa).normalize().y);
        let under_rows = |r: f32| top_row + 0.01 - r * UP_RISE / UP_PITCH;
        let pt = |f: f32, r: f32| (pa - na * (FRAME_OFF + r)).lerp(pb - nb * (FRAME_OFF + r), f);
        let r_end = GZ_RAKE * 0.62;
        let depth = 2.3;
        for f in [0.0f32, 1.0] {
            if blocked(at(f)) {
                continue;
            }
            // each column's raker is drawn by the bay on its right, and the last column's by the last bay
            if f == 1.0 && w[1] < *cols.last().unwrap() - 1e-3 {
                continue;
            }
            let top = |r: f32| under_rows(r) - 0.22;
            g.member(k, v(pt(f, 0.0), top(0.0)), v(pt(f, r_end), top(r_end)), 0.4, 0.4, side);
            g.member(k, v(pt(f, 0.0), top(0.0) - depth), v(pt(f, r_end), top(r_end) - depth), 0.4, 0.4, side);
            g.member(k, v(pt(f, 0.0), top(0.0) - depth), v(pt(f, 0.0), top(0.0)), 0.3, 0.3, side);
            // and back to the frame column at both chords
            for y in [top(0.0), top(0.0) - depth] {
                g.member(k, v(pt(f, 0.0), y), v(at(f), y), 0.4, 0.4, side);
            }
            let n = 6;
            for j in 0..n {
                let (r0, r1) = (r_end * j as f32 / n as f32, r_end * (j + 1) as f32 / n as f32);
                let (y0, y1) = if j % 2 == 0 { (top(r0), top(r1) - depth) } else { (top(r0) - depth, top(r1)) };
                g.member(k, v(pt(f, r0), y0), v(pt(f, r1), y1), 0.26, 0.26, side);
            }
        }
        {
            // the V: parallel to the bowl, in the plane of the raker trusses' bottom chords (2.3 m under the rows'
            // stepped underside): 45 degree arms (seen from the street) from near each raker down to a gusset at
            // mid-bay, sitting on a horizontal that runs raker to raker
            let plane = |r: f32| under_rows(r) - 0.22 - depth;
            let rake_dir = (Vec3::new(-nrm.x, 0.0, -nrm.y) + Vec3::new(0.0, -UP_RISE / UP_PITCH, 0.0)).normalize();
            let pn = side.cross(rake_dir).normalize();
            // the horizontal runs raker to raker about mid-height of the tall bay (seen from the street); the arms
            // rise from its gusset at 45 degrees to a tie along the top of the plane, just inside each raker
            let r_apex = 8.5;
            let r_top = 0.6;
            let drop = (r_apex - r_top) * UP_RISE / UP_PITCH;
            let reach = (drop / (pb - pa).length()).min(0.5);
            for (f0, r0, f1, r1, sz) in [(0.0, r_apex, 1.0, r_apex, 0.42), (0.0, r_top, 1.0, r_top, 0.36), (0.5 - reach, r_top, 0.5, r_apex, 0.4), (0.5 + reach, r_top, 0.5, r_apex, 0.4)] {
                let (p, q) = (pt(f0, r0), pt(f1, r1));
                if blocked(p) || blocked(q) {
                    continue;
                }
                g.member(k, v(p, plane(r0)), v(q, plane(r1)), sz, sz * 0.9, pn);
            }
            let c = pt(0.5, r_apex);
            if !blocked(c) {
                let up_rake = -rake_dir;
                let cp = v(c, plane(r_apex));
                g.member(k, cp - up_rake * 0.1, cp + up_rake * 0.9, 1.0, 0.5, pn);
            }
        }
        // the roof's white edge over the frame
        let ov = GZ_OVERHANG + 0.7;
        g.poly(k, &[v(pa + na * ov, roof), v(pb + nb * ov, roof), v(pb + nb * ov, roof + 1.3), v(pa + na * ov, roof + 1.3)], Vec3::new((pb - pa).normalize().x, 0.0, (pb - pa).normalize().y), Vec3::Y);
        poly_up(g, m.roof, &[v(pa + na * ov, roof + 1.3), v(pb + nb * ov, roof + 1.3), v(pb - nb * 4.0, roof + 1.3), v(pa - na * 4.0, roof + 1.3)]);
        let (r0, r1) = (pa + na * ov, pb + nb * ov);
        let (i0, i1) = (pa - na * 4.0, pb - nb * 4.0);
        g.poly(m.roof, &[v(i0, roof), v(i1, roof), v(r1, roof), v(r0, roof)], Vec3::new((pb - pa).normalize().x, 0.0, (pb - pa).normalize().y), Vec3::new(-nrm.x, 0.0, -nrm.y));
    }
    // the columns, full height from the street to the roof
    for &c in &cols {
        let (p, n) = place(c);
        if blocked(p) {
            continue;
        }
        let dn = Vec3::new(n.x, 0.0, n.y);
        g.member(k, v(p, 0.0), v(p, roof + 1.3), 0.8, 1.3, dn);
        let e = n.perp() * 0.4;
        g.block_ring(&[p - e - n * 0.65, p + e - n * 0.65, p + e + n * 0.65, p - e + n * 0.65], 0.0, roof);
    }
}

/// The footbridge from the terrace concourse to the tower, as in the photos: a concrete deck on white edge girders,
/// dark steel posts every 1.8 m with white X-bracing between them, glass guards, a pergola of cross beams over the
/// top, and planters along both top edges spilling red flowers. `frame`: the gate frame's line, where it starts.
fn footbridge(g: &mut Geo, m: &Mats, frame: &[(Vec2, Vec2)]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let t = Vec2::new(BRIDGE_TOWER.0, BRIDGE_TOWER.1) - HOME;
    let b = Vec2::new(BRIDGE_BOWL.0, BRIDGE_BOWL.1) - HOME;
    let e = (b - t).normalize();
    // where the axis crosses the frame line; the deck starts just inside it, on the terrace concourse's edge
    let mut start = b;
    for w in frame.windows(2) {
        let (p, q) = (w[0].0, w[1].0);
        let d = q - p;
        let den = e.perp_dot(d);
        if den.abs() < 1e-5 {
            continue;
        }
        let s = (p - t).perp_dot(d) / den;
        let u = (p - t).perp_dot(e) / den;
        if (0.0..=1.0).contains(&u) && s > 0.0 {
            // lap 1.5 m onto the terrace concourse's deck inside the frame
            start = t + e * (s + FRAME_OFF + 1.5);
        }
    }
    let (a, z) = (start, t);
    let len = (z - a).length();
    if len < 2.0 {
        return;
    }
    let dir = (z - a) / len;
    let side = dir.perp();
    let ev = Vec3::new(dir.x, 0.0, dir.y);
    let sv = Vec3::new(side.x, 0.0, side.y);
    let deck = TERR_Y + 9.0 * TERR_RISE + 0.25;
    let h = 3.4;
    let hw = BRIDGE_W / 2.0;
    let dark = Key::new(Mat::Plain(140), [0.07, 0.075, 0.08]);
    let glass = Key::new(Mat::Glass, [0.55, 0.62, 0.64]);
    // deck slab, soffit and white edge girders
    let q = [a - side * hw, z - side * hw, z + side * hw, a + side * hw];
    poly_up(g, m.concrete, &q.map(|p| v(p, deck)));
    g.poly(m.soffit, &[v(q[3], deck - 0.35), v(q[2], deck - 0.35), v(q[1], deck - 0.35), v(q[0], deck - 0.35)], ev, -sv);
    g.block_ring(&q, deck - 0.35, deck);
    for sg in [-1.0f32, 1.0] {
        let o = side * (sg * (hw - 0.15));
        g.member(m.white, v(a + o, deck - 0.8), v(z + o, deck - 0.8), 0.9, 0.35, sv);
    }
    // the side frames
    let n = (len / 1.8).round().max(1.0) as i32;
    for sg in [-1.0f32, 1.0] {
        let o = side * (sg * hw);
        let at = |i: i32| a.lerp(z, i as f32 / n as f32) + o;
        for i in 0..=n {
            g.member(dark, v(at(i), deck), v(at(i), deck + h), 0.18, 0.18, sv);
        }
        for i in 0..n {
            g.member(m.white, v(at(i), deck + 0.15), v(at(i + 1), deck + h - 0.15), 0.12, 0.1, sv);
            g.member(m.white, v(at(i + 1), deck + 0.15), v(at(i), deck + h - 0.15), 0.12, 0.1, sv);
        }
        for y in [deck + 0.05, deck + h] {
            g.member(m.white, v(a + o, y), v(z + o, y), 0.28, 0.22, sv);
        }
        // glass guard inside the frame
        let gi = side * (sg * (hw - 0.12));
        let (g0, g1) = (a + gi, z + gi);
        let face = if sg > 0.0 { -ev } else { ev };
        let (p0, p1) = if sg > 0.0 { (g1, g0) } else { (g0, g1) };
        g.poly(glass, &[v(p0, deck), v(p1, deck), v(p1, deck + 1.1), v(p0, deck + 1.1)], face, Vec3::Y);
        g.member(m.rail, v(g0, deck + 1.1), v(g1, deck + 1.1), 0.06, 0.06, sv);
        g.block_ring(&[g0 - side * 0.06, g1 - side * 0.06, g1 + side * 0.06, g0 + side * 0.06], deck, deck + 1.1);
        // the planter along the top edge, overflowing with red flowers
        let po = side * (sg * (hw + 0.2));
        let (pa, pz) = (a + po, z + po);
        g.member(dark, v(pa, deck + h + 0.2), v(pz, deck + h + 0.2), 0.45, 0.5, Vec3::Y);
        let k = (len / 0.35) as i32;
        for j in 0..k {
            let f = (j as f32 + 0.5) / k as f32;
            let hsh = mix(j as u64, sg.to_bits() as u64, 77);
            let jitter = ((hsh % 1000) as f32 / 1000.0 - 0.5) * 0.3;
            let drop = ((hsh >> 10) % 1000) as f32 / 1000.0;
            let c = pa.lerp(pz, f) + side * (sg * (0.15 + jitter));
            let col = [0.32 + 0.14 * drop, 0.015, 0.07];
            let blob = Key::new(Mat::Plain(200), col);
            let leaf = Key::new(Mat::Plain(220), [0.04, 0.12, 0.03]);
            let y0 = deck + h + 0.3 - drop * 0.9;
            let sz = 0.22 + 0.18 * drop;
            g.cuboid(if j % 3 == 0 { leaf } else { blob }, Vec3::new(c.x - sz, y0, c.y - sz), Vec3::new(c.x + sz, y0 + sz * 1.4, c.y + sz));
        }
    }
    // pergola cross beams over the walk
    for i in 0..=n {
        let p = a.lerp(z, i as f32 / n as f32);
        g.member(m.white, v(p - side * (hw + 0.2), deck + h + 0.1), v(p + side * (hw + 0.2), deck + h + 0.1), 0.16, 0.3, ev);
    }
    // the tower's door at the far end
    let d0 = z + dir * 0.02;
    g.poly(glass, &[v(d0 + side * 1.2, deck), v(d0 - side * 1.2, deck), v(d0 - side * 1.2, deck + 2.6), v(d0 + side * 1.2, deck + 2.6)], -sv, Vec3::Y);
}

// ---------------------------------------------------------------- south-west offices

/// A building mass: a prism over a ring of plan points (u, v) in a street-aligned frame `uv`, from `y0` up to
/// top(u) (flat, or sloped). Walls face out; sandstone is clad panel by panel; `slits` scatters the dark slit
/// windows the ballpark's sandstone blocks have. Collides.
fn prism(g: &mut Geo, m: &Mats, uv: &dyn Fn(f32, f32) -> Vec2, k: Key, ring: &[(f32, f32)], y0: f32, top: &dyn Fn(f32) -> f32, slits: bool) {
let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
let dark = Key::new(Mat::Plain(200), [0.05, 0.05, 0.055]);
    let n = ring.len();
    let area: f32 = (0..n).map(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        a.0 * b.1 - b.0 * a.1
    }).sum();
    let ring: Vec<(f32, f32)> = if area > 0.0 { ring.to_vec() } else { ring.iter().rev().copied().collect() };
    let pts: Vec<Vec2> = ring.iter().map(|&(u, v)| uv(u, v)).collect();
    let c = pts.iter().copied().sum::<Vec2>() / n as f32;
    for i in 0..n {
        let (ra, rb) = (ring[i], ring[(i + 1) % n]);
        let (pa, pb) = (pts[i], pts[(i + 1) % n]);
        // split where the top's slope changes so each piece is a convex quad
        let mut cuts = vec![0.0f32, 1.0];
        for s in 1..40 {
            let f = s as f32 / 40.0;
            let u = ra.0 + (rb.0 - ra.0) * f;
            let (u0, u1) = (ra.0 + (rb.0 - ra.0) * (s - 1) as f32 / 40.0, u);
            let d0 = top(u0 + 1e-3) - top(u0);
            let d1 = top(u1 + 1e-3) - top(u1);
            if (d0 - d1).abs() > 1e-4 {
                cuts.push(f);
            }
        }
        cuts.sort_by(f32::total_cmp);
        let e = (pb - pa).normalize_or_zero();
        let (mut p0, mut p1) = (pa, pb);
        let mut right = Vec3::new(e.x, 0.0, e.y);
        if e.perp().dot((pa + pb) / 2.0 - c) < 0.0 {
            right = -right;
            std::mem::swap(&mut p0, &mut p1);
        }
        let _ = (p0, p1);
        for w in cuts.windows(2) {
            let (f0, f1) = (w[0], w[1]);
            let (q0, q1) = (pa.lerp(pb, f0), pa.lerp(pb, f1));
            let (t0, t1) = (top(ra.0 + (rb.0 - ra.0) * f0), top(ra.0 + (rb.0 - ra.0) * f1));
            if t0 - y0 < 0.05 && t1 - y0 < 0.05 {
                continue;
            }
            let quad = [v3(q0, y0), v3(q1, y0), v3(q1, t1.max(y0)), v3(q0, t0.max(y0))];
            let quad: Vec<Vec3> = if right.dot(Vec3::new(e.x, 0.0, e.y)) > 0.0 { quad.to_vec() } else { quad.iter().rev().copied().collect() };
            wall_poly(g, k, &quad, right, Vec3::Y);
        }
        // precast and stucco: panel joints every 3.05 m up and 6.1 m along
        if k != m.sandstone {
            let nrm2 = e.perp() * if e.perp().dot((pa + pb) / 2.0 - c) < 0.0 { -1.0 } else { 1.0 };
            let ev3 = Vec3::new(e.x, 0.0, e.y);
            let (o, r3) = if ev3.cross(Vec3::Y).dot(Vec3::new(nrm2.x, 0.0, nrm2.y)) > 0.0 { (pa, ev3) } else { (pb, -ev3) };
            let len = (pb - pa).length();
            let hmin = top(ra.0).min(top(rb.0));
            let jk = Key::new(Mat::Plain(220), [0.42, 0.35, 0.27]);
            let o2 = o + nrm2 * 0.012;
            let mut y = (y0 / 3.05).floor() * 3.05 + 3.05;
            while y < hmin - 0.4 {
                g.quad(jk, v3(o2, y), r3 * len, Vec3::Y * 0.03);
                y += 3.05;
            }
            let mut u = 6.1;
            while u < len - 0.5 {
                g.quad(jk, v3(o2, y0) + r3 * u, r3 * 0.03, Vec3::Y * (hmin - y0));
                u += 6.1;
            }
        }
        // slit windows in the sandstone: rows every 3.6 m, a few per bay, below the wall's top
        if slits {
            let len = (pb - pa).length();
            let out = Vec3::new(right.z, 0.0, -right.x) * if right.dot(Vec3::new(e.x, 0.0, e.y)) > 0.0 { -1.0 } else { 1.0 };
            let _ = out;
            let nrm2 = e.perp() * if e.perp().dot((pa + pb) / 2.0 - c) < 0.0 { -1.0 } else { 1.0 };
            let mut row = 0;
            let mut y = y0 + 3.0;
            while y < 40.0 {
                let cnt = (len / 4.2) as i32;
                for j in 0..cnt {
                    let hsh = mix(i as u64 * 131 + row, j as u64, (pa.x * 10.0) as i64 as u64);
                    if hsh % 100 < 45 {
                        continue;
                    }
                    let f = (j as f32 + 0.3 + ((hsh >> 8) % 40) as f32 / 100.0) / cnt as f32;
                    let u = ra.0 + (rb.0 - ra.0) * f;
                    if y + 1.2 > top(u) {
                        continue;
                    }
                    let p = pa.lerp(pb, f) + nrm2 * 0.03;
                    let wdt = 1.2 + ((hsh >> 16) % 3) as f32 * 0.4;
                    let q0 = p - e * (wdt / 2.0);
                    let face = Vec3::new(nrm2.x, 0.0, nrm2.y);
                    let r3 = Vec3::Y.cross(face) * -1.0;
                    let r3 = if r3.dot(Vec3::new(e.x, 0.0, e.y)) >= 0.0 { Vec3::new(e.x, 0.0, e.y) } else { -Vec3::new(e.x, 0.0, e.y) };
                    let o = if r3.dot(Vec3::new(e.x, 0.0, e.y)) >= 0.0 { q0 } else { q0 + e * wdt };
                    g.quad(dark, v3(o, y), r3 * wdt, Vec3::Y * 0.4);
                }
                y += 3.6;
                row += 1;
            }
        }
    }
    // roof: flat parts flat, sloped parts sloped (fan over the ring at the top heights)
    let tops: Vec<Vec3> = ring.iter().map(|&(u, v)| v3(uv(u, v), top(u))).collect();
    let flat = ring.iter().all(|r| (top(r.0) - top(ring[0].0)).abs() < 1e-3);
    if flat {
        poly_up(g, m.concrete, &tops);
    } else {
        for i in 1..n - 1 {
            let tri = [tops[0], tops[i], tops[i + 1]];
            let nn = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
            let tri = if nn.y < 0.0 { [tri[0], tri[2], tri[1]] } else { tri };
            let nn = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
            if nn.length_squared() < 0.5 {
                continue;
            }
            let r = Vec3::Y.cross(nn).normalize_or(Vec3::X);
            let up = nn.cross(r);
            g.poly(k, &tri, r, up);
        }
    }
    let hmax = ring.iter().map(|r| top(r.0)).fold(0.0f32, f32::max);
    g.block_ring(&pts, y0, hmax);
}

/// The pale peach precast of the light towers and the office block (photos: about 0.8 of the white steel's value).
fn pale_precast() -> Key {
    Key::new(Mat::Surface("tjeneevfw"), [1.15, 0.95, 0.74]).tile(2.0)
}

/// The office block along the street south-west of the bowl, between the left-field side and the lantern tower
/// (lidar massing; photos from across the trolley tracks). Laid out on a grid along the street: u runs east-south-east
/// along it from (-95, 48), v toward it. A sandstone podium with planted roof terraces runs along the street; on it
/// a pale office block with ribbon windows; at the west end a sandstone mesa whose west face slopes up to a 31 m top;
/// a sandstone block steps up to the tower at the east end; terraces and a stair spine face the bowl. The sandstone
/// is pierced by slit windows.
fn sw_offices(g: &mut Geo, m: &Mats) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let a = 32f32.to_radians();
    let (eu, ev) = (Vec2::new(a.cos(), a.sin()), Vec2::new(-a.sin(), a.cos()));
    let uv = |u: f32, v: f32| Vec2::new(-95.0, 48.0) + eu * u + ev * v - HOME;
    let pale = pale_precast();
    // office glazing reads dark from the street: a dark glossy pane rather than see-through glass
    let glass = Key::new(Mat::Plain(185), [0.028, 0.036, 0.045]);
    let dark = Key::new(Mat::Plain(200), [0.05, 0.05, 0.055]);
    let mull = Key::new(Mat::Plain(120), [0.55, 0.56, 0.56]);
    // a prism over a ring of (u, v) with its top at height top(u) (flat, or sloped up to a ridge); walls face out
    let flat = |h: f32| move |_u: f32| h;
    // the west mesa and the mass behind it: their west faces slope up from the street
    prism(g, m, &uv, m.sandstone, &[(6.0, -3.0), (42.0, -3.0), (42.0, 13.0), (37.0, 13.0), (26.0, 6.0), (18.0, 2.0), (6.0, 0.0)], 0.0, &|u| ((u - 6.0) * 1.55).clamp(0.5, 24.5), true);
    // the higher box at the mesa's east corner (lidar: 31 m)
    prism(g, m, &uv, m.sandstone, &[(27.0, 0.0), (40.0, 0.0), (40.0, 10.0), (27.0, 10.0)], 24.5, &flat(31.0), false);
    prism(g, m, &uv, m.sandstone, &[(4.0, -12.0), (34.0, -12.0), (34.0, -3.0), (6.0, -3.0)], 0.0, &|u| ((u - 4.0) * 1.4).clamp(0.5, 25.0), true);
    // the podium along the street
    prism(g, m, &uv, m.sandstone, &[(42.0, 12.5), (108.0, 12.5), (108.0, 25.0), (72.0, 25.0), (60.0, 22.5), (48.0, 17.5), (42.0, 17.5)], 0.0, &flat(8.5), false);
    // the pale office block on it, and its terraces toward the bowl
    prism(g, m, &uv, pale, &[(42.0, 2.5), (93.0, 2.5), (93.0, 12.5), (42.0, 12.5)], 8.5, &flat(22.0), false);
    // (kept clear of the stair cores that stand between the offices and the bowl)
    prism(g, m, &uv, pale, &[(42.0, -8.0), (90.0, -8.0), (90.0, 2.5), (42.0, 2.5)], 0.0, &flat(8.5), false);
    prism(g, m, &uv, pale, &[(45.0, -6.0), (90.0, -6.0), (90.0, 2.5), (45.0, 2.5)], 8.5, &flat(17.5), false);
    prism(g, m, &uv, pale, &[(42.0, 2.5), (93.0, 2.5), (93.0, 12.5), (42.0, 12.5)], 0.0, &flat(8.5), false);
    // the stair spine and the sandstone block stepping up to the tower
    prism(g, m, &uv, m.sandstone, &[(90.0, -8.0), (94.0, -8.0), (94.0, 2.0), (90.0, 2.0)], 0.0, &flat(24.0), true);
    prism(g, m, &uv, m.sandstone, &[(93.0, 0.0), (104.0, 0.0), (104.0, 12.5), (93.0, 12.5)], 0.0, &flat(26.0), true);
    // the lantern tower's street face: a column of slit windows near its east end, and one tall slit at the top
    {
        let tr = tower_ring();
        let (a, b) = (Vec2::new(-21.0, 112.0) - HOME, Vec2::new(-6.0, 121.0) - HOME);
        let _ = tr;
        let e = (b - a).normalize();
        let n = Vec2::new(-e.y, e.x) * if Vec2::new(-e.y, e.x).dot(a - (Vec2::new(-10.0, 111.0) - HOME)) > 0.0 { 1.0 } else { -1.0 };
        let r3 = Vec3::new(e.x, 0.0, e.y);
        let r3 = if r3.cross(Vec3::Y).dot(Vec3::new(n.x, 0.0, n.y)) > 0.0 { r3 } else { -r3 };
        let len = (b - a).length();
        for (f, y, w, h) in [(0.78, 7.5, 2.4, 0.5), (0.78, 11.5, 2.4, 0.5), (0.78, 15.5, 2.4, 0.5), (0.78, 19.5, 2.4, 0.5), (0.78, 23.5, 2.4, 0.5), (0.86, 27.0, 0.7, 3.0)] {
            let c = a + e * (len * f) + n * 0.03;
            let o = if r3.dot(Vec3::new(e.x, 0.0, e.y)) > 0.0 { c - e * (w / 2.0) } else { c + e * (w / 2.0) };
            g.quad(dark, v3(o, y), r3 * w, Vec3::Y * h);
        }
    }
    // the office block's street face: a recessed loggia floor on the podium, then ribbon windows floor by floor
    let (fa, fb) = (uv(42.0, 12.5), uv(93.0, 12.5));
    let out = ev * 0.03;
    let along = (fb - fa).normalize();
    let len = (fb - fa).length();
    let face_r = Vec3::new(along.x, 0.0, along.y);
    // the face points +v (toward the street): right x up must point there
    let face_r = if face_r.cross(Vec3::Y).dot(Vec3::new(ev.x, 0.0, ev.y)) > 0.0 { face_r } else { -face_r };
    let o = if face_r.dot(Vec3::new(along.x, 0.0, along.y)) > 0.0 { fa } else { fb };
    g.quad(dark, v3(o + out, 9.1), face_r * len, Vec3::Y * 2.6);
    let mut u = 3.0;
    while u < len - 1.0 {
        let p = fa + along * u + ev * 0.6;
        g.member(pale, v3(p, 8.5), v3(p, 12.0), 0.6, 0.6, Vec3::new(ev.x, 0.0, ev.y));
        u += 6.0;
    }
    for (y0, y1) in [(12.6, 14.7), (16.1, 18.2), (19.5, 21.1)] {
        // wide windows between piers: 5.2 m of glass in three lights, 1.4 m piers
        let mut u = 0.9;
        while u + 5.2 < len {
            let q = if face_r.dot(Vec3::new(along.x, 0.0, along.y)) > 0.0 { fa + along * u } else { fa + along * (u + 5.2) };
            g.quad(glass, v3(q + out, y0), face_r * 5.2, Vec3::Y * (y1 - y0));
            for k in 1..3 {
                let p = fa + along * (u + 5.2 * k as f32 / 3.0) + ev * 0.06;
                g.member(mull, v3(p, y0), v3(p, y1), 0.08, 0.06, Vec3::new(ev.x, 0.0, ev.y));
            }
            u += 6.6;
        }
    }
}

// ---------------------------------------------------------------- east building

/// A flat stone-coloured key for the cladding's returns that are not upright (heads and sills of openings).
fn stone_flat() -> Key {
    Key::new(Mat::Surface("pt_cleft_sandstone"), STONE_TONE).tile(2.0).rough(1.2)
}

/// An upright wall seen from outside: from `left` to `right` (plan points as seen from the front), y0..y1, with
/// rectangular holes (u0, u1, v0, v1; u metres from `left`, v height). Sandstone is laid stone by stone.
fn wall_holes(g: &mut Geo, k: Key, left: Vec2, right: Vec2, y0: f32, y1: f32, holes: &[[f32; 4]]) {
    let len = (right - left).length();
    let r = (right - left) / len;
    let r3 = Vec3::new(r.x, 0.0, r.y);
    let (mut us, mut vs) = (vec![0.0, len], vec![y0, y1]);
    for h in holes {
        us.extend([h[0].clamp(0.0, len), h[1].clamp(0.0, len)]);
        vs.extend([h[2].clamp(y0, y1), h[3].clamp(y0, y1)]);
    }
    us.sort_by(f32::total_cmp);
    vs.sort_by(f32::total_cmp);
    us.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    vs.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    // merge cells along u into runs so stones are not cut at every hole edge
    for w in vs.windows(2) {
        let vm = (w[0] + w[1]) / 2.0;
        let mut run: Option<f32> = None;
        for i in 0..us.len() {
            let open = i + 1 < us.len() && {
                let um = (us[i] + us[i + 1]) / 2.0;
                !holes.iter().any(|h| um > h[0] && um < h[1] && vm > h[2] && vm < h[3])
            };
            match (run, open) {
                (None, true) => run = Some(us[i]),
                (Some(u0), false) => {
                    let (p, q) = (left + r * u0, left + r * us[i]);
                    wall_poly(g, k, &[Vec3::new(p.x, w[0], p.y), Vec3::new(q.x, w[0], q.y), Vec3::new(q.x, w[1], q.y), Vec3::new(p.x, w[1], p.y)], r3, Vec3::Y);
                    run = None;
                }
                _ => {}
            }
        }
    }
}

/// The returns of an opening `depth` deep in a wall (see `wall_holes`): jambs in the wall's material, head and sill flat.
fn reveal(g: &mut Geo, k: Key, left: Vec2, right: Vec2, h: [f32; 4], depth: f32) {
    let r = (right - left).normalize();
    let out = Vec2::new(-r.y, r.x) * -1.0;
    // right x up = out: check and flip
    let out = if Vec3::new(r.x, 0.0, r.y).cross(Vec3::Y).dot(Vec3::new(out.x, 0.0, out.y)) > 0.0 { out } else { -out };
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let (a, b) = (left + r * h[0], left + r * h[1]);
    let inw = -out * depth;
    let i3 = Vec3::new(-out.x, 0.0, -out.y);
    // left jamb faces +r, right jamb faces -r
    wall_poly(g, k, &[v3(a, h[2]), v3(a + inw, h[2]), v3(a + inw, h[3]), v3(a, h[3])], i3, Vec3::Y);
    wall_poly(g, k, &[v3(b + inw, h[2]), v3(b, h[2]), v3(b, h[3]), v3(b + inw, h[3])], -i3, Vec3::Y);
    let flat = if k == mats().sandstone { stone_flat() } else { k };
    poly_down(g, flat, &[v3(a, h[3]), v3(b, h[3]), v3(b + inw, h[3]), v3(a + inw, h[3])]);
    if h[2] > 0.05 {
        poly_up(g, flat, &[v3(a, h[2]), v3(b, h[2]), v3(b + inw, h[2]), v3(a + inw, h[2])]);
    }
}

/// A sunshade fixed at an opening's head, as on the east building: plain rectangular prisms in one light metal - a
/// frame (wall bar, outer fascia, cross arms at the ends and the thirds) carrying four wide flat slats that run the
/// opening's length with narrow gaps between. `y` is its top.
fn louvre(g: &mut Geo, left: Vec2, right: Vec2, out: Vec2, y: f32, proj: f32) {
    let k = Key::new(Mat::Plain(120), [0.74, 0.74, 0.72]);
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (right - left).length();
    let r = (right - left) / len;
    // frame
    g.member(k, v3(left + out * 0.06, y - 0.1), v3(right + out * 0.06, y - 0.1), 0.12, 0.2, Vec3::Y);
    g.member(k, v3(left + out * (proj - 0.06), y - 0.1), v3(right + out * (proj - 0.06), y - 0.1), 0.12, 0.2, Vec3::Y);
    let panels = (len / 2.8).round().max(1.0) as i32;
    for i in 0..=panels {
        let p = left + r * ((len - 0.12) * i as f32 / panels as f32 + 0.06);
        g.member(k, v3(p, y - 0.1), v3(p + out * proj, y - 0.1), 0.12, 0.2, Vec3::Y);
    }
    // slats
    let slats = 4;
    let (inner, gap) = (proj - 0.24, 0.045);
    let w = (inner - gap * (slats as f32 + 1.0)) / slats as f32;
    for j in 0..slats {
        let q = out * (0.12 + gap + w / 2.0 + j as f32 * (w + gap));
        g.member(k, v3(left + r * 0.1 + q, y - 0.05), v3(right - r * 0.1 + q, y - 0.05), w, 0.05, Vec3::Y);
    }
}

/// Signage in a 5x7 pixel face (generic words only): `origin` is the text's bottom-left, `right` its reading
/// direction; each pixel is `px` square. Faces right x up.
fn pixel_text(g: &mut Geo, k: Key, text: &str, origin: Vec3, right: Vec3, px: f32) {
    let glyph = |c: char| -> [&'static str; 7] {
        match c {
            'C' => [".####", "#....", "#....", "#....", "#....", "#....", ".####"],
            'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
            'I' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
            'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
            'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
            'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
            '0' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
            '1' => ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
            '2' => [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
            '3' => ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
            '4' => ["#...#", "#...#", "#...#", "#####", "....#", "....#", "....#"],
            '5' => ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
            '6' => [".###.", "#....", "#....", "####.", "#...#", "#...#", ".###."],
            '7' => ["#####", "....#", "...#.", "..#..", "..#..", "..#..", "..#.."],
            '8' => [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
            '9' => [".###.", "#...#", "#...#", ".####", "....#", "....#", ".###."],
            _ => ["....."; 7],
        }
    };
    for (ci, ch) in text.chars().enumerate() {
        for (row, line) in glyph(ch).iter().enumerate() {
            let b = line.as_bytes();
            let mut c = 0;
            while c < 5 {
                if b[c] != b'#' {
                    c += 1;
                    continue;
                }
                let st = c;
                while c < 5 && b[c] == b'#' {
                    c += 1;
                }
                let o = origin + right * ((ci * 6 + st) as f32 * px) + Vec3::Y * ((6 - row) as f32 * px);
                g.quad(k, o, right * ((c - st) as f32 * px), Vec3::Y * px);
            }
        }
    }
}

/// Trailing plants hanging from `y` along a ledge: thin leaf strips of uneven length in three greens.
fn vines(g: &mut Geo, left: Vec2, right: Vec2, out: Vec2, y: f32, drop: f32, seed: u64, dry: bool) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (right - left).length();
    let r = (right - left) / len;
    let r3 = Vec3::new(r.x, 0.0, r.y);
    let r3 = if r3.cross(Vec3::Y).dot(Vec3::new(out.x, 0.0, out.y)) > 0.0 { r3 } else { -r3 };
    let greens = if dry { [[0.2, 0.15, 0.07], [0.13, 0.12, 0.05], [0.09, 0.1, 0.04]] } else { [[0.035, 0.085, 0.03], [0.05, 0.11, 0.035], [0.028, 0.065, 0.028]] };
    let n = (len / 0.055) as i32;
    for i in 0..n {
        let h = mix(seed, i as u64, 17);
        let f = (h % 1000) as f32 / 1000.0;
        // clumps: long in some stretches, short between
        let clump = 0.5 + 0.5 * ((i as f32 * 0.055 * 2.1 + (seed % 7) as f32).sin() * 0.5 + 0.5);
        let l = drop * clump * (0.35 + 0.65 * f * f);
        let w = if dry { 0.02 + 0.02 * ((h >> 10) % 100) as f32 / 100.0 } else { 0.05 + 0.05 * ((h >> 10) % 100) as f32 / 100.0 };
        if dry && (h >> 50) % 3 != 0 {
            continue;
        }
        let p = left + r * (i as f32 * 0.055) + out * (0.05 + 0.12 * ((h >> 20) % 100) as f32 / 100.0);
        let o = if r3.dot(Vec3::new(r.x, 0.0, r.y)) > 0.0 { p } else { p + r * w };
        g.quad(Key::new(Mat::Plain(215), greens[(h >> 30) as usize % 3]), v3(o, y - l), r3 * w, Vec3::Y * l);
    }
}

/// Bougainvillea along a roof or terrace edge: a ragged line of small leaf and bract clumps, some spilling over.
fn bougainvillea(g: &mut Geo, left: Vec2, right: Vec2, out: Vec2, y: f32, drop: f32, seed: u64, red: bool) {
    let len = (right - left).length();
    let r = (right - left) / len;
    let n = (len / 0.018) as i32;
    for i in 0..n {
        let h = mix(seed, i as u64, 29);
        let f = (h % 1000) as f32 / 1000.0;
        let sz = 0.02 + 0.035 * ((h >> 10) % 100) as f32 / 100.0;
        let c = left + r * (i as f32 * 0.018) + out * (0.3 * ((h >> 20) % 100) as f32 / 100.0 - 0.12);
        // a low ragged mound on the edge, strands spilling over in places
        let spill = ((i as f32 * 0.018 * 1.7 + seed as f32).sin() * 0.5 + 0.5).powi(3);
        let yy = y + 0.3 * f * f - drop * spill * ((h >> 34) % 1000) as f32 / 1000.0;
        let col = match ((h >> 44) % 6, red) {
            (0 | 1, true) => [0.3, 0.02, 0.03],
            (2, true) => [0.2, 0.03, 0.03],
            (0, false) => [0.11, 0.09, 0.04],
            (1, false) => [0.085, 0.1, 0.04],
            (2 | 3, _) => [0.07, 0.085, 0.035],
            _ => [0.04, 0.075, 0.03],
        };
        g.cuboid(Key::new(Mat::Plain(215), col), Vec3::new(c.x - sz, yy - sz, c.y - sz), Vec3::new(c.x + sz, yy + sz, c.y + sz));
    }
}

/// A run of ticket windows as at the ballpark's box office (street photo): each booth a wide light with a round
/// speaking grille and a pass-through tray, a narrow sidelight, a transom over both, in white aluminium framing over a
/// counter shelf, numbered in gold above. `left` to `right` as seen from the front (`out`); `lanes` adds the queue
/// rails in front.
fn ticket_wall(g: &mut Geo, m: &Mats, left: Vec2, right: Vec2, out: Vec2, y0: f32, y1: f32, n: usize, first: usize, lanes: bool) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (right - left).length();
    let r = (right - left) / len;
    let (r3, o3) = (Vec3::new(r.x, 0.0, r.y), Vec3::new(out.x, 0.0, out.y));
    let glass = Key::new(Mat::Plain(200), [0.03, 0.036, 0.04]);
    let inside = Key::new(Mat::Plain(220), [0.1, 0.105, 0.11]);
    let steel = Key::new(Mat::Plain(70), [0.5, 0.5, 0.5]).wear(wear::METAL);
    let gold = Key::new(Mat::Plain(140), [0.62, 0.36, 0.06]);
    let rail = Key::new(Mat::Plain(150), [0.09, 0.03, 0.022]);
    let w = len / n as f32;
    let yt = y1 - 0.24 * (y1 - y0);
    g.quad(glass, v3(left + out * 0.02, y0), r3 * len, Vec3::Y * (y1 - y0));
    for y in [y0, yt, y1] {
        g.member(m.white, v3(left + out * 0.05, y), v3(right + out * 0.05, y), 0.06, 0.07, o3);
    }
    g.member(steel, v3(left + out * 0.2, y0 - 0.05), v3(right + out * 0.2, y0 - 0.05), 0.4, 0.05, Vec3::Y);
    for i in 0..n {
        let x0 = i as f32 * w;
        let wide = w * 0.64;
        let at = |x: f32| left + r * x;
        for x in [x0, x0 + wide] {
            g.member(m.white, v3(at(x) + out * 0.05, y0), v3(at(x) + out * 0.05, y1), 0.06, 0.07, o3);
        }
        // inside: a counter-height desk edge and a monitor's back, just behind the glass
        g.quad(inside, v3(at(x0 + 0.1) + out * 0.03, y0 + 0.05), r3 * (wide * 0.38), Vec3::Y * 0.36);
        // the speaking grille: a steel ring in the glass
        let c = v3(at(x0 + wide / 2.0) + out * 0.035, y0 + (yt - y0) * 0.55);
        // (after the booth model the user picked out: a broad ring, a dark perforated disc, a small hub)
        for (rad, k, off) in [(0.12f32, steel, 0.0f32), (0.085, inside, 0.004), (0.022, steel, 0.008)] {
            for s in 0..12 {
                let (a0, a1) = (s as f32 / 12.0 * std::f32::consts::TAU, (s + 1) as f32 / 12.0 * std::f32::consts::TAU);
                let p = |a: f32| c + o3 * off + r3 * (a.cos() * rad) + Vec3::Y * (a.sin() * rad);
                g.poly(k, &[c + o3 * off, p(a0), p(a1)], r3, Vec3::Y);
            }
        }
        // the wide light's own heavier frame, so each booth reads as a window and not a pane in a grid
        let (fa, fb2) = (at(x0 + 0.07) + out * 0.07, at(x0 + wide - 0.07) + out * 0.07);
        for y in [y0 + 0.07, yt - 0.07] {
            g.member(m.white, v3(fa, y), v3(fb2, y), 0.05, 0.05, o3);
        }
        for p in [fa, fb2] {
            g.member(m.white, v3(p, y0 + 0.07), v3(p, yt - 0.07), 0.05, 0.05, o3);
        }
        // a panelled base under the counter: a framed panel per booth
        let base = Key::new(Mat::Plain(200), [0.16, 0.1, 0.05]);
        let trim = Key::new(Mat::Plain(150), [0.3, 0.2, 0.1]);
        let (b0, b1) = (0.22f32, y0 - 0.16);
        if b1 > b0 + 0.2 {
            g.quad(base, v3(at(x0 + 0.1) + out * 0.025, b0), r3 * (w - 0.2), Vec3::Y * (b1 - b0));
            for y in [b0, b1] {
                g.member(trim, v3(at(x0 + 0.1) + out * 0.04, y), v3(at(x0 + w - 0.1) + out * 0.04, y), 0.04, 0.03, o3);
            }
            for x in [x0 + 0.1, x0 + w - 0.1] {
                g.member(trim, v3(at(x) + out * 0.04, b0), v3(at(x) + out * 0.04, b1), 0.04, 0.03, o3);
            }
        }
        // the tray
        g.member(steel, v3(at(x0 + wide * 0.3) + out * 0.18, y0 + 0.02), v3(at(x0 + wide * 0.7) + out * 0.18, y0 + 0.02), 0.3, 0.03, Vec3::Y);
        // its number
        let label = format!("{}", first + i);
        let px = 0.032;
        let tw = (label.len() as f32 * 6.0 - 1.0) * px;
        pixel_text(g, gold, &label, v3(at(x0 + wide / 2.0 - tw / 2.0) + out * 0.03, y1 + 0.1), r3, px);
    }
    g.member(m.white, v3(right + out * 0.05, y0), v3(right + out * 0.05, y1), 0.06, 0.07, o3);
    if lanes {
        // queue rails: a post at the counter and one 2.6 m out per booth line, two rails between
        for i in 0..=n {
            let p0 = left + r * (i as f32 * w) + out * 0.45;
            let p1 = p0 + out * 2.2;
            for p in [p0, p1] {
                g.tube(rail, v3(p, 0.1), v3(p, 1.08), 0.035, 8);
            }
            for y in [0.6f32, 1.05] {
                g.tube(rail, v3(p0, y), v3(p1, y), 0.03, 6);
            }
            g.block_ring(&[p0 - r * 0.04, p1 - r * 0.04, p1 + r * 0.04, p0 + r * 0.04], 0.1, 1.08);
        }
    }
}

/// The sandstone building along the street east of the home plate gate, built from measurement: its footprint and
/// heights from the lidar; its street face and south-west end face from street photos rectified onto a metre grid
/// (camera solved against the lidar corners; heights cross-checked between two photos to 0.5 m).
/// Frame: a runs north-east along the street from (66, 74), b in toward the bowl; the street face is the plane b = 0.5.
fn east_building(g: &mut Geo, m: &Mats) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let (t, n) = (Vec2::new(1.0, -1.0).normalize(), Vec2::new(-1.0, -1.0).normalize());
    let uv = |a: f32, b: f32| Vec2::new(66.0, 74.0) + t * a + n * b - HOME;
    let flat = |h: f32| move |_u: f32| h;
    let pale = pale_precast();
    let tan = Key::new(Mat::Surface("tjeneevfw"), [0.8, 0.62, 0.44]).tile(2.0);
    let pane = Key::new(Mat::Plain(235), [0.016, 0.018, 0.016]);
    let frost = Key::new(Mat::Plain(200), [0.26, 0.3, 0.32]);
    let frame = Key::new(Mat::Plain(150), [0.3, 0.31, 0.32]);
    let sill = Key::new(Mat::Plain(160), [0.62, 0.6, 0.56]);
    let dark = Key::new(Mat::Plain(230), [0.035, 0.03, 0.028]);
    let (top, fb, back) = (24.1, 0.5, 20.0);
    let (a0, a1) = (-13.0, 25.7);
    // the hall: from a = -9 to the podium's side wall, 5.8 m high, right through to the back; the slot over it
    let (h0, h1, hh, bb) = (-9.0f32, 8.17f32, 5.8f32, 19.5f32);
    let (s0, sy0, sy1, sd) = (-3.7f32, 8.8f32, 10.55f32, 1.5f32);

    // --- the street face: two rows of three strip windows under sunshades, the hall's mouth, the loggia slot
    let (sl, sr) = (uv(a0, fb), uv(a1, fb));
    let su = |a: f32| a - a0;
    let bays = [(-7.0f32, 1.5f32), (2.8, 11.2), (12.5, 21.0)];
    let rows = [(18.6f32, 19.8f32), (13.1, 14.3)];
    let mut holes: Vec<[f32; 4]> = vec![[su(h0), su(h1), 0.0, hh], [su(s0), su(h1), sy0, sy1]];
    for &(w0, w1) in &bays {
        for &(y0, y1) in &rows {
            holes.push([su(w0), su(w1), y0, y1]);
        }
    }
    wall_holes(g, m.sandstone, sl, sr, 0.0, top, &holes);
    let out = -n;
    let o3 = Vec3::new(out.x, 0.0, out.y);
    let t3 = Vec3::new(t.x, 0.0, t.y);
    let n3 = Vec3::new(n.x, 0.0, n.y);
    for &(w0, w1) in &bays {
        for &(y0, y1) in &rows {
            reveal(g, m.sandstone, sl, sr, [su(w0), su(w1), y0, y1], 1.0);
            let q = uv(w0, fb) - out * 1.0;
            g.quad(pane, v3(q, y0), t3 * (w1 - w0), Vec3::Y * (y1 - y0));
            for k in 1..3 {
                let p = uv(w0 + (w1 - w0) * k as f32 / 3.0, fb) - out * 0.96;
                g.member(dark, v3(p, y0), v3(p, y1), 0.07, 0.07, o3);
            }
            g.member(sill, v3(uv(w0, fb) + out * 0.04, y0 - 0.04), v3(uv(w1, fb) + out * 0.04, y0 - 0.04), 0.1, 0.16, o3);
            louvre(g, uv(w0, fb), uv(w1, fb), out, y1 + 0.1, 1.6);
        }
    }

    // --- the ticket hall (measured on a zoomed street photo): stone side walls, the right one with three bays of
    // ticket windows under dark openings; a brown back wall with a full-width band of ticket windows, a sign and
    // two screens; a tan ceiling; a floor slab
    let brown = Key::new(Mat::Plain(215), [0.1, 0.068, 0.035]);
    let ceil = Key::new(Mat::Surface("tjeneevfw"), [0.4, 0.29, 0.17]).tile(2.0);
    let paving = Key::new(Mat::Surface("vlzlajnfw"), [0.64, 0.62, 0.58]).tile(3.0).rough(0.8);
    wall_holes(g, m.sandstone, uv(h0, fb), uv(h0, bb), 0.0, hh, &[]);
    let (rl, rr) = (uv(h1, bb), uv(h1, fb));
    let ru = |d: f32| (bb - fb) - d;
    let side_bays = [(1.4f32, 6.5f32), (8.4, 12.4), (14.4, 18.6)];
    let mut rholes: Vec<[f32; 4]> = vec![];
    for &(d0, d1) in &side_bays {
        rholes.push([ru(d1), ru(d0), 4.1, 5.4]);
        rholes.push([ru(d1), ru(d0), 1.0, 2.65]);
    }
    wall_holes(g, m.sandstone, rl, rr, 0.0, hh, &rholes);
    for &(d0, d1) in &side_bays {
        reveal(g, m.sandstone, rl, rr, [ru(d1), ru(d0), 4.1, 5.4], 0.4);
        reveal(g, m.sandstone, rl, rr, [ru(d1), ru(d0), 1.0, 2.65], 0.25);
        let o = uv(h1, fb + d1);
        g.quad(dark, v3(o + t * 0.4, 4.1), -n3 * (d1 - d0), Vec3::Y * 1.3);
        let booths = ((d1 - d0) / 1.7).round().max(1.0) as usize;
        ticket_wall(g, m, o + t * 0.25, uv(h1, fb + d0) + t * 0.25, -t, 1.0, 2.65, booths, 10 + (ru(d1) / 1.7) as usize, false);
    }
    g.quad(brown, v3(uv(h0, bb), 0.0), t3 * (h1 - h0), Vec3::Y * hh);
    let (tw0, tw1) = (h0 + 0.6, h1 - 0.4);
    ticket_wall(g, m, uv(tw0, bb), uv(tw1, bb), out, 1.04, 2.61, 9, 1, true);
    pixel_text(g, Key::new(Mat::Plain(140), [0.62, 0.36, 0.06]), "TICKETS", v3(uv(-0.85, bb) + out * 0.04, 3.3), t3, 0.054);
    for (x0, x1) in [(2.3f32, 3.6f32), (4.35, 5.7)] {
        g.quad(Key::new(Mat::Plain(60), [0.02, 0.022, 0.026]), v3(uv(x0, bb) + out * 0.05, 3.28), t3 * (x1 - x0), Vec3::Y * 0.78);
    }
    poly_down(g, ceil, &[v3(uv(h0, fb), hh), v3(uv(h1, fb), hh), v3(uv(h1, bb), hh), v3(uv(h0, bb), hh)]);
    let floor = [uv(h0, fb), uv(h1, fb), uv(h1, bb), uv(h0, bb)];
    poly_up(g, paving, &floor.map(|p| v3(p, 0.1)));
    g.block_ring(&floor, -0.6, 0.1);
    for d in [8.0f32, 16.0] {
        g.light(v3(uv(-0.4, fb + d), hh - 1.6), 4.0, [1.0, 0.92, 0.78]);
    }

    // --- the loggia slot over the hall: a stone parapet (the face itself) with a rail, a shallow balcony behind,
    // a tan back wall with a door, a tan soffit
    {
        let (pl, pr) = (uv(s0, fb), uv(h1, fb));
        let inw = -out * sd;
        let i3 = -o3;
        wall_poly(g, m.sandstone, &[v3(pl, sy0), v3(pl + inw, sy0), v3(pl + inw, sy1), v3(pl, sy1)], i3, Vec3::Y);
        wall_poly(g, m.sandstone, &[v3(pr + inw, sy0), v3(pr, sy0), v3(pr, sy1), v3(pr + inw, sy1)], -i3, Vec3::Y);
        poly_down(g, ceil, &[v3(pl, sy1), v3(pr, sy1), v3(pr + inw, sy1), v3(pl + inw, sy1)]);
        poly_up(g, stone_flat(), &[v3(pl, sy0), v3(pr, sy0), v3(pr - out * 0.4, sy0), v3(pl - out * 0.4, sy0)]);
        // (a warmer stucco than the rest: in the photos it sits in shade lit by the stone's bounce)
        let slot_wall = Key::new(Mat::Surface("tjeneevfw"), [0.95, 0.6, 0.32]).tile(2.0);
        let (bl, br) = (pl + inw, pr + inw);
        let bu = |a: f32| a - s0;
        // a walkway leads back into the building on the left; on the right a doorway stands in a recess
        let (walk, alcove) = ([bu(-1.0), bu(0.75), 7.7, 10.3], [bu(6.25), bu(7.8), 7.7, 10.05]);
        wall_holes(g, slot_wall, bl, br, 7.7, sy1, &[walk, alcove]);
        // (their inner faces are in deep shade in the photos)
        let deep = Key::new(Mat::Surface("tjeneevfw"), [0.3, 0.2, 0.12]).tile(2.0);
        reveal(g, deep, bl, br, walk, 4.5);
        reveal(g, deep, bl, br, alcove, 0.9);
        g.quad(Key::new(Mat::Plain(60), [0.1, 0.14, 0.18]), v3(uv(-1.0, fb) + inw - out * 4.5, 7.7), t3 * 1.75, Vec3::Y * 2.6);
        g.member(dark, v3(uv(-0.125, fb) + inw - out * 4.45, 7.7), v3(uv(-0.125, fb) + inw - out * 4.45, 10.3), 0.05, 0.05, o3);
        // the door: a grey steel leaf with a small vision light, in a pale frame
        let door = Key::new(Mat::Plain(120), [0.2, 0.2, 0.2]);
        let db = inw - out * 0.9;
        g.quad(door, v3(uv(6.7, fb) + db + out * 0.02, 7.7), t3 * 0.8, Vec3::Y * 2.05);
        g.quad(dark, v3(uv(6.95, fb) + db + out * 0.03, 9.0), t3 * 0.3, Vec3::Y * 0.5);
        for a in [6.68f32, 7.52] {
            g.member(sill, v3(uv(a, fb) + db + out * 0.04, 7.7), v3(uv(a, fb) + db + out * 0.04, 9.77), 0.05, 0.06, o3);
        }
        g.member(sill, v3(uv(6.68, fb) + db + out * 0.04, 9.77), v3(uv(7.52, fb) + db + out * 0.04, 9.77), 0.06, 0.05, o3);
        g.member(Key::new(Mat::Plain(70), [0.5, 0.5, 0.5]), v3(uv(7.38, fb) + db + out * 0.06, 8.7), v3(uv(7.38, fb) + db + out * 0.06, 8.82), 0.03, 0.05, o3);
        g.member(frame, v3(pl - out * 0.2, sy0 + 0.08), v3(pr - out * 0.2, sy0 + 0.08), 0.05, 0.05, Vec3::Y);
    }

    // --- the south-west end face: three glazed bays (dark glass over a frosted band), louvres, a planter strip below
    let (el, er) = (uv(a0, back), uv(a0, fb));
    let eu = |b: f32| back - b;
    let eout = -t;
    let e3 = Vec3::new(-n.x, 0.0, -n.y);
    let ebays = [(18.75f32, 14.0f32), (12.6, 8.0), (6.5, 2.0)];
    let eholes: Vec<[f32; 4]> = ebays.iter().map(|&(b0, b1)| [eu(b0), eu(b1), 12.8, 19.5]).collect();
    wall_holes(g, m.sandstone, el, er, 0.0, top, &eholes);
    for (bi, &(b0, b1)) in ebays.iter().enumerate() {
        reveal(g, m.sandstone, el, er, [eu(b0), eu(b1), 12.8, 19.5], 0.5);
        let w = b0 - b1;
        let q = uv(a0, b0) - eout * 0.5;
        for (y0, y1, k) in [(12.8f32, 14.0f32, pane), (14.0, 14.4, frame), (14.4, 16.5, frost), (16.5, 16.9, frame), (16.9, 19.5, pane)] {
            g.quad(k, v3(q, y0), e3 * w, Vec3::Y * (y1 - y0));
        }
        for k in 1..4 {
            let p = uv(a0, b0 - w * k as f32 / 4.0) - eout * 0.46;
            g.member(frame, v3(p, 14.4), v3(p, 19.5), 0.06, 0.06, Vec3::new(eout.x, 0.0, eout.y));
        }
        louvre(g, uv(a0, b0), uv(a0, b1), eout, 19.7, 1.5);
        louvre(g, uv(a0, b0), uv(a0, b1), eout, 14.3, 1.5);
        vines(g, uv(a0, b0 - 0.15), uv(a0, b1 + 0.15), eout, 13.0, 1.3, bi as u64 + 40, false);
    }
    // --- the other two faces, the roof, collision (the hall stays open to walk through to the ticket windows)
    wall_holes(g, m.sandstone, uv(a1, fb), uv(a1, back), 0.0, top, &[]);
    wall_holes(g, m.sandstone, uv(a1, back), uv(a0, back), 0.0, top, &[]);
    poly_up(g, m.concrete, &[v3(uv(a0, fb), top - 0.3), v3(uv(a1, fb), top - 0.3), v3(uv(a1, back), top - 0.3), v3(uv(a0, back), top - 0.3)]);
    for (r, y0, y1) in [
        ([(a0, fb), (h0, fb), (h0, back), (a0, back)], 0.0, top),
        ([(h1, fb), (a1, fb), (a1, back), (h1, back)], 0.0, top),
        ([(h0, bb), (h1, bb), (h1, back), (h0, back)], 0.0, top),
        ([(h0, fb), (h1, fb), (h1, bb), (h0, bb)], hh, top),
    ] {
        g.block_ring(&r.map(|(a, b)| uv(a, b)), y0, y1);
    }
    // roof edge planting (green and brown mats hanging over the edge; no blossom up here in the photos)
    bougainvillea(g, uv(-0.5, fb), uv(3.0, fb), out, top, 1.4, 5, false);
    bougainvillea(g, uv(9.8, fb), uv(25.2, fb), out, top, 0.45, 6, false);
    bougainvillea(g, uv(a0, 3.0), uv(a0, 17.0), eout, top, 0.4, 7, false);

    // --- the taller pale volumes set back behind the face, and the block beyond
    for &(p0, p1, q0, q1, h) in &[(5.0f32, 8.0f32, 3.5f32, 13.0f32, 26.0f32), (8.0, 16.0, 3.5, 13.0, 28.5), (16.0, 40.0, 3.5, 13.0, 31.0), (16.0, 40.0, 13.0, 20.0, 23.0), (40.0, 58.0, 4.0, 20.0, 22.0)] {
        prism(g, m, &uv, pale, &[(p0, q0), (p1, q0), (p1, q1), (p0, q1)], top.min(h) - 0.2, &flat(h), false);
    }
    prism(g, m, &uv, pale, &[(a1, 3.5), (58.0, 3.5), (58.0, 20.0), (a1, 20.0)], 0.0, &flat(22.0), false);

    // --- the stone podium right of the hall, 4.3 m in front of the face and 8.76 m high: a kiosk panel on its side,
    // a sign and five poster panels on its front, red bougainvillea along its top, a terrace canopy, a hut, a screen
    let (pf, ptop) = (fb - 4.3, 8.76f32);
    prism(g, m, &uv, m.sandstone, &[(h1, pf), (56.0, pf), (56.0, fb), (h1, fb)], 0.0, &flat(ptop), false);
    g.quad(Key::new(Mat::Plain(120), [0.03, 0.07, 0.3]), v3(uv(h1, fb + 0.15) - t * 0.03, 0.73), -n3 * 1.35, Vec3::Y * 1.75);
    g.quad(Key::new(Mat::Plain(150), [0.7, 0.7, 0.68]), v3(uv(8.47, pf) + out * 0.03, 1.19), t3 * 0.58, Vec3::Y * 0.75);
    for k in 0..5 {
        let a = 9.32 + k as f32 * 1.8;
        let q = uv(a, pf) + out * 0.03;
        g.quad(Key::new(Mat::Plain(150), [0.035, 0.035, 0.035]), v3(q, 1.01), t3 * 1.63, Vec3::Y * 1.52);
        let pic = if k == 2 { [0.68, 0.63, 0.5] } else { [0.36, 0.29, 0.21] };
        g.quad(Key::new(Mat::Plain(170), pic), v3(q + t * 0.1 + out * 0.01, 1.11), t3 * 1.43, Vec3::Y * 1.32);
    }
    g.member(m.concrete, v3(uv(9.2, pf) + out * 0.12, 0.96), v3(uv(18.3, pf) + out * 0.12, 0.96), 0.24, 0.1, Vec3::Y);
    vines(g, uv(h1 + 0.1, pf), uv(14.0, pf), out, ptop, 2.3, 21, true);
    vines(g, uv(h1, pf + 0.1), uv(h1, fb - 0.1), -t, ptop, 2.0, 22, true);
    bougainvillea(g, uv(h1 + 0.1, pf), uv(21.0, pf), out, ptop, 1.1, 9, true);
    bougainvillea(g, uv(24.0, pf), uv(40.0, pf), out, ptop, 0.8, 10, true);
    bougainvillea(g, uv(h1, pf + 0.2), uv(h1, fb - 0.2), -t, ptop, 0.9, 11, true);
    let canopy = Key::new(Mat::Plain(225), [0.035, 0.035, 0.035]);
    g.member(canopy, v3(uv(8.4, pf + 0.95), 10.0), v3(uv(14.5, pf + 0.95), 10.0), 1.6, 0.14, Vec3::Y);
    for a in [8.5f32, 11.4, 14.4] {
        g.member(canopy, v3(uv(a, pf + 0.3), ptop - 1.0), v3(uv(a, pf + 0.3), 9.85), 0.08, 0.08, o3);
    }
    // the hut beside it: white walls, a dark gabled roof
    {
        let (x0, x1, z0, z1) = (14.7f32, 15.9f32, pf + 0.5, pf + 1.9);
        prism(g, m, &uv, m.white, &[(x0, z0), (x1, z0), (x1, z1), (x0, z1)], ptop - 1.0, &flat(10.0), false);
        let (e0, e1, rg) = (10.0f32, 10.7f32, (x0 + x1) / 2.0);
        for (xa, xb) in [(x0 - 0.15, rg), (x1 + 0.15, rg)] {
            let quad = [v3(uv(xa, z0 - 0.15), e0), v3(uv(xb, z0 - 0.15), e1), v3(uv(xb, z1 + 0.15), e1), v3(uv(xa, z1 + 0.15), e0)];
            let nn = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
            let quad = if nn.y < 0.0 { [quad[0], quad[3], quad[2], quad[1]] } else { quad };
            let nn = (quad[1] - quad[0]).cross(quad[2] - quad[0]).normalize();
            let r = Vec3::Y.cross(nn).normalize();
            g.poly(canopy, &quad, r, nn.cross(r));
        }
        g.poly(m.white, &[v3(uv(x0, z0), 10.0), v3(uv(x1, z0), 10.0), v3(uv(rg, z0), e1)], t3, Vec3::Y);
    }
    // the white screen along the terrace edge: a panel with four dark openings
    g.quad(m.white, v3(uv(16.3, pf + 0.15), ptop + 0.75), t3 * 4.4, Vec3::Y * 0.6);
    for k in 0..4 {
        g.quad(dark, v3(uv(16.55 + k as f32 * 1.07, pf + 0.15) + out * 0.02, ptop + 0.87), t3 * 0.82, Vec3::Y * 0.36);
    }
    // cream concrete bollards along the sidewalk in front
    let bollard = Key::new(Mat::Plain(175), [0.7, 0.66, 0.56]);
    let mut a = -10.0f32;
    while a <= 24.0 {
        let p = uv(a, pf - 4.7);
        g.tube(bollard, v3(p, -0.6), v3(p, 0.95), 0.16, 10);
        g.tube(bollard, v3(p, 0.95), v3(p, 1.02), 0.11, 10);
        g.block_box(Vec3::new(p.x - 0.16, 0.0, p.y - 0.16), Vec3::X * 0.32, Vec3::Z * 0.32, -0.6, 1.0);
        a += 2.2;
    }
}
