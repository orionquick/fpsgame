use serde::{Deserialize, Serialize};

/// Local tangent-plane frame: x = east (m), y = up (m), z = south (m), origin at home plate.
/// Linear in lat/lon, so an aerial image fetched as an equirectangular lat/lon raster lines up exactly.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub lat0: f64,
    pub lon0: f64,
    pub m_per_deg_lat: f64,
    pub m_per_deg_lon: f64,
}

impl Frame {
    pub fn new(lat0: f64, lon0: f64) -> Self {
        let p = lat0.to_radians();
        Self {
            lat0,
            lon0,
            m_per_deg_lat: 111_132.92 - 559.82 * (2.0 * p).cos() + 1.175 * (4.0 * p).cos(),
            m_per_deg_lon: 111_412.84 * p.cos() - 93.5 * (3.0 * p).cos(),
        }
    }
    pub fn to_xz(&self, lat: f64, lon: f64) -> [f64; 2] {
        [(lon - self.lon0) * self.m_per_deg_lon, -(lat - self.lat0) * self.m_per_deg_lat]
    }
    pub fn to_latlon(&self, x: f64, z: f64) -> (f64, f64) {
        (self.lat0 - z / self.m_per_deg_lat, self.lon0 + x / self.m_per_deg_lon)
    }
}

/// Lat/lon box: west, south, east, north.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LatLonBox {
    pub w: f64,
    pub s: f64,
    pub e: f64,
    pub n: f64,
}

impl LatLonBox {
    pub fn grow_m(&self, f: &Frame, m: f64) -> Self {
        let dlat = m / f.m_per_deg_lat;
        let dlon = m / f.m_per_deg_lon;
        Self { w: self.w - dlon, s: self.s - dlat, e: self.e + dlon, n: self.n + dlat }
    }
    /// x/z extents in the frame: (min_x, min_z, max_x, max_z).
    pub fn xz_rect(&self, f: &Frame) -> [f64; 4] {
        let a = f.to_xz(self.n, self.w);
        let b = f.to_xz(self.s, self.e);
        [a[0], a[1], b[0], b[1]]
    }
}

/// Home plate at the San Diego ballpark (approx: ~18 m in from the backstop curve of the OSM field polygon).
pub const HOME_PLATE: (f64, f64) = (32.70712, -117.15704);

/// The play area: 4th Ave → 14th St, Imperial Ave → Market St (ballpark, south Gaslamp, south East Village).
pub const PLAY_BOX: LatLonBox = LatLonBox { w: -117.1611, s: 32.7060, e: -117.1519, n: 32.7117 };

/// Extra context imported around the play box, metres.
pub const MARGIN_M: f64 = 120.0;
