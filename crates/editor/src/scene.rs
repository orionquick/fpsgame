//! A scene: gameplay objects, spawns and bounds layered on the generated city.
//! The op log (`<name>.ops.jsonl`) is the source of truth; `<name>.json` is the replayed snapshot, kept for diffs.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Blockout primitives until real assets arrive. Each has a default size (x = length, y = height, z = depth).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Concrete jersey barrier: waist-high cover, doesn't block sight.
    Barrier,
    /// Parked car: low cover, mostly see-over.
    Car,
    Bus,
    /// Shipping container: blocks movement and sight.
    Container,
    Dumpster,
    Crate,
    /// Construction fence / plywood wall: blocks sight, thin.
    Fence,
    /// Generic blockout box; set `size`.
    Box,
}

impl Kind {
    pub fn default_size(self) -> [f64; 3] {
        match self {
            Kind::Barrier => [3.0, 0.81, 0.6],
            Kind::Car => [4.6, 1.5, 1.85],
            Kind::Bus => [12.2, 3.2, 2.6],
            Kind::Container => [6.06, 2.6, 2.44],
            Kind::Dumpster => [1.9, 1.3, 1.6],
            Kind::Crate => [1.2, 1.2, 1.2],
            Kind::Fence => [3.0, 2.4, 0.15],
            Kind::Box => [2.0, 2.0, 2.0],
        }
    }
    pub fn all() -> &'static [Kind] {
        &[Kind::Barrier, Kind::Car, Kind::Bus, Kind::Container, Kind::Dumpster, Kind::Crate, Kind::Fence, Kind::Box]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Object {
    pub id: u32,
    pub kind: Kind,
    /// Centre on the ground, metres from home plate (x east, z south).
    pub x: f64,
    pub z: f64,
    /// Heading in degrees, clockwise from north; the object's length runs along it.
    pub rot: f64,
    pub size: [f64; 3],
}

impl Object {
    /// Footprint corners in x/z.
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let r = self.rot.to_radians();
        let (fx, fz) = (r.sin(), -r.cos()); // along length
        let (sx, sz) = (-fz, fx); // across
        let (hl, hd) = (self.size[0] * 0.5, self.size[2] * 0.5);
        [
            [self.x + fx * hl + sx * hd, self.z + fz * hl + sz * hd],
            [self.x - fx * hl + sx * hd, self.z - fz * hl + sz * hd],
            [self.x - fx * hl - sx * hd, self.z - fz * hl - sz * hd],
            [self.x + fx * hl - sx * hd, self.z + fz * hl - sz * hd],
        ]
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let r = self.rot.to_radians();
        let (fx, fz) = (r.sin(), -r.cos());
        let (dx, dz) = (p[0] - self.x, p[1] - self.z);
        let along = dx * fx + dz * fz;
        let across = dx * -fz + dz * fx;
        along.abs() <= self.size[0] * 0.5 && across.abs() <= self.size[2] * 0.5
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Team {
    A,
    B,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Spawn {
    pub team: Team,
    pub x: f64,
    pub z: f64,
    pub yaw: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub name: String,
    /// Playable rectangle: min_x, min_z, max_x, max_z. None = the whole imported play box.
    pub bounds: Option<[f64; 4]>,
    pub objects: Vec<Object>,
    pub spawns: Vec<Spawn>,
    pub next_id: u32,
}

/// One edit. Every change to a scene is an op, from the CLI, the MCP server or (later) the editor UI.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    /// Place one object. `rot` defaults to 0 (length north-south), `size` to the kind's default.
    Place { kind: Kind, x: f64, z: f64, #[serde(default)] rot: f64, #[serde(default)] size: Option<[f64; 3]> },
    /// A row of objects end to end from `from` to `to` (e.g. barriers closing a street).
    Line { kind: Kind, from: [f64; 2], to: [f64; 2], #[serde(default)] gap: f64 },
    /// `count` objects at seeded random spots in `rect`, not overlapping buildings or other objects.
    Scatter { kind: Kind, rect: [f64; 4], count: u32, #[serde(default)] seed: u64 },
    Move { id: u32, #[serde(default)] x: Option<f64>, #[serde(default)] z: Option<f64>, #[serde(default)] rot: Option<f64> },
    Delete { id: u32 },
    /// Delete every object whose centre is inside `rect`.
    DeleteRect { rect: [f64; 4] },
    /// Add a spawn point for a team.
    Spawn { team: Team, x: f64, z: f64, #[serde(default)] yaw: f64 },
    ClearSpawns,
    /// Set the playable rectangle.
    Bounds { rect: [f64; 4] },
}

/// Logged op with provenance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub by: String,
    #[serde(flatten)]
    pub op: Op,
}

/// Static obstacles the scene must respect when scattering (building footprints).
pub trait Obstacles {
    fn blocked(&self, p: [f64; 2]) -> bool;
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

impl Scene {
    fn add(&mut self, kind: Kind, x: f64, z: f64, rot: f64, size: Option<[f64; 3]>) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.objects.push(Object { id, kind, x, z, rot, size: size.unwrap_or(kind.default_size()) });
        id
    }

    /// Applies an op; returns the ids of objects it created.
    pub fn apply(&mut self, op: &Op, obs: &dyn Obstacles) -> Result<Vec<u32>, String> {
        let mut made = vec![];
        match op {
            Op::Place { kind, x, z, rot, size } => made.push(self.add(*kind, *x, *z, *rot, *size)),
            Op::Line { kind, from, to, gap } => {
                let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
                let len = dx.hypot(dz);
                let unit = kind.default_size()[0] + gap.max(0.0);
                let n = (len / unit).round().max(1.0) as usize;
                let rot = dx.atan2(-dz).to_degrees();
                for i in 0..n {
                    let t = (i as f64 + 0.5) / n as f64;
                    made.push(self.add(*kind, from[0] + dx * t, from[1] + dz * t, rot, None));
                }
            }
            Op::Scatter { kind, rect, count, seed } => {
                let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_add(self.next_id as u64);
                let mut tries = 0;
                while (made.len() as u32) < *count && tries < count * 200 {
                    tries += 1;
                    let (x, z) = (rect[0] + lcg(&mut s) * (rect[2] - rect[0]), rect[1] + lcg(&mut s) * (rect[3] - rect[1]));
                    let rot = (lcg(&mut s) * 4.0).floor() * 90.0 + (lcg(&mut s) - 0.5) * 20.0;
                    let cand = Object { id: 0, kind: *kind, x, z, rot, size: kind.default_size() };
                    let pts = cand.corners();
                    if pts.iter().chain([[x, z]].iter()).any(|p| obs.blocked(*p)) || self.objects.iter().any(|o| o.overlaps(&cand, 1.0)) {
                        continue;
                    }
                    made.push(self.add(*kind, x, z, rot, None));
                }
            }
            Op::Move { id, x, z, rot } => {
                let o = self.objects.iter_mut().find(|o| o.id == *id).ok_or(format!("no object {id}"))?;
                o.x = x.unwrap_or(o.x);
                o.z = z.unwrap_or(o.z);
                o.rot = rot.unwrap_or(o.rot);
            }
            Op::Delete { id } => {
                let n = self.objects.len();
                self.objects.retain(|o| o.id != *id);
                if self.objects.len() == n {
                    return Err(format!("no object {id}"));
                }
            }
            Op::DeleteRect { rect } => self.objects.retain(|o| !(o.x >= rect[0] && o.x <= rect[2] && o.z >= rect[1] && o.z <= rect[3])),
            Op::Spawn { team, x, z, yaw } => self.spawns.push(Spawn { team: *team, x: *x, z: *z, yaw: *yaw }),
            Op::ClearSpawns => self.spawns.clear(),
            Op::Bounds { rect } => self.bounds = Some([rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3])]),
        }
        Ok(made)
    }

    pub fn replay(name: &str, log: &[Entry], obs: &dyn Obstacles) -> Scene {
        let mut s = Scene { name: name.into(), ..Default::default() };
        for e in log {
            let _ = s.apply(&e.op, obs);
        }
        s
    }
}

impl Object {
    /// Separating-axis overlap test of two footprints, with extra clearance `pad` metres.
    pub fn overlaps(&self, o: &Object, pad: f64) -> bool {
        let (a, b) = (self.corners(), o.corners());
        for poly in [&a, &b] {
            for i in 0..4 {
                let (p, q) = (poly[i], poly[(i + 1) % 4]);
                let n = [q[1] - p[1], p[0] - q[0]];
                let l = n[0].hypot(n[1]);
                let n = [n[0] / l, n[1] / l];
                let proj = |c: &[[f64; 2]; 4]| c.iter().map(|v| v[0] * n[0] + v[1] * n[1]).fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)));
                let (a0, a1) = proj(&a);
                let (b0, b1) = proj(&b);
                if a1 + pad < b0 || b1 + pad < a0 {
                    return false;
                }
            }
        }
        true
    }
}

/// Files for a named scene under `dir`.
pub struct Store {
    pub dir: PathBuf,
    pub name: String,
}

impl Store {
    pub fn new(dir: &Path, name: &str) -> Self {
        Self { dir: dir.into(), name: name.into() }
    }
    pub fn log_path(&self) -> PathBuf {
        self.dir.join(format!("{}.ops.jsonl", self.name))
    }
    pub fn snapshot_path(&self) -> PathBuf {
        self.dir.join(format!("{}.json", self.name))
    }
    pub fn log(&self) -> Res<Vec<Entry>> {
        let Ok(t) = std::fs::read_to_string(self.log_path()) else { return Ok(vec![]) };
        t.lines().filter(|l| !l.trim().is_empty()).map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}: {l}", self.log_path().display()).into())).collect()
    }
    fn write(&self, log: &[Entry], scene: &Scene) -> Res<()> {
        std::fs::create_dir_all(&self.dir)?;
        let body: String = log.iter().map(|e| serde_json::to_string(e).unwrap() + "\n").collect();
        std::fs::write(self.log_path(), body)?;
        std::fs::write(self.snapshot_path(), serde_json::to_string_pretty(scene)? + "\n")?;
        Ok(())
    }
    pub fn load(&self, obs: &dyn Obstacles) -> Res<Scene> {
        Ok(Scene::replay(&self.name, &self.log()?, obs))
    }
    /// Applies ops atomically: if any fails, nothing is written. Returns the new scene and the created ids per op.
    pub fn commit(&self, ops: &[Op], by: &str, obs: &dyn Obstacles) -> Res<(Scene, Vec<Vec<u32>>)> {
        let mut log = self.log()?;
        let mut scene = Scene::replay(&self.name, &log, obs);
        let mut made = vec![];
        for op in ops {
            made.push(scene.apply(op, obs).map_err(|e| format!("{e} (in {})", serde_json::to_string(op).unwrap()))?);
            log.push(Entry { by: by.into(), op: op.clone() });
        }
        self.write(&log, &scene)?;
        Ok((scene, made))
    }
    /// Drops the last `n` ops.
    pub fn undo(&self, n: usize, obs: &dyn Obstacles) -> Res<Scene> {
        let mut log = self.log()?;
        log.truncate(log.len().saturating_sub(n));
        let scene = Scene::replay(&self.name, &log, obs);
        self.write(&log, &scene)?;
        Ok(scene)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Open;
    impl Obstacles for Open {
        fn blocked(&self, _: [f64; 2]) -> bool {
            false
        }
    }

    #[test]
    fn line_fills_the_span_end_to_end() {
        let mut s = Scene::default();
        let ids = s.apply(&Op::Line { kind: Kind::Barrier, from: [0.0, 0.0], to: [30.0, 0.0], gap: 0.0 }, &Open).unwrap();
        assert_eq!(ids.len(), 10);
        assert!(s.objects.iter().all(|o| (o.rot - 90.0).abs() < 1e-9), "length should run east-west");
        assert!(!s.objects[0].overlaps(&s.objects[1], -0.01));
    }

    #[test]
    fn scatter_is_deterministic_and_non_overlapping() {
        let op = Op::Scatter { kind: Kind::Car, rect: [0.0, 0.0, 60.0, 60.0], count: 15, seed: 7 };
        let (mut a, mut b) = (Scene::default(), Scene::default());
        a.apply(&op, &Open).unwrap();
        b.apply(&op, &Open).unwrap();
        assert_eq!(a.objects, b.objects);
        assert_eq!(a.objects.len(), 15);
        for (i, x) in a.objects.iter().enumerate() {
            for y in &a.objects[i + 1..] {
                assert!(!x.overlaps(y, 0.0));
            }
        }
    }

    #[test]
    fn failed_op_writes_nothing_and_undo_replays() {
        let dir = std::env::temp_dir().join(format!("fps-editor-test-{}", std::process::id()));
        let st = Store::new(&dir, "t");
        st.commit(&[Op::Place { kind: Kind::Crate, x: 1.0, z: 2.0, rot: 0.0, size: None }], "test", &Open).unwrap();
        assert!(st.commit(&[Op::Place { kind: Kind::Crate, x: 5.0, z: 5.0, rot: 0.0, size: None }, Op::Delete { id: 99 }], "test", &Open).is_err());
        assert_eq!(st.log().unwrap().len(), 1, "a failing batch must not be logged");
        st.commit(&[Op::Move { id: 1, x: Some(9.0), z: None, rot: None }], "test", &Open).unwrap();
        assert_eq!(st.load(&Open).unwrap().objects[0].x, 9.0);
        assert_eq!(st.undo(1, &Open).unwrap().objects[0].x, 1.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rotated_footprint_contains_its_corners_region() {
        let o = Object { id: 1, kind: Kind::Bus, x: 0.0, z: 0.0, rot: 90.0, size: Kind::Bus.default_size() };
        assert!(o.contains([5.0, 0.0]) && !o.contains([0.0, 5.0]));
    }
}
