//! Downtown San Diego: OSM + USGS import, the city model, and static meshes built from it.
pub mod geo;
pub mod import;
pub mod lidar;
pub mod mesh;
pub mod model;

pub use model::City;

impl City {
    pub fn load(path: &std::path::Path) -> Result<City, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }
}
