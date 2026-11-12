use city::City;
use render::{Gpu, Scene, Vertex};

/// The part of the city that is drawn while work is concentrated on the ballpark: its own block out to the far
/// kerbs of the streets round it - 7th Avenue on the west, J Street on the north, 10th Avenue and Park Boulevard on
/// the east and south-east, the railway on the south-west. Everything outside is left out of the picture: ground,
/// streets, trees, buildings. Two convex pieces (world x, z), both wound the same way.
/// `FPS_CITY=full` brings the whole city back.
const FOCUS: [&[[f64; 2]]; 2] = [
    &[[-129.0, -267.0], [152.0, -267.0], [152.0, -143.0], [-129.0, -143.0]],
    &[[-129.0, -143.0], [152.0, -143.0], [222.0, -36.0], [224.0, -22.0], [-7.0, 218.0], [-129.0, 132.0]],
];

pub fn focused() -> bool {
    // (the tests walk streets all over the city: they always see all of it)
    !cfg!(test) && std::env::var("FPS_CITY").map_or(true, |v| v != "full")
}

fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

fn in_piece(r: &[[f64; 2]], p: [f64; 2]) -> bool {
    (0..r.len()).all(|i| side(r[i], r[(i + 1) % r.len()], p) >= 0.0)
}

/// Whether a world point is drawn (always, when the whole city is shown).
pub fn in_focus(p: [f64; 2]) -> bool {
    !focused() || FOCUS.iter().any(|r| in_piece(r, p))
}

/// Whether a generic building is left out altogether (drawing, collision, bounce light): it stands outside the
/// focus area. Judged by the middle of its outline, so a building across the street is never cut in half.
pub fn building_hidden(b: &city::model::Building) -> bool {
    let n = b.outer.len().max(1) as f64;
    let c = [b.outer.iter().map(|p| p[0]).sum::<f64>() / n, b.outer.iter().map(|p| p[1]).sum::<f64>() / n];
    if !focused() {
        return false;
    }
    // 10th Avenue curves along the east side: the focus area's straight edge takes in a little ground beyond it,
    // but no building beyond the street's centre line belongs to the block
    const TENTH: [[f64; 2]; 4] = [[140.0, -140.0], [170.0, -67.0], [200.0, -37.0], [210.0, -27.0]];
    let beyond_tenth = TENTH.windows(2).any(|w| c[1] >= w[0][1] && c[1] <= w[1][1] && c[0] > w[0][0] + (w[1][0] - w[0][0]) * (c[1] - w[0][1]) / (w[1][1] - w[0][1]));
    !in_focus(c) || beyond_tenth
}

/// Cuts a city mesh down to the focus area: triangles are clipped on its boundary, not just dropped, so the ground
/// and streets end in a clean line.
fn clip_to_focus(m: &mut city::mesh::Mesh) {
    if !focused() {
        return;
    }
    #[derive(Clone, Copy)]
    struct V {
        pos: [f32; 3],
        nrm: [f32; 3],
        col: [f32; 3],
        uv: [f32; 2],
        tan: [f32; 4],
    }
    let get = |m: &city::mesh::Mesh, i: u32| V { pos: m.pos[i as usize], nrm: m.nrm[i as usize], col: m.col[i as usize], uv: m.uv[i as usize], tan: m.tan[i as usize] };
    let xz = |v: &V| [v.pos[0] as f64, v.pos[2] as f64];
    let mix = |a: &V, b: &V, t: f32| {
        let l = |x: f32, y: f32| x + (y - x) * t;
        V { pos: [l(a.pos[0], b.pos[0]), l(a.pos[1], b.pos[1]), l(a.pos[2], b.pos[2])], nrm: a.nrm, col: a.col, uv: [l(a.uv[0], b.uv[0]), l(a.uv[1], b.uv[1])], tan: a.tan }
    };
    let old = std::mem::take(&mut m.idx);
    let mut idx = Vec::with_capacity(old.len());
    for t in old.chunks_exact(3) {
        let tri = [get(m, t[0]), get(m, t[1]), get(m, t[2])];
        if FOCUS.iter().any(|r| tri.iter().all(|v| in_piece(r, xz(v)))) {
            idx.extend_from_slice(t);
            continue;
        }
        for r in FOCUS {
            let mut poly: Vec<V> = tri.to_vec();
            for i in 0..r.len() {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                let mut next = Vec::with_capacity(poly.len() + 2);
                for k in 0..poly.len() {
                    let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
                    let (dp, dq) = (side(a, b, xz(&p)), side(a, b, xz(&q)));
                    if dp >= 0.0 {
                        next.push(p);
                    }
                    if (dp >= 0.0) != (dq >= 0.0) {
                        next.push(mix(&p, &q, (dp / (dp - dq)) as f32));
                    }
                }
                poly = next;
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() >= 3 {
                let b = m.pos.len() as u32;
                for v in &poly {
                    m.pos.push(v.pos);
                    m.nrm.push(v.nrm);
                    m.col.push(v.col);
                    m.uv.push(v.uv);
                    m.tan.push(v.tan);
                }
                for k in 1..poly.len() as u32 - 1 {
                    idx.extend([b, b + k, b + k + 1]);
                }
            }
        }
    }
    m.idx = idx;
}

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
        for (slot, mut m) in city::mesh::build_slots_ex(c, &crate::bespoke::ground_holes()) {
            clip_to_focus(&mut m);
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
            if crate::bespoke::claimed(c, b) || building_hidden(b) {
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
    fn ballpark_stair_cores_reach_the_terrace_and_upper_deck() {
        let c = city();
        let col = Collision::new(&c, None);
        for (i, route) in crate::bespoke::petco::core_routes(&c).into_iter().enumerate() {
            walk(&c, &col, &format!("core {i}"), &route);
        }
    }

    #[test]
    fn ballpark_grand_stair_leads_onto_the_concourse_and_down_to_the_seats() {
        let c = city();
        let col = Collision::new(&c, None);
        let y = walk(&c, &col, "home gate", &crate::bespoke::petco::home_gate_route());
        assert!(y > 4.3 && y < 6.6, "under the terrace the rows should be between the open bowl and the concourse: {y:.2}");
    }

    #[test]
    fn ballpark_seats_face_the_field() {
        let c = city();
        let park = crate::bespoke::all(&c).into_iter().find(|b| b.osm == crate::bespoke::petco::OSM).expect("ballpark");
        let ring = crate::bespoke::petco::field_ring();
        let n = ring.len() as f64;
        let (cx, cz) = (ring.iter().map(|p| p[0]).sum::<f64>() / n, ring.iter().map(|p| p[1]).sum::<f64>() / n);
        let seats: Vec<_> = park.furniture.iter().filter(|p| p.asset == crate::procmodel::SEAT).collect();
        assert!(seats.len() > 20_000, "only {} chairs placed (run `fps proctex` and `fps assets` for the chair model)", seats.len());
        for p in seats {
            // a model's front (+z) points along the heading a quarter turn on from its `rot`
            let h = (p.rot + 90.0).to_radians();
            let (to_x, to_z) = (cx - p.x, cz - p.z);
            let facing = (h.sin() * to_x - h.cos() * to_z) / to_x.hypot(to_z);
            assert!(facing > 0.25, "the chair at ({:.1}, {:.1}) faces away from the field ({facing:.2})", p.x, p.z);
        }
    }

    #[test]
    fn park_hill_can_be_walked_up_from_the_lawn() {
        let c = city();
        let col = Collision::new(&c, None);
        walk(&c, &col, "park hill", &crate::bespoke::petco::park_hill_route());
    }

    /// Walks a route of waypoints with the player's collision; returns the floor height at the end.
    fn walk(c: &city::City, col: &Collision, name: &str, route: &[([f32; 2], Option<f32>)]) -> f32 {
        let mut p = route[0].0;
        let mut y = c.terrain.at(p[0] as f64, p[1] as f64) as f32;
        for (j, &(target, expect)) in route.iter().enumerate().skip(1) {
            for _ in 0..600 {
                let (dx, dz) = (target[0] - p[0], target[1] - p[1]);
                let d = dx.hypot(dz);
                if d < 0.15 {
                    break;
                }
                let s = d.min(0.1);
                let mut q = [p[0] + dx / d * s, p[1] + dz / d * s];
                col.push_out(&mut q, y + 0.45, 1.35, 0.35);
                p = q;
                let ground = c.terrain.at(p[0] as f64, p[1] as f64) as f32;
                // fall at most 0.5 m a tick (gravity), land on the highest floor within a step
                let floor = col.roof_under(p, y, 0.45).map_or(ground, |r| r.max(ground));
                y = floor.max(y - 0.5);
            }
            let miss = (target[0] - p[0]).hypot(target[1] - p[1]);
            if miss >= 0.6 {
                for s in &col.solids {
                    let q = [p[0] + (target[0] - p[0]) / miss * 0.4, p[1] + (target[1] - p[1]) / miss * 0.4];
                    if point_in(&s.ring, q) && s.y1 > y + 0.45 && s.y0 < y + 1.8 {
                        eprintln!("blocker y {:.2}..{:.2} ring {:?}", s.y0, s.y1, s.ring);
                    }
                }
            }
            assert!(miss < 0.6, "{name}: stuck before waypoint {j} at {p:?} y {y:.2} (target {target:?})");
            if let Some(e) = expect {
                assert!((y - e).abs() < 0.3, "{name}: at waypoint {j} floor {y:.2}, expected {e:.2}");
            }
        }
        y
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
