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
//!
//! Wrecked, the derrick folds ([`Folds`]): the boom comes down about its
//! foot pin, back along the hull and a little below level, so the wreck
//! lies with it, not with a mast standing up out of it.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::grappler::Grappler;
use crate::weapons::impacts::StruckMetal;
use crate::world::Layer;

pub struct GrapplerArmourPlugin;

impl Plugin for GrapplerArmourPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, armour_derricks)
            .add_systems(Update, fold_derricks);
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

/// What comes down when a grappler is wrecked: its boom and launcher
/// (their armour here; the look's holder, `render::logistics`). Its own
/// transform, under the grappler's root, is the standing pose.
#[derive(Component, Clone, Copy, Debug)]
pub struct Folds;

/// A folding piece's standing pose, kept from its first fold.
#[derive(Component, Clone, Copy, Debug)]
struct Standing(Transform);

/// How far a wrecked grappler's derrick has folded, 0..1.
#[derive(Component, Clone, Copy, Debug, Default)]
struct Folded(f32);

/// Seconds the fold takes, as the wreck goes down.
pub const FOLD_TIME: f32 = 0.5;
/// Folded, the boom points back along the hull this far below level (rad).
pub const FOLD_BELOW: f32 = 0.17;

/// The fold's full turn, about [`BOOM_FOOT`]: the boom's line from up and
/// forward to back and a little down.
pub fn fold_turn() -> Quat {
    let up = (BOOM_HEAD - BOOM_FOOT).normalize();
    let down = Vec3::new(0.0, -FOLD_BELOW.sin(), FOLD_BELOW.cos());
    Quat::from_rotation_arc(up, down)
}

/// `standing` turned `k` of the way through the fold, about the foot pin.
pub fn folded(standing: Transform, k: f32) -> Transform {
    let q = Quat::IDENTITY.slerp(fold_turn(), k.clamp(0.0, 1.0));
    Transform {
        translation: BOOM_FOOT + q * (standing.translation - BOOM_FOOT),
        rotation: q * standing.rotation,
        scale: standing.scale,
    }
}

#[allow(clippy::type_complexity)]
fn fold_derricks(
    mut commands: Commands,
    mut grapplers: Query<(Entity, &super::Robot, &Children, Option<&mut Folded>), With<Grappler>>,
    mut pieces: Query<(&mut Transform, Option<&Standing>), With<Folds>>,
    time: Res<Time>,
) {
    for (e, robot, children, folded_by) in &mut grapplers {
        if !robot.wrecked {
            continue;
        }
        let Some(mut f) = folded_by else {
            commands.entity(e).try_insert(Folded(0.0));
            continue;
        };
        if f.0 >= 1.0 {
            continue;
        }
        f.0 = (f.0 + time.delta_secs() / FOLD_TIME).min(1.0);
        // Eased: slow off the yoke, quick to the hull.
        let k = f.0 * f.0;
        for c in children.iter() {
            let Ok((mut t, standing)) = pieces.get_mut(c) else {
                continue;
            };
            let base = match standing {
                Some(s) => s.0,
                None => {
                    commands.entity(c).try_insert(Standing(*t));
                    *t
                }
            };
            *t = folded(base, k);
        }
    }
}

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
        // (Name, hit volume, place, whether it folds on a wreck.)
        let pieces = [
            ("Grappler boom", boom, boom_at, true),
            (
                "Grappler launcher",
                Collider::cuboid(LAUNCHER.x, LAUNCHER.y, LAUNCHER.z),
                Transform::from_translation(BOOM_HEAD),
                true,
            ),
            (
                "Grappler drum",
                Collider::cylinder(DRUM_R, DRUM_WIDE),
                Transform::from_translation(DRUM_AT)
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                false,
            ),
        ];
        commands.entity(e).with_children(|c| {
            for (name, collider, at, folds) in pieces {
                let mut piece = c.spawn((
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
                if folds {
                    piece.insert(Folds);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Folded, the boom lies back along the hull a little below level, its
    /// foot pin where it was; half way, it's on its way down.
    #[test]
    fn the_boom_folds_back_about_its_foot() {
        let (_, standing) = boom();
        let down = folded(standing, 1.0);
        let along = down.rotation * Vec3::Y;
        // (Pointing back, +Z, and down by FOLD_BELOW, either way along it.)
        let dir = if along.z < 0.0 { -along } else { along };
        assert!(dir.z > 0.95, "{dir}");
        assert!((dir.y + FOLD_BELOW.sin()).abs() < 0.02, "{dir}");
        // The foot pin doesn't move.
        let foot = |t: Transform| {
            let half = (BOOM_HEAD - BOOM_FOOT).length() * 0.5;
            t.translation + t.rotation * Vec3::Y * -half
        };
        assert!(foot(down).distance(BOOM_FOOT) < 1e-3, "{}", foot(down));
        assert!(
            folded(standing, 0.0)
                .translation
                .distance(standing.translation)
                < 1e-5
        );
        let mid = folded(standing, 0.5);
        assert!(
            mid.translation.y < standing.translation.y && mid.translation.y > down.translation.y
        );
    }

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
