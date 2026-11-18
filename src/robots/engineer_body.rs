//! The engineer's body (#98, Robot bodies; its mind and rules are Robot
//! minds'): unarmed and helpless on its own, a pale workaday cart on four
//! short legs, its kit bundled on its back and a scan lamp on a short arm
//! at its front. It replaces the minelayer.
//!
//! A walker in its brain and feet (`RobotKind::Walker`, `walker_brain`,
//! `Walker.path`), with [`Engineer`] to tell it apart and `Shape::Engineer`
//! for its stance. Its parts: the hull (Core), the eye (Sensor), the scan
//! lamp (`Lamp(0)`) and the kit bundle (`PartRole::Kit`): what it builds
//! with, salvage when it falls. No gun.
//!
//! Its mind sets [`ScanLamp`] while it surveys a site (the lamp's beam
//! sweeps it, `engineer_look`) and [`Hammering`] while it builds (sparks
//! and the beat of its work).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::anim::Shape;
use super::{Job, PartRole, part_bundle};
use crate::world::Zone;

/// An engineer (on a walker).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Engineer;

/// Its scan lamp: lit while it surveys a site (its mind's 3 s), on `aim`
/// if it's looking at something, else ahead and down.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ScanLamp {
    pub on: bool,
    pub aim: Option<Vec3>,
}

/// It's at work on a build: hammering, sparks.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Hammering(pub bool);

/// The hull's health: no armour, a worker's.
pub const CORE_HP: f32 = 60.0;
/// The kit bundle on its back: where it sits (robot space) and how big.
pub const KIT_AT: Vec3 = Vec3::new(0.0, 0.3, 0.36);
pub const KIT_SIZE: Vec3 = Vec3::new(0.5, 0.24, 0.6);
pub const KIT_HP: f32 = 40.0;
/// The scan lamp, on its short arm at the hull's front right.
pub const LAMP_AT: Vec3 = Vec3::new(0.25, 0.2, -0.44);
pub const LAMP_HP: f32 = 25.0;

/// An engineer at `at`, at work on `job`: an unarmed walker in the
/// engineer's stance, its kit on its back and its scan lamp out front.
/// Senses and a brain as a walker's (`spawn_walker_shaped`); its mind
/// replaces the brain as it likes.
pub fn spawn_engineer(commands: &mut Commands, zone: &Zone, at: Vec3, job: Job) -> Entity {
    let e = super::walker::spawn_walker_shaped(
        commands,
        zone,
        at,
        job,
        false,
        CORE_HP,
        1.0,
        None,
        Shape::Engineer,
    );
    commands
        .entity(e)
        .insert((
            Name::new("Engineer"),
            Engineer,
            ScanLamp::default(),
            Hammering::default(),
        ))
        .with_children(|c| {
            c.spawn((
                Name::new("Engineer kit"),
                part_bundle(
                    e,
                    PartRole::Kit,
                    KIT_HP,
                    Collider::cuboid(KIT_SIZE.x, KIT_SIZE.y, KIT_SIZE.z),
                    Transform::from_translation(KIT_AT),
                ),
            ));
            c.spawn((
                Name::new("Engineer scan lamp"),
                part_bundle(
                    e,
                    PartRole::Lamp(0),
                    LAMP_HP,
                    Collider::cylinder(0.08, 0.12),
                    Transform::from_translation(LAMP_AT)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ),
            ));
        });
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Its parts are a worker's: a hull, an eye, a lamp and its kit, four
    /// legs, and no gun.
    #[test]
    fn an_engineer_is_unarmed_with_its_kit_and_lamp() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let at = zone.on_ground(40.0, 250.0);
        let e = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let job = Job {
                kind: super::super::JobKind::Survey,
                site: at,
                duration: 1.0e6,
                exit: at,
            };
            let e = spawn_engineer(&mut c, &zone, at, job);
            q.apply(app.world_mut());
            e
        };
        app.update();
        let w = app.world_mut();
        assert!(w.get::<Engineer>(e).is_some());
        let roles: Vec<PartRole> = w
            .query::<&super::super::RobotPart>()
            .iter(w)
            .filter(|p| p.robot == e)
            .map(|p| p.role)
            .collect();
        for want in [
            PartRole::Core,
            PartRole::Sensor,
            PartRole::Kit,
            PartRole::Lamp(0),
        ] {
            assert!(roles.contains(&want), "{want:?} in {roles:?}");
        }
        assert!(!roles.contains(&PartRole::Weapon), "no gun: {roles:?}");
        assert_eq!(
            roles
                .iter()
                .filter(|r| matches!(r, PartRole::Leg(_)))
                .count(),
            4
        );
    }
}
