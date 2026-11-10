//! Gameplay checks on a 1 m grid: walkable space, reachability from spawns, sightlines, cover, spawn fairness, overlaps.
use crate::scene::{Kind, Obstacles, Scene, Team};
use city::City;
use serde::Serialize;

pub const RES: f64 = 1.0;
/// Objects taller than this block movement: the player's jump (0.67 m) plus step (0.45 m) clears anything lower.
pub const MOVE_BLOCK_H: f64 = 1.1;
/// Objects at least this tall block a standing player's view.
pub const SIGHT_BLOCK_H: f64 = 1.6;
/// Objects at least this tall count as cover.
pub const COVER_H: f64 = 0.8;
const EYE: f64 = 1.7;

/// The static city rasterised once: which 1 m cells are inside a building.
pub struct World {
    pub city: City,
    pub rect: [f64; 4],
    pub w: usize,
    pub h: usize,
    pub solid: Vec<bool>,
}

fn point_in(r: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    let mut j = r.len() - 1;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            c = !c;
        }
        j = i;
    }
    c
}

impl World {
    pub fn new(city: City) -> Self {
        let rect = city.import_rect();
        let w = ((rect[2] - rect[0]) / RES).ceil() as usize;
        let h = ((rect[3] - rect[1]) / RES).ceil() as usize;
        let mut solid = vec![false; w * h];
        for b in &city.buildings {
            let (y0, _) = city::mesh::building_span(b, &city.terrain);
            let ground = b.outer.iter().map(|p| city.terrain.at(p[0], p[1])).fold(f64::INFINITY, f64::min);
            if y0 - ground > 2.0 {
                continue; // overhead: walk under it
            }
            let min = b.outer.iter().fold([f64::MAX; 2], |a, p| [a[0].min(p[0]), a[1].min(p[1])]);
            let max = b.outer.iter().fold([f64::MIN; 2], |a, p| [a[0].max(p[0]), a[1].max(p[1])]);
            let (i0, j0) = (((min[0] - rect[0]) / RES).floor().max(0.0) as usize, ((min[1] - rect[1]) / RES).floor().max(0.0) as usize);
            let (i1, j1) = ((((max[0] - rect[0]) / RES).ceil() as usize).min(w), (((max[1] - rect[1]) / RES).ceil() as usize).min(h));
            for j in j0..j1 {
                for i in i0..i1 {
                    let c = [rect[0] + (i as f64 + 0.5) * RES, rect[1] + (j as f64 + 0.5) * RES];
                    if point_in(&b.outer, c) && !b.holes.iter().any(|hl| point_in(hl, c)) {
                        solid[j * w + i] = true;
                    }
                }
            }
        }
        Self { city, rect, w, h, solid }
    }
    pub fn cell(&self, p: [f64; 2]) -> Option<usize> {
        let (i, j) = (((p[0] - self.rect[0]) / RES).floor(), ((p[1] - self.rect[1]) / RES).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.w && (j as usize) < self.h).then(|| j as usize * self.w + i as usize)
    }
    pub fn bounds(&self, s: &Scene) -> [f64; 4] {
        s.bounds.unwrap_or_else(|| self.city.play_rect())
    }
}

impl Obstacles for World {
    fn blocked(&self, p: [f64; 2]) -> bool {
        self.cell(p).is_none_or(|k| self.solid[k])
    }
}

/// Analysis grids over the scene bounds.
pub struct Analysis {
    pub rect: [f64; 4],
    pub w: usize,
    pub h: usize,
    pub walk: Vec<bool>,
    pub reach: Vec<bool>,
    pub sight_block: Vec<bool>,
    /// Longest clear view from the cell, metres; NaN where not sampled.
    pub sight: Vec<f32>,
    pub cover: Vec<bool>,
    pub report: Report,
}

#[derive(Serialize, Clone, Debug)]
pub struct Sightline {
    pub from: [i32; 2],
    pub to: [i32; 2],
    pub length_m: i32,
}

#[derive(Serialize, Clone, Debug)]
pub struct SpawnCheck {
    pub team: Team,
    pub at: [i32; 2],
    pub ok: bool,
    pub problem: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Report {
    pub bounds: [i32; 4],
    pub walkable_m2: u32,
    /// Share of walkable area reachable from the spawns (or from the largest open region if there are none).
    pub reachable_pct: f32,
    /// Separate walkable regions larger than 25 m² (1 is ideal).
    pub regions: u32,
    /// Share of walkable area within 3 m of cover.
    pub cover_pct: f32,
    /// Share of sampled walkable cells that see farther than 80 m / 150 m along some direction.
    pub sight_over_80m_pct: f32,
    pub sight_over_150m_pct: f32,
    pub longest_sightlines: Vec<Sightline>,
    pub spawns: Vec<SpawnCheck>,
    /// Walking distance from each team's spawns to the centre of the reachable area, metres.
    pub team_a_to_centre_m: Option<i32>,
    pub team_b_to_centre_m: Option<i32>,
    /// Shortest walking distance between the two teams' spawns, metres.
    pub spawn_to_spawn_m: Option<i32>,
    /// Pairs of enemy spawns with a clear line of sight between them (bad).
    pub spawns_in_view: u32,
    pub objects: u32,
    pub objects_in_buildings: Vec<u32>,
    pub objects_overlapping: Vec<[u32; 2]>,
    pub objects_out_of_bounds: Vec<u32>,
    pub issues: Vec<String>,
}

pub fn analyze(world: &World, s: &Scene) -> Analysis {
    let rect = world.bounds(s);
    let w = ((rect[2] - rect[0]) / RES).ceil() as usize;
    let h = ((rect[3] - rect[1]) / RES).ceil() as usize;
    let center = |i: usize, j: usize| [rect[0] + (i as f64 + 0.5) * RES, rect[1] + (j as f64 + 0.5) * RES];
    let mut walk = vec![true; w * h];
    let mut sight_block = vec![false; w * h];
    let mut cover_src = vec![false; w * h];
    for j in 0..h {
        for i in 0..w {
            if world.cell(center(i, j)).is_none_or(|k| world.solid[k]) {
                walk[j * w + i] = false;
                sight_block[j * w + i] = true;
                cover_src[j * w + i] = true;
            }
        }
    }
    // rasterise objects by sampling their footprint every 0.25 m, so thin fences still mark cells
    let mut in_buildings = vec![];
    for o in &s.objects {
        let ht = o.size[1];
        let r = o.rot.to_radians();
        let (fx, fz) = (r.sin(), -r.cos());
        let (nl, nd) = ((o.size[0] / 0.25).ceil() as i32, (o.size[2] / 0.25).ceil() as i32);
        let mut hit_building = false;
        for a in 0..=nl {
            for b in 0..=nd {
                let (u, v) = (a as f64 / nl.max(1) as f64 - 0.5, b as f64 / nd.max(1) as f64 - 0.5);
                let p = [o.x + fx * u * o.size[0] - fz * v * o.size[2], o.z + fz * u * o.size[0] + fx * v * o.size[2]];
                hit_building |= world.cell(p).is_some_and(|k| world.solid[k]);
                let (i, j) = (((p[0] - rect[0]) / RES).floor(), ((p[1] - rect[1]) / RES).floor());
                if i < 0.0 || j < 0.0 || i as usize >= w || j as usize >= h {
                    continue;
                }
                let k = j as usize * w + i as usize;
                if ht > MOVE_BLOCK_H {
                    walk[k] = false;
                }
                if ht >= SIGHT_BLOCK_H {
                    sight_block[k] = true;
                }
                if ht >= COVER_H {
                    cover_src[k] = true;
                }
            }
        }
        if hit_building {
            in_buildings.push(o.id);
        }
    }

    // cover: walkable cells within 3 m of a cover source
    let mut cover = vec![false; w * h];
    let r = (3.0 / RES) as i64;
    for j in 0..h as i64 {
        for i in 0..w as i64 {
            if !walk[j as usize * w + i as usize] {
                continue;
            }
            'c: for dz in -r..=r {
                for dx in -r..=r {
                    let (a, b) = (i + dx, j + dz);
                    if a >= 0 && b >= 0 && (a as usize) < w && (b as usize) < h && dx * dx + dz * dz <= r * r && cover_src[b as usize * w + a as usize] && !(dx == 0 && dz == 0) {
                        cover[j as usize * w + i as usize] = true;
                        break 'c;
                    }
                }
            }
        }
    }

    // reachability and walking distances (8-connected BFS, diagonal 1.414)
    let dist_from = |starts: &[usize]| -> Vec<f32> {
        let mut d = vec![f32::INFINITY; w * h];
        let mut q = std::collections::VecDeque::new();
        for &s0 in starts {
            if walk[s0] {
                d[s0] = 0.0;
                q.push_back(s0);
            }
        }
        // Dijkstra-lite: two passes of BFS relaxation are close enough on an open grid; use a proper deque relax loop
        while let Some(k) = q.pop_front() {
            let (i, j) = ((k % w) as i64, (k / w) as i64);
            for (dx, dz, c) in [(1, 0, 1.0f32), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)] {
                let (a, b) = (i + dx, j + dz);
                if a < 0 || b < 0 || a as usize >= w || b as usize >= h {
                    continue;
                }
                let n = b as usize * w + a as usize;
                if walk[n] && d[k] + c < d[n] - 1e-3 {
                    d[n] = d[k] + c;
                    q.push_back(n);
                }
            }
        }
        d.iter().map(|v| v * RES as f32).collect()
    };
    let to_cell = |x: f64, z: f64| -> Option<usize> {
        let (i, j) = (((x - rect[0]) / RES).floor(), ((z - rect[1]) / RES).floor());
        (i >= 0.0 && j >= 0.0 && (i as usize) < w && (j as usize) < h).then(|| j as usize * w + i as usize)
    };
    let mut issues = vec![];
    let mut spawn_checks = vec![];
    let mut team_cells: [Vec<usize>; 2] = [vec![], vec![]];
    for sp in &s.spawns {
        let k = to_cell(sp.x, sp.z);
        let (ok, problem) = match k {
            None => (false, Some("outside bounds".to_string())),
            Some(k) if !walk[k] => (false, Some("inside a building or object".to_string())),
            Some(_) => (true, None),
        };
        if let (true, Some(k)) = (ok, k) {
            team_cells[(sp.team == Team::B) as usize].push(k);
        }
        spawn_checks.push(SpawnCheck { team: sp.team, at: [sp.x as i32, sp.z as i32], ok, problem });
    }
    let all_spawns: Vec<usize> = team_cells.concat();
    let seeds = if all_spawns.is_empty() {
        // largest walkable region
        largest_region(&walk, w, h).into_iter().take(1).collect()
    } else {
        all_spawns.clone()
    };
    let reach_d = dist_from(&seeds);
    let reach: Vec<bool> = reach_d.iter().map(|d| d.is_finite()).collect();
    let walkable = walk.iter().filter(|b| **b).count();
    let reachable = reach.iter().filter(|b| **b).count();
    let regions = count_regions(&walk, w, h, 25);

    let (mut a_c, mut b_c, mut s2s) = (None, None, None);
    if !team_cells[0].is_empty() && !team_cells[1].is_empty() {
        let da = dist_from(&team_cells[0]);
        let db = dist_from(&team_cells[1]);
        // centre: reachable cell nearest the centroid of the reachable area
        let (mut cx, mut cz, mut n) = (0.0, 0.0, 0.0);
        for k in 0..w * h {
            if reach[k] {
                cx += (k % w) as f64;
                cz += (k / w) as f64;
                n += 1.0;
            }
        }
        if n > 0.0 {
            let (cx, cz) = (cx / n, cz / n);
            let c = (0..w * h).filter(|&k| reach[k]).min_by(|&p, &q| {
                let d = |k: usize| ((k % w) as f64 - cx).powi(2) + ((k / w) as f64 - cz).powi(2);
                d(p).total_cmp(&d(q))
            });
            if let Some(c) = c {
                a_c = da[c].is_finite().then(|| da[c] as i32);
                b_c = db[c].is_finite().then(|| db[c] as i32);
            }
        }
        let m = team_cells[1].iter().map(|&k| da[k]).fold(f32::INFINITY, f32::min);
        s2s = m.is_finite().then(|| m as i32);
        if s2s.is_none() {
            issues.push("team A cannot walk to team B".into());
        }
        if let (Some(a), Some(b)) = (a_c, b_c) {
            let (lo, hi) = (a.min(b) as f32, a.max(b) as f32);
            if hi > lo * 1.25 + 10.0 {
                issues.push(format!("spawns unfair: team A is {a} m and team B {b} m from the centre"));
            }
        }
    }

    // sight blocking by buildings, objects >= 1.6 m and the bounds edge
    let ray = |x0: f64, z0: f64, dx: f64, dz: f64| -> f64 {
        let mut t = 0.0;
        loop {
            t += RES;
            let (x, z) = (x0 + dx * t, z0 + dz * t);
            match to_cell(x, z) {
                None => return t,
                Some(k) if sight_block[k] => return t,
                _ => {}
            }
            if t > 2000.0 {
                return t;
            }
        }
    };
    let mut in_view = 0;
    for &a in &team_cells[0] {
        for &b in &team_cells[1] {
            let (pa, pb) = (center(a % w, a / w), center(b % w, b / w));
            let (dx, dz) = (pb[0] - pa[0], pb[1] - pa[1]);
            let l = dx.hypot(dz);
            if ray(pa[0], pa[1], dx / l, dz / l) >= l {
                in_view += 1;
            }
        }
    }
    if in_view > 0 {
        issues.push(format!("{in_view} enemy spawn pair(s) can see each other"));
    }
    let _ = EYE;

    let dirs: Vec<(f64, f64)> = (0..16).map(|k| { let a = k as f64 * std::f64::consts::TAU / 16.0; (a.sin(), -a.cos()) }).collect();
    let mut sight = vec![f32::NAN; w * h];
    let mut lines = vec![];
    let samples: Vec<usize> = (0..w * h).filter(|&k| walk[k] && reach[k] && (k % w) % 2 == 0 && (k / w) % 2 == 0).collect();
    let results: Vec<(usize, f64, (f64, f64))> = std::thread::scope(|sc| {
        let chunks: Vec<_> = samples.chunks(samples.len().div_ceil(8).max(1)).map(|ch| {
            let dirs = &dirs;
            let ray = &ray;
            sc.spawn(move || ch.iter().map(|&k| {
                let p = center(k % w, k / w);
                let (best, d) = dirs.iter().map(|&(dx, dz)| (ray(p[0], p[1], dx, dz), (dx, dz))).fold((0.0, (0.0, 0.0)), |a, b| if b.0 > a.0 { b } else { a });
                (k, best, d)
            }).collect::<Vec<_>>())
        }).collect();
        chunks.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let (mut over80, mut over150) = (0, 0);
    for &(k, best, d) in &results {
        sight[k] = best as f32;
        over80 += (best > 80.0) as u32;
        over150 += (best > 150.0) as u32;
        lines.push((best, center(k % w, k / w), d));
    }
    lines.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut longest: Vec<Sightline> = vec![];
    for (len, p, d) in lines {
        if longest.len() >= 6 {
            break;
        }
        // skip lines running along one already listed (parallel, starting within 20 m of it)
        let dup = longest.iter().any(|l| {
            let (a, b) = ([l.from[0] as f64, l.from[1] as f64], [l.to[0] as f64, l.to[1] as f64]);
            let (ux, uz) = (b[0] - a[0], b[1] - a[1]);
            let ul = ux.hypot(uz).max(1e-6);
            let t = (((p[0] - a[0]) * ux + (p[1] - a[1]) * uz) / (ul * ul)).clamp(0.0, 1.0);
            let off = (p[0] - a[0] - ux * t).hypot(p[1] - a[1] - uz * t);
            off < 20.0 && ((d.0 * ux + d.1 * uz) / ul).abs() > 0.9
        });
        if dup {
            continue;
        }
        longest.push(Sightline { from: [p[0] as i32, p[1] as i32], to: [(p[0] + d.0 * len) as i32, (p[1] + d.1 * len) as i32], length_m: len as i32 });
    }
    let n_s = results.len().max(1) as f32;

    let mut overlapping = vec![];
    for (i, a) in s.objects.iter().enumerate() {
        for b in &s.objects[i + 1..] {
            if a.overlaps(b, -0.05) {
                overlapping.push([a.id, b.id]);
            }
        }
    }
    let oob: Vec<u32> = s.objects.iter().filter(|o| o.x < rect[0] || o.x > rect[2] || o.z < rect[1] || o.z > rect[3]).map(|o| o.id).collect();
    if !in_buildings.is_empty() {
        issues.push(format!("{} object(s) intersect buildings: {:?}", in_buildings.len(), in_buildings));
    }
    if !overlapping.is_empty() {
        issues.push(format!("{} overlapping object pair(s): {:?}", overlapping.len(), overlapping));
    }
    if !oob.is_empty() {
        issues.push(format!("{} object(s) outside the bounds: {:?}", oob.len(), oob));
    }
    if regions > 1 {
        issues.push(format!("walkable space is split into {regions} regions"));
    }
    if s.spawns.iter().filter(|p| p.team == Team::A).count() == 0 || s.spawns.iter().filter(|p| p.team == Team::B).count() == 0 {
        issues.push("each team needs at least one spawn".into());
    }
    for c in &spawn_checks {
        if let Some(p) = &c.problem {
            issues.push(format!("spawn {:?} at {:?}: {p}", c.team, c.at));
        }
    }
    let sight150 = over150 as f32 / n_s * 100.0;
    if sight150 > 10.0 {
        issues.push(format!("{sight150:.0}% of the map sees farther than 150 m (long open streets)"));
    }
    let cover_pct = cover.iter().filter(|b| **b).count() as f32 / walkable.max(1) as f32 * 100.0;
    if cover_pct < 50.0 {
        issues.push(format!("only {cover_pct:.0}% of walkable area is within 3 m of cover"));
    }

    let report = Report {
        bounds: [rect[0] as i32, rect[1] as i32, rect[2] as i32, rect[3] as i32],
        walkable_m2: (walkable as f64 * RES * RES) as u32,
        reachable_pct: reachable as f32 / walkable.max(1) as f32 * 100.0,
        regions,
        cover_pct,
        sight_over_80m_pct: over80 as f32 / n_s * 100.0,
        sight_over_150m_pct: sight150,
        longest_sightlines: longest,
        spawns: spawn_checks,
        team_a_to_centre_m: a_c,
        team_b_to_centre_m: b_c,
        spawn_to_spawn_m: s2s,
        spawns_in_view: in_view,
        objects: s.objects.len() as u32,
        objects_in_buildings: in_buildings,
        objects_overlapping: overlapping,
        objects_out_of_bounds: oob,
        issues,
    };
    let _ = Kind::all();
    Analysis { rect, w, h, walk, reach, sight_block, sight, cover, report }
}

fn regions_of(walk: &[bool], w: usize, h: usize) -> Vec<Vec<usize>> {
    let mut seen = vec![false; w * h];
    let mut out = vec![];
    for s in 0..w * h {
        if !walk[s] || seen[s] {
            continue;
        }
        let mut comp = vec![];
        let mut st = vec![s];
        seen[s] = true;
        while let Some(k) = st.pop() {
            comp.push(k);
            let (i, j) = (k % w, k / w);
            for (a, b) in [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)] {
                if a < w && b < h && walk[b * w + a] && !seen[b * w + a] {
                    seen[b * w + a] = true;
                    st.push(b * w + a);
                }
            }
        }
        out.push(comp);
    }
    out
}

fn count_regions(walk: &[bool], w: usize, h: usize, min_cells: usize) -> u32 {
    regions_of(walk, w, h).iter().filter(|c| c.len() >= min_cells).count() as u32
}

/// Cells of the largest walkable region, its first cell first.
fn largest_region(walk: &[bool], w: usize, h: usize) -> Vec<usize> {
    regions_of(walk, w, h).into_iter().max_by_key(|c| c.len()).unwrap_or_default()
}
