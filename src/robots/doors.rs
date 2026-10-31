//! Robots and doors. Buildings aren't a refuge: a robot hunting someone
//! indoors goes to the building's doors. A drone hovers outside one that
//! stands open and looks in; a walker opens a closed one, and batters
//! down one that's been wedged shut.
//!
//! The door wedge: set it down against the foot of a closed door and the
//! door won't open — for you either — until someone picks the wedge up,
//! or a walker tears the door off.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::body::InBackpack;
use crate::interact::{Held, InSocket};
use crate::items::{Catalog, Item};
use crate::world::buildings::{Building, Door, DoorLeaf};

/// A doorway: the door, its building, the middle of the opening (world,
/// about chest height) and the way out (unit, horizontal).
#[derive(Clone, Copy, Debug)]
pub struct Entrance {
    pub door: Entity,
    pub building: Entity,
    pub mid: Vec3,
    pub out: Vec3,
}

/// A building's box, for "is this point indoors?".
#[derive(Clone, Copy, Debug)]
pub struct Shell {
    pub building: Entity,
    pub xf: Transform,
    pub half: Vec2,
    pub height: f32,
    /// Where it has a roof (`buildings::roof_cover`): indoors is under it.
    /// None: the whole box, as high as its footprint.
    pub cover: Option<&'static crate::world::buildings::RoofCover>,
}

impl Shell {
    /// The shell of `building`, a `prefab` placed at `xf`.
    pub fn of(building: Entity, prefab: &str, xf: Transform) -> Self {
        let size = extraction_core::map::prefab_footprint(prefab);
        // An underground site's only walls up top are its entrance kiosk's.
        let half = if extraction_core::underground::is_site(prefab) {
            Vec2::from(extraction_core::underground::SHELL_HALF)
        } else {
            Vec2::new(size[0], size[2]) * 0.5
        };
        Self {
            building,
            xf,
            half,
            height: size[1],
            cover: crate::world::buildings::roof_cover(prefab),
        }
    }

    /// Whether a world point is inside, under its roof.
    pub fn contains(&self, p: Vec3) -> bool {
        let l = self.xf.compute_affine().inverse().transform_point3(p);
        match self.cover {
            Some(c) => c.covers(l),
            None => {
                l.x.abs() < self.half.x
                    && l.z.abs() < self.half.y
                    && l.y > -1.0
                    && l.y < self.height - 0.6
            }
        }
    }
}

/// Every building that has doors, and its doors.
#[derive(Resource, Default, Debug)]
pub struct Entrances {
    pub doors: Vec<Entrance>,
    pub shells: Vec<Shell>,
}

impl Entrances {
    /// The highest top of any building over (x, z), `pad` round its
    /// footprint (world y), if one's there: what a flyer's height is over.
    pub fn top_at(&self, x: f32, z: f32, pad: f32) -> Option<f32> {
        self.shells
            .iter()
            .filter(|s| {
                let l = s.xf.compute_affine().inverse().transform_point3(Vec3::new(
                    x,
                    s.xf.translation.y,
                    z,
                ));
                l.x.abs() < s.half.x + pad && l.z.abs() < s.half.y + pad
            })
            .map(|s| s.xf.translation.y + s.height)
            .reduce(f32::max)
    }

    /// The building `p` is inside (under its roof), if any.
    pub fn building_at(&self, p: Vec3) -> Option<Entity> {
        self.shells
            .iter()
            .find_map(|s| s.contains(p).then_some(s.building))
    }

    /// Where a robot outside should stand to deal with `building`: the
    /// doorway nearest `from`, preferring one that's already open (no
    /// door at all counts as open).
    pub fn approach(
        &self,
        building: Entity,
        from: Vec3,
        is_open: impl Fn(Entity) -> bool,
    ) -> Option<(Entrance, bool)> {
        self.doors
            .iter()
            .filter(|e| e.building == building)
            .map(|e| (*e, is_open(e.door)))
            .min_by(|(a, ao), (b, bo)| {
                // Open first, then nearest.
                bo.cmp(ao).then(
                    a.mid
                        .distance_squared(from)
                        .total_cmp(&b.mid.distance_squared(from)),
                )
            })
    }
}

/// Open enough to see (and shoot) through. A door that's gone is open.
pub fn is_open(door: Option<&Door>) -> bool {
    door.is_none_or(|d| d.angle > 0.4)
}

/// A door jammed shut by a wedge (the wedge item).
#[derive(Component, Debug, Clone, Copy)]
pub struct Wedged(pub Entity);

/// A wedge holding a door.
#[derive(Component, Debug, Clone, Copy)]
pub struct Wedging(pub Entity);

/// A walker hammering at a wedged door: seconds so far.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Battering(pub f32);

/// A walker's blow on a wedged door (for sound); `broke` on the last.
#[derive(Message, Clone, Copy, Debug)]
pub struct DoorBlow {
    pub at: Vec3,
    pub broke: bool,
}

/// How long a walker takes to tear a wedged door off.
pub const BATTER_TIME: f32 = 12.0;

pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Entrances>()
            .add_message::<DoorBlow>()
            .add_systems(
                FixedUpdate,
                (index_entrances, wedges).after(crate::interact::InteractSystems::Follow),
            );
    }
}

/// Once the buildings exist, note every doorway and building box: a box
/// for every building with a way in, a door or a bare doorway (a burnt
/// house's, a drowned one's), indoors under its roof.
fn index_entrances(
    mut entrances: ResMut<Entrances>,
    buildings: Query<(Entity, &Building, &Transform)>,
    doors: Query<(Entity, &DoorLeaf, &ChildOf)>,
) {
    if !entrances.shells.is_empty() || doors.is_empty() {
        return;
    }
    for (b, building, xf) in &buildings {
        let mine: Vec<_> = doors.iter().filter(|(_, _, p)| p.parent() == b).collect();
        // A way in: a door, or a doorway with none (`map::prefab_entrances`)
        // into somewhere with a roof (not a drowned house's walls).
        let roofed =
            crate::world::buildings::roof_cover(&building.prefab).is_some_and(|c| c.roofed().0 > 0);
        let way_in = !mine.is_empty()
            || (roofed && !extraction_core::map::prefab_entrances(&building.prefab).is_empty());
        if !way_in {
            continue;
        }
        entrances.shells.push(Shell::of(b, &building.prefab, *xf));
        for (door, leaf, _) in mine {
            let d = leaf.0;
            let rot = Quat::from_rotation_y(d.yaw);
            let mid = d.hinge + rot * Vec3::new(d.width * 0.5, 1.2, 0.0);
            let n = rot * Vec3::Z;
            let n = if Vec2::new(mid.x, mid.z).dot(Vec2::new(n.x, n.z)) >= 0.0 {
                n
            } else {
                -n
            };
            entrances.doors.push(Entrance {
                door,
                building: b,
                mid: xf.transform_point(mid),
                out: (xf.rotation * n).with_y(0.0).normalize_or(Vec3::Z),
            });
        }
    }
    info!(
        "{} doorways in {} buildings",
        entrances.doors.len(),
        entrances.shells.len()
    );
}

/// A wedge set down against the foot of a closed door jams it; picked up,
/// it lets go.
#[allow(clippy::type_complexity)]
fn wedges(
    mut commands: Commands,
    items: Query<(
        Entity,
        &Item,
        &GlobalTransform,
        &LinearVelocity,
        Option<&Wedging>,
        Has<Held>,
        Has<InSocket>,
        Has<InBackpack>,
    )>,
    doors: Query<(Entity, &Door, &DoorLeaf, &GlobalTransform, Has<Wedged>)>,
    catalog: Res<Catalog>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for (e, item, gt, vel, wedging, held, socketed, stowed) in &items {
        if catalog.get(item.kind()).id != "door_wedge" {
            continue;
        }
        if let Some(w) = wedging {
            if held || socketed || stowed || !doors.contains(w.0) {
                commands.entity(e).remove::<(Wedging, RigidBodyDisabled)>();
                if let Ok(mut d) = commands.get_entity(w.0) {
                    d.remove::<Wedged>();
                }
            }
            continue;
        }
        if held || socketed || stowed || vel.0.length() > 0.5 {
            continue;
        }
        let p = gt.translation();
        // Against the bottom edge of a closed leaf, hinge to latch.
        let door = doors.iter().find_map(|(d, door, leaf, dgt, wedged)| {
            if door.open || door.angle > 0.05 || wedged {
                return None;
            }
            let a = dgt.translation();
            let b = dgt.transform_point(Vec3::X * leaf.0.width);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            let q = a + ab * t;
            (Vec2::new(p.x - q.x, p.z - q.z).length() < 0.4 && (p.y - q.y).abs() < 0.4).then_some(d)
        });
        let Some(door) = door else {
            continue;
        };
        commands.entity(door).insert(Wedged(e));
        commands
            .entity(e)
            .insert((Wedging(door), RigidBodyDisabled));
        raid.say("The wedge bites under the door. Nothing's opening that now.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever doors it has, a point under a prefab's roof is indoors
    /// and one under the sky isn't (a burnt house under the half of its
    /// roof that's left, not its open back; a damaged one not under its
    /// hole; a drowned one nowhere). Prints which have a box now that had
    /// none (`-- --nocapture`).
    #[test]
    fn indoors_is_under_the_roof_whatever_the_doors() {
        use crate::world::buildings::{self, Shape};
        // The truth, the plain way: a ray straight up from the point
        // meets a piece.
        fn roofed(geo: &buildings::Geo, p: Vec3) -> bool {
            geo.pieces.iter().any(|piece| {
                let half = match piece.shape {
                    Shape::Box(s) => s * 0.5,
                    Shape::Prism(s) => Vec3::new(s.x * 0.5, s.y * 0.25, s.z * 0.5),
                    Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
                };
                if half.min_element() <= 0.005 || piece.mat == crate::render::materials::Mat::Glass
                {
                    return false;
                }
                let at = match piece.shape {
                    Shape::Prism(s) => piece.at * Transform::from_xyz(0.0, s.y * 0.25, 0.0),
                    _ => piece.at,
                };
                let inv = at.rotation.inverse();
                let o = inv * (p - at.translation);
                let d = inv * Vec3::Y;
                let (mut t0, mut t1) = (0.0f32, 100.0f32);
                for k in 0..3 {
                    if d[k].abs() < 1e-6 {
                        if o[k].abs() > half[k] {
                            return false;
                        }
                        continue;
                    }
                    let (a, b) = ((-half[k] - o[k]) / d[k], (half[k] - o[k]) / d[k]);
                    t0 = t0.max(a.min(b));
                    t1 = t1.min(a.max(b));
                }
                t0 <= t1 && t0 > 0.0
            })
        }
        let mut bad = Vec::new();
        let mut gained = Vec::new();
        let mut changed = Vec::new();
        let mut checked = 0;
        for id in buildings::PREFABS {
            let Some(geo) = buildings::prefab(id) else {
                continue;
            };
            let way_in =
                !geo.doors.is_empty() || !extraction_core::map::prefab_entrances(id).is_empty();
            if !way_in {
                continue;
            }
            if geo.doors.is_empty() {
                let (r, n) = buildings::roof_cover(id).map_or((0, 1), |c| c.roofed());
                if r > 0 {
                    gained.push(format!("{id} ({:.0}% roofed)", 100.0 * r as f32 / n as f32));
                }
            }
            let shell = Shell::of(Entity::PLACEHOLDER, id, Transform::IDENTITY);
            // What it was: a box, and only with a door.
            let was = Shell {
                cover: None,
                ..shell
            };
            let had = !geo.doors.is_empty();
            let (mut flips, mut all) = (0, 0);
            let f = extraction_core::map::prefab_footprint(id);
            let (hx, hz) = (f[0] * 0.5 - 0.3, f[2] * 0.5 - 0.3);
            let y = 1.8;
            let mut x = -hx;
            while x <= hx {
                let mut z = -hz;
                while z <= hz {
                    let p = Vec3::new(x, y, z);
                    all += 1;
                    if (had && was.contains(p)) != shell.contains(p) {
                        flips += 1;
                    }
                    // Clear of a roof's edge: the same all round.
                    let truth = roofed(&geo, p);
                    let steady = [(0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6)]
                        .iter()
                        .all(|(a, b)| roofed(&geo, p + Vec3::new(*a, 0.0, *b)) == truth);
                    // (Not inside a wall or a piece of furniture.)
                    let solid = !roofed(&geo, p - Vec3::Y * 0.05) && truth;
                    if steady && !solid {
                        checked += 1;
                        if shell.contains(p) != truth {
                            bad.push(format!(
                                "{id} at ({x:.1}, {z:.1}): {} but {}",
                                if truth {
                                    "under a roof"
                                } else {
                                    "under the sky"
                                },
                                if truth { "not indoors" } else { "indoors" }
                            ));
                        }
                    }
                    z += 1.0;
                }
                x += 1.0;
            }
            if flips > 0 {
                changed.push(format!("{id} {:.0}%", 100.0 * flips as f32 / all as f32));
            }
        }
        println!("indoors now, with no door: {}", gained.join(", "));
        println!("points whose indoors changed: {}", changed.join(", "));
        assert!(checked > 500, "only {checked} points checked");
        assert!(
            bad.is_empty(),
            "{} of {checked} wrong:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }
    use crate::robots::{Brain, Job, JobKind};

    /// A player hiding in a house; a walker sent to look for them.
    fn hide_and_send_walker(wedge: bool) -> (App, Entity) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let entrances = app.world().resource::<Entrances>();
        // A village house's front door.
        let ent = *entrances
            .doors
            .iter()
            .find(|e| {
                let b = app.world().get::<Building>(e.building).unwrap();
                b.prefab == "house_plank"
            })
            .expect("a house door");
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let inside = ent.mid - ent.out * 2.0;
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = inside.with_y(ent.mid.y - 1.15);
        // Standing (nothing drives the rig here).
        app.world_mut()
            .get_mut::<crate::rig::RigInput>(player)
            .unwrap()
            .head
            .translation
            .y = 1.65;
        if wedge {
            let catalog = app.world().resource::<Catalog>().clone();
            let leaf = *app.world().get::<GlobalTransform>(ent.door).unwrap();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut c = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind("door_wedge").unwrap(),
                        transform: Transform::from_translation(
                            leaf.transform_point(Vec3::new(0.5, 0.1, -0.15)),
                        ),
                        condition: 1.0,
                    },
                );
            }
            queue.apply(app.world_mut());
            for _ in 0..90 {
                app.update();
            }
            assert!(
                app.world().get::<Wedged>(ent.door).is_some(),
                "the wedge jams the door"
            );
        }
        let start = ent.mid + ent.out * 14.0;
        let start = zone.on_ground(start.x, start.z);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let walker = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::walker::spawn_walker_with(
                &mut c,
                &zone,
                start,
                Job {
                    kind: JobKind::Response,
                    site: start,
                    duration: 999.0,
                    exit: start,
                },
                false,
                100.0,
            )
        };
        queue.apply(app.world_mut());
        app.update();
        *app.world_mut().get_mut::<Brain>(walker).unwrap() = Brain::Investigate {
            at: inside,
            left: 120.0,
        };
        (app, ent.door)
    }

    #[test]
    fn a_drone_waits_outside_the_doorway() {
        let (mut app, door) = hide_and_send_walker(false);
        let ent = *app
            .world()
            .resource::<Entrances>()
            .doors
            .iter()
            .find(|e| e.door == door)
            .unwrap();
        let start = ent.mid + ent.out * 30.0 + Vec3::Y * 15.0;
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let drone = {
            let mut c = Commands::new(&mut queue, app.world());
            crate::robots::drone::spawn_drone(
                &mut c,
                start,
                Job {
                    kind: JobKind::Response,
                    site: start,
                    duration: 999.0,
                    exit: start,
                },
            )
        };
        queue.apply(app.world_mut());
        app.update();
        let inside = ent.mid - ent.out * 2.0;
        *app.world_mut().get_mut::<Brain>(drone).unwrap() = Brain::Investigate {
            at: inside,
            left: 120.0,
        };
        for _ in 0..90 * 15 {
            app.update();
        }
        let p = app
            .world()
            .get::<GlobalTransform>(drone)
            .unwrap()
            .translation();
        let flat = |v: Vec3| Vec2::new(v.x, v.z);
        let off = flat(p) - flat(ent.mid);
        assert!(
            off.length() < 10.0 && off.dot(flat(ent.out)) > 0.0,
            "hovering outside the door ({off:.1})"
        );
    }

    #[test]
    fn a_walker_opens_the_door_you_hide_behind() {
        let (mut app, door) = hide_and_send_walker(false);
        let mut opened = false;
        for _ in 0..90 * 40 {
            app.update();
            if app.world().get::<Door>(door).unwrap().open {
                opened = true;
                break;
            }
        }
        assert!(opened, "the walker opened the door");
    }

    #[test]
    fn a_wedged_door_holds_until_a_walker_tears_it_down() {
        let (mut app, door) = hide_and_send_walker(true);
        let mut gone_after = None;
        for i in 0..90 * 60 {
            app.update();
            if let Some(d) = app.world().get::<Door>(door) {
                assert!(!d.open, "a wedged door doesn't open");
            } else {
                gone_after = Some(i as f32 / 90.0);
                break;
            }
        }
        let t = gone_after.expect("the walker tore the door down");
        assert!(t > BATTER_TIME, "it took a while ({t:.1} s)");
    }
}
