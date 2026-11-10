use city::City;
use render::{Gpu, Scene, Vertex};

/// The static city mesh, built once; scene objects are appended per upload.
pub struct Base {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
}

impl Base {
    pub fn new(c: &City) -> Self {
        let m = city::mesh::build(c);
        Self { verts: (0..m.pos.len()).map(|i| Vertex { pos: m.pos[i], nrm: m.nrm[i], col: m.col[i] }).collect(), idx: m.idx }
    }
}

fn object_color(k: editor::Kind) -> [f32; 3] {
    let c = editor::draw::kind_color(k);
    let f = |v: u8| (v as f32 / 255.0).powf(2.2) * 0.8;
    [f(c[0]), f(c[1]), f(c[2])]
}

/// Uploads the city plus the scene's objects as boxes standing on the terrain.
pub fn scene(gpu: &Gpu, base: &Base, c: &City, s: Option<&editor::Scene>) -> Scene {
    let mut verts = base.verts.clone();
    let mut idx = base.idx.clone();
    for o in s.map_or(&[][..], |s| &s.objects[..]) {
        let y0 = o.corners().iter().map(|p| c.terrain.at(p[0], p[1])).fold(f64::INFINITY, f64::min) as f32 - 0.05;
        let y1 = c.terrain.at(o.x, o.z) as f32 + o.size[1] as f32;
        let corners = o.corners().map(|p| [p[0] as f32, p[1] as f32]);
        let col = object_color(o.kind);
        let mut quad = |p: [[f32; 3]; 4], n: [f32; 3]| {
            let i0 = verts.len() as u32;
            for q in p {
                verts.push(Vertex { pos: q, nrm: n, col });
            }
            idx.extend([i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
        };
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dz).max(1e-6);
            let ctr = [o.x as f32, o.z as f32];
            let mid = [(a[0] + b[0]) * 0.5 - ctr[0], (a[1] + b[1]) * 0.5 - ctr[1]];
            let mut n = [dz / l, 0.0, -dx / l];
            if n[0] * mid[0] + n[2] * mid[1] < 0.0 {
                n = [-n[0], 0.0, -n[2]];
            }
            quad([[a[0], y0, a[1]], [b[0], y0, b[1]], [b[0], y1, b[1]], [a[0], y1, a[1]]], n);
        }
        quad(corners.map(|p| [p[0], y1, p[1]]), [0.0, 1.0, 0.0]);
    }
    Scene::new(gpu, &verts, &idx)
}

/// Building footprints as vertical prisms, for player collision.
pub struct Solid {
    pub ring: Vec<[f32; 2]>,
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub y0: f32,
    pub y1: f32,
}

pub struct Collision {
    pub solids: Vec<Solid>,
}

impl Collision {
    pub fn new(c: &City, s: Option<&editor::Scene>) -> Self {
        let mut solids = vec![];
        for o in s.map_or(&[][..], |s| &s.objects[..]) {
            let ring: Vec<[f32; 2]> = o.corners().iter().map(|p| [p[0] as f32, p[1] as f32]).collect();
            let min = ring.iter().fold([f32::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
            let max = ring.iter().fold([f32::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
            let g = c.terrain.at(o.x, o.z) as f32;
            solids.push(Solid { ring, min, max, y0: g - 0.05, y1: g + o.size[1] as f32 });
        }
        for b in &c.buildings {
            let (y0, y1) = city::mesh::building_span(b, &c.terrain);
            for ring in std::iter::once(&b.outer).chain(&b.holes) {
                let ring: Vec<[f32; 2]> = ring.iter().map(|p| [p[0] as f32, p[1] as f32]).collect();
                let min = ring.iter().fold([f32::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
                let max = ring.iter().fold([f32::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
                solids.push(Solid { ring, min, max, y0: y0 as f32, y1: y1 as f32 });
            }
        }
        Self { solids }
    }

    /// Pushes a vertical capsule (radius `r`, feet `y`, height `h`) out of building walls, in x/z.
    pub fn push_out(&self, p: &mut [f32; 2], y: f32, h: f32, r: f32) {
        for _ in 0..3 {
            for s in &self.solids {
                if p[0] < s.min[0] - r || p[0] > s.max[0] + r || p[1] < s.min[1] - r || p[1] > s.max[1] + r || y > s.y1 || y + h < s.y0 {
                    continue;
                }
                let inside = point_in(&s.ring, *p);
                let (mut best, mut bd) = ([0.0f32; 2], f32::MAX);
                for i in 0..s.ring.len() {
                    let (a, b) = (s.ring[i], s.ring[(i + 1) % s.ring.len()]);
                    let q = closest_on_seg(*p, a, b);
                    let d = (q[0] - p[0]).hypot(q[1] - p[1]);
                    if d < bd {
                        bd = d;
                        best = q;
                    }
                }
                if inside {
                    let (dx, dz) = (best[0] - p[0], best[1] - p[1]);
                    let l = bd.max(1e-4);
                    *p = [best[0] + dx / l * r, best[1] + dz / l * r];
                } else if bd < r {
                    let (dx, dz) = (p[0] - best[0], p[1] - best[1]);
                    let l = bd.max(1e-4);
                    *p = [best[0] + dx / l * r, best[1] + dz / l * r];
                }
            }
        }
    }

    /// Highest roof under (x, z) that is at or below `y + step`, if any.
    pub fn roof_under(&self, p: [f32; 2], y: f32, step: f32) -> Option<f32> {
        self.solids.iter().filter(|s| s.y1 <= y + step && p[0] >= s.min[0] && p[0] <= s.max[0] && p[1] >= s.min[1] && p[1] <= s.max[1] && point_in(&s.ring, p)).map(|s| s.y1).reduce(f32::max)
    }
}

fn point_in(r: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut c = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

fn closest_on_seg(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 < 1e-8 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / l2).clamp(0.0, 1.0) };
    [a[0] + dx * t, a[1] + dz * t]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn city() -> City {
        City::load(&std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/city/downtown_sd.json")).expect("run `fps import` first")
    }

    fn inside_any(col: &Collision, p: [f32; 2], y: f32) -> bool {
        col.solids.iter().any(|s| y < s.y1 && y + 1.8 > s.y0 && point_in(&s.ring, p))
    }

    #[test]
    fn walking_up_5th_ave_never_enters_a_building() {
        let c = city();
        let col = Collision::new(&c, None);
        // 5th Ave centreline from L St north to Market St
        let mut p = [-291.5f32, -60.0];
        for i in 0..2100 {
            let y = c.terrain.at(p[0] as f64, p[1] as f64) as f32;
            let dir = [0.0, -1.0];
            p = [p[0] + dir[0] * 0.2, p[1] + dir[1] * 0.2];
            col.push_out(&mut p, y + 0.45, 1.35, 0.35);
            assert!(!inside_any(&col, p, y + 0.45), "inside a building at {p:?} (step {i})");
        }
        assert!(p[1] < -460.0, "walk north stalled at {p:?}");
    }

    #[test]
    fn walking_east_mid_block_stops_at_the_facade() {
        let c = city();
        let col = Collision::new(&c, None);
        // 5th Ave between Island Ave and Market St, walking east into the block
        let mut p = [-291.5f32, -430.0];
        for i in 0..400 {
            let y = c.terrain.at(p[0] as f64, p[1] as f64) as f32;
            p[0] += 0.2;
            col.push_out(&mut p, y + 0.45, 1.35, 0.35);
            assert!(!inside_any(&col, p, y + 0.45), "inside a building at {p:?} (step {i})");
        }
        assert!(p[0] < -270.0, "walked through the east-side buildings: {p:?}");
    }

    #[test]
    fn push_out_resolves_a_point_inside_a_footprint() {
        let c = city();
        let col = Collision::new(&c, None);
        let s = col.solids.iter().find(|s| s.max[0] - s.min[0] > 20.0 && s.max[1] - s.min[1] > 20.0 && point_in(&s.ring, [(s.min[0] + s.max[0]) * 0.5, (s.min[1] + s.max[1]) * 0.5])).unwrap();
        let mut p = [(s.min[0] + s.max[0]) * 0.5, (s.min[1] + s.max[1]) * 0.5];
        col.push_out(&mut p, s.y0 + 0.5, 1.3, 0.35);
        assert!(!point_in(&s.ring, p));
    }
}

#[cfg(test)]
mod arena_tests {
    /// The committed demo arena must stay valid as the city data and validators change.
    #[test]
    fn demo_arena_has_no_issues() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let c = city::City::load(&root.join("data/city/downtown_sd.json")).unwrap();
        let w = editor::World::new(c);
        let s = editor::Store::new(&root.join("data/scenes"), "arena").load(&w).unwrap();
        let r = editor::analyze(&w, &s).report;
        assert!(r.issues.is_empty(), "{:?}", r.issues);
        assert!(r.reachable_pct > 99.0 && r.sight_over_150m_pct < 1.0 && r.cover_pct >= 50.0);
    }
}
