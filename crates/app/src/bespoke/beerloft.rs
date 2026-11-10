//! The ground-floor bar lounge, built to match the interior photo of the converted warehouse's beer hall
//! (out/ref/decor.jpg): a freestanding tiled back-bar wall with glass-door cabinets, TVs and a marquee-letter
//! sign, a long counter with a cognac woven front and concrete top, rope pendants looping in front of it,
//! rose-painted spiral ducts and two-tier ring chandeliers overhead, and a lounge on a stencilled concrete rug:
//! black tufted sofa, black drum coffee table on a brass plinth, cognac sling chairs, woven poufs, cowhide drums,
//! a brass tray table, tan wingbacks and high-tops by the bar.
//!
//! Local plan coordinates (x east, z south); the bar faces west, so a visitor in the lounge looks east at it with
//! the south windows on the right, as in the photo.

use super::furnish::Furnisher;
use super::geom::{wear, Geo, Key, Mat};
use super::plan::Rect;
use glam::{Vec2, Vec3};

pub struct Site {
    /// Finished floor level and the underside of the deck above.
    pub y: f32,
    pub deck_b: f32,
    /// Column lines (x) and girder lines (z).
    pub cols: Vec<f32>,
    pub girders: Vec<f32>,
    pub core: Rect,
}

/// Face of the freestanding back-bar wall (x), and the bar's extent along it (z).
const BAR_WALL: f32 = 18.0;
const BAR_Z: (f32, f32) = (8.6, 20.2);
/// The tiled wall runs on north past the bar (as in the photo), leaving a passage at the north end.
const WALL_Z0: f32 = 3.6;
const WALL_H: f32 = 4.2;
/// The big duct runs along the bar over the counter; the rope lights hang between it and the back bar.
const DUCT_X: f32 = BAR_WALL - 1.1;
const ROPE_X: f32 = BAR_WALL - 0.47;
/// Bar stools as in the photo: round cognac leather seat with a low curved back, dark legs and footring.
const STOOL: &str = "sf_8cff98e6d1";
/// Brown faux leather seat, the dark legs darkened with it.
const COGNAC: [f32; 3] = [0.42, 0.27, 0.17];
/// The stencilled rug under the lounge (x0, z0, x1, z1): between the column lines, 7.2 x 5.4 m.
const RUG: (f32, f32, f32, f32) = (8.0, 11.2, 15.2, 16.6);

fn ins(k: Key) -> Key {
    k.inside()
}
fn charcoal() -> Key {
    ins(Key::new(Mat::Plain(110), [0.075, 0.078, 0.082]).wear(wear::METAL))
}
fn brass() -> Key {
    ins(Key::new(Mat::Plain(60), [0.62, 0.45, 0.2]))
}
fn walnut() -> Key {
    // dark walnut, as the chair frames and trim in the photo
    ins(Key::new(Mat::Surface("pht_black_walnut_veneer_01"), [0.24, 0.15, 0.11]).tile(1.2).rough(0.6))
}
/// Medium-brown stained oak for the back-bar surround (as in the photo).
fn oak() -> Key {
    ins(Key::new(Mat::Surface("pht_black_walnut_veneer_01"), [0.34, 0.28, 0.22]).tile(1.4).rough(0.6))
}
fn concrete_top() -> Key {
    ins(Key::new(Mat::Surface(super::m::CONCRETE), [0.62, 0.62, 0.6]).rough(0.7))
}
fn glass() -> Key {
    Key::new(Mat::Glass, [0.7, 0.74, 0.72])
}
fn bulb() -> Key {
    Key::new(Mat::Light, [1.8, 1.25, 0.62])
}
fn black() -> Key {
    ins(Key::new(Mat::Plain(130), [0.02, 0.02, 0.022]))
}

/// Everything on the bar floor.
pub fn build(g: &mut Geo, f: &mut Furnisher, s: &Site) {
    bar_wall(g, f, s);
    counter(g, f, s);
    rope_lights(g, s);
    lounge(g, f, s);
    chandeliers(g, s);
    ducts(g, s);
    track_lights(g, s);
    rug(g, s);
}

/// Places a model with a tint and uniform scale.
fn put(f: &mut Furnisher, g: &mut Geo, id: &str, c: Vec2, y: f32, face: Vec2, scale: f32, tint: [f32; 3], solid: bool) {
    let n = f.out.len();
    f.put(g, id, c, y, face, solid);
    if let Some(p) = f.out.get_mut(n) {
        p.scale = [scale; 3];
        p.tint = tint;
    }
}

fn v(x: f32, z: f32) -> Vec2 {
    Vec2::new(x, z)
}

/// Unit plan direction at `deg` degrees from +x towards +z.
fn dir(deg: f32) -> Vec2 {
    let r = deg.to_radians();
    v(r.cos(), r.sin())
}

// ---------------------------------------------------------------------------------------------------------------
// back bar

fn bar_wall(g: &mut Geo, f: &mut Furnisher, s: &Site) {
    let y = s.y;
    let (z0, z1) = BAR_Z;
    let x = BAR_WALL;
    let wz0 = WALL_Z0;
    // the wall: painted block body, glazed white subway tile on the bar side up to the ducts
    let body = ins(Key::new(Mat::Plain(200), [0.8, 0.79, 0.76]));
    g.cuboid(body, Vec3::new(x, y, wz0), Vec3::new(x + 0.22, y + WALL_H, z1));
    g.block_aabb(Vec3::new(x, y, wz0), Vec3::new(x + 0.22, y + WALL_H, z1));
    let tile = ins(Key::new(Mat::Surface("pt_subway_tile"), [0.8, 0.78, 0.74]));
    g.quad(tile, Vec3::new(x - 0.004, y, z1), Vec3::NEG_Z * (z1 - wz0), Vec3::Y * WALL_H);
    // charcoal cap and end trims
    g.cuboid(charcoal(), Vec3::new(x - 0.03, y + WALL_H, wz0 - 0.03), Vec3::new(x + 0.25, y + WALL_H + 0.06, z1 + 0.03));
    // a framed photo print on the tiled stretch north of the bar
    let print = ins(Key::new(Mat::Plain(200), [0.42, 0.41, 0.39]));
    g.cuboid(charcoal(), Vec3::new(x - 0.04, y + 1.0, wz0 + 0.8), Vec3::new(x, y + 2.9, wz0 + 3.0));
    g.quad(print, Vec3::new(x - 0.045, y + 1.08, wz0 + 2.92), Vec3::NEG_Z * 2.04, Vec3::Y * 1.74);
    for zz in [wz0 - 0.03, z1 - 0.02] {
        g.cuboid(charcoal(), Vec3::new(x - 0.03, y, zz), Vec3::new(x + 0.25, y + WALL_H, zz + 0.05));
    }

    // back counter: charcoal cupboards under a concrete top, the full length
    let bx = x - 0.62;
    g.cuboid(charcoal(), Vec3::new(bx, y, z0 + 0.1), Vec3::new(x, y + 0.9, z1 - 0.1));
    g.cuboid(concrete_top(), Vec3::new(bx - 0.03, y + 0.9, z0 + 0.08), Vec3::new(x, y + 0.95, z1 - 0.08));
    // cupboard door lines and pulls
    let mut zz = z0 + 0.1;
    while zz < z1 - 0.6 {
        g.cuboid(black(), Vec3::new(bx - 0.006, y + 0.1, zz + 0.595), Vec3::new(bx, y + 0.86, zz + 0.605));
        g.cuboid(brass(), Vec3::new(bx - 0.03, y + 0.72, zz + 0.52), Vec3::new(bx - 0.01, y + 0.74, zz + 0.56));
        zz += 0.6;
    }

    // the back-bar unit, as in the photo: a big wood surround over the back counter, split by a full-width
    // shelf; below it two charcoal cabinets with paned glass doors and two TVs between them; above it the
    // marquee letters with a TV in each corner; a neon box and dome cameras on the tile above
    let centre = (z0 + z1) * 0.5;
    let (fz0, fz1) = (centre - 4.65, centre + 4.65);
    let (fy0, fy1) = (y + 0.95, y + 3.45);
    let shelf = y + 2.15;
    let wf = oak();
    let d = 0.3; // depth of the surround off the tile
    g.timber(wf, Vec3::new(x - d, fy1 - 0.2, fz0), Vec3::new(x, fy1, fz1));
    g.timber(wf, Vec3::new(x - d, fy0, fz0), Vec3::new(x, fy1, fz0 + 0.2));
    g.timber(wf, Vec3::new(x - d, fy0, fz1 - 0.2), Vec3::new(x, fy1, fz1));
    g.timber(wf, Vec3::new(x - d - 0.04, shelf - 0.05, fz0), Vec3::new(x, shelf + 0.04, fz1));
    // posts either side of the centre bay, and a plinth rail along the counter
    // proportions measured off the photo (frame ~9.3 m): open shelves 0.65 m at the ends, cabinets 1.8 m,
    // a 3.75 m centre bay between posts
    for zc in [centre - 1.88, centre + 1.88] {
        g.timber(wf, Vec3::new(x - d, fy0, zc - 0.06), Vec3::new(x, shelf, zc + 0.06));
    }
    g.timber(wf, Vec3::new(x - d, fy0, fz0), Vec3::new(x, fy0 + 0.08, fz1));
    // charcoal cabinets with paned glass doors in the side bays
    for (ca, cb) in [(fz0 + 0.87, centre - 1.94), (centre + 1.94, fz1 - 0.87)] {
        cabinet(g, f, x, ca, cb, fy0 + 0.08, shelf - 0.05);
    }
    // open shelves with cans at the outer ends of the lower bays
    for (sa, sb) in [(fz0 + 0.2, fz0 + 0.83), (fz1 - 0.83, fz1 - 0.2)] {
        g.timber(wf, Vec3::new(x - d, fy0, sb), Vec3::new(x, shelf, sb + 0.04));
        // wooden backing behind the open shelves
        g.timber(wf, Vec3::new(x - 0.03, fy0, sa), Vec3::new(x - 0.005, shelf, sb));
        for k in 1..3 {
            let sy = fy0 + 0.08 + (shelf - fy0 - 0.13) * k as f32 / 3.0;
            g.timber(wf, Vec3::new(x - d, sy, sa), Vec3::new(x, sy + 0.03, sb));
        }
        for k in 0..3 {
            let sy = fy0 + 0.08 + (shelf - fy0 - 0.13) * k as f32 / 3.0 + if k == 0 { 0.0 } else { 0.03 };
            let mut zz = sa + 0.08;
            let mut j = 0;
            while zz < sb - 0.05 {
                let col = [[0.55, 0.45, 0.08], [0.05, 0.12, 0.35], [0.6, 0.6, 0.62], [0.45, 0.05, 0.04]][(j + k) % 4];
                let can = ins(Key::new(Mat::Plain(60), col));
                g.tube(can, Vec3::new(x - 0.15, sy, zz), Vec3::new(x - 0.15, sy + 0.12, zz), 0.033, 10);
                zz += 0.09;
                j += 1;
            }
        }
    }
    // two TVs side by side in the centre bay, one in each upper corner
    // big screens, as in the photo: two ~1.57 m side by side in the centre bay, ~1.7 m in each upper corner
    for (zc, yc, scale) in [(centre - 0.87, y + 1.58, 1.76), (centre + 0.87, y + 1.58, 1.76), (fz0 + 1.25, y + 2.72, 1.95), (fz1 - 1.25, y + 2.72, 1.95)] {
        // mounted forward, inside the depth of the surround
        tv_scaled(g, f, Vec3::new(x - d + 0.06, yc, zc), v(-1.0, 0.0), scale);
    }
    marquee(g, "TAP ROOM", x - 0.04, centre, shelf + 0.17, 0.095);
    // neon box above the surround (generic lettering), dome cameras on the tile
    // neon box resting on the frame top, a little right of centre; dome cameras on brackets on the tile above
    neon_box(g, x - 0.05, centre + 0.5, fy1 - 0.1);
    for zc in [fz0 + 0.25, centre - 2.7, centre + 2.6, fz1 - 0.3] {
        let p = Vec3::new(x - 0.12, y + 3.75, zc);
        g.cuboid(ins(Key::new(Mat::Plain(90), [0.7, 0.7, 0.69])), Vec3::new(x - 0.2, y + 3.78, zc - 0.07), Vec3::new(x, y + 3.82, zc + 0.07));
        frustum(g, ins(Key::new(Mat::Plain(40), [0.08, 0.08, 0.085])), p, p - Vec3::new(0.0, 0.1, 0.0), 0.075, 0.03, 14);
    }

    // tap towers on the back counter
    for zc in [z0 + 1.0, centre - 1.6, centre + 1.6, z1 - 1.0] {
        put(f, g, "sf_f79b73c0eb", v(bx + 0.13, zc), y + 0.95, v(-1.0, 0.0), 1.0, [1.0; 3], false);
    }
    // bar lights: warm wash on the tile and the counter
    for zc in [z0 + 1.5, centre - 2.5, centre, centre + 2.5, z1 - 1.5] {
        g.downlight(Vec3::new(x - 0.8, y + 2.9, zc), 4.0, [0.5, 0.36, 0.2]);
    }
}

/// A charcoal cabinet against the wall at `x`: two doors with glass in three panes each, dark inside with
/// shelves of bottles just showing through.
fn cabinet(g: &mut Geo, _f: &mut Furnisher, x: f32, z0: f32, z1: f32, y0: f32, y1: f32) {
    let d = 0.34;
    let c = charcoal();
    let xf = x - d;
    g.cuboid(c, Vec3::new(xf, y0, z0), Vec3::new(x, y1, z0 + 0.05));
    g.cuboid(c, Vec3::new(xf, y0, z1 - 0.05), Vec3::new(x, y1, z1));
    g.cuboid(c, Vec3::new(xf, y1 - 0.05, z0), Vec3::new(x, y1, z1));
    g.cuboid(c, Vec3::new(xf, y0, z0), Vec3::new(x, y0 + 0.05, z1));
    g.quad(ins(Key::new(Mat::Plain(160), [0.03, 0.03, 0.03])), Vec3::new(x - 0.01, y0, z1), Vec3::NEG_Z * (z1 - z0), Vec3::Y * (y1 - y0));
    let zm = (z0 + z1) * 0.5;
    for k in 1..3 {
        let sy = y0 + (y1 - y0) * k as f32 / 3.0;
        g.cuboid(c, Vec3::new(xf + 0.03, sy, z0 + 0.05), Vec3::new(x - 0.02, sy + 0.02, z1 - 0.05));
        let n = ((z1 - z0) / 0.16) as i32;
        for j in 0..n {
            let zc = z0 + 0.12 + j as f32 * (z1 - z0 - 0.24) / (n - 1).max(1) as f32;
            bottle(g, Vec3::new(x - d * 0.5, sy + 0.02, zc), (j + k) as usize % 3);
        }
    }
    g.downlight(Vec3::new(x - 0.2, y1 - 0.15, zm), 1.2, [0.1, 0.08, 0.05]);
    // doors: charcoal stiles and rails, three panes of dark glass each
    let glass = ins(Key::new(Mat::Plain(70), [0.03, 0.032, 0.034]));
    for (a, b) in [(z0 + 0.05, zm - 0.01), (zm + 0.01, z1 - 0.05)] {
        for zz in [a, b - 0.05] {
            g.cuboid(c, Vec3::new(xf - 0.035, y0 + 0.05, zz), Vec3::new(xf, y1 - 0.05, zz + 0.05));
        }
        for k in 0..4 {
            let yy = y0 + 0.05 + (y1 - y0 - 0.14) * k as f32 / 3.0;
            g.cuboid(c, Vec3::new(xf - 0.035, yy, a), Vec3::new(xf, yy + 0.04, b));
        }
        // the glass: a glossy dark film the room reflects in, set back a little
        g.quad(glass, Vec3::new(xf - 0.015, y0 + 0.05, a), Vec3::Z * (b - a), Vec3::Y * (y1 - y0 - 0.1));
        let pz = if a < zm { b - 0.1 } else { a + 0.07 };
        g.cuboid(brass(), Vec3::new(xf - 0.055, y0 + 0.3, pz), Vec3::new(xf - 0.035, y0 + 0.62, pz + 0.025));
    }
}

/// A neon sign box over the bar: a deep red cabinet with white-hot red tube lettering ("COLD BEER", generic).
fn neon_box(g: &mut Geo, x: f32, zc: f32, y0: f32) {
    let (w, h) = (1.95, 0.6);
    let red = ins(Key::new(Mat::Plain(80), [0.3, 0.02, 0.02]));
    g.cuboid(red, Vec3::new(x - 0.14, y0, zc - w / 2.0), Vec3::new(x, y0 + h, zc + w / 2.0));
    // the lit red face (one-sided, facing the room) and white-hot tube lettering on it
    g.quad(Key::new(Mat::Light, [0.32, 0.02, 0.015]), Vec3::new(x - 0.142, y0 + 0.03, zc - w / 2.0 + 0.03), Vec3::Z * (w - 0.06), Vec3::Y * (h - 0.06));
    let tube = Key::new(Mat::Light, [2.2, 1.6, 1.5]);
    // simple stroke lettering: each letter a few straight tubes on a 3 x 5 grid
    let strokes: &[(char, &[[f32; 4]])] = &[
        ('C', &[[2.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 4.0], [0.0, 4.0, 2.0, 4.0]]),
        ('O', &[[0.0, 0.0, 2.0, 0.0], [2.0, 0.0, 2.0, 4.0], [2.0, 4.0, 0.0, 4.0], [0.0, 4.0, 0.0, 0.0]]),
        ('L', &[[0.0, 0.0, 0.0, 4.0], [0.0, 4.0, 2.0, 4.0]]),
        ('D', &[[0.0, 0.0, 0.0, 4.0], [0.0, 0.0, 1.4, 0.0], [1.4, 0.0, 2.0, 0.8], [2.0, 0.8, 2.0, 3.2], [2.0, 3.2, 1.4, 4.0], [1.4, 4.0, 0.0, 4.0]]),
        ('B', &[[0.0, 0.0, 0.0, 4.0], [0.0, 0.0, 1.6, 0.0], [1.6, 0.0, 1.6, 2.0], [0.0, 2.0, 2.0, 2.0], [2.0, 2.0, 2.0, 4.0], [2.0, 4.0, 0.0, 4.0]]),
        ('E', &[[2.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 4.0], [0.0, 4.0, 2.0, 4.0], [0.0, 2.0, 1.5, 2.0]]),
        ('R', &[[0.0, 4.0, 0.0, 0.0], [0.0, 0.0, 2.0, 0.0], [2.0, 0.0, 2.0, 2.0], [2.0, 2.0, 0.0, 2.0], [0.8, 2.0, 2.0, 4.0]]),
    ];
    let text = "COLD BEER";
    let unit = 0.07;
    let total = text.chars().count() as f32 * 3.0 * unit;
    let z_start = zc - total / 2.0;
    for (i, ch) in text.chars().enumerate() {
        let Some((_, segs)) = strokes.iter().find(|(c, _)| *c == ch) else { continue };
        let oz = z_start + i as f32 * 3.0 * unit;
        for sgm in segs.iter() {
            let pa = Vec3::new(x - 0.16, y0 + h - 0.1 - sgm[1] * unit, oz + sgm[0] * unit);
            let pb = Vec3::new(x - 0.16, y0 + h - 0.1 - sgm[3] * unit, oz + sgm[2] * unit);
            g.tube(tube, pa, pb, 0.009, 6);
        }
    }
    g.downlight(Vec3::new(x - 0.5, y0 + h * 0.5, zc), 2.0, [0.25, 0.02, 0.01]);
}

/// A liquor bottle standing at `p`: amber, green or clear glass, a dark cap.
fn bottle(g: &mut Geo, p: Vec3, kind: usize) {
    let col = [[0.16, 0.08, 0.025], [0.05, 0.1, 0.05], [0.32, 0.3, 0.26]][kind % 3];
    let glass = ins(Key::new(Mat::Plain(20), col));
    let h = [0.24, 0.27, 0.22][kind % 3];
    g.tube(glass, p, p + Vec3::Y * (h * 0.66), 0.036, 10);
    g.tube(glass, p + Vec3::Y * (h * 0.66), p + Vec3::Y * (h * 0.78), 0.024, 8);
    g.tube(glass, p + Vec3::Y * (h * 0.78), p + Vec3::Y * h, 0.012, 6);
    g.tube(black(), p + Vec3::Y * h, p + Vec3::Y * (h + 0.02), 0.013, 6);
    disc(g, glass, p + Vec3::Y * (h * 0.66), 0.036, 10);
}

/// A wall TV centred at `c` facing `face`, showing the ball-game picture.
fn tv(g: &mut Geo, f: &mut Furnisher, c: Vec3, face: Vec2) {
    tv_scaled(g, f, c, face, 1.25)
}

/// `tv` at a given scale of the TV model (1.0 = 1.06 x 0.65 m).
fn tv_scaled(g: &mut Geo, f: &mut Furnisher, c: Vec3, face: Vec2, scale: f32) {
    let n = face.normalize();
    let k = scale / 1.25; // the catalog already scales the model 1.25x
    let (hw, hh) = (0.48 * scale, 0.272 * scale);
    let bottom = c.y - 0.325 * scale;
    put(f, g, "sf_ce05a5d98b", v(c.x, c.z), bottom, n, k, [1.0; 3], false);
    let nn = Vec3::new(n.x, 0.0, n.y);
    // the screen: the picture stretched once over the panel (emissive, wound to face the room)
    let r = Vec3::Y.cross(nn).normalize() * (2.0 * hw);
    let o = Vec3::new(c.x, c.y - hh, c.z) + nn * (0.061 * scale) - r * 0.5;
    // screen brightness for an interior exposure (~8x)
    let screen = Key::new(Mat::Screen("scr_ballgame"), [0.32, 0.32, 0.32]).tile(1.0);
    g.decal(screen, o, r, Vec3::Y * (2.0 * hh), 1.0);
}

/// Marquee letters on the sign: cream metal letter bodies with a warm bulb at every pixel of a 5x7 font, the
/// text centred on `zc` (reading left to right when facing the wall from the west), bottom at `y0`.
fn marquee(g: &mut Geo, text: &str, x: f32, zc: f32, y0: f32, px: f32) {
    let glyph = |c: char| -> [&'static str; 7] {
        match c {
            'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
            'M' => ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
            'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
            'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
            'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
            'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
            _ => ["....."; 7],
        }
    };
    // gold-painted metal letter bodies, as in the photo
    let body = ins(Key::new(Mat::Plain(70), [0.62, 0.45, 0.2]));
    let n = text.chars().count() as f32;
    let width = (n * 6.0 - 1.0) * px;
    // seen from the west (facing east), left to right runs towards +z
    let z_start = zc - width / 2.0;
    for (i, ch) in text.chars().enumerate() {
        for (r, row) in glyph(ch).iter().enumerate() {
            for (c, b) in row.bytes().enumerate() {
                if b != b'#' {
                    continue;
                }
                let zz = z_start + (i as f32 * 6.0 + c as f32 + 0.5) * px;
                let yy = y0 + (6 - r) as f32 * px + px * 0.5;
                g.cuboid(body, Vec3::new(x - 0.1, yy - px * 0.5, zz - px * 0.5), Vec3::new(x, yy + px * 0.5, zz + px * 0.5));
                g.cuboid(Key::new(Mat::Light, [1.1, 0.8, 0.44]), Vec3::new(x - 0.15, yy - 0.024, zz - 0.024), Vec3::new(x - 0.1, yy + 0.024, zz + 0.024));
            }
        }
    }
    g.downlight(Vec3::new(x - 0.4, y0 + 3.5 * px, zc), 3.0, [0.22, 0.15, 0.08]);
}

// ---------------------------------------------------------------------------------------------------------------
// bar counter

fn counter(g: &mut Geo, f: &mut Furnisher, s: &Site) {
    let y = s.y;
    let (z0, z1) = (BAR_Z.0 + 0.4, BAR_Z.1 - 0.4);
    let x1 = BAR_WALL - 0.62 - 0.95; // bartender's side
    let x0 = x1 - 0.7; // customer face
    let top = y + 1.07;
    // body: charcoal plinth, cognac woven panels in charcoal frames, concrete top with an overhang
    g.cuboid(charcoal(), Vec3::new(x0 + 0.04, y, z0), Vec3::new(x1, y + 0.12, z1));
    g.block_aabb(Vec3::new(x0, y, z0), Vec3::new(x1, top, z1));
    let weave = ins(Key::new(Mat::Surface("pt_basket_weave"), [0.88, 0.52, 0.3]).rough(0.9));
    let bay = 1.2;
    let n = ((z1 - z0) / bay).round().max(1.0) as i32;
    let step = (z1 - z0) / n as f32;
    for i in 0..n {
        let (a, b) = (z0 + i as f32 * step, z0 + (i + 1) as f32 * step);
        g.quad(weave, Vec3::new(x0, y + 0.14, b - 0.03), Vec3::NEG_Z * (b - a - 0.06), Vec3::Y * (top - y - 0.22));
        g.cuboid(charcoal(), Vec3::new(x0 - 0.01, y + 0.12, a - 0.015), Vec3::new(x0 + 0.03, top - 0.06, a + 0.015));
    }
    g.cuboid(charcoal(), Vec3::new(x0 - 0.01, y + 0.12, z1 - 0.015), Vec3::new(x0 + 0.03, top - 0.06, z1 + 0.015));
    g.cuboid(charcoal(), Vec3::new(x0 - 0.01, top - 0.08, z0), Vec3::new(x0 + 0.03, top - 0.05, z1));
    g.cuboid(charcoal(), Vec3::new(x0 + 0.03, y + 0.12, z0), Vec3::new(x1, top - 0.05, z0 + 0.04));
    g.cuboid(charcoal(), Vec3::new(x0 + 0.03, y + 0.12, z1 - 0.04), Vec3::new(x1, top - 0.05, z1));
    g.cuboid(concrete_top(), Vec3::new(x0 - 0.06, top - 0.05, z0 - 0.06), Vec3::new(x1 + 0.03, top, z1 + 0.06));
    // brass foot rail on brackets
    let rx = x0 - 0.16;
    g.tube(brass(), Vec3::new(rx, y + 0.22, z0 + 0.05), Vec3::new(rx, y + 0.22, z1 - 0.05), 0.025, 10);
    let mut zz = z0 + 0.4;
    while zz < z1 {
        g.cuboid(brass(), Vec3::new(rx, y + 0.2, zz - 0.012), Vec3::new(x0, y + 0.235, zz + 0.012));
        zz += 1.2;
    }
    // stools along the front, clear of the columns, each turned and pulled out a little differently
    let cx = s.cols.iter().copied().filter(|&c| c < x0).fold(f32::MIN, f32::max);
    let mut zz = z0 + 0.4;
    let mut k = 0u64;
    while zz < z1 - 0.3 {
        let near_col = s.girders.iter().any(|&gz| (gz - zz).abs() < 0.45) && (x0 - 0.45 - cx).abs() < 0.6;
        if !near_col {
            let r = super::rnd(0xBA5, k);
            let pull = 0.3 + r * 0.14;
            let turn = (r - 0.5) * 30.0;
            put(f, g, STOOL, v(x0 - pull, zz + (r - 0.5) * 0.06), y, dir(turn), 1.0, COGNAC, true);
        }
        zz += 0.74;
        k += 1;
    }
    // things on the bar
    for (i, zc) in [z0 + 1.3, z0 + 4.1, z1 - 2.2].iter().enumerate() {
        put(f, g, "sf_9d2b96b5e3", v(x0 + 0.25, *zc), top, dir(170.0 + i as f32 * 25.0), 1.0, [1.0; 3], false);
    }
}

/// Rope pendants: lengths of jute rope looping from the ceiling in front of the bar wall, Edison bulbs hanging
/// from the low points of some loops.
fn rope_lights(g: &mut Geo, s: &Site) {
    // thick jute rope hung from hooks under the girders in long swags that cross in front of the sign, bulbs
    // dropping from the low points of some
    let rope = ins(Key::new(Mat::Plain(230), [0.6, 0.48, 0.33]));
    let socket = ins(Key::new(Mat::Plain(90), [0.05, 0.045, 0.04]));
    let x = ROPE_X;
    let top = s.deck_b - 0.55;
    let centre = (BAR_Z.0 + BAR_Z.1) * 0.5;
    let n = 22;
    for i in 0..n {
        let r = |k: u64| super::rnd(0x60E + k, i);
        let za = centre - 4.5 + i as f32 * 8.4 / n as f32 + (r(0) - 0.5) * 0.3;
        let zb = za + 0.9 + 0.8 * r(1);
        let low = s.y + 2.55 + 0.7 * r(2);
        let sag = top - low;
        let xo = x + (r(3) - 0.5) * 0.12;
        let seg = 18;
        let pt = |t: f32| Vec3::new(xo + (t - 0.5) * 0.04, top - sag * 4.0 * t * (1.0 - t), za + (zb - za) * t);
        for k in 0..seg {
            g.tube(rope, pt(k as f32 / seg as f32), pt((k + 1) as f32 / seg as f32), 0.019, 7);
        }
        if i % 2 == 1 {
            let lo = pt(0.5);
            let b = lo - Vec3::Y * (0.08 + 0.25 * r(4));
            g.tube(rope, lo, b, 0.01, 5);
            g.tube(socket, b, b - Vec3::Y * 0.06, 0.02, 8);
            g.tube(Key::new(Mat::Light, [1.0, 0.72, 0.4]), b - Vec3::Y * 0.06, b - Vec3::Y * 0.17, 0.028, 8);
            g.downlight(b - Vec3::Y * 0.12, 2.0, [0.12, 0.085, 0.045]);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// lounge

fn lounge(g: &mut Geo, f: &mut Furnisher, s: &Site) {
    let y = s.y;
    let (cx, cz) = ((RUG.0 + RUG.2) * 0.5, (RUG.1 + RUG.3) * 0.5);
    // looking east at the bar from the lounge, north is on the left: arranged as in the photo
    // black tufted sofa with its back to the bar, facing the room; the drum table in front of it
    // long and low, as in the photo (~3.2 m, about three table widths): stretched more in length than height
    put(f, g, "sf_4b25966ff2", v(cx + 1.45, cz + 0.1), y, dir(180.0), 1.0, [1.0; 3], true);
    if let Some(p) = f.out.last_mut() {
        p.scale = [1.7, 1.35, 1.45];
    }
    g.block_aabb(Vec3::new(cx + 0.95, y, cz - 1.5), Vec3::new(cx + 1.95, y + 0.8, cz + 1.7));
    drum_table(g, v(cx - 0.3, cz + 0.05), y, 0.62, 0.42);
    // front left: tan tufted sofa with its back to us, angled in; woven poufs by the column
    biscuit_sofa(g, v(cx - 2.45, cz - 1.5), y, dir(32.0));
    pouf(g, v(cx - 1.25, cz - 2.35), y, 0.26, 23.0);
    pouf(g, v(cx - 2.05, cz - 2.75), y, 0.26, 61.0);
    // front right: cognac sling chairs turned to the table, a brass tray table between
    sling_chair(g, v(cx - 1.85, cz + 1.25), y, dir(-38.0));
    sling_chair(g, v(cx - 0.55, cz + 2.35), y, dir(-92.0));
    put(f, g, "sf_49e5ede6a1", v(cx - 1.55, cz + 0.25), y, dir(0.0), 1.15, [1.0; 3], true);
    // cowhide drums at the sofa's right end
    cow_drum(g, v(cx + 0.45, cz + 2.0), y, 0.24);
    cow_drum(g, v(cx + 1.35, cz + 2.15), y, 0.22);
    // greenery on the table
    put(f, g, "sf_1ab9bf841d", v(cx - 0.4, cz - 0.1), y + 0.42, dir(30.0), 0.35, [1.0; 3], false);

    // right (south): tan wingbacks round a black low table under the second chandelier
    let (wx, wz) = (cx + 1.0, RUG.3 + 2.3);
    put(f, g, "sf_dbdbcd544e", v(wx - 1.15, wz - 0.1), y, dir(8.0), 1.0, [0.62, 0.36, 0.2], true);
    put(f, g, "sf_dbdbcd544e", v(wx + 1.2, wz + 0.15), y, dir(186.0), 1.0, [0.6, 0.35, 0.19], true);
    put(f, g, "sf_dbdbcd544e", v(wx + 0.1, wz + 1.25), y, dir(-82.0), 1.0, [0.62, 0.36, 0.2], true);
    low_table(g, v(wx, wz + 0.1), y, 1.1, 0.62, 0.42);

    // left (north): high-tops with leather stools along the bar end
    for (i, (tx, tz)) in [(cx + 2.4, RUG.1 - 1.6), (cx - 0.6, RUG.1 - 1.9), (cx - 3.4, RUG.1 - 1.4)].iter().enumerate() {
        put(f, g, "sf_4517652104", v(*tx, *tz), y, dir(0.0), 1.0, [1.0; 3], true);
        for k in 0..4 {
            let r = super::rnd(0x4170 + i as u64, k);
            let a = k as f32 * 90.0 + 45.0 + (r - 0.5) * 30.0;
            let p = v(*tx, *tz) + dir(a) * (0.78 + r * 0.1);
            put(f, g, STOOL, p, y, dir(a + 180.0 + (r - 0.5) * 40.0), 1.0, COGNAC, true);
        }
    }
    // a TV on a ceiling mount by the column, angled to the lounge (as in the photo)
    // a TV high on the column by the wingbacks on a black arm bracket, turned to the lounge (as in the photo)
    if let (Some(&colx), Some(&colz)) = (s.cols.iter().filter(|&&c| c > cx).min_by(|a, b| a.total_cmp(b)), s.girders.iter().filter(|&&z| z > RUG.3).min_by(|a, b| a.total_cmp(b))) {
        let face = dir(196.0);
        let arm0 = Vec3::new(colx - 0.21, y + 3.35, colz);
        let fc = arm0 + Vec3::new(face.x, 0.0, face.y) * 0.32;
        g.cuboid(black(), Vec3::new(colx - 0.23, y + 3.2, colz - 0.08), Vec3::new(colx - 0.2, y + 3.5, colz + 0.08));
        g.tube(black(), arm0, fc, 0.025, 8);
        tv(g, f, Vec3::new(fc.x, y + 3.35, fc.z), face);
    }
    // black can speakers hung on rods
    // black pendant speakers (truncated cones, wide end down) on cables
    for p in [v(cx - 2.0, cz - 2.9), v(cx + 2.2, RUG.3 + 1.0), v(cx - 2.6, RUG.3 + 1.8), v(cx + 2.6, cz - 1.2), v(cx - 0.4, RUG.1 - 3.2), v(cx + 0.4, RUG.3 + 3.4)] {
        let top = s.deck_b - 0.5;
        let a = Vec3::new(p.x, top - 0.7, p.y);
        g.tube(black(), Vec3::new(p.x, top, p.y), a, 0.006, 5);
        frustum(g, black(), a, a - Vec3::Y * 0.38, 0.1, 0.17, 16);
        disc(g, black(), a, 0.1, 16);
    }
}

/// Box centred at `c` with unit axes `ax` (width), `ay` (up), `az` (depth) and half sizes `h`.
fn obox(g: &mut Geo, k: Key, c: Vec3, ax: Vec3, ay: Vec3, az: Vec3, h: Vec3) {
    let (x, y, z) = (ax * h.x, ay * h.y, az * h.z);
    g.quad(k, c + x - y - z, y * 2.0, z * 2.0);
    g.quad(k, c - x - y - z, z * 2.0, y * 2.0);
    g.quad(k, c + y - x - z, z * 2.0, x * 2.0);
    g.quad(k, c - y - x - z, x * 2.0, z * 2.0);
    g.quad(k, c + z - x - y, x * 2.0, y * 2.0);
    g.quad(k, c - z - x - y, y * 2.0, x * 2.0);
}

/// Local frame of a seat facing `face`: (side, up, forward) in 3D.
fn frame(face: Vec2) -> (Vec3, Vec3, Vec3) {
    let f = face.normalize();
    (Vec3::new(-f.y, 0.0, f.x), Vec3::Y, Vec3::new(f.x, 0.0, f.y))
}

/// Up and forward axes tilted back by `deg` about the side axis.
fn tilt(up: Vec3, fwd: Vec3, deg: f32) -> (Vec3, Vec3) {
    let (s, c) = deg.to_radians().sin_cos();
    (up * c - fwd * s, fwd * c + up * s)
}

/// Lounge chair as in the photo: a dark walnut frame (square corner posts, flat top rails for arms, low side and
/// back rails) with a thick, puffy cognac leather sling hung inside it, quilted in large diamonds, its back rolling
/// over the top rail.
fn sling_chair(g: &mut Geo, c: Vec2, y: f32, face: Vec2) {
    let (sd, up, fw) = frame(face);
    let base = Vec3::new(c.x, y, c.y);
    let wood = walnut();
    let leather = ins(Key::new(Mat::Surface("pt_quilted_leather"), [1.0, 1.0, 1.0]).tile(0.5));
    let p = |side: f32, h: f32, fwd: f32| base + sd * side + up * h + fw * fwd;
    let (hw, hd) = (0.4, 0.42); // half width and depth of the frame
    for side in [-hw, hw] {
        // corner posts, front and back
        obox(g, wood, p(side, 0.29, hd - 0.03), sd, up, fw, Vec3::new(0.026, 0.29, 0.026));
        obox(g, wood, p(side, 0.31, -hd + 0.03), sd, up, fw, Vec3::new(0.026, 0.31, 0.026));
        // flat top rail (the arm), running a little past the front post
        obox(g, wood, p(side, 0.6, 0.02), sd, up, fw, Vec3::new(0.03, 0.018, hd + 0.03));
        // low side rail
        obox(g, wood, p(side, 0.13, 0.0), sd, up, fw, Vec3::new(0.02, 0.022, hd - 0.03));
    }
    // back top rail and low back stretcher
    obox(g, wood, p(0.0, 0.62, -hd + 0.03), sd, up, fw, Vec3::new(hw, 0.02, 0.022));
    obox(g, wood, p(0.0, 0.13, -hd + 0.03), sd, up, fw, Vec3::new(hw, 0.02, 0.02));
    // the sling: seat, back and the roll over the top rail, soft and puffy
    let (su, sf) = tilt(up, fw, 5.0);
    soft_box(g, leather, p(0.0, 0.35, 0.05), sd, su, sf, Vec3::new(0.365, 0.12, 0.35), 0.11);
    let (bu, bf) = tilt(up, fw, 16.0);
    soft_box(g, leather, p(0.0, 0.58, -0.25), sd, bu, bf, Vec3::new(0.365, 0.24, 0.12), 0.11);
    soft_box(g, leather, p(0.0, 0.74, -0.36), sd, up, fw, Vec3::new(0.37, 0.08, 0.1), 0.08);
    g.block_aabb(Vec3::new(c.x - 0.45, y, c.y - 0.45), Vec3::new(c.x + 0.45, y + 0.7, c.y + 0.45));
}

/// A box with rounded edges and corners (radius `r`), smooth-shaded: puffy cushions. Centre `c`, unit axes `ax`,
/// `ay`, `az`, half sizes `h`. Texture coordinates are metres across each face.
fn soft_box(g: &mut Geo, k: Key, c: Vec3, ax: Vec3, ay: Vec3, az: Vec3, h: Vec3, r: f32) {
    use render::TexVertex;
    let n = 8usize;
    let inner = (h - Vec3::splat(r)).max(Vec3::ZERO);
    let to_world = |l: Vec3| c + ax * l.x + ay * l.y + az * l.z;
    // the six faces of the cube [-1,1]^3 as (normal axis, sign, u axis, v axis)
    let faces: [(usize, f32, usize, usize); 6] = [(0, 1.0, 2, 1), (0, -1.0, 1, 2), (1, 1.0, 0, 2), (1, -1.0, 2, 0), (2, 1.0, 1, 0), (2, -1.0, 0, 1)];
    for (na, sign, ua, va) in faces {
        let mut verts = vec![];
        for j in 0..=n {
            for i in 0..=n {
                let mut q = [0f32; 3];
                q[na] = sign;
                q[ua] = i as f32 / n as f32 * 2.0 - 1.0;
                q[va] = j as f32 / n as f32 * 2.0 - 1.0;
                // point on the box surface, then pulled onto the rounded shape round the inner core
                let pt = Vec3::new(q[0] * h.x, q[1] * h.y, q[2] * h.z);
                let core = pt.clamp(-inner, inner);
                let off = pt - core;
                let nl = if off.length_squared() > 1e-10 { off.normalize() } else { Vec3::new(q[0], q[1], q[2]) * if na == 0 { Vec3::X } else if na == 1 { Vec3::Y } else { Vec3::Z } };
                let lp = core + nl * r;
                let wpos = g.world(to_world(lp));
                let wn = (ax * nl.x + ay * nl.y + az * nl.z).normalize();
                let wn = g.world(c + wn) - g.world(c);
                let ua_v = [ax, ay, az][ua];
                let u = lp.dot(Vec3::new((ua == 0) as i32 as f32, (ua == 1) as i32 as f32, (ua == 2) as i32 as f32));
                let v = lp.dot(Vec3::new((va == 0) as i32 as f32, (va == 1) as i32 as f32, (va == 2) as i32 as f32));
                let wt = (g.world(c + ua_v) - g.world(c)).normalize();
                verts.push(TexVertex { pos: wpos.to_array(), nrm: wn.normalize().to_array(), uv: [u, -v], tan: [wt.x, wt.y, wt.z, 1.0], edge: [0.0; 4] });
            }
        }
        // wind the face so it is front-facing from outside, whatever the handedness of the axes
        let w = (n + 1) as usize;
        let mid = (n / 2) * w + n / 2;
        let (p0, p1, p2) = (Vec3::from(verts[mid].pos), Vec3::from(verts[mid + 1].pos), Vec3::from(verts[mid + w + 1].pos));
        let flip = (p1 - p0).cross(p2 - p0).dot(Vec3::from(verts[mid].nrm)) < 0.0;
        let grp = g.groups.entry(k).or_default();
        let base = grp.0.len() as u32;
        grp.0.extend(verts);
        let w = w as u32;
        for j in 0..n as u32 {
            for i in 0..n as u32 {
                let (a, b, cc, d) = (base + j * w + i, base + j * w + i + 1, base + (j + 1) * w + i + 1, base + (j + 1) * w + i);
                if flip {
                    grp.1.extend([a, cc, b, a, d, cc]);
                } else {
                    grp.1.extend([a, b, cc, a, cc, d]);
                }
            }
        }
    }
}

/// The front sofa, of a piece with the lounge chairs: a dark walnut frame (corner posts, flat top rails for arms,
/// low rails) carrying soft biscuit-tufted cognac cushions, two seat cushions and two back cushions.
fn biscuit_sofa(g: &mut Geo, c: Vec2, y: f32, face: Vec2) {
    let (sd, up, fw) = frame(face);
    let base = Vec3::new(c.x, y, c.y);
    let wood = walnut();
    let leather = ins(Key::new(Mat::Surface("pt_biscuit_leather"), [1.0, 1.0, 1.0]).tile(0.45));
    let p = |side: f32, h: f32, fwd: f32| base + sd * side + up * h + fw * fwd;
    let (hw, hd) = (1.05, 0.44);
    for side in [-hw, hw] {
        obox(g, wood, p(side, 0.29, hd - 0.03), sd, up, fw, Vec3::new(0.028, 0.29, 0.028));
        obox(g, wood, p(side, 0.31, -hd + 0.03), sd, up, fw, Vec3::new(0.028, 0.31, 0.028));
        obox(g, wood, p(side, 0.6, 0.02), sd, up, fw, Vec3::new(0.032, 0.018, hd + 0.03));
        obox(g, wood, p(side, 0.13, 0.0), sd, up, fw, Vec3::new(0.022, 0.022, hd - 0.03));
    }
    obox(g, wood, p(0.0, 0.13, hd - 0.03), sd, up, fw, Vec3::new(hw, 0.022, 0.02));
    obox(g, wood, p(0.0, 0.13, -hd + 0.03), sd, up, fw, Vec3::new(hw, 0.022, 0.02));
    obox(g, wood, p(0.0, 0.62, -hd + 0.03), sd, up, fw, Vec3::new(hw, 0.02, 0.022));
    // a slatted deck under the cushions
    obox(g, wood, p(0.0, 0.2, 0.0), sd, up, fw, Vec3::new(hw - 0.03, 0.015, hd - 0.04));
    let (su, sf) = tilt(up, fw, 4.0);
    let (bu, bf) = tilt(up, fw, 14.0);
    for side in [-0.5, 0.5] {
        soft_box(g, leather, p(side, 0.33, 0.06), sd, su, sf, Vec3::new(0.49, 0.11, 0.34), 0.1);
        soft_box(g, leather, p(side, 0.6, -0.27), sd, bu, bf, Vec3::new(0.49, 0.22, 0.11), 0.1);
    }
    let r = 1.1;
    g.block_aabb(Vec3::new(c.x - r, y, c.y - r), Vec3::new(c.x + r, y + 0.7, c.y + r));
}

/// Truncated cone from `a` (radius `ra`) to `b` (radius `rb`), sides only.
fn frustum(g: &mut Geo, k: Key, a: Vec3, b: Vec3, ra: f32, rb: f32, n: usize) {
    let axis = (b - a).normalize();
    let t1 = axis.cross(if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y }).normalize();
    let t2 = axis.cross(t1);
    for i in 0..n {
        let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
        let (d0, d1) = (t1 * a0.cos() + t2 * a0.sin(), t1 * a1.cos() + t2 * a1.sin());
        let (p0, p1, p2, p3) = (a + d0 * ra, a + d1 * ra, b + d1 * rb, b + d0 * rb);
        g.poly(k, &[p0, p3, p2, p1], (p1 - p0).normalize(), (p3 - p0).normalize());
        g.poly(k, &[p0, p1, p2, p3], (p3 - p0).normalize(), (p1 - p0).normalize());
    }
}

/// Drum coffee table: a black disc on a brass plinth.
fn drum_table(g: &mut Geo, c: Vec2, y: f32, r: f32, h: f32) {
    let p = |dy: f32| Vec3::new(c.x, y + dy, c.y);
    g.tube(brass(), p(0.0), p(0.1), r * 0.82, 32);
    // matte black drum
    g.tube(ins(Key::new(Mat::Plain(235), [0.03, 0.03, 0.032])), p(0.1), p(h - 0.03), r, 36);
    // polished black stone top
    g.tube(ins(Key::new(Mat::Plain(30), [0.025, 0.025, 0.027])), p(h - 0.03), p(h), r + 0.01, 36);
    disc(g, ins(Key::new(Mat::Plain(18), [0.022, 0.022, 0.024])), p(h), r + 0.01, 36);
    g.block_aabb(Vec3::new(c.x - r, y, c.y - r), Vec3::new(c.x + r, y + h, c.y + r));
}

/// Low rectangular table in black.
fn low_table(g: &mut Geo, c: Vec2, y: f32, w: f32, d: f32, h: f32) {
    let k = ins(Key::new(Mat::Plain(120), [0.03, 0.03, 0.033]));
    g.cuboid(k, Vec3::new(c.x - w / 2.0 + 0.06, y, c.y - d / 2.0 + 0.06), Vec3::new(c.x + w / 2.0 - 0.06, y + h - 0.05, c.y + d / 2.0 - 0.06));
    g.cuboid(k, Vec3::new(c.x - w / 2.0, y + h - 0.05, c.y - d / 2.0), Vec3::new(c.x + w / 2.0, y + h, c.y + d / 2.0));
    g.block_aabb(Vec3::new(c.x - w / 2.0, y, c.y - d / 2.0), Vec3::new(c.x + w / 2.0, y + h, c.y + d / 2.0));
}

/// A flat disc (facing up) at `p`.
fn disc(g: &mut Geo, k: Key, p: Vec3, r: f32, n: usize) {
    let pts: Vec<Vec3> = (0..n).map(|i| {
        let a = -(i as f32) / n as f32 * std::f32::consts::TAU;
        p + Vec3::new(a.cos() * r, 0.0, a.sin() * r)
    }).collect();
    g.poly(k, &pts, Vec3::X, Vec3::NEG_Z);
}

/// Knitted cube pouf in cream (soft rounded box with a fine weave), turned `deg`.
fn pouf(g: &mut Geo, c: Vec2, y: f32, half: f32, deg: f32) {
    let k = ins(Key::new(Mat::Surface("pt_basket_weave"), [1.08, 1.04, 0.98]).tile(0.12));
    let a = dir(deg);
    let (ax, az) = (Vec3::new(a.x, 0.0, a.y), Vec3::new(-a.y, 0.0, a.x));
    let h = 0.22;
    soft_box(g, k, Vec3::new(c.x, y + h, c.y), ax, Vec3::Y, az, Vec3::new(half, h, half), 0.07);
    g.block_aabb(Vec3::new(c.x - half * 1.4, y, c.y - half * 1.4), Vec3::new(c.x + half * 1.4, y + 2.0 * h, c.y + half * 1.4));
}

/// Cowhide-covered drum ottoman.
fn cow_drum(g: &mut Geo, c: Vec2, y: f32, r: f32) {
    let k = ins(Key::new(Mat::Surface("pt_cowhide"), [1.0, 1.0, 1.0]).tile(1.2));
    let h = 0.44;
    g.tube(k, Vec3::new(c.x, y, c.y), Vec3::new(c.x, y + h, c.y), r, 28);
    disc(g, k, Vec3::new(c.x, y + h, c.y), r, 28);
    g.block_aabb(Vec3::new(c.x - r, y, c.y - r), Vec3::new(c.x + r, y + h, c.y + r));
}

// ---------------------------------------------------------------------------------------------------------------
// overhead

/// Two-tier ring chandeliers, as in the photo: each tier a steel ring with a cross through it, a central steel
/// stem through both tiers, and Edison bulbs in clear glass cylinders all round each ring - a row standing on top
/// and a row hanging underneath.
fn chandeliers(g: &mut Geo, s: &Site) {
    let (cx, cz) = ((RUG.0 + RUG.2) * 0.5, (RUG.1 + RUG.3) * 0.5);
    for (p, scale) in [(v(cx - 1.2, cz - 0.8), 1.0), (v(cx + 1.0, RUG.3 + 2.4), 0.85), (v(cx + 2.6, RUG.1 - 2.0), 0.8)] {
        chandelier(g, p, s.deck_b, s.y, scale);
    }
}

fn steel() -> Key {
    // dark gunmetal steel, as in the photo
    ins(Key::new(Mat::Plain(80), [0.11, 0.11, 0.115]))
}

fn chandelier(g: &mut Geo, c: Vec2, ceiling: f32, floor: f32, scale: f32) {
    // tiers: (ring radius, height); the lower ring smaller, as in the photo
    let rings = [(1.2 * scale, floor + 3.5), (0.85 * scale, floor + 2.85)];
    let st = steel();
    let at = |r: f32, a: f32, y: f32| Vec3::new(c.x + a.cos() * r, y, c.y + a.sin() * r);
    // canopy, the central stem down through both tiers, a finial below
    g.tube(st, Vec3::new(c.x, ceiling - 0.05, c.y), Vec3::new(c.x, ceiling, c.y), 0.09, 16);
    g.tube(st, Vec3::new(c.x, ceiling, c.y), Vec3::new(c.x, rings[1].1 - 0.12, c.y), 0.016, 8);
    g.tube(st, Vec3::new(c.x, rings[1].1 - 0.12, c.y), Vec3::new(c.x, rings[1].1 - 0.2, c.y), 0.03, 10);
    // four thin rods from the stem down to the top ring
    let hub = Vec3::new(c.x, rings[0].1 + 0.6, c.y);
    for k in 0..4 {
        g.tube(st, hub, at(rings[0].0, k as f32 * std::f32::consts::FRAC_PI_2 + 0.785, rings[0].1), 0.006, 5);
    }
    let edison = Key::new(Mat::Light, [0.9, 0.46, 0.15]);
    for &(r, ry) in &rings {
        // the ring and the cross through it
        let n_seg = 48;
        for i in 0..n_seg {
            let (a0, a1) = (i as f32 / n_seg as f32 * std::f32::consts::TAU, (i + 1) as f32 / n_seg as f32 * std::f32::consts::TAU);
            g.tube(st, at(r, a0, ry), at(r, a1, ry), 0.026, 8);
        }
        for k in 0..2 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.785;
            g.tube(st, at(r, a, ry), at(r, a + std::f32::consts::PI, ry), 0.016, 6);
        }
        // the lower tier hangs from the upper cross on four vertical rods
        if ry < rings[0].1 {
            for k in 0..4 {
                let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.785;
                g.tube(st, at(r, a, ry), at(r, a, rings[0].1), 0.01, 6);
            }
        }
        // Edison bulbs in clear glass cylinders: one standing on top of the ring, one hanging below, all round
        let n = (std::f32::consts::TAU * r / 0.18) as usize;
        for i in 0..n {
            let q = at(r, (i as f32 + 0.5) / n as f32 * std::f32::consts::TAU, ry);
            for up in [1.0f32, -1.0] {
                let y = Vec3::Y * up;
                let base = q + y * 0.018;
                g.tube(st, base, base + y * 0.025, 0.028, 8);
                // a clear glass cup (nearly transparent: edges and faint reflections) round an Edison bulb: a warm
                // glowing globe with a brighter filament in it
                let clear = Key::new(Mat::Glass, [0.9, 0.88, 0.82]).rough(0.05);
                g.tube(clear, base + y * 0.025, base + y * 0.155, 0.05, 14);
                disc(g, clear, base + y * 0.155, 0.05, 14);
                g.tube(Key::new(Mat::Light, [0.22, 0.12, 0.045]), base + y * 0.045, base + y * 0.125, 0.024, 10);
                g.tube(edison, base + y * 0.06, base + y * 0.11, 0.007, 6);
            }
        }
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.3;
            g.downlight(at(r * 0.6, a, ry - 0.45), 7.0, [0.36, 0.25, 0.13]);
        }
    }
}

/// Rose-painted spiral ducts: one along the top of the bar wall, one across the south of the lounge, one north.
fn ducts(g: &mut Geo, s: &Site) {
    let rose = ins(Key::new(Mat::Surface("pt_flex_duct"), [1.0, 1.0, 1.0]).tile(0.5));
    let y = s.deck_b - 0.5 - 0.62;
    let runs = [
        // the big one, hung low just over the bar wall's cap (as in the photo)
        // (over the counter, in front of the rope lights, which hang behind it on the bar side)
        (Vec3::new(DUCT_X, s.y + WALL_H + 0.5, 1.5), Vec3::new(DUCT_X, s.y + WALL_H + 0.5, 28.0), 0.38),
        (Vec3::new(1.5, y + 0.05, RUG.3 + 4.2), Vec3::new(DUCT_X - 0.4, y + 0.05, RUG.3 + 4.2), 0.3),
        (Vec3::new(1.5, y + 0.05, RUG.1 - 4.6), Vec3::new(DUCT_X - 0.4, y + 0.05, RUG.1 - 4.6), 0.27),
    ];
    for (a, b, r) in runs {
        g.tube(rose, a, b, r, 28);
        // hanger straps every 2.4 m
        let d = b - a;
        let n = (d.length() / 2.4) as i32;
        for k in 1..n {
            let p = a + d * (k as f32 / n as f32);
            g.cuboid(black(), Vec3::new(p.x - 0.01, p.y, p.z - 0.01), Vec3::new(p.x + 0.01, s.deck_b, p.z + 0.01));
        }
    }
}

/// Black track lighting under the girders over the lounge and bar: a rail with spot cans aimed down at angles.
fn track_lights(g: &mut Geo, s: &Site) {
    let y = s.deck_b - 0.5 - 0.04;
    for &gz in s.girders.iter().filter(|&&z| z > BAR_Z.0 - 1.0 && z < BAR_Z.1 + 2.0) {
        let (x0, x1) = (RUG.0 - 0.5, BAR_WALL - 1.6);
        g.cuboid(black(), Vec3::new(x0, y - 0.03, gz - 0.02), Vec3::new(x1, y, gz + 0.02));
        let n = ((x1 - x0) / 1.4) as i32;
        for k in 0..n {
            let x = x0 + 0.5 + k as f32 * 1.4;
            let r = super::rnd(0x7AC, (gz * 10.0) as u64 + k as u64);
            let aim = Vec3::new((r - 0.5) * 0.6, -1.0, if k % 2 == 0 { 0.35 } else { -0.35 }).normalize();
            let a = Vec3::new(x, y - 0.06, gz);
            g.tube(black(), Vec3::new(x, y - 0.03, gz), a, 0.008, 5);
            g.tube(black(), a, a + aim * 0.16, 0.045, 10);
        }
    }
}

/// The stencilled concrete rug (an SVG decal) under the lounge: its long side across the room (along the bar),
/// with a copy either side of it.
fn rug(g: &mut Geo, s: &Site) {
    // the stain carries the floor's concrete maps: same tint and finish as the slabs (super::stone_slabs)
    let k = ins(Key::new(Mat::Surface("dcl_cement_rug"), [3.05, 3.3, 3.25]).tile(7.2).rough(0.5));
    let (cx, cz) = ((RUG.0 + RUG.2) * 0.5, (RUG.1 + RUG.3) * 0.5);
    for i in -1..=1 {
        // clear concrete between the rugs
        let (x0, z0) = (cx - 2.7, cz - 3.6 + i as f32 * 8.4);
        g.decal(k, Vec3::new(x0, s.y + 0.003, z0), Vec3::Z * 7.2, Vec3::X * 5.4, 7.2);
    }
}
