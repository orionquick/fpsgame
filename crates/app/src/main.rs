mod play;
mod review;
mod world;

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn city_path() -> PathBuf {
    root().join("data/city/downtown_sd.json")
}

fn out_dir() -> PathBuf {
    let p = root().join("out");
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn arg<T: std::str::FromStr>(args: &[String], key: &str, d: T) -> T {
    args.windows(2).find(|w| w[0] == key).and_then(|w| w[1].parse().ok()).unwrap_or(d)
}

const USAGE: &str = "usage:
  fps import                         fetch OSM + USGS data, build data/city/downtown_sd.json
  fps ortho [--mpp 0.5]              top-down render -> out/ortho.png
  fps compare                        render next to the NAIP aerial -> out/compare.png
  fps shot --x X --z Z [--yaw D] [--pitch D] [--w 1600] [--h 900] [--out name.png]
                                     eye-height shot (yaw 0 = north, 90 = east; metres from home plate, z = south)
  fps play [--x X --z Z --yaw D]     walk the city (default spawn: 5th Ave & Market St, facing north)";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
            let cx = review::Ctx::new(city::City::load(&city_path())?);
            let img = review::shot(&cx, arg(&args, "--x", 0.0), arg(&args, "--z", 0.0), arg(&args, "--yaw", 0.0), arg(&args, "--pitch", 0.0), arg(&args, "--w", 1600), arg(&args, "--h", 900));
            let p = out_dir().join(arg(&args, "--out", "shot.png".to_string()));
            img.save(&p)?;
            println!("wrote {}", p.display());
        }
        Some("play") => {
            let c = city::City::load(&city_path())?;
            play::run(c, (arg(&args, "--x", -291.5), arg(&args, "--z", -470.0)), arg(&args, "--yaw", 0.0))?;
        }
        _ => eprintln!("{USAGE}"),
    }
    Ok(())
}
