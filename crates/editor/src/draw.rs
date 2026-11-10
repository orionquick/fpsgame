//! Review images: grid, labels, objects with ids, spawns, sightlines and analysis heatmaps over a top-down render.
use crate::scene::{Kind, Scene, Team};
use crate::validate::Analysis;
use image::{Rgba, RgbaImage};

pub const GRID_M: f64 = 50.0;

pub fn blend(img: &mut RgbaImage, x: i64, y: i64, c: Rgba<u8>) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let a = c[3] as u32;
    for i in 0..3 {
        p[i] = ((p[i] as u32 * (255 - a) + c[i] as u32 * a) / 255) as u8;
    }
}

fn glyph(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        '0' => [7, 5, 5, 5, 7], '1' => [2, 6, 2, 2, 7], '2' => [7, 1, 7, 4, 7], '3' => [7, 1, 7, 1, 7], '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 7, 1, 7], '6' => [7, 4, 7, 5, 7], '7' => [7, 1, 1, 1, 1], '8' => [7, 5, 7, 5, 7], '9' => [7, 5, 7, 1, 7],
        'A' => [2, 5, 7, 5, 5], 'B' => [6, 5, 6, 5, 6], 'C' => [3, 4, 4, 4, 3], 'D' => [6, 5, 5, 5, 6], 'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4], 'G' => [3, 4, 5, 5, 3], 'H' => [5, 5, 7, 5, 5], 'I' => [7, 2, 2, 2, 7], 'J' => [1, 1, 1, 5, 2],
        'K' => [5, 5, 6, 5, 5], 'L' => [4, 4, 4, 4, 7], 'M' => [5, 7, 7, 5, 5], 'N' => [6, 5, 5, 5, 5], 'O' => [2, 5, 5, 5, 2],
        'P' => [6, 5, 6, 4, 4], 'Q' => [2, 5, 5, 6, 3], 'R' => [6, 5, 6, 5, 5], 'S' => [3, 4, 2, 1, 6], 'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7], 'V' => [5, 5, 5, 5, 2], 'W' => [5, 5, 7, 7, 5], 'X' => [5, 5, 2, 5, 5], 'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7], '-' => [0, 0, 7, 0, 0], '.' => [0, 0, 0, 0, 2], ':' => [0, 2, 0, 2, 0], '%' => [5, 1, 2, 4, 5],
        '>' => [4, 2, 1, 2, 4], '<' => [1, 2, 4, 2, 1], '/' => [1, 1, 2, 4, 4], '#' => [5, 7, 5, 7, 5], '(' => [1, 2, 2, 2, 1],
        ')' => [4, 2, 2, 2, 4], '=' => [0, 7, 0, 7, 0], '+' => [0, 2, 7, 2, 0], ',' => [0, 0, 0, 2, 4],
        _ => [0; 5],
    }
}

/// Text in a 3x5 bitmap font at `scale` px per dot, white on a translucent dark box. Returns the width drawn.
pub fn text(img: &mut RgbaImage, x: i64, y: i64, s: &str, scale: i64) -> i64 {
    let wpx = s.chars().count() as i64 * 4 * scale;
    for yy in -scale..6 * scale {
        for xx in -scale..wpx {
            blend(img, x + xx, y + yy, Rgba([0, 0, 0, 160]));
        }
    }
    for (k, ch) in s.chars().enumerate() {
        for (row, bits) in glyph(ch).iter().enumerate() {
            for col in 0..3 {
                if bits >> (2 - col) & 1 == 1 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            blend(img, x + k as i64 * 4 * scale + col * scale + dx, y + row as i64 * scale + dy, Rgba([255, 255, 255, 255]));
                        }
                    }
                }
            }
        }
    }
    wpx
}

pub fn line(img: &mut RgbaImage, a: [f64; 2], b: [f64; 2], c: Rgba<u8>, thick: i64) {
    let n = ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()).ceil() as i64).max(1);
    for t in 0..=n {
        let f = t as f64 / n as f64;
        let (x, y) = ((a[0] + (b[0] - a[0]) * f) as i64, (a[1] + (b[1] - a[1]) * f) as i64);
        for dy in -thick / 2..=thick / 2 {
            for dx in -thick / 2..=thick / 2 {
                blend(img, x + dx, y + dy, c);
            }
        }
    }
}

pub fn disc(img: &mut RgbaImage, c: [f64; 2], r: f64, col: Rgba<u8>) {
    let ri = r.ceil() as i64;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f64 <= r * r {
                blend(img, c[0] as i64 + dx, c[1] as i64 + dy, col);
            }
        }
    }
}

/// Maps world x/z to pixels of an image covering `rect`.
pub struct Map {
    pub rect: [f64; 4],
    pub w: f64,
    pub h: f64,
}

impl Map {
    pub fn px(&self, p: [f64; 2]) -> [f64; 2] {
        [(p[0] - self.rect[0]) / (self.rect[2] - self.rect[0]) * self.w, (p[1] - self.rect[1]) / (self.rect[3] - self.rect[1]) * self.h]
    }
    pub fn ppm(&self) -> f64 {
        self.w / (self.rect[2] - self.rect[0])
    }
}

pub fn grid(img: &mut RgbaImage, m: &Map, step: f64) {
    let c = Rgba([255, 255, 255, 90]);
    let mut gx = (m.rect[0] / step).ceil() * step;
    while gx <= m.rect[2] {
        let x = m.px([gx, 0.0])[0];
        line(img, [x, 0.0], [x, m.h], c, 1);
        text(img, x as i64 + 3, 3, &format!("{}", gx as i64), 2);
        gx += step;
    }
    let mut gz = (m.rect[1] / step).ceil() * step;
    while gz <= m.rect[3] {
        let y = m.px([0.0, gz])[1];
        line(img, [0.0, y], [m.w, y], c, 1);
        text(img, 3, y as i64 + 3, &format!("{}", gz as i64), 2);
        gz += step;
    }
}

pub fn kind_color(k: Kind) -> Rgba<u8> {
    match k {
        Kind::Barrier => Rgba([255, 170, 40, 255]),
        Kind::Car => Rgba([80, 170, 255, 255]),
        Kind::Bus => Rgba([255, 230, 60, 255]),
        Kind::Container => Rgba([220, 60, 60, 255]),
        Kind::Dumpster => Rgba([60, 160, 90, 255]),
        Kind::Crate => Rgba([200, 150, 90, 255]),
        Kind::Fence => Rgba([240, 240, 240, 255]),
        Kind::Box => Rgba([200, 100, 255, 255]),
    }
}

fn team_color(t: Team) -> Rgba<u8> {
    match t {
        Team::A => Rgba([40, 120, 255, 255]),
        Team::B => Rgba([255, 60, 60, 255]),
    }
}

fn objects_and_spawns(img: &mut RgbaImage, m: &Map, s: &Scene, ids: bool) {
    for o in &s.objects {
        let c = o.corners().map(|p| m.px(p));
        for i in 0..4 {
            line(img, c[i], c[(i + 1) % 4], kind_color(o.kind), 3);
        }
        if ids {
            let p = m.px([o.x, o.z]);
            text(img, p[0] as i64 + 4, p[1] as i64 + 4, &o.id.to_string(), 2);
        }
    }
    for sp in &s.spawns {
        let p = m.px([sp.x, sp.z]);
        let r = 3.0 * m.ppm();
        disc(img, p, r + 2.0, Rgba([255, 255, 255, 255]));
        disc(img, p, r, team_color(sp.team));
        let d = [sp.yaw.to_radians().sin(), -sp.yaw.to_radians().cos()];
        line(img, p, [p[0] + d[0] * r * 2.5, p[1] + d[1] * r * 2.5], Rgba([255, 255, 255, 255]), 3);
        text(img, (p[0] + r + 4.0) as i64, (p[1] - r) as i64, if sp.team == Team::A { "A" } else { "B" }, 3);
    }
}

fn heat(v: f32) -> Rgba<u8> {
    // green (short) -> yellow (80 m) -> red (150 m+)
    let t = ((v - 20.0) / 130.0).clamp(0.0, 1.0);
    let (r, g) = if t < 0.46 { ((t / 0.46 * 255.0) as u8, 220) } else { (255, ((1.0 - (t - 0.46) / 0.54) * 220.0) as u8) };
    Rgba([r, g, 40, 150])
}

/// Two panels side by side: the map (objects with ids, spawns, longest sightlines) and the analysis
/// (sightline heatmap, unreachable space, cells without cover), plus a header with the key numbers.
pub fn review(beauty: &RgbaImage, a: &Analysis, s: &Scene) -> RgbaImage {
    let m = Map { rect: a.rect, w: beauty.width() as f64, h: beauty.height() as f64 };
    let mut left = beauty.clone();
    grid(&mut left, &m, GRID_M);
    for l in &a.report.longest_sightlines {
        let (p, q) = (m.px([l.from[0] as f64, l.from[1] as f64]), m.px([l.to[0] as f64, l.to[1] as f64]));
        line(&mut left, p, q, Rgba([255, 230, 0, 220]), 3);
        disc(&mut left, p, 5.0, Rgba([255, 230, 0, 255]));
        text(&mut left, p[0] as i64 + 6, p[1] as i64 - 14, &format!("{}M", l.length_m), 2);
    }
    objects_and_spawns(&mut left, &m, s, true);

    let mut right = beauty.clone();
    for p in right.pixels_mut() {
        let g = ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8 / 2;
        *p = Rgba([g, g, g, 255]);
    }
    let cell = m.ppm() * crate::validate::RES;
    for j in 0..a.h {
        for i in 0..a.w {
            let k = j * a.w + i;
            if !a.walk[k] {
                continue;
            }
            let col = if !a.reach[k] {
                Some(Rgba([255, 0, 255, 170]))
            } else if a.sight[k].is_finite() {
                Some(heat(a.sight[k]))
            } else {
                None
            };
            let Some(col) = col else { continue };
            // sampled every 2 cells: paint a 2x2 block
            let span = if a.sight[k].is_finite() { 2.0 } else { 1.0 };
            let p = m.px([a.rect[0] + i as f64 * crate::validate::RES, a.rect[1] + j as f64 * crate::validate::RES]);
            for dy in 0..(cell * span).ceil() as i64 {
                for dx in 0..(cell * span).ceil() as i64 {
                    blend(&mut right, p[0] as i64 + dx, p[1] as i64 + dy, col);
                }
            }
        }
    }
    grid(&mut right, &m, GRID_M);
    objects_and_spawns(&mut right, &m, s, false);

    let head = 70u32;
    let gap = 12u32;
    let mut out = RgbaImage::from_pixel(left.width() * 2 + gap, left.height() + head, Rgba([24, 24, 28, 255]));
    image::imageops::overlay(&mut out, &left, 0, head as i64);
    image::imageops::overlay(&mut out, &right, (left.width() + gap) as i64, head as i64);
    let r = &a.report;
    text(&mut out, 10, 10, &format!("{}  OBJECTS {}  SPAWNS {}  WALKABLE {} M2  REACHABLE {:.0}%  REGIONS {}  COVER {:.0}%  SIGHT>80M {:.0}%  SIGHT>150M {:.0}%",
        s.name.to_uppercase(), r.objects, s.spawns.len(), r.walkable_m2, r.reachable_pct, r.regions, r.cover_pct, r.sight_over_80m_pct, r.sight_over_150m_pct), 3);
    text(&mut out, 10, 40, "LEFT: OBJECTS (ID), SPAWNS A/B, LONGEST SIGHTLINES (YELLOW).  RIGHT: LONGEST VIEW PER SPOT GREEN<40M YELLOW 80M RED>150M, MAGENTA = UNREACHABLE", 2);
    out
}
