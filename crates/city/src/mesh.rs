use crate::model::*;

#[derive(Default)]
pub struct Mesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub col: Vec<[f32; 3]>,
    pub idx: Vec<u32>,
}

impl Mesh {
    fn base(&self) -> u32 {
        self.pos.len() as u32
    }
    fn v(&mut self, p: [f64; 3], n: [f32; 3], c: [f32; 3]) {
        self.pos.push([p[0] as f32, p[1] as f32, p[2] as f32]);
        self.nrm.push(n);
        self.col.push(c);
    }
    fn quad(&mut self, a: [f64; 3], b: [f64; 3], cc: [f64; 3], d: [f64; 3], n: [f32; 3], col: [f32; 3]) {
        let i = self.base();
        for p in [a, b, cc, d] {
            self.v(p, n, col);
        }
        self.idx.extend([i, i + 1, i + 2, i, i + 2, i + 3]);
    }
}

fn srgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    let f = |c: u8| (c as f32 / 255.0).powf(2.2);
    [f(r), f(g), f(b)]
}

fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// Signed area in the x/z plane; positive = clockwise seen from above (z points south).
fn signed_area(r: &[[f64; 2]]) -> f64 {
    (0..r.len()).map(|i| { let (a, b) = (r[i], r[(i + 1) % r.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() * 0.5
}

/// Flat polygon (with holes) at a per-vertex height `y(x, z)`.
fn polygon(m: &mut Mesh, outer: &[[f64; 2]], holes: &[Ring], y: &dyn Fn(f64, f64) -> f64, col: [f32; 3]) {
    let mut flat: Vec<f64> = outer.iter().flat_map(|p| [p[0], p[1]]).collect();
    let mut hole_idx = vec![];
    for h in holes {
        hole_idx.push(flat.len() / 2);
        flat.extend(h.iter().flat_map(|p| [p[0], p[1]]));
    }
    let Ok(tris) = earcutr::earcut(&flat, &hole_idx, 2) else { return };
    let i0 = m.base();
    for p in flat.chunks(2) {
        m.v([p[0], y(p[0], p[1]), p[1]], UP, col);
    }
    for t in tris.chunks(3) {
        // keep triangles facing up (+y): in x/z with z south, counter-clockwise from above = negative cross
        let (a, b, c) = (t[0], t[1], t[2]);
        let (pa, pb, pc) = ([flat[a * 2], flat[a * 2 + 1]], [flat[b * 2], flat[b * 2 + 1]], [flat[c * 2], flat[c * 2 + 1]]);
        let cross = (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
        if cross < 0.0 {
            m.idx.extend([i0 + a as u32, i0 + b as u32, i0 + c as u32]);
        } else {
            m.idx.extend([i0 + a as u32, i0 + c as u32, i0 + b as u32]);
        }
    }
}

/// Ribbon along a polyline with mitred joins and square end caps, draped on terrain.
fn ribbon(m: &mut Mesh, pts: &[[f64; 2]], width: f64, lift: f64, ter: &Terrain, col: [f32; 3]) {
    let pts: Vec<[f64; 2]> = pts.windows(2).filter(|w| w[0] != w[1]).map(|w| w[0]).chain(pts.last().copied()).collect();
    if pts.len() < 2 {
        return;
    }
    let hw = width * 0.5;
    let dir = |a: [f64; 2], b: [f64; 2]| { let (dx, dz) = (b[0] - a[0], b[1] - a[1]); let l = (dx * dx + dz * dz).sqrt(); [dx / l, dz / l] };
    let n = pts.len();
    let mut left = vec![];
    let mut right = vec![];
    for i in 0..n {
        let d0 = if i > 0 { dir(pts[i - 1], pts[i]) } else { dir(pts[0], pts[1]) };
        let d1 = if i + 1 < n { dir(pts[i], pts[i + 1]) } else { d0 };
        let mut t = [d0[0] + d1[0], d0[1] + d1[1]];
        let tl = (t[0] * t[0] + t[1] * t[1]).sqrt();
        t = if tl < 1e-6 { d1 } else { [t[0] / tl, t[1] / tl] };
        let nrm = [-t[1], t[0]];
        let miter = (hw / (nrm[0] * -d1[1] + nrm[1] * d1[0]).abs().max(0.35)).min(hw * 2.5);
        let mut p = pts[i];
        if i == 0 {
            p = [p[0] - d1[0] * hw, p[1] - d1[1] * hw];
        }
        if i + 1 == n {
            p = [p[0] + d0[0] * hw, p[1] + d0[1] * hw];
        }
        let l = [p[0] + nrm[0] * miter, p[1] + nrm[1] * miter];
        let r = [p[0] - nrm[0] * miter, p[1] - nrm[1] * miter];
        left.push([l[0], ter.at(l[0], l[1]) + lift, l[1]]);
        right.push([r[0], ter.at(r[0], r[1]) + lift, r[1]]);
    }
    for i in 0..n - 1 {
        let i0 = m.base();
        for p in [left[i], right[i], right[i + 1], left[i + 1]] {
            m.v(p, UP, col);
        }
        // orient upward
        let a = left[i];
        let b = right[i];
        let c = right[i + 1];
        let cross = (b[0] - a[0]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[0] - a[0]);
        if cross < 0.0 {
            m.idx.extend([i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
        } else {
            m.idx.extend([i0, i0 + 2, i0 + 1, i0, i0 + 3, i0 + 2]);
        }
    }
}

fn walls(m: &mut Mesh, ring: &[[f64; 2]], y0: f64, y1: f64, outward_cw: bool, col: [f32; 3]) {
    let cw = signed_area(ring) > 0.0;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let l = (dx * dx + dz * dz).sqrt();
        if l < 1e-4 {
            continue;
        }
        // x east, z south: a clockwise ring (seen from above, north up) has its outside on the left of each edge
        let s = if cw == outward_cw { 1.0 } else { -1.0 };
        let n = [(dz / l * s) as f32, 0.0, (-dx / l * s) as f32];
        let (p0, p1, p2, p3) = ([a[0], y0, a[1]], [b[0], y0, b[1]], [b[0], y1, b[1]], [a[0], y1, a[1]]);
        let i0 = m.base();
        for p in [p0, p1, p2, p3] {
            m.v(p, n, col);
        }
        // front face must wind counter-clockwise seen from outside
        let e1 = [p1[0] - p0[0], 0.0, p1[2] - p0[2]];
        let e2 = [0.0, y1 - y0, 0.0];
        let fwd = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        if fwd[0] * n[0] as f64 + fwd[2] * n[2] as f64 > 0.0 {
            m.idx.extend([i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
        } else {
            m.idx.extend([i0, i0 + 2, i0 + 1, i0, i0 + 3, i0 + 2]);
        }
    }
}

const FACADES: [(u8, u8, u8); 8] = [
    (196, 186, 168), (170, 168, 162), (150, 92, 72), (214, 210, 200),
    (120, 140, 156), (182, 160, 130), (96, 104, 112), (204, 176, 150),
];

/// Building base/top heights used by both the mesh and collision.
pub fn building_span(b: &Building, ter: &Terrain) -> (f64, f64) {
    let g: Vec<f64> = b.outer.iter().map(|p| ter.at(p[0], p[1])).collect();
    let lo = g.iter().copied().fold(f64::INFINITY, f64::min);
    let avg = g.iter().sum::<f64>() / g.len() as f64;
    (lo - 0.5 + b.min_height, avg + b.height)
}

fn tree(m: &mut Mesh, x: f64, z: f64, y: f64, seed: u64) {
    let s = 0.8 + (seed % 100) as f64 / 250.0;
    let trunk = srgb(92, 74, 58);
    let leaf = [srgb(70, 104, 52), srgb(84, 118, 60), srgb(62, 96, 58)][(seed % 3) as usize];
    let r = 0.18 * s;
    for k in 0..4 {
        let a0 = k as f64 * std::f64::consts::FRAC_PI_2;
        let a1 = a0 + std::f64::consts::FRAC_PI_2;
        let (p0, p1) = ([x + r * a0.cos(), z + r * a0.sin()], [x + r * a1.cos(), z + r * a1.sin()]);
        let mid = (a0 + a1) * 0.5;
        m.quad([p0[0], y, p0[1]], [p0[0], y + 3.2 * s, p0[1]], [p1[0], y + 3.2 * s, p1[1]], [p1[0], y, p1[1]], [mid.cos() as f32, 0.0, mid.sin() as f32], trunk);
    }
    // crown: an octahedron with its faces split once
    let (cy, rr, rh) = (y + 4.6 * s, 2.6 * s, 2.2 * s);
    let v = [[0.0, rh, 0.0], [rr, 0.0, 0.0], [0.0, 0.0, rr], [-rr, 0.0, 0.0], [0.0, 0.0, -rr], [0.0, -rh, 0.0]];
    let faces = [[0, 2, 1], [0, 3, 2], [0, 4, 3], [0, 1, 4], [5, 1, 2], [5, 2, 3], [5, 3, 4], [5, 4, 1]];
    for f in faces {
        let (a, b, c) = (v[f[0]], v[f[1]], v[f[2]]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let n = [(n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32];
        let i0 = m.base();
        for p in [a, b, c] {
            m.v([x + p[0], cy + p[1], z + p[2]], n, leaf);
        }
        m.idx.extend([i0, i0 + 1, i0 + 2]);
    }
}

/// Everything static in the city as one triangle mesh.
pub fn build(c: &City) -> Mesh {
    let mut m = Mesh::default();
    let ter = &c.terrain;

    // ground: terrain grid, sidewalk concrete
    let r = c.import_rect();
    let step = 4.0;
    let (gw, gh) = (((r[2] - r[0]) / step).ceil() as usize, ((r[3] - r[1]) / step).ceil() as usize);
    let ground = srgb(178, 172, 162);
    let i0 = m.base();
    for j in 0..=gh {
        for i in 0..=gw {
            let (x, z) = (r[0] + i as f64 * (r[2] - r[0]) / gw as f64, r[1] + j as f64 * (r[3] - r[1]) / gh as f64);
            let (hx, hz) = (ter.at(x + 1.0, z) - ter.at(x - 1.0, z), ter.at(x, z + 1.0) - ter.at(x, z - 1.0));
            let n = [-hx as f32 * 0.5, 1.0, -hz as f32 * 0.5];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            m.v([x, ter.at(x, z), z], [n[0] / l, n[1] / l, n[2] / l], ground);
        }
    }
    let row = gw as u32 + 1;
    for j in 0..gh as u32 {
        for i in 0..gw as u32 {
            let a = i0 + j * row + i;
            m.idx.extend([a, a + row, a + 1, a + 1, a + row, a + row + 1]);
        }
    }

    for a in &c.areas {
        let (col, lift) = match a.kind {
            AreaKind::Grass => (srgb(96, 132, 70), 0.04),
            AreaKind::Pitch => (srgb(70, 128, 60), 0.05),
            AreaKind::Parking => (srgb(88, 88, 90), 0.03),
            AreaKind::Plaza => (srgb(196, 188, 176), 0.02),
        };
        polygon(&mut m, &a.outer, &[], &|x, z| ter.at(x, z) + lift, col);
    }

    let mut lines: Vec<&Line> = c.lines.iter().collect();
    let order = |k: LineKind| match k { LineKind::Path => 0, LineKind::Service => 1, LineKind::Rail => 2, LineKind::Road => 3 };
    lines.sort_by_key(|l| order(l.kind));
    for l in lines {
        let (col, lift) = match l.kind {
            LineKind::Road => (srgb(62, 62, 66), 0.10),
            LineKind::Service => (srgb(84, 84, 86), 0.08),
            LineKind::Path => (srgb(184, 178, 168), 0.06),
            LineKind::Rail => (srgb(92, 80, 70), 0.12),
        };
        ribbon(&mut m, &l.pts, l.width, lift, ter, col);
    }

    for b in &c.buildings {
        let (y0, y1) = building_span(b, ter);
        let h = hash(&b.osm);
        let f = FACADES[(h % FACADES.len() as u64) as usize];
        let wall = srgb(f.0, f.1, f.2);
        let g = 150 + (h >> 8) as u8 % 60;
        let roof = srgb(g, g, g.saturating_sub(4));
        walls(&mut m, &b.outer, y0, y1, true, wall);
        for hole in &b.holes {
            walls(&mut m, hole, y0, y1, false, wall);
        }
        polygon(&mut m, &b.outer, &b.holes, &|_, _| y1, roof);
    }

    for (i, t) in c.trees.iter().enumerate() {
        tree(&mut m, t[0], t[1], ter.at(t[0], t[1]), hash(&i.to_string()));
    }
    m
}
