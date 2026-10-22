//! Indoor navigation for bots.
//!
//! The Zone's nav grid ([`crate::robots::walker::Nav`]) blocks every
//! building's whole footprint: robots don't go indoors. Players do, so a
//! sim bot keeps a small fine grid per building, rasterised from the same
//! prefab geometry the colliders are built from (walls, furniture, steps),
//! and knows where the doors are. A route indoors is: the Zone grid to an
//! entrance, then this grid to the loot, opening doors on the way.

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use extraction_core::nav::{BLOCKED, NavGrid, Obstacle};

use crate::world::buildings::{Building, DoorLeaf, Shape, prefab};

/// Cell size of the indoor grids, metres.
pub const CELL: f32 = 0.25;
/// Grid extends this far past the footprint, to reach the doorsteps.
const MARGIN: f32 = 3.0;
/// The player capsule's radius plus a little.
const CLEARANCE: f32 = 0.22;
/// Geometry in this band above the floor blocks walking (below it you step
/// up; above it you duck under — well, you'd hit your head, but nothing
/// that low is above floor level indoors).
const BAND: (f32, f32) = (0.42, 1.75);
/// How far an open door has swung, radians (`swing_doors`: 1.75 per unit).
const OPEN_SWING: f32 = 1.75;
/// Loot higher than this above the floor is out of reach (upper floors).
pub const MAX_REACH_UP: f32 = 2.1;
/// The most one cell's floor may stand over its neighbour's and still be
/// walked between (a stair's riser is 0.2): under the body's own step
/// (`player::STEP_HEIGHT`, 0.4), or a route goes up a silt bank's flank
/// where it stands 0.42 over the ground and the body can't.
const STEP_UP: f32 = 0.35;

#[derive(Clone, Debug)]
pub struct DoorInfo {
    pub entity: Entity,
    /// The doorway as a segment across the gap, building-local (x, z).
    pub a: Vec2,
    pub b: Vec2,
    /// Where to stand outside it, building-local, if it's an outside door.
    pub outside: Option<Vec2>,
}

/// One building as a bot sees it.
#[derive(Clone, Debug)]
pub struct BuildingPlan {
    pub name: String,
    pub entity: Entity,
    pub xf: Transform,
    pub grid: NavGrid,
    /// Walking level, building-local y.
    pub floor: f32,
    pub doors: Vec<DoorInfo>,
    /// Places to come in from (building-local): outside doors, or for open
    /// structures, a ring of points round them.
    pub entries: Vec<Vec2>,
    /// Half extents of the footprint, building-local.
    pub half: Vec2,
    /// Cells outside the walls (reachable from the grid's edge with the
    /// doors shut).
    pub outside: Vec<bool>,
    /// Bounds of the cells inside the walls, building-local.
    pub inner: Option<(Vec2, Vec2)>,
    /// For a building on more than one level (an underground site): the
    /// floor under each cell, building-local y (NaN where there's none).
    /// A point belongs to the plan only near its cell's floor.
    pub levels: Option<Vec<f32>>,
    /// An underground site's cells shut only by heat (any that hurts):
    /// (cell, heat zone, its cost once that zone's vented).
    pub hot: Vec<(usize, u16, u8)>,
    /// An underground site's live room's wet cells: open in the plan, as
    /// they are once isolated; the raider shuts them (`shut_live`), since
    /// it can't tell live from cut and a wrong guess kills.
    pub live: Vec<usize>,
    /// An underground site's gas pockets' cells: open in the plan (gas
    /// doesn't stop feet); the raider shuts them (`shut_live`), since it
    /// carries no canary and can't tell a pocket that's gone up.
    pub gas: Vec<usize>,
}

impl BuildingPlan {
    /// The live room's water and the gas shut, for a raider that won't
    /// wade the one or breathe the other.
    pub fn shut_live(&mut self) {
        for &k in self.live.iter().chain(&self.gas) {
            self.grid.cost[k] = extraction_core::nav::BLOCKED;
        }
    }

    /// Heat zones vented: their cells walked again.
    pub fn vent(&mut self, zones: &[usize]) {
        let grid = &mut self.grid;
        self.hot.retain(|&(k, z, cost)| {
            let open = zones.contains(&(z as usize));
            if open {
                grid.cost[k] = cost;
            }
            !open
        });
    }

    pub fn to_local(&self, p: Vec3) -> Vec3 {
        self.xf.compute_affine().inverse().transform_point3(p)
    }

    pub fn to_world(&self, p: Vec2) -> Vec3 {
        self.xf.transform_point(Vec3::new(p.x, self.floor, p.y))
    }

    pub fn world2(&self, p: Vec2) -> Vec2 {
        let w = self.to_world(p);
        Vec2::new(w.x, w.z)
    }

    /// The floor under a building-local point, on a plan with levels.
    fn level(&self, l: Vec3) -> Option<f32> {
        let levels = self.levels.as_ref()?;
        let (i, j) = self.grid.cell_of([l.x, l.z])?;
        let y = levels[j * self.grid.n + i];
        (!y.is_nan()).then_some(y)
    }

    /// The floor under a building-local point: its level's, or the
    /// building's.
    pub fn floor_at(&self, p: Vec2) -> f32 {
        self.level(Vec3::new(p.x, self.floor, p.y))
            .unwrap_or(self.floor)
    }

    /// Whether a world point is inside (or on) the building's footprint
    /// (on a plan with levels: over one of its floors, near it).
    pub fn contains(&self, p: Vec3, pad: f32) -> bool {
        let l = self.to_local(p);
        if self.levels.is_some() {
            // The ground round a kiosk is the plan's, but it isn't in.
            let out = self
                .grid
                .cell_of([l.x, l.z])
                .is_some_and(|(i, j)| self.outside[j * self.grid.n + i]);
            return !out
                && self
                    .level(l)
                    .is_some_and(|y| l.y > y - 0.6 && l.y < y + MAX_REACH_UP + 0.5);
        }
        l.x.abs() <= self.half.x + pad && l.z.abs() <= self.half.y + pad
    }

    /// Whether a world point is within the indoor grid.
    pub fn in_grid(&self, p: Vec3) -> bool {
        let l = self.to_local(p);
        if self.levels.is_some() {
            return self.level(l).is_some_and(|y| (l.y - y).abs() < 2.5);
        }
        self.grid.cell_of([l.x, l.z]).is_some()
    }

    /// Whether loot at this world point can be reached from the floor.
    pub fn reachable_height(&self, p: Vec3) -> bool {
        let l = self.to_local(p);
        l.y - self.level(l).unwrap_or(self.floor) <= MAX_REACH_UP
    }

    /// Whether a building-local point is within the walls.
    pub fn indoors(&self, p: Vec2) -> bool {
        self.inner
            .is_some_and(|(lo, hi)| p.cmpge(lo - 0.4).all() && p.cmple(hi + 0.4).all())
    }

    /// An indoor path between two building-local points, as local
    /// waypoints (both ends snapped to walkable cells).
    pub fn path(&self, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
        self.grid
            .path(from.to_array(), to.to_array())
            .map(|p| p.into_iter().map(Vec2::from_array).collect())
    }

    /// The doors a local segment walks through, with where it crosses.
    pub fn doors_on(&self, a: Vec2, b: Vec2) -> Vec<(Entity, Vec2)> {
        let mut out: Vec<(f32, Entity, Vec2)> = self
            .doors
            .iter()
            .filter_map(|d| crossing(a, b, d.a, d.b).map(|(t, p)| (t, d.entity, p)))
            .collect();
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
        out.into_iter().map(|(_, e, p)| (e, p)).collect()
    }
}

/// Where segment `a`-`b` crosses `c`-`d`: (t along a-b, point).
fn crossing(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<(f32, Vec2)> {
    let r = b - a;
    let s = d - c;
    let den = r.perp_dot(s);
    if den.abs() < 1e-6 {
        return None;
    }
    let t = (c - a).perp_dot(s) / den;
    let u = (c - a).perp_dot(r) / den;
    // A little slack on the door's ends: the jambs are close enough.
    ((0.0..=1.0).contains(&t) && (-0.15..=1.15).contains(&u)).then(|| (t, a + r * t))
}

/// Builds plans for every generated building in the world.
pub fn plan_buildings(
    buildings: &Query<(Entity, &Building, &Transform)>,
    doors: &Query<(Entity, &DoorLeaf, &ChildOf)>,
) -> Vec<BuildingPlan> {
    let mut out = Vec::new();
    for (e, b, xf) in buildings.iter() {
        let Some(geo) = prefab(&b.prefab) else {
            continue;
        };
        if let Some(r) = extraction_core::underground::raster(&b.prefab) {
            let mine: Vec<(Entity, &DoorLeaf)> = doors
                .iter()
                .filter(|(_, _, p)| p.parent() == e)
                .map(|(d, leaf, _)| (d, leaf))
                .collect();
            out.push(plan_site(e, b, xf, &geo, r, &mine));
            continue;
        }
        let foot = crate::world::structures::footprint(&b.prefab);
        let half = Vec2::new(foot.x * 0.5, foot.z * 0.5);
        let mine: Vec<(Entity, &DoorLeaf)> = doors
            .iter()
            .filter(|(_, _, p)| p.parent() == e)
            .map(|(d, leaf, _)| (d, leaf))
            .collect();
        let floor = mine
            .iter()
            .map(|(_, l)| l.0.hinge.y)
            .min_by(|a, b| a.total_cmp(b))
            .or(geo.floor)
            .unwrap_or(0.0);
        let extent = half.x.max(half.y) + MARGIN;
        let n = ((extent * 2.0) / CELL).ceil() as usize;
        let mut grid = NavGrid {
            n,
            cell: CELL,
            origin: -extent,
            cost: vec![1; n * n],
            lanes: Vec::new(),
        };
        // A building stepped down a slope (the ore mill): each cell's own
        // floor, stairs and all, and each piece in the way or not at the
        // floor under it.
        let levels: Option<Vec<f32>> = crate::world::fell::stepped_floor(&b.prefab, Vec2::ZERO)
            .map(|_| {
                (0..n * n)
                    .map(|k| {
                        let c = Vec2::from(grid.centre(k % n, k / n));
                        crate::world::fell::stepped_floor(&b.prefab, c).unwrap_or(floor)
                    })
                    .collect()
            })
            // A drowned building: silt inside to the sills, banked down to
            // the ground out of its doorway. Each cell's floor is what's
            // under it (the ground round it at 0), so the bank's flanks by
            // the wall are a drop, not a way in.
            .or_else(|| {
                b.prefab.ends_with("_drowned").then(|| {
                    (0..n * n)
                        .map(|k| {
                            let c = Vec2::from(grid.centre(k % n, k / n));
                            geo.pieces
                                .iter()
                                .filter(|p| p.collide)
                                .filter_map(|p| surface_under(p.shape, p.at, c, floor + 0.1))
                                .fold(0.0f32, f32::max)
                        })
                        .collect()
                })
            });
        for piece in geo.pieces.iter().filter(|p| p.collide) {
            let c = piece.at.translation;
            let under = levels
                .as_ref()
                .and_then(|l| grid.cell_of([c.x, c.z]).map(|(i, j)| l[j * n + i]))
                .unwrap_or(floor);
            // What the floor itself is made of there (the silt, its bank,
            // buried at its foot) is walked on, not round: the levels say
            // where it rises.
            if levels.is_some()
                && surface_under(piece.shape, piece.at, c.xz(), under + 0.15)
                    .is_some_and(|y| y <= under + 0.15)
            {
                continue;
            }
            if let Some(ob) = obstacle(piece.shape, piece.at, under) {
                grid.block(ob, CLEARANCE);
            }
        }
        // Where one floor meets another more than a step up or down (a
        // retaining wall, a stair's open side), neither edge is a way
        // across.
        if let Some(l) = &levels {
            let mut cliff = Vec::new();
            for j in 0..n {
                for i in 0..n {
                    let y = l[j * n + i];
                    let steep = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(di, dj)| {
                            let (a, c) = (i as i64 + di, j as i64 + dj);
                            a >= 0
                                && c >= 0
                                && (a as usize) < n
                                && (c as usize) < n
                                && (l[c as usize * n + a as usize] - y).abs() > STEP_UP
                        });
                    if steep {
                        cliff.push(j * n + i);
                    }
                }
            }
            for k in cliff {
                grid.cost[k] = BLOCKED;
            }
        }
        // Open door leaves stand out from the wall (doors swing about 100
        // degrees; see `swing_doors`): in the way of anyone walking past.
        for (_, leaf) in &mine {
            let def = leaf.0;
            let open = Quat::from_rotation_y(def.yaw + OPEN_SWING);
            let tip = def.hinge + open * Vec3::new(def.width, 0.0, 0.0);
            let mid = (def.hinge + tip) * 0.5;
            let along = (tip - def.hinge).xz().normalize_or_zero();
            grid.block(
                Obstacle::Rect {
                    at: [mid.x, mid.z],
                    half: [def.width * 0.5, 0.04],
                    yaw: along.y.atan2(along.x),
                },
                CLEARANCE,
            );
        }
        // Doorways, local: the gap the leaf closes.
        let mut infos: Vec<DoorInfo> = mine
            .iter()
            .map(|(d, leaf)| {
                let def = leaf.0;
                let rot = Quat::from_rotation_y(def.yaw);
                let a3 = def.hinge;
                let b3 = def.hinge + rot * Vec3::new(def.width, 0.0, 0.0);
                DoorInfo {
                    entity: *d,
                    a: Vec2::new(a3.x, a3.z),
                    b: Vec2::new(b3.x, b3.z),
                    outside: None,
                }
            })
            .collect();
        // Which cells are "outside": flood from the border with the doors
        // shut.
        let mut shut = grid.clone();
        for d in &infos {
            let mid = (d.a + d.b) * 0.5;
            let along = (d.b - d.a).normalize_or_zero();
            shut.block(
                Obstacle::Rect {
                    at: mid.to_array(),
                    half: [(d.b - d.a).length() * 0.5 + 0.15, CELL],
                    yaw: along.y.atan2(along.x),
                },
                0.0,
            );
        }
        let outside = flood_from_border(&shut);
        let is_out = |p: Vec2| {
            shut.cell_of(p.to_array())
                .is_some_and(|(i, j)| outside[j * shut.n + i])
        };
        let mut entries = Vec::new();
        for d in &mut infos {
            let mid = (d.a + d.b) * 0.5;
            let along = (d.b - d.a).normalize_or_zero();
            let normal = Vec2::new(-along.y, along.x);
            // In a hall with open bays both sides flood as "outside": take
            // the side beyond the footprint.
            let beyond = |p: Vec2| p.x.abs() > half.x || p.y.abs() > half.y;
            let side = [1.0, -1.0]
                .into_iter()
                .filter(|s| is_out(mid + normal * *s * 0.9))
                .max_by_key(|s| beyond(mid + normal * *s * 0.9));
            if let Some(side) = side {
                let stand = mid + normal * side * 1.3;
                d.outside = Some(stand);
                entries.push(stand);
            }
        }
        if infos.is_empty() {
            // An open structure: come in from wherever is open, all round
            // it 1.2 m out (on a rectangle: on an ellipse the diagonals
            // fall inside a big building's corners).
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                let out = half + Vec2::splat(1.2);
                let p = d / (d.x.abs() / out.x).max(d.y.abs() / out.y);
                if grid.walkable(p.to_array()) {
                    entries.push(p);
                }
            }
        }
        let mut inner: Option<(Vec2, Vec2)> = None;
        for j in 0..grid.n {
            for i in 0..grid.n {
                let k = j * grid.n + i;
                if grid.cost[k] != BLOCKED && !outside[k] {
                    let c = Vec2::from_array(grid.centre(i, j));
                    inner = Some(match inner {
                        None => (c, c),
                        Some((lo, hi)) => (lo.min(c), hi.max(c)),
                    });
                }
            }
        }
        out.push(BuildingPlan {
            outside,
            inner,
            name: b.prefab.clone(),
            entity: e,
            xf: *xf,
            grid,
            floor,
            doors: infos,
            entries,
            half,
            levels,
            hot: Vec::new(),
            live: Vec::new(),
            gas: Vec::new(),
        });
    }
    out
}

/// An underground site's plan: its rooms, corridors and stairs at their
/// own floors, and a patch of ground round each kiosk to come in from.
/// One grid serves every level: a layout never has a stair over a room
/// (`underground::check`), and the rock between keeps them apart.
fn plan_site(
    e: Entity,
    b: &Building,
    xf: &Transform,
    geo: &crate::world::buildings::Geo,
    r: &'static extraction_core::underground::Raster,
    mine: &[(Entity, &DoorLeaf)],
) -> BuildingPlan {
    use extraction_core::underground::{APRON, KIOSK, WALL};
    // The ground round a kiosk, out past its apron; not where a room
    // under it comes near.
    let zone = |p: Vec2| -> Option<f32> {
        r.portals.iter().find_map(|portal| {
            let q = portal.to_portal(p.to_array());
            (q[0].abs() < KIOSK[0] + WALL + APRON + 3.0
                && q[1].abs() < KIOSK[1] + WALL + APRON + 3.0)
                .then_some(portal.y)
        })
    };
    let lo = Vec2::from(r.origin);
    let hi = lo + Vec2::new(r.nx as f32, r.nz as f32) * CELL;
    let extent = ((lo.abs().max(hi.abs()).max_element() + MARGIN) / CELL).ceil() * CELL;
    let n = (extent * 2.0 / CELL).round() as usize;
    let mut grid = NavGrid {
        n,
        cell: CELL,
        origin: -extent,
        cost: vec![BLOCKED; n * n],
        lanes: Vec::new(),
    };
    let mut levels = vec![f32::NAN; n * n];
    let mut ground = vec![false; n * n];
    for j in 0..n {
        for i in 0..n {
            let p = Vec2::from(grid.centre(i, j));
            let k = j * n + i;
            if let Some(f) = r.floor_at(p.to_array()) {
                levels[k] = f;
            } else if let Some(y) = zone(p) {
                // Not over (or beside) a room below.
                let near_room = (-4..=4).any(|dj: i32| {
                    (-4..=4).any(|di: i32| {
                        r.at((p + Vec2::new(di as f32, dj as f32) * CELL).to_array())
                            .flat()
                    })
                });
                if !near_room {
                    levels[k] = y;
                    ground[k] = true;
                }
            }
        }
    }
    // A cell's width off the rock: the body is wider than half a cell.
    for j in 0..n {
        for i in 0..n {
            if levels[j * n + i].is_nan() {
                continue;
            }
            let clear = (-1i64..=1).all(|dj| {
                (-1i64..=1).all(|di| {
                    let (a, c) = (i as i64 + di, j as i64 + dj);
                    a >= 0
                        && c >= 0
                        && (a as usize) < n
                        && (c as usize) < n
                        && !levels[c as usize * n + a as usize].is_nan()
                })
            });
            // A pit's floor: not a way at all. (Heat nothing crosses is
            // shut after, once the colliders are in: see `hot`.)
            let p = grid.centre(i, j);
            let barred = r.cell_of(p).is_some_and(|(a, c)| r.pit[c * r.nx + a]);
            if barred {
                grid.cost[j * n + i] = extraction_core::nav::BLOCKED;
            } else if clear {
                // Wading is slow; deep enough to swim, slower.
                let w = r.water_at(grid.centre(i, j));
                grid.cost[j * n + i] = if w >= extraction_core::underground::DEEP {
                    5
                } else if w > 0.0 {
                    3
                } else {
                    1
                };
            }
        }
    }
    // Colliders in the way at their own floor (tilted ones are the stairs'
    // ramps, walked on).
    for piece in geo.pieces.iter().filter(|p| p.collide) {
        let (_, pitch, roll) = piece.at.rotation.to_euler(EulerRot::YXZ);
        if pitch.abs() > 0.05 || roll.abs() > 0.05 {
            continue;
        }
        let c = piece.at.translation;
        let Some((i, j)) = grid.cell_of([c.x, c.z]) else {
            continue;
        };
        let floor = levels[j * n + i];
        let floor = if floor.is_nan() { r.floor } else { floor };
        if let Some(ob) = obstacle(piece.shape, piece.at, floor) {
            grid.block(ob, CLEARANCE);
        }
    }
    for (_, leaf) in mine {
        let def = leaf.0;
        let open = Quat::from_rotation_y(def.yaw + OPEN_SWING);
        let tip = def.hinge + open * Vec3::new(def.width, 0.0, 0.0);
        let mid = (def.hinge + tip) * 0.5;
        let along = (tip - def.hinge).xz().normalize_or_zero();
        grid.block(
            Obstacle::Rect {
                at: [mid.x, mid.z],
                half: [def.width * 0.5, 0.04],
                yaw: along.y.atan2(along.x),
            },
            CLEARANCE,
        );
    }
    // Heat nothing crosses: shut, each cell's cost kept for when its zone
    // is vented (`BuildingPlan::vent`).
    let mut hot = Vec::new();
    if let Some(l) = extraction_core::underground::layout(&b.prefab) {
        for j in 0..n {
            for i in 0..n {
                let k = j * n + i;
                let Some(z) = r.heat_at(grid.centre(i, j)) else {
                    continue;
                };
                // Any heat that hurts: a slow burn kills as surely, over
                // a long search (Cinder's burner gallery, seed 2).
                if l.heat[z as usize].level <= 0.0 {
                    continue;
                }
                if grid.cost[k] != extraction_core::nav::BLOCKED {
                    hot.push((k, z, grid.cost[k]));
                    grid.cost[k] = extraction_core::nav::BLOCKED;
                }
            }
        }
    }
    // Live water's cells (see `live`; the crawlers keep out of it the same
    // way, `underground::block_works`).
    let mut live = Vec::new();
    if let Some(v) = extraction_core::underground::layout(&b.prefab).and_then(|l| {
        let v = l.live.as_ref()?;
        l.rooms.iter().position(|room| room.id == v.room)
    }) {
        for j in 0..n {
            for i in 0..n {
                let c = grid.centre(i, j);
                let wet = r.water_at(c) > 0.05;
                let in_room = r.cell_of(c).is_some_and(|(a, z)| {
                    matches!(r.cells[z * r.nx + a], extraction_core::underground::Cell::Room(k) if k as usize == v)
                });
                if wet && in_room && grid.cost[j * n + i] != extraction_core::nav::BLOCKED {
                    live.push(j * n + i);
                }
            }
        }
    }
    // Gas pockets' cells (see `gas`).
    let mut gas = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            let foul = r
                .cell_of(grid.centre(i, j))
                .is_some_and(|(a, z)| r.gas[z * r.nx + a].is_some());
            if foul && grid.cost[k] != extraction_core::nav::BLOCKED {
                gas.push(k);
            }
        }
    }
    let is_ground = |p: Vec2| {
        grid.cell_of(p.to_array())
            .is_some_and(|(i, j)| ground[j * n + i])
    };
    let mut entries = Vec::new();
    let doors: Vec<DoorInfo> = mine
        .iter()
        .map(|(d, leaf)| {
            let def = leaf.0;
            let rot = Quat::from_rotation_y(def.yaw);
            let a3 = def.hinge;
            let b3 = def.hinge + rot * Vec3::new(def.width, 0.0, 0.0);
            let (a, c) = (a3.xz(), b3.xz());
            let mid = (a + c) * 0.5;
            let along = (c - a).normalize_or_zero();
            let normal = Vec2::new(-along.y, along.x);
            // A kiosk's door: stand out on the ground, past the apron.
            let outside = [1.0f32, -1.0]
                .into_iter()
                .find(|s| is_ground(mid + normal * *s * 0.9))
                .map(|s| mid + normal * s * 2.6);
            entries.extend(outside);
            DoorInfo {
                entity: *d,
                a,
                b: c,
                outside,
            }
        })
        .collect();
    let mut inner: Option<(Vec2, Vec2)> = None;
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            if grid.cost[k] != BLOCKED && !ground[k] {
                let c = Vec2::from_array(grid.centre(i, j));
                inner = Some(match inner {
                    None => (c, c),
                    Some((lo, hi)) => (lo.min(c), hi.max(c)),
                });
            }
        }
    }
    let foot = extraction_core::underground::FOOTPRINT;
    BuildingPlan {
        name: b.prefab.clone(),
        entity: e,
        xf: *xf,
        grid,
        floor: r.floor,
        doors,
        entries,
        half: Vec2::new(foot[0] * 0.5, foot[2] * 0.5),
        outside: ground,
        inner,
        levels: Some(levels),
        hot,
        live,
        gas,
    }
}

/// A collider piece as an obstacle on the floor, if it's in the way.
/// The top of a piece under building-local `p` (x, z), coming down from
/// `from`: where a ray straight down from there first meets it. None if
/// it misses, or starts inside it.
fn surface_under(shape: Shape, at: Transform, p: Vec2, from: f32) -> Option<f32> {
    let half = match shape {
        Shape::Box(s) => s * 0.5,
        Shape::Prism(s) => Vec3::new(s.x, s.y * 0.5, s.z) * 0.5,
        Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
    };
    let inv = at.rotation.inverse();
    let o = inv * (Vec3::new(p.x, from, p.y) - at.translation);
    let d = inv * Vec3::NEG_Y;
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for k in 0..3 {
        if d[k].abs() < 1e-6 {
            if o[k].abs() > half[k] {
                return None;
            }
            continue;
        }
        let (a, c) = ((-half[k] - o[k]) / d[k], (half[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(c));
        t1 = t1.min(a.max(c));
    }
    if let Shape::Cylinder(r, _) = shape {
        let q = o + d * t0.max(0.0);
        if q.x * q.x + q.z * q.z > r * r {
            return None;
        }
    }
    (t0 <= t1 && t0 >= 0.0).then(|| from - t0)
}

fn obstacle(shape: Shape, at: Transform, floor: f32) -> Option<Obstacle> {
    let (size, circle) = match shape {
        Shape::Box(s) => (s, None),
        Shape::Prism(s) => (Vec3::new(s.x, s.y * 0.5, s.z), None),
        Shape::Cylinder(r, h) => (Vec3::new(r * 2.0, h, r * 2.0), Some(r)),
    };
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for sx in [-0.5, 0.5] {
        for sy in [-0.5, 0.5] {
            for sz in [-0.5, 0.5] {
                let c = at.transform_point(size * Vec3::new(sx, sy, sz));
                lo = lo.min(c);
                hi = hi.max(c);
            }
        }
    }
    if hi.y < floor + BAND.0 || lo.y > floor + BAND.1 {
        return None;
    }
    let c = at.translation;
    if let Some(r) = circle {
        return Some(Obstacle::Circle {
            at: [c.x, c.z],
            radius: r,
        });
    }
    let (yaw, pitch, roll) = at.rotation.to_euler(EulerRot::YXZ);
    if pitch.abs() > 0.05 || roll.abs() > 0.05 {
        // Tilted (ramps, roof pieces): its box from above.
        return Some(Obstacle::Rect {
            at: [(lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5],
            half: [(hi.x - lo.x) * 0.5, (hi.z - lo.z) * 0.5],
            yaw: 0.0,
        });
    }
    // `Obstacle` turns the other way from Bevy's yaw.
    Some(Obstacle::Rect {
        at: [c.x, c.z],
        half: [size.x * 0.5, size.z * 0.5],
        yaw: -yaw,
    })
}

/// Walkable cells connected to the grid's border.
fn flood_from_border(g: &NavGrid) -> Vec<bool> {
    let n = g.n;
    let mut seen = vec![false; n * n];
    let mut stack = Vec::new();
    for k in 0..n {
        for (i, j) in [(k, 0), (k, n - 1), (0, k), (n - 1, k)] {
            let c = j * n + i;
            if g.cost[c] != BLOCKED && !seen[c] {
                seen[c] = true;
                stack.push((i, j));
            }
        }
    }
    while let Some((i, j)) = stack.pop() {
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let c = nj as usize * n + ni as usize;
            if g.cost[c] != BLOCKED && !seen[c] {
                seen[c] = true;
                stack.push((ni as usize, nj as usize));
            }
        }
    }
    seen
}

/// Debugging: each plan's grid as text (`#` blocked, `.` open, `D` doorway,
/// `E` entry, `L` loot spot), one file per building.
pub fn dump_plans(plans: &[BuildingPlan], spots: &[Vec3], dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    for (k, p) in plans.iter().enumerate() {
        let g = &p.grid;
        let mut rows = vec![vec![b'.'; g.n]; g.n];
        for j in 0..g.n {
            for i in 0..g.n {
                if g.cost[j * g.n + i] == BLOCKED {
                    rows[j][i] = b'#';
                }
            }
        }
        let mut mark = |q: Vec2, c: u8| {
            if let Some((i, j)) = g.cell_of(q.to_array()) {
                rows[j][i] = c;
            }
        };
        for d in &p.doors {
            for t in 0..=4 {
                mark(d.a.lerp(d.b, t as f32 / 4.0), b'D');
            }
        }
        for e in &p.entries {
            mark(*e, b'E');
        }
        for s in spots.iter().filter(|s| p.contains(**s, 0.5)) {
            mark(p.to_local(*s).xz(), b'L');
        }
        let text: Vec<String> = rows
            .into_iter()
            .map(|r| String::from_utf8(r).unwrap())
            .collect();
        let _ = std::fs::write(
            dir.join(format!("{k:02}_{}.txt", p.name)),
            format!(
                "{} at {:?}, floor {:.2}, {} doors\n{}\n",
                p.name,
                p.xf.translation,
                p.floor,
                p.doors.len(),
                text.join("\n")
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot::Bot;
    use crate::launch::LaunchOptions;
    use crate::rig::LocalPlayer;
    use bevy::ecs::system::RunSystemOnce;

    fn plans(app: &mut App) -> Vec<BuildingPlan> {
        app.world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap()
    }

    /// The ore mill's plan stands each bay at its own floor, and from the
    /// front door there's a way up the stairs to every bay and to every
    /// loot spot within reach of a floor (the top bay's included).
    #[test]
    fn the_mill_plan_climbs_every_bay() {
        use crate::world::fell::MILL_FLOORS;
        let mut world = World::new();
        let geo = prefab("ore_mill").unwrap();
        let root = world
            .spawn((
                Building {
                    prefab: "ore_mill".into(),
                },
                Transform::IDENTITY,
            ))
            .id();
        for d in &geo.doors {
            world.spawn((DoorLeaf(*d), ChildOf(root)));
        }
        let plans = world
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap();
        let plan = &plans[0];
        assert!(plan.levels.is_some(), "the mill's plan has its floors");
        let front = plan
            .doors
            .iter()
            .find_map(|d| d.outside.filter(|o| o.y < -20.0))
            .expect("a way in at the front door");
        for (k, (goal, f)) in [
            (Vec2::new(1.0, -17.0), MILL_FLOORS[0]),
            (Vec2::new(7.0, -3.0), MILL_FLOORS[1]),
            (Vec2::new(5.0, 4.0), MILL_FLOORS[2]),
            (Vec2::new(6.0, 15.0), MILL_FLOORS[3]),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(
                (plan.floor_at(goal) - f).abs() < 1e-3,
                "bay {k}'s floor is {:.2} at {goal}",
                plan.floor_at(goal)
            );
            let path = plan
                .path(front, goal)
                .unwrap_or_else(|| panic!("no way from the front door to bay {k}"));
            let end = *path.last().unwrap();
            assert!(end.distance(goal) < 0.5, "bay {k}: the path ends at {end}");
        }
        let mut tried = 0;
        for s in &geo.loot {
            if !plan.reachable_height(s.at) {
                continue;
            }
            tried += 1;
            let goal = s.at.xz();
            let cell = super::super::raider::reach_cell(plan, front, goal);
            assert!(
                cell.is_some_and(|c| super::super::raider::reaches(plan, c, goal)),
                "the {} spot at {:.2} is out of reach",
                s.table,
                s.at
            );
        }
        assert!(
            tried >= 6,
            "only {tried} loot spots within reach of a floor"
        );
    }

    /// A bot routed by a building plan gets through the front door and
    /// across the room.
    #[test]
    fn a_bot_walks_in_through_the_door() {
        let opts = LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let plans = plans(&mut app);
        let plan = plans
            .iter()
            .find(|p| p.name == "shop")
            .expect("the map has a shop")
            .clone();
        let entry = plan.entries[0];
        let start = plan.to_world(entry);
        let goal = Vec2::new(1.5, -1.5); // in the front room, by the counter
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let ground = app
            .world()
            .resource::<crate::world::Zone>()
            .ground(start.x, start.z);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(start.x, ground.max(start.y) + 0.1, start.z);
        app.update();
        let pos = app.world().get::<Transform>(player).unwrap().translation;
        let tasks = super::super::raider::indoor_route(&plan, pos, goal, 0.3).expect("a route");
        app.world_mut().entity_mut(player).insert(Bot::new(tasks));
        for _ in 0..1800 {
            app.update();
            if app.world().get::<Bot>(player).unwrap().done() {
                break;
            }
        }
        let t = app.world().get::<Transform>(player).unwrap().translation;
        let local = plan.to_local(t).xz();
        assert!(
            local.distance(goal) < 0.8,
            "the bot got to {local:?}, not {goal:?}"
        );
    }

    /// A bot that starts right at a shut door, inside its leaf's swing,
    /// steps back out of the swing before it opens it, then gets in (the
    /// leaf used to swing into it and pin it in the doorway: Kettle's
    /// gatekeeper's cottage, Marrow's bunker).
    #[test]
    fn a_bot_at_a_door_steps_back_before_opening_it() {
        let opts = LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let plans = plans(&mut app);
        let plan = plans
            .iter()
            .find(|p| p.name == "shop")
            .expect("the map has a shop")
            .clone();
        let door = plan
            .doors
            .iter()
            .find(|d| d.outside.is_some())
            .expect("an outside door")
            .clone();
        let mid = (door.a + door.b) * 0.5;
        let out = (door.outside.unwrap() - mid).normalize();
        // Right at the door, 0.3 m out.
        let start = plan.to_world(mid + out * 0.3);
        let goal = Vec2::new(1.5, -1.5);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let ground = app
            .world()
            .resource::<crate::world::Zone>()
            .ground(start.x, start.z);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = Vec3::new(start.x, ground.max(start.y) + 0.1, start.z);
        app.update();
        let pos = app.world().get::<Transform>(player).unwrap().translation;
        let tasks = super::super::raider::indoor_route(&plan, pos, goal, 0.3).expect("a route");
        let open = tasks
            .iter()
            .position(|t| matches!(t, crate::bot::Task::OpenDoor(_)))
            .expect("it opens the door");
        let stand = tasks[..open]
            .iter()
            .rev()
            .find_map(|t| match t {
                crate::bot::Task::WalkTo { to, .. } => Some(*to),
                _ => None,
            })
            .expect("it steps somewhere first");
        let stand = plan.to_local(Vec3::new(stand.x, 0.0, stand.y)).xz();
        assert!(
            stand.distance(mid) > 0.9,
            "out of the leaf's swing to open it ({:.2} m from the doorway)",
            stand.distance(mid)
        );
        app.world_mut().entity_mut(player).insert(Bot::new(tasks));
        for _ in 0..90 * 20 {
            app.update();
            if app.world().get::<Bot>(player).unwrap().done() {
                break;
            }
        }
        let t = app.world().get::<Transform>(player).unwrap().translation;
        let local = plan.to_local(t).xz();
        let bot = app.world().get::<Bot>(player).unwrap();
        assert!(
            local.distance(goal) < 0.8,
            "and in: at {local:?}, door mid {mid:?}, left {:?}, log {:?}",
            bot.tasks,
            bot.log
        );
    }

    /// A step just inside every outside door, and one on every bridge
    /// deck, lands on something built (so it sounds like boards or
    /// concrete, not the grass under it).
    #[test]
    fn every_floor_and_deck_has_a_surface() {
        use crate::render::materials::Mat;
        use crate::world::bridges::Bridge;
        use crate::world::surfaces::{Surfaces, underfoot};
        use avian3d::prelude::SpatialQuery;
        for map in crate::test_maps() {
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            let mut steps: Vec<(String, Vec3)> = Vec::new();
            for plan in plans(&mut app) {
                for d in &plan.doors {
                    let Some(entry) = d.outside else { continue };
                    // Earth floors are the ground's.
                    if plan.name == "barn" {
                        continue;
                    }
                    let mid = (d.a + d.b) * 0.5;
                    let inside = mid + (mid - entry).normalize() * 1.0;
                    let at = plan.xf.transform_point(Vec3::new(
                        inside.x,
                        plan.floor_at(inside),
                        inside.y,
                    ));
                    steps.push((format!("{} door", plan.name), at));
                }
            }
            let decks: Vec<Vec3> = app
                .world_mut()
                .query::<&Bridge>()
                .iter(app.world())
                .filter_map(|b| {
                    // The deck (what the feet meet): the middle one.
                    let deck: Vec<_> = b.pieces.iter().filter(|p| p.solid && !p.seen).collect();
                    let p = deck.get(deck.len() / 2)?;
                    Some(p.at.translation + p.at.rotation * Vec3::Y * p.size.y * 0.5)
                })
                .collect();
            steps.extend(decks.into_iter().map(|p| ("bridge".to_string(), p)));
            let found: Vec<(String, Vec3, Option<Mat>)> = app
                .world_mut()
                .run_system_once(
                    move |spatial: SpatialQuery, built: Query<(&GlobalTransform, &Surfaces)>| {
                        steps
                            .iter()
                            .map(|(n, p)| (n.clone(), *p, underfoot(&spatial, &built, *p)))
                            .collect::<Vec<_>>()
                    },
                )
                .unwrap();
            let missing: Vec<_> = found.iter().filter(|f| f.2.is_none()).collect();
            assert!(
                missing.is_empty(),
                "{map}: nothing built underfoot at {missing:?}"
            );
            assert!(
                found.iter().any(|f| f.2 == Some(Mat::WoodFloor)),
                "{map}: no floorboards anywhere"
            );
        }
    }

    /// A spot under a site in a room only its works open (heat vented, the
    /// crane ridden): the raider's plans bar those, as the raider can't.
    fn behind_the_works(plan: &BuildingPlan, spot: Vec3) -> bool {
        let Some(l) = extraction_core::underground::layout(&plan.name) else {
            return false;
        };
        let p = plan.to_local(spot);
        let open = l.open_rooms();
        l.rooms.iter().zip(open).any(|(r, open)| {
            !open
                && (p.x - r.at[0]).abs() <= r.size[0] * 0.5
                && (p.z - r.at[1]).abs() <= r.size[1] * 0.5
        })
    }

    /// Every loot spot in every building can be reached from somewhere a
    /// bot can stand, along a clear line: a barn's open gaps join its floor
    /// to the outside, which once left nothing in a barn reachable, and a
    /// thing just outside a window was reached for through the wall.
    #[test]
    fn every_loot_spot_is_reached_from_its_own_side() {
        for map in crate::test_maps() {
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let plans = plans(&mut app);
            let spots: Vec<Vec3> = app
                .world()
                .get_resource::<crate::world::buildings::BuildingLoot>()
                .map(|l| l.0.iter().map(|s| s.at).collect())
                .unwrap_or_default();
            let mut failed = Vec::new();
            let mut tried = 0;
            for plan in plans.iter().filter(|p| !p.entries.is_empty()) {
                for spot in spots.iter().filter(|s| plan.contains(**s, 0.5)) {
                    if !plan.reachable_height(*spot) || behind_the_works(plan, *spot) {
                        continue;
                    }
                    tried += 1;
                    let goal = plan.to_local(*spot).xz();
                    match super::super::raider::reach_cell(plan, plan.entries[0], goal) {
                        Some(c) if super::super::raider::reaches(plan, c, goal) => {}
                        Some(c) => failed.push(format!(
                            "{map} {} at {:.0?}: spot {goal:.2?} reached from {c:.2?} through a wall",
                            plan.name, plan.xf.translation
                        )),
                        None => {
                            failed.push(format!(
                                "{map} {} at {:.0?}: spot {goal:.2?} out of reach",
                                plan.name, plan.xf.translation
                            ))
                        }
                    }
                }
            }
            assert!(tried > 0, "{map}: no loot spots tried");
            assert!(
                failed.is_empty(),
                "{} of {tried} loot spots failed:\n{}",
                failed.len(),
                failed.join("\n")
            );
        }
    }

    /// Every outside door of every building on the map lets a bot in, as
    /// far as the room behind it.
    #[test]
    fn every_door_lets_a_bot_in() {
        // Every valley's doors (`EXTRACTION_TEST_MAP` for just one).
        for map in crate::test_maps() {
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let plans = plans(&mut app);
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            let mut failed = Vec::new();
            let mut tried = 0;
            for plan in &plans {
                for d in &plan.doors {
                    let Some(entry) = d.outside else { continue };
                    // Held shut by an underground site's power: nobody's
                    // in that way till it's on (the site's own tests).
                    if app
                        .world()
                        .get::<crate::world::underground::PowerLocked>(d.entity)
                        .is_some()
                    {
                        continue;
                    }
                    let mid = (d.a + d.b) * 0.5;
                    // A metre and a half past the doorway, inside (or as near
                    // as the furniture allows).
                    let Some(goal) = super::super::raider::stand_near(
                        plan,
                        entry,
                        mid + (mid - entry).normalize() * 1.5,
                    ) else {
                        failed.push(format!(
                            "{map} {} door {:?}: nowhere inside",
                            plan.name, d.entity
                        ));
                        continue;
                    };
                    let start = plan.to_world(entry);
                    let ground = app
                        .world()
                        .resource::<crate::world::Zone>()
                        .ground(start.x, start.z);
                    {
                        let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
                        t.translation = Vec3::new(start.x, ground.max(start.y) + 0.05, start.z);
                    }
                    app.world_mut()
                        .get_mut::<crate::player::Body>(player)
                        .unwrap()
                        .velocity = Vec3::ZERO;
                    app.update();
                    let pos = app.world().get::<Transform>(player).unwrap().translation;
                    let Some(tasks) = super::super::raider::indoor_route(plan, pos, goal, 0.3)
                    else {
                        failed.push(format!("{map} {} door {:?}: no route", plan.name, d.entity));
                        continue;
                    };
                    tried += 1;
                    app.world_mut().entity_mut(player).insert(Bot::new(tasks));
                    for _ in 0..90 * 25 {
                        app.update();
                        if app.world().get::<Bot>(player).unwrap().done() {
                            break;
                        }
                    }
                    let t = app.world().get::<Transform>(player).unwrap().translation;
                    let got = plan.to_local(t).xz();
                    if got.distance(goal) > 0.9 {
                        failed.push(format!(
                        "{map} {} at {:.0?}: door {:?} entry {entry:.2?} goal {goal:.2?}, stopped at {got:.2?} (y {:.2})",
                        plan.name,
                        plan.xf.translation,
                        d.entity,
                        plan.to_local(t).y
                    ));
                    }
                }
            }
            assert!(tried > 0, "no doors tried");
            assert!(
                failed.is_empty(),
                "{} of {tried} doors failed:\n{}",
                failed.len(),
                failed.join("\n")
            );
        }
    }

    /// A drowned house has no doors, only a doorway with its silt banked
    /// down from it: from every way in its plan offers, a bot walks to
    /// stand by its loot.
    #[test]
    fn every_drowned_house_lets_a_bot_in() {
        for map in crate::test_maps() {
            // (Only valleys with drowned houses are worth loading.)
            if !crate::world::load_map_def(&map)
                .placements
                .iter()
                .any(|p| p.prefab.ends_with("_drowned"))
            {
                continue;
            }
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let plans = plans(&mut app);
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            let mut failed = Vec::new();
            let mut tried = 0;
            // One of each kind (they're the same building wherever they
            // stand), from every way in, each to one of its loot spots in
            // turn: in the suite's time budget.
            let mut seen = std::collections::HashSet::new();
            for plan in plans
                .iter()
                .filter(|p| p.name.ends_with("_drowned") && seen.insert(p.name.clone()))
            {
                let geo = prefab(&plan.name).unwrap();
                for (k, &entry) in plan.entries.iter().enumerate() {
                    if let Some(spot) = geo.loot.get(k % geo.loot.len().max(1)) {
                        let Some(goal) =
                            super::super::raider::stand_near(plan, entry, spot.at.xz())
                        else {
                            failed.push(format!("{map} {}: nowhere by {:?}", plan.name, spot.at));
                            continue;
                        };
                        let start = plan.to_world(entry);
                        let ground = app
                            .world()
                            .resource::<crate::world::Zone>()
                            .ground(start.x, start.z);
                        {
                            let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
                            t.translation = Vec3::new(start.x, ground + 0.05, start.z);
                        }
                        app.world_mut()
                            .get_mut::<crate::player::Body>(player)
                            .unwrap()
                            .velocity = Vec3::ZERO;
                        app.update();
                        let pos = app.world().get::<Transform>(player).unwrap().translation;
                        let Some(tasks) = super::super::raider::indoor_route(plan, pos, goal, 0.3)
                        else {
                            failed.push(format!(
                                "{map} {} entry {entry:.1?}: no route to {goal:.1?}",
                                plan.name
                            ));
                            continue;
                        };
                        tried += 1;
                        app.world_mut().entity_mut(player).insert(Bot::new(tasks));
                        for _ in 0..90 * 25 {
                            app.update();
                            if app.world().get::<Bot>(player).unwrap().done() {
                                break;
                            }
                        }
                        let t = app.world().get::<Transform>(player).unwrap().translation;
                        let got = plan.to_local(t).xz();
                        if got.distance(goal) > 0.9 {
                            failed.push(format!(
                                "{map} {} at {:.0?}: entry {entry:.2?} goal {goal:.2?}, stopped at {got:.2?} (y {:.2})",
                                plan.name,
                                plan.xf.translation,
                                plan.to_local(t).y
                            ));
                        }
                    }
                }
            }
            println!("{map}: {tried} walks into drowned houses");
            assert!(
                failed.is_empty(),
                "{} of {tried} walks failed:\n{}",
                failed.len(),
                failed.join("\n")
            );
        }
    }

    /// Every loot spot in a building can be reached: some cell the body
    /// can stand in (furniture's colliders in place, the capsule's
    /// clearance round them) is within a bot's reach of it (`bot::REACH`),
    /// from a door.
    #[test]
    fn every_loot_spot_is_in_reach() {
        const REACH: f32 = crate::bot::REACH;
        let mut far = Vec::new();
        for map in crate::test_maps() {
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let plans = plans(&mut app);
            let spots: Vec<Vec3> = app
                .world()
                .resource::<crate::world::buildings::BuildingLoot>()
                .0
                .iter()
                .map(|s| s.at)
                .collect();
            if std::env::var_os("REACH_DUMP").is_some() {
                dump_plans(
                    &plans,
                    &spots,
                    &crate::asset_root_path().join("../target/reach").join(&map),
                );
            }
            for at in spots {
                let Some(plan) = plans.iter().find(|p| p.contains(at, 0.0)) else {
                    continue;
                };
                // A boxcar's floor is a climb up from the ground, which the
                // indoor grid doesn't walk.
                if plan.name == "boxcar" || behind_the_works(plan, at) {
                    continue;
                }
                let local = plan.to_local(at);
                let goal = local.xz();
                // Flood the open cells from the entries (geometry only: the
                // raider's own rules about which cells count are its own).
                let g = &plan.grid;
                let n = g.n;
                let mut seen = vec![false; n * n];
                let mut stack: Vec<(usize, usize)> = plan
                    .entries
                    .iter()
                    .filter_map(|e| g.nearest_walkable(e.to_array()))
                    .filter_map(|c| g.cell_of(c))
                    .collect();
                for &(i, j) in &stack {
                    seen[j * n + i] = true;
                }
                let mut best: Option<f32> = None;
                while let Some((i, j)) = stack.pop() {
                    let d = Vec2::from_array(g.centre(i, j)).distance(goal);
                    best = Some(best.map_or(d, |b| b.min(d)));
                    for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                        let (ni, nj) = (i as i64 + di, j as i64 + dj);
                        if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                            continue;
                        }
                        let k = nj as usize * n + ni as usize;
                        if !seen[k] && g.cost[k] != BLOCKED {
                            seen[k] = true;
                            stack.push((ni as usize, nj as usize));
                        }
                    }
                }
                match best {
                    Some(d) if d <= REACH => {}
                    _ => far.push(format!(
                        "{map} {} at {:.0?}: spot {:.2?} nearest standing {best:.2?}",
                        plan.name, plan.xf.translation, local
                    )),
                }
            }
            assert!(
                far.is_empty(),
                "{} spots out of reach:\n{}",
                far.len(),
                far.join("\n")
            );
        }
    }
}
