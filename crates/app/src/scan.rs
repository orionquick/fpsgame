//! A captured (photogrammetric) mesh of the ballpark's block, drawn as it was photographed: unlit, its own baked
//! light and shadow. The data is local only - `data/scan/*.bin` and its texture atlases in the asset inbox are not
//! part of the repository; without them nothing here does anything.
//!
//! `FPS_SCAN`: unset or `1` = draw the scan in place of the modelled city where it exists; `both` = draw both (to
//! check alignment); `0` = off.
use render::{Batch, Gpu, Instance, Renderer, TexVertex};

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Off,
    Only,
    Both,
}

fn file() -> std::path::PathBuf {
    crate::root().join("data/scan/block.bin")
}

pub fn mode() -> Mode {
    if cfg!(test) || !file().exists() {
        return Mode::Off;
    }
    match std::env::var("FPS_SCAN").as_deref() {
        Ok("0") => Mode::Off,
        Ok("both") => Mode::Both,
        _ => Mode::Only,
    }
}

/// One batch per texture atlas. Brightness `FPS_SCAN_GAIN` (default 0.8) scales the photographed colour.
pub fn batches(gpu: &Gpu, r: &Renderer, lib: &mut crate::library::Library) -> Vec<Batch> {
    let Ok(b) = std::fs::read(file()) else { return vec![] };
    if b.len() < 8 || &b[..4] != b"SCN1" {
        return vec![];
    }
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
    let gain = std::env::var("FPS_SCAN_GAIN").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.8);
    let mut o = 8;
    let mut parts = vec![];
    for _ in 0..u32_at(4) {
        let n = u32_at(o);
        let id = String::from_utf8_lossy(&b[o + 4..o + 4 + n]).to_string();
        o += 4 + n;
        let (nv, ni) = (u32_at(o), u32_at(o + 4));
        o += 8;
        let f = |k: usize| f32::from_le_bytes(b[k..k + 4].try_into().unwrap());
        let verts: Vec<TexVertex> = (0..nv).map(|i| { let q = o + i * 32; TexVertex { pos: [f(q), f(q + 4), f(q + 8)], nrm: [f(q + 12), f(q + 16), f(q + 20)], uv: [f(q + 24), f(q + 28)], tan: [1.0, 0.0, 0.0, 1.0], edge: [0.0; 4] } }).collect();
        o += nv * 32;
        let idx: Vec<u32> = (0..ni).map(|i| u32_at(o + i * 4) as u32).collect();
        o += ni * 4;
        parts.push((id, verts, idx));
    }
    let ids: Vec<String> = parts.iter().map(|p| p.0.clone()).collect();
    lib.preload(gpu, r, &ids);
    parts
        .into_iter()
        .filter_map(|(id, v, i)| {
            let mat = lib.material(gpu, r, &id);
            // params.x = 4: shown as photographed (no lighting of ours on top of the light baked into it)
            Batch::new(gpu, &v, &i, &[Instance::new(glam::Mat4::IDENTITY, [1.0; 3], 1.0).with_params([4.0, gain, 0.0, 0.0])], mat).map(|b| b.labelled(&id))
        })
        .collect()
}

/// Where one can stand on the scan: every upward-facing surface of the mesh, per grid cell, as height layers (a deck
/// and the ground under it are both floors). Written by the scan tooling beside the mesh.
pub struct Floors {
    x0: f32,
    z0: f32,
    cell: f32,
    w: usize,
    h: usize,
    off: Vec<u32>,
    hts: Vec<f32>,
}

impl Floors {
    pub fn load() -> Option<Self> {
        if mode() != Mode::Only {
            return None;
        }
        let b = std::fs::read(file().with_extension("floors")).ok()?;
        if b.len() < 24 || &b[..4] != b"SFL1" {
            return None;
        }
        let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let (w, h) = (u(16) as usize, u(20) as usize);
        let n = w * h + 1;
        let off: Vec<u32> = (0..n).map(|i| u(24 + i * 4)).collect();
        let base = 24 + n * 4;
        let hts: Vec<f32> = (0..(b.len() - base) / 4).map(|i| f(base + i * 4)).collect();
        Some(Self { x0: f(4), z0: f(8), cell: f(12), w, h, off, hts })
    }

    /// The highest floor at (x, z) that is no higher than `reach` (the feet plus a step); None: nothing to stand on.
    pub fn under(&self, x: f32, z: f32, reach: f32) -> Option<f32> {
        let (i, j) = (((x - self.x0) / self.cell).floor(), ((z - self.z0) / self.cell).floor());
        if i < 0.0 || j < 0.0 || i as usize >= self.w || j as usize >= self.h {
            return None;
        }
        let k = j as usize * self.w + i as usize;
        self.hts[self.off[k] as usize..self.off[k + 1] as usize].iter().copied().filter(|&y| y <= reach).fold(None, |m: Option<f32>, y| Some(m.map_or(y, |v| v.max(y))))
    }

    /// The ground to start on at (x, z): its lowest floor.
    pub fn lowest(&self, x: f32, z: f32) -> Option<f32> {
        self.under(x, z, f32::MAX).and_then(|_| {
            let (i, j) = (((x - self.x0) / self.cell).floor() as usize, ((z - self.z0) / self.cell).floor() as usize);
            let k = j * self.w + i;
            self.hts[self.off[k] as usize..self.off[k + 1] as usize].iter().copied().fold(None, |m: Option<f32>, y| Some(m.map_or(y, |v| v.min(y))))
        })
    }
}
