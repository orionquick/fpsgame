mod mcp;
mod play;
mod review;
mod service;
mod world;

use std::path::PathBuf;

fn root() -> PathBuf {
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
  fps mcp [--scene NAME]                 MCP server on stdio (see .mcp.json)";

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
        Some("ortho") => {
            let cx = review::Ctx::new(city::City::load(&city_path())?);
            let img = review::ortho(&cx, arg(&args, "--mpp", 0.5));
            let p = out_dir().join("ortho.png");
            img.save(&p)?;
            println!("wrote {} ({}x{})", p.display(), img.width(), img.height());
        }
        Some("compare") => {
            let cx = review::Ctx::new(city::City::load(&city_path())?);
            review::compare(&cx, &root().join("data/raw"), &out_dir())?;
            println!("wrote {}", out_dir().join("compare.png").display());
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
        Some("mcp") => {
            let ed = editor()?;
            mcp::serve(ed, &scene_arg(&args).unwrap_or_else(|| "arena".into()))?;
        }
        _ => eprintln!("{USAGE}"),
    }
    Ok(())
}
