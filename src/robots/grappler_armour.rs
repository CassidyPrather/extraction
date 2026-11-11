//! A grappler's derrick stops rounds: its boom, the launcher at its head
//! and the line drum behind the orb (Models' `harpoon.glb` `launcher`,
//! art/robots/logistics.py). Without them a round at the boom, the first
//! thing on a grappler you'd shoot at, went through it.
//!
//! Armour, not parts: a round stops on it with a spark and a ping
//! ([`StruckMetal`]) and does nothing, as on a walker's flare tube. They
//! meet nothing but queries (rounds, bolts, a flare's flight): no
//! contacts with the world, debris or other robots, so a 3 m head never
//! snags a branch or a door frame, and no mass, so a wreck rolls as it
//! did.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::grappler::Grappler;
use crate::weapons::impacts::StruckMetal;
use crate::world::Layer;

pub struct GrapplerArmourPlugin;

impl Plugin for GrapplerArmourPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, armour_derricks);
    }
}

/// The boom's pins, robot space: its foot on the slewing yoke and its
/// head under the launcher.
pub const BOOM_FOOT: Vec3 = Vec3::new(0.0, 0.55, 0.10);
pub const BOOM_HEAD: Vec3 = Vec3::new(0.0, 1.804, -0.567);
/// The boom's box: across (X), past each pin, deep.
const BOOM_ACROSS: f32 = 0.15;
const BOOM_PAST: f32 = 0.03;
const BOOM_DEEP: f32 = 0.17;
/// The launcher's housing at the head: across, tall, long (along Z).
const LAUNCHER: Vec3 = Vec3::new(0.18, 0.16, 0.42);
/// The line drum: its middle, flange radius and width (axis along X).
const DRUM_AT: Vec3 = Vec3::new(0.0, 0.50, 0.46);
const DRUM_R: f32 = 0.25;
const DRUM_WIDE: f32 = 0.42;

/// Marks a grappler's derrick armour (for tests and the look).
#[derive(Component, Clone, Copy, Debug)]
pub struct Derrick;

/// The boom's box and where it sits: centred between its pins, its long
/// side along them.
pub fn boom() -> (Collider, Transform) {
    let along = BOOM_HEAD - BOOM_FOOT;
    let len = along.length() + 2.0 * BOOM_PAST;
    (
        Collider::cuboid(BOOM_ACROSS, len, BOOM_DEEP),
        Transform::from_translation((BOOM_FOOT + BOOM_HEAD) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, along.normalize())),
    )
}

fn armour_derricks(mut commands: Commands, new: Query<Entity, Added<Grappler>>) {
    for e in &new {
        let (boom, boom_at) = boom();
        let pieces = [
            ("Grappler boom", boom, boom_at),
            (
                "Grappler launcher",
                Collider::cuboid(LAUNCHER.x, LAUNCHER.y, LAUNCHER.z),
                Transform::from_translation(BOOM_HEAD),
            ),
            (
                "Grappler drum",
                Collider::cylinder(DRUM_R, DRUM_WIDE),
                Transform::from_translation(DRUM_AT)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ),
        ];
        commands.entity(e).with_children(|c| {
            for (name, collider, at) in pieces {
                c.spawn((
                    Name::new(name),
                    Derrick,
                    StruckMetal,
                    collider,
                    // Met by queries masking Robot; meets nothing itself.
                    CollisionLayers::new(Layer::Robot, LayerMask::NONE),
                    ColliderDensity(0.0),
                    at,
                    Visibility::default(),
                ));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The boom's box runs pin to pin, a little past each.
    #[test]
    fn the_boom_box_spans_its_pins() {
        let (c, t) = boom();
        let half = c.shape_scaled().as_cuboid().unwrap().half_extents.y;
        let ends = [
            t.transform_point(Vec3::Y * half),
            t.transform_point(-Vec3::Y * half),
        ];
        let dir = (BOOM_HEAD - BOOM_FOOT).normalize();
        assert!(
            ends[0].distance(BOOM_HEAD + dir * BOOM_PAST) < 1e-3,
            "{:?}",
            ends[0]
        );
        assert!(
            ends[1].distance(BOOM_FOOT - dir * BOOM_PAST) < 1e-3,
            "{:?}",
            ends[1]
        );
    }

    /// A round at a grappler's boom, from in front, stops on it (a spark,
    /// metal), not on whatever's behind.
    #[test]
    fn a_round_at_the_boom_stops_on_it() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 255.0);
        let job = super::super::Job {
            kind: super::super::JobKind::Survey,
            site: at,
            duration: 999.0,
            exit: at,
        };
        let e = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::grappler::spawn_grappler(&mut c, &zone, at, job);
            c.entity(e).remove::<super::super::Senses>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..10 {
            app.update();
        }
        let root = *app.world().get::<GlobalTransform>(e).unwrap();
        let derricks = app
            .world_mut()
            .query_filtered::<&ChildOf, With<Derrick>>()
            .iter(app.world())
            .filter(|p| p.parent() == e)
            .count();
        assert_eq!(derricks, 3, "boom, launcher and drum");
        // High on the boom, above the orb's parts: 3/4 of the way up.
        let aim = root.transform_point(BOOM_FOOT.lerp(BOOM_HEAD, 0.75));
        let from = aim + root.right() * 6.0;
        let hit = app
            .world_mut()
            .run_system_once(move |q: SpatialQuery| {
                q.cast_ray(
                    from,
                    Dir3::new(aim - from).unwrap(),
                    20.0,
                    true,
                    &SpatialQueryFilter::from_mask([Layer::World, Layer::Robot]),
                )
            })
            .unwrap();
        let hit = hit.expect("the round went through the boom");
        assert!(
            app.world().get::<Derrick>(hit.entity).is_some()
                && app.world().get::<StruckMetal>(hit.entity).is_some(),
            "it stopped on {:?}",
            app.world().get::<Name>(hit.entity)
        );
        assert!((hit.distance - 6.0).abs() < 0.2, "at {:.2} m", hit.distance);
    }
}
