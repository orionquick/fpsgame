//! The editor service behind both the CLI and the MCP server: load/edit scenes, validate, render reviews.
use crate::review::{self, Ctx};
use editor::{Analysis, Op, Scene, Store, World};
use image::RgbaImage;
use serde_json::{json, Value};
use std::path::PathBuf;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Editor {
    pub cx: Ctx,
    pub world: World,
    pub dir: PathBuf,
}

impl Editor {
    pub fn new(city: city::City, dir: PathBuf) -> Self {
        let world = World::new(city.clone());
        Self { cx: Ctx::new(city), world, dir }
    }
    pub fn store(&self, name: &str) -> Store {
        Store::new(&self.dir, name)
    }
    pub fn load(&self, name: &str) -> Res<Scene> {
        self.store(name).load(&self.world)
    }
    pub fn apply(&self, name: &str, ops: &[Op], by: &str) -> Res<(Scene, Vec<Vec<u32>>, Analysis)> {
        let (s, made) = self.store(name).commit(ops, by, &self.world)?;
        let a = editor::analyze(&self.world, &s);
        Ok((s, made, a))
    }
    pub fn undo(&self, name: &str, n: usize) -> Res<Scene> {
        self.store(name).undo(n, &self.world)
    }
    pub fn analyze(&self, s: &Scene) -> Analysis {
        editor::analyze(&self.world, s)
    }
    pub fn review(&mut self, s: &Scene, a: &Analysis, mpp: f64) -> RgbaImage {
        self.cx.set_scene(s);
        review::scene_review(&self.cx, a, s, mpp)
    }
    pub fn shot(&mut self, s: &Scene, x: f32, z: f32, yaw: f32, pitch: f32, w: u32, h: u32) -> RgbaImage {
        self.cx.set_scene(s);
        review::shot(&self.cx, x, z, yaw, pitch, w, h)
    }

    /// Named streets inside `rect`: orientation and centreline position, so edits can be placed by street.
    pub fn streets(&self, rect: [f64; 4]) -> Value {
        use std::collections::BTreeMap;
        let mut by: BTreeMap<String, (f64, f64, f64, f64, f64, usize, Vec<[f64; 2]>)> = BTreeMap::new();
        for l in self.world.city.lines.iter().filter(|l| l.kind == city::model::LineKind::Road) {
            let Some(name) = &l.name else { continue };
            for p in &l.pts {
                if p[0] < rect[0] || p[0] > rect[2] || p[1] < rect[1] || p[1] > rect[3] {
                    continue;
                }
                let e = by.entry(name.clone()).or_insert((f64::MAX, f64::MIN, f64::MAX, f64::MIN, 0.0, 0, vec![]));
                e.6.push(*p);
                e.0 = e.0.min(p[0]);
                e.1 = e.1.max(p[0]);
                e.2 = e.2.min(p[1]);
                e.3 = e.3.max(p[1]);
                e.4 += l.width;
                e.5 += 1;
            }
        }
        let rows: Vec<Value> = by
            .into_iter()
            .map(|(name, (x0, x1, z0, z1, wsum, n, mut pts))| {
                let ns = (z1 - z0) > (x1 - x0);
                // median across the street, robust to bends and side carriageways
                let axis = if ns { 0 } else { 1 };
                pts.sort_by(|a, b| a[axis].total_cmp(&b[axis]));
                let med = pts[pts.len() / 2][axis];
                json!({
                    "name": name,
                    "runs": if ns { "north-south" } else { "east-west" },
                    "centreline": if ns { json!({"x": med.round()}) } else { json!({"z": med.round()}) },
                    "extent": if ns { json!([z0.round(), z1.round()]) } else { json!([x0.round(), x1.round()]) },
                    "width_m": (wsum / n as f64).round(),
                })
            })
            .collect();
        json!(rows)
    }

    /// Short text summary of a validation, for op results.
    pub fn summary(a: &Analysis) -> String {
        let r = &a.report;
        let mut s = format!(
            "objects {}, walkable {} m², reachable {:.0}%, regions {}, cover {:.0}%, sight >80 m {:.0}%, >150 m {:.0}%",
            r.objects, r.walkable_m2, r.reachable_pct, r.regions, r.cover_pct, r.sight_over_80m_pct, r.sight_over_150m_pct
        );
        if let (Some(a), Some(b)) = (r.team_a_to_centre_m, r.team_b_to_centre_m) {
            s += &format!(", spawn→centre A {a} m / B {b} m");
        }
        if let Some(d) = r.spawn_to_spawn_m {
            s += &format!(", spawn↔spawn {d} m");
        }
        if r.issues.is_empty() {
            s += "\nno issues";
        } else {
            s += "\nissues:\n- ";
            s += &r.issues.join("\n- ");
        }
        s
    }
}

/// Parses ops from a JSON value: one op object or an array of them.
pub fn parse_ops(v: Value) -> Res<Vec<Op>> {
    let arr = if v.is_array() { v } else { Value::Array(vec![v]) };
    Ok(serde_json::from_value(arr)?)
}
