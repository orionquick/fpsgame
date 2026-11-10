//! MCP server (JSON-RPC 2.0 over stdio) exposing the map editor to Claude: edit ops, validation, review renders.
use crate::service::{parse_ops, Editor};
use base64::Engine;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

const OP_DOC: &str = "Ops (JSON objects with an \"op\" field). Coordinates are metres from home plate: x east, z south (north is -z). \
rot/yaw are degrees clockwise from north; an object's length runs along rot (rot 90 = length east-west).\n\
- {\"op\":\"place\",\"kind\":K,\"x\":X,\"z\":Z,\"rot\":R,\"size\":[len,height,depth]?}\n\
- {\"op\":\"line\",\"kind\":K,\"from\":[x,z],\"to\":[x,z],\"gap\":G?}  row of objects end to end, e.g. barriers closing a street\n\
- {\"op\":\"scatter\",\"kind\":K,\"rect\":[x0,z0,x1,z1],\"count\":N,\"seed\":S?}  random, avoiding buildings and objects\n\
- {\"op\":\"move\",\"id\":I,\"x\"?,\"z\"?,\"rot\"?}   {\"op\":\"delete\",\"id\":I}   {\"op\":\"delete_rect\",\"rect\":[x0,z0,x1,z1]}\n\
- {\"op\":\"spawn\",\"team\":\"a\"|\"b\",\"x\":X,\"z\":Z,\"yaw\":Y}   {\"op\":\"clear_spawns\"}\n\
- {\"op\":\"bounds\",\"rect\":[x0,z0,x1,z1]}  playable rectangle; outside it does not exist for validation\n\
Kinds (default length x height x depth, m): barrier 3.0x0.81x0.6 (vaultable cover, see-over), car 4.6x1.5x1.85 (blocks movement, see-over), \
bus 12.2x3.2x2.6, container 6.06x2.6x2.44, dumpster 1.9x1.3x1.6, crate 1.2 cube, fence 3.0x2.4x0.15 (blocks sight), box (set size). \
Objects taller than 1.1 m block movement, at least 1.6 m block sight, at least 0.8 m count as cover.";

fn tools() -> Value {
    let scene = json!({"type": "string", "description": "Scene name (default from --scene)"});
    let rect = json!({"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4, "description": "[min_x, min_z, max_x, max_z] metres"});
    json!([
        {"name": "scene_status", "description": "Scene summary: bounds, object counts by kind, spawns, the last ops, and the op reference.",
         "inputSchema": {"type": "object", "properties": {"scene": scene}}},
        {"name": "streets", "description": "Named streets inside a rectangle (default: scene bounds) with orientation, centreline position and width, for placing things by street.",
         "inputSchema": {"type": "object", "properties": {"scene": scene, "rect": rect}}},
        {"name": "apply_ops", "description": format!("Apply edit ops atomically (all or nothing), log them, and return created object ids plus a validation summary.\n{OP_DOC}"),
         "inputSchema": {"type": "object", "properties": {"scene": scene, "ops": {"type": "array", "items": {"type": "object"}}}, "required": ["ops"]}},
        {"name": "undo", "description": "Remove the last n ops from the log.",
         "inputSchema": {"type": "object", "properties": {"scene": scene, "n": {"type": "integer", "minimum": 1}}}},
        {"name": "list_objects", "description": "Objects (id, kind, x, z, rot, size), optionally only inside a rectangle.",
         "inputSchema": {"type": "object", "properties": {"scene": scene, "rect": rect}}},
        {"name": "validate", "description": "Full gameplay report: walkable area, reachability, regions, cover, sightline stats with the longest sightlines (from/to/length), spawn checks and fairness, overlaps, issues.",
         "inputSchema": {"type": "object", "properties": {"scene": scene}}},
        {"name": "render_review", "description": "Top-down review image of the scene bounds: left = render with object ids, spawns and the longest sightlines; right = longest view per spot (green < 40 m, yellow 80 m, red > 150 m), unreachable space magenta. 50 m labelled grid.",
         "inputSchema": {"type": "object", "properties": {"scene": scene}}},
        {"name": "render_shot", "description": "Eye-height (1.7 m) view from a point. yaw 0 = north, 90 = east; pitch up positive.",
         "inputSchema": {"type": "object", "properties": {"scene": scene, "x": {"type": "number"}, "z": {"type": "number"}, "yaw": {"type": "number"}, "pitch": {"type": "number"}}, "required": ["x", "z"]}}
    ])
}

fn text(s: impl Into<String>) -> Value {
    json!({"type": "text", "text": s.into()})
}

fn png(img: &image::RgbaImage, max_w: u32) -> Value {
    let img = if img.width() > max_w {
        image::imageops::resize(img, max_w, (img.height() as f64 * max_w as f64 / img.width() as f64) as u32, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let mut buf = std::io::Cursor::new(vec![]);
    img.write_to(&mut buf, image::ImageFormat::Png).unwrap();
    json!({"type": "image", "mimeType": "image/png", "data": base64::engine::general_purpose::STANDARD.encode(buf.into_inner())})
}

fn call(ed: &mut Editor, default_scene: &str, name: &str, args: &Value) -> Result<Vec<Value>, String> {
    let scene = args["scene"].as_str().unwrap_or(default_scene).to_string();
    let e = |x: Box<dyn std::error::Error>| x.to_string();
    let rect = |v: &Value| -> Option<[f64; 4]> { serde_json::from_value(v.clone()).ok() };
    match name {
        "scene_status" => {
            let s = ed.load(&scene).map_err(e)?;
            let log = ed.store(&scene).log().map_err(e)?;
            let mut counts = std::collections::BTreeMap::new();
            for o in &s.objects {
                *counts.entry(format!("{:?}", o.kind).to_lowercase()).or_insert(0) += 1;
            }
            let last: Vec<Value> = log.iter().rev().take(10).map(|en| serde_json::to_value(en).unwrap()).collect();
            Ok(vec![text(serde_json::to_string_pretty(&json!({
                "scene": scene, "bounds": ed.world.bounds(&s), "objects": s.objects.len(), "by_kind": counts, "spawns": s.spawns,
                "ops_logged": log.len(), "last_ops_newest_first": last,
                "city_play_box": ed.world.city.play_rect(),
            })).unwrap()), text(OP_DOC)])
        }
        "streets" => {
            let s = ed.load(&scene).map_err(e)?;
            let r = rect(&args["rect"]).unwrap_or_else(|| ed.world.bounds(&s));
            Ok(vec![text(serde_json::to_string_pretty(&ed.streets(r)).unwrap())])
        }
        "apply_ops" => {
            let ops = parse_ops(args["ops"].clone()).map_err(e)?;
            let (_, made, a) = ed.apply(&scene, &ops, "claude").map_err(e)?;
            Ok(vec![text(format!("applied {} op(s); created ids per op: {:?}\n{}", ops.len(), made, Editor::summary(&a)))])
        }
        "undo" => {
            let n = args["n"].as_u64().unwrap_or(1) as usize;
            let s = ed.undo(&scene, n).map_err(e)?;
            let a = ed.analyze(&s);
            Ok(vec![text(format!("undid {n} op(s)\n{}", Editor::summary(&a)))])
        }
        "list_objects" => {
            let s = ed.load(&scene).map_err(e)?;
            let r = rect(&args["rect"]);
            let v: Vec<&editor::scene::Object> = s.objects.iter().filter(|o| r.is_none_or(|r| o.x >= r[0] && o.x <= r[2] && o.z >= r[1] && o.z <= r[3])).collect();
            Ok(vec![text(serde_json::to_string(&v).unwrap())])
        }
        "validate" => {
            let s = ed.load(&scene).map_err(e)?;
            let a = ed.analyze(&s);
            Ok(vec![text(serde_json::to_string_pretty(&a.report).unwrap())])
        }
        "render_review" => {
            let s = ed.load(&scene).map_err(e)?;
            let a = ed.analyze(&s);
            let img = ed.review(&s, &a, 0.5);
            let path = ed.dir.join("../../out").join(format!("{scene}_review.png"));
            let _ = img.save(&path);
            Ok(vec![png(&img, 1800), text(format!("{}\nfull resolution: {}", Editor::summary(&a), path.display()))])
        }
        "render_shot" => {
            let s = ed.load(&scene).map_err(e)?;
            let f = |k: &str, d: f64| args[k].as_f64().unwrap_or(d) as f32;
            let img = ed.shot(&s, f("x", 0.0), f("z", 0.0), f("yaw", 0.0), f("pitch", 0.0), 1280, 720);
            Ok(vec![png(&img, 1280)])
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

pub fn serve(mut ed: Editor, default_scene: &str) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let id = req.get("id").cloned();
        let method = req["method"].as_str().unwrap_or("");
        let result = match method {
            "initialize" => Some(json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "fps-editor", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Map editor for the downtown San Diego FPS. Start with scene_status and render_review, edit with apply_ops, check with validate/render_review, look with render_shot."
            })),
            "tools/list" => Some(json!({"tools": tools()})),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = req["params"]["arguments"].clone();
                Some(match call(&mut ed, default_scene, name, &args) {
                    Ok(content) => json!({"content": content}),
                    Err(err) => json!({"content": [text(err)], "isError": true}),
                })
            }
            "ping" => Some(json!({})),
            _ => None,
        };
        let Some(id) = id else { continue }; // notifications get no reply
        let msg = match result {
            Some(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            None => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("method not found: {method}")}}),
        };
        writeln!(out, "{msg}")?;
        out.flush()?;
    }
    Ok(())
}
