//! Models the project makes itself, written as glTF into the asset inbox for `fps assets` to compile.
//!
//! The ballpark chair is modelled on the fixed tip-up stadium chair in the reference photos (a moulded back with a
//! rounded top and a number plate, a self-rising seat, cast standards with armrests and a rear cupholder): no
//! downloadable model matched it, and tens of thousands are drawn, so it is built here with few triangles.
//! One small texture carries its paints in columns (u), baked occlusion down each column (v) and the seat numbers.
use glam::{Vec2, Vec3};
use std::io::Write;

#[derive(Default)]
struct Mesh {
    pos: Vec<Vec3>,
    nrm: Vec<Vec3>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    /// Coarser versions as index lists over the same vertices, with each one's error in metres.
    lods: Vec<(Vec<u32>, f32)>,
}

/// How far each coarser level strays from the full model, metres: the renderer switches to a level once that is
/// under a pixel. (The usual automatic reduction cannot be trusted with these models: their flat faces do not share
/// vertices, so it would drop whole faces and call the result exact.)
const LOD_ERROR: [f32; 3] = [0.03, 0.09, 0.22];

#[derive(Clone, Copy)]
enum Paint {
    Plastic,
    Iron,
    Pan,
    Plate,
}

/// sRGB colour and roughness of each paint, with the span of the texture (in u) it takes.
/// The plastic is matt (sun-chalked): a glossier dark paint turns pale grey whenever the sun is behind the viewer.
const PAINTS: [([u8; 3], f32, f32, f32); 4] = [([33, 39, 60], 0.85, 0.0, 0.5), ([24, 27, 38], 0.72, 0.5, 0.75), ([50, 50, 56], 0.9, 0.75, 0.875), ([226, 226, 220], 0.5, 0.875, 1.0)];
/// Darkest baked shading (the bottom of the swatches).
const SHADE_MIN: f32 = 0.2;
/// Texture size; the swatches take the rows above `SWATCH_ROWS`, the number plates a grid below.
const TEX: u32 = 512;
const SWATCH_ROWS: u32 = 368;
/// Seat numbers on the plates: `NUM_COLS` x 3 cells of `NUM_CELL` pixels from row `NUM_TOP`.
pub const SEAT_NUMBERS: usize = 24;
const NUM_COLS: u32 = 8;
const NUM_CELL: (u32, u32) = (64, 40);
const NUM_TOP: u32 = 384;

fn swatch(p: Paint, shade: f32) -> [f32; 2] {
    let (_, _, u0, u1) = PAINTS[p as usize];
    let t = ((1.0 - shade) / (1.0 - SHADE_MIN)).clamp(0.0, 1.0);
    [(u0 + u1) / 2.0, (0.04 + t * 0.92) * SWATCH_ROWS as f32 / TEX as f32]
}

/// Where plate `n` (from 1) sits in the texture: (u0, v0, u1, v1).
fn number_cell(n: usize) -> [f32; 4] {
    let k = (n.clamp(1, SEAT_NUMBERS) - 1) as u32;
    let (x, y) = ((k % NUM_COLS) * NUM_CELL.0, NUM_TOP + (k / NUM_COLS) * NUM_CELL.1);
    let t = TEX as f32;
    [(x as f32 + 1.0) / t, (y as f32 + 1.0) / t, ((x + NUM_CELL.0) as f32 - 1.0) / t, ((y + NUM_CELL.1) as f32 - 1.0) / t]
}

impl Mesh {
    /// A flat convex polygon facing `out`.
    fn poly(&mut self, pts: &[Vec3], out: Vec3, p: Paint, shade: f32) {
        self.poly_uv(pts, out, &vec![swatch(p, shade); pts.len()]);
    }

    fn poly_uv(&mut self, pts: &[Vec3], out: Vec3, uvs: &[[f32; 2]]) {
        let mut n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
        let flip = n.dot(out) < 0.0;
        if flip {
            n = -n;
        }
        let b = self.pos.len() as u32;
        for (&q, &uv) in pts.iter().zip(uvs) {
            self.pos.push(q);
            self.nrm.push(n);
            self.uv.push(uv);
        }
        for i in 1..pts.len() as u32 - 1 {
            self.idx.extend(if flip { [b, b + i + 1, b + i] } else { [b, b + i, b + i + 1] });
        }
    }

    /// A convex outline in the (z, y) plane extruded from `x0` to `x1`.
    fn prism(&mut self, zy: &[Vec2], x0: f32, x1: f32, p: Paint, shade: f32) {
        self.prism_open(zy, x0, x1, p, shade, &[]);
    }

    /// The same, leaving out the sides that start at the outline points `open` (hidden, or built by the caller).
    fn prism_open(&mut self, zy: &[Vec2], x0: f32, x1: f32, p: Paint, shade: f32, open: &[usize]) {
        let c = zy.iter().copied().sum::<Vec2>() / zy.len() as f32;
        let at = |x: f32, q: Vec2| Vec3::new(x, q.y, q.x);
        self.poly(&zy.iter().map(|&q| at(x0, q)).collect::<Vec<_>>(), -Vec3::X, p, shade);
        self.poly(&zy.iter().map(|&q| at(x1, q)).collect::<Vec<_>>(), Vec3::X, p, shade);
        for i in (0..zy.len()).filter(|i| !open.contains(i)) {
            let (a, b) = (zy[i], zy[(i + 1) % zy.len()]);
            let mid = (a + b) / 2.0 - c;
            self.poly(&[at(x0, a), at(x1, a), at(x1, b), at(x0, b)], Vec3::new(0.0, mid.y, mid.x), p, shade);
        }
    }

    fn slab(&mut self, lo: Vec3, hi: Vec3, p: Paint, shade: f32) {
        self.prism(&[Vec2::new(lo.z, lo.y), Vec2::new(hi.z, lo.y), Vec2::new(hi.z, hi.y), Vec2::new(lo.z, hi.y)], lo.x, hi.x, p, shade);
    }

    /// A smooth sheet through a grid of points (rows of columns), facing `out`, shaded per strip between two rows.
    /// Every face of these models samples one point of the texture: a face whose texture coordinates vary would be
    /// drawn from a blurred mip at a distance, where the paints run into one another.
    fn sheet(&mut self, grid: &[Vec<Vec3>], out: Vec3, p: Paint, shade: impl Fn(usize) -> f32) {
        let (rows, cols) = (grid.len(), grid[0].len());
        let normal = |i: usize, j: usize| {
            let du = grid[j][(i + 1).min(cols - 1)] - grid[j][i.saturating_sub(1)];
            let dv = grid[(j + 1).min(rows - 1)][i] - grid[j.saturating_sub(1)][i];
            let n = du.cross(dv).normalize_or_zero();
            if n.dot(out) < 0.0 { -n } else { n }
        };
        for j in 0..rows - 1 {
            let b = self.pos.len() as u32;
            for jj in [j, j + 1] {
                for i in 0..cols {
                    self.pos.push(grid[jj][i]);
                    self.nrm.push(normal(i, jj));
                    self.uv.push(swatch(p, shade(j)));
                }
            }
            for i in 0..cols - 1 {
                let (q0, q1, q2, q3) = (b + i as u32, b + i as u32 + 1, b + (cols + i) as u32 + 1, b + (cols + i) as u32);
                let n = (grid[j][i + 1] - grid[j][i]).cross(grid[j + 1][i] - grid[j][i]);
                self.idx.extend(if n.dot(out) >= 0.0 { [q0, q1, q2, q0, q2, q3] } else { [q0, q2, q1, q0, q3, q2] });
            }
        }
    }

    /// An upright open cup: wall seen from both sides, with a floor.
    fn cup(&mut self, c: Vec3, r: f32, h: f32, sides: usize, p: Paint) {
        let ring: Vec<Vec3> = (0..sides).map(|k| { let a = k as f32 / sides as f32 * std::f32::consts::TAU; Vec3::new(a.cos() * r, 0.0, a.sin() * r) }).collect();
        for k in 0..sides {
            let (a, b) = (ring[k], ring[(k + 1) % sides]);
            let out = (a + b).normalize();
            let q = [c + a, c + b, c + b + Vec3::Y * h, c + a + Vec3::Y * h];
            self.poly(&q, out, p, 0.9);
            self.poly(&q, -out, p, 0.45);
        }
        let floor: Vec<Vec3> = ring.iter().map(|&q| c + q).collect();
        self.poly(&floor, -Vec3::Y, p, 0.7);
        let inside: Vec<Vec3> = ring.iter().map(|&q| c + q + Vec3::Y * 0.006).collect();
        self.poly(&inside, Vec3::Y, p, 0.35);
    }

    fn tris(&self) -> usize {
        self.idx.len() / 3
    }

    /// Adds another mesh's vertices and returns its triangles as an index list over this one.
    fn append(&mut self, o: &Mesh) -> Vec<u32> {
        let b = self.pos.len() as u32;
        self.pos.extend(&o.pos);
        self.nrm.extend(&o.nrm);
        self.uv.extend(&o.uv);
        o.idx.iter().map(|i| i + b).collect()
    }
}

// ---- the ballpark chair -------------------------------------------------------------------------------------------
// Model axes: x along the row, y up, +z the way the chair faces; z = 0 is the middle of the standards' feet.

/// Seat module: standards stand this far apart.
pub const SEAT_MODULE: f32 = 0.5;
const BACK_HALF: f32 = 0.2275;
const BACK_Y0: f32 = 0.43;
const BACK_Y1: f32 = 0.83;
const BACK_THICK: f32 = 0.022;
const STD_HALF: f32 = 0.0125;

/// The sitter's side of the back: `s` across (-1..1), `t` up (0..1).
fn back_point(s: f32, t: f32) -> Vec3 {
    let corner = |r: f32| {
        let e = ((s.abs() - (1.0 - r / BACK_HALF)) / (r / BACK_HALF)).clamp(0.0, 1.0);
        r * (1.0 - (1.0 - e * e).sqrt())
    };
    let y0 = BACK_Y0 + corner(0.035);
    let y1 = BACK_Y1 - 0.012 * s * s - corner(0.068);
    let y = y0 + (y1 - y0) * t;
    Vec3::new(s * BACK_HALF, y, back_z(y, s))
}

/// The back's line in section: reclined, with a lumbar swell, the top rolled back and the sides dished forward.
fn back_z(y: f32, s: f32) -> f32 {
    let h = ((y - BACK_Y0) / (BACK_Y1 - BACK_Y0)).clamp(0.0, 1.0);
    -0.12 - (y - BACK_Y0) * 0.344 + 0.014 * (h * std::f32::consts::PI).sin() - 0.03 * h.powi(4) + 0.03 * s * s
}

fn back_and_pan(m: &mut Mesh) {
    // back: a moulded shell
    let ss = [-1.0, -0.94, -0.8, -0.6, 0.0, 0.6, 0.8, 0.94, 1.0];
    let ts = [0.0, 0.3, 0.65, 1.0];
    let front: Vec<Vec<Vec3>> = ts.iter().map(|&t| ss.iter().map(|&s| back_point(s, t)).collect()).collect();
    let rear: Vec<Vec<Vec3>> = front.iter().map(|r| r.iter().map(|&q| q - Vec3::Z * BACK_THICK).collect()).collect();
    // the lower part sits in the folded seat's shadow
    m.sheet(&front, Vec3::Z, Paint::Plastic, |j| [0.45, 0.7, 1.0][j]);
    m.sheet(&rear, -Vec3::Z, Paint::Plastic, |j| [0.75, 0.9, 1.0][j]);
    let mid = Vec3::new(0.0, (BACK_Y0 + BACK_Y1) / 2.0, back_z((BACK_Y0 + BACK_Y1) / 2.0, 0.0));
    let (rows, cols) = (ts.len(), ss.len());
    let mut rim = |a: (usize, usize), b: (usize, usize)| {
        let q = [front[a.0][a.1], front[b.0][b.1], rear[b.0][b.1], rear[a.0][a.1]];
        let out = (q[0] + q[1]) / 2.0 - mid;
        m.poly(&q, Vec3::new(out.x, out.y, 0.0), Paint::Plastic, 0.9);
    };
    for i in 0..cols - 1 {
        rim((0, i), (0, i + 1));
        rim((rows - 1, i), (rows - 1, i + 1));
    }
    for j in 0..rows - 1 {
        rim((j, 0), (j + 1, 0));
        rim((j, cols - 1), (j + 1, cols - 1));
    }
    // seat, risen: it turns about a pivot below the back and rests nearly upright in front of it
    let a = 16.0f32.to_radians();
    let pivot = Vec2::new(-0.005, 0.35); // (z, y)
    let (u, w) = (Vec2::new(-a.sin(), a.cos()), Vec2::new(a.cos(), a.sin()));
    let at = |uu: f32, ww: f32| pivot + u * uu + w * ww;
    let pan = [at(-0.02, 0.022), at(0.335, 0.022), at(0.358, 0.016), at(0.37, 0.0), at(0.365, -0.02), at(0.34, -0.03), at(-0.02, -0.03)];
    m.prism_open(&pan, -0.22, 0.22, Paint::Plastic, 0.85, &[0]);
    // its underside, which is what shows: a rim round a recessed, ribbed pan
    let q = |x: f32, uu: f32, ww: f32| { let q = at(uu, ww); Vec3::new(x, q.y, q.x) };
    let out = Vec3::new(0.0, w.y, w.x);
    let (o, i, f) = ([-0.22, 0.22, -0.02, 0.335], [-0.18, 0.18, 0.03, 0.3], [-0.172, 0.172, 0.038, 0.292]);
    let ring = |r: [f32; 4], ww: f32| [q(r[0], r[2], ww), q(r[1], r[2], ww), q(r[1], r[3], ww), q(r[0], r[3], ww)];
    let (ro, ri, rf) = (ring(o, 0.022), ring(i, 0.022), ring(f, 0.011));
    for k in 0..4 {
        let n = (k + 1) % 4;
        m.poly(&[ro[k], ro[n], ri[n], ri[k]], out, Paint::Plastic, 0.95);
        let wall = (ri[k] + ri[n]) / 2.0 - (rf[k] + rf[n]) / 2.0;
        m.poly(&[ri[k], ri[n], rf[n], rf[k]], out * 0.5 - wall.normalize(), Paint::Plastic, 0.6);
    }
    let split = 0.215;
    m.poly(&[q(f[0], split, 0.011), q(f[1], split, 0.011), rf[2], rf[3]], out, Paint::Pan, 0.95);
    m.poly(&[rf[0], rf[1], q(f[1], split, 0.011), q(f[0], split, 0.011)], out, Paint::Iron, 0.6);
    for uu in [0.075f32, 0.125, 0.175] {
        let (a, b, top) = (uu - 0.007, uu + 0.007, 0.018);
        m.poly(&[q(-0.165, a, top), q(0.165, a, top), q(0.165, b, top), q(-0.165, b, top)], out, Paint::Iron, 0.9);
        m.poly(&[q(-0.165, a - 0.004, 0.011), q(0.165, a - 0.004, 0.011), q(0.165, a, top), q(-0.165, a, top)], out - Vec3::new(0.0, u.y, u.x), Paint::Iron, 0.7);
        m.poly(&[q(-0.165, b, top), q(0.165, b, top), q(0.165, b + 0.004, 0.011), q(-0.165, b + 0.004, 0.011)], out + Vec3::new(0.0, u.y, u.x), Paint::Iron, 0.85);
    }
    // hinge bar between the standards
    m.slab(Vec3::new(-SEAT_MODULE / 2.0 + STD_HALF, 0.333, -0.022), Vec3::new(SEAT_MODULE / 2.0 - STD_HALF, 0.365, 0.012), Paint::Iron, 0.6);
}

/// The seat's number plate, let into the top of the back.
fn number_plate(m: &mut Mesh, n: usize) {
    let (px, py, ph) = (0.037f32, BACK_Y1 - 0.052, 0.0225f32);
    let [u0, v0, u1, v1] = number_cell(n);
    let c = 0.3; // corners cut
    let outline = [(-1.0 + c, -1.0), (1.0 - c, -1.0), (1.0, -1.0 + c * 1.6), (1.0, 1.0 - c * 1.6), (1.0 - c, 1.0), (-1.0 + c, 1.0), (-1.0, 1.0 - c * 1.6), (-1.0, -1.0 + c * 1.6)];
    let pts: Vec<Vec3> = outline.iter().map(|&(a, b)| { let (x, y) = (a * px, py + b * ph); Vec3::new(x, y, back_z(y, x / BACK_HALF) + 0.004) }).collect();
    let uvs: Vec<[f32; 2]> = outline.iter().map(|&(a, b)| [u0 + (u1 - u0) * (a + 1.0) / 2.0, v0 + (v1 - v0) * (1.0 - b) / 2.0]).collect();
    m.poly_uv(&pts, Vec3::Z, &uvs);
}

/// A cast standard centred on `x`: two legs, a panel, the armrest, the wing the back bolts to and the rear cupholder.
fn standard(m: &mut Mesh, x: f32) {
    let v = Vec2::new;
    let (x0, x1) = (x - STD_HALF, x + STD_HALF);
    for sgn in [-1.0f32, 1.0] {
        m.prism(&[v(0.055 * sgn, 0.0), v(0.09 * sgn, 0.0), v(0.105 * sgn, 0.27), v(0.06 * sgn, 0.27)], x0, x1, Paint::Iron, 0.7);
        m.slab(Vec3::new(x - 0.025, 0.0, sgn * 0.075 - 0.032), Vec3::new(x + 0.025, 0.012, sgn * 0.075 + 0.032), Paint::Iron, 0.6);
    }
    m.prism(&[v(-0.105, 0.27), v(0.105, 0.27), v(0.115, 0.575), v(-0.125, 0.575)], x0, x1, Paint::Iron, 0.85);
    m.prism(&[v(-0.14, 0.575), v(0.115, 0.575), v(0.13, 0.59), v(0.13, 0.612), v(-0.14, 0.612)], x - 0.025, x + 0.025, Paint::Iron, 1.0);
    let e = |y: f32| back_z(y, 1.0);
    m.prism(&[v(e(0.5) - 0.04, 0.5), v(e(0.5) + 0.008, 0.5), v(e(0.77) + 0.008, 0.77), v(e(0.77) - 0.04, 0.77)], x0, x1, Paint::Iron, 0.85);
    m.cup(Vec3::new(x, 0.6, e(0.66) - 0.04 - 0.039), 0.043, 0.09, 8, Paint::Iron);
}

/// The aisle side of a row's end standard: a cast disc and the row plate (`side` = +1 or -1, the way it faces in x).
fn aisle_face(m: &mut Mesh, x: f32, side: f32) {
    let xf = x + side * (STD_HALF + 0.004);
    let disc: Vec<Vec3> = (0..10).map(|k| { let a = k as f32 / 10.0 * std::f32::consts::TAU; Vec3::new(xf, 0.49 + a.sin() * 0.052, a.cos() * 0.052) }).collect();
    m.poly(&disc, Vec3::X * side, Paint::Plastic, 1.0);
    let ring: Vec<Vec3> = disc.iter().map(|q| Vec3::new(x + side * STD_HALF, 0.49 + (q.y - 0.49) * 1.12, q.z * 1.12)).collect();
    for k in 0..10 {
        let (a, b) = (k, (k + 1) % 10);
        let out = (disc[a] + disc[b]) / 2.0 - Vec3::new(xf, 0.49, 0.0);
        m.poly(&[ring[a], ring[b], disc[b], disc[a]], out + Vec3::X * side * 0.03, Paint::Iron, 1.0);
    }
    m.poly(&[Vec3::new(xf, 0.385, -0.024), Vec3::new(xf, 0.385, 0.024), Vec3::new(xf, 0.42, 0.024), Vec3::new(xf, 0.42, -0.024)], Vec3::X * side, Paint::Plate, 1.0);
}

/// The chair at a coarser `level` (1 to 3): the same masses with the small parts left off, then little more than
/// the back and the risen seat.
fn seat_coarse(level: usize, number: usize) -> Mesh {
    let mut m = Mesh::default();
    let a = 16.0f32.to_radians();
    let pivot = Vec2::new(-0.005, 0.35);
    let (u, w) = (Vec2::new(-a.sin(), a.cos()), Vec2::new(a.cos(), a.sin()));
    let at = |uu: f32, ww: f32| pivot + u * uu + w * ww;
    let q = |x: f32, uu: f32, ww: f32| { let q = at(uu, ww); Vec3::new(x, q.y, q.x) };
    let out = Vec3::new(0.0, w.y, w.x);
    if level == 3 {
        // from far off: one block the size of the back and risen seat together, a module wide, so that rows keep
        // their weight against the concrete
        let hw = SEAT_MODULE / 2.0 - STD_HALF;
        let (y1, yb) = (BACK_Y1 - 0.02, BACK_Y0);
        let rear = |x: f32, y: f32| Vec3::new(x, y, back_z(y, 0.7) - BACK_THICK);
        m.poly(&[rear(-hw, yb - 0.1), rear(hw, yb - 0.1), rear(hw, y1), rear(-hw, y1)], -Vec3::Z, Paint::Plastic, 0.7);
        // the top stands for the shaded gap between the two and the dark standards, more than for the plastic
        m.poly(&[rear(-hw, y1), rear(hw, y1), q(hw, 0.36, 0.022), q(-hw, 0.36, 0.022)], Vec3::Y, Paint::Iron, 0.45);
        m.poly(&[q(-hw, -0.2, 0.022), q(hw, -0.2, 0.022), q(hw, 0.36, 0.022), q(-hw, 0.36, 0.022)], out, Paint::Iron, 0.85);
        // and the armrests' share of the tread in front
        m.poly(&[Vec3::new(-hw, 0.6, 0.0), Vec3::new(hw, 0.6, 0.0), Vec3::new(hw, 0.6, 0.075), Vec3::new(-hw, 0.6, 0.075)], Vec3::Y, Paint::Iron, 0.6);
        return m;
    }
    // back
    let (ss, ts): (&[f32], &[f32]) = if level == 1 { (&[-1.0, -0.8, 0.0, 0.8, 1.0], &[0.0, 0.5, 1.0]) } else { (&[-1.0, 1.0], &[0.0, 1.0]) };
    let front: Vec<Vec<Vec3>> = ts
        .iter()
        .map(|&t| ss.iter().map(|&s| if level == 1 { back_point(s, t) } else { let y = BACK_Y0 + (BACK_Y1 - 0.025 - BACK_Y0) * t; Vec3::new(s * BACK_HALF, y, back_z(y, 0.7)) }).collect())
        .collect();
    let rear: Vec<Vec<Vec3>> = front.iter().map(|r| r.iter().map(|&q| q - Vec3::Z * BACK_THICK).collect()).collect();
    let strips = ts.len() - 1;
    m.sheet(&front, Vec3::Z, Paint::Plastic, |j| if strips == 1 { 0.8 } else { [0.5, 0.95][j] });
    m.sheet(&rear, -Vec3::Z, Paint::Plastic, |j| if strips == 1 { 0.9 } else { [0.8, 1.0][j] });
    let (px, py, ph) = (0.037f32, BACK_Y1 - 0.052, 0.0225f32);
    let [u0, v0, u1, v1] = number_cell(number);
    let pt = |x: f32, y: f32| Vec3::new(x, y, back_z(y, 0.0) + 0.006);
    m.poly_uv(&[pt(-px, py - ph), pt(px, py - ph), pt(px, py + ph), pt(-px, py + ph)], Vec3::Z, &[[u0, v1], [u1, v1], [u1, v0], [u0, v0]]);
    // risen seat: its underside in bands (rim, ribbed pan, the lighter strip, rim), not laid over one another
    let bands: &[(f32, f32, Paint, f32)] = if level == 1 {
        &[(-0.02, 0.035, Paint::Plastic, 0.95), (0.035, 0.215, Paint::Iron, 0.65), (0.215, 0.295, Paint::Pan, 0.95), (0.295, 0.36, Paint::Plastic, 0.95)]
    } else {
        &[(-0.02, 0.215, Paint::Iron, 0.7), (0.215, 0.36, Paint::Pan, 0.8)]
    };
    for &(u0, u1, p, shade) in bands {
        m.poly(&[q(-0.22, u0, 0.022), q(0.22, u0, 0.022), q(0.22, u1, 0.022), q(-0.22, u1, 0.022)], out, p, shade);
    }
    let pan = [at(-0.02, 0.022), at(0.36, 0.022), at(0.36, -0.03), at(-0.02, -0.03)];
    m.prism_open(&pan, -0.22, 0.22, Paint::Plastic, 0.85, if level == 1 { &[0] } else { &[0, 2, 3] });
    standard_coarse(&mut m, -SEAT_MODULE / 2.0, level);
    m
}

fn standard_coarse(m: &mut Mesh, x: f32, level: usize) {
    let v = Vec2::new;
    let (x0, x1) = (x - STD_HALF, x + STD_HALF);
    let arm = [v(-0.14, 0.575), v(0.13, 0.575), v(0.13, 0.612), v(-0.14, 0.612)];
    match level {
        1 => {
            for sgn in [-1.0f32, 1.0] {
                m.prism_open(&[v(0.055 * sgn, 0.0), v(0.09 * sgn, 0.0), v(0.105 * sgn, 0.27), v(0.06 * sgn, 0.27)], x0, x1, Paint::Iron, 0.7, &[0, 2]);
            }
            m.prism_open(&[v(-0.105, 0.27), v(0.105, 0.27), v(0.115, 0.575), v(-0.125, 0.575)], x0, x1, Paint::Iron, 0.85, &[2]);
            m.prism(&arm, x - 0.025, x + 0.025, Paint::Iron, 1.0);
            let e = |y: f32| back_z(y, 1.0);
            m.prism_open(&[v(e(0.5) - 0.04, 0.5), v(e(0.5) + 0.008, 0.5), v(e(0.77) + 0.008, 0.77), v(e(0.77) - 0.04, 0.77)], x0, x1, Paint::Iron, 0.85, &[0, 1, 2]);
            let c = e(0.66) - 0.079;
            m.prism_open(&[v(c - 0.038, 0.6), v(c + 0.038, 0.6), v(c + 0.038, 0.69), v(c - 0.038, 0.69)], x - 0.038, x + 0.038, Paint::Iron, 0.8, &[]);
        }
        2 => {
            m.prism_open(&[v(-0.105, 0.0), v(0.105, 0.0), v(0.115, 0.575), v(-0.125, 0.575)], x0, x1, Paint::Iron, 0.8, &[0, 1, 2, 3]);
            m.prism_open(&arm, x - 0.025, x + 0.025, Paint::Iron, 1.0, &[0, 3]);
        }
        _ => m.prism_open(&[v(-0.105, 0.0), v(0.105, 0.0), v(0.13, 0.612), v(-0.14, 0.612)], x0, x1, Paint::Iron, 0.85, &[0, 1, 2, 3]),
    }
}

/// Gives `full` its coarser levels.
fn with_lods(mut full: Mesh, coarse: [Mesh; 3]) -> Mesh {
    for (k, c) in coarse.iter().enumerate() {
        let idx = full.append(c);
        // FPS_LOD_DEBUG=k shows level k from any distance (to look at it up close)
        let dbg = std::env::var("FPS_LOD_DEBUG").ok().and_then(|v| v.parse::<usize>().ok());
        full.lods.push((idx, dbg.map_or(LOD_ERROR[k], |d| if k < d { 0.0 } else { 1e6 })));
    }
    full
}

/// The plates' numbers, black on the plate's own colour, as one strip of cells.
fn numbers() -> std::io::Result<image::RgbImage> {
    use resvg::{tiny_skia, usvg};
    let err = |e: String| std::io::Error::other(e);
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_font_file(crate::root().join("data/megascans_inbox/fonts/Montserrat-SemiBold.ttf"))?;
    let (cw, ch) = NUM_CELL;
    let (w, h) = (NUM_COLS * cw, SEAT_NUMBERS as u32 / NUM_COLS * ch);
    let [r, g, b] = PAINTS[Paint::Plate as usize].0;
    let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="rgb({r},{g},{b})"/>"##);
    for k in 0..SEAT_NUMBERS as u32 {
        let (x, y) = ((k % NUM_COLS) * cw + cw / 2, (k / NUM_COLS) * ch + ch / 2);
        svg += &format!(r##"<text x="{x}" y="{}" text-anchor="middle" font-family="Montserrat" font-weight="600" font-size="27" fill="#15161a">{}</text>"##, y + 10, k + 1);
    }
    svg += "</svg>";
    let tree = usvg::Tree::from_str(&svg, &opt).map_err(|e| err(e.to_string()))?;
    let mut pix = tiny_skia::Pixmap::new(w, h).ok_or_else(|| err("bad size".into()))?;
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pix.as_mut());
    let img = image::RgbImage::from_fn(w, h, |x, y| { let p = pix.pixel(x, y).unwrap().demultiply(); image::Rgb([p.red(), p.green(), p.blue()]) });
    if !img.pixels().any(|p| p[0] < 80) {
        return Err(err("seat numbers: no ink (Montserrat missing from the inbox's fonts?)".into()));
    }
    Ok(img)
}

/// Colour and occlusion/roughness/metal textures.
fn texture() -> std::io::Result<(image::RgbImage, image::RgbImage)> {
    let paint = |x: u32| PAINTS.iter().find(|p| (x as f32 + 0.5) / TEX as f32 <= p.3).unwrap_or(&PAINTS[3]);
    let shade = |y: u32| { let v = (y as f32 + 0.5) / SWATCH_ROWS as f32; 1.0 - ((v - 0.04) / 0.92).clamp(0.0, 1.0) * (1.0 - SHADE_MIN) };
    let plate = PAINTS[Paint::Plate as usize];
    // flat colour (block compression bands a dark gradient); the baked shading goes into the occlusion channel
    let mut col = image::RgbImage::from_fn(TEX, TEX, |x, y| image::Rgb(if y >= SWATCH_ROWS { plate.0 } else { paint(x).0 }));
    image::imageops::replace(&mut col, &numbers()?, 0, NUM_TOP as i64);
    let surf = image::RgbImage::from_fn(TEX, TEX, |x, y| if y >= SWATCH_ROWS { image::Rgb([255, (plate.1 * 255.0) as u8, 0]) } else { image::Rgb([(shade(y) * 255.0) as u8, (paint(x).1 * 255.0) as u8, 0]) });
    Ok((col, surf))
}

fn png(img: &image::RgbImage) -> Vec<u8> {
    let mut c = std::io::Cursor::new(vec![]);
    img.write_to(&mut c, image::ImageFormat::Png).unwrap();
    c.into_inner()
}

/// Writes meshes as the parts of one single-material glTF, zipped into the inbox's `procedural` group.
fn write_model(id: &str, title: &str, parts: &[(String, Mesh)]) -> std::io::Result<()> {
    let mut bin: Vec<u8> = vec![];
    let (mut views, mut accessors, mut meshes, mut nodes) = (vec![], vec![], vec![], vec![]);
    for (k, (name, mesh)) in parts.iter().enumerate() {
        let mut view = |bin: &mut Vec<u8>, bytes: Vec<u8>, target: u32| {
            views.push(serde_json::json!({"buffer": 0, "byteOffset": bin.len(), "byteLength": bytes.len(), "target": target}));
            bin.extend(bytes);
        };
        let f3 = |v: &[Vec3]| v.iter().flat_map(|q| q.to_array()).flat_map(|c| c.to_le_bytes()).collect::<Vec<u8>>();
        view(&mut bin, f3(&mesh.pos), 34962);
        view(&mut bin, f3(&mesh.nrm), 34962);
        view(&mut bin, mesh.uv.iter().flatten().flat_map(|c| c.to_le_bytes()).collect(), 34962);
        view(&mut bin, mesh.idx.iter().flat_map(|i| i.to_le_bytes()).collect(), 34963);
        let (lo, hi) = mesh.pos.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), &q| (a.min(q), b.max(q)));
        let (nv, b) = (mesh.pos.len(), k * 4);
        accessors.push(serde_json::json!({"bufferView": b, "componentType": 5126, "count": nv, "type": "VEC3", "min": lo.to_array(), "max": hi.to_array()}));
        accessors.push(serde_json::json!({"bufferView": b + 1, "componentType": 5126, "count": nv, "type": "VEC3"}));
        accessors.push(serde_json::json!({"bufferView": b + 2, "componentType": 5126, "count": nv, "type": "VEC2"}));
        accessors.push(serde_json::json!({"bufferView": b + 3, "componentType": 5125, "count": mesh.idx.len(), "type": "SCALAR"}));
        let mut prim = serde_json::json!({"attributes": {"POSITION": b, "NORMAL": b + 1, "TEXCOORD_0": b + 2}, "indices": b + 3, "material": 0});
        if mesh.lods.len() == 3 {
            // read by the asset compiler (see `assets::extract_model`)
            prim["extras"] = serde_json::json!({"lods": mesh.lods.iter().map(|(l, e)| serde_json::json!({"error": e, "indices": l})).collect::<Vec<_>>()});
        }
        meshes.push(serde_json::json!({"primitives": [prim]}));
        nodes.push(serde_json::json!({"mesh": k, "name": name}));
    }
    let gltf = serde_json::json!({
        "asset": {"version": "2.0", "generator": "fps procmodel"},
        "scene": 0, "scenes": [{"nodes": (0..parts.len()).collect::<Vec<_>>()}], "nodes": nodes, "meshes": meshes,
        "materials": [{"name": "paint", "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicRoughnessTexture": {"index": 1}}, "occlusionTexture": {"index": 1}}],
        "textures": [{"source": 0}, {"source": 1}],
        "images": [{"uri": format!("{id}_col.png")}, {"uri": format!("{id}_surf.png")}],
        "buffers": [{"uri": format!("{id}.bin"), "byteLength": bin.len()}],
        "bufferViews": views, "accessors": accessors,
    });
    let (col, surf) = texture()?;
    let dir = crate::root().join("data/megascans_inbox/procedural");
    std::fs::create_dir_all(&dir)?;
    let mut z = zip::ZipWriter::new(std::fs::File::create(dir.join(format!("{id}.zip")))?);
    let o = zip::write::SimpleFileOptions::default();
    let meta = serde_json::json!({"id": id, "name": title, "tags": ["procedural", "seat"], "source": "crates/app/src/procmodel.rs"});
    for (name, bytes) in [(format!("{id}.gltf"), gltf.to_string().into_bytes()), (format!("{id}.bin"), bin), (format!("{id}_col.png"), png(&col)), (format!("{id}_surf.png"), png(&surf)), (format!("{id}.json"), meta.to_string().into_bytes())] {
        z.start_file(name, o)?;
        z.write_all(&bytes)?;
    }
    z.finish()?;
    let m = &parts[0].1;
    println!("wrote {id}: {} part(s) of {} triangles{}", parts.len(), m.tris(), if m.lods.is_empty() { String::new() } else { format!(", then {}", m.lods.iter().map(|(l, _)| (l.len() / 3).to_string()).collect::<Vec<_>>().join(", ")) });
    Ok(())
}

pub const SEAT: &str = "pm_ballpark_seat";
pub const SEAT_END: &str = "pm_ballpark_seat_end";
pub const SEAT_ROW_PLATE: &str = "pm_ballpark_row_plate";

pub fn all() -> std::io::Result<()> {
    // one chair with the standard on its left, as one part per seat number (part k carries number k + 1);
    // the row's last chair gets `SEAT_END` on its right
    let seats: Vec<(String, Mesh)> = (1..=SEAT_NUMBERS)
        .map(|n| {
            let mut seat = Mesh::default();
            back_and_pan(&mut seat);
            standard(&mut seat, -SEAT_MODULE / 2.0);
            number_plate(&mut seat, n);
            (format!("seat {n}"), with_lods(seat, [1, 2, 3].map(|l| seat_coarse(l, n))))
        })
        .collect();
    write_model(SEAT, "Ballpark chair, seat risen", &seats)?;
    let mut end = Mesh::default();
    standard(&mut end, 0.0);
    aisle_face(&mut end, 0.0, 1.0);
    let end = with_lods(end, [1, 2, 3].map(|l| {
        let mut m = Mesh::default();
        standard_coarse(&mut m, 0.0, l);
        if l < 3 {
            aisle_face(&mut m, 0.0, 1.0);
        }
        m
    }));
    write_model(SEAT_END, "Ballpark chair: end standard", &[("end".into(), end)])?;
    let mut plate = Mesh::default();
    aisle_face(&mut plate, 0.0, -1.0);
    write_model(SEAT_ROW_PLATE, "Ballpark chair: aisle face of the first standard", &[("plate".into(), plate)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chair_faces_plus_z_and_its_levels_hold_together() {
        let mut seat = Mesh::default();
        back_and_pan(&mut seat);
        standard(&mut seat, -SEAT_MODULE / 2.0);
        number_plate(&mut seat, 7);
        let full = seat.pos.len();
        let (lo, hi) = seat.pos.iter().fold((Vec3::MAX, Vec3::MIN), |(a, b), &q| (a.min(q), b.max(q)));
        // the back leans away behind the standards; the armrest is the foremost thing
        assert!(lo.z < -0.28 && hi.z > 0.1 && hi.z < 0.2, "depth {:.2}..{:.2}", lo.z, hi.z);
        assert!(lo.y.abs() < 1e-4 && (hi.y - BACK_Y1).abs() < 1e-4, "height {:.2}..{:.2}", lo.y, hi.y);
        assert!(hi.x - lo.x > SEAT_MODULE && hi.x - lo.x < SEAT_MODULE + 0.06, "one module wide with its standard: {:.3}", hi.x - lo.x);
        let seat = with_lods(seat, [1, 2, 3].map(|l| seat_coarse(l, 7)));
        let mut last = seat.tris();
        for (l, e) in &seat.lods {
            assert!(l.len() / 3 < last / 2 && *e > 0.0, "each level at most half the one before: {} after {last}", l.len() / 3);
            assert!(l.iter().all(|&i| i as usize >= full && (i as usize) < seat.pos.len()), "a level uses only its own vertices");
            last = l.len() / 3;
        }
        // nothing in a coarser level pokes out of the full chair's box (placement goes by the bounds)
        assert!(seat.pos[full..].iter().all(|q| q.cmpge(lo - 1e-3).all() && q.cmple(hi + 1e-3).all()));
        // every face samples the texture at one point, the number plate excepted
        for t in seat.idx.chunks(3) {
            let uv: Vec<[f32; 2]> = t.iter().map(|&i| seat.uv[i as usize]).collect();
            assert!(uv[0][1] > NUM_TOP as f32 / TEX as f32 || (uv[0] == uv[1] && uv[1] == uv[2]));
        }
    }
}
