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
}

impl BuildingPlan {
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

    /// Whether a world point is inside (or on) the building's footprint.
    pub fn contains(&self, p: Vec3, pad: f32) -> bool {
        let l = self.to_local(p);
        l.x.abs() <= self.half.x + pad && l.z.abs() <= self.half.y + pad
    }

    /// Whether a world point is within the indoor grid.
    pub fn in_grid(&self, p: Vec3) -> bool {
        let l = self.to_local(p);
        self.grid.cell_of([l.x, l.z]).is_some()
    }

    /// Whether loot at this world point can be reached from the floor.
    pub fn reachable_height(&self, p: Vec3) -> bool {
        self.to_local(p).y - self.floor <= MAX_REACH_UP
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
            .unwrap_or(0.0);
        let extent = half.x.max(half.y) + MARGIN;
        let n = ((extent * 2.0) / CELL).ceil() as usize;
        let mut grid = NavGrid {
            n,
            cell: CELL,
            origin: -extent,
            cost: vec![1; n * n],
        };
        for piece in geo.pieces.iter().filter(|p| p.collide) {
            if let Some(ob) = obstacle(piece.shape, piece.at, floor) {
                grid.block(ob, CLEARANCE);
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
            for side in [1.0, -1.0] {
                let probe = mid + normal * side * 0.9;
                if is_out(probe) {
                    let stand = mid + normal * side * 1.3;
                    d.outside = Some(stand);
                    entries.push(stand);
                    break;
                }
            }
        }
        if infos.is_empty() {
            // An open structure: come in from wherever is open.
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let p = Vec2::new(a.cos() * (half.x + 1.2), a.sin() * (half.y + 1.2));
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
        });
    }
    out
}

/// A collider piece as an obstacle on the floor, if it's in the way.
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
                    let mid = (d.a + d.b) * 0.5;
                    // A metre and a half past the doorway, inside (or as near
                    // as the furniture allows).
                    let Some(goal) = super::super::raider::reach_cell(
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
}
