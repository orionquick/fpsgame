//! Office floor plans for rectangular floor plates, generated from the window bays.
//!
//! Local frame: x in [0, w], z in [0, d]. The service core sits against the +x wall (the blind party wall);
//! the other three sides carry perimeter offices, one per one or two window bays, off a corridor ring.
//! A cross corridor leads from the west corridor to the core; central rooms fill the rest.
use glam::Vec2;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
}

impl Rect {
    pub fn new(x0: f32, z0: f32, x1: f32, z1: f32) -> Self {
        Self { x0: x0.min(x1), z0: z0.min(z1), x1: x0.max(x1), z1: z0.max(z1) }
    }
    pub fn w(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn d(&self) -> f32 {
        self.z1 - self.z0
    }
    pub fn centre(&self) -> Vec2 {
        Vec2::new((self.x0 + self.x1) * 0.5, (self.z0 + self.z1) * 0.5)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Office,
    Meeting,
    Storage,
    Corridor,
    Lobby,
    Hall,
    Restroom,
    Stair,
    Elevator,
    Riser,
}

#[derive(Clone, Debug)]
pub struct Room {
    pub r: Rect,
    pub kind: Use,
}

/// A partition along a->b with door openings centred at the given distances from `a`.
#[derive(Clone, Debug)]
pub struct Partition {
    pub a: Vec2,
    pub b: Vec2,
    pub doors: Vec<f32>,
    /// Core walls are masonry (thicker, painted concrete); others drywall.
    pub core: bool,
}

#[derive(Clone, Debug)]
pub struct Core {
    pub r: Rect,
    pub stair: Rect,
    pub elevator: Rect,
    pub riser: Rect,
    pub restrooms: [Rect; 2],
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub rooms: Vec<Room>,
    pub walls: Vec<Partition>,
    pub core: Core,
}

#[derive(Clone)]
pub struct Input {
    pub w: f32,
    pub d: f32,
    /// Exterior wall thickness.
    pub t: f32,
    /// Pier centres along the north (x), west (z) and south (x) facades: candidate partition lines.
    pub piers_n: Vec<f32>,
    pub piers_w: Vec<f32>,
    pub piers_s: Vec<f32>,
    pub seed: u64,
}

pub const BAND: f32 = 4.5;
pub const CORRIDOR: f32 = 1.8;
pub const CORE_D: f32 = 6.6;
pub const CORE_W: f32 = 10.0;
pub const DOOR_W: f32 = 0.95;

fn rnd(seed: u64, i: u64) -> f32 {
    let mut x = seed ^ i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}

/// Splits [lo, hi] at pier lines into rooms of one or two bays (never narrower than `min`).
fn split(lo: f32, hi: f32, piers: &[f32], min: f32, seed: u64) -> Vec<(f32, f32)> {
    let mut cuts: Vec<f32> = piers.iter().copied().filter(|&p| p > lo + min && p < hi - min).collect();
    cuts.sort_by(f32::total_cmp);
    // drop every other cut at random so rooms take one or two bays
    let mut kept = vec![];
    let mut last = lo;
    for (i, &c) in cuts.iter().enumerate() {
        let wide = c - last;
        if wide >= min && (rnd(seed, i as u64) < 0.55 || wide > 6.5) {
            kept.push(c);
            last = c;
        }
    }
    if let Some(&l) = kept.last() {
        if hi - l < min {
            kept.pop();
        }
    }
    let mut out = vec![];
    let mut a = lo;
    for c in kept {
        out.push((a, c));
        a = c;
    }
    out.push((a, hi));
    out
}

/// Even split of [lo, hi] into rooms near `target` wide.
fn even(lo: f32, hi: f32, target: f32) -> Vec<(f32, f32)> {
    let n = ((hi - lo) / target).round().max(1.0) as usize;
    let s = (hi - lo) / n as f32;
    (0..n).map(|i| (lo + s * i as f32, lo + s * (i + 1) as f32)).collect()
}

/// Office floor: perimeter offices, corridor ring + cross corridor, central rooms, core.
pub fn office(i: &Input) -> Plan {
    let (w, d, t) = (i.w, i.d, i.t);
    let (p, c) = (BAND, CORRIDOR);
    let core = core(i);
    let mut rooms = vec![];
    let mut walls = vec![];
    let v = Vec2::new;

    let zn = t + p; // north band inner line
    let zs = d - t - p; // south band inner line
    let xw = t + p; // west band inner line
    let zc = core.r.centre().y;
    let (zx0, zx1) = (zc - c / 2.0, zc + c / 2.0); // cross corridor

    // north and south bands: full width, the corner office reaches past the corridor corner for its door
    for (south, zi, zo, piers) in [(false, zn, t, &i.piers_n), (true, zs, d - t, &i.piers_s)] {
        // the corner office runs past the corridor corner so its door opens onto the corridor
        let piers: Vec<f32> = piers.iter().copied().filter(|&x| x > xw + c + 0.6).collect();
        let spans = split(t, w - t, &piers, 2.8, i.seed ^ south as u64 * 77);
        let mut doors = vec![];
        for (k, &(a, b)) in spans.iter().enumerate() {
            rooms.push(Room { r: Rect::new(a, zo, b, zi), kind: Use::Office });
            // door near the corridor end of the room, inside the corridor's reach
            let dx = if k == 0 { xw + c / 2.0 } else { a + 0.35 + DOOR_W / 2.0 };
            doors.push(dx - t);
            if k + 1 < spans.len() {
                walls.push(Partition { a: v(b, zo), b: v(b, zi), doors: vec![], core: false });
            }
        }
        walls.push(Partition { a: v(t, zi), b: v(w - t, zi), doors, core: false });
    }
    // west band between the corner offices
    {
        let spans = split(zn, zs, &i.piers_w, 2.8, i.seed ^ 991);
        let mut doors = vec![];
        for (k, &(a, b)) in spans.iter().enumerate() {
            rooms.push(Room { r: Rect::new(t, a, xw, b), kind: Use::Office });
            doors.push(b - 0.35 - DOOR_W / 2.0 - zn);
            if k + 1 < spans.len() {
                walls.push(Partition { a: v(t, b), b: v(xw, b), doors: vec![], core: false });
            }
        }
        walls.push(Partition { a: v(xw, zn), b: v(xw, zs), doors, core: false });
    }
    // corridors
    rooms.push(Room { r: Rect::new(xw, zn, xw + c, zs), kind: Use::Corridor });
    rooms.push(Room { r: Rect::new(xw + c, zn, w - t, zn + c), kind: Use::Corridor });
    rooms.push(Room { r: Rect::new(xw + c, zs - c, w - t, zs), kind: Use::Corridor });
    // cross corridor into a lift lobby in front of the core
    let xl = core.r.x0 - 2.0;
    rooms.push(Room { r: Rect::new(xw + c, zx0, xl, zx1), kind: Use::Corridor });
    rooms.push(Room { r: Rect::new(xl, core.r.z0, core.r.x0, core.r.z1), kind: Use::Lobby });

    // central blocks north and south of the cross corridor
    let xc0 = xw + c;
    for (south, za, zb, strip) in [(false, zn + c, zx0, (zn + c, core.r.z0)), (true, zx1, zs - c, (core.r.z1, zs - c))] {
        let spans = even(xc0, xl, 6.0);
        let mut doors = vec![];
        for (k, &(a, b)) in spans.iter().enumerate() {
            let kind = if (k + south as usize) % 2 == 0 { Use::Meeting } else { Use::Office };
            rooms.push(Room { r: Rect::new(a, za, b, zb), kind });
            doors.push((a + b) / 2.0 - xc0);
            if k + 1 < spans.len() {
                walls.push(Partition { a: v(b, za), b: v(b, zb), doors: vec![], core: false });
            }
        }
        // the wall facing the cross corridor carries the doors
        let zline = if south { za } else { zb };
        walls.push(Partition { a: v(xc0, zline), b: v(xl, zline), doors, core: false });
        // walls facing the north/south corridor, the west corridor and the lobby/storage side
        let zouter = if south { zb } else { za };
        walls.push(Partition { a: v(xc0, zouter), b: v(xl, zouter), doors: vec![], core: false });
        walls.push(Partition { a: v(xc0, za), b: v(xc0, zb), doors: vec![], core: false });
        walls.push(Partition { a: v(xl, za), b: v(xl, zb), doors: vec![], core: false });
        // storage strip between the core and the north/south corridor, closed off from the lobby
        let (s0, s1) = strip;
        if (s1 - s0).abs() > 1.2 {
            rooms.push(Room { r: Rect::new(xl, s0, w - t, s1), kind: Use::Storage });
            let (zl, zlobby) = if south { (s1, s0) } else { (s0, s1) };
            walls.push(Partition { a: v(xl, zl), b: v(w - t, zl), doors: vec![(w - t - xl) / 2.0], core: false });
            walls.push(Partition { a: v(xl, zlobby), b: v(core.r.x0, zlobby), doors: vec![], core: false });
        }
    }
    core_walls(&core, &mut walls, &mut rooms);
    Plan { rooms, walls, core }
}

/// Ground floor: one open hall around the core, with a lobby in front of it.
pub fn ground(i: &Input) -> Plan {
    let core = core(i);
    let (w, d, t) = (i.w, i.d, i.t);
    // the hall wraps the core: everything west of it, plus the strips north and south of it
    let mut rooms = vec![
        Room { r: Rect::new(t, t, core.r.x0, d - t), kind: Use::Hall },
        Room { r: Rect::new(core.r.x0, t, w - t, core.r.z0), kind: Use::Hall },
        Room { r: Rect::new(core.r.x0, core.r.z1, w - t, d - t), kind: Use::Hall },
    ];
    let mut walls = vec![];
    core_walls(&core, &mut walls, &mut rooms);
    Plan { rooms, walls, core }
}

fn core(i: &Input) -> Core {
    let (w, d, t) = (i.w, i.d, i.t);
    let r = Rect::new(w - t - CORE_D, d / 2.0 - CORE_W / 2.0, w - t, d / 2.0 + CORE_W / 2.0);
    let stair = Rect::new(r.x0, r.z0, r.x1, r.z0 + 3.1);
    let elevator = Rect::new(r.x0, stair.z1, r.x0 + 2.5, stair.z1 + 2.5);
    let riser = Rect::new(elevator.x1, stair.z1, r.x1, elevator.z1);
    let rz = (r.z1 - elevator.z1) / 2.0;
    let restrooms = [Rect::new(r.x0, elevator.z1, r.x1, elevator.z1 + rz), Rect::new(r.x0, elevator.z1 + rz, r.x1, r.z1)];
    Core { r, stair, elevator, riser, restrooms }
}

fn core_walls(core: &Core, walls: &mut Vec<Partition>, rooms: &mut Vec<Room>) {
    let v = Vec2::new;
    let r = core.r;
    // front (west) wall: stair door, elevator door, restroom doors
    let f = |z: f32| z - r.z0;
    let doors = vec![f(core.stair.z0 + 0.9), f(core.elevator.centre().y), f(core.restrooms[0].z0 + 0.8), f(core.restrooms[1].z1 - 0.8)];
    walls.push(Partition { a: v(r.x0, r.z0), b: v(r.x0, r.z1), doors, core: true });
    walls.push(Partition { a: v(r.x0, r.z0), b: v(r.x1, r.z0), doors: vec![], core: true });
    walls.push(Partition { a: v(r.x0, r.z1), b: v(r.x1, r.z1), doors: vec![], core: true });
    walls.push(Partition { a: v(r.x0, core.stair.z1), b: v(r.x1, core.stair.z1), doors: vec![], core: true });
    walls.push(Partition { a: v(r.x0, core.elevator.z1), b: v(r.x1, core.elevator.z1), doors: vec![], core: true });
    walls.push(Partition { a: v(core.elevator.x1, core.elevator.z0), b: v(core.elevator.x1, core.elevator.z1), doors: vec![], core: true });
    walls.push(Partition { a: v(r.x0, core.restrooms[0].z1), b: v(r.x1, core.restrooms[0].z1), doors: vec![], core: true });
    rooms.push(Room { r: core.stair, kind: Use::Stair });
    rooms.push(Room { r: core.elevator, kind: Use::Elevator });
    rooms.push(Room { r: core.riser, kind: Use::Riser });
    for rr in core.restrooms {
        rooms.push(Room { r: rr, kind: Use::Restroom });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input { w: 30.9, d: 30.0, t: 0.45, piers_n: vec![5.0, 9.0], piers_w: vec![4.3, 9.4, 14.4, 19.4, 24.4], piers_s: vec![4.3, 9.4, 14.4, 19.5, 24.6], seed: 7 }
    }

    #[test]
    fn rooms_stay_inside_and_doors_fit_their_walls() {
        let i = input();
        let p = office(&i);
        for r in &p.rooms {
            assert!(r.r.x0 >= i.t - 1e-3 && r.r.x1 <= i.w - i.t + 1e-3 && r.r.z0 >= i.t - 1e-3 && r.r.z1 <= i.d - i.t + 1e-3, "{r:?}");
            assert!(r.r.w() > 1.0 && r.r.d() > 1.0, "{r:?}");
        }
        for wl in &p.walls {
            let l = (wl.b - wl.a).length();
            for &dpos in &wl.doors {
                assert!(dpos - DOOR_W / 2.0 >= -1e-3 && dpos + DOOR_W / 2.0 <= l + 1e-3, "door {dpos} on wall {wl:?}");
            }
        }
        let offices = p.rooms.iter().filter(|r| r.kind == Use::Office).count();
        assert!(offices >= 8, "{offices} offices");
    }

    #[test]
    fn perimeter_rooms_take_one_or_two_bays() {
        let i = input();
        let p = office(&i);
        for r in p.rooms.iter().filter(|r| r.kind == Use::Office && r.r.x0 < i.t + 0.01 && r.r.z0 > BAND) {
            assert!(r.r.d() < 11.0, "west office {:?} spans more than two bays", r.r);
        }
    }
}
