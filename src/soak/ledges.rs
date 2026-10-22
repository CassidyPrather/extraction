//! Ledge traps: somewhere in the built world a body can fall onto but
//! not walk off, short of a drop.
//!
//! A sim raider fell 7 m off Sallow's face stair onto the dam face's
//! 0.6 m course step and paced it for twenty minutes: the bots' steering
//! never takes a drop, and a player who steps off lands hard. So over
//! every valley's buildings (their static colliders; doors and items don't
//! count) this finds each standable surface (facing up past the player's
//! ground, 0.65, with a body's height clear above it), joins surfaces into
//! areas where neighbours are within a step, and flags an area as a perch
//! when:
//! - it's at least a body's width;
//! - it doesn't reach the ground and no ladder serves it;
//! - every way off it drops at least [`DROP`];
//! - somewhere someone can be (an area on the ground, or a ladder's)
//!   overlooks it from higher up within [`FALL_REACH`]: you can land on
//!   it. A roof under a taller roof nobody stands on isn't one.
//!
//! Past 8 m/s a landing hurts (about 3 m of fall) and past about 18 m it
//! kills (`health::SAFE_LANDING`), so a perch is a soft-lock for a bot and
//! a hard landing for a player. Known walks are listed in [`WALKS`].

use avian3d::prelude::*;
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::world::Layer;
use crate::world::Zone;
use crate::world::buildings::Building;

/// The player's ground: surfaces facing up more than this.
const GROUND_NORMAL: f32 = 0.65;
/// What the player steps up or down without falling (`player::STEP_HEIGHT`).
const STEP: f32 = 0.4;
/// A body's height, clear over a standable surface.
const HEADROOM: f32 = 1.8;
/// A perch's every way off drops at least this, metres.
pub const DROP: f32 = 3.0;
/// A perch is overlooked by something walkable within this, horizontally.
pub const FALL_REACH: f32 = 4.0;
/// The grid the world is sampled on, metres.
const CELL: f32 = 0.5;
/// At least this many cells: room for a body (0.6 m across) to stand.
const MIN_CELLS: usize = 3;

/// Known walks: (map, prefab, why). (The probe's half-metre cells are
/// coarser than some walks: they read a narrow ring as cut off.)
const WALKS: &[(&str, &str, &str)] = &[
    (
        "marrow_creek",
        "water_tower",
        "a 0.45 m ring round the tank, wide enough for the 0.28 m capsule, the ladder at its back",
    ),
    (
        "sallow_reservoir",
        "water_tower",
        "a 0.45 m ring round the tank, wide enough for the 0.28 m capsule, the ladder at its back",
    ),
];

/// Perches found and with their owners: (map, prefab, who, what). Each
/// comes off when it's fixed; a perch anywhere else fails the test.
const OPEN: &[(&str, &str, &str, &str)] = &[(
    "sallow_reservoir",
    "dam_face",
    "Fourth valley",
    "the face's course steps: being weathered at 60 degrees",
)];

/// A standable surface in a column: its height and whether it's the ground.
#[derive(Clone, Copy, Debug)]
struct Surface {
    y: f32,
    ground: bool,
}

/// One perch found.
#[derive(Debug)]
pub struct Perch {
    pub prefab: String,
    pub at: Vec3,
    pub cells: usize,
    pub drop: f32,
}

impl std::fmt::Display for Perch {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} at ({:.1}, {:.1}, {:.1}): {} m² that drops {:.1} m every way off",
            self.prefab,
            self.at.x,
            self.at.y,
            self.at.z,
            self.cells as f32 * CELL * CELL,
            self.drop
        )
    }
}

/// Every standable surface down a column at (x, z), top first, from `top`.
fn column(
    q: &SpatialQuery,
    zone: &Zone,
    x: f32,
    z: f32,
    top: f32,
    statics: &impl Fn(Entity) -> bool,
) -> Vec<Surface> {
    let filter = SpatialQueryFilter::from_mask(Layer::World);
    let ground = zone.ground(x, z);
    let mut out = Vec::new();
    let mut y = top;
    for _ in 0..40 {
        let from = Vec3::new(x, y, z);
        let Some(hit) = q.cast_ray(from, Dir3::NEG_Y, y - (ground - 1.0), false, &filter) else {
            break;
        };
        let at = y - hit.distance;
        if statics(hit.entity) && hit.normal.y > GROUND_NORMAL {
            let up = Vec3::new(x, at + 0.05, z);
            let clear = q
                .cast_ray(up, Dir3::Y, HEADROOM, true, &filter)
                .is_none_or(|h| !statics(h.entity));
            if clear {
                out.push(Surface {
                    y: at,
                    ground: (at - ground).abs() < 0.2,
                });
            }
        }
        y = at - 0.02;
        if y < ground - 1.0 {
            break;
        }
    }
    out
}

/// The perches around one map's buildings.
pub fn perches(app: &mut App) -> Vec<Perch> {
    let map = app.world().resource::<crate::LaunchOptions>().map.clone();
    let zone = app.world().resource::<Zone>().clone();
    let buildings: Vec<(String, ColliderAabb)> = app
        .world_mut()
        .query::<(&Building, &ColliderAabb)>()
        .iter(app.world())
        .map(|(b, a)| (b.prefab.clone(), *a))
        .collect();
    let ladders: Vec<(Vec3, Vec3)> = app
        .world_mut()
        .query::<&crate::interact::hand_climb::Ladder>()
        .iter(app.world())
        .map(|l| (l.bottom, l.top))
        .collect();
    let mut out = Vec::new();
    for (prefab, aabb) in buildings {
        if WALKS.iter().any(|(m, p, _)| *m == map && *p == prefab) {
            continue;
        }
        let (min, max) = (
            aabb.min - Vec3::new(3.0, 0.0, 3.0),
            aabb.max + Vec3::new(3.0, 0.0, 3.0),
        );
        let nx = ((max.x - min.x) / CELL).ceil() as i32;
        let nz = ((max.z - min.z) / CELL).ceil() as i32;
        if nx * nz > 400_000 {
            continue;
        }
        let top = max.y + 2.0;
        let zone2 = zone.clone();
        let cells: HashMap<(i32, i32), Vec<Surface>> = app
            .world_mut()
            .run_system_once(
                move |q: SpatialQuery, bodies: Query<&RigidBody>, of: Query<&ColliderOf>| {
                    let statics = |e: Entity| {
                        of.get(e)
                            .ok()
                            .and_then(|c| bodies.get(c.body).ok())
                            .or_else(|| bodies.get(e).ok())
                            .is_some_and(|b| *b == RigidBody::Static)
                    };
                    let mut cells = HashMap::new();
                    for i in 0..nx {
                        for k in 0..nz {
                            let x = min.x + (i as f32 + 0.5) * CELL;
                            let z = min.z + (k as f32 + 0.5) * CELL;
                            cells.insert((i, k), column(&q, &zone2, x, z, top, &statics));
                        }
                    }
                    cells
                },
            )
            .unwrap();
        // Join surfaces into areas: neighbours within a step.
        let mut ids: HashMap<(i32, i32, usize), usize> = HashMap::new();
        let mut parent: Vec<usize> = Vec::new();
        for (&(i, k), ss) in &cells {
            for s in 0..ss.len() {
                ids.insert((i, k, s), parent.len());
                parent.push(parent.len());
            }
        }
        fn find(p: &mut Vec<usize>, a: usize) -> usize {
            let mut a = a;
            while p[a] != a {
                p[a] = p[p[a]];
                a = p[a];
            }
            a
        }
        for (&(i, k), ss) in &cells {
            for (di, dk) in [(1, 0), (0, 1)] {
                let Some(nn) = cells.get(&(i + di, k + dk)) else {
                    continue;
                };
                for (a, sa) in ss.iter().enumerate() {
                    for (b, sb) in nn.iter().enumerate() {
                        if (sa.y - sb.y).abs() <= STEP {
                            let (x, y) = (ids[&(i, k, a)], ids[&(i + di, k + dk, b)]);
                            let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
                            parent[rx] = ry;
                        }
                    }
                }
            }
        }
        let mut areas: HashMap<usize, Vec<(i32, i32, usize)>> = HashMap::new();
        for (&key, &id) in &ids {
            let r = find(&mut parent, id);
            areas.entry(r).or_default().push(key);
        }
        let world = |i: i32, k: i32, y: f32| {
            Vec3::new(
                min.x + (i as f32 + 0.5) * CELL,
                y,
                min.z + (k as f32 + 0.5) * CELL,
            )
        };
        // Where someone can be: an area on the ground or a ladder's ends.
        let near_ladder = |p: Vec3| {
            ladders
                .iter()
                .any(|(b, t)| p.distance(*t) < 1.5 || p.distance(*b) < 1.5)
        };
        let mut reachable: std::collections::HashSet<(i32, i32, usize)> =
            std::collections::HashSet::new();
        for members in areas.values() {
            let on = members.iter().any(|&(i, k, s)| {
                let c = cells[&(i, k)][s];
                c.ground || near_ladder(world(i, k, c.y))
            });
            if on {
                reachable.extend(members.iter().copied());
            }
        }
        for members in areas.values() {
            if members.len() < MIN_CELLS {
                continue;
            }
            let surf = |&(i, k, s): &(i32, i32, usize)| cells[&(i, k)][s];
            if members.iter().any(|m| surf(m).ground) {
                continue;
            }
            // On the sampled area's rim: it may go on beyond; not judged.
            if members
                .iter()
                .any(|&(i, k, _)| i == 0 || k == 0 || i == nx - 1 || k == nz - 1)
            {
                continue;
            }
            let top_y = members.iter().map(|m| surf(m).y).fold(f32::MIN, f32::max);
            let pts: Vec<Vec3> = members.iter().map(|m| world(m.0, m.1, surf(m).y)).collect();
            if pts.iter().any(|p| near_ladder(*p)) {
                continue;
            }
            // Every way off: the drop to whatever's under the next cell.
            let mut least = f32::MAX;
            for &(i, k, s) in members {
                let y = cells[&(i, k)][s].y;
                for (di, dk) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let n = (i + di, k + dk);
                    let joined = cells
                        .get(&n)
                        .is_some_and(|nn| nn.iter().any(|o| (o.y - y).abs() <= STEP));
                    if joined {
                        continue;
                    }
                    let landing = cells
                        .get(&n)
                        .and_then(|nn| nn.iter().map(|o| o.y).filter(|oy| *oy < y).reduce(f32::max))
                        .unwrap_or_else(|| {
                            let p = world(n.0, n.1, 0.0);
                            zone.ground(p.x, p.z)
                        });
                    least = least.min(y - landing);
                }
            }
            if least < DROP {
                continue;
            }
            // Overlooked: a walkable surface higher up within reach.
            let reach = (FALL_REACH / CELL).ceil() as i32;
            let overlooked = members.iter().any(|&(i, k, _)| {
                (-reach..=reach).any(|di| {
                    (-reach..=reach).any(|dk| {
                        cells.get(&(i + di, k + dk)).is_some_and(|nn| {
                            nn.iter().enumerate().any(|(s, o)| {
                                o.y > top_y + STEP && reachable.contains(&(i + di, k + dk, s))
                            })
                        })
                    })
                })
            });
            if !overlooked {
                continue;
            }
            let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
            out.push(Perch {
                prefab: prefab.clone(),
                at: centre,
                cells: members.len(),
                drop: least,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(map: &str) -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            map: map.into(),
            map_explicit: true,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        app
    }

    /// No valley's buildings hold a perch: somewhere you can fall onto
    /// but not walk off of, short of a drop of 3 m or more.
    #[test]
    fn no_valley_has_a_ledge_trap() {
        let mut found = Vec::new();
        for map in &crate::test_maps() {
            let mut app = game(map);
            for p in perches(&mut app) {
                if OPEN.iter().any(|(m, pf, ..)| m == map && *pf == p.prefab) {
                    continue;
                }
                found.push(format!("{map}: {p}"));
            }
        }
        assert!(
            found.is_empty(),
            "ledge traps (perches with every way off a {DROP} m drop or more):\n{}",
            found.join("\n")
        );
    }
}
