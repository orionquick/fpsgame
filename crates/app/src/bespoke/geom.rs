//! Mesh kit for hand-authored buildings: walls with real openings, boxes, sloped parapet pieces.
//! Everything is built in a building-local frame (x, y up, z) and transformed to the world on output.
//! UVs are planar projections in metres, so neighbouring pieces on one plane line up seamlessly.
use glam::{Vec2, Vec3};
use render::TexVertex;
use std::collections::HashMap;

/// What a face is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mat {
    /// A Megascans surface by catalog id.
    Surface(&'static str),
    /// Untextured, shows the tint; roughness 0..255.
    Plain(u8),
    /// Clear glazing (transparent pass).
    Glass,
    /// Emissive panel (ceiling lights).
    Light,
}

/// Material plus tint and lighting context: one draw batch per distinct key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub mat: Mat,
    tint: [u16; 3],
    /// Indoor surfaces get dimmer sky light and lamp light.
    pub indoor: bool,
    /// Texture tile size override in centimetres (0 = the surface's own size).
    pub tile_cm: u16,
    /// Roughness scale in hundredths (0 = as textured): lower = glossier (oiled timber, polished tile).
    pub gloss: u16,
    /// Edge wear (see Wear).
    pub wear: u8,
}

/// Edge wear kinds (the shader's weathered-edge effect).
pub mod wear {
    pub const NONE: u8 = 0;
    /// Timber: corners rubbed lighter and raw.
    pub const WOOD: u8 = 1;
    /// Painted or blackened steel: bare metal on the edges.
    pub const METAL: u8 = 2;
    /// Brick, stone, concrete: chipped arrises.
    pub const STONE: u8 = 3;
}

impl Key {
    pub fn new(mat: Mat, tint: [f32; 3]) -> Self {
        Self { mat, tint: tint.map(|t| (t * 1000.0).round() as u16), indoor: false, tile_cm: 0, gloss: 0, wear: 0 }
    }
    pub fn inside(mut self) -> Self {
        self.indoor = true;
        self
    }
    /// Scale the texture's roughness (0.5 = twice as glossy).
    pub fn rough(mut self, k: f32) -> Self {
        self.gloss = (k * 100.0).round() as u16;
        self
    }
    pub fn wear(mut self, kind: u8) -> Self {
        self.wear = kind;
        self
    }
    /// Stretch the texture so one tile covers `m` metres.
    pub fn tile(mut self, m: f32) -> Self {
        self.tile_cm = (m * 100.0).round() as u16;
        self
    }
    pub fn tint(&self) -> [f32; 3] {
        self.tint.map(|t| t as f32 / 1000.0)
    }
}

/// A vertical prism for player collision, in world coordinates.
pub struct Block {
    pub ring: Vec<[f32; 2]>,
    pub y0: f32,
    pub y1: f32,
}

/// Rectangle in a wall's own coordinates: u along the wall, y height (local).
#[derive(Clone, Copy, Debug)]
pub struct Hole {
    pub u0: f32,
    pub u1: f32,
    pub y0: f32,
    pub y1: f32,
}

pub struct Geo {
    origin: Vec3,
    ex: Vec3,
    ez: Vec3,
    pub groups: HashMap<Key, (Vec<TexVertex>, Vec<u32>)>,
    pub blocks: Vec<Block>,
    /// Lamps that light the interior (world space).
    pub lights: Vec<render::PointLight>,
    /// Reflection probes: capture point, box min, box max (world).
    pub probes: Vec<[Vec3; 3]>,
}

impl Geo {
    /// A reflection probe for the local box `a`..`b`, captured at `p`.
    pub fn probe(&mut self, p: Vec3, a: Vec3, b: Vec3) {
        let corners = [Vec3::new(a.x, a.y, a.z), Vec3::new(b.x, a.y, a.z), Vec3::new(a.x, a.y, b.z), Vec3::new(b.x, a.y, b.z), Vec3::new(a.x, b.y, a.z), Vec3::new(b.x, b.y, b.z)];
        let w: Vec<Vec3> = corners.iter().map(|&c| self.world(c)).collect();
        let mn = w.iter().fold(Vec3::splat(f32::MAX), |m, v| m.min(*v));
        let mx = w.iter().fold(Vec3::splat(f32::MIN), |m, v| m.max(*v));
        self.probes.push([self.world(p), mn, mx]);
    }

    /// A lamp at local `p` with range `radius` and linear colour (intensity folded in).
    pub fn light(&mut self, p: Vec3, radius: f32, color: [f32; 3]) {
        let w = self.world(p);
        self.lights.push(render::PointLight { pos: w.to_array(), radius, color, _pad: 0.0 });
    }

    /// `origin` is the world position of local (0, 0, 0); `ex` the world direction of local +x (horizontal).
    pub fn new(origin: Vec3, ex: Vec3) -> Self {
        let ex = Vec3::new(ex.x, 0.0, ex.z).normalize();
        // local +z is +x turned 90 degrees, matching world x-east/z-south handedness
        let ez = Vec3::new(-ex.z, 0.0, ex.x);
        Self { origin, ex, ez, groups: HashMap::new(), blocks: vec![], lights: vec![], probes: vec![] }
    }

    /// World (x, z) of a local plan point.
    pub fn world_xz(&self, p: Vec2) -> [f64; 2] {
        let w = self.world(Vec3::new(p.x, 0.0, p.y));
        [w.x as f64, w.z as f64]
    }
    /// World height of a local height.
    pub fn world_y(&self, y: f32) -> f64 {
        (self.origin.y + y) as f64
    }
    /// Heading (degrees clockwise from north) of a local plan direction.
    pub fn heading(&self, d: Vec2) -> f64 {
        let w = self.wdir(Vec3::new(d.x, 0.0, d.y));
        (w.x as f64).atan2(-w.z as f64).to_degrees()
    }

    fn world(&self, p: Vec3) -> Vec3 {
        self.origin + self.ex * p.x + Vec3::Y * p.y + self.ez * p.z
    }
    fn wdir(&self, d: Vec3) -> Vec3 {
        self.ex * d.x + Vec3::Y * d.y + self.ez * d.z
    }

    /// Planar polygon (convex, 3-4 points, counter-clockwise seen from the front) with right/up axes for UVs.
    /// The normal is right x up; UVs are (p.right, -p.up) in metres.
    pub fn poly(&mut self, k: Key, pts: &[Vec3], right: Vec3, up: Vec3) {
        let n = right.cross(up).normalize();
        let (wn, wt) = (self.wdir(n), self.wdir(right));
        // face-local coordinates for edge wear
        let (mut u0, mut v0, mut u1, mut v1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in pts {
            (u0, v0, u1, v1) = (u0.min(p.dot(right)), v0.min(p.dot(up)), u1.max(p.dot(right)), v1.max(p.dot(up)));
        }
        let verts: Vec<TexVertex> = pts
            .iter()
            .map(|&p| {
                let w = self.world(p);
                let edge = [p.dot(right) - u0, p.dot(up) - v0, u1 - u0, v1 - v0];
                TexVertex { pos: w.to_array(), nrm: wn.to_array(), uv: [p.dot(right), -p.dot(up)], tan: [wt.x, wt.y, wt.z, 1.0], edge }
            })
            .collect();
        let g = self.groups.entry(k).or_default();
        let base = g.0.len() as u32;
        g.0.extend(verts);
        for i in 1..pts.len() as u32 - 1 {
            g.1.extend([base, base + i, base + i + 1]);
        }
    }

    /// A quad whose texture coordinates are shifted by `uv` (metres) and turned a quarter `turns` times, so
    /// neighbouring slabs of one material show different parts of the texture.
    pub fn quad_uv(&mut self, k: Key, o: Vec3, r: Vec3, u: Vec3, uv: [f32; 2], turns: u32) {
        let n = r.cross(u).normalize();
        let (rn, un) = (r.normalize(), u.normalize());
        let (tr, tu) = match turns % 4 {
            0 => (rn, un),
            1 => (un, -rn),
            2 => (-rn, -un),
            _ => (-un, rn),
        };
        let (wn, wt) = (self.wdir(n), self.wdir(tr));
        let (lr, lu) = (r.length(), u.length());
        let verts: Vec<TexVertex> = [(o, 0.0, 0.0), (o + r, lr, 0.0), (o + r + u, lr, lu), (o + u, 0.0, lu)]
            .iter()
            .map(|&(p, eu, ev)| TexVertex { pos: self.world(p).to_array(), nrm: wn.to_array(), uv: [p.dot(tr) + uv[0], -p.dot(tu) + uv[1]], tan: [wt.x, wt.y, wt.z, 1.0], edge: [eu, ev, lr, lu] })
            .collect();
        let g = self.groups.entry(k).or_default();
        let base = g.0.len() as u32;
        g.0.extend(verts);
        g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Rectangle from corner `o` spanning `r` (rightwards) and `u` (upwards); faces r x u.
    pub fn quad(&mut self, k: Key, o: Vec3, r: Vec3, u: Vec3) {
        if r.length_squared() < 1e-8 || u.length_squared() < 1e-8 {
            return;
        }
        self.poly(k, &[o, o + r, o + r + u, o + u], r.normalize(), u.normalize());
    }

    /// Axis-aligned box in local coordinates, all six faces outward.
    pub fn cuboid(&mut self, k: Key, a: Vec3, b: Vec3) {
        let (mn, mx) = (a.min(b), a.max(b));
        let s = mx - mn;
        if s.min_element() <= 1e-5 {
            return;
        }
        let (x, y, z) = (Vec3::X * s.x, Vec3::Y * s.y, Vec3::Z * s.z);
        self.quad(k, mn + z, x, y); // +z face
        self.quad(k, mn + x, -x, y); // -z face
        self.quad(k, mn + x + z, -z, y); // +x face
        self.quad(k, mn, z, y); // -x face
        self.quad(k, mn + y + z, x, -z); // top
        self.quad(k, mn, x, z); // bottom
    }

    /// Axis-aligned timber: like `cuboid`, but every face's texture V axis runs along the member's length,
    /// so wood grain follows the beam (texture grain is along V in the Megascans wood sets).
    pub fn timber(&mut self, k: Key, a: Vec3, b: Vec3) {
        let (mn, mx) = (a.min(b), a.max(b));
        let s = mx - mn;
        if s.min_element() <= 1e-5 {
            return;
        }
        let long = if s.x >= s.y && s.x >= s.z { Vec3::X } else if s.y >= s.z { Vec3::Y } else { Vec3::Z };
        let c = (mn + mx) * 0.5;
        for n in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
            // up = the long axis when it lies in this face, else any in-plane axis
            let up = if n.dot(long).abs() < 0.5 { long } else if n.y.abs() < 0.5 { Vec3::Y } else { Vec3::Z };
            let right = up.cross(n);
            let (hr, hu) = ((right.abs() * s).max_element() * 0.5, (up.abs() * s).max_element() * 0.5);
            let fc = c + n * ((n.abs() * s).max_element() * 0.5);
            let o = fc - right * hr - up * hu;
            let (r, u) = (right * 2.0 * hr, up * 2.0 * hu);
            self.poly(k, &[o, o + r, o + r + u, o + u], right, up);
        }
    }

    /// Box oriented along a direction in the ground plane: `a`/`b` are the ends of its centreline at height `y0`.
    pub fn beam(&mut self, k: Key, a: Vec2, b: Vec2, half_w: f32, y0: f32, y1: f32) {
        let d = b - a;
        let l = d.length();
        if l < 1e-5 || y1 <= y0 {
            return;
        }
        let r = Vec3::new(d.x, 0.0, d.y) / l;
        let n = r.cross(Vec3::Y);
        let c0 = Vec3::new(a.x, y0, a.y) - n * half_w;
        let (len, w, h) = (r * l, n * 2.0 * half_w, Vec3::Y * (y1 - y0));
        self.quad(k, c0 + len, -len, h); // -n side
        self.quad(k, c0 + w, len, h); // +n side
        self.quad(k, c0 + len + w, -w, h); // end at b
        self.quad(k, c0, w, h); // end at a
        self.quad(k, c0 + h + w, len, -w); // top
        self.quad(k, c0, len, w); // bottom
    }

    /// Thick wall along a->b (outer face on the line, normal = right of travel x up), `t` deep towards the left,
    /// from `y0` to `y1`, with rectangular openings (u along the wall from a). Openings get reveals; ends are capped.
    pub fn wall(&mut self, out: Key, inn: Key, a: Vec2, b: Vec2, y0: f32, y1: f32, t: f32, holes: &[Hole], caps: bool) {
        let d = b - a;
        let l = d.length();
        if l < 1e-4 || y1 <= y0 {
            return;
        }
        let r = Vec3::new(d.x, 0.0, d.y) / l;
        let n_out = r.cross(Vec3::Y);
        let inward = -n_out;
        let p0 = Vec3::new(a.x, 0.0, a.y);
        let holes: Vec<Hole> = holes.iter().map(|h| Hole { u0: h.u0.max(0.0), u1: h.u1.min(l), y0: h.y0.max(y0), y1: h.y1.min(y1) }).filter(|h| h.u1 > h.u0 && h.y1 > h.y0).collect();
        let mut us = vec![0.0, l];
        let mut ys = vec![y0, y1];
        for h in &holes {
            us.extend([h.u0, h.u1]);
            ys.extend([h.y0, h.y1]);
        }
        let sort = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
        };
        sort(&mut us);
        sort(&mut ys);
        let solid = |u: f32, y: f32| !holes.iter().any(|h| u > h.u0 && u < h.u1 && y > h.y0 && y < h.y1);
        for i in 0..us.len() - 1 {
            let mut run: Option<f32> = None;
            for j in 0..ys.len() - 1 {
                let (u0, u1, ya, yb) = (us[i], us[i + 1], ys[j], ys[j + 1]);
                let s = solid((u0 + u1) * 0.5, (ya + yb) * 0.5);
                if s {
                    let o = p0 + r * u0 + Vec3::Y * ya;
                    self.quad(out, o, r * (u1 - u0), Vec3::Y * (yb - ya));
                    self.quad(inn, o + inward * t + r * (u1 - u0), -r * (u1 - u0), Vec3::Y * (yb - ya));
                    run.get_or_insert(ya);
                }
                if !s || j == ys.len() - 2 {
                    if let Some(ys0) = run.take() {
                        let ye = if s { yb } else { ya };
                        self.block_box(p0 + r * u0, r * (u1 - u0), inward * t, ys0, ye);
                    }
                }
            }
        }
        for h in &holes {
            let o = p0 + r * h.u0;
            let (w, hh) = (r * (h.u1 - h.u0), Vec3::Y * (h.y1 - h.y0));
            let dep = inward * t;
            // jambs face into the opening, head faces down, sill faces up
            self.quad(out, o + Vec3::Y * h.y0, dep, hh); // left jamb, normal = inward x up = +r
            self.quad(out, o + w + dep + Vec3::Y * h.y0, -dep, hh); // right jamb, normal -r
            self.quad(out, o + Vec3::Y * h.y1 + dep, w, -dep); // head: r x (-dep) -> down
            self.quad(out, o + Vec3::Y * h.y0, w, dep); // sill: r x dep -> up
        }
        if caps {
            let h = Vec3::Y * (y1 - y0);
            self.quad(out, p0 + inward * t + Vec3::Y * y0, -inward * t, h); // end at a
            self.quad(out, p0 + r * l + Vec3::Y * y0, inward * t, h); // end at b
            self.quad(out, p0 + Vec3::Y * y1, r * l, inward * t); // top
        }
    }

    /// Wall piece with a sloped top: heights `ya`/`yb` at its ends (parapets, gables). No openings.
    pub fn wall_sloped(&mut self, out: Key, inn: Key, a: Vec2, b: Vec2, y0: f32, ya: f32, yb: f32, t: f32) {
        let d = b - a;
        let l = d.length();
        if l < 1e-4 {
            return;
        }
        let r = Vec3::new(d.x, 0.0, d.y) / l;
        let inward = -r.cross(Vec3::Y);
        let (pa, pb) = (Vec3::new(a.x, 0.0, a.y), Vec3::new(b.x, 0.0, b.y));
        let y = |p: Vec3, h: f32| p + Vec3::Y * h;
        self.poly(out, &[y(pa, y0), y(pb, y0), y(pb, yb), y(pa, ya)], r, Vec3::Y);
        let (ia, ib) = (pa + inward * t, pb + inward * t);
        self.poly(inn, &[y(ib, y0), y(ia, y0), y(ia, ya), y(ib, yb)], -r, Vec3::Y);
        let hl = (yb - ya) / l;
        let slope = (r + Vec3::Y * hl).normalize();
        self.poly(out, &[y(pa, ya), y(pb, yb), y(ib, yb), y(ia, ya)], slope, inward);
        self.block_box(pa, r * l, inward * t, y0, ya.max(yb));
    }

    /// Round tube from `a` to `b` (local), smooth-shaded: ducts, pipes, cables.
    pub fn tube(&mut self, k: Key, a: Vec3, b: Vec3, r: f32, seg: usize) {
        let d = b - a;
        let l = d.length();
        if l < 1e-4 {
            return;
        }
        let ax = d / l;
        let up = if ax.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let u = ax.cross(up).normalize();
        let v = ax.cross(u);
        let circ = std::f32::consts::TAU * r;
        let wt = self.wdir(ax);
        let mut verts = vec![];
        for i in 0..=seg {
            let t = i as f32 / seg as f32 * std::f32::consts::TAU;
            let n = u * t.cos() + v * t.sin();
            let wn = self.wdir(n);
            for (end, along) in [(a, 0.0), (b, l)] {
                let w = self.world(end + n * r);
                verts.push(TexVertex { pos: w.to_array(), nrm: wn.to_array(), uv: [along, -(i as f32 / seg as f32) * circ], tan: [wt.x, wt.y, wt.z, 1.0], edge: [0.0; 4] });
            }
        }
        let g = self.groups.entry(k).or_default();
        let base = g.0.len() as u32;
        g.0.extend(verts);
        for i in 0..seg as u32 {
            let (p0, p1, q0, q1) = (base + 2 * i, base + 2 * i + 1, base + 2 * i + 2, base + 2 * i + 3);
            g.1.extend([p0, q0, p1, p1, q0, q1]);
        }
    }

    /// Collision prism for a box given by a corner, its length and depth vectors (local), and a height span.
    pub fn block_box(&mut self, o: Vec3, len: Vec3, dep: Vec3, y0: f32, y1: f32) {
        if y1 - y0 < 0.02 {
            return;
        }
        let ring = [o, o + len, o + len + dep, o + dep].map(|p| {
            let w = self.world(Vec3::new(p.x, 0.0, p.z));
            [w.x, w.z]
        });
        self.blocks.push(Block { ring: ring.to_vec(), y0: self.origin.y + y0, y1: self.origin.y + y1 });
    }

    /// Collision for an axis-aligned local box.
    pub fn block_aabb(&mut self, a: Vec3, b: Vec3) {
        let (mn, mx) = (a.min(b), a.max(b));
        self.block_box(Vec3::new(mn.x, 0.0, mn.z), Vec3::X * (mx.x - mn.x), Vec3::Z * (mx.z - mn.z), mn.y, mx.y);
    }

    /// Solid box: drawn and collidable.
    pub fn solid(&mut self, k: Key, a: Vec3, b: Vec3) {
        self.cuboid(k, a, b);
        self.block_aabb(a, b);
    }

    pub fn triangles(&self) -> usize {
        self.groups.values().map(|g| g.1.len() / 3).sum()
    }
}
