mod bespoke;
mod buildings;
mod dressing;
mod gi;
mod library;
mod mcp;
mod play;
mod procmodel;
mod proctex;
mod review;
mod service;
mod world;

use std::path::PathBuf;

/// Camera white balance indoors: the light is mostly bluish sky through windows, which a camera (or the eye)
/// would neutralise; warm a little.
pub const INTERIOR_WHITE: [f32; 3] = [1.0, 0.92, 0.76];
/// White balance when the interior lamps are on: their light is already warm, so the camera stays near neutral.
pub const LAMPLIT_WHITE: [f32; 3] = [1.0, 0.98, 0.93];
/// Camera exposure indoors with GI: daylight inside a deep brick hall is a few percent of outdoors, so a camera
/// opens up about four stops (windows and sun patches blow out, as in the reference photos).
pub const INTERIOR_EXPOSURE: f32 = 18.0;
/// Metering target for interior shots: scene log-average luminance x exposure (calibrated so the empty daylit
/// hall meters to about the exposure that matched its photo).
pub const INTERIOR_KEY: f32 = 0.11;
/// Lamp-lit rooms keep deep shadows (bright lamps and a dark ceiling, as in the bar photo): a lower key.
pub const LAMPLIT_KEY: f32 = 0.075;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn city_path() -> PathBuf {
    root().join("data/city/downtown_sd.json")
}

fn scenes_dir() -> PathBuf {
    root().join("data/scenes")
}

fn out_dir() -> PathBuf {
    let p = root().join("out");
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn arg<T: std::str::FromStr>(args: &[String], key: &str, d: T) -> T {
    args.windows(2).find(|w| w[0] == key).and_then(|w| w[1].parse().ok()).unwrap_or(d)
}

/// The scene named by `--scene`, if any.
fn scene_arg(args: &[String]) -> Option<String> {
    args.windows(2).find(|w| w[0] == "--scene").map(|w| w[1].clone())
}

fn editor() -> Result<service::Editor, Box<dyn std::error::Error>> {
    Ok(service::Editor::new(city::City::load(&city_path())?, scenes_dir()))
}

const USAGE: &str = "usage:
  fps import                         fetch OSM + USGS data, build data/city/downtown_sd.json
  fps ortho [--mpp 0.5]              top-down render -> out/ortho.png
  fps compare                        render next to the NAIP aerial -> out/compare.png
  fps shot --x X --z Z [--yaw D] [--pitch D] [--w 1600] [--h 900] [--out name.png] [--scene NAME]
                                     eye-height shot (yaw 0 = north, 90 = east; metres from home plate, z = south)
  fps play [--scene NAME] [--x X --z Z --yaw D]
                                     walk the city (spawns at the scene's first spawn, else 5th Ave & J St)

map editor (scenes live in data/scenes/NAME.ops.jsonl, the op log; NAME.json is the replayed snapshot):
  fps ops NAME '<op or [ops] as JSON>'   apply ops, print created ids + validation summary
  fps ops NAME --file ops.json           same, from a file
  fps undo NAME [N]                      drop the last N ops (default 1)
  fps validate NAME                      full gameplay report as JSON
  fps review NAME                        review image -> out/NAME_review.png
  fps streets NAME                       named streets in the scene bounds, with centreline positions
  fps mcp [--scene NAME]                 MCP server on stdio (see .mcp.json)
  fps gallery GROUP|ID-PREFIX [--fit]    contact sheet of an asset group's models (orientation/scale check)
  fps assets                             compile data/megascans_inbox -> data/assets (meshes + catalog)";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = || args.get(1).cloned().ok_or("missing scene name");
    match args.first().map(String::as_str) {
        Some("import") => {
            let c = city::import::import(&root().join("data/raw"))?;
            let n_lv = c.buildings.iter().filter(|b| b.height_src != "default").count();
            println!(
                "buildings {} ({} with real height/levels), lines {}, areas {}, trees {}, home plate elev {:.1} m",
                c.buildings.len(), n_lv, c.lines.len(), c.areas.len(), c.trees.len(), c.origin_elev
            );
            c.save(&city_path())?;
            println!("wrote {}", city_path().display());
        }
        Some("plan") => {
            // fps plan X0 Z0 X1 Z1 CUT_Y [--mpp m] [--out f]: interior plan, everything above CUT_Y cut away
            let f = |i: usize| args.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or("usage: fps plan X0 Z0 X1 Z1 CUT_Y");
            let (rect, cut) = ([f(1)?, f(2)?, f(3)?, f(4)?], f(5)?);
            let mut ed = editor()?;
            let img = ed.plan(rect, cut, arg(&args, "--mpp", 0.02));
            let p = out_dir().join(arg(&args, "--out", "plan.png".to_string()));
            img.save(&p)?;
            println!("wrote {}", p.display());
        }
        Some("ortho") => {
            let mut cx = review::Ctx::new(city::City::load(&city_path())?);
            cx.set_focus(cx.city.play_rect());
            let img = review::ortho(&cx, arg(&args, "--mpp", 0.5));
            let p = out_dir().join("ortho.png");
            img.save(&p)?;
            println!("wrote {} ({}x{})", p.display(), img.width(), img.height());
        }
        Some("compare") => {
            let mut cx = review::Ctx::new(city::City::load(&city_path())?);
            cx.set_focus(cx.city.play_rect());
            review::compare(&cx, &root().join("data/raw"), &out_dir())?;
            println!("wrote {}", out_dir().join("compare.png").display());
        }
        Some("decal") => {
            // fps decal SVG ID W_M H_M PX: render an SVG into a cutout decal surface (dark ink where the SVG draws)
            // packaged for `fps assets` as data/megascans_inbox/decals/<ID>.zip
            let svg = args.get(1).ok_or("usage: fps decal SVG ID W_M H_M PX")?;
            let id = args.get(2).ok_or("missing id")?;
            let (wm, hm): (f32, f32) = (args.get(3).and_then(|v| v.parse().ok()).ok_or("missing width")?, args.get(4).and_then(|v| v.parse().ok()).ok_or("missing height")?);
            let px: u32 = args.get(5).and_then(|v| v.parse().ok()).unwrap_or(2048);
            // optional: SURFACE TILE_M to stain into that surface's own maps
            let under = args.get(6).zip(args.get(7).and_then(|t| t.parse::<f32>().ok()));
            proctex::decal(std::path::Path::new(svg), id, wm, hm, px, under.map(|(s, t)| (s.as_str(), t)))?;
        }
        Some("proctex") => proctex::all()?,
        Some("svgtex") => {
            // fps svgtex SVG ID W_M H_M PX: a colour SVG as an opaque surface (TV pictures, posters)
            let f = |i: usize| args.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or("usage: fps svgtex SVG ID W_M H_M PX");
            proctex::svg_surface(std::path::Path::new(args.get(1).ok_or("missing svg")?), args.get(2).ok_or("missing id")?, f(3)?, f(4)?, f(5)? as u32)?;
        }
        Some("sbs") => {
            // fps sbs OUT.png H img1 img2 ...: reference photos and renders side by side at one height.
            let out = args.get(1).ok_or("usage: fps sbs OUT.png H img...")?;
            let h: u32 = args.get(2).and_then(|s| s.parse().ok()).ok_or("missing height")?;
            let ims: Vec<image::RgbImage> = args[3..]
                .iter()
                .map(|p| {
                    let im = image::open(p).map(|i| i.to_rgb8())?;
                    let w = (im.width() as f32 * h as f32 / im.height() as f32).round() as u32;
                    Ok(image::imageops::resize(&im, w, h, image::imageops::FilterType::Lanczos3))
                })
                .collect::<Result<_, image::ImageError>>()?;
            let total = ims.iter().map(|i| i.width() + 6).sum::<u32>() - 6;
            let mut c = image::RgbImage::from_pixel(total, h, image::Rgb([20, 20, 20]));
            let mut x = 0;
            for im in &ims {
                image::imageops::replace(&mut c, im, x as i64, 0);
                x += im.width() + 6;
            }
            c.save(out)?;
            println!("wrote {out}");
        }
        Some("gallery") => {
            let group = args.get(1).cloned().ok_or("missing asset group")?;
            let mut cx = review::Ctx::new(city::City::load(&city_path())?);
            let (img, legend) = review::gallery(&mut cx, &group, arg(&args, "--w", 1800), arg(&args, "--h", 1100), args.iter().any(|a| a == "--fit"));
            let name: String = if group.len() > 60 { format!("{}_{}", &group[..20], group.len()) } else { group.clone() };
            let p = out_dir().join(arg(&args, "--out", format!("gallery_{name}.png")));
            img.save(&p)?;
            for (i, id, t) in legend {
                println!("{i:3} {id:12} {t}");
            }
            println!("wrote {}", p.display());
        }
        Some("shot") => {
            let mut ed = editor()?;
            let s = scene_arg(&args).map(|n| ed.load(&n)).transpose()?.unwrap_or_default();
            let img = ed.shot(&s, arg(&args, "--x", 0.0), arg(&args, "--z", 0.0), arg(&args, "--yaw", 0.0), arg(&args, "--pitch", 0.0), arg(&args, "--w", 1600), arg(&args, "--h", 900));
            let p = out_dir().join(arg(&args, "--out", "shot.png".to_string()));
            img.save(&p)?;
            println!("wrote {}", p.display());
        }
        Some("play") => {
            let c = city::City::load(&city_path())?;
            let scene = scene_arg(&args).map(|n| editor::Store::new(&scenes_dir(), &n).load(&editor::World::new(c.clone()))).transpose()?;
            play::run(c, scene, (arg(&args, "--x", -291.5), arg(&args, "--z", -250.0)), arg(&args, "--yaw", 0.0))?;
        }
        Some("dsm") => {
            // fps dsm X0 Z0 X1 Z1: lidar surface (highest return) over a rect at 0.5 m -> out/dsm.bin (f32 metres above
            // home plate, row-major, NaN = no return) + out/dsm.json, a shaded height image, and the NAIP aerial crop
            let f = |i: usize| args.get(i).and_then(|v| v.parse::<f64>().ok()).ok_or("usage: fps dsm X0 Z0 X1 Z1");
            let rect = [f(1)?, f(2)?, f(3)?, f(4)?];
            let c = city::City::load(&city_path())?;
            let g = city::lidar::rasterize(&city::lidar::tile_paths(&root().join("data/raw/lidar")), &c.frame, rect)?;
            let rel: Vec<f32> = g.top.iter().map(|v| v - c.origin_elev as f32).collect();
            std::fs::write(out_dir().join("dsm.bin"), bytemuck::cast_slice(&rel))?;
            std::fs::write(out_dir().join("dsm.json"), format!("{{\"rect\": {:?}, \"w\": {}, \"h\": {}, \"res\": {}}}", rect, g.w, g.h, city::lidar::RES))?;
            // shaded relief coloured by height (0 m blue .. 45 m red)
            let at = |i: usize, j: usize| rel[j.min(g.h - 1) * g.w + i.min(g.w - 1)];
            let img = image::RgbImage::from_fn(g.w as u32, g.h as u32, |i, j| {
                let (i, j) = (i as usize, j as usize);
                let z = at(i, j);
                if !z.is_finite() {
                    return image::Rgb([0, 0, 0]);
                }
                let dx = at(i + 1, j) - at(i.saturating_sub(1), j);
                let dz = at(i, j + 1) - at(i, j.saturating_sub(1));
                let shade = (0.75 - 0.25 * (dx - dz)).clamp(0.25, 1.0);
                let t = (z / 45.0).clamp(0.0, 1.0);
                let col = [(255.0 * t), (255.0 * (1.0 - (2.0 * t - 1.0).abs())), (255.0 * (1.0 - t))];
                image::Rgb(col.map(|c| (c * shade) as u8))
            });
            img.save(out_dir().join("dsm.png"))?;
            // the aerial over the same rect
            let naip = image::open(root().join("data/raw").join(city::import::naip_name(&c.import_box)))?.to_rgb8();
            let ib = c.import_box;
            let crop = image::RgbImage::from_fn(g.w as u32, g.h as u32, |i, j| {
                let (x, z) = (rect[0] + (i as f64 + 0.5) * city::lidar::RES, rect[1] + (j as f64 + 0.5) * city::lidar::RES);
                let (lat, lon) = c.frame.to_latlon(x, z);
                // the image server keeps square-degree pixels: the requested width holds, the height is centred
                let dpp = (ib.e - ib.w) / naip.width() as f64;
                let top = (ib.n + ib.s) / 2.0 + dpp * naip.height() as f64 / 2.0;
                let u = ((lon - ib.w) / dpp) as u32;
                let v = ((top - lat) / dpp) as u32;
                *naip.get_pixel(u.min(naip.width() - 1), v.min(naip.height() - 1))
            });
            crop.save(out_dir().join("naip_crop.png"))?;
            std::fs::write(out_dir().join("naip_crop.rgb"), crop.as_raw())?;
            println!("dsm {}x{} over {:?} -> out/dsm.bin, dsm.png, naip_crop.png", g.w, g.h, rect);
        }
        Some("bench") => {
            // fps bench [--scene NAME] [--w 1440 --h 900 --frames 60]: headless frame timing at fixed views
            let c = city::City::load(&city_path())?;
            let scene = scene_arg(&args).map(|n| editor::Store::new(&scenes_dir(), &n).load(&editor::World::new(c.clone()))).transpose()?;
            let ground = |x: f32, z: f32| c.terrain.at(x as f64, z as f64) as f32;
            let eye = |x: f32, z: f32, up: f32| glam::Vec3::new(x, ground(x, z) + up, z);
            let mut views = vec![];
            if let Some(sp) = scene.as_ref().and_then(|s| s.spawns.first()) {
                views.push((eye(sp.x as f32, sp.z as f32, 1.7), sp.yaw as f32, 0.0));
            }
            // the beer hall (inside the factory), a long street, and from above
            views.push((glam::Vec3::new(73.4, 5.04 + 1.65, -163.9), 80.0, 9.0));
            views.push((eye(-291.5, -250.0, 1.7), 0.0, 0.0));
            views.push((eye(40.0, -120.0, 60.0), 300.0, -20.0));
            // the ballpark: an upper-deck seat behind home, and the batter's box
            views.push((eye(-24.0, 50.0, 30.5), 18.0, -15.0));
            views.push((eye(-2.1, 8.6, 1.7), 0.0, 2.0));
            // the home plate plaza from the street: gate fence, palm beds, paving
            views.push((eye(30.0, 128.0, 1.7), -40.0, 8.0));
            play::bench(&c, scene.as_ref(), &views, arg(&args, "--w", 1440), arg(&args, "--h", 900), arg(&args, "--frames", 40));
        }
        Some("ops") => {
            let ed = editor()?;
            let json = match args.windows(2).find(|w| w[0] == "--file") {
                Some(w) => std::fs::read_to_string(&w[1])?,
                None => args.get(2).cloned().ok_or("missing ops JSON")?,
            };
            let ops = service::parse_ops(serde_json::from_str(&json)?)?;
            let (_, made, a) = ed.apply(&name()?, &ops, "cli")?;
            println!("applied {} op(s); created ids per op: {made:?}\n{}", ops.len(), service::Editor::summary(&a));
        }
        Some("undo") => {
            let ed = editor()?;
            let s = ed.undo(&name()?, args.get(2).and_then(|n| n.parse().ok()).unwrap_or(1))?;
            println!("{}", service::Editor::summary(&ed.analyze(&s)));
        }
        Some("validate") => {
            let ed = editor()?;
            let s = ed.load(&name()?)?;
            println!("{}", serde_json::to_string_pretty(&ed.analyze(&s).report)?);
        }
        Some("review") => {
            let mut ed = editor()?;
            let n = name()?;
            let s = ed.load(&n)?;
            let a = ed.analyze(&s);
            let img = ed.review(&s, &a, arg(&args, "--mpp", 0.5));
            let p = out_dir().join(format!("{n}_review.png"));
            img.save(&p)?;
            println!("wrote {}\n{}", p.display(), service::Editor::summary(&a));
        }
        Some("streets") => {
            let ed = editor()?;
            let s = ed.load(&name()?)?;
            println!("{}", serde_json::to_string_pretty(&ed.streets(ed.world.bounds(&s)))?);
        }
        Some("assets") => {
            let t = std::time::Instant::now();
            let (cat, errors) = assets::compile(&root().join("data/megascans_inbox"), &root().join("data/assets")).map_err(|e| e.to_string())?;
            let models = cat.assets.iter().filter(|a| a.kind == assets::AssetKind::Model).count();
            let parts: usize = cat.assets.iter().map(|a| a.parts.len()).sum();
            println!("compiled {} assets ({} models with {} parts, {} surfaces) in {:.1}s; {} errors", cat.assets.len(), models, parts, cat.assets.len() - models, t.elapsed().as_secs_f32(), errors.len());
            for e in errors.iter().take(10) {
                println!("  {e}");
            }
        }
        Some("buildstats") => {
            let c = city::City::load(&city_path())?;
            let lib = library::Library::open(&root()).ok_or("run `fps assets` first")?;
            let focus = [-127.0, -263.0, 234.0, -127.0];
            let (pl, st) = buildings::build(&lib, &c, focus);
            println!("prefab buildings {}: {} pieces, {:.1}M triangles", st.buildings, st.pieces, st.tris as f64 / 1e6);
            let mut by: std::collections::HashMap<(String, u8), (usize, f64)> = std::collections::HashMap::new();
            for p in &pl {
                let Some(a) = lib.catalog.get(&p.asset) else { continue };
                let t = a.parts.get(p.part).map_or(0.0, |q| q.tris as f64 * [1.0, 0.25, 0.06][p.lod as usize]);
                let e = by.entry((a.title.clone(), p.lod)).or_default();
                e.0 += 1;
                e.1 += t;
            }
            let mut v: Vec<_> = by.into_iter().collect();
            v.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
            for ((t, l), (n, tris)) in v.iter().take(14) {
                println!("  {:45} lod{l} {n:7} pieces {:7.1}M", t, tris / 1e6);
            }
        }
        Some("mcp") => {
            let ed = editor()?;
            mcp::serve(ed, &scene_arg(&args).unwrap_or_else(|| "arena".into()))?;
        }
        _ => eprintln!("{USAGE}"),
    }
    Ok(())
}
