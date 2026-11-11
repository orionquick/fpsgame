//! Procedural PBR surfaces for things no texture library had close enough to the reference photos (glazed subway
//! tile, cowhide, basket weave), and SVG stencil decals. Each is written as a surface zip in the inbox
//! (`_BaseColor`, `_Normal`, `_Roughness`, `_Opacity` + physical size) for `fps assets`.

use std::io::Write;

fn png_rgb(img: &image::RgbImage) -> Vec<u8> {
    let mut c = std::io::Cursor::new(vec![]);
    img.write_to(&mut c, image::ImageFormat::Png).unwrap();
    c.into_inner()
}

fn png_gray(img: &image::GrayImage) -> Vec<u8> {
    let mut c = std::io::Cursor::new(vec![]);
    img.write_to(&mut c, image::ImageFormat::Png).unwrap();
    c.into_inner()
}

/// Writes `data/megascans_inbox/<group>/<id>.zip` with the given maps (suffix, PNG bytes).
pub fn write_surface(group: &str, id: &str, maps: &[(&str, Vec<u8>)], size_m: (f32, f32), source: &str) -> std::io::Result<()> {
    let dir = crate::root().join("data/megascans_inbox").join(group);
    std::fs::create_dir_all(&dir)?;
    let mut z = zip::ZipWriter::new(std::fs::File::create(dir.join(format!("{id}.zip")))?);
    let o = zip::write::SimpleFileOptions::default();
    for (suffix, bytes) in maps {
        z.start_file(format!("{id}_{suffix}.png"), o)?;
        z.write_all(bytes)?;
    }
    z.start_file(format!("{id}.json"), o)?;
    let meta = serde_json::json!({"id": id, "name": id, "tags": [group], "maps": [{"physicalSize": format!("{:.2}x{:.2}", size_m.0, size_m.1)}], "source": source});
    z.write_all(meta.to_string().as_bytes())?;
    z.finish()?;
    Ok(())
}

/// Tangent-space normal map (OpenGL convention) from a tiling height field; `strength` scales the slopes.
fn normals(h: &[f32], w: usize, hh: usize, strength: f32) -> image::RgbImage {
    let at = |x: isize, y: isize| h[(y.rem_euclid(hh as isize) as usize) * w + x.rem_euclid(w as isize) as usize];
    image::RgbImage::from_fn(w as u32, hh as u32, |x, y| {
        let (x, y) = (x as isize, y as isize);
        let dx = (at(x + 1, y) - at(x - 1, y)) * strength;
        let dy = (at(x, y + 1) - at(x, y - 1)) * strength;
        let n = glam::Vec3::new(-dx, dy, 1.0).normalize();
        image::Rgb([((n.x * 0.5 + 0.5) * 255.0) as u8, ((n.y * 0.5 + 0.5) * 255.0) as u8, ((n.z * 0.5 + 0.5) * 255.0) as u8])
    })
}

fn hash(x: i64, y: i64, s: u64) -> f32 {
    let mut v = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ s;
    v ^= v >> 31;
    v = v.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v ^= v >> 29;
    (v >> 40) as f32 / (1u64 << 24) as f32
}

/// Tiling value noise with period `p` cells.
fn vnoise(x: f32, y: f32, p: i64, s: u64) -> f32 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let h = |a: i64, b: i64| hash(a.rem_euclid(p), b.rem_euclid(p), s);
    let a = h(xi, yi) + (h(xi + 1, yi) - h(xi, yi)) * ux;
    let b = h(xi, yi + 1) + (h(xi + 1, yi + 1) - h(xi, yi + 1)) * ux;
    a + (b - a) * uy
}

/// Glazed white subway tile, 75 x 150 mm in running bond with 3 mm grey grout (0.6 m square repeat).
fn subway() -> std::io::Result<()> {
    let n = 1024usize;
    let mpp = 0.6 / n as f32;
    let (tw, th, grout, bevel) = (0.15, 0.075, 0.003, 0.0025);
    let mut h = vec![0f32; n * n];
    let mut base = image::RgbImage::new(n as u32, n as u32);
    let mut rough = image::GrayImage::new(n as u32, n as u32);
    for y in 0..n {
        for x in 0..n {
            let (px, py) = (x as f32 * mpp, y as f32 * mpp);
            let row = (py / th).floor();
            let sx = px + if row as i64 % 2 == 1 { tw / 2.0 } else { 0.0 };
            let col = (sx / tw).floor();
            let (lx, ly) = (sx - col * tw, py - row * th);
            // distance into the tile from its nearest edge (grout centred on the joints)
            let d = (lx.min(tw - lx).min(ly).min(th - ly)) - grout / 2.0;
            let tile = d > 0.0;
            let t = (d / bevel).clamp(0.0, 1.0);
            let height = if tile { (t * std::f32::consts::FRAC_PI_2).sin() } else { 0.0 };
            h[y * n + x] = height * 0.6;
            let v = hash(col as i64, row as i64, 7) * 0.05 - 0.025;
            let c = if tile { [0.93 + v, 0.925 + v, 0.905 + v] } else { [0.58, 0.57, 0.55] };
            base.put_pixel(x as u32, y as u32, image::Rgb(c.map(|c: f32| (c.clamp(0.0, 1.0) * 255.0) as u8)));
            rough.put_pixel(x as u32, y as u32, image::Luma([if tile { 28 + (hash(x as i64, y as i64, 3) * 10.0) as u8 } else { 225 }]));
        }
    }
    let nm = normals(&h, n, n, 6.0);
    write_surface("procedural", "pt_subway_tile", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nm)), ("Roughness", png_gray(&rough))], (0.6, 0.6), "fps proctex")
}

/// Brown and white cowhide (1.2 m square repeat): big irregular patches, fine hair.
fn cowhide() -> std::io::Result<()> {
    let n = 1024usize;
    let mut h = vec![0f32; n * n];
    let mut base = image::RgbImage::new(n as u32, n as u32);
    let mut rough = image::GrayImage::new(n as u32, n as u32);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let f = vnoise(u * 3.0, v * 3.0, 3, 1) * 0.55 + vnoise(u * 7.0, v * 7.0, 7, 2) * 0.3 + vnoise(u * 19.0, v * 19.0, 19, 3) * 0.15;
            let brown = ((f - 0.5) / 0.035).clamp(0.0, 1.0);
            let hair = vnoise(u * 400.0, v * 60.0, 400, 4);
            let shade = 0.88 + 0.12 * hair;
            let w = [0.86, 0.82, 0.76];
            let b = [0.36, 0.2, 0.11];
            let c: Vec<f32> = (0..3).map(|k| (w[k] + (b[k] - w[k]) * brown) * shade).collect();
            base.put_pixel(x as u32, y as u32, image::Rgb([(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8]));
            h[y * n + x] = hair * 0.15;
            rough.put_pixel(x as u32, y as u32, image::Luma([180 + (hair * 40.0) as u8]));
        }
    }
    let nm = normals(&h, n, n, 4.0);
    write_surface("procedural", "pt_cowhide", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nm)), ("Roughness", png_gray(&rough))], (1.2, 1.2), "fps proctex")
}

/// Basket weave of flat strands (0.24 m repeat): cream jute for poufs, tinted to cognac for the bar front.
fn weave() -> std::io::Result<()> {
    let n = 1024usize;
    let cells = 8.0; // strands per repeat each way
    let mut h = vec![0f32; n * n];
    let mut base = image::RgbImage::new(n as u32, n as u32);
    let mut rough = image::GrayImage::new(n as u32, n as u32);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32 * cells, y as f32 / n as f32 * cells);
            let (cx, cy) = (u.floor() as i64, v.floor() as i64);
            let (fx, fy) = (u.fract(), v.fract());
            // over/under: horizontal strand on top in checkerboard cells
            let horiz = (cx + cy) % 2 == 0;
            let across = if horiz { fy } else { fx };
            let along = if horiz { fx } else { fy };
            let edge = (across.min(1.0 - across) / 0.12).clamp(0.0, 1.0);
            let bulge = (along * std::f32::consts::PI).sin();
            let height = edge.sqrt() * (0.55 + 0.45 * bulge);
            h[y * n + x] = height;
            let fibre = vnoise(if horiz { u * 30.0 } else { u * 3.0 }, if horiz { v * 3.0 } else { v * 30.0 }, 240, 5);
            let k = (0.62 + 0.38 * height) * (0.9 + 0.1 * fibre);
            base.put_pixel(x as u32, y as u32, image::Rgb([(0.84 * k * 255.0) as u8, (0.78 * k * 255.0) as u8, (0.66 * k * 255.0) as u8]));
            rough.put_pixel(x as u32, y as u32, image::Luma([210]));
        }
    }
    let nm = normals(&h, n, n, 3.0);
    write_surface("procedural", "pt_basket_weave", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nm)), ("Roughness", png_gray(&rough))], (0.24, 0.24), "fps proctex")
}

/// Cognac leather, puffed between stitched seams: `diamond` quilting (seams on the diagonals, 0.24 m repeat with
/// 0.12 m diamonds) or square biscuit tufting with a button at each corner.
fn leather(id: &str, diamond: bool) -> std::io::Result<()> {
    let n = 1024usize;
    let cells = 2.0;
    let mut h = vec![0f32; n * n];
    let mut base = image::RgbImage::new(n as u32, n as u32);
    let mut rough = image::GrayImage::new(n as u32, n as u32);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32 * cells, y as f32 / n as f32 * cells);
            let (a, b) = if diamond { (u + v, u - v) } else { (u * 2.0, v * 2.0) };
            let (fa, fb) = (a - a.floor(), b - b.floor());
            // distance to the nearest seam (0 on it, 0.5 at the puff's centre)
            let d = fa.min(1.0 - fa).min(fb.min(1.0 - fb));
            let puff = (d * 2.0).min(1.0).powf(0.6);
            let seam = (d / 0.018).min(1.0);
            let button = if diamond { 1.0 } else { (((fa - 0.0).powi(2).min((1.0 - fa).powi(2)) + (fb).powi(2).min((1.0 - fb).powi(2))).sqrt() / 0.05).min(1.0) };
            h[y * n + x] = puff * seam * button;
            let grain = vnoise(u * 90.0, v * 90.0, 180, 9);
            let k = (0.78 + 0.22 * puff) * (0.92 + 0.08 * grain) * (0.55 + 0.45 * seam) * (0.6 + 0.4 * button);
            // light cognac (sRGB ~ 190, 125, 80 on the puffs, as in the photo)
            base.put_pixel(x as u32, y as u32, image::Rgb([(0.74 * k * 255.0).min(255.0) as u8, (0.54 * k * 255.0) as u8, (0.38 * k * 255.0) as u8]));
            rough.put_pixel(x as u32, y as u32, image::Luma([(95.0 + 70.0 * (1.0 - puff) + grain * 20.0) as u8]));
        }
    }
    let nm = normals(&h, n, n, 5.0);
    write_surface("procedural", id, &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nm)), ("Roughness", png_gray(&rough))], (0.24, 0.24), "fps proctex")
}

/// Flexible duct jacket (as in the bar photo): fine ribs running round the duct with a slight spiral, crests
/// catching the light, dusty mauve-brown. Texture x runs along the duct (0.5 m repeat, 20 ribs), y round it.
fn flex_duct() -> std::io::Result<()> {
    let n = 1024usize;
    let ribs = 20.0;
    let mut h = vec![0f32; n * n];
    let mut base = image::RgbImage::new(n as u32, n as u32);
    let mut rough = image::GrayImage::new(n as u32, n as u32);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            // one rib per 1/20 along the duct; a whole-rib spiral pitch round the circumference keeps it tiling
            let t = (u * ribs + v).fract();
            // rounded crest, narrow dark valley (the wire helix)
            let crest = (t * std::f32::consts::PI).sin().powf(0.6);
            h[y * n + x] = crest;
            let dust = vnoise(u * 12.0, v * 12.0, 12, 11) * 0.6 + vnoise(u * 50.0, v * 50.0, 50, 12) * 0.4;
            let k = (0.55 + 0.45 * crest) * (0.88 + 0.16 * dust);
            base.put_pixel(x as u32, y as u32, image::Rgb([(0.6 * k * 255.0) as u8, (0.44 * k * 255.0) as u8, (0.4 * k * 255.0) as u8]));
            rough.put_pixel(x as u32, y as u32, image::Luma([(150.0 - 50.0 * crest) as u8]));
        }
    }
    let nm = normals(&h, n, n, 7.0);
    write_surface("procedural", "pt_flex_duct", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nm)), ("Roughness", png_gray(&rough))], (0.5, 0.5), "fps proctex")
}

/// Sawn sandstone cladding as on the ballpark's towers: Poly Haven's "Sandstone Blocks 08" scan (CC0, Rob Tuytel;
/// sawn panels in 0.6 m courses, 3 m repeat) with its own grain, normals and roughness, each panel retinted to one of
/// the tones measured off the reference photos (gold, rose, ochre, buff) as the real cladding varies panel to panel.
/// The panels are found from the scan's height map (joints are its low lines).
fn ashlar() -> std::io::Result<()> {
    let dir = crate::root().join("data/megascans_inbox/polyhaven_src/sandstone_blocks_08");
    let load = |m: &str| image::open(dir.join(format!("sandstone_blocks_08_{m}_2k.jpg"))).map_err(|e| std::io::Error::other(e.to_string()));
    let diff = load("diff")?.to_rgb8();
    let disp = load("disp")?.to_luma8();
    let nor = load("nor_gl")?.to_rgb8();
    let rough = load("rough")?.to_luma8();
    let (w, h) = (diff.width() as usize, diff.height() as usize);
    // joints: pixels well below the local maximum (panels sit at different heights, so no global threshold works)
    let d: Vec<f32> = disp.pixels().map(|p| p[0] as f32).collect();
    let r = 5isize;
    let maxf = |src: &[f32], horiz: bool| -> Vec<f32> {
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as isize, (i / w) as isize);
                (-r..=r)
                    .map(|k| {
                        let (nx, ny) = if horiz { ((x + k).rem_euclid(w as isize), y) } else { (x, (y + k).rem_euclid(h as isize)) };
                        src[ny as usize * w + nx as usize]
                    })
                    .fold(f32::MIN, f32::max)
            })
            .collect()
    };
    let local_max = maxf(&maxf(&d, true), false);
    // ...or where the scan's normals tilt hard (the joints' edges); then grow by a pixel to close hairline gaps
    let tilt = |i: usize| {
        let p = nor.get_pixel((i % w) as u32, (i / w) as u32);
        let (nx, ny) = (p[0] as f32 / 127.5 - 1.0, p[1] as f32 / 127.5 - 1.0);
        (nx * nx + ny * ny).sqrt()
    };
    let raw: Vec<bool> = (0..w * h).map(|i| local_max[i] - d[i] > 5.0 || tilt(i) > 0.3).collect();
    let joint: Vec<bool> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            (-1..=1).any(|dy| (-1..=1).any(|dx| raw[((y + dy).rem_euclid(h as isize) as usize) * w + (x + dx).rem_euclid(w as isize) as usize]))
        })
        .collect();
    if std::env::var("FPS_ASHLAR_MASK").is_ok() {
        let m = image::GrayImage::from_fn(w as u32, h as u32, |x, y| image::Luma([if joint[y as usize * w + x as usize] { 0 } else { 255 }]));
        m.save(crate::root().join("out/ashlar_mask.png")).ok();
    }
    // the panels: courses between the horizontal joint lines, then panels between the vertical joints in each
    // course (a joint column is mostly joint pixels over the course's height)
    let row_frac: Vec<f32> = (0..h).map(|y| (0..w).filter(|&x| joint[y * w + x]).count() as f32 / w as f32).collect();
    let bands = |frac: &[f32], thr: f32| -> Vec<(usize, usize)> {
        // runs of non-joint positions, wrapping (a run crossing the edge is reported with end > len)
        let n = frac.len();
        let start = (0..n).find(|&i| frac[i] > thr).unwrap_or(0);
        let mut out = vec![];
        let mut run: Option<usize> = None;
        for k in 0..=n {
            let i = (start + k) % n;
            let j = frac[i] > thr || k == n;
            match (run, j) {
                (None, false) => run = Some(start + k),
                (Some(r), true) => {
                    out.push((r, start + k));
                    run = None;
                }
                _ => {}
            }
        }
        out.into_iter().filter(|(a, b)| b - a > 8).collect()
    };
    let courses = bands(&row_frac, 0.45);
    let mut label = vec![u32::MAX; w * h];
    let mut sizes: Vec<usize> = vec![];
    let mut rects: Vec<(usize, usize, usize, usize)> = vec![];
    for &(y0, y1) in &courses {
        let inner = (y0 + 4, y1.saturating_sub(4).max(y0 + 5));
        let col_frac: Vec<f32> = (0..w).map(|x| (inner.0..inner.1).filter(|&y| joint[(y % h) * w + x]).count() as f32 / (inner.1 - inner.0) as f32).collect();
        for (x0, x1) in bands(&col_frac, 0.55) {
            let id = sizes.len() as u32;
            rects.push((x0, y0, x1, y1));
            let mut n = 0;
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy % h) * w + (xx % w);
                    if !joint[i] {
                        label[i] = id;
                        n += 1;
                    }
                }
            }
            sizes.push(n);
        }
    }
    // source mean colour (linear), and the target tones (sRGB, from the photos)
    let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
    let mut mean = [0f32; 3];
    for p in diff.pixels() {
        for c in 0..3 {
            mean[c] += lin(p[c]);
        }
    }
    mean = mean.map(|m| m / (w * h) as f32);
    let pal: [[f32; 3]; 5] = [[196.0, 152.0, 100.0], [190.0, 136.0, 104.0], [182.0, 140.0, 90.0], [206.0, 176.0, 132.0], [200.0, 150.0, 108.0]];
    let gains: Vec<[f32; 3]> = (0..sizes.len())
        .map(|k| {
            if sizes[k] < 3000 {
                return [1.0; 3];
            }
            let t = hash(k as i64, 3, 91);
            let (a, b) = (pal[(t * 5.0) as usize % 5], pal[(hash(k as i64, 4, 92) * 5.0) as usize % 5]);
            let mix = hash(k as i64, 5, 93) * 0.4;
            let bright = 0.93 + hash(k as i64, 6, 94) * 0.14;
            let mut g = [0f32; 3];
            for c in 0..3 {
                let target = (a[c] * (1.0 - mix) + b[c] * mix) / 255.0;
                g[c] = target.powf(2.2) / mean[c] * bright;
            }
            // the photos read softer than the raw tones: pull a third of the way to a grey gain
            let l = (g[0] + g[1] + g[2]) / 3.0;
            g.map(|v| v + (l - v) * 0.35)
        })
        .collect();
    // joints and tiny chips take the average gain so they sit between their neighbours
    let avg: [f32; 3] = {
        let mut s = [0f32; 3];
        for g in &gains {
            for c in 0..3 {
                s[c] += g[c];
            }
        }
        s.map(|v| v / gains.len().max(1) as f32)
    };
    let to8 = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    let base = image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
        let i = y as usize * w + x as usize;
        let g = if label[i] == u32::MAX { avg } else { gains[label[i] as usize] };
        let p = diff.get_pixel(x, y);
        image::Rgb([to8(lin(p[0]) * g[0]), to8(lin(p[1]) * g[1]), to8(lin(p[2]) * g[2])])
    });
    let rough_g = image::GrayImage::from_fn(w as u32, h as u32, |x, y| image::Luma([rough.get_pixel(x, y)[0]]));
    println!("ashlar: {} panels from the scan", sizes.iter().filter(|&&n| n >= 3000).count());
    write_surface("procedural", "pt_ashlar_sandstone", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nor)), ("Roughness", png_gray(&rough_g))], (3.0, 3.0), "Poly Haven sandstone_blocks_08 (CC0), retinted by fps proctex")?;

    // the colour-neutral version for panel-by-panel cladding: every panel brought to one sandstone mean (its own
    // grain, veins and chips kept, its colour cast removed) so a per-panel tint sets its tone; joints a pale
    // mortar. Plus the panels' windows (metres in the 3 m tile, with half a joint each side) for the cladder.
    let target = [184.0f32, 148.0, 114.0].map(|c| (c / 255.0).powf(2.2));
    let mortar = [178.0f32, 170.0, 156.0].map(|c| (c / 255.0).powf(2.2));
    let n = sizes.len();
    let mut pm = vec![[0f64; 3]; n];
    let mut jm = [0f64; 3];
    let mut jn = 0f64;
    for (i, p) in diff.pixels().enumerate() {
        let l = [lin(p[0]) as f64, lin(p[1]) as f64, lin(p[2]) as f64];
        if label[i] == u32::MAX {
            for c in 0..3 {
                jm[c] += l[c];
            }
            jn += 1.0;
        } else {
            for c in 0..3 {
                pm[label[i] as usize][c] += l[c];
            }
        }
    }
    let neutral = image::RgbImage::from_fn(w as u32, h as u32, |x, y| {
        let i = y as usize * w + x as usize;
        let p = diff.get_pixel(x, y);
        let g: [f32; 3] = if label[i] == u32::MAX {
            [0, 1, 2].map(|c| mortar[c] / (jm[c] / jn.max(1.0)) as f32)
        } else {
            let k = label[i] as usize;
            [0, 1, 2].map(|c| target[c] / (pm[k][c] / sizes[k].max(1) as f64) as f32)
        };
        image::Rgb([to8(lin(p[0]) * g[0]), to8(lin(p[1]) * g[1]), to8(lin(p[2]) * g[2])])
    });
    write_surface("procedural", "pt_ashlar_neutral", &[("BaseColor", png_rgb(&neutral)), ("Normal", png_rgb(&nor)), ("Roughness", png_gray(&rough_g))], (3.0, 3.0), "Poly Haven sandstone_blocks_08 (CC0), panels neutralised by fps proctex")?;
    let m_per_px = 3.0 / w as f32;
    let pad = 5.0;
    let windows: Vec<[f32; 4]> = rects
        .iter()
        .zip(&sizes)
        .filter(|(_, n)| **n >= 3000)
        .map(|(&(x0, y0, x1, y1), _)| [(x0 as f32 - pad) * m_per_px, (y0 as f32 - pad) * m_per_px, (x1 as f32 + pad) * m_per_px, (y1 as f32 + pad) * m_per_px])
        .collect();
    let path = crate::root().join("data/megascans_inbox/procedural/pt_ashlar_panels.json");
    std::fs::write(&path, serde_json::to_string(&windows).unwrap())?;
    println!("ashlar: {} panel windows -> {}", windows.len(), path.display());
    Ok(())
}

/// Split-face sandstone for stone-by-stone cladding: Poly Haven's "Sandstone Cracks" (CC0, a 2 m cleft sandstone
/// face) with its colour brought to a neutral mid grey mean - its grain, veins and stains kept - so each stone's
/// tint sets its tone; the scan's normals and roughness unchanged.
fn cleft() -> std::io::Result<()> {
    let dir = crate::root().join("data/megascans_inbox/polyhaven_src/sandstone_cracks");
    let load = |m: &str| image::open(dir.join(format!("sandstone_cracks_{m}_2k.jpg"))).map_err(|e| std::io::Error::other(e.to_string()));
    let diff = load("diff")?.to_rgb8();
    let nor = load("nor_gl")?.to_rgb8();
    let rough = load("rough")?.to_luma8();
    let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
    let mut mean = [0f64; 3];
    for p in diff.pixels() {
        for c in 0..3 {
            mean[c] += lin(p[c]) as f64;
        }
    }
    let n = (diff.width() * diff.height()) as f64;
    let gain = [0, 1, 2].map(|c| 0.5 / (mean[c] / n) as f32);
    let to8 = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    let base = image::RgbImage::from_fn(diff.width(), diff.height(), |x, y| {
        let p = diff.get_pixel(x, y);
        image::Rgb([to8(lin(p[0]) * gain[0]), to8(lin(p[1]) * gain[1]), to8(lin(p[2]) * gain[2])])
    });
    let rough_g = image::GrayImage::from_fn(rough.width(), rough.height(), |x, y| image::Luma([rough.get_pixel(x, y)[0]]));
    write_surface("procedural", "pt_cleft_sandstone", &[("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nor)), ("Roughness", png_gray(&rough_g))], (2.0, 2.0), "Poly Haven sandstone_cracks (CC0), colour neutralised by fps proctex")?;
    println!("cleft sandstone: neutral 2 m face");
    Ok(())
}

/// Lettering for signs, set in real typefaces: each sign is a cutout surface (white, tinted where it is used) whose
/// text fills a box of the size measured on the building. Fonts (OFL, in data/megascans_inbox/fonts): Graduate for
/// the athletic slab lettering and numerals, Montserrat SemiBold for the plain sans. `contain`: keep the glyphs'
/// proportions and centre them in the box (single numerals) instead of filling it.
fn signs() -> std::io::Result<()> {
    use resvg::{tiny_skia, usvg};
    let dir = crate::root().join("data/megascans_inbox/fonts");
    let mut opt = usvg::Options::default();
    for f in ["Graduate-Regular.ttf", "Montserrat-SemiBold.ttf"] {
        opt.fontdb_mut().load_font_file(dir.join(f))?;
    }
    let err = |e: String| std::io::Error::other(e);
    // (id, text, family, extra stroke as a fraction of the font size (bolder), box metres, contain)
    let mut list: Vec<(String, String, &str, f32, (f32, f32), bool)> = vec![
        ("sgn_tickets".into(), "TICKETS".into(), "Montserrat", 0.0, (1.73, 0.33), false),
        ("sgn_ballpark".into(), "BALLPARK".into(), "Graduate", 0.05, (3.4, 0.72), false),
        ("sgn_will_call".into(), "WILL CALL".into(), "Montserrat", 0.0, (1.5, 0.2), false),
        ("sgn_atm".into(), "ATM".into(), "Montserrat", 0.02, (0.62, 0.26), false),
    ];
    for n in 1..=9 {
        list.push((format!("sgn_n{n}"), n.to_string(), "Graduate", 0.04, (0.24, 0.26), true));
    }
    for (id, text, family, stroke, (wm, hm), contain) in list {
        let fs = 200.0f32;
        let (cw, ch) = ((text.len() as f32 * fs * 1.2 + 200.0) as u32, (fs * 2.0) as u32);
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{cw}" height="{ch}"><text x="100" y="{y}" font-family="{family}" font-weight="{wt}" font-size="{fs}" fill="#fff" stroke="#fff" stroke-width="{sw}" stroke-linejoin="miter">{text}</text></svg>"##,
            y = fs * 1.4,
            wt = if family == "Montserrat" { 600 } else { 400 },
            sw = fs * stroke
        );
        let tree = usvg::Tree::from_str(&svg, &opt).map_err(|e| err(e.to_string()))?;
        let mut pix = tiny_skia::Pixmap::new(cw, ch).ok_or_else(|| err("bad size".into()))?;
        resvg::render(&tree, tiny_skia::Transform::identity(), &mut pix.as_mut());
        let a = image::GrayImage::from_fn(cw, ch, |x, y| image::Luma([pix.pixel(x, y).map_or(0, |p| p.alpha())]));
        // the ink's bounding box
        let (mut x0, mut y0, mut x1, mut y1) = (cw, ch, 0u32, 0u32);
        for (x, y, p) in a.enumerate_pixels() {
            if p[0] > 24 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
        if x1 <= x0 || y1 <= y0 {
            return Err(err(format!("sign {id}: no ink (font {family} missing?)")));
        }
        let ink = image::imageops::crop_imm(&a, x0, y0, x1 - x0, y1 - y0).to_image();
        // into the box at 512 px per metre of height-normalised resolution
        let ph = 192u32;
        let pw = ((ph as f32) * wm / hm).round().max(8.0) as u32;
        let alpha = if contain {
            let k = (pw as f32 / ink.width() as f32).min(ph as f32 / ink.height() as f32);
            let (iw, ih) = (((ink.width() as f32) * k) as u32, ((ink.height() as f32) * k) as u32);
            let small = image::imageops::resize(&ink, iw.max(1), ih.max(1), image::imageops::FilterType::Lanczos3);
            let mut canvas = image::GrayImage::new(pw, ph);
            image::imageops::overlay(&mut canvas, &small, ((pw - iw.min(pw)) / 2) as i64, ((ph - ih.min(ph)) / 2) as i64);
            canvas
        } else {
            image::imageops::resize(&ink, pw, ph, image::imageops::FilterType::Lanczos3)
        };
        // a clear margin so the repeat-wrapped texture never bleeds one edge into the other
        let alpha = {
            let mut c = image::GrayImage::new(pw + 16, ph + 16);
            image::imageops::overlay(&mut c, &alpha, 8, 8);
            c
        };
        let (pw, ph) = (pw + 16, ph + 16);
        let base = image::RgbImage::from_pixel(pw, ph, image::Rgb([255, 255, 255]));
        write_surface("signs", &id, &[("BaseColor", png_rgb(&base)), ("Opacity", png_gray(&alpha))], (wm, hm), &format!("\"{text}\" set in {family} (OFL) by fps proctex"))?;
    }
    println!("signs: lettering surfaces written");
    Ok(())
}

pub fn all() -> std::io::Result<()> {
    ashlar()?;
    cleft()?;
    signs()?;
    flex_duct()?;
    subway()?;
    cowhide()?;
    weave()?;
    leather("pt_quilted_leather", true)?;
    leather("pt_biscuit_leather", false)?;
    println!("wrote pt_subway_tile, pt_cowhide, pt_basket_weave, pt_quilted_leather, pt_biscuit_leather");
    Ok(())
}

/// Rasterises a colour SVG (a TV picture, a poster) into an opaque surface.
pub fn svg_surface(svg: &std::path::Path, id: &str, wm: f32, hm: f32, px: u32) -> Result<(), Box<dyn std::error::Error>> {
    let tree = resvg::usvg::Tree::from_data(&std::fs::read(svg)?, &resvg::usvg::Options::default())?;
    let size = tree.size();
    let (w, h) = (px, ((px as f32) * size.height() / size.width()).round() as u32);
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height()), &mut pix.as_mut());
    let img = image::RgbImage::from_fn(w, h, |x, y| {
        let p = pix.pixel(x, y).map(|p| p.demultiply()).unwrap();
        image::Rgb([p.red(), p.green(), p.blue()])
    });
    write_surface("screens", id, &[("BaseColor", png_rgb(&img))], (wm, hm), &svg.display().to_string())?;
    println!("wrote {id} ({w}x{h}) from {}", svg.display());
    Ok(())
}

/// Rasterises a stencil SVG (drawn in black) into a decal surface. With `under` = (surface id, tile metres) the
/// decal carries that surface's own maps (colour darkened as stain, normal, roughness, AO) tiled at its scale, so
/// the stencil lights, shines and reflects like the floor it is stained into; otherwise flat ink.
pub fn decal(svg: &std::path::Path, id: &str, wm: f32, hm: f32, px: u32, under: Option<(&str, f32)>) -> Result<(), Box<dyn std::error::Error>> {
    let tree = resvg::usvg::Tree::from_data(&std::fs::read(svg)?, &resvg::usvg::Options::default())?;
    let size = tree.size();
    let (w, h) = (px, ((px as f32) * size.height() / size.width()).round() as u32);
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height()), &mut pix.as_mut());
    let alpha = image::GrayImage::from_fn(w, h, |x, y| image::Luma([pix.pixel(x, y).map_or(0, |p| p.alpha())]));
    let mut maps = vec![("Opacity", png_gray(&alpha))];
    match under {
        Some((sid, tile)) => {
            let root = crate::root();
            let cat = assets::Catalog::load(&root.join("data/assets")).map_err(|e| e.to_string())?;
            let a = cat.get(sid).ok_or("no such surface")?;
            let m = assets::load_material(&root.join("data/megascans_inbox"), a, 1024).map_err(|e| e.to_string())?;
            // texels of the source per decal pixel: the decal spans wm metres over w pixels, the source tile metres over 1024
            let k = 1024.0 * wm / (tile * w as f32);
            let at = |img: &image::RgbaImage, x: u32, y: u32| *img.get_pixel(((x as f32 * k) as u32) % 1024, ((y as f32 * k) as u32) % 1024);
            // stain: an even ink at the concrete's mean colour, darkened (the photo's stencil is solid, not
            // mottled); the concrete's finish (roughness, AO) and a softened normal keep it lit like the slabs
            let mut mean = [0f64; 3];
            for p in m.base.pixels() {
                for c in 0..3 {
                    mean[c] += (p[c] as f64 / 255.0).powf(2.2);
                }
            }
            let n = (m.base.width() * m.base.height()) as f64;
            let ink: Vec<u8> = (0..3).map(|c| (((mean[c] / n) * [0.2, 0.21, 0.22][c]).powf(1.0 / 2.2) * 255.0) as u8).collect();
            let base = image::RgbImage::from_fn(w, h, |_, _| image::Rgb([ink[0], ink[1], ink[2]]));
            let nrm = image::RgbImage::from_fn(w, h, |x, y| {
                let p = at(&m.normal, x, y);
                let soft = |v: u8| ((v as f32 - 128.0) * 0.3 + 128.0) as u8;
                image::Rgb([soft(p[0]), soft(p[1]), 255])
            });
            let rough = image::GrayImage::from_fn(w, h, |x, y| image::Luma([at(&m.orm, x, y)[1]]));
            let ao = image::GrayImage::from_fn(w, h, |x, y| image::Luma([at(&m.orm, x, y)[0]]));
            maps.extend([("BaseColor", png_rgb(&base)), ("Normal", png_rgb(&nrm)), ("Roughness", png_gray(&rough)), ("AO", png_gray(&ao))]);
        }
        None => {
            let ink = image::RgbImage::from_fn(w, h, |_, _| image::Rgb([74, 72, 68]));
            maps.push(("BaseColor", png_rgb(&ink)));
        }
    }
    write_surface("decals", id, &maps, (wm, hm), &svg.display().to_string())?;
    alpha.save(crate::out_dir().join(format!("{id}_preview.png")))?;
    println!("wrote decal {id} ({w}x{h}) from {}", svg.display());
    Ok(())
}
