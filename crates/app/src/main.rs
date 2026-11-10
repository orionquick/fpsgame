mod bespoke;
mod buildings;
mod dressing;
mod gi;
mod library;
mod mcp;
mod play;
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
