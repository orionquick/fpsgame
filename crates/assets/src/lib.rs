//! Megascans asset compiler and loader.
//!
//! `compile` scans the inbox (zips downloaded from Fab), extracts every mesh into metres / Y-up with UVs and
//! tangents (`data/assets/mesh/<id>.bin`) and writes a searchable catalog (`data/assets/catalog.json`).
//! Textures stay inside the zips; `load_material` decodes and resizes them on demand.
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Model,
    Surface,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub tris: u32,
    /// Index into `Asset::materials` (multi-material models).
    #[serde(default)]
    pub material: Option<usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Maps {
    pub base: Option<String>,
    pub normal: Option<String>,
    /// Packed occlusion (R), roughness (G), metalness (B), as in the glTF exports.
    pub orm: Option<String>,
    pub roughness: Option<String>,
    pub gloss: Option<String>,
    pub ao: Option<String>,
    /// Separate opacity map (decals); plants carry opacity in the base map's alpha (`_B-O`).
    #[serde(default)]
    pub opacity: Option<String>,
    /// The packed map is occlusion/roughness/translucency (plants), not metalness.
    #[serde(default)]
    pub orm_is_ort: bool,
    /// The packed map is glTF metallic-roughness only (R unused): take occlusion from `ao` or none.
    #[serde(default)]
    pub orm_no_ao: bool,
    /// glTF material factors (multiply the textures, or stand in for missing ones).
    #[serde(default)]
    pub base_factor: Option<[f32; 4]>,
    #[serde(default)]
    pub rough_factor: Option<f32>,
    #[serde(default)]
    pub metal_factor: Option<f32>,
    /// Transparent glazing (glTF blend mode, low alpha or named glass): drawn in the glass pass.
    #[serde(default)]
    pub glass: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Asset {
    /// Megascans id (e.g. "uhbpbj0dw"); unique per asset.
    pub id: String,
    pub title: String,
    pub group: String,
    pub kind: AssetKind,
    /// Zip path relative to the inbox.
    pub zip: String,
    pub tags: Vec<String>,
    pub parts: Vec<Part>,
    pub maps: Maps,
    /// Real-world size of one texture tile, metres (surfaces).
    pub tile_m: Option<[f32; 2]>,
    /// Normal map uses the DirectX (green-down) convention: the "_ue_" glTF exports.
    pub normal_dx: bool,
    /// Per-part materials for multi-material models (empty: the asset's single `maps`).
    #[serde(default)]
    pub materials: Vec<Maps>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub assets: Vec<Asset>,
}

impl Catalog {
    pub fn load(dir: &Path) -> Res<Catalog> {
        Ok(serde_json::from_slice(&std::fs::read(dir.join("catalog.json"))?)?)
    }
    pub fn get(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }
    /// Case-insensitive search over title, group and tags; every word must match.
    pub fn search(&self, q: &str) -> Vec<&Asset> {
        let words: Vec<String> = q.to_lowercase().split_whitespace().map(String::from).collect();
        self.assets
            .iter()
            .filter(|a| {
                let hay = format!("{} {} {}", a.title, a.group, a.tags.join(" ")).to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }
}

/// Interleaved vertex as stored in mesh files and uploaded to the GPU.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
    pub tan: [f32; 4],
}

#[derive(Clone)]
pub struct MeshPart {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
    /// Simplified index lists over the same vertices: ~25% and ~6% of the triangles.
    pub lods: [Vec<u32>; 2],
}

/// Index-only simplification (borders locked so neighbouring modules stay sealed).
fn simplify(verts: &[Vertex], idx: &[u32], ratio: f32, error: f32) -> Vec<u32> {
    if idx.len() < 300 {
        return idx.to_vec();
    }
    let bytes = unsafe { std::slice::from_raw_parts(verts.as_ptr() as *const u8, std::mem::size_of_val(verts)) };
    let Ok(adapter) = meshopt::VertexDataAdapter::new(bytes, std::mem::size_of::<Vertex>(), 0) else { return idx.to_vec() };
    let target = ((idx.len() as f32 * ratio) as usize / 3 * 3).max(3);
    let out = meshopt::simplify(idx, &adapter, target, error, meshopt::SimplifyOptions::LockBorder, None);
    if out.len() < 3 { idx.to_vec() } else { out }
}

fn mesh_path(out: &Path, id: &str) -> PathBuf {
    out.join("mesh").join(format!("{id}.bin"))
}

const MESH_MAGIC: &[u8; 4] = b"FPM2";

fn write_parts(path: &Path, parts: &[MeshPart]) -> Res<()> {
    let mut b: Vec<u8> = MESH_MAGIC.to_vec();
    b.extend((parts.len() as u32).to_le_bytes());
    for p in parts {
        b.extend((p.verts.len() as u32).to_le_bytes());
        for v in &p.verts {
            for f in v.pos.iter().chain(&v.nrm).chain(&v.uv).chain(&v.tan) {
                b.extend(f.to_le_bytes());
            }
        }
        for list in [&p.idx, &p.lods[0], &p.lods[1]] {
            b.extend((list.len() as u32).to_le_bytes());
            for i in list.iter() {
                b.extend(i.to_le_bytes());
            }
        }
    }
    std::fs::write(path, b)?;
    Ok(())
}

/// Reads the compiled mesh parts (with their simplified levels) of a model asset.
pub fn load_mesh(out: &Path, id: &str) -> Res<Vec<MeshPart>> {
    let b = std::fs::read(mesh_path(out, id))?;
    if b.len() < 8 || &b[..4] != MESH_MAGIC {
        return Err(format!("{id}: old mesh format, rerun `fps assets`").into());
    }
    let mut o = 4usize;
    let u32_at = |o: &mut usize| { let v = u32::from_le_bytes(b[*o..*o + 4].try_into().unwrap()); *o += 4; v };
    let n = u32_at(&mut o);
    let mut parts = vec![];
    for _ in 0..n {
        let nv = u32_at(&mut o) as usize;
        let mut verts = Vec::with_capacity(nv);
        for _ in 0..nv {
            let f: Vec<f32> = (0..12).map(|k| f32::from_le_bytes(b[o + k * 4..o + k * 4 + 4].try_into().unwrap())).collect();
            o += 48;
            verts.push(Vertex { pos: [f[0], f[1], f[2]], nrm: [f[3], f[4], f[5]], uv: [f[6], f[7]], tan: [f[8], f[9], f[10], f[11]] });
        }
        let mut lists: Vec<Vec<u32>> = vec![];
        for _ in 0..3 {
            let ni = u32_at(&mut o) as usize;
            lists.push((0..ni).map(|k| u32::from_le_bytes(b[o + k * 4..o + k * 4 + 4].try_into().unwrap())).collect());
            o += ni * 4;
        }
        let l2 = lists.pop().unwrap();
        let l1 = lists.pop().unwrap();
        let idx = lists.pop().unwrap();
        parts.push(MeshPart { verts, idx, lods: [l1, l2] });
    }
    Ok(parts)
}

type M4 = [[f32; 4]; 4];

fn mul(a: &M4, b: &M4) -> M4 {
    let mut r = [[0f32; 4]; 4];
    for c in 0..4 {
        for row in 0..4 {
            r[c][row] = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}

fn xform(m: &M4, p: [f32; 3], w: f32) -> [f32; 3] {
    [0, 1, 2].map(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r] * w)
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn read_zip_file(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Res<Vec<u8>> {
    let mut f = zip.by_name(name)?;
    let mut v = vec![];
    f.read_to_end(&mut v)?;
    Ok(v)
}

/// Extracts every mesh node of a glTF in the zip, transformed by its node chain (cm Z-up → m Y-up).
type Extracted = (Vec<MeshPart>, Vec<Part>, Vec<Maps>, Vec<Option<usize>>, Vec<String>);

/// `megascans`: apply the Megascans centimetre Z-up fix; `split`: break a prop pack into one object per child of
/// its root node (returned per part, with the object names).
fn extract_model(zip: &mut zip::ZipArchive<std::fs::File>, gltf_name: &str, megascans: bool, split: bool) -> Res<Extracted> {
    let mut v: serde_json::Value = serde_json::from_slice(&read_zip_file(zip, gltf_name)?)?;
    // Unreal exports add custom "_..." vertex attributes the gltf crate rejects; we don't use them.
    for m in v["meshes"].as_array_mut().into_iter().flatten() {
        for p in m["primitives"].as_array_mut().into_iter().flatten() {
            if let Some(a) = p["attributes"].as_object_mut() {
                a.retain(|k, _| !k.starts_with('_'));
            }
        }
    }
    // Optional material extensions (glass transmission) are marked required; we shade them as opaque PBR.
    if let Some(o) = v.as_object_mut() {
        o.remove("extensionsRequired");
        o.remove("extensionsUsed");
    }
    // transmissive (glass) materials, noted before their extensions are stripped
    let transmissive: Vec<bool> = v["materials"].as_array().map_or(vec![], |a| a.iter().map(|m| m["extensions"].get("KHR_materials_transmission").is_some()).collect());
    for m in v["materials"].as_array_mut().into_iter().flatten() {
        if let Some(o) = m.as_object_mut() {
            o.remove("extensions");
        }
    }
    let doc = gltf::Gltf::from_slice(&serde_json::to_vec(&v)?)?;
    let dir = gltf_name.rsplit_once('/').map_or(String::new(), |(d, _)| format!("{d}/"));
    let mut buffers = vec![];
    for b in doc.buffers() {
        match b.source() {
            gltf::buffer::Source::Uri(u) => buffers.push(read_zip_file(zip, &format!("{dir}{u}"))?),
            gltf::buffer::Source::Bin => buffers.push(doc.blob.clone().unwrap_or_default()),
        }
    }
    let mut parts = vec![];
    let mut infos = vec![];
    let multi = doc.materials().len() > 1;
    fn walk<'a>(n: gltf::Node<'a>, parent: M4, obj: Option<usize>, split_at: Option<usize>, out: &mut Vec<(gltf::Node<'a>, M4, Option<usize>)>, objs: &mut Vec<String>) {
        let m = mul(&parent, &n.transform().matrix());
        out.push((n.clone(), m, obj));
        for c in n.children() {
            // children of the split node each start an object
            let o = if Some(n.index()) == split_at {
                let name = c.name().map(String::from).or_else(|| c.mesh().and_then(|m| m.primitives().next().and_then(|p| p.material().name().map(String::from)))).unwrap_or_else(|| format!("object {}", objs.len()));
                objs.push(name);
                Some(objs.len() - 1)
            } else {
                obj
            };
            walk(c, m, o, split_at, out, objs);
        }
    }
    let ident: M4 = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [0., 0., 0., 1.]];
    // packs: descend the single-child chain from the root to the node whose children are the props
    let split_at = if split {
        doc.default_scene().or_else(|| doc.scenes().next()).and_then(|sc| {
            let mut n = sc.nodes().next()?;
            while n.children().count() == 1 {
                n = n.children().next()?;
            }
            (n.children().count() >= 4).then(|| n.index())
        })
    } else {
        None
    };
    let mut nodes = vec![];
    let mut objs = vec![];
    for s in doc.scenes() {
        for n in s.nodes() {
            walk(n, ident, None, split_at, &mut nodes, &mut objs);
        }
    }
    // Megascans centimetre exports are Z-up; some nodes carry the +90° X rotation to Y-up and some don't.
    let to_y_up: M4 = [[1., 0., 0., 0.], [0., 0., 1., 0.], [0., -1., 0., 0.], [0., 0., 0., 1.]];
    let mut part_obj = vec![];
    for (node, m, obj) in nodes {
        let Some(mesh) = node.mesh() else { continue };
        // Only the centimetre Z-up exports (node scale 0.01) lack it; metre exports without a node scale are already Y-up.
        let zlen = (m[2][0].powi(2) + m[2][1].powi(2) + m[2][2].powi(2)).sqrt();
        let m = if megascans && zlen < 0.1 && m[2][1].abs() < 0.5 * zlen { mul(&to_y_up, &m) } else { m };
        // multi-material models: one part per material so each gets its own textures
        let mut groups: Vec<Option<usize>> = if multi { mesh.primitives().map(|p| p.material().index()).collect() } else { vec![None] };
        groups.dedup();
        groups.sort();
        groups.dedup();
        for group in groups {
        let mut verts = vec![];
        let mut idx = vec![];
        for prim in mesh.primitives().filter(|p| !multi || p.material().index() == group) {
            let r = prim.reader(|b| buffers.get(b.index()).map(|v| v.as_slice()));
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<[f32; 3]> = pos.collect();
            let nrm: Vec<[f32; 3]> = r.read_normals().map_or(vec![[0., 1., 0.]; pos.len()], |n| n.collect());
            let uv: Vec<[f32; 2]> = r.read_tex_coords(0).map_or(vec![[0., 0.]; pos.len()], |t| t.into_f32().collect());
            let tan: Vec<[f32; 4]> = r.read_tangents().map_or(vec![[1., 0., 0., 1.]; pos.len()], |t| t.collect());
            let base = verts.len() as u32;
            for i in 0..pos.len() {
                let t = norm3(xform(&m, [tan[i][0], tan[i][1], tan[i][2]], 0.0));
                verts.push(Vertex { pos: xform(&m, pos[i], 1.0), nrm: norm3(xform(&m, nrm[i], 0.0)), uv: uv[i], tan: [t[0], t[1], t[2], tan[i][3]] });
            }
            match r.read_indices() {
                Some(ii) => idx.extend(ii.into_u32().map(|i| i + base)),
                None => idx.extend(base..base + pos.len() as u32),
            }
        }
        if verts.is_empty() {
            continue;
        }
        let min = verts.iter().fold([f32::MAX; 3], |a, v| [a[0].min(v.pos[0]), a[1].min(v.pos[1]), a[2].min(v.pos[2])]);
        let max = verts.iter().fold([f32::MIN; 3], |a, v| [a[0].max(v.pos[0]), a[1].max(v.pos[1]), a[2].max(v.pos[2])]);
        infos.push(Part { name: node.name().unwrap_or("").to_string(), min, max, tris: idx.len() as u32 / 3, material: group });
        part_obj.push(obj);
        let lods = [simplify(&verts, &idx, 0.25, 0.02), simplify(&verts, &idx, 0.06, 0.08)];
        parts.push(MeshPart { verts, idx, lods });
        }
    }
    // materials straight from the glTF (used for multi-material models from Poly Haven / Sketchfab)
    let uri = |t: Option<gltf::texture::Texture>| -> Option<String> {
        match t?.source().source() {
            gltf::image::Source::Uri { uri, .. } => Some(format!("{dir}{}", uri.replace("%20", " "))),
            _ => None,
        }
    };
    let mats = doc
        .materials()
        .map(|mt| {
            let pbr = mt.pbr_metallic_roughness();
            let mr = uri(pbr.metallic_roughness_texture().map(|t| t.texture()));
            let occ = uri(mt.occlusion_texture().map(|t| t.texture()));
            let packed = mr.is_some() && mr == occ;
            let name = mt.name().unwrap_or("").to_lowercase();
            let trans = mt.index().and_then(|i| transmissive.get(i).copied()).unwrap_or(false);
            let glass = trans || mt.alpha_mode() == gltf::material::AlphaMode::Blend && pbr.base_color_factor()[3] < 0.95 || pbr.base_color_factor()[3] < 0.6 || name.contains("glass") || name.contains("bottle");
            Maps {
                glass,
                base: uri(pbr.base_color_texture().map(|t| t.texture())),
                normal: uri(mt.normal_texture().map(|t| t.texture())),
                orm: mr.clone(),
                ao: if packed { None } else { occ },
                orm_no_ao: mr.is_some() && !packed,
                base_factor: Some(pbr.base_color_factor()),
                rough_factor: Some(pbr.roughness_factor()),
                metal_factor: Some(pbr.metallic_factor()),
                ..Default::default()
            }
        })
        .collect();
    Ok((parts, infos, mats, part_obj, objs))
}

/// Unit fixes for third-party models modelled at the wrong scale (by id prefix; checked with `fps gallery`).
const SCALE: &[(&str, f32)] = &[
    ("sf_260ff7a37a", 0.22), // Office Props Pack (crazyshroomz): everything ~4.5x real size
    ("sf_7377ec591d", 0.2),  // Desk lamp
    ("sf_4517652104", 0.02), // Bar Table (centimetres)
    ("sf_aaf556f40f", 1.75), // Bar Stool (Saandy)
    ("sf_561abc2fc9", 0.6),  // Desktop Computer (Tytan)
    ("sf_d021f42ea8", 0.0046), // American fire hydrant (cm, oversized)
    ("sf_f3eb47d02e", 0.4),  // Street Asset Pack (vmatthew)
    ("sf_2f52ea53c5", 0.85), // City Props Collection vol. 2
    ("sf_383923d441", 0.135), // Coffee Maker
    ("sf_6881aa1e84", 0.15), // Street Lamp (bez_glaza)
    ("sf_f61aeec14f", 0.0085), // Trash Can (Lyskilde)
    ("sf_3f0da232c2", 0.3),  // Chalkboard menu stand
    ("sf_bab05bc780", 0.4),  // Commercial Coffee Machine
    ("sf_334377879c", 0.025), // Glass Bottles
    ("sf_f5e340ffca", 0.0025), // Metal Industrial Bar Stool (mm)
    ("sf_5b9c3706a6", 0.47), // Modern Round Bistro Table
    ("sf_ca9ed87062", 0.01), // Restaurant Building Corner Booth (cm)
    ("sf_0720ffc9b2", 0.29), // Sheet Metal Frame Tolix Style Chairs
    ("sf_3fdcc5bc70", 0.29), // Tolix Industrial Chairs
    ("sf_1ea37815fb", 0.15), // The ultimate glass pack
    ("sf_221f20f292", 0.5),  // Willoughsby Espresso Machine
    ("sf_a1961b9e5a", 0.8),  // Wooden Back Bar Cabinet
    ("sf_61275205fe", 0.2),  // Dining Set Scandinavian Style
    ("sf_bcdd3c1ba9", 0.025), // Coffee Grinder
    ("sf_2aff2683d3", 0.35), // Coffee Grinder, PBR
    ("sf_9d2b96b5e3", 0.0145), // Coffee Latte In Mug With Saucer
    ("sf_294c5f6a0d", 1.8),  // Cup of Coffee
    ("sf_9b961b9aab", 0.05), // Hanging Pothos
    ("sf_a651092825", 0.0008), // Laptop (FREE)
    ("sf_46c7d789b0", 0.06), // Napkin Holder
    ("sf_307e887d74", 0.035), // Neon Signs
    ("sf_2bc72a0509", 0.1),  // Paintings and Posters Collection
];

fn rescale(parts: &mut [MeshPart], infos: &mut [Part], k: f32) {
    for (mp, info) in parts.iter_mut().zip(infos.iter_mut()) {
        for v in mp.verts.iter_mut() {
            v.pos = v.pos.map(|c| c * k);
        }
        info.min = info.min.map(|c| c * k);
        info.max = info.max.map(|c| c * k);
    }
}

/// Upright exports whose metadata "height" is really the depth, so the Z-up rule would knock them over
/// (checked by eye with `fps gallery`).
const META_HEIGHT_IS_DEPTH: &[&str] = &[
    "yfoobem", // Urban Office Equipment Cabinet Metal Medium 02
    "yfpocj3", // Cabinet Wood Old 01
    "yfinchh", // Chair Mixed Old 04
    "yfjjfhh", // Chair Mixed Old 05
    "yfrhfbf", // Chair Mixed Old 08
    "yghlabs", // Lamp Metal Old
    "vh1vcba", // Pinboard Cork Old 02
    "ye4pcby", // Pinboard Felt Stained
    "yf2fcdo", // Radiator Metal Weathered
    "yf5eajt", // Shelf Wood Old 01
];

fn suffix_map(names: &[String], keys: &[&str]) -> Option<String> {
    names.iter().find(|n| { let l = n.to_lowercase(); keys.iter().any(|k| l.ends_with(&format!("{k}.jpg")) || l.ends_with(&format!("{k}.png")) || l.ends_with(&format!("{k}.exr"))) }).cloned()
}

fn compile_one(inbox: &Path, out: &Path, rel: &str, title: Option<&str>, group: &str) -> Res<Vec<Asset>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(inbox.join(rel))?)?;
    let names: Vec<String> = zip.file_names().filter_map(|r| r.ok().map(|n| n.to_string())).collect();
    let meta_name = names.iter().find(|n| n.ends_with(".json")).cloned();
    let meta: serde_json::Value = meta_name.as_ref().and_then(|n| read_zip_file(&mut zip, n).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let id = meta["id"].as_str().map(String::from).or_else(|| meta_name.as_ref().map(|n| n.trim_end_matches(".json").rsplit('/').next().unwrap().to_string())).unwrap_or_else(|| rel.replace('/', "_"));
    let title = title.map(String::from).or_else(|| meta["name"].as_str().map(String::from)).unwrap_or_else(|| id.clone());
    let tags: Vec<String> = meta["tags"].as_array().map_or(vec![], |a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect());
    if let Some(g) = names.iter().find(|n| n.ends_with(".gltf")).cloned() {
        let third_party = group == "polyhaven" || group == "sketchfab";
        let pack = group == "sketchfab" && { let t = title.to_lowercase(); ["pack", "assets", "props", "collection", "kit"].iter().any(|k| t.contains(k)) };
        let (mut parts, mut infos, gmats, part_obj, objs) = extract_model(&mut zip, &g, !third_party, pack)?;
        if let Some(&(_, k)) = SCALE.iter().find(|(p, _)| id.starts_with(p)) {
            rescale(&mut parts, &mut infos, k);
        }
        // Some metre-scale exports are still Z-up: if the metadata height matches the Z extent, stand them up.
        let meta_h = meta["meta"].as_array().and_then(|m| m.iter().find(|x| x["key"] == "height")).and_then(|x| x["value"].as_str().map(String::from).or_else(|| x["value"].as_f64().map(|f| f.to_string()))).and_then(|s| s.trim_end_matches('m').trim().parse::<f32>().ok());
        let ext = |i: usize| infos.iter().map(|p| p.max[i]).fold(f32::MIN, f32::max) - infos.iter().map(|p| p.min[i]).fold(f32::MAX, f32::min);
        let by_meta = meta_h.is_some_and(|h| (ext(2) - h).abs() < 0.05 * h.max(0.2) && (ext(1) - h).abs() > 0.15 * h.max(0.2));
        if by_meta && !META_HEIGHT_IS_DEPTH.contains(&id.as_str()) {
            {
                for (mp, info) in parts.iter_mut().zip(infos.iter_mut()) {
                    // -90° about X: (x, y, z) -> (x, z, -y)
                    let r = |v: [f32; 3]| [v[0], v[2], -v[1]];
                    for v in mp.verts.iter_mut() {
                        v.pos = r(v.pos);
                        v.nrm = r(v.nrm);
                        let t = r([v.tan[0], v.tan[1], v.tan[2]]);
                        v.tan = [t[0], t[1], t[2], v.tan[3]];
                    }
                    info.min = mp.verts.iter().fold([f32::MAX; 3], |a, v| [a[0].min(v.pos[0]), a[1].min(v.pos[1]), a[2].min(v.pos[2])]);
                    info.max = mp.verts.iter().fold([f32::MIN; 3], |a, v| [a[0].max(v.pos[0]), a[1].max(v.pos[1]), a[2].max(v.pos[2])]);
                }
            }
        }
        write_parts(&mesh_path(out, &id), &parts)?;
        let ort = suffix_map(&names, &["_ort"]);
        let maps = Maps {
            base: suffix_map(&names, &["_b-o"]).filter(|n| !n.to_lowercase().contains("billboard")).or_else(|| suffix_map(&names, &["_b"])),
            normal: suffix_map(&names, &["_n"]),
            orm: suffix_map(&names, &["_orm"]).or(ort.clone()),
            orm_is_ort: suffix_map(&names, &["_orm"]).is_none() && ort.is_some(),
            ..Default::default()
        };
        // Poly Haven / Sketchfab models carry their materials in the glTF itself
        let gltf_mats = group == "polyhaven" || group == "sketchfab" || (maps.base.is_none() && gmats.iter().any(|m| m.base.is_some()));
        let (maps, materials) = if gltf_mats { (gmats.first().cloned().unwrap_or_default(), gmats) } else { (maps, vec![]) };
        if pack && !objs.is_empty() {
            // one asset per prop in the pack, each with its own mesh file; materials are shared
            let mut out_assets = vec![];
            for (k, name) in objs.iter().enumerate() {
                let sel: Vec<usize> = (0..parts.len()).filter(|&i| part_obj[i] == Some(k)).collect();
                if sel.is_empty() {
                    continue;
                }
                let sub_id = format!("{id}_{k:02}");
                let sub_parts: Vec<MeshPart> = sel.iter().map(|&i| parts[i].clone()).collect();
                write_parts(&mesh_path(out, &sub_id), &sub_parts)?;
                out_assets.push(Asset { id: sub_id, title: format!("{title} / {name}"), group: group.into(), kind: AssetKind::Model, zip: rel.into(), tags: tags.clone(), parts: sel.iter().map(|&i| infos[i].clone()).collect(), maps: maps.clone(), tile_m: None, normal_dx: false, materials: materials.clone() });
            }
            return Ok(out_assets);
        }
        return Ok(vec![Asset { id, title, group: group.into(), kind: AssetKind::Model, zip: rel.into(), tags, parts: infos, maps, tile_m: None, normal_dx: rel.contains("_ue_"), materials }]);
    }
    let tile = meta["maps"].as_array().and_then(|m| m.iter().find_map(|x| x["physicalSize"].as_str())).and_then(|s| {
        let (a, b) = s.split_once('x')?;
        Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
    });
    let maps = Maps {
        base: suffix_map(&names, &["_basecolor", "_albedo"]),
        normal: suffix_map(&names, &["_normal"]),
        roughness: suffix_map(&names, &["_roughness"]),
        gloss: suffix_map(&names, &["_gloss"]),
        ao: suffix_map(&names, &["_ao"]),
        opacity: suffix_map(&names, &["_opacity"]),
        ..Default::default()
    };
    if maps.base.is_none() {
        return Err(format!("{rel}: no mesh and no base colour map").into());
    }
    Ok(vec![Asset { id, title, group: group.into(), kind: AssetKind::Surface, zip: rel.into(), tags, parts: vec![], maps, tile_m: tile.or(Some([2.0, 2.0])), normal_dx: false, materials: vec![] }])
}

/// Compiles every zip under `inbox` (titles and groups from `manifest.jsonl` when present) into `out`.
pub fn compile(inbox: &Path, out: &Path) -> Res<(Catalog, Vec<String>)> {
    std::fs::create_dir_all(out.join("mesh"))?;
    let mut manifest = std::collections::HashMap::new();
    if let Ok(t) = std::fs::read_to_string(inbox.join("manifest.jsonl")) {
        for l in t.lines() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(l) {
                if let (Some(p), Some(t)) = (v["path"].as_str(), v["title"].as_str()) {
                    manifest.insert(p.to_string(), t.to_string());
                }
            }
        }
    }
    let mut jobs = vec![];
    for e in std::fs::read_dir(inbox)? {
        let p = e?.path();
        if p.is_dir() {
            let group = p.file_name().unwrap().to_string_lossy().to_string();
            for f in std::fs::read_dir(&p)? {
                let f = f?.path();
                if f.extension().is_some_and(|x| x == "zip") {
                    jobs.push((format!("{group}/{}", f.file_name().unwrap().to_string_lossy()), group.clone()));
                }
            }
        } else if p.extension().is_some_and(|x| x == "zip") {
            jobs.push((p.file_name().unwrap().to_string_lossy().to_string(), "surface".into()));
        }
    }
    jobs.sort();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(vec![]);
    std::thread::scope(|s| {
        for _ in 0..std::thread::available_parallelism().map_or(4, |n| n.get()) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some((rel, group)) = jobs.get(i) else { break };
                let r = compile_one(inbox, out, rel, manifest.get(rel).map(String::as_str), group).map_err(|e| format!("{rel}: {e}"));
                results.lock().unwrap().push(r);
            });
        }
    });
    let mut cat = Catalog::default();
    let mut errors = vec![];
    let mut seen = std::collections::HashSet::new();
    for r in results.into_inner().unwrap() {
        match r {
            Ok(list) => {
                for a in list {
                    if seen.insert(a.id.clone()) {
                        cat.assets.push(a);
                    }
                }
            }
            Err(e) => errors.push(e),
        }
    }
    cat.assets.sort_by(|a, b| (&a.group, &a.title, &a.id).cmp(&(&b.group, &b.title, &b.id)));
    std::fs::write(out.join("catalog.json"), serde_json::to_vec_pretty(&cat)?)?;
    Ok((cat, errors))
}

/// Decoded, resized PBR textures for one asset: base colour (sRGB), ORM (linear), normal (linear, OpenGL convention).
pub struct MaterialImages {
    pub base: image::RgbaImage,
    pub orm: image::RgbaImage,
    pub normal: image::RgbaImage,
}

fn decode(zip: &mut zip::ZipArchive<std::fs::File>, name: &Option<String>, size: u32) -> Option<image::RgbaImage> {
    let bytes = read_zip_file(zip, name.as_ref()?).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(if img.width() != size { image::imageops::resize(&img, size, size, image::imageops::FilterType::Triangle) } else { img })
}

/// Loads an asset's textures from its zip at `size` x `size`.
pub fn load_material(inbox: &Path, a: &Asset, size: u32) -> Res<MaterialImages> {
    load_material_n(inbox, a, None, size)
}

/// Loads material `n` of a multi-material asset (None: the asset's own maps).
pub fn load_material_n(inbox: &Path, a: &Asset, n: Option<usize>, size: u32) -> Res<MaterialImages> {
    let maps = n.and_then(|i| a.materials.get(i)).unwrap_or(&a.maps);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(inbox.join(&a.zip))?)?;
    let bf = maps.base_factor.unwrap_or([1.0; 4]);
    let to8 = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    let mut base = decode(&mut zip, &maps.base, size).unwrap_or_else(|| image::RgbaImage::from_pixel(size, size, if maps.base_factor.is_some() { image::Rgba([255, 255, 255, 255]) } else { image::Rgba([200, 200, 200, 255]) }));
    if maps.base_factor.is_some() && bf != [1.0; 4] {
        // factor is linear: apply in linear space
        let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
        base.pixels_mut().for_each(|p| {
            for k in 0..3 {
                p[k] = to8(lin(p[k]) * bf[k]);
            }
            p[3] = (p[3] as f32 * bf[3]) as u8;
        });
    }
    let base = base;
    let mut normal = decode(&mut zip, &maps.normal, size).unwrap_or_else(|| image::RgbaImage::from_pixel(size, size, image::Rgba([128, 128, 255, 255])));
    if a.normal_dx {
        normal.pixels_mut().for_each(|p| p[1] = 255 - p[1]);
    }
    let mut base = base;
    if let Some(op) = decode(&mut zip, &maps.opacity, size) {
        for (p, o) in base.pixels_mut().zip(op.pixels()) {
            p[3] = o[0];
        }
    }
    let rf = maps.rough_factor.unwrap_or(1.0);
    let mf = maps.metal_factor.unwrap_or(1.0);
    let orm = match decode(&mut zip, &maps.orm, size) {
        Some(mut o) => {
            if maps.orm_is_ort {
                o.pixels_mut().for_each(|p| p[2] = 0);
            }
            if maps.orm_no_ao {
                let ao = decode(&mut zip, &maps.ao, size);
                for (x, y, p) in o.enumerate_pixels_mut() {
                    p[0] = ao.as_ref().map_or(255, |i| i.get_pixel(x, y)[0]);
                }
            }
            if maps.rough_factor.is_some() {
                o.pixels_mut().for_each(|p| {
                    p[1] = (p[1] as f32 * rf) as u8;
                    p[2] = (p[2] as f32 * mf) as u8;
                });
            }
            o
        }
        None if maps.rough_factor.is_some() => {
            let ao = decode(&mut zip, &maps.ao, size);
            image::RgbaImage::from_fn(size, size, |x, y| image::Rgba([ao.as_ref().map_or(255, |i| i.get_pixel(x, y)[0]), (rf * 255.0) as u8, (mf * 255.0) as u8, 255]))
        }
        None => {
            let ao = decode(&mut zip, &maps.ao, size);
            let rough = decode(&mut zip, &maps.roughness, size);
            let gloss = if rough.is_none() { decode(&mut zip, &maps.gloss, size) } else { None };
            image::RgbaImage::from_fn(size, size, |x, y| {
                let o = ao.as_ref().map_or(255, |i| i.get_pixel(x, y)[0]);
                let r = rough.as_ref().map(|i| i.get_pixel(x, y)[0]).or_else(|| gloss.as_ref().map(|i| 255 - i.get_pixel(x, y)[0])).unwrap_or(200);
                image::Rgba([o, r, 0, 255])
            })
        }
    };
    Ok(MaterialImages { base, orm, normal })
}
