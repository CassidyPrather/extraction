//! One world, one hearing: how a sound gets from where it's made to an
//! ear, for the player's ears (`audio::occlude`) and the robots'
//! (`robots::senses`) alike, so a sound you hear muffled is one they hear
//! cut short, by the same number.
//!
//! Each building is a small graph ([`Graph`]): its rooms (the floor
//! flooded with its doors shut; everything round it is the outside) and
//! what joins them: a doorway (open, [`DOORWAY_LOSS`]; shut, the door's
//! own loss by what it's made of, [`door_loss`]) or a wall (by what it's
//! built of, [`wall_loss`]). Every room's loss to every other is worked
//! out once, and again only when one of its doors moves, so asking
//! ([`Hearing::path`]) is a lookup at each end and a table read: O(1) per
//! sound however many robots listen. A building's upper storeys add a
//! floor's loss each; a site underground reaches the surface through its
//! way in, or through the rock ([`ROCK_LOSS`]), whichever is louder; out in
//! the open, a hill between still stands in the way (the heightmap, no
//! rays).
//!
//! Losses are in dB, scaled to the robots' hearing, not to a sound
//! engineer's: through a timber wall a robot hears [`OCCLUDED`] as far,
//! which under inverse-distance falloff is `20 log10(0.45)` ≈ 7 dB, and
//! the rest are set against it. [`WALL_SCALE`] moves them all.
//!
//! [`OCCLUDED`]: crate::robots::senses::OCCLUDED

use bevy::prelude::*;
use std::collections::HashMap;

use crate::acoustics::Occlusion;
use crate::interact::touch::Stuff;
use crate::world::Zone;
use crate::world::buildings::{Building, Door, DoorDef, DoorLeaf, Shape};

pub struct HearingPlugin;

impl Plugin for HearingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hearing>()
            .add_systems(PreUpdate, (build_graphs, follow_doors).chain());
    }
}

/// One knob for every wall and door: 1 is today's balance.
pub const WALL_SCALE: f32 = 1.0;
/// Round an open doorway (dB lost): a little quieter, a little duller.
pub const DOORWAY_LOSS: f32 = 3.0;
/// Through the rock between a site and the surface.
pub const ROCK_LOSS: f32 = 14.0;
/// A building's storeys, m, and the floor between each.
pub const STOREY: f32 = 3.0;
/// Over a hill (as `robots::senses::HEARD_OVER` measures it).
pub fn hill_loss() -> f32 {
    -crate::acoustics::wall_db()
}

/// Through a wall of `stuff` (dB lost).
pub fn wall_loss(stuff: Stuff) -> f32 {
    let timber = -crate::acoustics::wall_db();
    WALL_SCALE
        * match stuff {
            Stuff::Wood => timber,
            Stuff::Metal => 8.0,
            Stuff::Stone => 10.0,
            // A window in the wall: the pane passes a lot.
            Stuff::Glass => 4.0,
            Stuff::Skin | Stuff::Wire => 2.0,
        }
}

/// Through a shut door of `stuff` (dB lost).
pub fn door_loss(stuff: Stuff) -> f32 {
    WALL_SCALE
        * match stuff {
            Stuff::Metal | Stuff::Stone => 8.0,
            _ => 5.0,
        }
}

/// The muffle that goes with a loss: clear at none, a doorway's 3 kHz at
/// [`DOORWAY_LOSS`], a wall's 700 Hz at a timber wall, darker past it.
pub fn cutoff(loss: f32) -> f32 {
    const KNOTS: [(f32, f32); 4] = [(0.0, 20_480.0), (3.0, 3_000.0), (6.9, 700.0), (20.0, 250.0)];
    if loss <= 0.0 {
        return KNOTS[0].1;
    }
    for w in KNOTS.windows(2) {
        let ((a, fa), (b, fb)) = (w[0], w[1]);
        if loss <= b {
            let t = (loss - a) / (b - a);
            return (fa.log2() + (fb.log2() - fa.log2()) * t).exp2();
        }
    }
    KNOTS[3].1
}

/// What a loss does: gain (dB) and muffle.
pub fn occlusion(loss: f32) -> Occlusion {
    Occlusion {
        db: -loss,
        cutoff: cutoff(loss),
    }
}

/// The room grid's cell (m) and the margin round a footprint.
const CELL: f32 = 0.25;
const MARGIN: f32 = 2.0;
/// Ear height over a floor; and the heights walls are looked for at (a
/// sill, an ear, a lintel's foot).
const EAR: f32 = 1.3;
const HEIGHTS: [f32; 3] = [0.6, 1.3, 2.0];
/// The thickest a wall is (m): anything thicker both ways is furniture.
const WALL_THICK: f32 = 0.6;
/// Not a room: a wall, or a pocket too small to stand in.
const NONE: u16 = u16::MAX;
const OUTSIDE: u16 = 0;
const SMALLEST_ROOM: usize = 8;
/// The world split into tiles this big (m) to find a building fast.
const TILE: f32 = 16.0;

/// A building as sound gets round it.
#[derive(Debug)]
pub struct Graph {
    pub building: Entity,
    inv: bevy::math::Affine3A,
    n: usize,
    origin: f32,
    floor: f32,
    /// Underground: each cell's floor (NaN where none).
    levels: Option<Vec<f32>>,
    /// Its bounds, building-local (x, z).
    lo: Vec2,
    hi: Vec2,
    room: Vec<u16>,
    pub rooms: usize,
    /// Each door: its leaf, the rooms either side, its loss shut, open now.
    doors: Vec<(Entity, u16, u16, f32, bool)>,
    /// The quietest wall between two rooms.
    walls: HashMap<(u16, u16), f32>,
    /// Loss from every room to every other, rooms x rooms.
    table: Vec<f32>,
    /// How much each room smears where a sound seems to come from
    /// ([`Hearing::smear`]).
    smear: Vec<f32>,
    site: bool,
}

impl Graph {
    /// A building's graph, from its prefab: its walls rastered at ear
    /// height (each cell remembers what it's built of), flooded with its
    /// doors shut into rooms.
    pub fn new(
        building: Entity,
        prefab: &str,
        xf: &Transform,
        doors: &[(Entity, DoorDef, bool)],
    ) -> Option<Self> {
        let geo = crate::world::buildings::prefab(prefab)?;
        let site = extraction_core::underground::is_site(prefab);
        let foot = crate::world::structures::footprint(prefab);
        let half = Vec2::new(foot.x * 0.5, foot.z * 0.5);
        let floor = doors
            .iter()
            .map(|(_, d, _)| d.hinge.y)
            .min_by(f32::total_cmp)
            .or(geo.floor)
            .unwrap_or(0.0);
        // A site runs far past its kiosk: its raster's bounds.
        let raster = site
            .then(|| extraction_core::underground::raster(prefab))
            .flatten();
        let (lo, hi) = match raster {
            Some(r) => {
                let lo = Vec2::from(r.origin);
                let size = Vec2::new(r.nx as f32, r.nz as f32) * extraction_core::underground::CELL;
                (lo, lo + size)
            }
            None => (-half, half),
        };
        let origin = lo.x.min(lo.y) - MARGIN;
        let n = ((hi.x.max(hi.y) + MARGIN - origin) / CELL).ceil() as usize;
        let centre = |i: usize, j: usize| {
            Vec2::new(
                origin + (i as f32 + 0.5) * CELL,
                origin + (j as f32 + 0.5) * CELL,
            )
        };
        let levels: Option<Vec<f32>> = raster.map(|r| {
            (0..n * n)
                .map(|k| {
                    let c = centre(k % n, k / n);
                    r.floor_at([c.x, c.y]).unwrap_or(f32::NAN)
                })
                .collect()
        });
        let ear = |k: usize| {
            levels
                .as_ref()
                .map_or(floor, |l| if l[k].is_nan() { floor } else { l[k] })
                + EAR
        };
        // Walls: what stands thin and long across the room's height (a
        // table is thick both ways; a wall, a sill, a partition is not),
        // and what it's made of. Looked for at three heights over the
        // floor: walled below and open at ear height is a window (glass
        // or none, it passes a lot).
        let mut at: Vec<[Option<Stuff>; 3]> = vec![[None; 3]; n * n];
        for piece in geo.pieces.iter().filter(|p| p.collide) {
            let size = match piece.shape {
                Shape::Box(s) => s,
                Shape::Prism(s) => Vec3::new(s.x, s.y * 0.5, s.z),
                Shape::Cylinder(r, h) => Vec3::new(r * 2.0, h, r * 2.0),
            };
            if size.x.min(size.z) > WALL_THICK || size.x.max(size.z) < 0.6 || size.y < 0.4 {
                continue;
            }
            let half = size * 0.5 + Vec3::new(CELL * 0.5, 0.0, CELL * 0.5);
            let inv = piece.at.rotation.inverse();
            let reach = half.x.max(half.z) + CELL;
            let c = piece.at.translation;
            let cell = |v: f32| ((v - origin) / CELL).floor() as i64;
            for j in cell(c.z - reach).max(0)..=cell(c.z + reach).min(n as i64 - 1) {
                for i in cell(c.x - reach).max(0)..=cell(c.x + reach).min(n as i64 - 1) {
                    let k = j as usize * n + i as usize;
                    let p = centre(i as usize, j as usize);
                    for (h, y) in HEIGHTS.iter().enumerate() {
                        let q = inv * (Vec3::new(p.x, ear(k) - EAR + y, p.y) - c);
                        if q.x.abs() <= half.x && q.z.abs() <= half.z && q.y.abs() <= half.y {
                            at[k][h] = Some(Stuff::of_mat(piece.mat));
                        }
                    }
                }
            }
        }
        let wall: Vec<Option<Stuff>> = at
            .iter()
            .map(|[low, mid, high]| match (low, mid, high) {
                (_, Some(s), _) => Some(*s),
                (Some(_), None, _) => Some(Stuff::Glass),
                _ => None,
            })
            .collect();
        let mut blocked: Vec<bool> = wall.iter().map(Option::is_some).collect();
        // Underground: no floor, no room.
        if let Some(l) = &levels {
            for (k, y) in l.iter().enumerate() {
                if y.is_nan() {
                    blocked[k] = true;
                }
            }
        }
        // Doors shut: each doorway's gap closed.
        let gap = |d: &DoorDef| {
            let along = Quat::from_rotation_y(d.yaw) * Vec3::X;
            (
                Vec2::new(d.hinge.x, d.hinge.z),
                Vec2::new(along.x, along.z),
                d.width,
            )
        };
        let mut shut = blocked.clone();
        for (_, d, _) in doors {
            let (a, along, w) = gap(d);
            for (k, s) in shut.iter_mut().enumerate() {
                let p = centre(k % n, k / n) - a;
                let t = p.dot(along);
                let perp = (p - along * t).length();
                if (-0.15..=w + 0.15).contains(&t) && perp <= CELL * 0.75 {
                    *s = true;
                }
            }
        }
        // Rooms: what floods together with the doors shut; whatever
        // reaches the edge is outside.
        let mut room = vec![NONE; n * n];
        let mut rooms = 1usize;
        for start in 0..n * n {
            if shut[start] || room[start] != NONE {
                continue;
            }
            let mut cells = vec![start];
            let mut stack = vec![start];
            room[start] = 0;
            let mut edge = false;
            while let Some(k) = stack.pop() {
                let (i, j) = (k % n, k / n);
                edge |= i == 0 || j == 0 || i == n - 1 || j == n - 1;
                for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                    let (a, b) = (i as i64 + di, j as i64 + dj);
                    if a < 0 || b < 0 || a >= n as i64 || b >= n as i64 {
                        continue;
                    }
                    let c = b as usize * n + a as usize;
                    if !shut[c] && room[c] == NONE {
                        room[c] = 0;
                        cells.push(c);
                        stack.push(c);
                    }
                }
            }
            let id = if edge {
                OUTSIDE
            } else if cells.len() < SMALLEST_ROOM {
                NONE
            } else {
                rooms += 1;
                (rooms - 1) as u16
            };
            for c in cells {
                room[c] = id;
            }
        }
        let mut g = Graph {
            building,
            inv: xf.compute_affine().inverse(),
            n,
            origin,
            floor,
            levels,
            lo,
            hi,
            room,
            rooms,
            doors: Vec::new(),
            walls: HashMap::new(),
            table: Vec::new(),
            smear: Vec::new(),
            site,
        };
        // Each door joins the rooms either side of it.
        for (e, d, open) in doors {
            let (a, along, w) = gap(d);
            let mid = a + along * w * 0.5;
            let normal = Vec2::new(-along.y, along.x);
            let (r1, r2) = (
                g.room_near(mid + normal * 0.7),
                g.room_near(mid - normal * 0.7),
            );
            if r1 != NONE && r2 != NONE && r1 != r2 {
                g.doors
                    .push((*e, r1, r2, door_loss(Stuff::of_mat(d.mat)), *open));
            }
        }
        // A site's ways in: each kiosk's door (at its -Z end) opens on the
        // ground outside, which the raster doesn't reach. Its room joins
        // the outside there, through the kiosk's door if one hangs in it
        // (as it stands), or open (an arch).
        if let Some(r) = raster {
            use extraction_core::underground::KIOSK;
            for portal in &r.portals {
                let inside = Vec2::from(portal.to_site([0.0, -KIOSK[1] + 0.6]));
                let outside = Vec2::from(portal.to_site([0.0, -KIOSK[1] - 0.6]));
                let room = g.room_near(inside);
                if room == NONE || room == OUTSIDE {
                    continue;
                }
                let leaf = doors.iter().find(|(_, d, _)| {
                    let (a, along, w) = gap(d);
                    (a + along * w * 0.5).distance((inside + outside) * 0.5) < 1.5
                });
                g.doors.push(match leaf {
                    Some((e, d, open)) => {
                        (*e, room, OUTSIDE, door_loss(Stuff::of_mat(d.mat)), *open)
                    }
                    None => (Entity::PLACEHOLDER, room, OUTSIDE, DOORWAY_LOSS, true),
                });
            }
        }
        // Walls join the rooms either side of them, by the quietest
        // between each pair (a window lets more through than brick); a
        // site's rooms are cut in the rock.
        for k in 0..n * n {
            let Some(stuff) = wall[k] else { continue };
            let (i, j) = ((k % n) as i64, (k / n) as i64);
            for (di, dj) in [(1i64, 0i64), (0, 1)] {
                let side = |s: i64| {
                    (1..=4).find_map(|step| {
                        let (a, b) = (i + di * s * step, j + dj * s * step);
                        if a < 0 || b < 0 || a >= n as i64 || b >= n as i64 {
                            return Some(NONE);
                        }
                        let c = b as usize * n + a as usize;
                        (!blocked[c]).then_some(g.room[c])
                    })
                };
                let (Some(r1), Some(r2)) = (side(1), side(-1)) else {
                    continue;
                };
                if r1 == NONE || r2 == NONE || r1 == r2 {
                    continue;
                }
                let loss = if site { ROCK_LOSS } else { wall_loss(stuff) };
                let key = (r1.min(r2), r1.max(r2));
                let w = g.walls.entry(key).or_insert(loss);
                *w = w.min(loss);
            }
        }
        // Each room's ring: how big it is, and how hard what bounds it
        // (the walls round it, by what they're built of; a site's rock).
        let mut area = vec![0usize; g.rooms];
        let mut hard = vec![(0.0f32, 0usize); g.rooms];
        for k in 0..n * n {
            let r = g.room[k];
            if r == NONE || r == OUTSIDE {
                continue;
            }
            area[r as usize] += 1;
            let (i, j) = (k % n, k / n);
            for (di, dj) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i as i64 + di, j as i64 + dj);
                if a < 0 || b < 0 || a >= n as i64 || b >= n as i64 {
                    continue;
                }
                let c = b as usize * n + a as usize;
                let h = match wall[c] {
                    _ if site && blocked[c] => Some(ROCK_HARD),
                    Some(stuff) => Some(hardness(stuff)),
                    None => None,
                };
                if let Some(h) = h {
                    hard[r as usize].0 += h;
                    hard[r as usize].1 += 1;
                }
            }
        }
        g.smear = (0..g.rooms)
            .map(|r| {
                if r == OUTSIDE as usize || hard[r].1 == 0 {
                    return 1.0;
                }
                let m2 = area[r] as f32 * CELL * CELL;
                1.0 + hard[r].0 / hard[r].1 as f32 * (m2 / SMEAR_AREA).min(1.0)
            })
            .collect();
        g.solve();
        Some(g)
    }

    /// Every room's loss to every other, round doors and through walls,
    /// whichever way is quietest (Floyd-Warshall: a building's handful
    /// of rooms).
    fn solve(&mut self) {
        let r = self.rooms;
        let mut t = vec![f32::INFINITY; r * r];
        for i in 0..r {
            t[i * r + i] = 0.0;
        }
        let mut join = |a: u16, b: u16, loss: f32| {
            let (a, b) = (a as usize, b as usize);
            t[a * r + b] = t[a * r + b].min(loss);
            t[b * r + a] = t[b * r + a].min(loss);
        };
        for &(_, a, b, shut, open) in &self.doors {
            join(a, b, if open { DOORWAY_LOSS } else { shut });
        }
        for (&(a, b), &loss) in &self.walls {
            join(a, b, loss);
        }
        for k in 0..r {
            for i in 0..r {
                for j in 0..r {
                    let via = t[i * r + k] + t[k * r + j];
                    if via < t[i * r + j] {
                        t[i * r + j] = via;
                    }
                }
            }
        }
        self.table = t;
    }

    /// A door opened or shut: its rooms' losses again.
    fn set_door(&mut self, door: Entity, open: bool) -> bool {
        let mut changed = false;
        for d in &mut self.doors {
            if d.0 == door && d.4 != open {
                d.4 = open;
                changed = true;
            }
        }
        if changed {
            self.solve();
        }
        changed
    }

    fn cell_of(&self, p: Vec2) -> Option<usize> {
        let i = ((p.x - self.origin) / CELL).floor();
        let j = ((p.y - self.origin) / CELL).floor();
        (i >= 0.0 && j >= 0.0 && (i as usize) < self.n && (j as usize) < self.n)
            .then(|| j as usize * self.n + i as usize)
    }

    /// The room at a building-local point (x, z), or the nearest within a
    /// metre (a point in a wall, a doorway's jamb).
    fn room_near(&self, p: Vec2) -> u16 {
        let Some(k) = self.cell_of(p) else {
            return OUTSIDE;
        };
        if self.room[k] != NONE {
            return self.room[k];
        }
        let (i, j) = ((k % self.n) as i64, (k / self.n) as i64);
        for r in 1..=4i64 {
            for dj in -r..=r {
                for di in -r..=r {
                    if di.abs() != r && dj.abs() != r {
                        continue;
                    }
                    let (a, b) = (i + di, j + dj);
                    if a < 0 || b < 0 || a >= self.n as i64 || b >= self.n as i64 {
                        continue;
                    }
                    let c = b as usize * self.n + a as usize;
                    if self.room[c] != NONE {
                        return self.room[c];
                    }
                }
            }
        }
        NONE
    }

    /// Where a world point is in this building: its room and storey, or
    /// None if it's not in it (outside its walls, above a site's rock).
    fn place(&self, p: Vec3) -> Option<(u16, i32)> {
        let l = self.inv.transform_point3(p);
        if l.x < self.lo.x - 0.3
            || l.z < self.lo.y - 0.3
            || l.x > self.hi.x + 0.3
            || l.z > self.hi.y + 0.3
        {
            return None;
        }
        let floor = match &self.levels {
            Some(levels) => {
                let y = levels[self.cell_of(Vec2::new(l.x, l.z))?];
                if y.is_nan() || (l.y - y).abs() > 4.0 {
                    return None;
                }
                y
            }
            None => self.floor,
        };
        if l.y < floor - 1.0 {
            return None;
        }
        let room = self.room_near(Vec2::new(l.x, l.z));
        if room == OUTSIDE || room == NONE {
            return None;
        }
        let storey = if self.site {
            0
        } else {
            ((l.y - floor) / STOREY).floor().max(0.0) as i32
        };
        Some((room, storey))
    }

    fn loss(&self, a: u16, b: u16) -> f32 {
        self.table[a as usize * self.rooms + b as usize]
    }
}

/// A room this big (m²) rings as much as its walls let it.
const SMEAR_AREA: f32 = 100.0;
/// How much a wall of each kind rings (adds to a room's smear at full
/// size): the rock round a site's galleries most, timber least.
const ROCK_HARD: f32 = 1.5;
fn hardness(stuff: Stuff) -> f32 {
    match stuff {
        Stuff::Stone | Stuff::Metal => 1.0,
        Stuff::Glass => 0.8,
        Stuff::Wood => 0.6,
        Stuff::Skin | Stuff::Wire => 0.2,
    }
}

/// Every building's graph, and where to find them.
#[derive(Resource, Default)]
pub struct Hearing {
    pub graphs: Vec<Graph>,
    tiles: HashMap<(i32, i32), Vec<u16>>,
    dirty: bool,
    /// Out in the open, how much the ground round smears a sound
    /// (`weapons::reverb::valley_smear`), on its grid.
    valley: Valley,
}

/// The valley grid's cell (m).
const VALLEY_CELL: f32 = 8.0;

/// The ground's smear on a grid over the map.

#[derive(Default)]
struct Valley {
    n: usize,
    origin: f32,
    smear: Vec<f32>,
}

impl Valley {
    fn of(zone: &Zone) -> Self {
        use crate::weapons::reverb::valley_smear;
        let n = (zone.map.size / VALLEY_CELL).ceil() as usize;
        let origin = -zone.map.size * 0.5;
        let smear = (0..n * n)
            .map(|k| {
                let x = origin + ((k % n) as f32 + 0.5) * VALLEY_CELL;
                let z = origin + ((k / n) as f32 + 0.5) * VALLEY_CELL;
                valley_smear(|x, z| zone.ground(x, z), x, z)
            })
            .collect();
        Self { n, origin, smear }
    }

    fn at(&self, p: Vec3) -> f32 {
        let cell = VALLEY_CELL;
        let (i, j) = ((p.x - self.origin) / cell, (p.z - self.origin) / cell);
        if self.n == 0 || i < 0.0 || j < 0.0 || i as usize >= self.n || j as usize >= self.n {
            return 1.0;
        }
        self.smear[j as usize * self.n + i as usize]
    }
}

/// Where a point is: in a building's room (graph, room, storey), or out.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    In(usize, u16, i32),
    Out,
}

impl Hearing {
    /// The graphs, for buildings at these poses, and the tiles that find
    /// them.
    pub fn from_graphs(graphs: Vec<Graph>) -> Self {
        let mut tiles: HashMap<(i32, i32), Vec<u16>> = HashMap::new();
        for (k, g) in graphs.iter().enumerate() {
            let xf = g.inv.inverse();
            let mid = (g.lo + g.hi) * 0.5;
            let r = ((g.hi - g.lo) * 0.5).length() + 1.0;
            let c = xf.transform_point3(Vec3::new(mid.x, 0.0, mid.y));
            let t = |v: f32| (v / TILE).floor() as i32;
            for tz in t(c.z - r)..=t(c.z + r) {
                for tx in t(c.x - r)..=t(c.x + r) {
                    tiles.entry((tx, tz)).or_default().push(k as u16);
                }
            }
        }
        Self {
            graphs,
            tiles,
            dirty: false,
            valley: Valley::default(),
        }
    }

    fn place(&self, p: Vec3) -> Place {
        let key = ((p.x / TILE).floor() as i32, (p.z / TILE).floor() as i32);
        for &k in self.tiles.get(&key).into_iter().flatten() {
            if let Some((room, storey)) = self.graphs[k as usize].place(p) {
                return Place::In(k as usize, room, storey);
            }
        }
        Place::Out
    }

    /// The loss from a place to the open air round its building (dB).
    fn out_of(&self, p: Place) -> f32 {
        match p {
            Place::In(g, room, _) => {
                let g = &self.graphs[g];
                let way = g.loss(room, OUTSIDE);
                if g.site { way.min(ROCK_LOSS) } else { way }
            }
            Place::Out => 0.0,
        }
    }

    /// How a sound made at `from` reaches an ear at `to`: the dB it loses
    /// on the way and the muffle that goes with it. The same answer for
    /// the player's ears and a robot's. Out in the open on both ends,
    /// `blocked` is asked (once) whether anything but a building stands
    /// between (a compound wall, an outcrop, a wreck: `acoustics::blocked`,
    /// two rays); `None` asks only the heightmap.
    pub fn path(
        &self,
        zone: Option<&Zone>,
        from: Vec3,
        to: Vec3,
        blocked: Option<&dyn Fn(Vec3, Vec3) -> bool>,
    ) -> Occlusion {
        occlusion(self.loss(zone, from, to, blocked))
    }

    /// How much where a sound at `at` seems to come from is smeared by
    /// the room it rings in, for a robot's guess (`robots::senses`,
    /// `HEARD_SPREAD`): 1 in a small soft room, up to 2 in a big hard
    /// hall, 2.5 in a rock gallery; outside, by how the ground rises round
    /// it (1 on a plain, up to 2.2 in a narrow valley).
    pub fn smear(&self, at: Vec3) -> f32 {
        match self.place(at) {
            Place::In(g, room, _) => self.graphs[g].smear[room as usize],
            Place::Out => self.valley.at(at),
        }
    }

    #[cfg(test)]
    fn loss_open(&self, from: Vec3, to: Vec3) -> f32 {
        self.loss(None, from, to, None)
    }

    /// [`Self::path`], and whether anything stands in the way beyond an
    /// open doorway (a wall, a shut door, the rock, a hill): what the
    /// robots' ears call "through a wall".
    pub fn heard(
        &self,
        zone: Option<&Zone>,
        from: Vec3,
        to: Vec3,
        blocked: Option<&dyn Fn(Vec3, Vec3) -> bool>,
    ) -> (Occlusion, bool) {
        let loss = self.loss(zone, from, to, blocked);
        (occlusion(loss), loss > DOORWAY_LOSS + 0.01)
    }

    /// The dB a sound loses from `from` to `to`.
    pub fn loss(
        &self,
        zone: Option<&Zone>,
        from: Vec3,
        to: Vec3,
        blocked: Option<&dyn Fn(Vec3, Vec3) -> bool>,
    ) -> f32 {
        let (a, b) = (self.place(from), self.place(to));
        match (a, b) {
            (Place::In(g, r1, s1), Place::In(h, r2, s2)) if g == h => {
                let graph = &self.graphs[g];
                let floors = (s1 - s2).unsigned_abs() as f32 * wall_loss(Stuff::Wood);
                graph.loss(r1, r2) + floors
            }
            (Place::Out, Place::Out) => {
                let cut = match blocked {
                    Some(b) => b(from, to),
                    None => zone.is_some_and(|z| over_a_hill(z, from, to)),
                };
                if cut { hill_loss() } else { 0.0 }
            }
            _ => self.out_of(a) + self.out_of(b),
        }
    }
}

/// Out in the open: is the ground between higher than the line from just
/// over the sound (`robots::senses::HEARD_OVER`) to the ear? Sampled on
/// the heightmap, no rays.
pub fn over_a_hill(zone: &Zone, from: Vec3, to: Vec3) -> bool {
    let src = from + Vec3::Y * crate::robots::senses::HEARD_OVER;
    let d = src.xz().distance(to.xz());
    let steps = ((d / 2.0) as usize).clamp(4, 48);
    (1..steps).any(|k| {
        let p = src.lerp(to, k as f32 / steps as f32);
        zone.ground(p.x, p.z) > p.y
    })
}

/// Buildings spawned or the map changed: every graph again.
#[allow(clippy::type_complexity)]
fn build_graphs(
    mut hearing: ResMut<Hearing>,
    zone: Option<Res<Zone>>,
    added: Query<(), Added<Building>>,
    buildings: Query<(Entity, &Building, &Transform)>,
    leaves: Query<(Entity, &DoorLeaf, &ChildOf, Option<&Door>)>,
) {
    if !added.is_empty() || zone.as_ref().is_some_and(|z| z.is_changed()) {
        hearing.dirty = true;
        return;
    }
    if !hearing.dirty {
        return;
    }
    let graphs: Vec<Graph> = buildings
        .iter()
        .filter_map(|(e, b, xf)| {
            let doors: Vec<(Entity, DoorDef, bool)> = leaves
                .iter()
                .filter(|(_, _, p, _)| p.parent() == e)
                .map(|(d, leaf, _, door)| (d, leaf.0, crate::robots::doors::is_open(door)))
                .collect();
            Graph::new(e, &b.prefab, xf, &doors)
        })
        .collect();
    *hearing = Hearing::from_graphs(graphs);
    if let Some(zone) = zone {
        hearing.valley = Valley::of(&zone);
    }
}

/// A door opened or shut, broken or gone: its building's table again.
fn follow_doors(
    mut hearing: ResMut<Hearing>,
    doors: Query<(Entity, &Door), Changed<Door>>,
    mut gone: RemovedComponents<Door>,
) {
    let changes: Vec<(Entity, bool)> = doors
        .iter()
        .map(|(e, d)| (e, crate::robots::doors::is_open(Some(d))))
        .chain(gone.read().map(|e| (e, true)))
        .collect();
    if changes.is_empty() {
        return;
    }
    let hearing = hearing.bypass_change_detection();
    for (e, open) in changes {
        for g in &mut hearing.graphs {
            g.set_door(e, open);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every building that has doors: each door joins two rooms (or a room
    /// and the outside); open, they're a doorway apart; shut, the door's
    /// loss; and a room is nothing from itself.
    #[test]
    fn doors_join_rooms() {
        let mut checked = 0;
        for id in crate::world::buildings::PREFABS {
            let Some(geo) = crate::world::buildings::prefab(id) else {
                continue;
            };
            if geo.doors.is_empty() || extraction_core::underground::is_site(id) {
                continue;
            }
            let doors = |open: bool| -> Vec<(Entity, DoorDef, bool)> {
                geo.doors
                    .iter()
                    .enumerate()
                    .map(|(k, d)| (Entity::from_raw_u32(k as u32 + 1).unwrap(), *d, open))
                    .collect()
            };
            let xf = Transform::default();
            let (Some(open), Some(shut)) = (
                Graph::new(Entity::PLACEHOLDER, id, &xf, &doors(true)),
                Graph::new(Entity::PLACEHOLDER, id, &xf, &doors(false)),
            ) else {
                continue;
            };
            for (k, &(_, a, b, loss, _)) in open.doors.iter().enumerate() {
                assert_ne!(a, b, "{id}: a door between a room and itself");
                assert!(open.loss(a, b) <= DOORWAY_LOSS + 1e-3, "{id} door {k}");
                let s = shut.loss(a, b);
                assert!(
                    s > DOORWAY_LOSS && s <= loss + 1e-3,
                    "{id} door {k}: shut {s}"
                );
                assert_eq!(open.loss(a, a), 0.0);
                checked += 1;
            }
        }
        assert!(checked > 5, "only {checked} doors");
    }

    /// The muffle darkens as the loss grows, and is clear at none.
    #[test]
    fn the_muffle_follows_the_loss() {
        assert!(cutoff(0.0) > 20_000.0);
        assert!((cutoff(DOORWAY_LOSS) - 3_000.0).abs() < 1.0);
        assert!((cutoff(wall_loss(Stuff::Wood)) - 700.0).abs() < 20.0);
        assert!(cutoff(ROCK_LOSS) < cutoff(wall_loss(Stuff::Stone)));
    }

    /// A cell of each room, building-local (x, z), for tests.
    fn somewhere_in(g: &Graph, room: u16) -> Option<Vec2> {
        let k = g.room.iter().position(|r| *r == room)?;
        Some(Vec2::new(
            g.origin + ((k % g.n) as f32 + 0.5) * CELL,
            g.origin + ((k / g.n) as f32 + 0.5) * CELL,
        ))
    }

    fn graph(id: &str, open: bool) -> Graph {
        let geo = crate::world::buildings::prefab(id).unwrap();
        let doors: Vec<(Entity, DoorDef, bool)> = geo
            .doors
            .iter()
            .enumerate()
            .map(|(k, d)| (Entity::from_raw_u32(k as u32 + 1).unwrap(), *d, open))
            .collect();
        Graph::new(Entity::PLACEHOLDER, id, &Transform::default(), &doors).unwrap()
    }

    /// The set pairs, through `Hearing::loss` as both ears ask it: the
    /// room behind a shut door is quieter than behind an open one; the
    /// next room through a wall is the wall's loss; a storey up a floor's;
    /// a site's room heard from the surface over it through its way out or
    /// the rock, never worse than the rock.
    #[test]
    fn set_pairs() {
        // The shop: a room behind its back door, shut and open.
        let (open, shut) = (graph("shop", true), graph("shop", false));
        let (_, a, b, door, _) = open.doors[0];
        let pa = somewhere_in(&open, a).unwrap();
        let pb = somewhere_in(&open, b).unwrap();
        let f = open.floor;
        let p3 = |p: Vec2, up: f32| Vec3::new(p.x, f + 1.3 + up, p.y);
        let h_open = Hearing::from_graphs(vec![open]);
        let h_shut = Hearing::from_graphs(vec![shut]);
        let lo = h_open.loss_open(p3(pa, 0.0), p3(pb, 0.0));
        let ls = h_shut.loss_open(p3(pa, 0.0), p3(pb, 0.0));
        assert!(lo <= DOORWAY_LOSS + 1e-3, "open {lo}");
        assert!(ls > lo && ls <= door + 1e-3, "shut {ls} open {lo}");
        // Same room: nothing lost; a storey up, a floor's.
        assert_eq!(h_open.loss_open(p3(pa, 0.0), p3(pa, 0.0)), 0.0);
        // A house's inside from its yard, door shut: its door or a wall;
        // and a storey up in it, a floor's loss (the house is two storeys
        // tall; the shop isn't, and over its roof is out of doors).
        let house = graph("house_brick", false);
        let inside = somewhere_in(&house, 1).unwrap();
        let floor = house.floor;
        let h = Hearing::from_graphs(vec![house]);
        let at = |up: f32| Vec3::new(inside.x, floor + 1.3 + up, inside.y);
        let up = h.loss_open(at(0.0), at(STOREY));
        assert!((up - wall_loss(Stuff::Wood)).abs() < 1e-3, "upstairs {up}");
        let yard = Vec3::new(0.0, floor + 1.3, 30.0);
        let l = h.loss_open(Vec3::new(inside.x, floor + 1.3, inside.y), yard);
        assert!(
            l > DOORWAY_LOSS && l <= wall_loss(Stuff::Stone) + 1e-3,
            "house {l}"
        );
        // A site below: its deepest room from the ground over it.
        let id = extraction_core::underground::SITES[0].0;
        let site = graph(id, true);
        let (k, y) = site
            .levels
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(k, y)| !y.is_nan() && site.room[*k] != NONE && site.room[*k] != OUTSIDE)
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(k, y)| (k, *y))
            .unwrap();
        let c = Vec2::new(
            site.origin + ((k % site.n) as f32 + 0.5) * CELL,
            site.origin + ((k / site.n) as f32 + 0.5) * CELL,
        );
        let h = Hearing::from_graphs(vec![site]);
        let below = Vec3::new(c.x, y + 1.3, c.y);
        let above = Vec3::new(c.x, 1.3, c.y);
        let l = h.loss_open(below, above);
        assert!(
            l > 0.0 && l <= ROCK_LOSS + 1e-3,
            "{id} room at {y} from the surface: {l}"
        );
        eprintln!(
            "set pairs: door open {lo:.1} / shut {ls:.1} dB, upstairs {up:.1}, \
            house from its yard (shut) see above, {id} at {y:.0} m from the surface {l:.1} dB"
        );
    }

    /// The open air doesn't smear; a room does, a rock gallery most.
    #[test]
    fn rooms_smear() {
        let house = graph("house_plank", true);
        let inside = somewhere_in(&house, 1).unwrap();
        let f = house.floor;
        let h = Hearing::from_graphs(vec![house]);
        assert_eq!(h.smear(Vec3::new(40.0, 1.0, 40.0)), 1.0);
        let room = h.smear(Vec3::new(inside.x, f + 1.3, inside.y));
        assert!(room > 1.0 && room < 2.0, "a house room {room}");
        let id = extraction_core::underground::SITES[0].0;
        let site = graph(id, true);
        let most = site.smear.iter().cloned().fold(1.0f32, f32::max);
        assert!(
            most > room,
            "{id}'s biggest room {most} over a house's {room}"
        );
        assert!(most <= 1.0 + ROCK_HARD + 1e-3);
    }
}

/// The gauge: for set pairs on Marrow Creek, what a walker actually hears
/// (its threshold, found by trying louder and quieter noises: through the
/// obstacle against the same distance in the open) agrees with the dB the
/// player's ears lose on the same path, within a dB.
#[cfg(test)]
mod gauge {
    use super::*;
    use crate::robots::{Awareness, Brain, Job, JobKind, Noise};
    use bevy::ecs::world::CommandQueue;

    fn app() -> (App, Zone) {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        // The player out of the way, standing still (no footsteps).
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = zone.on_ground(250.0, -250.0);
        (app, zone)
    }

    fn walker(app: &mut App, zone: &Zone, post: Vec3) -> Entity {
        let mut queue = CommandQueue::default();
        let w = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::walker::spawn_walker_with(
                &mut c,
                zone,
                post,
                Job {
                    kind: JobKind::Watch,
                    site: post,
                    duration: 999.0,
                    exit: post,
                },
                false,
                130.0,
            )
        };
        queue.apply(app.world_mut());
        for _ in 0..90 {
            app.update();
        }
        *app.world_mut().get_mut::<Brain>(w).unwrap() = Brain::Working {
            at: post,
            left: 999.0,
        };
        w
    }

    /// The quietest noise at `at` the walker hears (its radius, m).
    fn threshold(app: &mut App, robot: Entity, at: Vec3) -> f32 {
        let (mut lo, mut hi) = (0.2f32, 400.0f32);
        for _ in 0..16 {
            let r = (lo * hi).sqrt();
            app.world_mut()
                .get_mut::<Awareness>(robot)
                .unwrap()
                .suspicion = 0.0;
            app.world_mut().write_message(Noise {
                at,
                radius: r,
                source: None,
            });
            app.update();
            let heard = app.world().get::<Awareness>(robot).unwrap().suspicion > 1e-4;
            if heard {
                hi = r;
            } else {
                lo = r;
            }
        }
        hi
    }

    fn to_world(g: &Graph, room: u16, up: f32) -> Vec3 {
        let k = g.room.iter().position(|r| *r == room).unwrap();
        let floor = g.levels.as_ref().map_or(g.floor, |l| l[k]);
        let local = Vec3::new(
            g.origin + ((k % g.n) as f32 + 0.5) * CELL,
            floor + up,
            g.origin + ((k / g.n) as f32 + 0.5) * CELL,
        );
        g.inv.inverse().transform_point3(local)
    }

    #[test]
    fn robots_hear_what_you_hear() {
        let (mut app, zone) = app();
        let graph_of = |app: &mut App, prefab: &str| -> usize {
            let e = app
                .world_mut()
                .query::<(Entity, &Building)>()
                .iter(app.world())
                .find(|(_, b)| b.prefab == prefab)
                .unwrap()
                .0;
            app.world()
                .resource::<Hearing>()
                .graphs
                .iter()
                .position(|g| g.building == e)
                .unwrap()
        };
        // (name, the noise, where the walker stands; a door to open first)
        let mut pairs: Vec<(String, Vec3, Vec3, Option<Entity>)> = Vec::new();
        {
            let k = graph_of(&mut app, "house_brick");
            let h = app.world().resource::<Hearing>();
            let g = &h.graphs[k];
            let (door, a, b, ..) = g.doors[0];
            let inside = if a == OUTSIDE { b } else { a };
            let noise = to_world(g, inside, 0.3);
            let out = to_world(g, OUTSIDE, 0.0);
            let away = (out - noise).with_y(0.0).normalize_or_zero();
            let post = zone.on_ground(noise.x + away.x * 7.0, noise.z + away.z * 7.0);
            pairs.push(("house, door shut".into(), noise, post, None));
            pairs.push(("house, door open".into(), noise, post, Some(door)));
        }
        {
            let k = graph_of(&mut app, "ranger_tower");
            let h = app.world().resource::<Hearing>();
            let g = &h.graphs[k];
            let cabin = (1..g.rooms as u16).next().unwrap();
            let noise = to_world(g, cabin, 0.3);
            let post = zone.on_ground(noise.x + 6.0, noise.z);
            pairs.push(("the tower's cabin, from below".into(), noise, post, None));
        }
        {
            let k = graph_of(&mut app, "bunker_marrow");
            let h = app.world().resource::<Hearing>();
            let g = &h.graphs[k];
            let deep = (1..g.rooms as u16)
                .min_by(|a, b| to_world(g, *a, 0.0).y.total_cmp(&to_world(g, *b, 0.0).y))
                .unwrap();
            let noise = to_world(g, deep, 0.3);
            let post = zone.on_ground(noise.x, noise.z);
            pairs.push((
                "the bunker's deepest room, from above".into(),
                noise,
                post,
                None,
            ));
            // Its way in: inside the kiosk's door, from the ground at its
            // mouth.
            let r = extraction_core::underground::raster("bunker_marrow").unwrap();
            let p = &r.portals[0];
            let k = extraction_core::underground::KIOSK[1];
            let world = g.inv.inverse();
            let at = |z: f32, y: f32| {
                let q = p.to_site([0.0, z]);
                world.transform_point3(Vec3::new(q[0], y, q[1]))
            };
            let noise = at(-k + 1.0, p.top() + 0.3);
            let mouth = at(-k - 5.0, 0.0);
            let post = zone.on_ground(mouth.x, mouth.z);
            pairs.push((
                "the bunker's way in, from its mouth".into(),
                noise,
                post,
                None,
            ));
            // The door of the kiosk the noise is in.
            let room = g.place(noise).map(|p| p.0);
            let kiosk = g
                .doors
                .iter()
                .find(|d| d.2 == OUTSIDE && Some(d.1) == room && d.0 != Entity::PLACEHOLDER)
                .map(|d| d.0);
            if kiosk.is_some() {
                pairs.push(("the same, its door open".into(), noise, post, kiosk));
            }
        }
        let mut worst = 0.0f32;
        for (name, noise, post, open) in pairs {
            if let Some(d) = open {
                if let Some(mut door) = app.world_mut().get_mut::<Door>(d) {
                    door.open = true;
                    door.angle = 1.5;
                }
                app.update();
            }
            let robot = walker(&mut app, &zone, post);
            let ear = app
                .world()
                .get::<GlobalTransform>(robot)
                .unwrap()
                .translation();
            // The same distance off in the open, on the far side of it.
            let clear = ear + (ear - noise).with_y(0.0).normalize_or_zero() * noise.distance(ear);
            let clear = clear.with_y(
                zone.ground(clear.x, clear.z)
                    + (noise.y - zone.ground(noise.x, noise.z)).clamp(0.0, 2.0),
            );
            let r_through = threshold(&mut app, robot, noise);
            let r_open = threshold(&mut app, robot, clear);
            // How much louder it must be made through than in the open,
            // distance for distance: the robot's loss, dB.
            let robot_db =
                20.0 * ((r_through / noise.distance(ear)) / (r_open / clear.distance(ear))).log10();
            let player_db = app
                .world()
                .resource::<Hearing>()
                .loss(Some(&zone), noise, ear, None);
            eprintln!(
                "{name}: the player loses {player_db:.1} dB; the walker hears it {robot_db:.1} dB down"
            );
            worst = worst.max((robot_db - player_db).abs());
            app.world_mut().entity_mut(robot).despawn();
        }
        assert!(worst < 1.0, "player and robot disagree by {worst:.1} dB");
    }
}
