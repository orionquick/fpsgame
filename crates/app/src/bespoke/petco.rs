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
}

fn mats() -> Mats {
    let s = |id: &'static str, t: [f32; 3]| Key::new(Mat::Surface(id), t);
    Mats {
        grass: s("sfklehwa", [0.62, 0.92, 0.5]).tile(2.0).rough(0.85),
        grass_dark: s("sfklehwa", [0.5, 0.78, 0.42]).tile(2.0).rough(0.85),
        dirt: s("pht_baseball_playground", [1.12, 0.82, 0.62]).tile(3.0),
        track: s("pht_red_laterite_soil_stones", [0.95, 0.78, 0.68]).tile(2.0),
        chalk: Key::new(Mat::Plain(200), [0.92, 0.92, 0.9]),
        rubber: Key::new(Mat::Plain(160), [0.95, 0.95, 0.94]),
        concrete: s("pht_smooth_concrete_floor", [1.25, 1.22, 1.15]).tile(2.0).wear(wear::STONE),
        riser: s("pht_smooth_concrete_floor", [1.05, 1.02, 0.97]).tile(2.0),
        stucco: s("tjeneevfw", [1.1, 0.92, 0.7]).tile(2.0),
        sandstone: s("pht_large_sandstone_blocks", [1.15, 0.98, 0.78]).tile(3.0).wear(wear::STONE),
        steel: Key::new(Mat::Plain(110), [0.86, 0.87, 0.87]).wear(wear::METAL),
        navy: Key::new(Mat::Plain(150), [0.035, 0.055, 0.12]),
        pad: Key::new(Mat::Plain(190), [0.03, 0.06, 0.15]),
        led: Key::new(Mat::Light, [0.25, 0.45, 1.1]),
        glass: Key::new(Mat::Glass, [0.55, 0.62, 0.66]),
        yellow: Key::new(Mat::Plain(120), [0.95, 0.72, 0.05]),
        rail: Key::new(Mat::Plain(70), [0.58, 0.6, 0.6]).wear(wear::METAL),
        roof: Key::new(Mat::Plain(140), [0.8, 0.81, 0.8]),
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
    bowl(&mut g, &mut f, &m, &st, &front);
    outfield(&mut g, &mut f, &m);
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
fn bowl(g: &mut Geo, f: &mut Furnisher, m: &Mats, st: &[Station], front: &[Vec2]) {
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
    for i in 0..st.len() - 1 {
        let (a, b) = (st[i], st[i + 1]);
        let mid_s = (arc[i] + arc[i + 1]) / 2.0;
        let in_aisle = aisle(mid_s);
        // --- front wall (true front line): padded, 1.1 m, LED ribbon on top behind home
        let (fa, fb) = (at(&a, a.front), at(&b, b.front));
        let e = (fb - fa).normalize_or_zero();
        g.poly(m.pad, &[v(fb, 0.0), v(fa, 0.0), v(fa, 1.1), v(fb, 1.1)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.poly(m.concrete, &[v(fa, 1.1), v(fb, 1.1), v(fb + b.n * 0.3, 1.1), v(fa + a.n * 0.3, 1.1)], Vec3::new(e.x, 0.0, e.y), Vec3::Y.cross(Vec3::new(e.x, 0.0, e.y)));
        g.block_ring(&[fa, fb, fb + b.n * 0.3, fa + a.n * 0.3], 0.0, 1.1);
        // --- lower bowl: treads and risers from the front line to the back
        let rows_a = ((a.lb_end - 0.8) / LB_PITCH).floor() as i32;
        let rows = rows_a.max(1);
        for j in 0..rows {
            let n0 = 0.8 + j as f32 * LB_PITCH;
            let n1 = n0 + LB_PITCH;
            if n1 < a.front.min(b.front) {
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
        // club-level back wall behind the terrace, up to the upper deck's soffit
        g.poly(m.stucco, &[v(at(&b, t_back_b), t_top), v(at(&a, t_back_a), t_top), v(at(&a, t_back_a), UP_Y - 1.0), v(at(&b, t_back_b), UP_Y - 1.0)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.block_ring(&[at(&a, t_back_a), at(&b, t_back_b), at(&b, t_back_b + 0.3), at(&a, t_back_a + 0.3)], t_top - 0.1, UP_Y);
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
        g.poly(m.riser, &[v(ua, UP_Y - 2.3), v(ub, UP_Y - 2.3), v(at(&b, up_back_b), up_top - 2.3), v(at(&a, up_back_a), up_top - 2.3)], Vec3::new(e.x, 0.0, e.y), Vec3::new(a.n.x, -0.6, a.n.y).normalize());
        // back rail and the sandstone back wall down to the ground
        g.tube(m.rail, v(at(&a, up_back_a), up_top + 1.1), v(at(&b, up_back_b), up_top + 1.1), 0.03, 6);
        // the back: a stucco parapet band behind the top rows only; below it the white steel frame stands open
        // over sandstone concourse blocks at street level (as in the exterior photos)
        let (wa, wb) = (at(&a, up_back_a + 0.2), at(&b, up_back_b + 0.2));
        let band = up_top - 3.0;
        g.poly(m.stucco, &[v(wa, band), v(wb, band), v(wb, up_top + 1.4), v(wa, up_top + 1.4)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        g.poly(m.stucco, &[v(wb, band), v(wa, band), v(wa, up_top + 1.4), v(wb, up_top + 1.4)], Vec3::new(-e.x, 0.0, -e.y), Vec3::Y);
        g.block_ring(&[wa, wb, wb + b.n * 0.4, wa + a.n * 0.4], band, up_top + 1.4);
        if i % 7 == 0 && i + 7 < st.len() {
            // raker beam under the deck, and the lattice bay to the next column: cross braces and a mid girder
            g.tube(m.steel, v(ua, UP_Y - 3.0), v(at(&a, up_back_a), up_top - 3.0), 0.28, 6);
            let c = st[i + 7];
            let wc = at(&c, c.upper + 0.3 + UP_ROWS * UP_PITCH + 0.2);
            let blk_top = 14.0;
            g.tube(m.steel, v(wa, blk_top), v(wc, band - 1.2), 0.11, 4);
            g.tube(m.steel, v(wc, blk_top), v(wa, band - 1.2), 0.11, 4);
            g.tube(m.steel, v(wa, (blk_top + band) / 2.0), v(wc, (blk_top + band) / 2.0), 0.13, 4);
        }
        if i % 7 == 0 {
            // raker column from the ground to the deck's back, and a brace to the terrace's back
            g.tube(m.steel, v(wa, 0.0), v(wa, band), 0.32, 8);
            g.tube(m.steel, v(wa, band * 0.45), v(at(&a, t_back_a + 0.2), t_top - 0.5), 0.18, 6);
            g.block_ring(&[wa - e * 0.35 - a.n * 0.35, wa + e * 0.35 - a.n * 0.35, wa + e * 0.35 + a.n * 0.35, wa - e * 0.35 + a.n * 0.35], 0.0, band);
        }
        // a white horizontal girder along the back at the upper concourse level
        g.poly(m.steel, &[v(wa, band - 1.2), v(wb, band - 1.2), v(wb, band), v(wa, band)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        // sandstone concourse blocks at street level, stepping in height along the bowl
        let blk = 9.0 + 5.0 * (((arc[i] / 23.0).floor() as i32).rem_euclid(3) as f32);
        let (ga, gb) = (at(&a, up_back_a - 6.0), at(&b, up_back_b - 6.0));
        let (oa, ob) = (wa + a.n * 4.0, wb + b.n * 4.0);
        g.poly(m.sandstone, &[v(oa, 0.0), v(ob, 0.0), v(ob, blk), v(oa, blk)], Vec3::new(e.x, 0.0, e.y), Vec3::Y);
        poly_up(g, m.concrete, &[v(ga, blk), v(gb, blk), v(ob, blk), v(oa, blk)]);
        g.block_ring(&[ga, gb, ob, oa], 0.0, blk);
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
    // end walls closing the bowl at both foul poles
    for s in [st[0], st[st.len() - 1]] {
        let e = s.n.perp();
        let pts = [s.p + s.n * s.front, s.p + s.n * (s.upper + 0.3 + UP_ROWS * UP_PITCH + 0.6)];
        let top = UP_Y + UP_ROWS * UP_RISE + 1.4;
        g.poly(m.sandstone, &[v(pts[0], 0.0), v(pts[1], 0.0), v(pts[1], top), v(pts[0], top * 0.25)], Vec3::new(s.n.x, 0.0, s.n.y), Vec3::Y);
        g.poly(m.sandstone, &[v(pts[1], 0.0), v(pts[0], 0.0), v(pts[0], top * 0.25), v(pts[1], top)], Vec3::new(-s.n.x, 0.0, -s.n.y), Vec3::Y);
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
        g.poly(m.sandstone, &[v(ba, 0.0), v(bb, 0.0), v(bb, y1 + 1.4), v(ba, y1 + 1.4)], ev, Vec3::Y);
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
        g.poly(m.sandstone, &[v(a.p + a.n * 18.0, 0.0), v(b.p + b.n * 18.0, 0.0), v(b.p + b.n * 18.0, 3.5), v(a.p + a.n * 18.0, 3.5)], -Vec3::new(e.x, 0.0, e.y), Vec3::Y);
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
        g.poly(m.sandstone, &[v(b.p + b.n * n0, 0.0), v(a.p + a.n * n0, 0.0), v(a.p + a.n * n0, 3.5), v(b.p + b.n * n0, 3.5)], -Vec3::new(e.x, 0.0, e.y) * -1.0, Vec3::Y);
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
    g.solid(m.sandstone, v(c - Vec2::splat(r * 0.8), 0.0), v(c + Vec2::splat(r * 0.8), shaft_top));
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
