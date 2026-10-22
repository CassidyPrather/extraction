//! The ballistics range: every gun fired at a panel of every material,
//! through the real projectile path, against a table of what should
//! happen (it stops, it goes through, it glances off). Built high above
//! the valley, out of everything's way, with a witness plate behind each
//! panel: a hit on the witness is a round that got through.
//!
//! `run_range` drives it in a headless app; the scenario `ballistics_range`
//! and a test both use it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm_terminal::Stuff;

use super::Gun;
use super::ballistics::{Hit, Ricochet, Shot};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::render::materials::Mat;
use crate::world::Layer;
use crate::world::surfaces::Surfaces;

/// What should happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Stop,
    Through,
    /// Most rounds at a graze glance off.
    Glance,
}

/// The guns on the range (catalog ids).
pub const GUNS: [&str; 11] = [
    "pm_pistol",
    "ak74",
    "mosin",
    "sks",
    "toz_double",
    "mp133",
    "svd",
    "toz8",
    "rpk74",
    "nagant",
    "ppsh",
];

/// Robots' guns on the range, firing their own 8 g rounds through the same
/// projectiles: (name, muzzle speed, damage), as `robots` fires them.
pub const ROBOT_GUNS: [(&str, f32, f32); 3] = [
    ("walker", 450.0, 14.0),
    ("drone", 380.0, 9.0),
    ("sentry", 600.0, 16.0),
];

/// The panels: a material and its thickness, as buildings use them.
pub const PANELS: [(Mat, f32); 10] = [
    (Mat::Planks, 0.1),
    (Mat::LogWall, 0.25),
    (Mat::Plaster, 0.15),
    (Mat::Brick, 0.25),
    (Mat::Concrete, 0.2),
    (Mat::Boulder, 0.5),
    (Mat::Corrugated, 0.02),
    (Mat::RobotShell, 0.03),
    (Mat::Glass, 0.01),
    // A car's door, as a wreck's parts are (`destruction::strip`): a
    // hollow of two skins.
    (Mat::PaintedMetal, 0.092),
];

/// Head on: does `gun` get through `mat`, `thick` metres of it?
pub fn expected(gun: &str, mat: Mat, thick: f32) -> Expect {
    use Expect::*;
    let rifle = matches!(gun, "ak74" | "mosin" | "sks" | "svd" | "rpk74");
    // Pistols, revolvers, buckshot and a .22: what a car door stops.
    let light = matches!(
        gun,
        "pm_pistol" | "nagant" | "toz_double" | "mp133" | "toz8"
    );
    match super::ballistics::piece_stuff(mat, Vec3::splat(thick * 0.5)) {
        Stuff::Glass => Through,
        Stuff::Shell => {
            if light {
                Stop
            } else {
                Through
            }
        }
        // Robots' rounds: through planks; a walker's and a sentry's
        // through plaster too, a drone's not.
        Stuff::Planks if matches!(gun, "walker" | "drone" | "sentry") => Through,
        Stuff::Plaster if matches!(gun, "walker" | "sentry") => Through,
        // A .22 stops in tin.
        Stuff::Sheet if gun != "toz8" => Through,
        Stuff::Planks | Stuff::Plaster if rifle => Through,
        // The PPSh's 7.62x25 is a hot, light, pointed bullet: through a
        // plank wall where a pistol's stops.
        Stuff::Planks if gun == "ppsh" => Through,
        Stuff::Logs if matches!(gun, "mosin" | "svd") => Through,
        _ => Stop,
    }
}

/// At a graze: hard faces turn rifle rounds.
pub fn expected_graze(mat: Mat, thick: f32) -> Option<Expect> {
    super::ballistics::piece_stuff(mat, Vec3::splat(thick * 0.5))
        .ricochets()
        .then_some(Expect::Glance)
}

/// The line of fire: head on into a panel facing +Z, or a graze, 0.03
/// off its face.
fn line(graze: bool) -> Vec3 {
    if graze {
        Vec3::new(-1.0, 0.0, -0.03).normalize()
    } else {
        Vec3::NEG_Z
    }
}

/// Where the range is built: well above any valley.
const ALTITUDE: f32 = 900.0;
/// Panels this far apart, so no round strays into a neighbour.
const SPACING: f32 = 6.0;

#[derive(Component)]
struct Panel;

#[derive(Component)]
struct Witness {
    cell: usize,
}

/// One trial: a gun, a panel, head on or at a graze.
#[derive(Clone, Debug)]
pub struct Cell {
    pub gun: &'static str,
    pub mat: Mat,
    pub graze: bool,
    pub want: Expect,
    pub shots: u32,
    pub through: u32,
    pub glanced: u32,
}

impl Cell {
    pub fn got(&self) -> Expect {
        match self.want {
            Expect::Glance if self.glanced * 2 > self.shots => Expect::Glance,
            _ if self.through > 0 && self.through * 2 >= self.shots => Expect::Through,
            _ if self.glanced * 2 > self.shots => Expect::Glance,
            _ => Expect::Stop,
        }
    }
}

/// Builds the range in `app`'s world and fires every trial; returns the
/// cells with what happened.
pub fn run_range(app: &mut App) -> Vec<Cell> {
    let catalog = app.world().resource::<Catalog>().clone();
    let mut cells = Vec::new();
    let mut guns = Vec::new();
    let names = GUNS.iter().chain(ROBOT_GUNS.iter().map(|(n, _, _)| n));
    for (g, id) in names.enumerate() {
        let robot = ROBOT_GUNS.iter().any(|(n, _, _)| n == id);
        if robot {
            // A robot's gun: no cartridge, its own round.
            let gun = app.world_mut().spawn(Name::new("Robot's gun")).id();
            guns.push((*id, gun));
        }
        for (mat, thick) in PANELS.iter() {
            if !robot {
                break;
            }
            cells.push(Cell {
                gun: id,
                mat: *mat,
                graze: false,
                want: expected(id, *mat, *thick),
                shots: 0,
                through: 0,
                glanced: 0,
            });
        }
        if robot {
            continue;
        }
        let kind = catalog.kind(id).expect("a gun on the range");
        let gun = {
            let mut commands = app.world_mut().commands();
            spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_xyz(-50.0, ALTITUDE - 40.0, g as f32 * 3.0),
                    condition: 1.0,
                },
            )
        };
        guns.push((*id, gun));
        for (mat, thick) in PANELS.iter() {
            cells.push(Cell {
                gun: id,
                mat: *mat,
                graze: false,
                want: expected(id, *mat, *thick),
                shots: 0,
                through: 0,
                glanced: 0,
            });
            if let (Some(want), true) = (
                expected_graze(*mat, *thick),
                *id == "ak74" || *id == "mosin",
            ) {
                cells.push(Cell {
                    gun: id,
                    mat: *mat,
                    graze: true,
                    want,
                    shots: 0,
                    through: 0,
                    glanced: 0,
                });
            }
        }
    }
    app.world_mut().flush();
    app.update();
    for (i, cell) in cells.iter().enumerate() {
        let thick = PANELS.iter().find(|(m, _)| *m == cell.mat).unwrap().1;
        let at = Vec3::new(i as f32 * SPACING, ALTITUDE, 0.0);
        // The panel faces +Z; the round comes at it head on or at a graze.
        let half = Vec3::new(1.0, 1.0, thick * 0.5);
        let dir = line(cell.graze);
        app.world_mut().spawn((
            Panel,
            Surfaces(vec![(half, Transform::IDENTITY, cell.mat)]),
            Transform::from_translation(at),
            RigidBody::Static,
            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        // The witness, on along the round's line: it catches what went
        // through; a glance leaves the other way.
        app.world_mut().spawn((
            Witness { cell: i },
            Transform::from_translation(at + dir * 2.5).looking_to(dir, Vec3::Y),
            RigidBody::Static,
            Collider::cuboid(1.2, 1.2, 0.05),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    app.update();
    #[derive(Resource, Default)]
    struct Seen {
        witness: Vec<usize>,
        glances: Vec<Vec3>,
    }
    app.init_resource::<Seen>().add_systems(
        FixedUpdate,
        (|mut hits: MessageReader<Hit>,
          mut glances: MessageReader<Ricochet>,
          witnesses: Query<&Witness>,
          mut seen: ResMut<Seen>| {
            for h in hits.read() {
                if let Ok(w) = witnesses.get(h.entity) {
                    seen.witness.push(w.cell);
                }
            }
            seen.glances.extend(glances.read().map(|g| g.point));
        })
        .after(super::ballistics::BallisticsSystems),
    );
    // Fire each trial in turn and let its rounds settle.
    for i in 0..cells.len() {
        let gun_id = cells[i].gun;
        let gun = guns.iter().find(|(id, _)| *id == gun_id).unwrap().1;
        let (speed, damage) = match ROBOT_GUNS.iter().find(|(n, _, _)| *n == gun_id) {
            Some((_, v, d)) => (*v, *d),
            None => {
                let g = app.world().get::<Gun>(gun).expect("the range's gun");
                (g.firearm.def.muzzle_velocity, g.firearm.def.damage)
            }
        };
        let at = Vec3::new(i as f32 * SPACING, ALTITUDE, 0.0);
        let shots = if cells[i].graze { 8 } else { 3 };
        for k in 0..shots {
            // At the middle of the panel's face, from 6 m back along the line.
            let thick = PANELS.iter().find(|(m, _)| *m == cells[i].mat).unwrap().1;
            let dir = line(cells[i].graze);
            let face = at + Vec3::new(0.0, 0.1 * k as f32 - 0.3, thick * 0.5);
            let origin = face - dir * 6.0;
            let before = {
                let s = app.world().resource::<Seen>();
                (
                    s.witness.iter().filter(|c| **c == i).count(),
                    s.glances.len(),
                )
            };
            app.world_mut().write_message(Shot {
                gun,
                shooter: None,
                origin,
                direction: dir,
                speed,
                damage,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: (i as u64) << 8 | k as u64,
            });
            for _ in 0..6 {
                app.update();
            }
            let s = app.world().resource::<Seen>();
            let through = s.witness.iter().filter(|c| **c == i).count() > before.0;
            let glanced = s.glances.len() > before.1;
            cells[i].shots += 1;
            cells[i].through += u32::from(through && !glanced);
            cells[i].glanced += u32::from(glanced);
        }
    }
    cells
}

/// The cells that didn't go as the table says, for a report.
pub fn failures(cells: &[Cell]) -> Vec<String> {
    cells
        .iter()
        .filter(|c| c.got() != c.want)
        .map(|c| {
            format!(
                "{} {} {:?}: wanted {:?}, got {:?} ({} shots, {} through, {} glanced)",
                c.gun,
                if c.graze { "grazing" } else { "head on" },
                c.mat,
                c.want,
                c.got(),
                c.shots,
                c.through,
                c.glanced
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every gun at every material, head on, and the rifles at a graze off
    /// the hard ones: the pass/stop/glance matrix holds through the real
    /// projectiles.
    #[test]
    fn the_range_matrix_holds() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let cells = run_range(&mut app);
        assert!(cells.len() >= (GUNS.len() + ROBOT_GUNS.len()) * PANELS.len());
        let bad = failures(&cells);
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    use crate::rig::Side;
    use crate::weapons::ballistics::Hit;
    use crate::weapons::feel::vr_tests::{Range, hold, load, take_in};
    use extraction_core::firearm::FireMode;

    /// The hits written from now on, as the tick after `n` steps reads them.
    fn hits_over(vr: &mut crate::vr_harness::Vr, n: usize) -> Vec<Hit> {
        let mut cursor = vr
            .app
            .world()
            .resource::<Messages<Hit>>()
            .get_cursor_current();
        let mut out = Vec::new();
        for _ in 0..n {
            vr.step(1);
            out.extend(
                cursor
                    .read(vr.app.world().resource::<Messages<Hit>>())
                    .cloned(),
            );
        }
        out
    }

    /// A car door carried off a wreck and held up in the left hand as a
    /// shield: a pistol's round stops in it; a walker's 8 g round comes
    /// through it into the body behind.
    #[test]
    fn a_car_door_stops_a_pistol_not_a_walkers_round() {
        let mut vr = crate::vr_harness::Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        let mut r = Range {
            vr,
            gun: Entity::PLACEHOLDER,
            feet,
        };
        let door = take_in(
            &mut r,
            Side::Left,
            "car_door",
            feet + Vec3::new(0.0, 1.25, -0.4),
        );
        r.vr.step(20);
        assert_eq!(r.vr.hand_state(Side::Left).holding, Some(door));
        let walker_gun = r.vr.app.world_mut().spawn(Name::new("Walker's gun")).id();
        let pistol = r.vr.spawn(
            "pm_pistol",
            Transform::from_translation(feet + Vec3::new(6.0, 0.3, 6.0)),
        );
        r.vr.app
            .world_mut()
            .entity_mut(pistol)
            .insert(RigidBody::Static);
        r.vr.step(3);
        let mut bodies =
            r.vr.app
                .world_mut()
                .query_filtered::<Entity, With<crate::player::BodyCollider>>();
        let body: Vec<Entity> = bodies.iter(r.vr.app.world()).collect();
        let shoot = |r: &mut Range, gun: Entity, speed: f32, damage: f32| {
            // From 15 m off, through the door's face beside the hand on it,
            // where the door is now.
            let face =
                r.vr.app
                    .world()
                    .get::<GlobalTransform>(door)
                    .unwrap()
                    .translation();
            let at = face + Vec3::new(0.18, 0.1, -15.0);
            r.vr.app.world_mut().write_message(Shot {
                gun,
                shooter: Some(gun),
                origin: at,
                direction: Vec3::Z,
                speed,
                damage,
                projectiles: 1,
                spread: 0.0,
                recoil: 0.0,
                seed: 7,
            });
            let hits = hits_over(&mut r.vr, 10);
            let on_door = hits.iter().any(|h| h.entity == door);
            let on_body = hits.iter().any(|h| body.contains(&h.entity));
            (on_door, on_body)
        };
        let (door_hit, hurt) = shoot(&mut r, pistol, 315.0, 34.0);
        assert!(door_hit, "the pistol's round meets the door");
        assert!(!hurt, "and stops in it");
        let (door_hit, hurt) = shoot(&mut r, walker_gun, ROBOT_GUNS[0].1, ROBOT_GUNS[0].2);
        assert!(door_hit, "the walker's round meets the door");
        assert!(hurt, "and comes through it");
    }

    /// A rifle round fired from the hand goes in one side of a drone's
    /// thin shell and out the other, on to the plate behind it; a pistol's
    /// lodges in it.
    #[test]
    fn a_rifle_round_goes_through_a_drone_and_on() {
        let through = |id: &str, family: &str| -> (bool, bool) {
            let mut r = hold(id, false, false);
            load(&mut r, family, 10, FireMode::Semi);
            r.vr.step(5);
            let gt = *r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            let fwd = gt.forward().as_vec3();
            let at = gt.translation() + fwd * 8.0;
            // Side on to the line, held still and senseless.
            let drone = {
                let mut commands = r.vr.app.world_mut().commands();
                let job = crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site: at,
                    duration: 1.0e6,
                    exit: at,
                };
                crate::robots::drone::spawn_drone(&mut commands, at, job)
            };
            r.vr.app.world_mut().flush();
            r.vr.app.world_mut().entity_mut(drone).insert((
                RigidBody::Static,
                crate::robots::Stunned(60.0),
                Transform::from_translation(at).looking_to(fwd.cross(Vec3::Y).normalize(), Vec3::Y),
            ));
            let witness =
                r.vr.app
                    .world_mut()
                    .spawn((
                        Witness { cell: 0 },
                        Transform::from_translation(at + fwd * 3.0).looking_to(fwd, Vec3::Y),
                        RigidBody::Static,
                        Collider::cuboid(1.5, 1.5, 0.05),
                        CollisionLayers::new(Layer::World, LayerMask::ALL),
                    ))
                    .id();
            r.vr.step(5);
            // One pull of the trigger, the hits read as they land.
            r.vr.trigger(Side::Right, 1.0);
            let hits = hits_over(&mut r.vr, 20);
            r.vr.trigger(Side::Right, 0.0);
            let mut parts = r.vr.app.world_mut().query::<&crate::robots::RobotPart>();
            let on_drone = hits.iter().any(|h| {
                parts
                    .get(r.vr.app.world(), h.entity)
                    .is_ok_and(|p| p.robot == drone)
            });
            (on_drone, hits.iter().any(|h| h.entity == witness))
        };
        assert_eq!(
            through("ak74", "ak545"),
            (true, true),
            "an AK round: through and on"
        );
        assert_eq!(
            through("pm_pistol", "pm"),
            (true, false),
            "a pistol's: lodged"
        );
    }
}
