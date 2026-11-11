use city::City;
use render::{Gpu, Scene, Vertex};

/// The static city, built once: flat-shaded geometry (grass, pitch, trees) and textured surfaces per material slot.
pub struct Base {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
    pub slots: Vec<(city::mesh::Slot, Vec<render::TexVertex>, Vec<u32>)>,
}

impl Base {
    /// `prefabs`: Megascans trees and prefab-built buildings replace the placeholder trees and the extruded
    /// building volumes, so leave those out (the volumes stay as collision only).
    pub fn new(c: &City, prefabs: bool) -> Self {
        let (mut verts, mut idx, mut slots) = (vec![], vec![], vec![]);
        for (slot, m) in city::mesh::build_slots_ex(c, &crate::bespoke::ground_holes()) {
            if prefabs && matches!(slot, city::mesh::Slot::Tree | city::mesh::Slot::Wall(_) | city::mesh::Slot::Roof) {
                continue;
            }
            if crate::library::slot_surface(slot).is_some() {
                let tv = (0..m.pos.len()).map(|i| render::TexVertex { pos: m.pos[i], nrm: m.nrm[i], uv: m.uv[i], tan: m.tan[i], edge: [0.0; 4] }).collect();
                slots.push((slot, tv, m.idx));
            } else {
                let b = verts.len() as u32;
                verts.extend((0..m.pos.len()).map(|i| Vertex { pos: m.pos[i], nrm: m.nrm[i], col: m.col[i] }));
                idx.extend(m.idx.iter().map(|i| i + b));
            }
        }
        Self { verts, idx, slots }
    }
}

/// One textured batch per city material slot (Megascans surfaces at their real-world tile size).
pub fn city_batches(gpu: &Gpu, r: &render::Renderer, lib: &mut crate::library::Library, base: &Base) -> Vec<render::Batch> {
    let pick = |lib: &crate::library::Library, title: &str| -> Option<(String, f32)> {
        let v: Vec<&assets::Asset> = lib.catalog.assets.iter().filter(|a| a.title == title && a.kind == assets::AssetKind::Surface).collect();
        // prefer square tiles of at least 1.5 m (big variants repeat less), tagged as the thing itself when possible
        let square = |a: &&&assets::Asset| a.tile_m.is_some_and(|t| (t[0] - t[1]).abs() < 1e-3 && t[0] >= 1.5);
        let a = v.iter().find(|a| square(a) && a.tags.iter().any(|t| t == "lawn" || t == "garden")).or_else(|| v.iter().find(square)).or_else(|| v.iter().find(|a| a.tile_m.is_some_and(|t| (t[0] - t[1]).abs() < 1e-3))).or(v.first())?;
        Some((a.id.clone(), a.tile_m.map_or(2.0, |t| t[0])))
    };
    let chosen: Vec<_> = base.slots.iter().map(|(slot, _, _)| crate::library::slot_surface(*slot).and_then(|(t, tint)| pick(lib, t).map(|p| (p, tint)))).collect();
    let ids: Vec<String> = chosen.iter().flatten().map(|((id, _), _)| id.clone()).collect();
    lib.preload(gpu, r, &ids);
    let mut out = vec![];
    for ((_, v, i), ch) in base.slots.iter().zip(chosen) {
        let (mat, tile, tint) = match ch {
            Some(((id, tile), tint)) => (lib.material(gpu, r, &id), tile, tint),
            None => (lib.plain(gpu, r), 1.0, [0.7, 0.7, 0.7]),
        };
        out.extend(render::Batch::new(gpu, v, i, &[render::Instance::new(glam::Mat4::IDENTITY, tint, 1.0 / tile)], mat));
    }
    out
}

/// Megascans prefabs standing in for an editor object kind (variants picked per object), if there are any.
pub fn kind_prefab(k: editor::Kind) -> Option<&'static [&'static str]> {
    const CONTAINERS: &[&str] = &["Industrial Construction Container Shipping Metal Blue 01", "Industrial Construction Container Shipping Metal Red 01"];
    Some(match k {
        editor::Kind::Barrier => &["Concrete Barrier"],
        editor::Kind::Crate => &["Wooden Crate"],
        editor::Kind::Car => &["Urban Australian Desolation Vehicle Metal Rusty", "Industrial Junkyard Vehicle Chassis Burned 01", "Industrial Junkyard Vehicle Chassis Rusty 01"],
        editor::Kind::Container | editor::Kind::Bus | editor::Kind::Dumpster => CONTAINERS,
        _ => return None,
    })
}

/// Prefab placements for scene objects whose kind has a Megascans model, scaled to the object's size.
/// Models built lengthwise along Z are turned so their length follows the object's heading.
pub fn object_placements(lib: &crate::library::Library, c: &City, s: &editor::Scene) -> Vec<crate::library::Placement> {
    s.objects
        .iter()
        .filter_map(|o| {
            let titles = kind_prefab(o.kind)?;
            let title = titles[(o.id as usize * 2654435761usize >> 7) % titles.len()];
            let a = lib.catalog.assets.iter().find(|a| a.title == title && !a.parts.is_empty())?;
            let e = crate::library::extent(a);
            let along_z = e[2] > e[0];
            let (ex, ez) = if along_z { (e[2], e[0]) } else { (e[0], e[2]) };
            let sc = |want: f64, have: f32| ((want / have.max(0.01) as f64) as f32).clamp(0.4, 2.6);
            let (sx, sy, sz) = (sc(o.size[0], ex), sc(o.size[1], e[1]), sc(o.size[2], ez));
            let scale = if along_z { [sz, sy, sx] } else { [sx, sy, sz] };
            Some(crate::library::Placement { asset: a.id.clone(), part: crate::library::ALL_PARTS, x: o.x, z: o.z, y: c.terrain.at(o.x, o.z), rot: o.rot + if along_z { 90.0 } else { 0.0 }, scale, tint: [1.0, 1.0, 1.0], lod: 0, indoor: false })
        })
        .collect()
}

fn object_color(k: editor::Kind) -> [f32; 3] {
    let c = editor::draw::kind_color(k);
    let f = |v: u8| (v as f32 / 255.0).powf(2.2) * 0.8;
    [f(c[0]), f(c[1]), f(c[2])]
}

/// Uploads the flat-shaded city plus scene objects that have no prefab, as boxes standing on the terrain.
pub fn scene(gpu: &Gpu, base: &Base, c: &City, s: Option<&editor::Scene>) -> Scene {
    let mut verts = base.verts.clone();
    let mut idx = base.idx.clone();
    for o in s.map_or(&[][..], |s| &s.objects[..]).iter().filter(|o| kind_prefab(o.kind).is_none()) {
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
            if crate::bespoke::claimed(c, b) {
                continue;
            }
            let (y0, y1) = city::mesh::building_span(b, &c.terrain);
            for ring in std::iter::once(&b.outer).chain(&b.holes) {
                let ring: Vec<[f32; 2]> = ring.iter().map(|p| [p[0] as f32, p[1] as f32]).collect();
                let min = ring.iter().fold([f32::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
                let max = ring.iter().fold([f32::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
                solids.push(Solid { ring, min, max, y0: y0 as f32, y1: y1 as f32 });
            }
        }
        // hand-built buildings collide wall by wall, floor by floor
        for bb in crate::bespoke::all(c) {
            for k in bb.geo.blocks {
                let min = k.ring.iter().fold([f32::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
                let max = k.ring.iter().fold([f32::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
                solids.push(Solid { ring: k.ring, min, max, y0: k.y0, y1: k.y1 });
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
    fn factory_has_walkable_floors_and_solid_walls() {
        let c = city();
        let col = Collision::new(&c, None);
        let b = crate::bespoke::all(&c).into_iter().next().expect("factory");
        // the lowest slab top is the ground floor (the tall hall); the upper floors sit 6.0 and 9.9 m above it
        let slab_tops: Vec<f32> = b.geo.blocks.iter().map(|k| k.y1).collect();
        let ground = slab_tops.iter().copied().filter(|&y| y > 0.0).fold(f32::MAX, f32::min);
        let p = [76.0f32, -170.0]; // inside, in the open office area
        for lvl in [6.0f32, 9.9] {
            let f = col.roof_under(p, ground + lvl + 0.1, 0.45).expect("floor under");
            assert!((f - (ground + lvl)).abs() < 0.05, "floor at {f}, expected {}", ground + lvl);
        }
        // a player brushing the west front below the first-floor sills is held off the brick
        let mut q = [65.6f32, -177.3];
        col.push_out(&mut q, ground + 5.0, 0.4, 0.35);
        assert!(q[0] < 65.5, "ended at {q:?}");
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
