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

pub fn all() -> std::io::Result<()> {
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
