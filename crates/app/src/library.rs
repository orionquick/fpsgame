//! Megascans materials and prefab meshes for rendering: picks assets by title, loads them on demand, caches them.
use assets::{Asset, Catalog};
use city::mesh::Slot;
use render::{Batch, Gpu, Instance, Material, Renderer, TexVertex};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Library {
    pub catalog: Catalog,
    pub inbox: PathBuf,
    pub out: PathBuf,
    materials: HashMap<String, Arc<Material>>,
    meshes: HashMap<String, Arc<Vec<(Vec<TexVertex>, [Vec<u32>; 3])>>>,
    plain: Option<Arc<Material>>,
    glass: Option<Arc<Material>>,
}

/// Texture size loaded onto the GPU (from the 2K sources).
const TEX: u32 = 1024;

/// Megascans surface per city slot (first match by title wins).
pub fn slot_surface(s: Slot) -> Option<(&'static str, [f32; 3])> {
    Some(match s {
        Slot::Road => ("Cracked Asphalt", [0.66, 0.66, 0.68]),
        Slot::Service | Slot::Parking => ("Cracked Asphalt", [0.58, 0.58, 0.6]),
        Slot::Path => ("Concrete Pavement", [0.95, 0.95, 0.95]),
        Slot::Ground => ("Concrete Pavement", [1.0, 1.0, 1.0]),
        Slot::Plaza => ("Brushed Concrete Floor", [1.0, 1.0, 1.0]),
        Slot::Rail => ("Concrete Pavement", [0.55, 0.5, 0.45]),
        Slot::Roof => ("Brushed Concrete Floor", [0.7, 0.7, 0.7]),
        Slot::Wall(i) => (["Brick Facade", "Black Brick Facade", "Stucco Facade", "Plaster Facade", "Brick Facade", "Concrete Facade", "Metal Panels Facade", "White Facade", "Terrazzo Facade"][i as usize % 9], [1.0, 1.0, 1.0]),
        Slot::Grass => ("Lawn Grass", [0.85, 0.9, 0.8]),
        Slot::Pitch => ("Lawn Grass", [0.7, 0.85, 0.65]),
        Slot::Tree => return None,
    })
}

impl Library {
    pub fn open(root: &std::path::Path) -> Option<Self> {
        let out = root.join("data/assets");
        let catalog = Catalog::load(&out).ok()?;
        Some(Self { catalog, inbox: root.join("data/megascans_inbox"), out, materials: HashMap::new(), meshes: HashMap::new(), plain: None, glass: None })
    }


    /// Loads (in parallel) every material not yet cached.
    pub fn preload(&mut self, gpu: &Gpu, r: &Renderer, ids: &[String]) {
        // keys are an asset id, or "id#n" for material n of a multi-material model
        let todo: Vec<(String, Asset, Option<usize>)> = ids
            .iter()
            .filter(|id| !self.materials.contains_key(*id))
            .filter_map(|key| {
                let (id, n) = match key.split_once('#') {
                    Some((id, n)) => (id, n.parse().ok()),
                    None => (key.as_str(), None),
                };
                self.catalog.get(id).cloned().map(|a| (key.clone(), a, n))
            })
            .collect();
        let inbox = self.inbox.clone();
        let cache_dir = self.out.join("texcache");
        let _ = std::fs::create_dir_all(&cache_dir);
        let bc = gpu.bc;
        // with block compression everything loads at full size; without it, small props drop to half size
        enum Loaded {
            Bc(render::CompressedTex3),
            Rgba(assets::MaterialImages),
            Missing,
        }
        let loaded: Vec<(String, Loaded)> = std::thread::scope(|s| {
            let hs: Vec<_> = todo
                .iter()
                .map(|(key, a, n)| {
                    let (inbox, cache_dir) = (&inbox, &cache_dir);
                    let small = matches!(a.group.as_str(), "interior" | "polyhaven" | "sketchfab");
                    let size = if bc || !small { TEX } else { TEX / 2 };
                    s.spawn(move || {
                        let file = cache_dir.join(format!("{}_{size}.bct", key.replace('#', "_")));
                        if bc {
                            if let Some(t) = std::fs::read(&file).ok().and_then(|b| render::read_compressed(&b)) {
                                return (key.clone(), Loaded::Bc(t));
                            }
                        }
                        match assets::load_material_n(inbox, a, *n, size) {
                            Ok(i) if bc => {
                                let t = render::compress_material(&i.base, &i.orm, &i.normal);
                                let _ = std::fs::write(&file, render::write_compressed(&t));
                                (key.clone(), Loaded::Bc(t))
                            }
                            Ok(i) => (key.clone(), Loaded::Rgba(i)),
                            Err(_) => (key.clone(), Loaded::Missing),
                        }
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (id, l) in loaded {
            let m = match l {
                Loaded::Bc(t) => Arc::new(Material::from_compressed(gpu, r, &t)),
                Loaded::Rgba(i) => Arc::new(Material::new(gpu, r, &i.base, &i.orm, &i.normal)),
                Loaded::Missing => self.plain(gpu, r),
            };
            self.materials.insert(id, m);
        }
    }

    pub fn material(&mut self, gpu: &Gpu, r: &Renderer, id: &str) -> Arc<Material> {
        self.preload(gpu, r, &[id.to_string()]);
        self.materials.get(id).cloned().unwrap_or_else(|| self.plain(gpu, r))
    }

    /// Smooth dark glazing: shows the sky reflection.
    pub fn glass(&mut self, gpu: &Gpu, r: &Renderer) -> Arc<Material> {
        self.glass.get_or_insert_with(|| Arc::new(Material::plain(gpu, r, 30))).clone()
    }

    pub fn plain(&mut self, gpu: &Gpu, r: &Renderer) -> Arc<Material> {
        self.plain.get_or_insert_with(|| Arc::new(Material::plain(gpu, r, 200))).clone()
    }

    pub fn mesh(&mut self, id: &str) -> Option<Arc<Vec<(Vec<TexVertex>, [Vec<u32>; 3])>>> {
        if let Some(m) = self.meshes.get(id) {
            return Some(m.clone());
        }
        let parts = assets::load_mesh(&self.out, id).ok()?;
        let m: Arc<Vec<(Vec<TexVertex>, [Vec<u32>; 3])>> = Arc::new(parts.into_iter().map(|p| { let [l1, l2] = p.lods; (p.verts.iter().map(|v| TexVertex { pos: v.pos, nrm: v.nrm, uv: v.uv, tan: v.tan }).collect(), [p.idx, l1, l2]) }).collect());
        self.meshes.insert(id.to_string(), m.clone());
        Some(m)
    }
}

/// Bounding-box size of all parts (x, y, z).
pub fn extent(a: &Asset) -> [f32; 3] {
    let mn = a.parts.iter().fold([f32::MAX; 3], |m, p| [m[0].min(p.min[0]), m[1].min(p.min[1]), m[2].min(p.min[2])]);
    let mx = a.parts.iter().fold([f32::MIN; 3], |m, p| [m[0].max(p.max[0]), m[1].max(p.max[1]), m[2].max(p.max[2])]);
    [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]]
}

/// A placed prefab: one asset part at a position on the ground with a heading and per-axis scale.
pub struct Placement {
    pub asset: String,
    pub part: usize,
    pub x: f64,
    pub z: f64,
    pub y: f64,
    /// Degrees clockwise from north; the asset's length (X) runs along it.
    pub rot: f64,
    pub scale: [f32; 3],
    pub tint: [f32; 3],
    /// Mesh level: 0 full, 1 ~25%, 2 ~6% of the triangles.
    pub lod: u8,
    /// Lit as an interior (dim sky light, lamp light).
    pub indoor: bool,
}

/// Indoor lighting: sky light factor and lamp light strength.
pub const INDOOR_AMBIENT: f32 = 0.22;
pub const INDOOR_LAMP: f32 = 0.22;

/// Pseudo-asset: a unit glazing quad (x -0.5..0.5, y 0..1, facing +Z), drawn with the glass material.
pub const GLASS: &str = "__glass";

/// `Placement::part` value meaning "every part, as one object" (multi-part plants and kits).
pub const ALL_PARTS: usize = usize::MAX;

/// Model matrix: centre the part (or the whole asset) on its footprint, stand it on y, turn its X axis to the heading.
pub fn placement_matrix(a: &Asset, part: usize, p: &Placement) -> glam::Mat4 {
    let (mn, mx) = if part == ALL_PARTS {
        let mn = a.parts.iter().fold([f32::MAX; 3], |m, q| [m[0].min(q.min[0]), m[1].min(q.min[1]), m[2].min(q.min[2])]);
        let mx = a.parts.iter().fold([f32::MIN; 3], |m, q| [m[0].max(q.max[0]), m[1].max(q.max[1]), m[2].max(q.max[2])]);
        (mn, mx)
    } else {
        let q = &a.parts[part.min(a.parts.len() - 1)];
        (q.min, q.max)
    };
    let c = glam::Vec3::new((mn[0] + mx[0]) * 0.5, mn[1], (mn[2] + mx[2]) * 0.5);
    glam::Mat4::from_translation(glam::Vec3::new(p.x as f32, p.y as f32, p.z as f32))
        * glam::Mat4::from_rotation_y((90.0 - p.rot as f32).to_radians())
        * glam::Mat4::from_scale(glam::Vec3::from(p.scale))
        * glam::Mat4::from_translation(-c)
}

/// Groups placements into one instanced batch per asset part.
pub fn prefab_batches(gpu: &Gpu, r: &Renderer, lib: &mut Library, placements: &[Placement]) -> Vec<Batch> {
    let mut groups: HashMap<(String, usize, u8), Vec<Instance>> = HashMap::new();
    let mut part_mat: HashMap<(String, usize), String> = HashMap::new();
    let mut glass_parts: HashMap<(String, usize), Vec<Instance>> = HashMap::new();
    let indoor = |p: &Placement| if p.indoor { [0.0, INDOOR_AMBIENT, INDOOR_LAMP, 0.0] } else { [0.0; 4] };
    let mut glass = vec![];
    for p in placements {
        if p.asset == GLASS {
            let m = glam::Mat4::from_translation(glam::Vec3::new(p.x as f32, p.y as f32, p.z as f32)) * glam::Mat4::from_rotation_y((90.0 - p.rot as f32).to_radians()) * glam::Mat4::from_scale(glam::Vec3::from(p.scale));
            glass.push(Instance::new(m, p.tint, 1.0).with_params([3.0, 0.0, 0.0, 0.0]));
            continue;
        }
        let Some(a) = lib.catalog.get(&p.asset).cloned() else { continue };
        if a.parts.is_empty() {
            continue;
        }
        let m = placement_matrix(&a, p.part, p);
        let parts: Vec<usize> = if p.part == ALL_PARTS { (0..a.parts.len()).collect() } else { vec![p.part.min(a.parts.len() - 1)] };
        for part in parts {
            // glass materials in models go to the transparent pass
            let is_glass = a.parts[part].material.and_then(|m| a.materials.get(m)).is_some_and(|m| m.glass);
            if is_glass {
                glass_parts.entry((p.asset.clone(), part)).or_default().push(Instance::new(m, [0.85, 0.88, 0.88], 1.0));
                continue;
            }
            let key = a.parts[part].material.filter(|_| !a.materials.is_empty()).map_or(a.id.clone(), |m| format!("{}#{m}", a.id));
            part_mat.insert((p.asset.clone(), part), key);
            groups.entry((p.asset.clone(), part, p.lod.min(2))).or_default().push(Instance::new(m, p.tint, 1.0).with_params(indoor(p)));
        }
    }
    let mut out = vec![];
    if !glass.is_empty() {
        let q = |x: f32, y: f32| TexVertex { pos: [x, y, 0.0], nrm: [0.0, 0.0, 1.0], uv: [x + 0.5, 1.0 - y], tan: [1.0, 0.0, 0.0, 1.0] };
        let mat = lib.glass(gpu, r);
        out.extend(Batch::new(gpu, &[q(-0.5, 0.0), q(0.5, 0.0), q(0.5, 1.0), q(-0.5, 1.0)], &[0, 1, 2, 0, 2, 3], &glass, mat));
    }
    for ((id, part), inst) in glass_parts {
        let Some(mesh) = lib.mesh(&id) else { continue };
        let mat = lib.glass(gpu, r);
        if let Some((v, lists)) = mesh.get(part) {
            out.extend(Batch::new(gpu, v, &lists[0], &inst, mat).map(|b| b.transparent()));
        }
    }
    let ids: Vec<String> = part_mat.values().cloned().collect();
    lib.preload(gpu, r, &ids);
    for ((id, part, lod), inst) in groups {
        let Some(mesh) = lib.mesh(&id) else { continue };
        let mat = lib.material(gpu, r, part_mat.get(&(id.clone(), part)).unwrap_or(&id));
        if let Some((v, lists)) = mesh.get(part) {
            out.extend(Batch::new(gpu, v, &lists[lod as usize], &inst, mat));
        }
    }
    out
}
