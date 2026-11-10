use crate::geo::{Frame, LatLonBox};
use serde::{Deserialize, Serialize};

pub type Ring = Vec<[f64; 2]>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub osm: String,
    pub outer: Ring,
    pub holes: Vec<Ring>,
    pub height: f64,
    pub min_height: f64,
    /// Where the height came from: "height", "levels", "default".
    pub height_src: String,
    pub kind: String,
    pub name: Option<String>,
    /// Ground height under the building (relative to home plate) when measured by lidar.
    #[serde(default)]
    pub base_y: Option<f64>,
    /// For an upper tier: the building it stands on.
    #[serde(default)]
    pub part_of: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineKind {
    Road,
    Service,
    Path,
    Rail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Line {
    pub kind: LineKind,
    pub tag: String,
    pub width: f64,
    pub pts: Vec<[f64; 2]>,
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AreaKind {
    Grass,
    Pitch,
    Parking,
    Plaza,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Area {
    pub kind: AreaKind,
    pub outer: Ring,
}

/// Bare-earth heights on a regular grid, metres relative to home plate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Terrain {
    /// x/z rect covered: min_x, min_z, max_x, max_z.
    pub rect: [f64; 4],
    pub w: usize,
    pub h: usize,
    pub y: Vec<f32>,
}

impl Terrain {
    pub fn flat(rect: [f64; 4]) -> Self {
        Self { rect, w: 2, h: 2, y: vec![0.0; 4] }
    }
    pub fn at(&self, x: f64, z: f64) -> f64 {
        let fx = ((x - self.rect[0]) / (self.rect[2] - self.rect[0]) * (self.w - 1) as f64).clamp(0.0, (self.w - 1) as f64);
        let fz = ((z - self.rect[1]) / (self.rect[3] - self.rect[1]) * (self.h - 1) as f64).clamp(0.0, (self.h - 1) as f64);
        let (ix, iz) = ((fx as usize).min(self.w - 2), (fz as usize).min(self.h - 2));
        let (tx, tz) = (fx - ix as f64, fz - iz as f64);
        let g = |i: usize, j: usize| self.y[j * self.w + i] as f64;
        let a = g(ix, iz) * (1.0 - tx) + g(ix + 1, iz) * tx;
        let b = g(ix, iz + 1) * (1.0 - tx) + g(ix + 1, iz + 1) * tx;
        a * (1.0 - tz) + b * tz
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct City {
    pub frame: Frame,
    pub play_box: LatLonBox,
    pub import_box: LatLonBox,
    /// Elevation of home plate above NAVD88, metres (terrain y is relative to it).
    pub origin_elev: f64,
    pub terrain: Terrain,
    pub buildings: Vec<Building>,
    pub lines: Vec<Line>,
    pub areas: Vec<Area>,
    pub trees: Vec<[f64; 2]>,
    /// Tree heights above ground, parallel to `trees` (empty: default size).
    #[serde(default)]
    pub tree_h: Vec<f64>,
    pub attribution: String,
}

impl City {
    pub fn play_rect(&self) -> [f64; 4] {
        self.play_box.xz_rect(&self.frame)
    }
    pub fn import_rect(&self) -> [f64; 4] {
        self.import_box.xz_rect(&self.frame)
    }
}
