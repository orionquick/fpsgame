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
/// The pavilion under the park's amphitheatre hill beyond centre field: built here as the hill.
pub const PARK_PAVILION: &str = "way/1435588827";
/// The Western Metal Supply Co. building (its own OSM way), built here as part of the ballpark.
pub const WESTERN_METAL: &str = "way/769800588";
/// Home plate (world x, z): the centre of the dirt circle on the aerial.
const HOME: Vec2 = Vec2::new(-2.1, 8.1);
/// Seat model (folding seat with arms, 0.49 m wide) and the navy it is painted.
use crate::library::ALL_PARTS;
use crate::procmodel::{SEAT, SEAT_END, SEAT_MODULE, SEAT_NUMBERS, SEAT_ROW_PLATE};
/// Chairs stand with the middle of their standards this far in front of the riser behind them.
const SEAT_SETBACK: f32 = 0.24;

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
        // (grey: the smooth-concrete scan used before reads brown in this renderer)
        concrete: s("vlzlajnfw", [0.6, 0.59, 0.56]).tile(3.0).rough(0.8).wear(wear::STONE),
        riser: Key::new(Mat::Plain(200), [0.4, 0.39, 0.37]),
        stucco: s("tjeneevfw", [1.1, 0.92, 0.7]).tile(2.0),
        sandstone: s("pt_ashlar_sandstone", [1.0, 1.0, 1.0]).tile(3.0),
        steel: Key::new(Mat::Plain(110), [0.86, 0.87, 0.87]).wear(wear::METAL),
        // the park's navy, about half as light again as the seats (measured on a banner beside them in an overcast
        // photo). Dark paints are kept matt: with any gloss the sun's highlight outweighs so dark a colour, and the
        // fascia and the wall turn pale grey whenever the sun is behind the viewer
        navy: Key::new(Mat::Plain(235), [0.02, 0.031, 0.07]),
        pad: Key::new(Mat::Plain(242), [0.018, 0.032, 0.08]),
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
/// The main concourse, level with the podium outside the home plate frame (lidar: 7.70-7.75 m all round it).
const CONCOURSE_Y: f32 = 7.7;
/// The lower bowl's height where it passes under the terrace's face (lidar: 4.6 m at the last open row); from there
/// ten more rows climb under the overhang to the concourse.
const LB_BREAK_Y: f32 = 4.6;
const LB_EXT_ROWS: i32 = 10;
const TERR_Y: f32 = 11.6;
const UP_Y: f32 = 21.8;
const CANOPY_DEPTH: f32 = 12.0;
/// Spacing of the white steel rakers round the back of the bowl.
const BAY: f32 = 9.5;
/// The home plate gate's exposed frame (from the photos, scaled by people on its decks and the canopy height): the
/// frame's plane out from the upper deck's back, its bay, the upper concourse floor, the lettering's top, and the
/// truss chords (letter truss, roof truss top under the canopy).
const FRAME_OFF: f32 = 0.85;
const GZ_BAY: f32 = 11.0;
const GZ_UPPER_CONCOURSE: f32 = 23.5;
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
    wall_panels(&mut g, &m);
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
    east_building(&mut g, &mut f, &m, &|p: Vec2| c.terrain.at((p.x + HOME.x) as f64, (p.y + HOME.y) as f64) as f32);
    outfield(&mut g, &mut f, &m);
    backstop_net(&mut g, &m, &st);
    for b in [95.0f32, 265.0] {
        disc(&mut g, m.dirt, dir(b) * 9.5, 1.52, 0.02);
    }
    western_metal(&mut g, &mut f, &m, &|p: Vec2| c.terrain.at((p.x + HOME.x) as f64, (p.y + HOME.y) as f64) as f32);
    lights_and_boards(&mut g, &m);
    centre_field(&mut g, &m, &|p: Vec2| c.terrain.at((p.x + HOME.x) as f64, (p.y + HOME.y) as f64) as f32);
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

/// Pale broom-finished concrete slabs: plazas, the podium, the concourse.
fn paving() -> Key {
    Key::new(Mat::Surface("vlzlajnfw"), [0.64, 0.62, 0.58]).tile(3.0).rough(0.8)
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

/// What hangs on the outfield wall: the distances in white numerals and a run of message panels, none across the
/// hitter's background in dead centre. (The real wall carries sponsors' panels; these are the project's own words.)
fn wall_panels(g: &mut Geo, m: &Mats) {
    // the wall curves: a flat panel stands this far off it at its middle so its ends stay in front
    const PAD: f32 = 0.14;
    let white = Key::new(Mat::Plain(225), [0.82, 0.82, 0.8]);
    let ink = [0.03, 0.05, 0.16];
    let chalk = [0.95, 0.95, 0.93];
    let at = |b: f32| {
        let p = dir(b) * (interp(&WALL, b) - PAD);
        let e = (dir(b + 0.5) * interp(&WALL, b + 0.5) - dir(b - 0.5) * interp(&WALL, b - 0.5)).normalize();
        (p, e)
    };
    for (b, digits) in [(-42.5f32, [3usize, 3, 4]), (-27.0, [3, 5, 7]), (8.6, [3, 9, 6]), (25.5, [3, 9, 1]), (42.8, [3, 2, 2])] {
        let (p, e) = at(b);
        let h = wall_h(b);
        for (k, d) in digits.into_iter().enumerate() {
            sign(g, SGN_N[d - 1], chalk, p + e * ((k as f32 - 1.0) * 0.66) + dir(b) * (PAD - 0.03), h * 0.5 - 0.45, e, 0.56, 0.9);
        }
    }
    let words = [("sgn_ballpark", 4.4f32, 0.92f32), ("sgn_san_diego", 4.6, 0.85), ("sgn_play_ball", 4.3, 0.84), ("sgn_tickets", 3.9, 0.74)];
    let mut k = 0usize;
    for b in [-38.5f32, -34.5, -31.0, -23.0, -19.5, -16.5, -13.2, -10.0, 12.5, 16.0, 19.5, 22.5, 29.0, 32.5, 36.0, 39.5] {
        let (p, e) = at(b);
        let h = wall_h(b);
        let (w, ph) = (5.0f32, (h - 0.5).min(1.55));
        let y0 = (h - ph) / 2.0;
        let light = k % 2 == 1;
        let o = p - e * (w / 2.0);
        let n = Vec2::new(-e.y, e.x) * if Vec2::new(-e.y, e.x).dot(p) < 0.0 { 1.0 } else { -1.0 };
        let back = Vec3::new(-n.x, 0.0, -n.y) * PAD;
        g.quad(if light { white } else { m.navy }, Vec3::new(o.x, y0, o.y), Vec3::new(e.x, 0.0, e.y) * w, Vec3::Y * ph);
        // the pad's edges
        let (q0, q1) = (Vec3::new(o.x, y0, o.y), Vec3::new(o.x, y0, o.y) + Vec3::new(e.x, 0.0, e.y) * w);
        g.quad(m.navy, q0 + back, -back, Vec3::Y * ph);
        g.quad(m.navy, q1, back, Vec3::Y * ph);
        g.quad(m.navy, q0 + Vec3::Y * ph, Vec3::new(e.x, 0.0, e.y) * w, back);
        if !light {
            // a thin white keyline round the dark panels
            for (ya, yb) in [(y0, y0 + 0.05), (y0 + ph - 0.05, y0 + ph)] {
                let q = p - e * (w / 2.0) + n * 0.01;
                g.quad(white, Vec3::new(q.x, ya, q.y), Vec3::new(e.x, 0.0, e.y) * w, Vec3::Y * (yb - ya));
            }
        }
        let (id, sw, sh) = words[k % words.len()];
        let sh = sh.min(ph * 0.6);
        sign(g, id, if light { ink } else { chalk }, p + n * 0.02, y0 + (ph - sh) / 2.0, e, sw.min(w - 0.7), sh);
        k += 1;
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
    0.45 + (LB_BREAK_Y - 0.45) * t.powf(1.35)
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
        // which ends of this stretch of row stand on an aisle (or the end of the stand)
        let ends = (i == 0 || aisle((arc[i - 1] + arc[i]) / 2.0), i + 2 >= st.len() || aisle((arc[i + 1] + arc[i + 2]) / 2.0));
        // how far this stretch starts from its section's aisle
        let along = arc[i] - ((mid_s / 11.5).floor() * 11.5 + 0.65);
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
                seat_row(g, f, at(&a, n0 + LB_PITCH - SEAT_SETBACK), at(&b, n0 + LB_PITCH - SEAT_SETBACK), (ya + yb) / 2.0, -(a.n + b.n).normalize(), ends, along);
            }
        }
        let lba = 0.8 + rows as f32 * LB_PITCH;
        // --- the rows carry on under the terrace's overhang up to the main concourse
        let ext_rise = (CONCOURSE_Y - LB_BREAK_Y) / LB_EXT_ROWS as f32;
        for k in 0..LB_EXT_ROWS {
            let (n0, n1) = (lba + k as f32 * LB_PITCH, lba + (k + 1) as f32 * LB_PITCH);
            let y = LB_BREAK_Y + k as f32 * ext_rise;
            g.poly(m.concrete, &[v(at(&a, n0), y), v(at(&a, n1), y), v(at(&b, n1), y), v(at(&b, n0), y)], Vec3::new(a.n.x, 0.0, a.n.y), Vec3::new(e.x, 0.0, e.y));
            g.poly(m.riser, &[v(at(&a, n1), y), v(at(&b, n1), y), v(at(&b, n1), y + ext_rise), v(at(&a, n1), y + ext_rise)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
            g.block_ring(&[at(&a, n0), at(&a, n1), at(&b, n1), at(&b, n0)], 0.0, y);
            if !in_aisle {
                seat_row(g, f, at(&a, n0 + LB_PITCH - SEAT_SETBACK), at(&b, n0 + LB_PITCH - SEAT_SETBACK), y, -(a.n + b.n).normalize(), ends, along);
            }
        }
        // where the concourse begins
        let nc = lba + LB_EXT_ROWS as f32 * LB_PITCH;
        // --- terrace facia with an LED ribbon; its underside follows the rake (drawn below, once its depth is known)
        let (ta, tb) = (at(&a, a.terrace), at(&b, b.terrace));
        let t_under = TERR_Y - 0.9;
        g.poly(m.navy, &[v(tb, t_under), v(ta, t_under), v(ta, TERR_Y + 0.2), v(tb, TERR_Y + 0.2)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.poly(m.led, &[v(tb - b.n * 0.02, 10.9), v(ta - a.n * 0.02, 10.9), v(ta - a.n * 0.02, 11.5), v(tb - b.n * 0.02, 11.5)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        // --- terrace rows: 10, under the upper deck's overhang
        let t_rows = (((a.upper + 3.5) - (a.terrace + 0.3)) / TERR_PITCH).floor().clamp(4.0, 12.0) as i32;
        for j in 0..t_rows {
            let n0 = a.terrace + 0.3 + j as f32 * TERR_PITCH;
            let (n0a, n0b) = (n0, b.terrace + 0.3 + j as f32 * TERR_PITCH);
            let y = TERR_Y + j as f32 * TERR_RISE;
            step_row(g, m, &a, &b, n0a, n0b, TERR_PITCH, y, TERR_RISE, in_aisle);
            if !in_aisle {
                seat_row(g, f, at(&a, n0a + TERR_PITCH - SEAT_SETBACK), at(&b, n0b + TERR_PITCH - SEAT_SETBACK), y, -(a.n + b.n).normalize(), ends, along);
            }
        }
        let t_back_a = a.terrace + 0.3 + t_rows as f32 * TERR_PITCH;
        let t_back_b = b.terrace + 0.3 + t_rows as f32 * TERR_PITCH;
        let t_top = TERR_Y + t_rows as f32 * TERR_RISE;
        g.poly(m.soffit, &[v(ta, t_under), v(tb, t_under), v(at(&b, t_back_b), t_top - 0.9), v(at(&a, t_back_a), t_top - 0.9)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, -0.6, a.n.y).normalize());
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
                seat_row(g, f, at(&a, n0a + UP_PITCH - SEAT_SETBACK), at(&b, n0b + UP_PITCH - SEAT_SETBACK), y, -(a.n + b.n).normalize(), ends, along);
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
        // --- the main concourse: from the top of the lower rows back to the concourse blocks; at a gate stair to
        // the stair's head; behind home out through the frame onto the podium
        {
            let at_gate = GATES.iter().any(|&gb| (a.bearing - gb).abs() < 2.2);
            let gz0 = home_gate(a.bearing) && home_gate(b.bearing);
            // the podium stands outside the frame from about 138 to 186 degrees (lidar footprint)
            let podium = gz0 && a.bearing > 138.0 && b.bearing < 186.0;
            let (c1a, c1b) = if at_gate {
                (a.terrace + 9.0, b.terrace + 9.0)
            } else if podium {
                (up_back_a + 3.0, up_back_b + 3.0)
            } else if gz0 {
                (up_back_a + 0.35, up_back_b + 0.35)
            } else {
                (up_back_a - 6.0, up_back_b - 6.0)
            };
            // inner part, to the line of the concourse blocks; the outer part is left out where a stair core stands
            let (m_a, m_b) = (c1a.min(up_back_a - 6.0), c1b.min(up_back_b - 6.0));
            let q = [at(&a, nc), at(&a, m_a), at(&b, m_b), at(&b, nc)];
            poly_up(g, paving(), &q.map(|p| v(p, CONCOURSE_Y)));
            g.block_ring(&q, 0.0, CONCOURSE_Y);
            g.tube(m.rail, v(at(&a, nc), CONCOURSE_Y + 1.0), v(at(&b, nc), CONCOURSE_Y + 1.0), 0.03, 6);
            if c1a > m_a + 0.01 {
                let (ta4, tb4) = (at(&a, up_back_a + 4.2), at(&b, up_back_b + 4.2));
                for (f0, f1, hole) in runs(|fr| cores.iter().any(|c| c.covers(ta4.lerp(tb4, fr)))) {
                    if hole {
                        continue;
                    }
                    let (i0, i1) = (at(&a, m_a).lerp(at(&b, m_b), f0), at(&a, m_a).lerp(at(&b, m_b), f1));
                    let (o0, o1) = (at(&a, c1a).lerp(at(&b, c1b), f0), at(&a, c1a).lerp(at(&b, c1b), f1));
                    let q = [i0, o0, o1, i1];
                    poly_up(g, paving(), &q.map(|p| v(p, CONCOURSE_Y)));
                    g.block_ring(&q, 0.0, CONCOURSE_Y);
                    if !podium {
                        // an open edge over the street: a stone base below, a rail above
                        wall_poly(g, m.sandstone, &[v(o0, 0.0), v(o1, 0.0), v(o1, CONCOURSE_Y), v(o0, CONCOURSE_Y)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
                        for hh in [0.55f32, 1.05] {
                            g.tube(m.rail, v(o0, CONCOURSE_Y + hh), v(o1, CONCOURSE_Y + hh), 0.025, 5);
                        }
                        g.block_ring(&[o0, o1, o1 + b.n * 0.1, o0 + a.n * 0.1], CONCOURSE_Y, CONCOURSE_Y + 1.05);
                    }
                }
            }
        }
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
            // the upper deck's soffit height at offset r (it rises with the rake from the front to the back)
            let soffit = |s: &Station, ub: f32, r: f32| (UP_Y - 2.3) + (up_top - UP_Y) * ((r - s.upper) / (ub - s.upper)).clamp(0.0, 1.0);
            // decks above the main concourse (height, inner offset at a and b, outer offset at a and b, fascia
            // depth, as measured on the old head-on photo): the terrace concourse level with the terrace's top row;
            // the upper concourse, out from under the low end of the soffit; each out to the frame
            let mut decks = vec![];
            decks.push((t_tread + 0.25, t_back_a + 0.3, t_back_b + 0.3, up_back_a + edge, up_back_b + edge, 0.75));
            let (ri_a, ri_b) = (a.upper + 0.5 * (up_back_a - a.upper), b.upper + 0.5 * (up_back_b - b.upper));
            decks.push((GZ_UPPER_CONCOURSE, ri_a, ri_b, up_back_a + edge, up_back_b + edge, 1.0));
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
            // behind home the main concourse runs out through the frame (drawn above)
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
    grand_stair(g, f, m);
    home_towers(g, m);
    sw_offices(g, m);
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

/// A row of chairs standing on the line `a`-`b` (the middle of their standards' feet), facing `face`.
/// Each chair carries the standard on its left, so the next stretch of row closes this one; where the row stops
/// (`ends`: at `a`, at `b`) the last chair gets an end standard, and both ends show their aisle face.
/// `along` is how far `a` is from the section's aisle, which numbers the seats.
fn seat_row(g: &mut Geo, f: &mut Furnisher, a: Vec2, b: Vec2, y: f32, face: Vec2, ends: (bool, bool), along: f32) {
    let l = (b - a).length();
    let n = (l / SEAT_MODULE).round() as i32;
    if n < 1 {
        return;
    }
    let e = (b - a) / l;
    let mut fwd = Vec2::new(-e.y, e.x);
    if fwd.dot(face) < 0.0 {
        fwd = -fwd;
    }
    // chairs are set out along the model's +x
    let flip = Vec2::new(fwd.y, -fwd.x).dot(e) < 0.0;
    let (from, step, ends) = if flip { (b, -e, (ends.1, ends.0)) } else { (a, e, ends) };
    let w = l / n as f32;
    let sx = w / SEAT_MODULE;
    for k in 0..n {
        let d = w * (k as f32 + 0.5);
        let o = from + step * d;
        let number = ((along + if flip { l - d } else { d }) / SEAT_MODULE).floor().clamp(0.0, SEAT_NUMBERS as f32 - 1.0) as usize;
        // moulded plastic weathers a little unevenly from chair to chair
        let h = ((o.x * 12.9898 + o.y * 78.233 + y * 37.719).sin() * 43758.547).fract().abs();
        f.put_origin(g, SEAT, number, o, y, fwd, [sx, 1.0, 1.0], [0.93 + 0.14 * h; 3]);
    }
    if ends.1 {
        f.put_origin(g, SEAT_END, ALL_PARTS, from + step * l, y, fwd, [1.0; 3], [1.0; 3]);
    }
    if ends.0 {
        f.put_origin(g, SEAT_ROW_PLATE, ALL_PARTS, from, y, fwd, [1.0; 3], [1.0; 3]);
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
        let prev = arc;
        arc += (b.p - a.p).length();
        let on_aisle = |s: f32| ((s / 11.5).round() * 11.5 - s).abs() < 0.65;
        let in_aisle = on_aisle(arc);
        let ends = (i == 0 || on_aisle(prev), i + 2 >= st.len() || on_aisle(arc + (st[i + 2].p - b.p).length()));
        let along = prev - (((prev + arc) / 2.0 / 11.5).floor() * 11.5 + 0.65);
        for j in 0..rows {
            let n = n0 + j as f32 * LB_PITCH;
            let y = y0 + j as f32 * rise;
            step_row(g, m, &a, &b, n, n, LB_PITCH, y, rise, in_aisle && rise > 0.3);
            if !in_aisle {
                seat_row(g, f, at(&a, n + LB_PITCH - SEAT_SETBACK), at(&b, n + LB_PITCH - SEAT_SETBACK), y, -(a.n + b.n).normalize(), ends, along);
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
        wall_poly(g, m.sandstone, &[v(a.p + a.n * 18.0, 0.0), v(b.p + b.n * 18.0, 0.0), v(b.p + b.n * 18.0, CF_TERR), v(a.p + a.n * 18.0, CF_TERR)], -Vec3::new(e.x, 0.0, e.y), Vec3::Y);
    }
    for (bb, k) in [(-13.0f32, 0), (-8.0, 1)] {
        let p = dir(bb) * (interp(&WALL, bb) + 9.0);
        let _ = k;
        disc(g, m.dirt, p, 2.7, 0.03);
        mound(g, m, p);
    }
    // centre-field terrace at 4 m (grass) from 6 m behind the wall; the batter's eye stands on it (`centre_field`)
    let cf = wall_stations(-5.5, 17.0);
    for w in cf.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n0 = if a.bearing < 6.0 { 6.0 } else { 13.0 };
        poly_up(g, m.grass_dark, &[v(a.p + a.n * n0, CF_TERR), v(b.p + b.n * n0, CF_TERR), v(b.p + b.n * 45.0, CF_TERR), v(a.p + a.n * 45.0, CF_TERR)]);
        let e = (b.p - a.p).normalize();
        // dead centre the hitter's background is kept dark all the way down
        let face = if a.bearing > -7.5 && a.bearing < 7.5 { eye_dark() } else { m.sandstone };
        wall_poly(g, face, &[v(b.p + b.n * n0, 0.0), v(a.p + a.n * n0, 0.0), v(a.p + a.n * n0, CF_TERR), v(b.p + b.n * n0, CF_TERR)], -Vec3::new(e.x, 0.0, e.y) * -1.0, Vec3::Y);
        poly_up(g, m.grass_dark, &[v(a.p + a.n * 0.45, 0.01), v(b.p + b.n * 0.45, 0.01), v(b.p + b.n * n0, 0.01), v(a.p + a.n * n0, 0.01)]);
        g.block_ring(&[a.p + a.n * n0, b.p + b.n * n0, b.p + b.n * 45.0, a.p + a.n * 45.0], 0.0, CF_TERR);
    }
}

// ---------------------------------------------------------------- centre field and the park beyond
// From the lidar surface and the aerial image (positions, sizes, levels) and a 2019 photo from the upper deck
// (what each thing is). World coordinates in the comments; `w` turns them into the ballpark's frame.

/// The terrace behind centre field (lidar 3-4 m; just over the surveyed ground, which would otherwise cover it).
const CF_TERR: f32 = 4.0;

fn eye_dark() -> Key {
    Key::new(Mat::Plain(245), [0.012, 0.016, 0.014])
}

/// The park's hill: the lidar surface on a 3 m grid (world x from -3, z from -213; the 20th-percentile height in
/// each cell, median-filtered and capped at the plateau so trees and people drop out). A level crescent at 8.7 m
/// over the pavilion in the north-east, lawn sloping down from it to the south-west, a ramp climbing along its
/// north-west side, a wall where it drops to the lawn.
const HILL_GRID: (f32, f32, f32) = (-3.0, -213.0, 3.0);
const HILL: [[f32; 22]; 22] = [
    [4.3, 4.3, 4.3, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.4, 4.7, 4.8, 4.8, 4.8, 4.8, 4.9, 4.9],
    [4.2, 4.2, 4.4, 4.6, 4.9, 5.1, 5.3, 5.3, 5.4, 5.6, 5.6, 5.2, 5.1, 4.3, 4.3, 4.2, 4.3, 4.8, 4.8, 4.8, 4.8, 4.9],
    [4.2, 4.5, 4.9, 5.5, 6.1, 6.5, 6.6, 6.7, 6.9, 7.1, 7.3, 7.4, 7.4, 5.1, 4.2, 4.2, 4.2, 4.6, 5.2, 4.9, 4.9, 4.9],
    [4.6, 5.3, 6.2, 6.4, 6.5, 6.6, 8.3, 8.3, 8.3, 7.9, 8.7, 8.7, 8.7, 8.7, 4.3, 4.2, 4.2, 4.5, 5.2, 5.2, 4.9, 4.9],
    [6.3, 6.3, 6.4, 6.5, 6.5, 8.4, 9.3, 9.3, 9.3, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.6, 4.2, 4.2, 4.5, 4.9, 4.9, 4.9],
    [6.3, 6.4, 6.4, 6.5, 7.0, 8.6, 9.3, 9.3, 9.3, 8.7, 8.6, 8.7, 8.7, 8.7, 8.7, 8.7, 8.6, 4.4, 4.5, 4.7, 4.8, 4.9],
    [6.1, 6.1, 6.4, 6.4, 6.5, 8.6, 8.6, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.6, 4.8, 4.7, 4.8, 4.8],
    [5.6, 5.5, 5.5, 5.4, 5.3, 5.1, 4.8, 4.4, 8.7, 8.7, 8.7, 8.7, 8.7, 8.7, 8.6, 8.6, 8.6, 8.6, 8.6, 4.8, 4.8, 4.8],
    [5.2, 5.1, 4.9, 4.8, 4.7, 4.5, 4.2, 4.1, 4.2, 4.2, 8.7, 8.7, 8.6, 8.6, 8.6, 8.6, 8.6, 8.6, 8.6, 4.9, 4.7, 4.8],
    [4.7, 4.6, 4.5, 4.4, 4.3, 4.0, 4.0, 4.1, 4.1, 4.3, 4.9, 5.8, 8.6, 8.6, 8.6, 8.6, 8.6, 8.6, 8.6, 4.9, 4.7, 4.7],
    [4.2, 4.1, 4.1, 4.1, 4.0, 4.0, 4.0, 4.1, 4.3, 4.8, 5.4, 6.0, 6.6, 8.7, 8.6, 8.7, 8.6, 8.6, 6.7, 5.4, 4.7, 4.7],
    [3.9, 3.9, 4.0, 3.9, 3.9, 4.0, 4.0, 4.3, 4.7, 5.2, 5.6, 6.0, 6.6, 7.2, 8.6, 8.6, 8.6, 8.1, 7.1, 5.7, 4.7, 4.8],
    [3.9, 4.0, 4.0, 3.9, 3.9, 4.0, 4.1, 4.3, 4.7, 5.2, 5.6, 6.0, 6.5, 7.1, 7.7, 8.1, 8.1, 7.8, 7.3, 6.1, 4.7, 4.8],
    [4.0, 4.0, 4.0, 4.0, 4.0, 4.1, 4.1, 4.1, 4.5, 4.9, 5.2, 5.7, 6.3, 6.8, 7.4, 7.7, 7.7, 7.4, 7.3, 6.5, 4.7, 4.8],
    [3.8, 3.9, 4.0, 4.0, 4.0, 4.1, 4.1, 4.1, 4.1, 4.6, 4.9, 5.3, 6.0, 6.5, 7.0, 7.4, 7.4, 7.3, 6.9, 6.5, 4.7, 4.7],
    [3.8, 3.9, 4.0, 4.0, 3.9, 4.0, 4.1, 4.1, 4.0, 4.3, 4.7, 5.2, 5.8, 6.2, 6.5, 7.0, 7.0, 6.9, 6.5, 6.1, 4.7, 4.7],
    [3.7, 3.9, 4.0, 3.9, 3.9, 4.1, 4.1, 4.2, 4.1, 4.0, 4.6, 5.1, 5.4, 5.9, 6.2, 6.5, 6.5, 6.4, 6.1, 5.6, 4.7, 4.7],
    [3.7, 4.0, 3.8, 3.8, 3.8, 4.1, 4.2, 4.2, 4.2, 4.0, 4.5, 4.7, 5.3, 5.5, 5.9, 6.1, 6.1, 5.9, 5.6, 5.1, 4.7, 4.7],
    [3.9, 3.9, 4.0, 3.9, 3.9, 4.0, 4.1, 4.2, 4.2, 4.1, 4.2, 4.5, 5.0, 5.3, 5.5, 5.6, 5.6, 5.5, 5.1, 4.6, 4.7, 4.7],
    [3.9, 3.9, 3.9, 3.8, 3.9, 3.9, 4.1, 4.1, 4.1, 4.0, 4.2, 4.5, 4.6, 5.0, 5.1, 5.2, 5.2, 5.0, 4.8, 4.6, 4.6, 4.6],
    [3.9, 3.9, 3.9, 3.8, 3.8, 3.8, 3.9, 4.1, 4.0, 4.0, 4.0, 4.3, 4.5, 4.6, 4.7, 4.8, 4.8, 4.8, 4.5, 4.5, 4.6, 4.6],
    [3.6, 3.7, 3.7, 3.7, 3.7, 3.7, 3.8, 3.8, 3.9, 3.9, 3.9, 4.1, 4.3, 4.4, 4.5, 4.7, 4.7, 4.4, 4.4, 4.5, 4.5, 4.6],
];

/// The plateau's level, and the grid height above which a point belongs to it.
const HILL_TOP: f32 = 8.7;
const HILL_EDGE: f32 = 8.3;

/// The hill without its plateau: plateau nodes take the lowest ground beside them, so the lawn runs level to the
/// foot of the plateau's wall instead of ramping up it (except where the lawn reaches the top by itself).
fn hill_low(i: usize, j: usize) -> f32 {
    let h = HILL[j][i];
    if h < HILL_EDGE {
        return h;
    }
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for dj in -1i32..=1 {
        for di in -1i32..=1 {
            let (a, b) = (i as i32 + di, j as i32 + dj);
            if a >= 0 && b >= 0 && (a as usize) < HILL[0].len() && (b as usize) < HILL.len() && HILL[b as usize][a as usize] < HILL_EDGE {
                low = low.min(HILL[b as usize][a as usize]);
                high = high.max(HILL[b as usize][a as usize]);
            }
        }
    }
    // where the lawn itself climbs to within a step or two of the top it runs on up to it: no wall there
    if low == f32::MAX || high > 7.2 { HILL_TOP } else { low }
}

/// Bilinear sample of a node function over the hill's grid at a world point.
fn hill_sample(f: &dyn Fn(usize, usize) -> f32, x: f32, z: f32) -> Option<f32> {
    let (x0, z0, st) = HILL_GRID;
    let (u, v) = ((x - x0) / st, (z - z0) / st);
    if u < 0.0 || v < 0.0 || u > (HILL[0].len() - 1) as f32 || v > (HILL.len() - 1) as f32 {
        return None;
    }
    let (i, j) = ((u as usize).min(HILL[0].len() - 2), (v as usize).min(HILL.len() - 2));
    let (fu, fv) = (u - i as f32, v - j as f32);
    Some((f(i, j) * (1.0 - fu) + f(i + 1, j) * fu) * (1.0 - fv) + (f(i, j + 1) * (1.0 - fu) + f(i + 1, j + 1) * fu) * fv)
}

/// The park beyond centre field (world x0, x1, z0, z1): its ground is built here, not by the city's ground mesh.
const PARK: (f32, f32, f32, f32) = (-45.0, 62.0, -214.0, -143.5);

pub fn park_ring() -> Vec<[f64; 2]> {
    let (x0, x1, z0, z1) = (PARK.0 as f64, PARK.1 as f64, PARK.2 as f64, PARK.3 as f64);
    vec![[x0, z0], [x1, z0], [x1, z1], [x0, z1]]
}

fn centre_field(g: &mut Geo, m: &Mats, terr: &dyn Fn(Vec2) -> f32) {
    let w = |x: f32, z: f32| Vec2::new(x, z) - HOME;
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let dark = eye_dark();
    let sand = Key::new(Mat::Plain(240), [0.4, 0.33, 0.22]);
    let hedge = Key::new(Mat::Plain(240), [0.04, 0.1, 0.035]);

    // --- the batter's eye (lidar: 26 x 17 m, its roof 14 m at the field edge rising to 18 m behind): a black
    // block, a deck on it, and a metal roof on white posts that tips toward the field, the name across it
    let (ex0, ex1, ez0, ez1) = (-14.0 - HOME.x, 12.0 - HOME.x, -143.0 - HOME.y, -126.0 - HOME.y);
    let deck = 10.8;
    g.solid(dark, Vec3::new(ex0, 0.0, ez0), Vec3::new(ex1, deck, ez1));
    for (a, b) in [(Vec2::new(ex0, ez1), Vec2::new(ex1, ez1)), (Vec2::new(ex1, ez1), Vec2::new(ex1, ez0)), (Vec2::new(ex1, ez0), Vec2::new(ex0, ez0)), (Vec2::new(ex0, ez0), Vec2::new(ex0, ez1))] {
        let inw = ((Vec2::new(ex0, ez0) + Vec2::new(ex1, ez1)) / 2.0 - (a + b) / 2.0).normalize() * 0.25;
        face(g, dark, a, b, -inw, deck, deck + 1.1, &[]);
        face(g, dark, a + inw, b + inw, inw, deck, deck + 1.1, &[]);
        poly_up(g, dark, &[v(a, deck + 1.1), v(b, deck + 1.1), v(b + inw, deck + 1.1), v(a + inw, deck + 1.1)]);
    }
    let roof = Key::new(Mat::Surface("pht_corrugated_iron_02"), [1.0, 0.98, 1.22]).tile(2.7);
    let (rx0, rx1, rz0, rz1, ry_front, ry_back) = (ex0 - 1.5, ex1 + 1.5, ez0 - 1.0, ez1 + 1.2, 14.0f32, 18.2f32);
    let slope = Vec3::new(0.0, ry_back - ry_front, rz0 - rz1);
    let up = slope.normalize();
    let nrm = Vec3::X.cross(up);
    let (c0, c1, c2, c3) = (Vec3::new(rx0, ry_front, rz1), Vec3::new(rx1, ry_front, rz1), Vec3::new(rx1, ry_back, rz0), Vec3::new(rx0, ry_back, rz0));
    g.poly(roof, &[c0, c1, c2, c3], Vec3::X, up);
    g.poly(m.white, &[c1 - nrm * 0.3, c0 - nrm * 0.3, c3 - nrm * 0.3, c2 - nrm * 0.3], -Vec3::X, up);
    g.quad(m.white, c0 - nrm * 0.3, Vec3::X * (rx1 - rx0), nrm * 0.3);
    g.quad(m.white, c2 - nrm * 0.3, -Vec3::X * (rx1 - rx0), nrm * 0.3);
    for k in 0..5 {
        let x = ex0 + 0.4 + (ex1 - ex0 - 0.8) * k as f32 / 4.0;
        for (z, y) in [(ez1 - 0.4, ry_front + (ry_back - ry_front) * 0.09), (ez0 + 0.4, ry_back - (ry_back - ry_front) * 0.07)] {
            g.member(m.white, Vec3::new(x, deck, z), Vec3::new(x, y - 0.3, z), 0.3, 0.3, Vec3::Z);
        }
    }
    // the name on the roof, read from home plate
    let (sw, sh) = (19.0f32, 4.2f32);
    let o = c0 + Vec3::X * ((rx1 - rx0 - sw) / 2.0) + up * (slope.length() * 0.5 - sh / 2.0) + nrm * 0.04;
    g.decal(Key::new(Mat::Surface("sgn_ballpark"), [0.07, 0.008, 0.03]).tile(1.0), o - Vec3::X * (sh / 24.0) - up * (sh / 24.0), Vec3::X * (sw + sh / 12.0), up * (sh * 13.0 / 12.0), 1.0);
    // clipped hedges either side of the black
    for (xa, xb) in [(ex0 - 4.0, ex0 - 0.2), (ex1 + 0.2, ex1 + 4.0)] {
        g.cuboid(hedge, Vec3::new(xa, CF_TERR - 0.1, ez1 - 1.6), Vec3::new(xb, 8.6, ez1 + 1.6));
    }

    // --- left of it: a terrace bar at 5 m with a kiosk and parasols, big free-standing letters over it (lidar: a
    // 12 m long frame 12 m high) and a row of flagpoles behind
    let (tx0, tx1, tz0, tz1) = (-36.0 - HOME.x, -16.5 - HOME.x, -141.0 - HOME.y, -127.5 - HOME.y);
    g.solid(m.concrete, Vec3::new(tx0, 3.4, tz0), Vec3::new(tx1, 5.0, tz1));
    wall_holes(g, m.sandstone, Vec2::new(tx0, tz1 + 0.02), Vec2::new(tx1, tz1 + 0.02), 3.4, 5.0, &[]);
    rail_run(g, m, Vec2::new(tx0, tz1 - 0.15), Vec2::new(tx1, tz1 - 0.15), 5.0, 1.05);
    g.cuboid(Key::new(Mat::Plain(200), [0.12, 0.12, 0.13]), Vec3::new(tx0 + 2.0, 5.0, tz0 + 0.3), Vec3::new(tx0 + 11.0, 8.0, tz0 + 3.0));
    let awning = Key::new(Mat::Plain(220), [0.85, 0.62, 0.08]);
    g.poly(awning, &[Vec3::new(tx0 + 1.6, 7.2, tz0 + 4.6), Vec3::new(tx0 + 11.4, 7.2, tz0 + 4.6), Vec3::new(tx0 + 11.4, 8.0, tz0 + 3.0), Vec3::new(tx0 + 1.6, 8.0, tz0 + 3.0)], Vec3::X, Vec3::new(0.0, 0.45, -0.9).normalize());
    let red = Key::new(Mat::Plain(230), [0.55, 0.05, 0.05]);
    for k in 0..8 {
        let c = Vec2::new(tx0 + 2.6 + (k / 2) as f32 * 4.6, tz1 - 2.6 - (k % 2) as f32 * 4.2);
        g.tube(m.rail, v(c, 5.0), v(c, 7.5), 0.025, 4);
        for s in 0..8 {
            let (a0, a1) = (s as f32 / 8.0 * std::f32::consts::TAU, (s + 1) as f32 / 8.0 * std::f32::consts::TAU);
            let (p0, p1) = (c + Vec2::new(a0.cos(), a0.sin()) * 1.5, c + Vec2::new(a1.cos(), a1.sin()) * 1.5);
            let mid = ((p0 + p1) / 2.0 - c).normalize();
            g.poly(red, &[v(p0, 7.0), v(p1, 7.0), v(c, 7.55)], Vec3::new(-mid.y, 0.0, mid.x), Vec3::new(-mid.x, 0.4, -mid.y).normalize());
        }
    }
    let (lx0, lx1, lz) = (-30.5 - HOME.x, -17.5 - HOME.x, -135.0 - HOME.y);
    for x in [lx0, (lx0 + lx1) / 2.0, lx1] {
        g.member(m.white, Vec3::new(x, 5.0, lz - 0.25), Vec3::new(x, 12.0, lz - 0.25), 0.22, 0.22, Vec3::Z);
    }
    for y in [9.25f32, 12.0] {
        g.member(m.white, Vec3::new(lx0, y, lz - 0.25), Vec3::new(lx1, y, lz - 0.25), 0.16, 0.16, Vec3::Z);
    }
    sign(g, "sgn_san_diego", [0.95, 0.95, 0.93], Vec2::new((lx0 + lx1) / 2.0, lz), 9.5, Vec2::X, lx1 - lx0 - 0.6, 2.3);
    for k in 0..5 {
        let c = Vec2::new(tx0 + 1.5 + k as f32 * 4.4, tz0 - 1.5);
        let gy = terr(c);
        g.tube(m.steel, v(c, gy), v(c, gy + 17.0), 0.07, 6);
        let cloth = Key::new(Mat::Plain(230), [[0.6, 0.06, 0.06], [0.85, 0.85, 0.83], [0.05, 0.1, 0.32]][k % 3]);
        g.quad(cloth, v(c, gy + 15.6) + Vec3::X * 0.08, Vec3::new(0.94, 0.0, 0.34) * 1.9, Vec3::Y * 1.2);
        g.quad(cloth, v(c, gy + 15.6) + Vec3::X * 0.08 + Vec3::new(0.94, 0.0, 0.34) * 1.9, -Vec3::new(0.94, 0.0, 0.34) * 1.9, Vec3::Y * 1.2);
    }

    // --- right of it: the sand terrace behind the fence, named on its wall
    let beach = wall_stations(6.5, 16.5);
    for s in beach.windows(2) {
        let (a, b) = (s[0], s[1]);
        poly_up(g, sand, &[v(a.p + a.n * 13.2, CF_TERR + 0.05), v(b.p + b.n * 13.2, CF_TERR + 0.05), v(b.p + b.n * 36.0, CF_TERR + 0.05), v(a.p + a.n * 36.0, CF_TERR + 0.05)]);
        rail_run(g, m, a.p + a.n * 13.15, b.p + b.n * 13.15, CF_TERR, 1.05);
    }
    let (ba, bb) = (beach[beach.len() / 2 - 2], beach[beach.len() / 2 + 2]);
    let (pa, pb) = (ba.p + ba.n * 12.96, bb.p + bb.n * 12.96);
    let e = (pb - pa).normalize();
    g.quad(m.navy, v(pa, CF_TERR - 1.2), Vec3::new(e.x, 0.0, e.y) * (pb - pa).length(), Vec3::Y * 1.1);
    sign(g, "sgn_the_beach", [0.95, 0.95, 0.93], (pa + pb) / 2.0 - Vec2::new(-e.y, e.x) * 0.03, CF_TERR - 0.98, -e, (pb - pa).length() * 0.7, 0.66);

    // --- the walk behind centre field (world z -150..-143.5) and the paved walk up the park's west side
    let drape = |g: &mut Geo, k: Key, x0: f32, x1: f32, z0: f32, z1: f32, step: f32, lift: f32, skip: &dyn Fn(f32, f32) -> bool| {
        let (nx, nz) = (((x1 - x0) / step).ceil() as i32, ((z1 - z0) / step).ceil() as i32);
        for i in 0..nx {
            for j in 0..nz {
                let (xa, xb) = (x0 + (x1 - x0) * i as f32 / nx as f32, x0 + (x1 - x0) * (i + 1) as f32 / nx as f32);
                let (za, zb) = (z0 + (z1 - z0) * j as f32 / nz as f32, z0 + (z1 - z0) * (j + 1) as f32 / nz as f32);
                if skip((xa + xb) / 2.0, (za + zb) / 2.0) {
                    continue;
                }
                let q = [w(xa, zb), w(xb, zb), w(xb, za), w(xa, za)].map(|p| v(p, terr(p) + lift));
                g.poly(k, &[q[0], q[1], q[2]], Vec3::X, -Vec3::Z);
                g.poly(k, &[q[0], q[2], q[3]], Vec3::X, -Vec3::Z);
            }
        }
    };
    let never = |_: f32, _: f32| false;
    // the surveyed ground is cut out over the park (`park_ring`): these strips are all there is underfoot
    let pavers = Key::new(Mat::Surface("vlzlajnfw"), [0.8, 0.52, 0.45]).tile(1.5).rough(0.85);
    let (px0, px1, pz0, pz1) = (PARK.0, PARK.1, PARK.2, PARK.3);
    drape(g, paving(), px0, px1, -150.0, pz1, 5.0, 0.0, &never);
    drape(g, paving(), px0, -40.0, pz0, -150.0, 5.0, 0.0, &never);
    drape(g, pavers, -40.0, -33.5, pz0, -150.0, 5.0, 0.0, &never);
    drape(g, paving(), 58.0, px1, pz0, -150.0, 5.0, 0.0, &never);

    // --- the park: lawn, the children's ball field (aerial: x -33..-10, z -172..-150), the hill
    let (hx0_w, hz0_w) = (HILL_GRID.0, HILL_GRID.1);
    let hx1_w = hx0_w + HILL_GRID.2 * (HILL[0].len() - 1) as f32;
    let field = |x: f32, z: f32| x > -33.0 && x < -10.0 && z > -172.0 && z < -150.0;
    drape(g, m.grass, -33.5, hx0_w + 3.0, pz0, -150.0, 4.0, 0.0, &|x, z| field(x, z));
    drape(g, m.grass, hx0_w + 3.0, hx1_w - 3.0, pz0, hz0_w + 3.0, 3.0, 0.0, &never);
    drape(g, m.grass, hx0_w + 3.0, hx1_w - 3.0, -153.0, -150.0, 3.0, 0.0, &never);
    drape(g, m.grass, hx1_w - 3.0, 58.0, pz0, -150.0, 3.0, 0.0, &never);
    drape(g, m.grass_dark, -33.0, -10.0, -172.0, -150.0, 4.0, 0.0, &never);
    // base paths and mound in infield dirt, a low fence round it
    let (fx0, fx1, fz0, fz1) = (-31.5f32, -12.0f32, -170.5f32, -151.0f32);
    let lift = |p: Vec2| v(p, terr(p) + 0.04);
    for (a, b) in [((fx0, fz1), (fx1, fz1)), ((fx1, fz1), (fx1, fz0)), ((fx1, fz0), (fx0, fz0)), ((fx0, fz0), (fx0, fz1))] {
        let (pa, pb) = (w(a.0, a.1), w(b.0, b.1));
        let e = (pb - pa).normalize();
        let inw = Vec2::new(-e.y, e.x) * if Vec2::new(-e.y, e.x).dot(w((fx0 + fx1) / 2.0, (fz0 + fz1) / 2.0) - pa) > 0.0 { 1.8 } else { -1.8 };
        poly_up(g, m.dirt, &[lift(pa), lift(pb), lift(pb + inw), lift(pa + inw)]);
        rail_run(g, m, w(a.0, a.1) - inw * 0.6, w(b.0, b.1) - inw * 0.6, terr(pa), 1.1);
    }
    disc(g, m.dirt, w((fx0 + fx1) / 2.0, (fz0 + fz1) / 2.0), 1.6, terr(w((fx0 + fx1) / 2.0, (fz0 + fz1) / 2.0)) + 0.04);

    // the hill, from its lidar grid. Each grid square is split along the plateau's edge (where the surveyed height
    // crosses `HILL_EDGE`): the part above is the level top, the part below is lawn, and a stone wall stands on
    // the line between them. The square on top is paved (aerial: x 35..47, z -185..-177).
    let (hx0, hz0, hst) = HILL_GRID;
    let (ni, nj) = (HILL[0].len(), HILL.len());
    let raw = |i: usize, j: usize| HILL[j][i];
    let ground = |x: f32, z: f32| -> f32 {
        let t = terr(w(x, z));
        let edge = x < hx0 + 0.01 || z < hz0 + 0.01 || x > hx0 + hst * (ni - 1) as f32 - 0.01 || z > hz0 + hst * (nj - 1) as f32 - 0.01;
        // the sheet's rim dips under the lawn laid round it (which overlaps its outer ring), so no gap can open
        // where the two are sampled differently
        if edge { t - 0.6 } else { hill_sample(&hill_low, x, z).unwrap_or(t).max(t) }
    };
    // on the plateau's edge the lawn either stops at the foot of the wall or, within a metre of the top, meets it
    let at_cut = |x: f32, z: f32| { let y = ground(x, z); if HILL_TOP - y < 1.0 { HILL_TOP } else { y } };
    let stone = stone_flat();
    for j in 0..nj - 1 {
        for i in 0..ni - 1 {
            // corners in order round the square, with their surveyed heights
            let c = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)].map(|(a, b)| (Vec2::new(hx0 + a as f32 * hst, hz0 + b as f32 * hst), raw(a, b)));
            let (mut top, mut low, mut cut): (Vec<Vec2>, Vec<Vec2>, Vec<Vec2>) = (vec![], vec![], vec![]);
            for k in 0..4 {
                let ((p, h), (q, hq)) = (c[k], c[(k + 1) % 4]);
                if h >= HILL_EDGE { top.push(p) } else { low.push(p) }
                if (h >= HILL_EDGE) != (hq >= HILL_EDGE) {
                    let x = p.lerp(q, (HILL_EDGE - h) / (hq - h));
                    top.push(x);
                    low.push(x);
                    cut.push(x);
                }
            }
            let paved = |p: &[Vec2]| { let m = p.iter().copied().sum::<Vec2>() / p.len() as f32; m.x > 35.0 && m.x < 47.0 && m.y > -185.0 && m.y < -177.0 };
            if top.len() >= 3 {
                let pts: Vec<Vec3> = top.iter().rev().map(|p| v(w(p.x, p.y), HILL_TOP)).collect();
                g.poly(if paved(&top) { paving() } else { m.grass }, &pts, Vec3::X, -Vec3::Z);
                g.block_ring(&top.iter().map(|p| w(p.x, p.y)).collect::<Vec<_>>(), 0.0, HILL_TOP);
            }
            if low.len() >= 3 {
                let pts: Vec<Vec3> = low.iter().rev().map(|p| v(w(p.x, p.y), if cut.contains(p) { at_cut(p.x, p.y) } else { ground(p.x, p.y) })).collect();
                for t in 1..pts.len() - 1 {
                    g.poly(m.grass, &[pts[0], pts[t], pts[t + 1]], Vec3::X, -Vec3::Z);
                }
            }
            if cut.len() == 2 {
                // the wall, facing the low side, with a kerb standing over the top
                let (a, b) = (cut[0], cut[1]);
                let e = (b - a).normalize_or_zero();
                let lowc = low.iter().copied().sum::<Vec2>() / low.len().max(1) as f32;
                let out = Vec2::new(-e.y, e.x) * if Vec2::new(-e.y, e.x).dot(lowc - a) > 0.0 { 1.0 } else { -1.0 };
                let (ya, yb) = (at_cut(a.x, a.y), at_cut(b.x, b.y));
                if HILL_TOP - ya.min(yb) > 0.25 {
                    let (l, r) = if Vec3::new(e.x, 0.0, e.y).cross(Vec3::Y).dot(Vec3::new(out.x, 0.0, out.y)) > 0.0 { (a, b) } else { (b, a) };
                    let (yl, yr) = if l == a { (ya, yb) } else { (yb, ya) };
                    let (pl, pr) = (w(l.x, l.y), w(r.x, r.y));
                    let r3 = Vec3::new((pr - pl).normalize().x, 0.0, (pr - pl).normalize().y);
                    g.poly(stone, &[v(pl, yl - 0.2), v(pr, yr - 0.2), v(pr, HILL_TOP + 0.45), v(pl, HILL_TOP + 0.45)], r3, Vec3::Y);
                    let inw = -out * 0.3;
                    g.poly(stone, &[v(pr + inw, HILL_TOP), v(pl + inw, HILL_TOP), v(pl + inw, HILL_TOP + 0.45), v(pr + inw, HILL_TOP + 0.45)], -r3, Vec3::Y);
                    poly_up(g, stone, &[v(pl, HILL_TOP + 0.45), v(pr, HILL_TOP + 0.45), v(pr + inw, HILL_TOP + 0.45), v(pl + inw, HILL_TOP + 0.45)]);
                }
            }
        }
    }
    // the lawn's collision, in 1 m steps so even its steepest part (1 in 3) can be walked
    let cell = 1.0f32;
    let (nx, nz) = ((hst * (ni - 1) as f32 / cell).round() as i32, (hst * (nj - 1) as f32 / cell).round() as i32);
    for j in 0..nz {
        for i in 0..nx {
            let (xa, za) = (hx0 + i as f32 * cell, hz0 + j as f32 * cell);
            let (cx, cz) = (xa + cell / 2.0, za + cell / 2.0);
            if hill_sample(&raw, cx, cz).is_some_and(|h| h < HILL_EDGE) {
                let y = [(xa, za), (xa + cell, za), (xa + cell, za + cell), (xa, za + cell)].iter().map(|&(x, z)| ground(x, z)).fold(ground(cx, cz), f32::max);
                g.block_ring(&[w(xa, za), w(xa + cell, za), w(xa + cell, za + cell), w(xa, za + cell)], 0.0, y.min(HILL_TOP).max(terr(w(cx, cz))));
            }
        }
    }
    // a plinth on the square
    let sy = HILL_TOP;
    g.solid(pale_precast(), v(w(40.2, -181.8), sy), v(w(41.8, -180.2), sy + 1.3));
}

// ---------------------------------------------------------------- Western Metal Supply Co., towers, boards

/// The old metal-supply warehouse in the left-field corner, the foul pole on its south-east corner. Measured on a
/// long-lens photo square to its field face (41 px per metre across the 32.1 m face, heights tied to the lidar: roof
/// 23 m, top balcony 17.1 m) and on the lidar for the roof:
/// - red-brown brick, four storeys; a black painted band 2.3 m deep under the parapet carries the name in white;
/// - the field face has seven bays at 4.43 m, each opening 2.85 m wide with dark green steel sash set back in the
///   brick: floors at 6.6, 11.4 and 17.1 m;
/// - white concrete balconies on tapered brackets: one the whole length at the top floor (standing room), three
///   below it and two below those, each with two rows of seats;
/// - on the roof (23 m) a rail all round, the stair penthouse to 31 m and two blocks of bleachers rising west to 28 m;
/// - the street faces keep their punched windows; a pale stair tower and a brick annex stand against the north face.
/// The name on the band is a stand-in (no real business names in the game).
fn western_metal(g: &mut Geo, f: &mut Furnisher, m: &Mats, terr: &dyn Fn(Vec2) -> f32) {
    let brick = Key::new(Mat::Surface("ufmhehts"), WM_BRICK).tile(2.0);
    let white = Key::new(Mat::Plain(215), [0.8, 0.8, 0.78]);
    let black = Key::new(Mat::Plain(245), [0.013, 0.013, 0.015]);
    let sash = Key::new(Mat::Plain(190), [0.06, 0.085, 0.09]);
    let pane = Key::new(Mat::Plain(40), [0.016, 0.02, 0.024]);
    let (x0, x1, z0, z1) = (-105.4 - HOME.x, -74.1 - HOME.x, -94.2 - HOME.y, -62.1 - HOME.y);
    let (deck, parapet, band) = (23.0, 23.3, 21.0);
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let (sw, se, ne, nw) = (Vec2::new(x0, z1), Vec2::new(x1, z1), Vec2::new(x1, z0), Vec2::new(x0, z0));
    const PITCH: f32 = 4.43;
    const OPEN: f32 = 2.85;
    const FIRST: f32 = 1.02;
    // an opening's sash: glass set back, a frame, a heavy transom at door height and small lights above
    let glazing = |g: &mut Geo, left: Vec2, right: Vec2, h: [f32; 4], depth: f32, doors: bool| {
        let r = (right - left).normalize();
        let out = Vec2::new(-r.y, r.x);
        let r3 = Vec3::new(r.x, 0.0, r.y);
        let at = |u: f32, y: f32, fwd: f32| v(left + r * u - out * (depth - fwd), y);
        g.quad(pane, at(h[0], h[2], 0.0), r3 * (h[1] - h[0]), Vec3::Y * (h[3] - h[2]));
        let bar = |g: &mut Geo, u0: f32, u1: f32, y0: f32, y1: f32| g.quad(sash, at(u0, y0, 0.03), r3 * (u1 - u0), Vec3::Y * (y1 - y0));
        let (w, t) = (h[1] - h[0], 0.07);
        bar(g, h[0], h[1], h[3] - t, h[3]);
        bar(g, h[0], h[1], h[2], h[2] + t);
        let head = if doors { (h[2] + 2.35).min(h[3] - 0.6) } else { h[2] + (h[3] - h[2]) * 0.5 };
        bar(g, h[0], h[1], head - 0.06, head + 0.06);
        for k in 0..=3 {
            let u = h[0] + w * k as f32 / 3.0;
            bar(g, (u - t / 2.0).max(h[0]), (u + t / 2.0).min(h[1]), h[2], h[3]);
        }
        // the small lights over the transom
        let rows = ((h[3] - head) / 0.75).round().max(1.0) as i32;
        for j in 1..rows {
            let y = head + (h[3] - head) * j as f32 / rows as f32;
            bar(g, h[0], h[1], y - 0.02, y + 0.02);
        }
        for k in 0..6 {
            let u = h[0] + w * (k as f32 + 0.5) / 6.0;
            bar(g, u - 0.015, u + 0.015, head, h[3]);
        }
    };
    // one face: brick with its openings, the band above; `rows` are (sill, head, doors) per storey, `skip` the bays
    // left as plain brick on a storey (bay, row)
    let wall = |g: &mut Geo, left: Vec2, right: Vec2, base: f32, rows: &[(f32, f32, bool)], skip: &dyn Fn(usize, usize) -> bool| {
        let len = (right - left).length();
        let bays = ((len - 2.0 * FIRST + (PITCH - OPEN)) / PITCH).floor() as usize;
        let first = (len - (bays as f32 * PITCH - (PITCH - OPEN))) / 2.0;
        let mut holes = vec![];
        for (j, &(y0, y1, _)) in rows.iter().enumerate() {
            for b in 0..bays {
                if !skip(b, j) && y1 > base + 0.5 {
                    holes.push((j, [first + b as f32 * PITCH, first + b as f32 * PITCH + OPEN, y0.max(base), y1]));
                }
            }
        }
        let hs: Vec<[f32; 4]> = holes.iter().map(|h| h.1).collect();
        wall_holes(g, brick, left, right, base.min(0.0), band, &hs);
        wall_holes(g, black, left, right, band, parapet, &[]);
        for (j, h) in holes {
            reveal(g, brick, left, right, h, 0.45);
            glazing(g, left, right, h, 0.45, rows[j].2);
        }
    };
    let none = |_: usize, _: usize| false;
    // field face (seen from the field: the pole end on the left). Ground floor: two openings by the pole, the rest
    // stands behind the seats
    let field_rows = [(0.3, 4.4, true), (6.6, 10.05, true), (11.4, 16.1, true), (17.1, band, true)];
    wall(g, se, ne, 0.0, &field_rows, &|b, j| j == 0 && b > 1);
    // the face on the foul line: plain brick this end; the grandstand covers the rest
    wall(g, sw, se, 0.0, &[], &none);
    // street faces: storefronts and three storeys of windows
    let street = |p: Vec2| terr(p).max(0.0);
    let (gw, gn) = (street(Vec2::new(x0 - 2.0, (z0 + z1) / 2.0)), street(Vec2::new((x0 + x1) / 2.0, z0 - 2.0)));
    let street_rows = |gy: f32| [(gy + 0.5, gy + 4.3, true), (7.6, 10.0, false), (12.4, 15.6, false), (18.0, 20.5, false)];
    wall(g, nw, sw, gw, &street_rows(gw), &none);
    // against the north face (lidar): a pale stair tower 6 m wide running 18 m north, 31 m high, and beside it a
    // brick annex 10 m square at roof height; between them they cover the face from x -92 to -76
    let (tx0, tx1, ax1) = (-92.0 - HOME.x, -86.0 - HOME.x, -76.0 - HOME.x);
    let north_len = x1 - x0;
    let n_bays = ((north_len - 2.0 * FIRST + (PITCH - OPEN)) / PITCH).floor() as usize;
    let n_first = (north_len - (n_bays as f32 * PITCH - (PITCH - OPEN))) / 2.0;
    wall(g, ne, nw, gn, &street_rows(gn), &|b, _| {
        let (u0, u1) = (n_first + b as f32 * PITCH, n_first + b as f32 * PITCH + OPEN);
        x1 - u1 < ax1 + 0.4 && x1 - u0 > tx0 - 0.4
    });
    let stucco = pale_precast();
    let (tn, an) = (z0 - 17.8, z0 - 9.8);
    g.solid(stucco, Vec3::new(tx0, gn.min(0.0), tn), Vec3::new(tx1, 31.0, z0));
    // the tower's street face: three raking slits and a tall narrow light (aerial photos)
    let slit = Key::new(Mat::Plain(60), [0.02, 0.024, 0.028]);
    for (k, y) in [11.0f32, 17.5, 24.0].into_iter().enumerate() {
        let za = tn + 3.0 + k as f32 * 0.6;
        g.poly(slit, &[Vec3::new(tx0 - 0.02, y, za + 4.2), Vec3::new(tx0 - 0.02, y + 0.9, za), Vec3::new(tx0 - 0.02, y + 1.5, za), Vec3::new(tx0 - 0.02, y + 0.6, za + 4.2)], -Vec3::Z, Vec3::Y);
    }
    g.quad(slit, Vec3::new(tx0 - 0.02, 4.0, tn + 10.4), -Vec3::Z * 0.5, Vec3::Y * 24.0);
    wall_holes(g, brick, Vec2::new(ax1, an), Vec2::new(tx1, an), gn.min(0.0), deck, &[]);
    wall_holes(g, brick, Vec2::new(ax1, z0), Vec2::new(ax1, an), gn.min(0.0), deck, &[]);
    poly_up(g, m.concrete, &[Vec3::new(tx1, deck, z0), Vec3::new(ax1, deck, z0), Vec3::new(ax1, deck, an), Vec3::new(tx1, deck, an)]);
    g.block_ring(&[Vec2::new(tx1, z0), Vec2::new(ax1, z0), Vec2::new(ax1, an), Vec2::new(tx1, an)], 0.0, deck);
    // the name, on both field faces and the north face (where the tower hides its middle)
    for (a, b) in [(se, ne), (sw, se), (ne, nw)] {
        let r = (b - a).normalize();
        let out = Vec2::new(-r.y, r.x);
        let len = (b - a).length();
        sign(g, "sgn_supply", [0.9, 0.9, 0.87], (a + b) / 2.0 + out * 0.03, band + (parapet - band - 1.45) / 2.0, r, len - 1.1, 1.45);
    }
    // roof: deck, coping, rail
    poly_up(g, m.concrete, &[v(sw, deck), v(se, deck), v(ne, deck), v(nw, deck)]);
    g.block_ring(&[sw, se, ne, nw], 0.0, deck);
    let ring = [sw, se, ne, nw];
    for k in 0..4 {
        let (a, b) = (ring[k], ring[(k + 1) % 4]);
        let e = (b - a).normalize();
        let inw = Vec2::new(-e.y, e.x) * if Vec2::new(-e.y, e.x).dot((sw + ne) / 2.0 - a) > 0.0 { 1.0 } else { -1.0 };
        // inner face and top of the parapet
        g.poly(white, &[v(b + inw * 0.3, deck), v(a + inw * 0.3, deck), v(a + inw * 0.3, parapet), v(b + inw * 0.3, parapet)], -Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        poly_up(g, white, &[v(a, parapet), v(b, parapet), v(b + inw * 0.3, parapet), v(a + inw * 0.3, parapet)]);
        rail_run(g, m, a + inw * 0.15, b + inw * 0.15, parapet, 0.8);
    }
    // stair penthouse
    g.solid(pale_precast(), Vec3::new(-85.0 - HOME.x, deck, -82.0 - HOME.y), Vec3::new(-81.0 - HOME.x, 31.0, -72.0 - HOME.y));
    // two blocks of bleachers rising west, an aisle between
    let (front, rows, pitch, rise) = (-86.0 - HOME.x, 11, 0.9f32, 0.45f32);
    for (za, zb) in [(-71.0 - HOME.y, -63.3 - HOME.y), (-80.6 - HOME.y, -73.0 - HOME.y)] {
        for j in 0..rows {
            let (xa, xb) = (front - j as f32 * pitch, front - (j + 1) as f32 * pitch);
            let y = deck + 0.3 + j as f32 * rise;
            g.cuboid(m.concrete, Vec3::new(xb, deck, za), Vec3::new(xa, y, zb));
            seat_row(g, f, Vec2::new(xb + SEAT_SETBACK, za + 0.3), Vec2::new(xb + SEAT_SETBACK, zb - 0.3), y, Vec2::X, (true, true), 0.0);
        }
        // closed sides and back, in the park's navy
        let (xw, top) = (front - rows as f32 * pitch, deck + 0.3 + rows as f32 * rise);
        g.cuboid(m.navy, Vec3::new(xw - 0.15, deck, za), Vec3::new(xw, top + 1.0, zb));
    }
    // balconies on the field face: (level, depth, runs along the face from the pole end, seats)
    let face_len = z1 - z0;
    let balconies: [(f32, f32, &[(f32, f32)], bool); 3] = [(17.1, 4.5, &[(0.0, face_len - 0.9)], false), (11.4, 3.6, &[(-0.3, 8.55), (8.95, 22.0), (22.5, face_len - 0.8)], true), (6.6, 3.6, &[(-0.3, 8.55), (8.95, 22.0)], true)];
    for (y, d, runs, seats) in balconies {
        for &(s0, s1) in runs {
            let (za, zb) = (z1 - s0, z1 - s1);
            // slab, deep edge beam, soffit
            g.cuboid(white, Vec3::new(x1, y - 0.3, zb), Vec3::new(x1 + d, y, za));
            g.cuboid(white, Vec3::new(x1 + d - 0.25, y - 0.8, zb), Vec3::new(x1 + d, y + 0.08, za));
            // tapered brackets on the piers
            for k in 0..=7 {
                let s = (FIRST + k as f32 * PITCH - (PITCH - OPEN) / 2.0).clamp(0.45, face_len - 0.45);
                if s < s0 + 0.2 || s > s1 - 0.2 {
                    continue;
                }
                let z = z1 - s;
                let side = [Vec3::new(x1, y - 0.3, 0.0), Vec3::new(x1 + d - 0.25, y - 0.3, 0.0), Vec3::new(x1 + d - 0.25, y - 0.8, 0.0), Vec3::new(x1, y - 1.5, 0.0)];
                for (dz, n) in [(-0.2f32, -Vec3::Z), (0.2, Vec3::Z)] {
                    g.poly(white, &side.map(|q| q + Vec3::Z * (z + dz)), -n.cross(Vec3::Y), Vec3::Y);
                }
                g.poly(white, &[Vec3::new(x1, y - 1.5, z + 0.2), Vec3::new(x1, y - 1.5, z - 0.2), Vec3::new(x1 + d - 0.25, y - 0.8, z - 0.2), Vec3::new(x1 + d - 0.25, y - 0.8, z + 0.2)], Vec3::Z, Vec3::new(1.0, 0.2, 0.0).normalize());
            }
            // rail: front and both ends
            let (fa, fb) = (Vec2::new(x1 + d - 0.1, za), Vec2::new(x1 + d - 0.1, zb));
            rail_run(g, m, fa, fb, y + 0.08, 1.05);
            rail_run(g, m, Vec2::new(x1, za), fa, y, 1.1);
            rail_run(g, m, fb, Vec2::new(x1, zb), y, 1.1);
            if seats {
                // two rows, the back one a step up
                for (k, back) in [(0, 0.0f32), (1, 0.9)] {
                    let x = x1 + d - 1.25 - back;
                    let yy = y + k as f32 * 0.3;
                    if k == 1 {
                        g.cuboid(m.concrete, Vec3::new(x1 + 0.9, y, zb + 0.15), Vec3::new(x + 0.6, yy, za - 0.15));
                    }
                    seat_row(g, f, Vec2::new(x, zb + 0.4), Vec2::new(x, za - 0.4), yy, Vec2::X, (true, true), 0.0);
                }
            }
        }
    }
}

/// Brick tint for the warehouse: red-brown, between its measure against the white balcony fascias in an overcast
/// photo and its deeper colour in a sunlit aerial.
const WM_BRICK: [f32; 3] = [0.92, 0.58, 0.46];

/// A guard rail from `a` to `b` standing on level `y`, `h` high: posts, a top rail and two lighter rails below it.
fn rail_run(g: &mut Geo, m: &Mats, a: Vec2, b: Vec2, y: f32, h: f32) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (b - a).length();
    let n = (len / 1.6).ceil().max(1.0) as i32;
    for k in 0..=n {
        let p = a.lerp(b, k as f32 / n as f32);
        g.tube(m.rail, v(p, y), v(p, y + h), 0.022, 4);
    }
    for t in [1.0f32, 0.55, 0.12] {
        g.tube(m.rail, v(a, y + h * t), v(b, y + h * t), if t == 1.0 { 0.028 } else { 0.014 }, 4);
    }
}

// ---------------------------------------------------------------- lights, towers and boards
// All from the lidar (0.5 m surface): where each thing stands, how long and deep it is and how high. Their look
// is from photos: the lamp banks are open white steel frames of floodlights tipped toward the field, standing on
// white portals on the canopy or on masts over the boards; the two towers are plain cream shafts.

/// An oriented box (plan centre `c`, unit long axis `a`, half length and half width), walls, top and collision.
fn obox(g: &mut Geo, k: Key, c: Vec2, a: Vec2, hl: f32, hw: f32, y0: f32, y1: f32) {
    let b = Vec2::new(-a.y, a.x);
    let r = [c - a * hl - b * hw, c + a * hl - b * hw, c + a * hl + b * hw, c - a * hl + b * hw];
    for k2 in 0..4 {
        let (p, q) = (r[k2], r[(k2 + 1) % 4]);
        face(g, k, p, q, (p + q) / 2.0 - c, y0, y1, &[]);
    }
    poly_up(g, k, &r.map(|p| Vec3::new(p.x, y1, p.y)));
    g.block_ring(&r, y0, y1);
}

/// A bank of floodlights: an open frame `w` wide from `y0` to `y1`, its foot centred on `c`, facing `fwd` and
/// tipped forward by `lean` at the top; lamps in rows a metre apart.
fn light_bank(g: &mut Geo, m: &Mats, c: Vec2, fwd: Vec2, y0: f32, y1: f32, w: f32, lean: f32) {
    let lens = Key::new(Mat::Plain(35), [0.5, 0.53, 0.57]);
    let fwd = fwd.normalize();
    let right = Vec2::new(fwd.y, -fwd.x);
    let r3 = Vec3::new(right.x, 0.0, right.y);
    let rise = Vec3::new(fwd.x * lean, y1 - y0, fwd.y * lean);
    let up = rise.normalize();
    let n = r3.cross(up);
    let at = |s: f32, t: f32| Vec3::new(c.x, y0, c.y) + r3 * s + rise * t;
    // frame: the sides, top and bottom, a rail behind every other row
    let h = rise.length();
    let rows = (h / 1.0).round().max(2.0) as i32;
    let cols = (w / 0.9).round().max(2.0) as i32;
    for s in [-w / 2.0, w / 2.0] {
        g.member(m.white, at(s, 0.0), at(s, 1.0), 0.22, 0.3, n);
    }
    for j in (0..=rows).step_by(2) {
        let t = j as f32 / rows as f32;
        g.member(m.white, at(-w / 2.0, t) - n * 0.2, at(w / 2.0, t) - n * 0.2, 0.16, 0.2, n);
    }
    g.member(m.white, at(-w / 2.0, 1.0), at(w / 2.0, 1.0), 0.2, 0.3, n);
    // lamps: a lens in front, the housing behind
    let (dx, dt, sz) = (w / cols as f32, 1.0 / rows as f32, 0.62f32);
    for j in 0..rows {
        for i in 0..cols {
            let p = at(-w / 2.0 + (i as f32 + 0.5) * dx, (j as f32 + 0.5) * dt);
            g.quad(lens, p - r3 * (sz / 2.0) - up * (sz / 2.0) + n * 0.12, r3 * sz, up * sz);
            g.quad(m.white, p + r3 * (sz / 2.0) - up * (sz / 2.0) - n * 0.14, -r3 * sz, up * sz);
        }
    }
}

/// A lamp bank on a white steel portal standing on the canopy (`base` high): legs splayed along the bank, each
/// with a stay behind it, tied at the feet.
fn light_rack(g: &mut Geo, m: &Mats, c: Vec2, fwd: Vec2, base: f32, y0: f32, y1: f32, w: f32) {
    let fwd = fwd.normalize();
    let right = Vec2::new(fwd.y, -fwd.x);
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let f3 = Vec3::new(fwd.x, 0.0, fwd.y);
    let n = (w / 7.0).round().max(2.0) as i32;
    for k in 0..n {
        let s = -w / 2.0 + w * (k as f32 + 0.5) / n as f32;
        let top = c + right * s;
        for d in [-1.6f32, 1.6] {
            g.member(m.white, v(top + right * d - fwd * 0.4, base), v(top, y0 + 0.3), 0.3, 0.3, f3);
        }
        g.member(m.white, v(top - fwd * 3.2, base), v(top, y0 + (y1 - y0) * 0.6) + f3 * 0.2, 0.26, 0.26, Vec3::new(right.x, 0.0, right.y));
        g.member(m.white, v(top + right * -1.6 - fwd * 0.4, base + 0.15), v(top + right * 1.6 - fwd * 0.4, base + 0.15), 0.24, 0.24, f3);
    }
    light_bank(g, m, c, fwd, y0, y1, w, 1.1);
}

/// One of the two cream towers behind the upper deck (lidar: a shaft 15.7 x 8.5 m turned 45 degrees to the grid,
/// walls to 59 m, its field end 6 m higher; photos: plain pale stucco, a line of small square windows, a tall lamp
/// bank on the field end). `c` plan centre, `a` the long axis toward the field.
fn stair_tower(g: &mut Geo, m: &Mats, c: Vec2, a: Vec2) {
    let stucco = pale_precast();
    let dark = Key::new(Mat::Plain(60), [0.02, 0.024, 0.028]);
    let a = a.normalize();
    let b = Vec2::new(-a.y, a.x);
    let (hl, hw, top, tall, tall_len) = (7.85, 4.25, 59.0, 63.5, 6.0);
    obox(g, stucco, c, a, hl, hw, 0.0, top);
    obox(g, stucco, c + a * (hl - tall_len / 2.0), a, tall_len / 2.0, hw, top, tall);
    // windows up both long faces, near the field end; a deep notch under the top of the lower part
    for side in [-1.0f32, 1.0] {
        let out = b * side;
        let right = Vec2::new(out.y, -out.x);
        let r3 = Vec3::new(right.x, 0.0, right.y);
        let p = c + a * (hl - tall_len - 1.6) + out * (hw + 0.02);
        let mut y = 26.0;
        while y < 55.0 {
            g.quad(dark, Vec3::new(p.x, y, p.y) - r3 * 0.45, r3 * 0.9, Vec3::Y * 0.9);
            y += 3.6;
        }
        let q = c - a * (hl - 3.2) + out * (hw + 0.02);
        g.quad(dark, Vec3::new(q.x, top - 6.5, q.y) - r3 * 1.6, r3 * 3.2, Vec3::Y * 3.4);
    }
    // the lamp bank on brackets off the field end
    let f = c + a * (hl + 1.3);
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    for s in [-2.6f32, 2.6] {
        for y in [56.0f32, 60.0, 63.3] {
            g.member(m.white, v(c + a * hl + b * s, y), v(f + b * s + a * 0.25 * (y - 55.5) / 9.0, y), 0.22, 0.22, Vec3::Y);
        }
    }
    light_bank(g, m, f, a, 55.5, 64.6, 6.4, 1.0);
}

/// A video board on a white steel frame (lidar: plan centre `c` of the frame, `n` the way the screen faces, the
/// frame from `back` to `front` along `n`): the cabinet `w` wide from `y0` to `y1` showing `screen`; columns in two
/// rows down to `base`, girts and bracing behind; `sign` = a name panel on top (from, to along the board, top);
/// `lights` = a mast and lamp bank over it (from, to along the board, bank foot, bank top).
fn video_board(g: &mut Geo, m: &Mats, c: Vec2, n: Vec2, back: f32, front: f32, w: f32, y0: f32, y1: f32, base: f32, screen: &'static str, sign_on: Option<(f32, f32, f32)>, lights: (f32, f32, f32, f32)) {
    let cab = Key::new(Mat::Plain(240), [0.016, 0.018, 0.024]);
    let n = n.normalize();
    let right = Vec2::new(n.y, -n.x);
    let (r3, n3) = (Vec3::new(right.x, 0.0, right.y), Vec3::new(n.x, 0.0, n.y));
    let v = |t: f32, d: f32, y: f32| { let p = c + right * t + n * d; Vec3::new(p.x, y, p.y) };
    // cabinet and picture
    obox(g, cab, c + n * (front - 0.8), right, w / 2.0, 0.8, y0, y1);
    let pw = w - 0.9;
    g.decal(Key::new(Mat::Screen(screen), [1.0, 1.0, 1.0]).tile(pw), v(-pw / 2.0, front + 0.03, y0 + 0.45), r3 * pw, Vec3::Y * (y1 - y0 - 0.9), pw);
    // frame: columns under the cabinet and at the back, girts, cross bracing on the back
    let bays = (w / 9.0).round().max(2.0) as i32;
    let back_col = back + 0.5;
    for k in 0..=bays {
        let t = -w / 2.0 + 0.6 + (w - 1.2) * k as f32 / bays as f32;
        g.member(m.white, v(t, front - 2.0, base), v(t, front - 2.0, y0), 0.6, 0.6, n3);
        g.member(m.white, v(t, back_col, base), v(t, back_col, y1 - 0.5), 0.6, 0.6, n3);
        for y in [y0 - 0.4, (y0 + y1) / 2.0, y1 - 0.8] {
            g.member(m.white, v(t, back_col, y), v(t, front - 1.6, y), 0.35, 0.35, r3);
        }
        if k < bays {
            let t1 = -w / 2.0 + 0.6 + (w - 1.2) * (k + 1) as f32 / bays as f32;
            for (ya, yb) in [(y0 - 0.4, (y0 + y1) / 2.0), ((y0 + y1) / 2.0, y1 - 0.8)] {
                g.member(m.white, v(t, back_col, ya), v(t1, back_col, yb), 0.28, 0.28, n3);
                g.member(m.white, v(t1, back_col, ya), v(t, back_col, yb), 0.28, 0.28, n3);
            }
            for y in [base + 0.5, y0 - 0.4, (y0 + y1) / 2.0, y1 - 0.8] {
                g.member(m.white, v(t, back_col, y), v(t1, back_col, y), 0.35, 0.35, n3);
            }
        }
    }
    if sign_on.is_some() {
        // the city side of the main board: a billboard on the back of the frame with the name across it
        let (bw, by0, by1) = (w * 0.62, y0 + 1.5, y1 - 1.0);
        g.quad(cab, v(bw / 2.0, back - 0.1, by0), -r3 * bw, Vec3::Y * (by1 - by0));
        let bc = c - n * (back.abs() + 0.14);
        sign(g, "sgn_ballpark", [0.95, 0.95, 0.93], Vec2::new(bc.x, bc.y), (by0 + by1) / 2.0 - 1.6, -right, bw * 0.8, 3.2);
    }
    if let Some((t0, t1, top)) = sign_on {
        obox(g, cab, c + right * ((t0 + t1) / 2.0) + n * (front - 0.5), right, (t1 - t0) / 2.0, 0.5, y1, top);
        let sc = c + right * ((t0 + t1) / 2.0) + n * (front + 0.04);
        sign(g, "sgn_ballpark", [0.95, 0.95, 0.93], sc, y1 + (top - y1) * 0.22, right, (t1 - t0) * 0.8, (top - y1) * 0.56);
    }
    // the mast: two braced legs up from the frame, the bank at their head
    let (t0, t1, ly0, ly1) = lights;
    let d = (front + back) / 2.0;
    for t in [t0 + 0.6, t1 - 0.6] {
        g.member(m.white, v(t, d - 1.2, y1 - 0.5), v(t, d - 1.2, ly1 - 1.0), 0.7, 0.7, n3);
        g.member(m.white, v(t, d + 1.6, y1 - 0.5), v(t, d + 0.4, ly0 + 1.0), 0.5, 0.5, r3);
    }
    let mut y = y1 + 3.0;
    let mut flip = false;
    while y + 5.0 < ly1 {
        let (ta, tb) = if flip { (t1 - 0.6, t0 + 0.6) } else { (t0 + 0.6, t1 - 0.6) };
        g.member(m.white, v(ta, d - 1.2, y), v(tb, d - 1.2, y + 5.0), 0.3, 0.3, n3);
        g.member(m.white, v(t0 + 0.6, d - 1.2, y), v(t1 - 0.6, d - 1.2, y), 0.3, 0.3, n3);
        y += 5.0;
        flip = !flip;
    }
    let bc = c + right * ((t0 + t1) / 2.0) + n * (d + 0.6);
    light_bank(g, m, bc, n, ly0, ly1, t1 - t0 + 1.5, 1.2);
}

/// Everything that lights the field or shows it a picture.
fn lights_and_boards(g: &mut Geo, m: &Mats) {
    let w = |x: f32, z: f32| Vec2::new(x, z) - HOME;
    // the park is all but symmetric about x = -0.4: the first-base side mirrors the third-base side's measures
    let mx = |p: Vec2| Vec2::new(-0.8 - (p.x + HOME.x) - HOME.x, p.y);
    let md = |d: Vec2| Vec2::new(-d.x, d.y);
    let canopy = 40.5;
    // cream towers
    let (tc, ta) = (w(-68.05, 21.55), Vec2::new(0.70711, -0.70711));
    stair_tower(g, m, tc, ta);
    stair_tower(g, m, mx(tc), md(ta));
    // portals on the canopy either side of home: a 15 m bank to 54 m
    let (rc, rf) = (w(-55.0, 43.5), Vec2::new(0.676, -0.737));
    light_rack(g, m, rc, rf, canopy, 47.3, 54.0, 15.0);
    light_rack(g, m, mx(rc), md(rf), canopy, 47.3, 54.0, 15.0);
    // along the end of each upper deck: two banks side by side on a 28 m run, to 54 m
    for zc in [-51.0f32, -37.0] {
        let ec = w(-100.2, zc);
        light_rack(g, m, ec, Vec2::X, canopy, 47.6, 54.0, 12.6);
        light_rack(g, m, mx(ec), Vec2::NEG_X, canopy, 47.6, 54.0, 12.6);
    }
    // left field: the main board, 37.7 x 18.7 m, its name over the middle and a lamp bank on a mast to 66 m
    video_board(g, m, w(-58.8, -110.6), Vec2::new(0.691, 0.723), -6.0, 4.5, 37.7, 17.8, 36.5, 7.0, "scr_lf_board", Some((-8.0, 7.0, 41.0)), (-6.5, 4.5, 56.5, 66.0));
    // right field: a long three-panel board at the top of the stand, lamps to 58.5 m
    video_board(g, m, w(87.3, -110.9), Vec2::new(-0.66, 0.751), -3.0, 3.0, 38.0, 29.5, 39.0, 17.0, "scr_rf_board", None, (-4.5, 8.0, 50.5, 58.5));
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

/// Gate stairs from the street (bearing from home), one each side; behind home the way in is the grand stair and
/// the podium.
const GATES: [f32; 2] = [120.0, 240.0];
/// Stair cores (bearing from home).
const CORES: [f32; 3] = [208.0, 126.0, 246.0];

/// A stair from the street (at offset `n_out`) up to the concourse floor (at `n_in`), between stations a and b.
fn gate_stair(g: &mut Geo, m: &Mats, a: &Station, b: &Station, n_out: f32, n_in: f32) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let at = |s: &Station, n: f32| s.p + s.n * n;
    let e = (b.p - a.p).normalize_or_zero();
    let ev = Vec3::new(e.x, 0.0, e.y);
    let steps = 46;
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

/// The way in behind home, as walk waypoints (world x, z) with the floor expected there: from the plaza up the
/// grand stair, along the podium under the footbridge, through the frame onto the main concourse, and down the rows
/// under the terrace.
/// Across the park's lawn and up the amphitheatre hill to the square on its rim (world waypoints, expected levels).
pub fn park_hill_route() -> Vec<([f32; 2], Option<f32>)> {
    vec![([0.0, -160.0], None), ([20.0, -166.0], None), ([38.0, -170.0], None), ([42.0, -183.5], Some(HILL_TOP))]
}

pub fn home_gate_route() -> Vec<([f32; 2], Option<f32>)> {
    vec![
        ([7.05, 104.35], None),
        ([-9.8, 94.6], Some(CONCOURSE_Y)),
        ([-5.5, 83.5], Some(CONCOURSE_Y)),
        ([0.0, 75.5], Some(CONCOURSE_Y)),
        ([-0.28, 66.1], Some(CONCOURSE_Y)),
        ([-0.53, 58.1], Some(CONCOURSE_Y)),
        ([-0.84, 48.1], None),
    ]
}

/// The stair cores, one a little clockwise of each gate, standing on the street.
fn cores(city: &city::City, st: &[Station]) -> Vec<Core> {
    CORES.iter().filter_map(|&cb| core_at(st, cb)).map(|mut c| {
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
        (ba - 163.0).abs().total_cmp(&(bb - 163.0).abs())
    }).unwrap_or(total / 2.0);
    let _ = (width, glyph, px, n_ch);
    let (p, n, _) = place(centre);
    // (flat, standing 1.2 m proud of the curved frame at its middle; red with a white edge behind)
    let right = Vec2::new(n.y, -n.x);
    let right = if Vec3::new(right.x, 0.0, right.y).cross(Vec3::Y).dot(Vec3::new(n.x, 0.0, n.y)) > 0.0 { right } else { -right };
    sign(g, "sgn_ballpark", [0.95, 0.95, 0.93], p + n * 1.15, 31.5, right, 20.6, 4.6);
    sign(g, "sgn_ballpark", [0.36, 0.012, 0.02], p + n * 1.2, 31.6, right, 20.4, 4.4);
}

/// The footbridge from the terrace concourse out to the tower (satellite): its tower end on the tower's north face,
/// and a point toward the bowl along its axis (world); it starts where the axis crosses the frame.
const BRIDGE_TOWER: (f32, f32) = (-15.3, 98.9);
const BRIDGE_BOWL: (f32, f32) = (0.0, 74.9);
const BRIDGE_W: f32 = 8.0;
/// Its top (lidar: 23-24 m along its whole length; 23.4 on the street photo).
const BRIDGE_TOP: f32 = 23.4;

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
    [(-16.1, 100.3), (0.0, 110.0), (-6.0, 121.0), (-22.1, 111.3)].iter().map(|&(x, z)| Vec2::new(x, z) - HOME).collect()
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

/// A wall from `a` to `b` whose outside is toward `out`, with holes (metres from `a`, height): see `wall_holes`.
fn face(g: &mut Geo, k: Key, a: Vec2, b: Vec2, out: Vec2, y0: f32, y1: f32, holes: &[[f32; 4]]) {
    let r = (b - a).normalize();
    if Vec2::new(-r.y, r.x).dot(out) > 0.0 {
        wall_holes(g, k, a, b, y0, y1, holes);
    } else {
        let len = (b - a).length();
        let hs: Vec<[f32; 4]> = holes.iter().map(|h| [len - h[1], len - h[0], h[2], h[3]]).collect();
        wall_holes(g, k, b, a, y0, y1, &hs);
    }
}

/// The lantern tower's stone (measured on a street photo with the camera solved against the lidar corners, heights
/// agreeing with the lidar): a 27 m base on the full footprint; a 31 m shoulder on its west strip; the tall south
/// part to 36.5 m under the mesh lantern. Horizontal notches 0.7 m high are cut into the south and east corners at four
/// levels (4 m along the street face, 1.3 m round the corner); a recessed full-height glass slot divides the plaza
/// face between the tall part and the east pier; a slit window near the top of the street face; the bridge's door.
fn lantern_tower(g: &mut Geo, m: &Mats) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let w = |x: f32, z: f32| Vec2::new(x, z) - HOME;
    let (nn, e, s, wc) = (w(-16.1, 100.3), w(0.0, 110.0), w(-6.0, 121.0), w(-22.1, 111.3));
    let (t_n, t_e, t_w) = (w(-11.0, 108.0), w(-2.0, 113.0), w(-16.0, 115.0));
    let (d_sw, d_se) = ((wc - s).normalize(), (e - s).normalize());
    let (o_sw, o_se) = (-d_se, -d_sw);
    // (heights: the photo read with the pano's own pitch puts the shoulder at 31, the stone top at 36.5 and the
    // lantern's top at 42 - the three heights the lidar gives for this footprint)
    let (base, shoulder, tall) = (27.0f32, 31.0f32, 36.5f32);
    // (notches and slit placed down from the stone's top as read off the photo at the pano's pose)
    let levels = [8.6f32, 13.1, 17.9, 23.1];
    let nh = 0.75;
    let (l_se, l_ne) = ((e - s).length(), (nn - e).length());
    let pane = Key::new(Mat::Plain(40), [0.012, 0.03, 0.075]);
    let frame = Key::new(Mat::Plain(120), [0.45, 0.47, 0.5]);
    let dark = Key::new(Mat::Plain(230), [0.03, 0.028, 0.026]);
    // --- the base's four faces
    let h_sw: Vec<[f32; 4]> = levels.iter().map(|&y| [0.0, 4.0, y, y + nh]).collect();
    let slot = [8.3f32, 9.7, 2.0, base];
    let mut h_se: Vec<[f32; 4]> = levels.iter().flat_map(|&y| [[0.0, 1.3, y, y + nh], [l_se - 1.3, l_se, y, y + nh]]).collect();
    h_se.push(slot);
    // the bridge lands on the north-east face
    let bridge = (Vec2::new(BRIDGE_TOWER.0, BRIDGE_TOWER.1) - HOME - e).dot(d_sw);
    let deck = TERR_Y + 9.0 * TERR_RISE + 0.25;
    let mut h_ne: Vec<[f32; 4]> = levels.iter().map(|&y| [0.0, 1.3, y, y + nh]).collect();
    let _ = (bridge, deck);
    face(g, m.sandstone, s, wc, o_sw, 0.0, base, &h_sw);
    face(g, m.sandstone, s, e, o_se, 0.0, base, &h_se);
    face(g, m.sandstone, e, nn, d_se, 0.0, base, &h_ne);
    face(g, m.sandstone, nn, wc, d_sw, 0.0, base, &[]);
    poly_up(g, m.concrete, &[v3(nn, base), v3(e, base), v3(s, base), v3(wc, base)]);
    g.block_ring(&[nn, e, s, wc], 0.0, base);
    // --- the notches' insides (in shade), south corner then east corner
    g.shade = true;
    for &y in &levels {
        let (p1, p2, p3) = (s + d_sw * 4.0, s + d_sw * 4.0 + d_se * 1.3, s + d_se * 1.3);
        face(g, m.sandstone, p3, p2, o_sw, y, y + nh, &[]);
        face(g, m.sandstone, p1, p2, o_se, y, y + nh, &[]);
        poly_up(g, stone_flat(), &[v3(s, y), v3(p1, y), v3(p2, y), v3(p3, y)]);
        poly_down(g, stone_flat(), &[v3(s, y + nh), v3(p1, y + nh), v3(p2, y + nh), v3(p3, y + nh)]);
        let (q1, q2, q3) = (e - d_se * 1.3, e - d_se * 1.3 + d_sw * 1.3, e + d_sw * 1.3);
        face(g, m.sandstone, q1, q2, d_se, y, y + nh, &[]);
        face(g, m.sandstone, q3, q2, o_se, y, y + nh, &[]);
        poly_up(g, stone_flat(), &[v3(e, y), v3(q1, y), v3(q2, y), v3(q3, y)]);
        poly_down(g, stone_flat(), &[v3(e, y + nh), v3(q1, y + nh), v3(q2, y + nh), v3(q3, y + nh)]);
    }
    // --- the glass slot: stone jambs 0.7 m deep, dark blue glass with a mullion at each floor
    {
        let (pa, pb) = (s + d_se * slot[0], s + d_se * slot[1]);
        let inw = d_sw * 0.7;
        face(g, m.sandstone, pa, pa + inw, d_se, slot[2], slot[3], &[]);
        face(g, m.sandstone, pb, pb + inw, -d_se, slot[2], slot[3], &[]);
        let fwd = Vec2::new(-d_se.y, d_se.x).dot(o_se) > 0.0;
        let (o, r) = if fwd { (pa + inw, d_se) } else { (pb + inw, -d_se) };
        g.quad(pane, v3(o, slot[2]), Vec3::new(r.x, 0.0, r.y) * (slot[1] - slot[0]), Vec3::Y * (slot[3] - slot[2]));
        // the slot is a stack of tall windows: a pale spandrel between each (second street photo)
        let mut y = slot[2] + 4.2;
        while y + 1.7 < slot[3] {
            g.quad(stone_flat(), v3(o + o_se * 0.05, y), Vec3::new(r.x, 0.0, r.y) * (slot[1] - slot[0]), Vec3::Y * 1.7);
            y += 5.9;
        }
        let mut y = slot[2];
        while y <= slot[3] {
            g.member(frame, v3(pa + inw + o_se * 0.04, y), v3(pb + inw + o_se * 0.04, y), 0.07, 0.07, Vec3::new(o_se.x, 0.0, o_se.y));
            y += 3.4;
        }
        for p in [pa, pb] {
            g.member(frame, v3(p + inw + o_se * 0.04, slot[2]), v3(p + inw + o_se * 0.04, slot[3]), 0.07, 0.07, Vec3::new(o_se.x, 0.0, o_se.y));
        }
    }
    g.shade = false;
    // --- the west shoulder and the tall south part
    let id = |x: f32, z: f32| Vec2::new(x, z);
    let cut = 7.1;
    let ring = [wc, nn, nn - d_sw * cut, wc - d_sw * cut].map(|p| (p.x, p.y));
    prism(g, m, &id, m.sandstone, &ring, base, &|_| shoulder, false);
    let slit = [1.2f32, 1.9, 31.6, 35.4];
    face(g, m.sandstone, s, t_w, o_sw, base, tall, &[slit]);
    face(g, m.sandstone, s, t_e, o_se, base, tall, &[]);
    face(g, m.sandstone, t_e, t_n, d_se, base, tall, &[]);
    face(g, m.sandstone, t_n, t_w, d_sw, base, tall, &[]);
    poly_up(g, m.concrete, &[v3(t_n, tall), v3(t_e, tall), v3(s, tall), v3(t_w, tall)]);
    g.block_ring(&[t_n, t_e, s, t_w], base, tall);
    {
        let (pa, pb) = (s + d_sw * slit[0] - o_sw * 0.3, s + d_sw * slit[1] - o_sw * 0.3);
        let fwd = Vec2::new(-d_sw.y, d_sw.x).dot(o_sw) > 0.0;
        let (o, r) = if fwd { (pa, d_sw) } else { (pb, -d_sw) };
        g.quad(dark, v3(o, slit[2]), Vec3::new(r.x, 0.0, r.y) * (slit[1] - slit[0]), Vec3::Y * (slit[3] - slit[2]));
    }
    let _ = l_ne;
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
    let _ = &clad;
    lantern_tower(g, m);
    // the lantern: a dark steel mesh cage on the tower's upper part
    let mesh = Key::new(Mat::Plain(100), [0.16, 0.16, 0.15]).wear(wear::METAL);
    let (y0, y1) = (36.5, 40.6);
    let tr = lantern_ring();
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
    // (square to the east building: a joint on its face line and on the ticket hall's right wall)
    let ax = Vec2::new(1.0, -1.0).normalize();
    let az = Vec2::new(-ax.y, ax.x);
    let o = w(66.0, 74.0) + Vec2::new(-1.0, -1.0).normalize() * 0.5 + ax * 8.17;
    let (cell, slab) = (1.5f32, 2i32);
    for i in -80i32..40 {
        for j in -40i32..40 {
            let at = |di: f32, dj: f32| o + ax * ((i as f32 + di) * cell) + az * ((j as f32 + dj) * cell);
            let c = at(0.5, 0.5);
            let cw = c + HOME;
            if cw.x < -24.0 || cw.x > 96.0 || cw.y < 50.0 || cw.y > 152.0 {
                continue;
            }
            let (rd, rh) = road_d(c);
            if rd < rh + 0.35 || !outside_bowl(c) || point_in(&tower, c) || beds.iter().any(|r| point_in(r, c)) {
                continue;
            }
            if others.iter().any(|r| point_in(r, c)) {
                continue;
            }
            // the east building's footprint (its ticket hall has its own floor)
            let eb = cw - Vec2::new(66.0, 74.0);
            let (ea, ebb) = (eb.dot(ax), eb.dot(Vec2::new(-1.0, -1.0).normalize()));
            if ea > -13.0 && ea < 25.7 && ebb > 0.5 && ebb < 23.3 {
                continue;
            }
            // lawns inside the plaza (the tower's garden) stay grass; the plaza's own strip beds are the palm beds
            let lawn = lawns.iter().any(|r| point_in(r, c));
            let pts = [at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)].map(|p| v(p, ground(p)));
            let (si, sj) = (i.div_euclid(slab), j.div_euclid(slab));
            let h = (si.wrapping_mul(73_856_093) ^ sj.wrapping_mul(19_349_663)) as u32;
            let uv = [(h % 7) as f32 * 3.0 + 0.0, ((h >> 4) % 5) as f32 * 3.0];
            // (no grass strip in front of the east building: its forecourt is paved to the curb)
            let east_front = (cw - Vec2::new(66.0, 74.0)).dot(ax) > -16.0;
            let (k, uv) = if (rd < rh + 2.3 && !east_front) || lawn { (grass, [0.0, 0.0]) } else { (paving, uv) };
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
        // under cover (arcades, the ticket hall) the photos show the stone a good deal darker and warmer than the sky
        // light alone gives: the engine's ambient has no occlusion there
        let cover = if g.shade { [0.42, 0.36, 0.28] } else { [1.0; 3] };
        let tint = [0, 1, 2].map(|c| (lv + (t[c] - l) * 0.35) * dim * STONE_TONE[c] * cover[c]);
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
    let centre = (0..=400).map(|k| total * k as f32 / 400.0).min_by(|&a, &b| (bearing(place(a).0) - 163.0).abs().total_cmp(&(bearing(place(b).0) - 163.0).abs())).unwrap();
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
    // (heights from the old head-on photo, scaled between the lidar roof edge and the terrace deck: trusses 12.9 to
    // 15.6 and 19.0 to 22.5 m)
    let b0 = 12.9f32;
    let bands: [(f32, f32, &[usize]); 4] = [(b0, d1 - 0.75, &[2, 3]), (19.0, GZ_UPPER_CONCOURSE - 1.0, &[1, 4]), (31.5, 33.4, &[2, 3]), (top_row + 1.15, roof, &[])];
    // the big arms, left half of the bay (mirrored for the right): (fraction, height) to (fraction, height), section.
    // From the photos: at the bottom, arms from near the bay's middle under the terrace deck down through the truss
    // to the street-level deck's edge by the columns, crossed by legs from each column's foot of the truss; under
    // the upper concourse, arms from the terrace deck by the column up through the truss to the fascia; the tall
    // bay's from the upper concourse by the column to the letter truss. Where an arm crosses a truss it is that
    // panel's diagonal.
    let mid_lo = 19.0;
    // each reaches the floor of the level below; steep (about 60 degrees), not 45
    // every arm hangs from the bottom chord of the truss above its level down to that level's floor; the trusses
    // keep their own diagonals
    // at the bottom a leg from each column's top corner runs down to meet the big arm (a Y, not a cross)
    let big = ((2.0f32 / 6.0, b0), (0.0f32, CONCOURSE_Y));
    let leg_dir = (0.36f32, CONCOURSE_Y - b0);
    let leg_end = {
        // solve (0, b0) + t * leg_dir = big.0 + s * (big.1 - big.0)
        let (ax, ay) = (big.1 .0 - big.0 .0, big.1 .1 - big.0 .1);
        let den = leg_dir.0 * ay - leg_dir.1 * ax;
        let t = ((big.0 .0) * ay - (big.0 .1 - b0) * ax) / den;
        (leg_dir.0 * t, b0 + leg_dir.1 * t)
    };
    let arms = [
        (big.0 .0, big.0 .1, big.1 .0, big.1 .1, 0.6),
        (0.0, b0, leg_end.0, leg_end.1, 0.42),
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
                point_in(&br, p.lerp(q, t)) && y > CONCOURSE_Y - 0.1 && y < BRIDGE_TOP + 0.1
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

/// The footbridge from the terrace concourse to the block at the tower's north corner. Plan from the lidar (a band
/// 8 m wide, 23-24 m high, heading 32.5 degrees); heights from a street photo with the camera solved against the
/// tower, read on the band's plaza-side face, where they come out level along its length: a white deck edge 15.8 to
/// 16.7 m; a pale mesh screen from there to 23.4 m carrying lettering 4.2 m tall (17.0 to 21.2 m) along 19.5 m; a
/// truss under the deck down to 11.3 m; dry planting along the top. `frame`: the gate frame's line, where it starts.
fn footbridge(g: &mut Geo, m: &Mats, frame: &[(Vec2, Vec2)]) {
    let v = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let t = Vec2::new(BRIDGE_TOWER.0, BRIDGE_TOWER.1) - HOME;
    let b = Vec2::new(BRIDGE_BOWL.0, BRIDGE_BOWL.1) - HOME;
    let e = (b - t).normalize();
    // where the axis crosses the frame line; the deck laps 1.5 m onto the terrace concourse inside it
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
    let (ev, sv) = (Vec3::new(dir.x, 0.0, dir.y), Vec3::new(side.x, 0.0, side.y));
    let deck = TERR_Y + 9.0 * TERR_RISE + 0.25;
    let (edge0, edge1, low, top) = (15.8f32, 16.7f32, 11.3f32, BRIDGE_TOP);
    let hw = BRIDGE_W / 2.0;
    let steel = Key::new(Mat::Plain(120), [0.7, 0.71, 0.71]);
    let mesh = Key::new(Mat::Plain(190), [0.62, 0.63, 0.63]);
    // deck slab and soffit
    let q = [a - side * hw, z - side * hw, z + side * hw, a + side * hw];
    // (grey broom-finished slabs: the scanned smooth concrete reads brown)
    let paving = Key::new(Mat::Surface("vlzlajnfw"), [0.64, 0.62, 0.58]).tile(3.0).rough(0.8);
    poly_up(g, paving, &q.map(|p| v(p, deck)));
    // the doorway into the block at the tower end: a dark opening in a steel frame
    {
        let (c, w) = (z - dir * 0.2, 1.5f32);
        let door = Key::new(Mat::Plain(230), [0.03, 0.03, 0.032]);
        let r = if Vec3::new(side.x, 0.0, side.y).cross(Vec3::Y).dot(Vec3::new(-dir.x, 0.0, -dir.y)) > 0.0 { side } else { -side };
        g.quad(door, v(c - r * w, deck), Vec3::new(r.x, 0.0, r.y) * (2.0 * w), Vec3::Y * 2.7);
        for sg in [-1.0f32, 1.0] {
            g.member(steel, v(c + r * (sg * w) - dir * 0.03, deck), v(c + r * (sg * w) - dir * 0.03, deck + 2.75), 0.1, 0.1, ev);
        }
        g.member(steel, v(c - r * w - dir * 0.03, deck + 2.75), v(c + r * w - dir * 0.03, deck + 2.75), 0.1, 0.1, ev);
    }
    g.poly(m.soffit, &[v(q[3], edge0), v(q[2], edge0), v(q[1], edge0), v(q[0], edge0)], ev, -sv);
    g.block_ring(&q, edge0, deck);
    let n = (len / 3.2).round().max(1.0) as i32;
    let plaza = if side.dot(Vec2::new(0.84, 0.54)) > 0.0 { 1.0 } else { -1.0 };
    for sg in [-1.0f32, 1.0] {
        let o = side * (sg * hw);
        let at = |i: i32| a.lerp(z, i as f32 / n as f32) + o;
        // the white deck edge
        g.member(steel, v(a + o, (edge0 + edge1) / 2.0), v(z + o, (edge0 + edge1) / 2.0), edge1 - edge0, 0.3, sv);
        // the truss under the deck: bottom chord, posts and a Warren web
        g.member(steel, v(a + o, low), v(z + o, low), 0.3, 0.3, sv);
        for i in 0..=n {
            g.member(steel, v(at(i), low), v(at(i), edge0), 0.22, 0.22, sv);
        }
        for i in 0..n {
            let (y0, y1) = if i % 2 == 0 { (low, edge0) } else { (edge0, low) };
            g.member(steel, v(at(i), y0), v(at(i + 1), y1), 0.2, 0.2, sv);
        }
        // the screen: posts and a top rail behind a pale mesh (drawn solid: it reads opaque from the street)
        for i in 0..=n {
            g.member(steel, v(at(i), edge1), v(at(i), top), 0.2, 0.2, sv);
        }
        g.member(steel, v(a + o, top), v(z + o, top), 0.22, 0.22, sv);
        let face_out = side * sg;
        for (off, flip) in [(0.12f32, false), (-0.12, true)] {
            let (p0, p1) = (a + o + face_out * off, z + o + face_out * off);
            // front faces out, back faces the walk
            let (l, r) = if (Vec2::new(-(p1 - p0).y, (p1 - p0).x).dot(face_out) > 0.0) != flip { (p0, p1) } else { (p1, p0) };
            let rr = (r - l).normalize();
            g.quad(mesh, v(l, edge1), Vec3::new(rr.x, 0.0, rr.y) * len, Vec3::Y * (top - edge1));
        }
        g.block_ring(&[a + o - side * 0.15, z + o - side * 0.15, z + o + side * 0.15, a + o + side * 0.15], deck, top);
        // dry planting along the top edge
        bougainvillea(g, a + o, z + o, face_out, top + 0.1, 0.5, 31 + (sg > 0.0) as u64, false);
        // the lettering, on the plaza side: 19.5 m long from 1.7 m short of the tower end... measured from that end
        if sg == plaza {
            let mid = z + o - dir * (-1.7f32 + 19.5 / 2.0).min(len / 2.0);
            let lw = 19.5f32.min(len - 1.0);
            let read = Vec2::new(-face_out.y, face_out.x);
            sign(g, "sgn_ballpark", [0.93, 0.93, 0.91], mid + face_out * 0.16, 17.0, if Vec3::new(read.x, 0.0, read.y).cross(Vec3::Y).dot(Vec3::new(face_out.x, 0.0, face_out.y)) > 0.0 { read } else { -read }, lw, 4.2);
        }
    }
    // cross beams over the walk
    for i in 0..=n {
        let p = a.lerp(z, i as f32 / n as f32);
        g.member(steel, v(p - side * hw, top - 0.15), v(p + side * hw, top - 0.15), 0.2, 0.3, ev);
    }
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
    let greens = if dry { [[0.3, 0.24, 0.13], [0.2, 0.18, 0.09], [0.12, 0.13, 0.06]] } else { [[0.035, 0.085, 0.03], [0.05, 0.11, 0.035], [0.028, 0.065, 0.028]] };
    let n = (len / 0.055) as i32;
    for i in 0..n {
        let h = mix(seed, i as u64, 17);
        let f = (h % 1000) as f32 / 1000.0;
        // clumps: long in some stretches, short between
        let clump = 0.5 + 0.5 * ((i as f32 * 0.055 * 2.1 + (seed % 7) as f32).sin() * 0.5 + 0.5);
        let l = drop * clump * (0.35 + 0.65 * f * f);
        // dry mats hang as a dense curtain of thin strands, thinning toward the bottom
        let w = if dry { 0.035 + 0.035 * ((h >> 10) % 100) as f32 / 100.0 } else { 0.05 + 0.05 * ((h >> 10) % 100) as f32 / 100.0 };
        if dry && (h >> 50) % 3 == 0 {
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
    let step = 0.018;
    let n = (len / step) as i32;
    // heavy runs (a long drop) are a mound two or three clumps deep, as on the terrace parapet in the photos
    let layers = if drop > 0.8 { 3 } else { 1 };
    for layer in 0..layers {
        for i in 0..n {
            let h = mix(seed + layer as u64 * 977, i as u64, 29);
            let f = (h % 1000) as f32 / 1000.0;
            let u = ((h >> 10) % 100) as f32 / 100.0;
            let v = ((h >> 20) % 100) as f32 / 100.0;
            // a low ragged line on the edge, strands spilling over in places; the mound's upper layers sit on top
            let spill = ((i as f32 * step * 1.7 + seed as f32).sin() * 0.5 + 0.5).powi(3);
            let (sz, c, yy) = if layer == 0 {
                (0.02 + 0.035 * u, left + r * (i as f32 * step) + out * (0.3 * v - 0.12), y + 0.3 * f * f - drop * spill * ((h >> 34) % 1000) as f32 / 1000.0)
            } else {
                (0.05 + 0.07 * u, left + r * (i as f32 * step) + out * (0.45 * v - 0.3), y + 0.05 + (0.22 + 0.2 * layer as f32) * f - 0.35 * spill * v)
            };
            let col = match ((h >> 44) % 6, red, layer) {
                (0..=3, true, 1..) => [0.3 + 0.1 * u, 0.02, 0.035],
                (0 | 1, true, 0) => [0.3, 0.02, 0.03],
                (2, true, 0) => [0.2, 0.03, 0.03],
                (0, false, _) => [0.11, 0.09, 0.04],
                (1, false, _) => [0.085, 0.1, 0.04],
                (2 | 3, _, _) => [0.07, 0.085, 0.035],
                _ => [0.04, 0.075, 0.03],
            };
            g.cuboid(Key::new(Mat::Plain(215), col), Vec3::new(c.x - sz, yy - sz, c.y - sz), Vec3::new(c.x + sz, yy + sz, c.y + sz));
        }
    }
}

/// Places a catalog model so that its point (model x, z; default its footprint's centre) stands on local `at`, model
/// height 0 at `y`, its +X axis along `x_dir` (its +Z a quarter turn clockwise of that, seen from above).
fn place(g: &Geo, f: &mut Furnisher, id: &str, anchor: Option<(f32, f32)>, at: Vec2, y: f32, x_dir: Vec2, scale: f32) {
    let Some((mn, mx)) = f.bounds(id) else { return };
    let (cx, cz) = ((mn[0] + mx[0]) * 0.5, (mn[2] + mx[2]) * 0.5);
    let (ax, az) = anchor.unwrap_or((cx, cz));
    let x = x_dir.normalize();
    let z = Vec2::new(-x.y, x.x);
    let p = at - (x * (ax - cx) + z * (az - cz)) * scale;
    let [wx, wz] = g.world_xz(p);
    f.out.push(crate::library::Placement { asset: id.to_string(), part: crate::library::ALL_PARTS, x: wx, z: wz, y: g.world_y(y + mn[1] * scale), rot: g.heading(x), scale: [scale; 3], tint: [1.0; 3], lod: 0, indoor: false });
}

const SGN_N: [&str; 9] = ["sgn_n1", "sgn_n2", "sgn_n3", "sgn_n4", "sgn_n5", "sgn_n6", "sgn_n7", "sgn_n8", "sgn_n9"];

/// Lettering from `fps proctex` (real typefaces, cutout): the text fills a box `w` x `h` whose bottom centre is `c`
/// (plan) at height `y`, read along `right`.
fn sign(g: &mut Geo, id: &'static str, tint: [f32; 3], c: Vec2, y: f32, right: Vec2, w: f32, h: f32) {
    // the surface carries an 8 px margin round a 192 px high box
    let (wp, hp) = (w + h / 12.0, h * 13.0 / 12.0);
    let k = Key::new(Mat::Surface(id), tint).tile(1.0);
    let o = c - right * (wp / 2.0);
    g.decal(k, Vec3::new(o.x, y - h / 24.0, o.y), Vec3::new(right.x, 0.0, right.y) * wp, Vec3::Y * hp, 1.0);
}

/// A run of ticket windows as at the ballpark's box office, measured on rectified street photos: 1.40 m modules, each
/// a 0.82 m light with a round speaking grille at its centre and a 0.58 m light to its right, under a transom 0.27
/// of the band's height, in white aluminium framing over a counter ledge. `modules` share the run evenly.
/// `numbers`: the module that is booth 1 (numerals in gold over the grille lights, up to 9). `lanes`: belt
/// stanchions in front. `left` to `right` as seen from the front (`out`).
fn ticket_wall(g: &mut Geo, f: &mut Furnisher, m: &Mats, left: Vec2, right: Vec2, out: Vec2, y0: f32, y1: f32, modules: usize, numbers: Option<usize>, lanes: bool) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let len = (right - left).length();
    let r = (right - left) / len;
    let (r3, o3) = (Vec3::new(r.x, 0.0, r.y), Vec3::new(out.x, 0.0, out.y));
    let glass = Key::new(Mat::Plain(120), [0.018, 0.022, 0.026]);
    let inside = Key::new(Mat::Plain(220), [0.07, 0.075, 0.08]);
    let steel = Key::new(Mat::Plain(90), [0.55, 0.55, 0.54]);
    let gold = [0.4, 0.25, 0.085];
    let w = len / modules as f32;
    let wide = w * 0.586;
    let yt = y1 - 0.268 * (y1 - y0);
    let at = |x: f32| left + r * x;
    g.quad(glass, v3(left + out * 0.02, y0), r3 * len, Vec3::Y * (y1 - y0));
    for y in [y0, yt, y1] {
        g.member(m.white, v3(left + out * 0.05, y), v3(right + out * 0.05, y), 0.05, 0.06, o3);
    }
    g.member(steel, v3(left + out * 0.17, y0 - 0.05), v3(right + out * 0.17, y0 - 0.05), 0.34, 0.05, Vec3::Y);
    for i in 0..modules {
        let x0 = i as f32 * w;
        for x in [x0, x0 + wide] {
            g.member(m.white, v3(at(x) + out * 0.05, y0), v3(at(x) + out * 0.05, y1), 0.05, 0.06, o3);
        }
        // what shows through the glass: a monitor's back and a desk edge
        g.quad(inside, v3(at(x0 + 0.08) + out * 0.03, y0 + 0.04), r3 * (wide * 0.36), Vec3::Y * 0.34);
        // the speaking grille: a pale disc with a dark centre, a third of the way up the light
        let c = v3(at(x0 + wide / 2.0) + out * 0.035, y0 + (y1 - y0) * 0.345);
        for (rad, k, off) in [(0.08f32, steel, 0.0f32), (0.03, inside, 0.004)] {
            for s in 0..12 {
                let (a0, a1) = (s as f32 / 12.0 * std::f32::consts::TAU, (s + 1) as f32 / 12.0 * std::f32::consts::TAU);
                let p = |a: f32| c + o3 * off + r3 * (a.cos() * rad) + Vec3::Y * (a.sin() * rad);
                g.poly(k, &[c + o3 * off, p(a0), p(a1)], r3, Vec3::Y);
            }
        }
        if let Some(first) = numbers {
            if i >= first && i - first < 9 {
                sign(g, SGN_N[i - first], gold, at(x0 + wide / 2.0) + out * 0.03, y1 + 0.06, r, 0.24, 0.26);
            }
        }
    }
    g.member(m.white, v3(right + out * 0.05, y0), v3(right + out * 0.05, y1), 0.05, 0.06, o3);
    if lanes {
        // belt stanchions: the post and head of a model by eventdesignws (CC BY), with red belts as in the photo - a
        // lane out from the counter every second booth, and a belt along the queue line 2.6 m out
        let belt = Key::new(Mat::Plain(190), [0.3, 0.03, 0.03]);
        let mut post = |g: &mut Geo, f: &mut Furnisher, p: Vec2| {
            let [x, z] = g.world_xz(p);
            for (part, y) in [(1usize, 0.09f32), (2, 0.95)] {
                f.out.push(crate::library::Placement { asset: "sf_d9146a6882".into(), part, x, z, y: g.world_y(y), rot: 0.0, scale: [1.0; 3], tint: [1.0; 3], lod: 0, indoor: false });
            }
        };
        let n_lanes = modules / 2;
        for i in 0..=n_lanes {
            let p0 = at((i * 2) as f32 * w) + out * 0.7;
            let p1 = p0 + out * 1.9;
            post(g, f, p0);
            post(g, f, p1);
            g.member(belt, v3(p0, 0.99), v3(p1, 0.99), 0.012, 0.05, r3);
            g.block_ring(&[p0 - r * 0.03, p1 - r * 0.03, p1 + r * 0.03, p0 + r * 0.03], 0.1, 1.0);
            if i < n_lanes && i % 2 == 0 {
                let q1 = at(((i + 1) * 2) as f32 * w) + out * 2.6;
                g.member(belt, v3(p1, 0.99), v3(q1, 0.99), 0.012, 0.05, o3);
                g.block_ring(&[p1 - out * 0.03, q1 - out * 0.03, q1 + out * 0.03, p1 + out * 0.03], 0.1, 1.0);
            }
        }
    }
}

/// The sandstone building along the street east of the home plate gate, built from measurement: its footprint and
/// heights from the lidar; its street face and south-west end face from street photos rectified onto a metre grid
/// (camera solved against the lidar corners; heights cross-checked between two photos to 0.5 m).
/// Frame: a runs north-east along the street from (66, 74), b in toward the bowl; the street face is the plane b = 0.5.
fn east_building(g: &mut Geo, f: &mut Furnisher, m: &Mats, terr: &dyn Fn(Vec2) -> f32) {
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
    // (its depth, 21.85 m, from the far photo's side-wall intersection and from the booths' angular size in two
    // photos; the ceiling sits 0.2 m above a 0.6 m stone lip at the mouth)
    let (h0, h1, hh, bb) = (-9.0f32, 8.17f32, 5.8f32, 22.35f32);
    let hc = 6.0f32;
    let (s0, sy0, sy1, sd) = (-3.7f32, 8.8f32, 10.55f32, 1.5f32);

    // --- the street face: two rows of three strip windows under sunshades, the hall's mouth, the loggia slot
    let (sl, sr) = (uv(a0, fb), uv(a1, fb));
    let su = |a: f32| a - a0;
    let bays = [(-7.0f32, 1.5f32), (2.8, 11.2), (12.5, 21.0)];
    let rows = [(18.6f32, 19.8f32), (13.1, 14.3)];
    // (the slot runs on to the right past the hall, open onto the podium's terrace)
    let s1 = 16.2f32;
    let mut holes: Vec<[f32; 4]> = vec![[su(h0), su(h1), 0.0, hh], [su(s0), su(s1), sy0, sy1], [su(h1), su(s1), 7.7, sy0]];
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

    // --- the ticket hall (measured on street photos from 52 m and from 18 m): stone side walls, the right one with
    // three bays of ticket windows under windows that reach the ceiling; a ribbed brown back wall with twelve
    // window modules (three will-call, booths 1-9), lettering and screens; a stone lip, then a panelled ceiling;
    // paving on the building's grid
    // the floor meets the ground outside the mouth
    let fy = [uv(h0, fb), uv(h1, fb), uv((h0 + h1) / 2.0, fb)].iter().map(|&p| terr(p)).fold(f32::MIN, f32::max) + 0.06;
    let brown = Key::new(Mat::Plain(215), [0.08, 0.045, 0.02]);
    let rib = Key::new(Mat::Plain(215), [0.045, 0.025, 0.011]);
    let ceil = Key::new(Mat::Surface("tjeneevfw"), [0.62, 0.45, 0.27]).tile(2.0);
    let cjoint = Key::new(Mat::Plain(220), [0.14, 0.1, 0.06]);
    let gold = [0.4, 0.25, 0.085];
    // (under cover: lit by the sky only dimly, as measured in the photos - a fifth of the open shade)
    g.shade = true;
    wall_holes(g, m.sandstone, uv(h0, fb), uv(h0, bb), 0.0, hc, &[]);
    let (rl, rr) = (uv(h1, bb), uv(h1, fb));
    let ru = |d: f32| (bb - fb) - d;
    let side_bays = [(1.3f32, 6.6f32), (7.9, 13.8), (15.0, 21.4)];
    let mut rholes: Vec<[f32; 4]> = vec![];
    for &(d0, d1) in &side_bays {
        rholes.push([ru(d1), ru(d0), 4.1, hc]);
        rholes.push([ru(d1), ru(d0), 1.04, 2.61]);
    }
    wall_holes(g, m.sandstone, rl, rr, 0.0, hc, &rholes);
    for &(d0, d1) in &side_bays {
        reveal(g, m.sandstone, rl, rr, [ru(d1), ru(d0), 4.1, hc], 0.35);
        reveal(g, m.sandstone, rl, rr, [ru(d1), ru(d0), 1.04, 2.61], 0.25);
        let o = uv(h1, fb + d1);
        // the upper window: dark glass up to the ceiling, two mullions
        g.quad(pane, v3(o + t * 0.35, 4.1), -n3 * (d1 - d0), Vec3::Y * (hc - 4.1));
        for k in 1..3 {
            let p = uv(h1, fb + d0 + (d1 - d0) * k as f32 / 3.0) + t * 0.32;
            g.member(dark, v3(p, 4.1), v3(p, hc), 0.06, 0.06, t3);
        }
        let mods = ((d1 - d0) / 1.4).round().max(1.0) as usize;
        ticket_wall(g, f, m, o + t * 0.25, uv(h1, fb + d0) + t * 0.25, -t, 1.04, 2.61, mods, None, false);
    }
    // the back wall: brown, ribbed above the windows
    g.quad(brown, v3(uv(h0, bb), 0.0), t3 * (h1 - h0), Vec3::Y * hc);
    let mut y = 2.75f32;
    while y < hc - 0.05 {
        g.quad(rib, v3(uv(h0, bb) + out * 0.004, y), t3 * (h1 - h0), Vec3::Y * 0.035);
        y += 0.12;
    }
    // twelve 1.40 m modules ending at the right wall; booth 4's grille on a = 0
    let (tw1, nmod) = (h1 - 0.05, 12usize);
    let tw0 = tw1 - nmod as f32 * 1.4;
    ticket_wall(g, f, m, uv(tw0, bb), uv(tw1, bb), out, 1.04, 2.61, nmod, Some(3), true);
    let grille = |k: f32| tw0 + (k + 2.0) * 1.4 + 0.41;
    sign(g, "sgn_will_call", gold, uv(tw0 + 2.1, bb) + out * 0.03, 2.7, t, 1.5, 0.2);
    sign(g, "sgn_tickets", gold, uv(grille(4.0), bb) + out * 0.03, 3.02, t, 1.73, 0.33);
    sign(g, "sgn_ballpark", gold, uv(grille(4.0), bb) + out * 0.03, 3.42, t, 3.4, 0.72);
    let scr = |g: &mut Geo, x0: f32, lit: bool| {
        let q = uv(grille(4.0) + x0, bb) + out * 0.05;
        g.quad(Key::new(Mat::Plain(160), [0.02, 0.02, 0.022]), v3(q - t * 0.04, 3.21), t3 * 1.43, Vec3::Y * 0.84);
        if lit {
            g.decal(Key::new(Mat::Screen("scr_ballgame"), [0.22, 0.22, 0.22]).tile(1.35), v3(q + out * 0.01, 3.25), t3 * 1.35, Vec3::Y * 0.76, 1.35);
        } else {
            g.quad(Key::new(Mat::Plain(50), [0.012, 0.014, 0.018]), v3(q + out * 0.01, 3.25), t3 * 1.35, Vec3::Y * 0.76);
        }
    };
    scr(g, 2.27, true);
    scr(g, 4.43, false);
    scr(g, -3.58, false);
    scr(g, -6.0, false);
    // the stone lip at the mouth, then the ceiling: large panels with joints
    let lip = 0.6f32;
    poly_down(g, stone_flat(), &[v3(uv(h0, fb), hh), v3(uv(h1, fb), hh), v3(uv(h1, fb + lip), hh), v3(uv(h0, fb + lip), hh)]);
    g.quad(stone_flat(), v3(uv(h1, fb + lip), hh), -t3 * (h1 - h0), Vec3::Y * (hc - hh));
    poly_down(g, ceil, &[v3(uv(h0, fb + lip), hc), v3(uv(h1, fb + lip), hc), v3(uv(h1, bb), hc), v3(uv(h0, bb), hc)]);
    let mut a = grille(4.0) + 1.55 - 2.44 * 4.0;
    while a < h1 {
        if a > h0 + 0.1 {
            poly_down(g, cjoint, &[v3(uv(a - 0.012, fb + lip), hc - 0.004), v3(uv(a + 0.012, fb + lip), hc - 0.004), v3(uv(a + 0.012, bb), hc - 0.004), v3(uv(a - 0.012, bb), hc - 0.004)]);
        }
        a += 2.44;
    }
    let mut b = fb + lip + 1.22;
    while b < bb - 0.1 {
        poly_down(g, cjoint, &[v3(uv(h0, b - 0.012), hc - 0.004), v3(uv(h1, b - 0.012), hc - 0.004), v3(uv(h1, b + 0.012), hc - 0.004), v3(uv(h0, b + 0.012), hc - 0.004)]);
        b += 1.22;
    }
    // the floor: 3 m slabs with scored 1.5 m cells on the building's grid (joints on the face line and on the
    // right wall's line, carried on outside by the plaza)
    {
        // (the hall's slabs are darker and warmer than the sunlit forecourt's in the photos)
        let paving = Key::new(Mat::Surface("vlzlajnfw"), [0.24, 0.17, 0.13]).tile(3.0).rough(0.8);
        let joint = Key::new(Mat::Plain(200), [0.12, 0.1, 0.085]);
        let cell = 1.5f32;
        let (na, nb) = (((h1 - h0) / cell).ceil() as i32, ((bb - fb) / cell).ceil() as i32);
        for i in 0..na {
            for j in 0..nb {
                let (a1, a0) = (h1 - i as f32 * cell, (h1 - (i + 1) as f32 * cell).max(h0));
                let (b0, b1) = (fb + j as f32 * cell, (fb + (j + 1) as f32 * cell).min(bb));
                let hsh = ((i / 2).wrapping_mul(73_856_093) ^ (j / 2).wrapping_mul(19_349_663)) as u32;
                let off = [(hsh % 7) as f32 * 3.0, ((hsh >> 4) % 5) as f32 * 3.0];
                g.poly_uv(paving, &[v3(uv(a0, b0), fy), v3(uv(a0, b1), fy), v3(uv(a1, b1), fy), v3(uv(a1, b0), fy)], t3, -n3, off);
                let jw = 0.018;
                if i % 2 == 0 {
                    poly_up(g, joint, &[v3(uv(a1 - jw, b0), fy + 0.004), v3(uv(a1 + jw, b0), fy + 0.004), v3(uv(a1 + jw, b1), fy + 0.004), v3(uv(a1 - jw, b1), fy + 0.004)]);
                }
                if j % 2 == 0 {
                    poly_up(g, joint, &[v3(uv(a0, b0 - jw), fy + 0.004), v3(uv(a1, b0 - jw), fy + 0.004), v3(uv(a1, b0 + jw), fy + 0.004), v3(uv(a0, b0 + jw), fy + 0.004)]);
                }
            }
        }
        g.block_ring(&[uv(h0, fb), uv(h1, fb), uv(h1, bb), uv(h0, bb)], -0.6, fy);
    }
    // warm light bounced in off the sunlit forecourt
    g.light(v3(uv(-0.4, fb + 5.0), 2.6), 9.0, [0.34, 0.235, 0.135]);
    g.light(v3(uv(-0.4, fb + 14.0), 3.0), 8.0, [0.2, 0.14, 0.08]);
    g.shade = false;
    // on the podium's side wall at the mouth: a cash machine (white header, blue body, dark screen), its sign, a camera
    {
        let q = uv(h1, fb - 0.05) - t * 0.03;
        g.quad(Key::new(Mat::Plain(140), [0.75, 0.75, 0.74]), v3(q, 2.5), -n3 * 1.05, Vec3::Y * 0.35);
        g.quad(Key::new(Mat::Plain(120), [0.03, 0.07, 0.32]), v3(q, 0.9), -n3 * 1.05, Vec3::Y * 1.6);
        g.quad(Key::new(Mat::Plain(60), [0.015, 0.02, 0.03]), v3(q - t * 0.01 - n * 0.25, 1.3), -n3 * 0.55, Vec3::Y * 0.5);
        g.quad(Key::new(Mat::Plain(140), [0.5, 0.05, 0.05]), v3(q - t * 0.01 - n * 0.15, 2.57), -n3 * 0.3, Vec3::Y * 0.2);
        sign(g, "sgn_atm", gold, uv(h1, fb + 0.9) - t * 0.03, 3.45, -n, 0.62, 0.26);
        // a wall camera (model by TampaJoey, CC BY: its mount at z = 0.04, its body out along +Z)
        place(g, f, "sf_631834aba2", Some((0.0, 0.04)), uv(h1, fb - 1.6), 3.7, Vec2::new(-t.y, t.x), 1.0);
    }

    // --- the loggia slot over the hall: a stone parapet (the face itself) with a rail and a shallow balcony that
    // runs on to the right onto the podium's terrace; a tan back wall; at its left end a wide hallway leads back
    // into the building from the slot's left edge; near the right a doorway stands in a recess
    {
        let (pl, pr) = (uv(s0, fb), uv(h1, fb));
        let pe = uv(s1, fb);
        let inw = -out * sd;
        let i3 = -o3;
        g.shade = true;
        // right end jamb (beyond the terrace canopy), soffit, floor
        wall_poly(g, m.sandstone, &[v3(pe + inw, sy0), v3(pe, sy0), v3(pe, sy1), v3(pe + inw, sy1)], -i3, Vec3::Y);
        poly_down(g, ceil, &[v3(pl, sy1), v3(pe, sy1), v3(pe + inw, sy1), v3(pl + inw, sy1)]);
        poly_up(g, stone_flat(), &[v3(pl, sy0), v3(pr, sy0), v3(pr - out * 0.4, sy0), v3(pl - out * 0.4, sy0)]);
        poly_up(g, m.concrete, &[v3(pl - out * 0.4, 7.7), v3(pe - out * 0.4, 7.7), v3(pe + inw, 7.7), v3(pl + inw, 7.7)]);
        // (a warmer stucco than the rest: in the photos it sits in shade lit by the stone's bounce)
        let slot_wall = Key::new(Mat::Surface("tjeneevfw"), [0.95, 0.6, 0.32]).tile(2.0);
        let deep = Key::new(Mat::Surface("tjeneevfw"), [0.3, 0.2, 0.12]).tile(2.0);
        let (bl, br) = (pl + inw, pe + inw);
        let bu = |a: f32| a - s0;
        // the hallway: from the slot's left edge 4.4 m along, full height, 5 m deep, a glazed door at its end
        let (walk, alcove) = ([0.0, bu(0.7), 7.7, sy1], [bu(6.1), bu(7.6), 7.7, 10.05]);
        wall_holes(g, slot_wall, bl, br, 7.7, sy1, &[walk, alcove]);
        let hall_d = 5.0;
        let (hl, hr) = (bl, uv(0.7, fb) + inw);
        let back = -out * hall_d;
        wall_poly(g, m.sandstone, &[v3(pl, sy0), v3(pl + inw, sy0), v3(pl + inw, sy1), v3(pl, sy1)], i3, Vec3::Y);
        g.quad(deep, v3(hl, 7.7), i3 * hall_d, Vec3::Y * (sy1 - 7.7));
        g.quad(deep, v3(hr + back, 7.7), o3 * hall_d, Vec3::Y * (sy1 - 7.7));
        g.quad(deep, v3(hl + back, 7.7), t3 * 4.4, Vec3::Y * (sy1 - 7.7));
        poly_down(g, deep, &[v3(hl, sy1), v3(hr, sy1), v3(hr + back, sy1), v3(hl + back, sy1)]);
        poly_up(g, m.concrete, &[v3(hl, 7.7), v3(hr, 7.7), v3(hr + back, 7.7), v3(hl + back, 7.7)]);
        g.quad(Key::new(Mat::Plain(60), [0.1, 0.14, 0.18]), v3(uv(-0.9, fb) + inw + back + out * 0.03, 7.7), t3 * 1.5, Vec3::Y * 2.3);
        g.member(dark, v3(uv(-0.15, fb) + inw + back + out * 0.05, 7.7), v3(uv(-0.15, fb) + inw + back + out * 0.05, 10.0), 0.05, 0.05, o3);
        reveal(g, deep, bl, br, alcove, 0.9);
        // the door (model by tboiston, CC BY, modelled in centimetres): a grey steel leaf with a vision light
        place(g, f, "sf_4da6b53842", None, uv(6.85, fb) + inw - out * 0.82, 7.7, t, 0.01);
        g.shade = false;
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
    wall_holes(g, m.sandstone, uv(a1, back), uv(a0, back), 0.0, top, &[[a1 - h1, a1 - h0, 0.0, hc]]);
    // the low block behind, which the hall runs into (lidar: 8-17 m back there)
    let rear = bb + 0.9;
    prism(g, m, &uv, pale, &[(h0 - 3.0, back), (h0 - 0.06, back), (h0 - 0.06, rear), (h0 - 3.0, rear)], 0.0, &flat(8.5), false);
    prism(g, m, &uv, pale, &[(h1 + 0.06, back), (h1 + 3.0, back), (h1 + 3.0, rear), (h1 + 0.06, rear)], 0.0, &flat(8.5), false);
    prism(g, m, &uv, pale, &[(h0, bb + 0.06), (h1, bb + 0.06), (h1, rear), (h0, rear)], 0.0, &flat(8.5), false);
    prism(g, m, &uv, pale, &[(h0, back), (h1, back), (h1, bb), (h0, bb)], hc + 0.3, &flat(8.5), false);
    poly_up(g, m.concrete, &[v3(uv(a0, fb), top - 0.3), v3(uv(a1, fb), top - 0.3), v3(uv(a1, back), top - 0.3), v3(uv(a0, back), top - 0.3)]);
    for (r, y0, y1) in [
        ([(a0, fb), (h0, fb), (h0, back), (a0, back)], 0.0, top),
        ([(h1, fb), (a1, fb), (a1, back), (h1, back)], 0.0, top),
        ([(h0, fb), (h1, fb), (h1, back), (h0, back)], hc, top),
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

    // --- the stone podium right of the hall (measured on a zoomed street photo, on its front plane 4.3 m out from
    // the face): 8.76 m high; its left 11 m is a terrace sunk behind a parapet (floor at the slot's level) with a
    // canopy, a gabled hut and a white counter; its front has poster panels, two small barred windows and a wide
    // dark opening over a planter; bougainvillea along the top
    let (pf, ptop, tf, te, pw) = (fb - 4.3, 8.76f32, 7.7f32, 19.0f32, 0.4f32);
    let pu = |a: f32| a - h1;
    let (open, win1, win2) = ([pu(19.1), pu(23.9), 4.92, 6.06], [pu(11.8), pu(12.9), 4.63, 5.12], [pu(15.8), pu(17.0), 4.63, 5.12]);
    let (fl, fr) = (uv(h1, pf), uv(56.0, pf));
    wall_holes(g, m.sandstone, fl, fr, 0.0, ptop, &[open, win1, win2]);
    wall_holes(g, m.sandstone, uv(h1, fb), uv(h1, pf), 0.0, ptop, &[]);
    wall_holes(g, m.sandstone, uv(56.0, pf), uv(56.0, fb), 0.0, ptop, &[]);
    // the opening: a deep dark recess under a pale lintel; the windows: shallow, barred
    reveal(g, m.sandstone, fl, fr, open, 1.6);
    g.quad(dark, v3(uv(19.1, pf + 1.6), 4.92), t3 * 4.8, Vec3::Y * 1.14);
    g.quad(pale, v3(uv(19.1, pf) + out * 0.02, 6.06), t3 * 4.8, Vec3::Y * 0.22);
    for w in [win1, win2] {
        reveal(g, m.sandstone, fl, fr, w, 0.3);
        let (wa, wb) = (w[0] + h1, w[1] + h1);
        g.quad(pane, v3(uv(wa, pf + 0.3), w[2]), t3 * (wb - wa), Vec3::Y * (w[3] - w[2]));
        let bars = ((wb - wa) / 0.14) as i32;
        for k in 1..bars {
            let q = uv(wa + (wb - wa) * k as f32 / bars as f32, pf + 0.12);
            g.member(frame, v3(q, w[2]), v3(q, w[3]), 0.02, 0.02, o3);
        }
    }
    // the terrace: floor, parapet (inner faces and tops), the raised part's end and top
    poly_up(g, m.concrete, &[v3(uv(h1 + pw, pf + pw), tf), v3(uv(te, pf + pw), tf), v3(uv(te, fb), tf), v3(uv(h1 + pw, fb), tf)]);
    wall_holes(g, m.sandstone, uv(te, pf + pw), uv(h1 + pw, pf + pw), tf, ptop, &[]);
    wall_holes(g, m.sandstone, uv(h1 + pw, pf + pw), uv(h1 + pw, fb), tf, ptop, &[]);
    wall_holes(g, m.sandstone, uv(te, fb), uv(te, pf + pw), tf, ptop, &[]);
    poly_up(g, stone_flat(), &[v3(uv(h1, pf), ptop), v3(uv(te, pf), ptop), v3(uv(te, pf + pw), ptop), v3(uv(h1, pf + pw), ptop)]);
    poly_up(g, stone_flat(), &[v3(uv(h1, pf + pw), ptop), v3(uv(h1 + pw, pf + pw), ptop), v3(uv(h1 + pw, fb), ptop), v3(uv(h1, fb), ptop)]);
    poly_up(g, m.concrete, &[v3(uv(te, pf), ptop), v3(uv(56.0, pf), ptop), v3(uv(56.0, fb), ptop), v3(uv(te, fb), ptop)]);
    for (r, y0, y1) in [
        ([(h1, pf), (56.0, pf), (56.0, fb), (h1, fb)], 0.0, tf),
        ([(te, pf), (56.0, pf), (56.0, fb), (te, fb)], tf, ptop),
        ([(h1, pf), (te, pf), (te, pf + pw), (h1, pf + pw)], tf, ptop),
        ([(h1, pf + pw), (h1 + pw, pf + pw), (h1 + pw, fb), (h1, fb)], tf, ptop),
    ] {
        g.block_ring(&r.map(|(a, b)| uv(a, b)), y0, y1);
    }
    // on the front: a small white sign and five poster panels on a ledge
    g.quad(Key::new(Mat::Plain(150), [0.7, 0.7, 0.68]), v3(uv(8.47, pf) + out * 0.03, 1.19), t3 * 0.58, Vec3::Y * 0.75);
    for k in 0..5 {
        let a = 9.32 + k as f32 * 1.8;
        let q = uv(a, pf) + out * 0.03;
        g.quad(Key::new(Mat::Plain(150), [0.035, 0.035, 0.035]), v3(q, 1.01), t3 * 1.63, Vec3::Y * 1.5);
        let pic = if k == 2 { [0.68, 0.63, 0.5] } else { [0.36, 0.29, 0.21] };
        g.quad(Key::new(Mat::Plain(170), pic), v3(q + t * 0.1 + out * 0.01, 1.11), t3 * 1.43, Vec3::Y * 1.3);
    }
    g.member(m.concrete, v3(uv(9.2, pf) + out * 0.12, 0.96), v3(uv(18.3, pf) + out * 0.12, 0.96), 0.24, 0.1, Vec3::Y);
    // planting: heavy red bougainvillea with dry strands over the terrace's parapet, a lighter line beyond, and
    // the planter under the opening
    vines(g, uv(h1 + 0.1, pf), uv(13.2, pf), out, ptop, 2.3, 21, true);
    vines(g, uv(h1, pf + 0.1), uv(h1, fb - 0.1), -t, ptop, 2.0, 22, true);
    bougainvillea(g, uv(h1 + 0.1, pf), uv(13.4, pf), out, ptop, 1.1, 9, true);
    bougainvillea(g, uv(h1, pf + 0.2), uv(h1, fb - 0.2), -t, ptop, 0.9, 11, true);
    bougainvillea(g, uv(14.5, pf), uv(40.0, pf), out, ptop, 0.6, 10, true);
    bougainvillea(g, uv(19.3, pf), uv(23.7, pf), out, 4.92, 0.7, 12, true);
    // the canopy over the terrace's left part: a thin dark roof from the parapet back to the face, on thin posts
    let canopy = Key::new(Mat::Plain(225), [0.035, 0.035, 0.035]);
    let (c0, c1, cy) = (8.2f32, 13.1f32, 9.62f32);
    g.member(canopy, v3(uv(c0, (pf + 0.2 + fb) / 2.0), cy), v3(uv(c1, (pf + 0.2 + fb) / 2.0), cy), fb - pf - 0.2, 0.3, Vec3::Y);
    g.member(Key::new(Mat::Plain(200), [0.1, 0.1, 0.1]), v3(uv(c0, pf + 0.2), cy + 0.12), v3(uv(c1, pf + 0.2), cy + 0.12), 0.05, 0.08, Vec3::Y);
    for a in [c0 + 0.1, (c0 + c1) / 2.0, c1 - 0.1] {
        g.member(canopy, v3(uv(a, pf + 0.3), tf), v3(uv(a, pf + 0.3), cy - 0.15), 0.07, 0.07, o3);
    }
    // the hut beside it: a white kiosk under a dark gabled roof, gable to the street
    {
        let (x0, x1, z0, z1) = (13.1f32, 14.7f32, pf + 0.7, pf + 2.3);
        let (eave, apex, rg) = (9.7f32, 10.3f32, (x0 + x1) / 2.0);
        prism(g, m, &uv, m.white, &[(x0 + 0.4, z0 + 0.1), (x1 - 0.4, z0 + 0.1), (x1 - 0.4, z1), (x0 + 0.4, z1)], tf, &flat(eave), false);
        for (xa, xb) in [(x0, rg), (x1, rg)] {
            let quad = [v3(uv(xa, z0 - 0.1), eave), v3(uv(xb, z0 - 0.1), apex), v3(uv(xb, z1 + 0.1), apex), v3(uv(xa, z1 + 0.1), eave)];
            let nn = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
            let quad = if nn.y < 0.0 { [quad[0], quad[3], quad[2], quad[1]] } else { quad };
            let nn = (quad[1] - quad[0]).cross(quad[2] - quad[0]).normalize();
            let r = Vec3::Y.cross(nn).normalize();
            g.poly(canopy, &quad, r, nn.cross(r));
            // its underside, so the roof is not see-through from the street
            g.poly(canopy, &[quad[0], quad[3], quad[2], quad[1]], -r, nn.cross(r));
        }
        g.poly(canopy, &[v3(uv(x0, z0 - 0.1), eave), v3(uv(x1, z0 - 0.1), eave), v3(uv(rg, z0 - 0.1), apex)], t3, Vec3::Y);
    }
    // the white counter along the parapet: a long box with four dark openings
    {
        let (x0, x1, z0) = (14.76f32, 18.13f32, pf + 1.0);
        prism(g, m, &uv, m.white, &[(x0, z0), (x1, z0), (x1, z0 + 0.7), (x0, z0 + 0.7)], tf, &flat(9.45), false);
        for k in 0..4 {
            g.quad(dark, v3(uv(x0 + 0.15 + k as f32 * 0.83, z0) + out * 0.02, 9.02), t3 * 0.63, Vec3::Y * 0.3);
        }
    }
    // cream concrete bollards 7 m out from the face, 1.8 m apart (close street photo)
    let bollard = Key::new(Mat::Plain(175), [0.62, 0.57, 0.47]);
    let mut a = -10.0f32;
    while a <= 24.0 {
        let p = uv(a, fb - 7.0);
        let y = terr(p);
        g.tube(bollard, v3(p, y - 0.3), v3(p, y + 1.05), 0.14, 12);
        g.tube(bollard, v3(p, y + 1.05), v3(p, y + 1.1), 0.1, 12);
        g.block_box(Vec3::new(p.x - 0.14, 0.0, p.y - 0.14), Vec3::X * 0.28, Vec3::Z * 0.28, y - 0.3, y + 1.1);
        a += 1.55;
    }
}

// ---------------------------------------------------------------- grand stair

/// The grand stair from the home plate plaza up to the podium under the footbridge. Its plan from the map's thirty
/// stair-flight lines (three tiers heading 120 degrees, the lower ones fanning out to the north-east); its section
/// from the lidar along the run: the top nosing 2.6 m out from the bridge's centre line at 7.70 m, landings at
/// 5.2 and 2.6 m, the foot 19.1 m out (flights of 15, 16 and 16 risers at 0.58 m per metre). Round it the podium:
/// the lidar's 7.70 m footprint outside the frame - under the bridge, the pocket east of it, the planted terrace
/// beside the stair (its 12-14 m returns are trees) and the terrace along the frame to the east.
fn grand_stair(g: &mut Geo, f: &mut Furnisher, m: &Mats) {
    let v3 = |p: Vec2, y: f32| Vec3::new(p.x, y, p.y);
    let (d, c) = (Vec2::new(0.866, 0.5), Vec2::new(-0.5, 0.866));
    let uv = |t: f32, w: f32| Vec2::new(-7.7, 88.9) + d * t + c * w - HOME;
    let wl = |x: f32, z: f32| Vec2::new(x, z) - HOME;
    let tiers = [(15i32, 0.0f32), (16, 2.2), (16, 6.0)];
    let lands = [1.6f32, 1.6];
    let top = CONCOURSE_Y;
    let (ri, tr, wide, t_top) = (top / 47.0, 0.2835f32, 12.5f32, 2.6f32);
    let paving = paving();
    let riser = Key::new(Mat::Plain(200), [0.4, 0.39, 0.37]);
    let r3 = Vec3::new(-c.x, 0.0, -c.y);
    // the flights: (start along the run, height at its top, risers, north-east extension)
    let mut flights: Vec<(f32, f32, i32, f32)> = vec![];
    let (mut t, mut h) = (t_top, top);
    for (i, &(n, ext)) in tiers.iter().enumerate() {
        flights.push((t, h, n, ext));
        t += n as f32 * tr;
        h -= n as f32 * ri;
        if i < lands.len() {
            // the landing, as wide as the flight below it
            let ext2 = tiers[i + 1].1;
            let q = [uv(t, -ext2), uv(t + lands[i], -ext2), uv(t + lands[i], wide), uv(t, wide)];
            poly_up(g, paving, &q.map(|p| v3(p, h)));
            g.block_ring(&q, -0.5, h);
            t += lands[i];
        }
    }
    let total = t;
    for &(t0, h0, n, ext) in &flights {
        for k in 0..n {
            let (ts, te, y) = (t0 + k as f32 * tr, t0 + (k + 1) as f32 * tr, h0 - (k + 1) as f32 * ri);
            let q = [uv(ts, -ext), uv(te, -ext), uv(te, wide), uv(ts, wide)];
            poly_up(g, paving, &q.map(|p| v3(p, y)));
            g.quad(riser, v3(uv(ts, wide), y), r3 * (wide + ext), Vec3::Y * ri);
            g.block_ring(&q, -0.5, y);
        }
        // handrails: at both sides and the thirds
        let (y0, y1, t1) = (h0, h0 - n as f32 * ri, t0 + n as f32 * tr);
        let mut ws = vec![0.25, wide / 3.0, wide * 2.0 / 3.0, wide - 0.25];
        if ext > 0.5 {
            ws.push(-ext + 0.25);
        }
        for w in ws {
            g.tube(m.rail, v3(uv(t0, w), y0 + 0.95), v3(uv(t1, w), y1 + 0.95), 0.025, 6);
            for fr in [0.0f32, 0.5, 1.0] {
                let (tt, yy) = (t0 + (t1 - t0) * fr, y0 + (y1 - y0) * fr);
                g.tube(m.rail, v3(uv(tt, w), yy - ri), v3(uv(tt, w), yy + 0.95), 0.02, 6);
            }
        }
    }
    // the stair's surface height along the run, for the cheek walls
    let fl = flights.clone();
    let height = move |t: f32| -> f32 {
        let mut y = top;
        for &(t0, h0, n, _) in &fl {
            if t >= t0 {
                y = h0 - ((t - t0) / tr).clamp(0.0, n as f32) * ri;
            }
        }
        y
    };
    let cheek = |t: f32| height(t) + 1.05;
    // stone cheeks: along the tower side (tapering with the gap to the tower), and stepping out with the fan
    prism(g, m, &uv, m.sandstone, &[(t_top, wide), (total, wide), (total, wide + 0.3), (t_top, wide + 1.1)], -0.5, &cheek, false);
    let (t1, t2) = (flights[1].0 - lands[0], flights[2].0 - lands[1]);
    prism(g, m, &uv, m.sandstone, &[(t_top, -0.4), (t1, -0.4), (t1, 0.0), (t_top, 0.0)], -0.5, &cheek, false);
    prism(g, m, &uv, m.sandstone, &[(t1 - 0.4, -2.6), (t2, -2.6), (t2, -2.2), (t1 - 0.4, -2.2)], -0.5, &cheek, false);
    prism(g, m, &uv, m.sandstone, &[(t1 - 0.4, -2.2), (t1, -2.2), (t1, -0.4), (t1 - 0.4, -0.4)], -0.5, &cheek, false);
    prism(g, m, &uv, m.sandstone, &[(t2 - 0.4, -6.4), (total, -6.4), (total, -6.0), (t2 - 0.4, -6.0)], -0.5, &cheek, false);
    prism(g, m, &uv, m.sandstone, &[(t2 - 0.4, -6.0), (t2, -6.0), (t2, -2.6), (t2 - 0.4, -2.6)], -0.5, &cheek, false);
    // --- the podium: a stone-walled mass with paving on top; `parapet` edges (index of their first point) get a
    // 1 m stone parapet (they stand over the plaza)
    let id = |x: f32, z: f32| Vec2::new(x, z);
    let flat = |hh: f32| move |_u: f32| hh;
    let mut podium = |g: &mut Geo, ring: &[Vec2], parapet: &[usize]| {
        let r: Vec<(f32, f32)> = ring.iter().map(|p| (p.x, p.y)).collect();
        prism(g, m, &id, m.sandstone, &r, -0.5, &flat(top), false);
        poly_up(g, paving, &ring.iter().map(|&p| v3(p, top + 0.005)).collect::<Vec<_>>());
        let centre = ring.iter().copied().sum::<Vec2>() / ring.len() as f32;
        for &k in parapet {
            let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
            let e = (q - p).normalize();
            let n = if e.perp().dot(centre - p) > 0.0 { e.perp() } else { -e.perp() } * 0.4;
            let pr = [p, q, q + n, p + n];
            prism(g, m, &id, m.sandstone, &pr.map(|x| (x.x, x.y)), top, &flat(top + 1.0), false);
        }
    };
    // the head of the stair back to the offices, and under the bridge to the frame
    podium(g, &[uv(-4.6, 0.0), uv(t_top, 0.0), uv(t_top, wide + 1.1), uv(-4.6, wide + 1.1)], &[]);
    let (bt, bb) = (Vec2::new(BRIDGE_TOWER.0, BRIDGE_TOWER.1) - HOME, Vec2::new(BRIDGE_BOWL.0, BRIDGE_BOWL.1) - HOME);
    let be = (bt - bb).normalize();
    let bs = be.perp() * (BRIDGE_W / 2.0 + 0.3);
    let s_a = (uv(0.0, 0.0) - bb).dot(be);
    let (p0, p1) = (bb - be * 1.0, bb + be * (s_a + 1.5));
    podium(g, &[p0 - bs, p1 - bs, p1 + bs, p0 + bs], &[]);
    // the pocket east of the bridge, the planted terrace beside the stair, and the terrace along the frame (the
    // lidar footprint's outline, in convex pieces)
    podium(g, &[wl(3.8, 76.1), wl(13.8, 74.1), wl(14.8, 82.1), wl(2.0, 83.5), wl(-1.0, 80.0)], &[]);
    podium(g, &[wl(-4.0, 89.6), wl(3.3, 90.1), wl(13.3, 89.1), wl(15.8, 87.6), wl(14.8, 82.1), wl(2.0, 83.5), wl(-2.0, 87.0)], &[0, 1, 2, 3]);
    podium(g, &[wl(13.8, 74.1), wl(26.8, 70.1), wl(24.3, 80.1), wl(17.3, 80.1), wl(14.8, 82.1)], &[2, 3]);
    podium(g, &[wl(26.8, 70.1), wl(38.3, 62.6), wl(39.8, 72.1), wl(35.3, 74.6), wl(24.3, 80.1)], &[2, 3]);
    // (stopping short of the stair core that stands on its east tip)
    podium(g, &[wl(38.3, 62.6), wl(41.6, 59.0), wl(43.6, 61.0), wl(39.0, 67.0)], &[1, 2]);
    podium(g, &[wl(38.3, 62.6), wl(39.0, 67.0), wl(39.8, 72.1)], &[1]);
    // trees on the planted terrace
    if let Some((mn, mx)) = f.bounds("sf_380a2c4bfa") {
        for (k, (x, z)) in [(2.0f32, 86.6f32), (8.0, 86.2), (13.0, 85.6)].into_iter().enumerate() {
            let sc = 1.6f32;
            let (cx, cz) = ((mn[0] + mx[0]) * 0.5, (mn[2] + mx[2]) * 0.5);
            let rot = 40.0 + k as f64 * 75.0;
            let r = (rot as f32).to_radians();
            // the trunk is the model's origin: offset the footprint's centre so the trunk stands on the spot
            let (ux, uz, zx, zz) = (r.sin(), -r.cos(), r.cos(), r.sin());
            let (ox, oz) = (-cx * sc, -cz * sc);
            let [wx, wz] = g.world_xz(wl(x, z));
            f.out.push(crate::library::Placement { asset: "sf_380a2c4bfa".into(), part: crate::library::ALL_PARTS, x: wx - (ox * ux + oz * zx) as f64, z: wz - (ox * uz + oz * zz) as f64, y: g.world_y(top), rot, scale: [sc; 3], tint: [0.72; 3], lod: 0, indoor: false });
        }
    }
}
