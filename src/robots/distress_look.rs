//! A distress flare's look (Robot minds' `distress`): the tube a failing
//! walker or sentry raises over its tell, its red tick lamp, the tube shot
//! to pieces, and the canister going up.
//!
//! The tube is a squat launcher housing at the mount and a barrel that
//! rises out of it as `FlareTube::raised` goes 0 to 1. Raised, it fills
//! the tube's collider (`distress::TUBE_SIZE`), its top standing above a
//! walker's gun pod, where you can see it, and shoot it, from in front.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::distress::{Canister, Distress, FlareTube, TUBE_SIZE};

pub struct DistressLookPlugin;

impl Plugin for DistressLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_tubes, pose_tubes, dress_canisters));
    }
}

/// The housing's height, m: the foot of the tube's collider.
pub const HOUSING: f32 = 0.2;

/// The barrel's radius and length, m.
pub const BARREL_R: f32 = 0.09;
pub const BARREL: f32 = 0.42;

/// How fast the tick lamp ticks over the tell (a second).
pub const TICK: f32 = 5.0;

/// A tube's barrel, its lamp, and the materials it changes.
#[derive(Component)]
struct TubeLook {
    barrel: Entity,
    lamp: Entity,
    lamp_mat: Handle<StandardMaterial>,
    barrel_mat: Handle<StandardMaterial>,
    broken: bool,
}

#[derive(Default)]
struct Shared {
    housing: Handle<Mesh>,
    barrel: Handle<Mesh>,
    lamp: Handle<Mesh>,
    steel: Handle<StandardMaterial>,
}

/// Where the barrel's middle sits, local to the tube, `raised` 0 to 1:
/// sunk in its housing, then up till its top meets the collider's.
pub fn barrel_y(raised: f32) -> f32 {
    let foot = -TUBE_SIZE.y * 0.5;
    let down = foot + HOUSING - BARREL * 0.5 + 0.04;
    let up = TUBE_SIZE.y * 0.5 - BARREL * 0.5;
    let k = raised.clamp(0.0, 1.0);
    down + (up - down) * (k * k * (3.0 - 2.0 * k))
}

fn dress_tubes(
    mut commands: Commands,
    tubes: Query<Entity, Added<FlareTube>>,
    mut shared: Local<Option<Shared>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &tubes {
        let s = shared.get_or_insert_with(|| Shared {
            housing: meshes.add(Cuboid::new(TUBE_SIZE.x, HOUSING, TUBE_SIZE.z)),
            barrel: meshes.add(Cylinder::new(BARREL_R, BARREL)),
            lamp: meshes.add(Sphere::new(0.05).mesh().ico(2).unwrap()),
            steel: materials.add(StandardMaterial {
                base_color: Color::srgb(0.22, 0.23, 0.24),
                perceptual_roughness: 0.6,
                metallic: 0.6,
                ..default()
            }),
        });
        let barrel_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.32, 0.33, 0.34),
            perceptual_roughness: 0.5,
            metallic: 0.7,
            ..default()
        });
        // A lamp: through fog, and dark till the tell.
        let lamp_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.02, 0.02),
            emissive: LinearRgba::BLACK,
            fog_enabled: false,
            ..default()
        });
        commands.spawn((
            Mesh3d(s.housing.clone()),
            MeshMaterial3d(s.steel.clone()),
            Transform::from_xyz(0.0, -TUBE_SIZE.y * 0.5 + HOUSING * 0.5, 0.0),
            ChildOf(e),
        ));
        let barrel = commands
            .spawn((
                Mesh3d(s.barrel.clone()),
                MeshMaterial3d(barrel_mat.clone()),
                Transform::from_xyz(0.0, barrel_y(0.0), 0.0),
                ChildOf(e),
            ))
            .id();
        let lamp = commands
            .spawn((
                Mesh3d(s.lamp.clone()),
                MeshMaterial3d(lamp_mat.clone()),
                Transform::from_xyz(0.0, BARREL * 0.5 + 0.03, 0.0),
                NotShadowCaster,
                ChildOf(barrel),
            ))
            .id();
        commands.entity(e).insert(TubeLook {
            barrel,
            lamp,
            lamp_mat,
            barrel_mat,
            broken: false,
        });
    }
}

/// The barrel up by `raised`; the lamp ticking red over the tell; shot
/// (`hp` gone), the barrel bent over, scorched, its lamp out.
fn pose_tubes(
    mut tubes: Query<(&FlareTube, &mut TubeLook)>,
    robots: Query<&Distress>,
    mut places: Query<&mut Transform>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    for (tube, mut look) in &mut tubes {
        let shot = tube.hp <= 0.0;
        if shot && !look.broken {
            look.broken = true;
            if let Some(mut m) = materials.get_mut(&look.barrel_mat) {
                m.base_color = Color::srgb(0.06, 0.055, 0.05);
                m.perceptual_roughness = 0.95;
                m.metallic = 0.1;
            }
        }
        if let Ok(mut t) = places.get_mut(look.barrel) {
            let y = barrel_y(tube.raised);
            let want = if look.broken {
                // Split and bent over, back off the mount.
                Transform::from_xyz(0.0, y - 0.05, 0.06)
                    .with_rotation(Quat::from_rotation_x(0.75) * Quat::from_rotation_z(0.2))
            } else {
                Transform::from_xyz(0.0, y, 0.0)
            };
            if *t != want {
                *t = want;
            }
        }
        let telling = robots
            .get(tube.robot)
            .is_ok_and(|d| matches!(d, Distress::Telling(_)));
        let on = !look.broken && telling && (time.elapsed_secs() * TICK).fract() < 0.5;
        let want = if on {
            LinearRgba::rgb(4.0, 0.06, 0.03)
        } else {
            LinearRgba::BLACK
        };
        if let Some(m) = materials.get(&look.lamp_mat) {
            if m.emissive != want {
                if let Some(mut m) = materials.get_mut(&look.lamp_mat) {
                    m.emissive = want;
                }
            }
        }
        let _ = look.lamp;
    }
}

/// A canister: a bright red cylinder (its trail is `fx`'s).
fn dress_canisters(
    mut commands: Commands,
    cans: Query<Entity, Added<Canister>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &cans {
        let (mesh, mat) = look
            .get_or_insert_with(|| {
                (
                    meshes.add(Cylinder::new(0.05, 0.2)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.9, 0.08, 0.05),
                        emissive: LinearRgba::rgb(3.0, 0.06, 0.03),
                        fog_enabled: false,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
            NotShadowCaster,
            ChildOf(e),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stowed, the barrel sits in its housing; raised, its top meets the
    /// top of the tube's collider: the part you shoot is the part you see.
    #[test]
    fn the_barrel_rises_to_the_top_of_its_collider() {
        let top = |r: f32| barrel_y(r) + BARREL * 0.5;
        let housing_top = -TUBE_SIZE.y * 0.5 + HOUSING;
        assert!(top(0.0) <= housing_top + 0.05, "stowed top {:.2}", top(0.0));
        assert!(
            (top(1.0) - TUBE_SIZE.y * 0.5).abs() < 1e-4,
            "raised top {:.2}",
            top(1.0)
        );
        assert!(barrel_y(0.5) > barrel_y(0.0) && barrel_y(0.5) < barrel_y(1.0));
    }
}
