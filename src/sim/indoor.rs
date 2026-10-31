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

use crate::world::buildings::{Building, DoorLeaf, Geo, Shape, prefab};

/// Cell size of the indoor grids, metres.
pub const CELL: f32 = 0.25;
/// Grid extends this far past the footprint, to reach the doorsteps.
const MARGIN: f32 = 3.0;
/// The whole capsule's radius (`player`'s 0.28), as down a site: with
/// 0.22 a raider was routed through gaps between furniture it sticks in.
/// Eight raids a valley, seed 100: stucks 14 -> 7 (Cinder), 49 -> 11
/// (Kettle), 18 -> 11 (Marrow); loot 748 -> 803; extractions unchanged.
const CLEARANCE: f32 = 0.28;
/// Down an underground site, the whole capsule's radius (`player`'s 0.28):
/// a gap between props narrower than a body is no way, or a raider routed
/// through it sticks there.
pub const SITE_CLEARANCE: f32 = 0.28;
/// The body's slack round its capsule (the move's skin, a hand's sway):
/// a way to loot must be this much wider than the capsule as well, or a
/// raider routed to the very edge of it pins there (`tight_plans`).
pub const CAPSULE_SKIN: f32 = 0.06;
/// Geometry in this band above the floor blocks walking (below it you step
/// up; above it you duck under — well, you'd hit your head, but nothing
/// that low is above floor level indoors).
const BAND: (f32, f32) = (0.42, 1.75);
/// A wall this thin whose top stands this far over the ground it stands
/// on is a sill nobody climbs from outside, whatever the floor inside.
const SILL_THIN: f32 = 0.5;
const SILL_STEP: f32 = 0.45;
/// How far an open door has swung, radians (`swing_doors`: 1.75 per unit).
const OPEN_SWING: f32 = 1.75;
/// Loot higher than this above the floor is out of reach (upper floors).
pub const MAX_REACH_UP: f32 = 2.1;
/// The most one cell's floor may stand over its neighbour's and still be
/// walked between (a stair's riser is 0.2): under the body's own step
/// (`player::STEP_HEIGHT`, 0.4), or a route goes up a silt bank's flank
/// where it stands 0.42 over the ground and the body can't.
const STEP_UP: f32 = 0.35;
/// A door's steps reaching out at least this far (m) are a flight, its
/// flanks kept off (`plan_buildings`); slack either side of the doorway
/// in its lane.
const FLIGHT_MIN: f32 = 0.5;
const FLIGHT_LANE: f32 = 0.15;

#[derive(Clone, Debug)]
pub struct DoorInfo {
    pub entity: Entity,
    /// The doorway as a segment across the gap, building-local (x, z).
    pub a: Vec2,
    pub b: Vec2,
    /// Where to stand outside it, building-local, if it's an outside door.
    pub outside: Option<Vec2>,
}

impl DoorInfo {
    /// The doorway shut, as an obstacle across its gap.
    pub fn leaf(&self) -> Obstacle {
        let along = self.b - self.a;
        let dir = along.normalize_or(Vec2::X);
        Obstacle::Rect {
            at: ((self.a + self.b) * 0.5).to_array(),
            half: [along.length() * 0.5 + 0.1, 0.3],
            yaw: dir.y.atan2(dir.x),
        }
    }
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
    /// An underground site's cells shut only by heat nothing crosses:
    /// (cell, heat zone, its cost once that zone's vented).
    pub hot: Vec<(usize, u16, u8)>,
    /// Its cells in heat that hurts but can be crossed: open in the plan;
    /// the raider shuts them like `hot` (`shut_live`), since a slow burn
    /// over a long search kills as surely (Cinder's burner gallery).
    pub warm: Vec<(usize, u16, u8)>,
    /// An underground site's live room's wet cells: open in the plan, as
    /// they are once isolated; the raider shuts them (`shut_live`), since
    /// it can't tell live from cut and a wrong guess kills.
    pub live: Vec<usize>,
    /// An underground site's gas pockets' cells: open in the plan (gas
    /// doesn't stop feet); the raider shuts them (`shut_live`), since it
    /// carries no canary and can't tell a pocket that's gone up.
    pub gas: Vec<usize>,
    /// Its doors held shut on an underground site's power right now (the
    /// raider keeps it up to date): no way through but by its switches.
    pub held: Vec<Entity>,
}

impl BuildingPlan {
    /// The live room's water, the gas and the warm heat shut, for a
    /// raider that won't wade, breathe or stand in them; the heat opens
    /// again when vented, as `hot` does.
    pub fn shut_live(&mut self) {
        for &k in self.live.iter().chain(&self.gas) {
            self.grid.cost[k] = extraction_core::nav::BLOCKED;
        }
        for &(k, ..) in &self.warm {
            self.grid.cost[k] = extraction_core::nav::BLOCKED;
        }
        self.hot.append(&mut self.warm);
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

    /// Standing in (or by) heat its plan shuts, caught in it in a fight
    /// or as it rose: those zones opened again, since a plan can't start
    /// in a shut cell and the nearest open one may be through a wall.
    /// Whether any were.
    pub fn open_heat_at(&mut self, local: Vec2) -> bool {
        let Some((i, j)) = self.grid.cell_of(local.to_array()) else {
            return false;
        };
        let n = self.grid.n as i64;
        let near = |k: usize| {
            let (a, b) = ((k as i64 % n) - i as i64, (k as i64 / n) - j as i64);
            a.abs() <= 1 && b.abs() <= 1
        };
        let mut zones: Vec<usize> = self
            .hot
            .iter()
            .filter(|h| near(h.0))
            .map(|h| h.1 as usize)
            .collect();
        zones.dedup();
        if zones.is_empty() {
            return false;
        }
        self.vent(&zones);
        true
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

    /// Whether a building-local point is within the walls. (An
    /// underground site's bounds can take in a kiosk, the adit's its
    /// entrance between the drift and the haulage: the ground round a
    /// kiosk isn't in, as for [`Self::contains`]; it was, and a raider's
    /// way out by that kiosk found nowhere outside it to stand.)
    pub fn indoors(&self, p: Vec2) -> bool {
        let boxed = self
            .inner
            .is_some_and(|(lo, hi)| p.cmpge(lo - 0.4).all() && p.cmple(hi + 0.4).all());
        boxed
            && (self.levels.is_none()
                || !self
                    .grid
                    .cell_of(p.to_array())
                    .is_some_and(|(i, j)| self.outside[j * self.grid.n + i]))
    }

    /// An indoor path between two building-local points, as local
    /// waypoints (both ends snapped to walkable cells).
    pub fn path(&self, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
        if self.held.is_empty() {
            return self.path_through_held(from, to);
        }
        let mut grid = self.grid.clone();
        for d in self.doors.iter().filter(|d| self.held.contains(&d.entity)) {
            grid.block(d.leaf(), 0.0);
        }
        grid.path(from.to_array(), to.to_array())
            .map(|p| p.into_iter().map(Vec2::from_array).collect())
    }

    /// As [`Self::path`], through doors held shut on power too: for a
    /// route that throws their switches on the way.
    pub fn path_through_held(&self, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
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
    plan_buildings_with(buildings, doors, SITE_CLEARANCE)
}

/// The same, the underground sites' furniture given `site_clearance`
/// round it (the tests' tighter plans, `tight_plans`).
pub(crate) fn plan_buildings_with(
    buildings: &Query<(Entity, &Building, &Transform)>,
    doors: &Query<(Entity, &DoorLeaf, &ChildOf)>,
    site_clearance: f32,
) -> Vec<BuildingPlan> {
    let mut out = Vec::new();
    // Staged scenes (`world::scenes`): furniture of their own, wherever
    // they're left (one in a site is in its way too).
    let scenes: Vec<(Transform, Geo)> = buildings
        .iter()
        .filter(|(_, b, _)| b.prefab.starts_with(crate::world::scenes::PREFIX))
        .filter_map(|(_, b, xf)| Some((*xf, prefab(&b.prefab)?)))
        .collect();
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
            // The scenes left down here, in site space.
            let to_site = xf.compute_affine().inverse();
            let staged: Vec<(Transform, &Geo)> = scenes
                .iter()
                .map(|(sxf, g)| {
                    (
                        Transform::from_matrix(Mat4::from(to_site * sxf.compute_affine())),
                        g,
                    )
                })
                .filter(|(at, _)| {
                    let p = at.translation;
                    r.floor_at([p.x, p.z])
                        .is_some_and(|f| (p.y - f).abs() < 1.0)
                })
                .collect();
            out.push(plan_site(e, b, xf, &geo, r, &mine, &staged, site_clearance));
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
            // The sill rule is a drowned house's (its silt a floor over
            // the ground), not every building that says how high its floor
            // is: setting a ruin's floor made its low walls a climb, and
            // Cinder's damaged houses lost every way in (Models, 31 October).
            let drowned = b.prefab.ends_with("_drowned");
            if let Some(ob) = obstacle(piece.shape, piece.at, under, drowned) {
                grid.block(ob, CLEARANCE);
            }
        }
        // A door's flight of steps (`buildings::door_steps`, each step a
        // tread wider either side than the one above) is climbed straight
        // on, in the door's lane. Along its flanks a route hugging the wall
        // takes each step from its side with the wall at its shoulder, and
        // the step up meets the wall and fails (Marrow sims, 30 October:
        // raiders stood at a house's door steps a metre short of the
        // doorway). The flanks are kept off; the lane stays open.
        if !b.prefab.ends_with("_drowned") {
            for en in &geo.entrances {
                let steps = en.clear - 1.0;
                if steps < FLIGHT_MIN {
                    continue;
                }
                let (at, out) = (Vec2::from(en.at), Vec2::from(en.out));
                let along = out.perp();
                let lane = en.width * 0.5 + FLIGHT_LANE;
                for j in 0..n {
                    for i in 0..n {
                        let l = Vec2::from(grid.centre(i, j)) - at;
                        let (o, s) = (l.dot(out), l.dot(along).abs());
                        if o > 0.0 && o < steps && s > lane && s < en.width * 0.5 + steps + 0.3 {
                            grid.cost[j * n + i] = BLOCKED;
                        }
                    }
                }
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
            warm: Vec::new(),
            live: Vec::new(),
            gas: Vec::new(),
            held: Vec::new(),
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
    geo: &Geo,
    r: &'static extraction_core::underground::Raster,
    mine: &[(Entity, &DoorLeaf)],
    staged: &[(Transform, &Geo)],
    clearance: f32,
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
        if let Some(ob) = obstacle(piece.shape, piece.at, floor, false) {
            grid.block(ob, clearance);
        }
    }
    // Its carts on their rails (`world::workings`): bodies of their own,
    // not the site's pieces, so the raster hasn't them; a raider walking
    // the deep kettle's haulage down its rail met one (Kettle sims, 31
    // October). Where they're set down: nobody the plan's for pushes them.
    for (at, half, yaw) in crate::world::workings::cart_footprints(&b.prefab) {
        grid.block(
            Obstacle::Rect {
                at: at.to_array(),
                half: half.to_array(),
                yaw: -yaw,
            },
            clearance,
        );
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
            clearance,
        );
    }
    // A scene staged down here: its crate and chair are in the way like
    // the site's own (Cinder's "Valve three" pinned raiders in the valve
    // chamber reaching for what's on it).
    for (at, g) in staged {
        for piece in g.pieces.iter().filter(|p| p.collide) {
            let at = *at * piece.at;
            let c = at.translation;
            let Some((i, j)) = grid.cell_of([c.x, c.z]) else {
                continue;
            };
            let floor = levels[j * n + i];
            let floor = if floor.is_nan() { r.floor } else { floor };
            if let Some(ob) = obstacle(piece.shape, at, floor, false) {
                grid.block(ob, clearance);
            }
        }
    }
    // Heat nothing crosses: shut, each cell's cost kept for when its zone
    // is vented (`BuildingPlan::vent`).
    let (mut hot, mut warm) = (Vec::new(), Vec::new());
    if let Some(l) = extraction_core::underground::layout(&b.prefab) {
        for j in 0..n {
            for i in 0..n {
                let k = j * n + i;
                let Some(z) = r.heat_at(grid.centre(i, j)) else {
                    continue;
                };
                let level = l.heat[z as usize].level;
                if level <= 0.0 || grid.cost[k] == extraction_core::nav::BLOCKED {
                    continue;
                }
                if level < extraction_core::underground::SCORCHING {
                    warm.push((k, z, grid.cost[k]));
                } else {
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
    // A pocket (cells a body stands in that no way in reaches: between a
    // prop and a wall, say) is no place: shut, or a raider making for the
    // nearest standing to its loot picks one it can't get to, and sticks.
    // (Heat and live water count as ways through, being shut for now;
    // what's behind the works stays as it is.)
    let through: Vec<usize> = hot
        .iter()
        .chain(&warm)
        .map(|h| h.0)
        .chain(live.iter().copied())
        .collect();
    shut_pockets(&mut grid, &entries, &through, |c| site_closed(&b.prefab, c));
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
        warm,
        live,
        gas,
        held: Vec::new(),
    }
}

/// Whether a site-space point is behind its works (a room the stair
/// doesn't reach till something's done: past a pit, heat, the cage; or a
/// corridor between two such).
pub fn site_closed(prefab: &str, c: [f32; 2]) -> bool {
    use extraction_core::underground::{self as ug, Cell};
    let (Some(l), Some(r)) = (ug::layout(prefab), ug::raster(prefab)) else {
        return false;
    };
    let open = l.open_rooms();
    let room = |id: &str| l.rooms.iter().position(|q| q.id == id);
    match r.at(c) {
        Cell::Room(k) => !open[k as usize],
        Cell::Hall(li) => l.links.get(li as usize).is_some_and(|k| {
            [&k.from, &k.to]
                .iter()
                .all(|id| room(id).is_some_and(|q| !open[q]))
        }),
        _ => false,
    }
}

/// Shut the cells a body could stand in that no way in reaches (`through`
/// cells count as passable, though shut), bar those `closed` says are
/// behind the works.
fn shut_pockets(
    grid: &mut NavGrid,
    entries: &[Vec2],
    through: &[usize],
    closed: impl Fn([f32; 2]) -> bool,
) {
    let n = grid.n;
    let passable = |grid: &NavGrid, k: usize| grid.cost[k] != BLOCKED || through.contains(&k);
    let mut seen = vec![false; n * n];
    let mut stack: Vec<(usize, usize)> = entries
        .iter()
        .filter_map(|e| grid.nearest_walkable(e.to_array()))
        .filter_map(|c| grid.cell_of(c))
        .collect();
    for &(i, j) in &stack {
        seen[j * n + i] = true;
    }
    while let Some((i, j)) = stack.pop() {
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (a, c) = (i as i64 + di, j as i64 + dj);
            if a < 0 || c < 0 || a >= n as i64 || c >= n as i64 {
                continue;
            }
            let k = c as usize * n + a as usize;
            if !seen[k] && passable(grid, k) {
                seen[k] = true;
                stack.push((a as usize, c as usize));
            }
        }
    }
    for k in 0..n * n {
        if !seen[k] && grid.cost[k] != BLOCKED && !closed(grid.centre(k % n, k / n)) {
            grid.cost[k] = BLOCKED;
        }
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

fn obstacle(shape: Shape, at: Transform, floor: f32, sills: bool) -> Option<Obstacle> {
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
    // A floor raised over the ground (`sills`: a drowned house's silt, its
    // `Geo::floor`) puts a window's sill at its knee: from inside a step
    // out, from outside a climb no walker makes. A thin wall standing more
    // than a step over the ground outside blocks, however low it is over
    // the floor.
    let thin = (hi.x - lo.x).min(hi.z - lo.z) < SILL_THIN;
    let sill = sills && thin && hi.y > SILL_STEP && hi.y < floor + BAND.0;
    if !sill && (hi.y < floor + BAND.0 || lo.y > floor + BAND.1) {
        return None;
    }
    let c = at.translation;
    // Round from above only standing up: a wheel or a pipe on its side is
    // its box (below), not a disc its radius round its middle.
    if let Some(r) = circle
        && (at.rotation * Vec3::Y).y.abs() > 0.99
    {
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

    /// Every recipe's plan floor (its lowest door's sill, else its
    /// `Geo::floor`, else the ground: `plan_buildings`) is the height of
    /// what's walked on inside it, the top of its widest flat floor slab
    /// under 1.2 m. A building with no door that left `Geo::floor` unset was
    /// planned on the ground under a raised floor: the blast furnace's cast
    /// house and the mill race's hut (31 October), where no raider found
    /// room for a body by the loot.
    #[test]
    fn every_plan_floor_is_its_floor() {
        use crate::world::buildings::{self, Shape};
        let ids = buildings::PREFABS
            .iter()
            .chain(crate::world::dam::PREFABS.iter())
            .chain(crate::world::fell::PREFABS.iter());
        let mut bad = Vec::new();
        let mut seen = 0;
        for id in ids {
            // Stepped down a slope: each cell has its own floor. Not walked
            // into: a wreck's body, the robots' gate.
            if crate::world::fell::stepped_floor(id, Vec2::ZERO).is_some()
                || id.starts_with("wreck_")
                || *id == "robot_gate"
            {
                continue;
            }
            let Some(geo) = buildings::prefab(id) else {
                continue;
            };
            // No way in (the headframe, the steeple): nothing walked on.
            if geo.entrances.is_empty() && geo.doors.is_empty() {
                continue;
            }
            let plan = geo
                .doors
                .iter()
                .map(|d| d.hinge.y)
                .min_by(|a, b| a.total_cmp(b))
                .or(geo.floor)
                .unwrap_or(0.0);
            let slabs: Vec<(f32, f32)> = geo
                .pieces
                .iter()
                .filter(|p| p.collide)
                .filter_map(|p| {
                    let Shape::Box(s) = p.shape else {
                        return None;
                    };
                    if (p.at.rotation * Vec3::Y).y < 0.999 {
                        return None;
                    }
                    let s = s * p.at.scale;
                    let top = p.at.translation.y + s.y * 0.5;
                    (top > -0.05 && top < 1.2 && s.x >= 1.5 && s.z >= 1.5)
                        .then_some((s.x * s.z, top))
                })
                .collect();
            // The highest of its wide slabs (a drowned house's silt over its
            // footing, a floor on its plinth).
            let widest = slabs.iter().map(|s| s.0).fold(0.0, f32::max);
            let Some(floor) = slabs
                .iter()
                .filter(|s| s.0 >= widest * 0.4)
                .map(|s| s.1)
                .reduce(f32::max)
            else {
                continue;
            };
            seen += 1;
            if (floor - plan).abs() > 0.1 {
                bad.push(format!(
                    "{id}: planned at {plan:.2}, its floor at {floor:.2}"
                ));
            }
        }
        assert!(seen >= 30, "only {seen} recipes with a floor");
        assert!(
            bad.is_empty(),
            "plans off their floors:\n{}",
            bad.join("\n")
        );
    }

    /// A route down a site with a mine cart on its rail goes round the
    /// cart, not through it (Kettle sims, 31 October: raiders walking the
    /// deep kettle's haulage down its rail stood pressed to the cart):
    /// from either side of every cart, along its rail.
    #[test]
    fn a_route_goes_round_a_mine_cart() {
        let opts = LaunchOptions {
            robots: false,
            map: "kettle_pass".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let mut tried = 0;
        let mut through = vec![];
        for plan in plans(&mut app) {
            for (at, half, yaw) in crate::world::workings::cart_footprints(&plan.name) {
                let along = Vec2::new(yaw.sin(), yaw.cos());
                let across = along.perp();
                let inside = |q: Vec2| {
                    let d = q - at;
                    d.dot(across).abs() < half.x && d.dot(along).abs() < half.y
                };
                let (a, b) = (at - along * 3.0, at + along * 3.0);
                let Some(path) = plan.path(a, b) else {
                    continue;
                };
                tried += 1;
                let hit = path
                    .windows(2)
                    .any(|w| (0..=20).any(|k| inside(w[0].lerp(w[1], k as f32 / 20.0))));
                if hit {
                    through.push(format!("{} cart at {at}", plan.name));
                }
            }
        }
        assert!(tried >= 1, "no cart tried");
        assert!(through.is_empty(), "through a cart: {through:?}");
    }

    /// A route in at a door with steps up to it comes onto the flight
    /// in the door's lane, square on, never along its flanks (Marrow sims,
    /// 30 October: raiders walking the wall took a house's steps from
    /// their sides, the wall at their shoulder, and stood a metre short of
    /// the doorway). From along the wall either side, on every map.
    #[test]
    fn a_route_takes_door_steps_square_on() {
        let mut flanked = vec![];
        let mut tried = 0;
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
            for plan in plans(&mut app) {
                if plan.name.ends_with("_drowned") {
                    continue;
                }
                let Some(geo) = prefab(&plan.name) else {
                    continue;
                };
                for en in &geo.entrances {
                    let steps = en.clear - 1.0;
                    if steps < FLIGHT_MIN {
                        continue;
                    }
                    let (at, out) = (Vec2::from(en.at), Vec2::from(en.out));
                    let along = out.perp();
                    let lane = en.width * 0.5 + FLIGHT_LANE + 1e-3;
                    let half = en.width * 0.5 + steps;
                    let inside = at - out * 1.5;
                    for (side, off, from) in [-1.0f32, 1.0].into_iter().flat_map(|s| {
                        [
                            (s, 0.4, half + 0.5),
                            (s, 1.1, half + 0.8),
                            (s, 0.6, half + 1.5),
                        ]
                    }) {
                        let start = at + along * side * from + out * off;
                        let Some(path) = plan.path(start, inside) else {
                            continue;
                        };
                        tried += 1;
                        let on_flank = path.windows(2).any(|w| {
                            (0..=20).any(|k| {
                                let l = w[0].lerp(w[1], k as f32 / 20.0) - at;
                                let (o, s) = (l.dot(out), l.dot(along).abs());
                                o > 0.05 && o < steps && s > lane && s < half
                            })
                        });
                        if on_flank {
                            flanked.push(format!(
                                "{map} {} at {:.1}: from {side} {from:.1} along {off} out",
                                plan.name,
                                plan.xf.translation.xz()
                            ));
                        }
                    }
                }
            }
        }
        assert!(tried >= 20, "tried {tried}");
        assert!(
            flanked.is_empty(),
            "up a door's steps from the side: {flanked:#?}"
        );
    }

    fn plans(app: &mut App) -> Vec<BuildingPlan> {
        app.world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap()
    }

    /// The underground sites' plans with a body's slack round their
    /// furniture too (`CAPSULE_SKIN`): what a way only just wide enough
    /// for the capsule loses.
    fn tight_plans(app: &mut App) -> Vec<BuildingPlan> {
        app.world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| {
                    plan_buildings_with(&b, &d, SITE_CLEARANCE + CAPSULE_SKIN)
                },
            )
            .unwrap()
            .into_iter()
            .filter(|p| extraction_core::underground::layout(&p.name).is_some())
            .collect()
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
    fn a_cylinder_on_its_side_blocks_its_own_box() {
        // Marrow's mill wheel: 2.4 m round, 1.2 m wide, its axle along X.
        let wheel = Transform::from_xyz(-0.5, 2.55, 0.0)
            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2));
        let Some(Obstacle::Rect { at, half, .. }) =
            obstacle(Shape::Cylinder(2.4, 1.2), wheel, 0.0, false)
        else {
            panic!("a wheel on its side should block as a box");
        };
        assert!(
            (half[0] - 0.6).abs() < 0.01 && (half[1] - 2.4).abs() < 0.01,
            "{half:?}"
        );
        assert!((at[0] + 0.5).abs() < 0.01, "{at:?}");
        // Standing up it's a disc.
        let post = Transform::from_xyz(0.0, 1.0, 0.0);
        assert!(matches!(
            obstacle(Shape::Cylinder(0.3, 2.0), post, 0.0, false),
            Some(Obstacle::Circle { .. })
        ));
    }

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
            let tight = tight_plans(&mut app);
            let spots: Vec<Vec3> = app
                .world()
                .get_resource::<crate::world::buildings::BuildingLoot>()
                .map(|l| l.0.iter().map(|s| s.at).collect())
                .unwrap_or_default();
            let mut failed = Vec::new();
            let mut tried = 0;
            for plan in plans.iter().filter(|p| !p.entries.is_empty()) {
                // An underground site's spots are tried too, from its own
                // plan (each on its own side of a gallery's wall).
                let site = extraction_core::underground::layout(&plan.name).is_some();
                let before = tried;
                for spot in spots.iter().filter(|s| plan.contains(**s, 0.5)) {
                    if !plan.reachable_height(*spot) || behind_the_works(plan, *spot) {
                        continue;
                    }
                    tried += 1;
                    let goal = plan.to_local(*spot).xz();
                    // Down a site, reached with a body's slack to spare: a
                    // spot only reached through a gap narrower than the
                    // capsule and its skin pins whoever goes for it (the
                    // gallery's ammo crate, once).
                    if let Some(t) = tight.iter().find(|t| t.entity == plan.entity) {
                        if super::super::raider::reach_cell(t, t.entries[0], goal).is_none() {
                            failed.push(format!(
                                "{map} {}: spot {goal:.2?} only through a gap narrower than a body",
                                plan.name
                            ));
                        }
                    }
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
                if site && tried == before {
                    failed.push(format!("{map} {}: none of its loot spots tried", plan.name));
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
    /// Down every underground site, for a whole body (`SITE_CLEARANCE`):
    /// every cell it can stand in, bar those behind the works (past a pit,
    /// heat or the cage), joins up with the ways out. No pocket a raider
    /// could be routed into or caught in.
    /// A scene staged in an underground site (`world::scenes`) is in its
    /// plan's way like the site's own furniture: nobody's sent to stand
    /// inside its crate for what's on it.
    #[test]
    fn scenes_staged_in_a_site_stand_in_its_plan() {
        let mut checked = 0;
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
            let scenes: Vec<(String, Transform)> = app
                .world_mut()
                .query::<(&Building, &Transform)>()
                .iter(app.world())
                .filter(|(b, _)| b.prefab.starts_with(crate::world::scenes::PREFIX))
                .map(|(b, t)| (b.prefab.clone(), *t))
                .collect();
            for plan in plans(&mut app) {
                let Some(r) = extraction_core::underground::raster(&plan.name) else {
                    continue;
                };
                for (id, sxf) in &scenes {
                    let at = Transform::from_matrix(Mat4::from(
                        plan.xf.compute_affine().inverse() * sxf.compute_affine(),
                    ));
                    let p = at.translation;
                    if !r
                        .floor_at([p.x, p.z])
                        .is_some_and(|f| (p.y - f).abs() < 1.0)
                    {
                        continue;
                    }
                    for piece in prefab(id).unwrap().pieces.iter().filter(|p| p.collide) {
                        let c = (at * piece.at).translation;
                        // What's stepped over (a bedroll) is no one's way.
                        let floor = r.floor_at([c.x, c.z]).unwrap_or(r.floor);
                        if obstacle(piece.shape, at * piece.at, floor, false).is_none() {
                            continue;
                        }
                        let k = plan
                            .grid
                            .cell_of([c.x, c.z])
                            .map(|(i, j)| j * plan.grid.n + i);
                        assert!(
                            k.is_some_and(|k| plan.grid.cost[k] == BLOCKED),
                            "{map} {}: {id}'s piece at {c:.2?} is open in the plan",
                            plan.name
                        );
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "no scene staged in a site");
    }

    /// From every room of every underground site, a raider finds where to
    /// stand out of each of its ways out: the adit's entrance lies inside
    /// its bounds (the haulage runs south past it), and from its deep level
    /// and its magazine `stand_near` found nowhere by that kiosk to stand,
    /// so raiders walked at the rock toward the RV (Rime seeds 2 and 4).
    #[test]
    fn every_site_room_can_stand_out_each_way() {
        let mut bad = Vec::new();
        let mut checked = 0;
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
            for plan in plans(&mut app) {
                let Some(l) = extraction_core::underground::layout(&plan.name) else {
                    continue;
                };
                let open = l.open_rooms();
                for (room, _) in l.rooms.iter().zip(open).filter(|(r, o)| *o && !r.pit) {
                    let from = Vec2::from(room.at);
                    let Some(from) = plan.grid.nearest_walkable(from.to_array()).map(Vec2::from)
                    else {
                        continue;
                    };
                    for &e in &plan.entries {
                        checked += 1;
                        if plan.indoors(e) {
                            bad.push(format!(
                                "{map} {}: way out {e} counts as indoors",
                                plan.name
                            ));
                        }
                        if super::super::raider::stand_near(&plan, from, e).is_none() {
                            bad.push(format!(
                                "{map} {} from {}: nowhere to stand out at {e}",
                                plan.name, room.id
                            ));
                        }
                    }
                }
            }
        }
        assert!(checked > 0);
        bad.dedup();
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn every_site_cell_reaches_a_way_out() {
        let mut pockets = Vec::new();
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
            for plan in plans(&mut app)
                .iter()
                .filter(|p| extraction_core::underground::layout(&p.name).is_some())
            {
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
                while let Some((i, j)) = stack.pop() {
                    for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                        let (ni, nj) = (i as i64 + di, j as i64 + dj);
                        if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                            continue;
                        }
                        let k = nj as usize * n + ni as usize;
                        let through = plan.hot.iter().chain(&plan.warm).any(|h| h.0 == k)
                            || plan.live.contains(&k);
                        if !seen[k] && (g.cost[k] != BLOCKED || through) {
                            seen[k] = true;
                            stack.push((ni as usize, nj as usize));
                        }
                    }
                }
                let cut: Vec<[f32; 2]> = (0..n * n)
                    .filter(|k| g.cost[*k] != BLOCKED && !seen[*k])
                    .map(|k| g.centre(k % n, k / n))
                    .filter(|c| !site_closed(&plan.name, *c))
                    .collect();
                if !cut.is_empty() {
                    pockets.push(format!(
                        "{map} {}: {} cells cut off, e.g. {:.1?}",
                        plan.name,
                        cut.len(),
                        &cut[..cut.len().min(4)]
                    ));
                }
            }
        }
        assert!(pockets.is_empty(), "pockets:\n{}", pockets.join("\n"));
    }

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
