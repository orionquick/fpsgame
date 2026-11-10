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
    focus: Option<[f64; 4]>,
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
        Self { gpu, renderer, base, scene, city, lib, city_batches, prefab_batches: vec![], building_batches: vec![], focus: None }
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
            for (id, v, i) in marks {
                let mat = lib.material(&self.gpu, &self.renderer, &id);
                let yellow = lib.catalog.get(&id).is_some_and(|a| a.title.contains("Yellow"));
                let tint = if yellow { [1.15, 1.05, 0.55] } else { [1.0, 1.0, 1.0] };
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
        self.city_batches.iter().chain(&self.building_batches).chain(&self.prefab_batches).collect()
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
    let y = cx.city.terrain.at(x as f64, z as f64) as f32 + up;
    let cam = Camera::perspective(glam::Vec3::new(x, y, z), yaw, pitch, 70.0, w as f32 / h as f32);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &cx.batches(), &cam, w, h)
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
        .filter(|a| (a.group == group || a.id.starts_with(group)) && matches!(a.kind, assets::AssetKind::Model))
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
    let mut batches = crate::library::prefab_batches(&cx.gpu, &cx.renderer, lib, &pl);
    // grey platform with a 1 m grid line pattern via the plain material
    let (gw, gd) = (cols as f32 * cell as f32, rows as f32 * cell as f32);
    let q = |x: f32, z: f32| render::TexVertex { pos: [x - cell as f32 / 2.0, y0 as f32 - 0.01, z - cell as f32 / 2.0], nrm: [0.0, 1.0, 0.0], uv: [x, z], tan: [1.0, 0.0, 0.0, 1.0] };
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
