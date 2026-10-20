//! Armour plates: curved plates bolted over a walker's orb (low front and
//! both flanks) and under a drone's belly. Each is a part with its own
//! collider, standing proud of the shell, so a round that strikes it hurts
//! the plate, not the core behind. Shoot one through and it comes loose
//! and drops (a piece of debris like any other), and the core there is
//! open. From the front a walker takes a few more hits; flank it, or strip
//! a plate, and its core is there to be had. The eye is never covered.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::wreck::{Debris, Piece, spawn_debris};
use super::{PartDestroyed, PartRole, Robot, RobotKind, part_bundle};

/// A walker plate's hit points (the core has 130).
pub const WALKER_PLATE_HP: f32 = 40.0;
/// A drone's belly plate (its shell has 70).
pub const DRONE_PLATE_HP: f32 = 20.0;
/// The Sorter's carriage plates (the carriage has 260).
pub const SORTER_PLATE_HP: f32 = 70.0;
/// A dirigible gondola's side plates (the gondola has 260).
pub const GONDOLA_PLATE_HP: f32 = 80.0;
/// How far out from the orb's centre a walker plate's middle sits.
const PLATE_OUT: f32 = 0.405;

/// Where a walker's plates sit: (latitude, azimuth) in degrees, azimuth 0
/// facing forward (-Z), positive to the right.
pub const WALKER_PLATES: [(f32, f32); 3] = [(-22.0, 0.0), (5.0, -90.0), (5.0, 90.0)];

pub struct ArmourPlugin;

impl Plugin for ArmourPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (fit_plates, shed_plates, pop_eyes).after(super::perceive),
        );
    }
}

/// The outward direction of a walker plate, robot space.
pub fn walker_plate_dir(i: usize) -> Vec3 {
    let (lat, az) = WALKER_PLATES[i];
    let (lat, az) = (lat.to_radians(), az.to_radians());
    Vec3::new(lat.cos() * az.sin(), lat.sin(), -lat.cos() * az.cos())
}

/// The plate's place on its robot (local to the root).
fn plate_transform(kind: RobotKind, i: usize) -> Transform {
    match kind {
        RobotKind::Sorter => {
            // Either side of the carriage, facing out (the model's plate is
            // on +X; the second turns round to -X).
            if i == 0 {
                Transform::from_xyz(0.74, 0.0, 0.0)
            } else {
                Transform::from_xyz(-0.74, 0.0, 0.0)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            }
        }
        RobotKind::Dirigible => {
            // Down each side of the gondola (its centre is 3.1 m under the
            // envelope's), facing out.
            let side = if i == 0 { 1.0 } else { -1.0 };
            Transform::from_xyz(side * 0.73, -super::dirigible::RADIUS - 0.5, 0.0)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::X * side))
        }
        RobotKind::Drone => {
            // Under the belly, between it and the gun pod, facing down.
            Transform::from_xyz(0.0, -0.075, 0.03)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::PI))
        }
        _ => {
            let d = walker_plate_dir(i);
            Transform::from_translation(d * PLATE_OUT)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, d))
        }
    }
}

/// Armour goes on as robots are made: three plates on walkers (grappler,
/// collector and crawler too: they're walkers), one on drones, two down
/// the sides of a dirigible's gondola.
fn fit_plates(
    mut commands: Commands,
    robots: Query<(Entity, &Robot, Has<super::hound::Hound>), Added<Robot>>,
) {
    for (e, robot, hound) in &robots {
        // (A hound's body is all legs and speed.)
        if hound {
            continue;
        }
        let (n, hp, collider) = match robot.kind {
            RobotKind::Walker => (
                WALKER_PLATES.len(),
                WALKER_PLATE_HP,
                Collider::cuboid(0.3, 0.03, 0.28),
            ),
            RobotKind::Drone => (1, DRONE_PLATE_HP, Collider::cuboid(0.3, 0.02, 0.28)),
            RobotKind::Sorter => (2, SORTER_PLATE_HP, Collider::cuboid(0.06, 0.5, 1.0)),
            // (Local Y is out: thin along it.)
            RobotKind::Dirigible => (2, GONDOLA_PLATE_HP, Collider::cuboid(0.86, 0.06, 3.3)),
            _ => continue,
        };
        let Ok(mut ec) = commands.get_entity(e) else {
            continue;
        };
        ec.with_children(|c| {
            for i in 0..n {
                c.spawn((
                    Name::new("Armour plate"),
                    part_bundle(
                        e,
                        PartRole::Armour(i as u8),
                        hp,
                        collider.clone(),
                        plate_transform(robot.kind, i),
                    ),
                ));
            }
        });
    }
}

/// A plate shot through comes loose and drops, and its collider goes with
/// it: what's behind is open.
fn shed_plates(
    mut commands: Commands,
    mut events: MessageReader<PartDestroyed>,
    parts: Query<&GlobalTransform>,
    robots: Query<(
        &Robot,
        Option<&LinearVelocity>,
        Option<&super::walker::Walker>,
    )>,
) {
    for ev in events.read() {
        let PartRole::Armour(_) = ev.role else {
            continue;
        };
        let Ok(gt) = parts.get(ev.part) else {
            continue;
        };
        let Ok((robot, lin, walker)) = robots.get(ev.robot) else {
            continue;
        };
        let (scale, rot, at) = gt.to_scale_rotation_translation();
        let out = rot * Vec3::Y;
        let v0 = lin
            .map(|v| v.0)
            .or(walker.map(|w| w.velocity))
            .unwrap_or_default();
        let seed = ev.part.index_u32() as f32;
        let (collider, mass, inertia) = if robot.kind == RobotKind::Dirigible {
            (
                Collider::cuboid(0.86, 0.06, 3.3),
                40.0,
                Vec3::new(36.0, 39.0, 2.5),
            )
        } else if robot.kind == RobotKind::Sorter {
            (
                Collider::cuboid(0.06, 0.5, 1.0),
                14.0,
                Vec3::new(1.4, 1.2, 0.3),
            )
        } else {
            (
                Collider::cuboid(0.3, 0.03, 0.28),
                2.0,
                Vec3::new(0.013, 0.028, 0.015),
            )
        };
        spawn_debris(
            &mut commands,
            (
                Debris {
                    kind: robot.kind,
                    piece: Piece::Plate,
                },
                Transform::from_translation(at).with_rotation(rot),
                v0 + out * 1.6 + Vec3::Y * 0.6,
                Vec3::new(seed.sin(), 0.4, seed.cos()) * 5.0,
                mass,
                collider,
                inertia,
            ),
            scale.x,
        );
        commands.entity(ev.part).despawn();
    }
}

/// An eye shot out pops off its mount: a drone's camera dome or a
/// walker's visor flies off as debris. The part itself stays (hidden, no
/// collider), so the robot still knows it's blind.
fn pop_eyes(
    mut commands: Commands,
    mut events: MessageReader<PartDestroyed>,
    parts: Query<&GlobalTransform>,
    robots: Query<(
        &Robot,
        Option<&LinearVelocity>,
        Option<&super::walker::Walker>,
    )>,
) {
    for ev in events.read() {
        if ev.role != PartRole::Sensor {
            continue;
        }
        let (Ok(gt), Ok((robot, lin, walker))) = (parts.get(ev.part), robots.get(ev.robot)) else {
            continue;
        };
        if !matches!(robot.kind, RobotKind::Drone | RobotKind::Walker) {
            continue;
        }
        let (scale, rot, at) = gt.to_scale_rotation_translation();
        let v0 = lin
            .map(|v| v.0)
            .or(walker.map(|w| w.velocity))
            .unwrap_or_default();
        let fwd = rot * Vec3::NEG_Z;
        let (collider, mass, inertia) = if robot.kind == RobotKind::Drone {
            (Collider::sphere(0.07), 0.3, Vec3::splat(0.0006))
        } else {
            (
                Collider::cuboid(0.3, 0.08, 0.06),
                1.2,
                Vec3::new(0.001, 0.009, 0.009),
            )
        };
        spawn_debris(
            &mut commands,
            (
                Debris {
                    kind: robot.kind,
                    piece: Piece::Sensor,
                },
                Transform::from_translation(at).with_rotation(rot),
                v0 + fwd * 1.5 + Vec3::Y * 1.0,
                Vec3::new(4.0, 2.0, -3.0),
                mass,
                collider,
                inertia,
            ),
            scale.x,
        );
        commands
            .entity(ev.part)
            .insert((Visibility::Hidden, ColliderDisabled));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plates face out from the orb, never over the eye (straight ahead
    /// and a little up).
    #[test]
    fn plates_leave_the_eye_clear() {
        let eye = Vec3::new(0.0, 0.12, -0.36).normalize();
        for i in 0..WALKER_PLATES.len() {
            let d = walker_plate_dir(i);
            assert!((d.length() - 1.0).abs() < 1e-4);
            assert!(
                d.angle_between(eye).to_degrees() > 30.0,
                "plate {i} covers the eye"
            );
        }
    }

    /// A walker shot square in its low front plate: the rounds hurt the
    /// plate, not the core, until the plate comes off; then the same line
    /// of fire reaches the core.
    #[test]
    fn a_plate_takes_the_rounds_until_it_drops() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 230.0);
        let walker = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::walker::spawn_walker(
                &mut c,
                &zone,
                at,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..20 {
            app.update();
        }
        let hp = |app: &mut App, role: PartRole| {
            app.world_mut()
                .query::<&super::super::RobotPart>()
                .iter(app.world())
                .find(|p| p.robot == walker && p.role == role)
                .map(|p| p.hp)
        };
        assert_eq!(hp(&mut app, PartRole::Armour(0)), Some(WALKER_PLATE_HP));
        let core0 = hp(&mut app, PartRole::Core).unwrap();
        let fire = |app: &mut App| {
            let t = *app.world().get::<GlobalTransform>(walker).unwrap();
            let aim = t.transform_point(walker_plate_dir(0) * 0.3);
            // (Close: the low plate faces down toward the ground.)
            let origin = t.transform_point(walker_plate_dir(0) * 1.8);
            app.world_mut()
                .write_message(crate::weapons::ballistics::Shot {
                    gun: Entity::PLACEHOLDER,
                    shooter: None,
                    origin,
                    direction: (aim - origin).normalize(),
                    speed: 700.0,
                    damage: 10.0,
                    projectiles: 1,
                    spread: 0.0,
                    recoil: 0.0,
                    seed: 1,
                });
            for _ in 0..6 {
                app.update();
            }
        };
        for _ in 0..3 {
            fire(&mut app);
        }
        assert_eq!(
            hp(&mut app, PartRole::Core),
            Some(core0),
            "the plate took them"
        );
        assert!(hp(&mut app, PartRole::Armour(0)).unwrap() < WALKER_PLATE_HP);
        for _ in 0..2 {
            fire(&mut app);
        }
        assert_eq!(
            hp(&mut app, PartRole::Armour(0)),
            None,
            "the plate came off"
        );
        let loose = app
            .world_mut()
            .query::<&Debris>()
            .iter(app.world())
            .filter(|d| d.piece == Piece::Plate)
            .count();
        assert_eq!(loose, 1, "and lies on the ground");
        // (Let it fall clear of the line of fire.)
        for _ in 0..90 {
            app.update();
        }
        fire(&mut app);
        assert!(
            hp(&mut app, PartRole::Core).unwrap() < core0,
            "now the core is open"
        );
    }

    /// A drone's eye shot out pops off as a loose dome, and the drone stays
    /// blind (its sensor part is kept, hidden).
    #[test]
    fn a_shot_out_eye_pops_off() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let site = zone.on_ground(40.0, 200.0) + Vec3::Y * 8.0;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::drone::spawn_drone(
                &mut c,
                site,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
            q.apply(app.world_mut());
            e
        };
        for _ in 0..10 {
            app.update();
        }
        let (eye, at) = app
            .world_mut()
            .query::<(Entity, &super::super::RobotPart, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, p, _)| p.robot == drone && p.role == PartRole::Sensor)
            .map(|(e, _, g)| (e, g.translation()))
            .unwrap();
        app.world_mut()
            .write_message(crate::weapons::ballistics::Hit {
                entity: eye,
                point: at,
                normal: Vec3::NEG_Z,
                velocity: Vec3::Z * 500.0,
                damage: 100.0,
                shooter: None,
                mass_g: 3.4,
            });
        for _ in 0..3 {
            app.update();
        }
        let domes = app
            .world_mut()
            .query::<&Debris>()
            .iter(app.world())
            .filter(|d| d.piece == Piece::Sensor)
            .count();
        assert_eq!(domes, 1, "the dome flew off");
        assert_eq!(
            app.world().get::<Visibility>(eye),
            Some(&Visibility::Hidden)
        );
        assert!(
            !app.world()
                .get::<super::super::RobotPart>(eye)
                .unwrap()
                .alive(),
            "and the drone knows it's blind"
        );
    }
}
