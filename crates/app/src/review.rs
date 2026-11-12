//! Headless review renders: top-down ortho, side-by-side with the aerial photo, and eye-height shots.
use city::City;
use image::{Rgba, RgbaImage};
use render::{Camera, Gpu, Renderer};
use std::path::Path;

pub const GRID_M: f64 = 100.0;

pub struct Ctx {
    pub gpu: Gpu,
    pub renderer: Renderer,
    pub base: crate::world::Base,
    pub scene: render::Scene,
    pub city: City,
    pub lib: Option<crate::library::Library>,
    pub city_batches: Vec<render::Batch>,
    pub prefab_batches: Vec<render::Batch>,
    /// Prefab buildings, rebuilt when the detail focus moves.
    pub building_batches: Vec<render::Batch>,
    /// Interior lamps (nearest ones are uploaded per render).
    pub lights: Vec<render::PointLight>,
    /// Reflection probes (capture point, box min, box max); the one holding the camera is baked per shot.
    pub probes: Vec<[glam::Vec3; 3]>,
    /// Bespoke building surfaces for the GI voxelizer.
    pub gi_tris: Vec<crate::gi::Tri>,
    pub gi_city: Vec<crate::gi::Prism>,
    focus: Option<[f64; 4]>,
    /// The captured mesh of the ballpark's block, when its local data is present.
    scan_batches: Vec<render::Batch>,
}

impl Ctx {
    pub fn new(city: City) -> Self {
        let gpu = Gpu::headless();
        let renderer = Renderer::new(&gpu, wgpu_format());
        let mut lib = crate::library::Library::open(&crate::root());
        let tree_pl = lib.as_ref().map_or(vec![], |l| crate::dressing::trees(l, &city, city.play_rect()));
        let base = crate::world::Base::new(&city, !tree_pl.is_empty());
        let scene = crate::world::scene(&gpu, &base, &city, None);
        let mut city_batches = vec![];
        if let Some(l) = lib.as_mut() {
            city_batches = crate::world::city_batches(&gpu, &renderer, l, &base);
            city_batches.extend(crate::library::prefab_batches(&gpu, &renderer, l, &tree_pl));
            city_batches.extend(crate::bespoke::all_batches(&gpu, &renderer, l, &city));
        }
        // FPS_LAMPS_OFF (and undecorated mode): daytime with the interior lamps off (as in the empty-hall photos)
        let lights = if std::env::var("FPS_LAMPS_OFF").is_ok() || crate::bespoke::undecorated() { vec![] } else { crate::bespoke::all_lights(&city) };
        let probes = crate::bespoke::all_probes(&city);
        let gi_city = crate::gi::prisms(&city);
        let gi_tris = lib.as_ref().map_or(vec![], |l| crate::gi::tris(l, &city));
        let scan_batches = if crate::scan::mode() == crate::scan::Mode::Off { vec![] } else { lib.as_mut().map_or(vec![], |l| crate::scan::batches(&gpu, &renderer, l)) };
        Self { gpu, renderer, base, scene, city, lib, city_batches, prefab_batches: vec![], building_batches: vec![], lights, probes, gi_tris, gi_city, focus: None, scan_batches }
    }
    /// Re-uploads the city with this editor scene's objects.
    pub fn set_scene(&mut self, s: &editor::Scene) {
        self.scene = crate::world::scene(&self.gpu, &self.base, &self.city, Some(s));
        if let Some(lib) = self.lib.as_mut() {
            let mut pl = crate::world::object_placements(lib, &self.city, s);
            let area = s.bounds.unwrap_or_else(|| self.city.play_rect());
            pl.extend(crate::dressing::dress(lib, &self.city, area));
            let marks = crate::dressing::markings(lib, &self.city, area);
            self.prefab_batches = crate::library::prefab_batches(&self.gpu, &self.renderer, lib, &pl);
            for (id, tint, v, i) in marks {
                let mat = lib.material(&self.gpu, &self.renderer, &id);
                self.prefab_batches.extend(render::Batch::new(&self.gpu, &v, &i, &[render::Instance::new(glam::Mat4::IDENTITY, tint, 1.0)], mat));
            }
        }
    }
    /// Full prefab detail around `rect`; coarser away from it.
    pub fn set_focus(&mut self, rect: [f64; 4]) {
        if self.focus == Some(rect) {
            return;
        }
        if let Some(lib) = self.lib.as_mut() {
            let pl = crate::buildings::build(lib, &self.city, rect).0;
            self.building_batches = crate::library::prefab_batches(&self.gpu, &self.renderer, lib, &pl);
        }
        self.focus = Some(rect);
    }
    pub fn batches(&self) -> Vec<&render::Batch> {
        match crate::scan::mode() {
            crate::scan::Mode::Only => self.scan_batches.iter().collect(),
            _ => self.city_batches.iter().chain(&self.building_batches).chain(&self.prefab_batches).chain(&self.scan_batches).collect(),
        }
    }
}

/// Top-down render of `rect` at `mpp` metres per pixel.
pub fn ortho_rect(cx: &Ctx, rect: [f64; 4], mpp: f64) -> RgbaImage {
    let (w, h) = (((rect[2] - rect[0]) / mpp).round() as u32, ((rect[3] - rect[1]) / mpp).round() as u32);
    let cam = Camera::ortho([rect[0] as f32, rect[1] as f32, rect[2] as f32, rect[3] as f32], cx.scene.max.y);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &cx.batches(), &cam, w, h)
}

/// The editor review image for a scene: map with ids/spawns/sightlines next to the analysis heatmap.
pub fn scene_review(cx: &Ctx, a: &editor::Analysis, s: &editor::Scene, mpp: f64) -> RgbaImage {
    editor::draw::review(&ortho_rect(cx, a.rect, mpp), a, s)
}

fn wgpu_format() -> render::wgpu_reexport::TextureFormat {
    render::wgpu_reexport::TextureFormat::Rgba8UnormSrgb
}

/// Top-down render of the whole imported area at `mpp` metres per pixel.
pub fn ortho(cx: &Ctx, mpp: f64) -> RgbaImage {
    let r = cx.city.import_rect();
    let (w, h) = (((r[2] - r[0]) / mpp).round() as u32, ((r[3] - r[1]) / mpp).round() as u32);
    let cam = Camera::ortho([r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32], cx.scene.max.y);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &cx.batches(), &cam, w, h)
}

pub fn shot(cx: &Ctx, x: f32, z: f32, yaw: f32, pitch: f32, w: u32, h: u32) -> RgbaImage {
    let up = std::env::var("FPS_CAM_UP").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.7);
    // FPS_CAM_Y: the eye's absolute height (metres over home plate) instead of a height over the ground;
    // FPS_FOV: vertical field of view in degrees - both for rendering from a camera solved on a photo
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
    let y = env("FPS_CAM_Y").unwrap_or(cx.city.terrain.at(x as f64, z as f64) as f32 + up);
    let cam = Camera::perspective(glam::Vec3::new(x, y, z), yaw, pitch, env("FPS_FOV").unwrap_or(70.0), w as f32 / h as f32);
    let batches = cx.batches();
    // flood-fill GI around the camera (before the probe bake, so reflections see the bounced light too)
    if std::env::var("FPS_NO_GI").is_err() && !cx.gi_tris.is_empty() {
        let t0 = std::time::Instant::now();
        let terrain = |x: f32, z: f32| cx.city.terrain.at(x as f64, z as f64) as f32;
        let f = crate::gi::build(&cx.gi_tris, &cx.gi_city, terrain, &cx.lights, cx.renderer.sun_dir, cam.eye);
        if std::env::var("FPS_GI_DEBUG").is_ok() {
            f.dump(cam.eye.y + std::env::var("FPS_GI_SLICE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0), &crate::out_dir().join("gi_slice.png"));
        }
        cx.renderer.gi.upload(&cx.gpu, f.origin, crate::gi::CELL, &f.vox, &f.emit, true);
        let steps = std::env::var("FPS_GI_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(2000);
        cx.renderer.gi_iterate(&cx.gpu, steps);
        let _ = cx.gpu.device.poll(render::wgpu_reexport::PollType::wait_indefinitely());
        if std::env::var("FPS_GI_DEBUG").is_ok() {
            let (mut mn, mut mx) = (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN));
            for t in &cx.gi_tris {
                for p in t.p {
                    mn = mn.min(p);
                    mx = mx.max(p);
                }
            }
            eprintln!("gi: bespoke bounds {mn} .. {mx}");
        }
        eprintln!("gi: voxelized {} tris + {steps} steps in {:.2}s", cx.gi_tris.len(), t0.elapsed().as_secs_f32());
    } else {
        cx.renderer.gi.disable();
    }
    if std::env::var("FPS_GI_DEBUG").is_ok() {
        eprintln!("eye {} probes {:?}", cam.eye, cx.probes);
    }
    // bake the reflection probe of the room the camera is in (lights set for the capture point first)
    let mut metered = false;
    match cx.probes.iter().find(|p| cam.eye.cmpge(p[1]).all() && cam.eye.cmple(p[2]).all()) {
        Some(p) if std::env::var("FPS_NO_PROBE").is_err() => {
            cx.renderer.set_exposure(1.0);
            cx.renderer.set_lights(&cx.gpu, &cx.lights, p[0]);
            cx.renderer.bake_probe(&cx.gpu, &cx.scene, &batches, p[0], p[1], p[2]);
            cx.renderer.set_exposure(std::env::var("FPS_EXPOSURE").ok().and_then(|v| v.parse().ok()).unwrap_or(if cx.renderer.gi.enabled() { crate::INTERIOR_EXPOSURE } else { 2.2 }));
            cx.renderer.set_white_balance(if cx.lights.is_empty() { crate::INTERIOR_WHITE } else { crate::LAMPLIT_WHITE });
            metered = cx.renderer.gi.enabled();
        }
        _ => {
            cx.renderer.clear_probe();
            cx.renderer.set_exposure(1.0);
            cx.renderer.set_white_balance([1.0; 3]);
        }
    }
    cx.renderer.set_lights(&cx.gpu, &cx.lights, cam.eye);
    // interiors: meter the exposure like a camera (unless FPS_EXPOSURE pins it)
    if metered && std::env::var("FPS_EXPOSURE").is_err() {
        let (img, e) = render::render_image_metered(&cx.gpu, &cx.renderer, &cx.scene, &batches, &cam, w, h, if cx.lights.is_empty() { crate::INTERIOR_KEY } else { crate::LAMPLIT_KEY }, (1.0, 40.0));
        eprintln!("exposure {e:.1}");
        return img;
    }
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &batches, &cam, w, h)
}

/// Interior plan: GI around the rect's centre, a fixed bright exposure, cut away above `cut`.
pub fn plan(cx: &Ctx, rect: [f32; 4], cut: f32, mpp: f32) -> RgbaImage {
    let batches = cx.batches();
    let c = glam::Vec3::new((rect[0] + rect[2]) * 0.5, cut - 1.0, (rect[1] + rect[3]) * 0.5);
    if !cx.gi_tris.is_empty() {
        let terrain = |x: f32, z: f32| cx.city.terrain.at(x as f64, z as f64) as f32;
        let f = crate::gi::build(&cx.gi_tris, &cx.gi_city, terrain, &cx.lights, cx.renderer.sun_dir, c);
        cx.renderer.gi.upload(&cx.gpu, f.origin, crate::gi::CELL, &f.vox, &f.emit, true);
        cx.renderer.gi_iterate(&cx.gpu, 200);
    }
    cx.renderer.clear_probe();
    cx.renderer.set_exposure(std::env::var("FPS_EXPOSURE").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0));
    cx.renderer.set_lights(&cx.gpu, &cx.lights, c);
    let (w, h) = (((rect[2] - rect[0]) / mpp) as u32, ((rect[3] - rect[1]) / mpp) as u32);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &batches, &Camera::plan(rect, cut), w, h)
}

/// Grid every 100 m with coordinate labels, plus the play-area outline. `rect` is what the image covers.
pub fn annotate(img: &mut RgbaImage, rect: [f64; 4], play: [f64; 4]) {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let px = |x: f64| (x - rect[0]) / (rect[2] - rect[0]) * w;
    let pz = |z: f64| (z - rect[1]) / (rect[3] - rect[1]) * h;
    let grid = Rgba([255, 255, 255, 110]);
    let mut gx = (rect[0] / GRID_M).ceil() * GRID_M;
    while gx <= rect[2] {
        let x = px(gx) as i64;
        for y in 0..img.height() as i64 {
            blend(img, x, y, grid);
        }
        label(img, x + 3, 3, gx as i64);
        gx += GRID_M;
    }
    let mut gz = (rect[1] / GRID_M).ceil() * GRID_M;
    while gz <= rect[3] {
        let y = pz(gz) as i64;
        for x in 0..img.width() as i64 {
            blend(img, x, y, grid);
        }
        label(img, 3, y + 3, gz as i64);
        gz += GRID_M;
    }
    let yellow = Rgba([255, 214, 0, 255]);
    let (x0, x1, z0, z1) = (px(play[0]) as i64, px(play[2]) as i64, pz(play[1]) as i64, pz(play[3]) as i64);
    for t in 0..3 {
        for x in x0..=x1 {
            blend(img, x, z0 + t, yellow);
            blend(img, x, z1 - t, yellow);
        }
        for z in z0..=z1 {
            blend(img, x0 + t, z, yellow);
            blend(img, x1 - t, z, yellow);
        }
    }
}

/// Our render and the aerial photo side by side, both annotated with the same grid.
pub fn compare(cx: &Ctx, raw: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mpp = city::import::NAIP_MPP;
    let mut ours = ortho(cx, mpp);
    let mut naip = image::open(raw.join(city::import::naip_name(&cx.city.import_box)))?.to_rgba8();
    if naip.dimensions() != ours.dimensions() {
        naip = image::imageops::resize(&naip, ours.width(), ours.height(), image::imageops::FilterType::Triangle);
    }
    let (rect, play) = (cx.city.import_rect(), cx.city.play_rect());
    ours.save(out.join("ortho.png"))?;
    annotate(&mut ours, rect, play);
    annotate(&mut naip, rect, play);
    let gap = 16;
    let mut both = RgbaImage::from_pixel(ours.width() * 2 + gap, ours.height(), Rgba([20, 20, 20, 255]));
    image::imageops::overlay(&mut both, &ours, 0, 0);
    image::imageops::overlay(&mut both, &naip, (ours.width() + gap) as i64, 0);
    both.save(out.join("compare.png"))?;
    image::imageops::resize(&both, both.width() / 3, both.height() / 3, image::imageops::FilterType::Triangle).save(out.join("compare_small.png"))?;
    Ok(())
}

fn blend(img: &mut RgbaImage, x: i64, y: i64, c: Rgba<u8>) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let a = c[3] as u32;
    for i in 0..3 {
        p[i] = ((p[i] as u32 * (255 - a) + c[i] as u32 * a) / 255) as u8;
    }
}

const DIGITS: [[u8; 5]; 11] = [
    [7, 5, 5, 5, 7], [2, 6, 2, 2, 7], [7, 1, 7, 4, 7], [7, 1, 7, 1, 7], [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7], [7, 4, 7, 5, 7], [7, 1, 1, 1, 1], [7, 5, 7, 5, 7], [7, 5, 7, 1, 7], [0, 0, 7, 0, 0],
];

/// Draws an integer with a tiny 3x5 bitmap font, scaled 4x, white on a dark box.
fn label(img: &mut RgbaImage, x: i64, y: i64, v: i64) {
    let s = v.to_string();
    let sc = 4;
    let wpx = s.len() as i64 * 4 * sc;
    for yy in -2..5 * sc + 2 {
        for xx in -2..wpx {
            blend(img, x + xx, y + yy, Rgba([0, 0, 0, 150]));
        }
    }
    for (k, ch) in s.chars().enumerate() {
        let g = if ch == '-' { DIGITS[10] } else { DIGITS[ch.to_digit(10).unwrap_or(0) as usize] };
        for (row, bits) in g.iter().enumerate() {
            for col in 0..3 {
                if bits >> (2 - col) & 1 == 1 {
                    for dy in 0..sc {
                        for dx in 0..sc {
                            blend(img, x + k as i64 * 4 * sc + col * sc + dx, y + row as i64 * sc + dy, Rgba([255, 255, 255, 255]));
                        }
                    }
                }
            }
        }
    }
}

/// Contact sheet of every model in an asset group, laid out on a platform above the city, with index labels.
/// For vetting downloads: orientation, scale, materials. Returns the image and the (index, id, title) legend.
pub fn gallery(cx: &mut Ctx, group: &str, w: u32, h: u32, fit: bool) -> (RgbaImage, Vec<(usize, String, String)>) {
    let Some(lib) = cx.lib.as_mut() else { return (RgbaImage::new(1, 1), vec![]) };
    let assets: Vec<(String, String, f32)> = lib
        .catalog
        .assets
        .iter()
        .filter(|a| group.split(',').any(|g| a.group == g || a.id.starts_with(g)) && (matches!(a.kind, assets::AssetKind::Model) || std::env::var("FPS_SWATCHES").is_ok()))
        .map(|a| {
            let e = crate::library::extent(a);
            (a.id.clone(), a.title.clone(), if fit { 2.2 / e[0].max(e[1]).max(e[2]).max(1e-3) } else { 1.0 })
        })
        .collect();
    let (cols, cell, y0) = (8usize, 2.6f64, 500.0f64);
    let rows = assets.len().div_ceil(cols).max(1);
    let pos = |i: usize| ((i % cols) as f64 * cell, (i / cols) as f64 * cell);
    let pl: Vec<crate::library::Placement> = assets
        .iter()
        .enumerate()
        .map(|(i, (id, _, sc))| {
            let (x, z) = pos(i);
            crate::library::Placement { asset: id.clone(), part: crate::library::ALL_PARTS, x, z, y: y0, rot: 0.0, scale: [*sc; 3], tint: [1.0; 3], lod: 0, indoor: false }
        })
        .collect();
    let surfaces: Vec<(usize, String)> = assets.iter().enumerate().filter(|(_, (id, _, _))| lib.catalog.get(id).is_some_and(|a| matches!(a.kind, assets::AssetKind::Surface))).map(|(i, (id, _, _))| (i, id.clone())).collect();
    let mut batches = crate::library::prefab_batches(&cx.gpu, &cx.renderer, lib, &pl);
    // surfaces: a 2.2 m swatch each, at true texture scale, tilted toward the camera
    for (i, id) in surfaces {
        let (x, z) = pos(i);
        let tile = lib.catalog.get(&id).and_then(|a| a.tile_m).map_or(2.0, |t| t[0]);
        let m = lib.material(&cx.gpu, &cx.renderer, &id);
        let (x, z) = (x as f32, z as f32);
        let q = |dx: f32, dz: f32| render::TexVertex { pos: [x + dx, y0 as f32 + 0.01 + (1.1 - dz) * 0.5, z + dz], nrm: [0.0, 0.894, 0.447], uv: [dx + 1.1, dz + 1.1], tan: [1.0, 0.0, 0.0, 1.0], edge: [0.0; 4] };
        batches.extend(render::Batch::new(&cx.gpu, &[q(-1.1, -1.1), q(1.1, -1.1), q(1.1, 1.1), q(-1.1, 1.1)], &[0, 2, 1, 0, 3, 2], &[render::Instance::new(glam::Mat4::IDENTITY, [1.0; 3], 1.0 / tile)], m));
    }
    // grey platform with a 1 m grid line pattern via the plain material
    let (gw, gd) = (cols as f32 * cell as f32, rows as f32 * cell as f32);
    let q = |x: f32, z: f32| render::TexVertex { pos: [x - cell as f32 / 2.0, y0 as f32 - 0.01, z - cell as f32 / 2.0], nrm: [0.0, 1.0, 0.0], uv: [x, z], tan: [1.0, 0.0, 0.0, 1.0], edge: [0.0; 4] };
    let plain = std::sync::Arc::new(render::Material::plain(&cx.gpu, &cx.renderer, 200));
    batches.extend(render::Batch::new(&cx.gpu, &[q(0.0, 0.0), q(gw, 0.0), q(gw, gd), q(0.0, gd)], &[0, 2, 1, 0, 3, 2], &[render::Instance::new(glam::Mat4::IDENTITY, [0.5, 0.5, 0.5], 1.0)], plain));
    let centre = glam::Vec3::new(gw / 2.0 - cell as f32 / 2.0, y0 as f32, gd / 2.0);
    let top = std::env::var("FPS_GALLERY_TOP").is_ok();
    let eye = if top { centre + glam::Vec3::new(0.0, gd.max(gw * 0.6) + 4.0, 0.01) } else { centre + glam::Vec3::new(0.0, gd * 0.55 + 3.0, gd * 0.75 + 4.0) };
    let d = (centre - eye).normalize();
    let pitch = d.y.asin().to_degrees();
    let cam = Camera::perspective(eye, 0.0, pitch, 50.0, w as f32 / h as f32);
    let refs: Vec<&render::Batch> = batches.iter().collect();
    let mut img = render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &refs, &cam, w, h);
    let mut legend = vec![];
    for (i, (id, title, sc)) in assets.iter().enumerate() {
        let (x, z) = pos(i);
        let top = lib_height(cx, id) * sc;
        let p = cam.view_proj * glam::Vec4::new(x as f32, y0 as f32 + top + 0.15, z as f32, 1.0);
        if p.w > 0.0 {
            let (sx, sy) = ((p.x / p.w * 0.5 + 0.5) * w as f32, (0.5 - p.y / p.w * 0.5) * h as f32);
            editor::draw::text(&mut img, sx as i64 - 6, sy as i64, &i.to_string(), 2);
        }
        legend.push((i, id.clone(), title.clone()));
    }
    (img, legend)
}

fn lib_height(cx: &Ctx, id: &str) -> f32 {
    cx.lib.as_ref().and_then(|l| l.catalog.get(id)).map_or(0.5, |a| crate::library::extent(a)[1])
}
