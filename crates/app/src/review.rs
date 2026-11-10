//! Headless review renders: top-down ortho, side-by-side with the aerial photo, and eye-height shots.
use city::City;
use image::{Rgba, RgbaImage};
use render::{Camera, Gpu, Renderer};
use std::path::Path;

pub const GRID_M: f64 = 100.0;

pub struct Ctx {
    pub gpu: Gpu,
    pub renderer: Renderer,
    pub scene: render::Scene,
    pub city: City,
}

impl Ctx {
    pub fn new(city: City) -> Self {
        let gpu = Gpu::headless();
        let renderer = Renderer::new(&gpu, wgpu_format());
        let scene = crate::world::scene(&gpu, &city);
        Self { gpu, renderer, scene, city }
    }
}

fn wgpu_format() -> render::wgpu_reexport::TextureFormat {
    render::wgpu_reexport::TextureFormat::Rgba8UnormSrgb
}

/// Top-down render of the whole imported area at `mpp` metres per pixel.
pub fn ortho(cx: &Ctx, mpp: f64) -> RgbaImage {
    let r = cx.city.import_rect();
    let (w, h) = (((r[2] - r[0]) / mpp).round() as u32, ((r[3] - r[1]) / mpp).round() as u32);
    let cam = Camera::ortho([r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32], cx.scene.max.y);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &cam, w, h)
}

pub fn shot(cx: &Ctx, x: f32, z: f32, yaw: f32, pitch: f32, w: u32, h: u32) -> RgbaImage {
    let y = cx.city.terrain.at(x as f64, z as f64) as f32 + 1.7;
    let cam = Camera::perspective(glam::Vec3::new(x, y, z), yaw, pitch, 70.0, w as f32 / h as f32);
    render::render_image(&cx.gpu, &cx.renderer, &cx.scene, &cam, w, h)
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
    let mut naip = image::open(raw.join("naip.png"))?.to_rgba8();
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
