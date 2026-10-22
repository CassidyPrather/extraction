//! Rendezvous beacons: a steel pole with a green lamp flashing its number
//! (`map_landmarks::beacon_character`: the map's first rendezvous once,
//! its second twice...) beside where the truck parks at each
//! (`map_landmarks::beacon_at`), so the way out can be found, and told
//! which, from the valley side before the truck's there, in drizzle and
//! after dark. `map_lint` holds each in sight from most of the
//! way in. Green: the landmarks' lamps are red (`landmarks`), the drones'
//! beacons amber and blinking.

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::map_landmarks::{BEACON_HEIGHT, beacon_at, beacon_character};

use super::landmarks::{DayGlow, Lamp, falloff};
use crate::world::Zone;
use crate::world::halos::{Halo, HaloReach};

pub struct RvBeaconsPlugin;

impl Plugin for RvBeaconsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_beacons);
    }
}

/// The lamp's radius, its halo's and its day glow's (m).
const LAMP_R: f32 = 0.3;
const HALO_R: f32 = 1.0;
const DAY_R: f32 = 1.4;
/// The halo's colour (linear) and how far off it still shows (m): further
/// than a landmark's, the way out being the thing you look for.
const HALO: LinearRgba = LinearRgba::rgb(0.1, 1.7, 0.35);
const HALO_REACH: f32 = 400.0;
/// The pole's radius (m).
const POLE_R: f32 = 0.08;

/// One beacon.
#[derive(Component)]
pub struct RvBeacon;

fn spawn_beacons(
    mut commands: Commands,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let pole_h = BEACON_HEIGHT - LAMP_R;
    let pole_mesh = meshes.add(Cylinder::new(POLE_R, pole_h).mesh().resolution(10));
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.18),
        perceptual_roughness: 0.6,
        metallic: 0.7,
        ..default()
    });
    // A green-painted plate under the lamp, the colour of the way out by day.
    let plate_mesh = meshes.add(Cuboid::new(0.6, 0.45, 0.04));
    let plate = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.42, 0.16),
        perceptual_roughness: 0.7,
        ..default()
    });
    let lamp_mesh = meshes.add(Sphere::new(LAMP_R).mesh().uv(12, 8));
    let lamp = materials.add(StandardMaterial {
        base_color: Color::srgb(0.02, 0.5, 0.1),
        emissive: LinearRgba::rgb(4.0, 110.0, 20.0),
        ..default()
    });
    let glow_mesh = meshes.add(Rectangle::new(DAY_R * 2.0, DAY_R * 2.0));
    let glow = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(0.15, 3.0, 0.6)),
        base_color_texture: Some(images.add(falloff())),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        fog_enabled: false,
        ..default()
    });
    for (i, rv) in zone.map.markers_of(&MarkerKind::Rendezvous).enumerate() {
        let at = beacon_at(rv.at);
        let foot = zone.on_ground(at[0], at[1]);
        // The plate faces the valley's middle, where you come from.
        let face = Vec3::new(-at[0], 0.0, -at[1]).normalize_or(Vec3::Z);
        let yaw = face.x.atan2(face.z);
        commands
            .spawn((
                Name::new(format!("Rendezvous beacon: {}", rv.name)),
                RvBeacon,
                Transform::from_translation(foot).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .with_children(|c| {
                c.spawn((
                    Name::new("Beacon pole"),
                    Mesh3d(pole_mesh.clone()),
                    MeshMaterial3d(steel.clone()),
                    Transform::from_xyz(0.0, pole_h * 0.5, 0.0),
                    RigidBody::Static,
                    Collider::cylinder(POLE_R, pole_h),
                ));
                c.spawn((
                    Name::new("Beacon plate"),
                    Mesh3d(plate_mesh.clone()),
                    MeshMaterial3d(plate.clone()),
                    Transform::from_xyz(0.0, pole_h - 0.6, POLE_R + 0.03),
                ));
                c.spawn((
                    Name::new("Beacon lamp"),
                    // Green, flashing its number: which way out it is.
                    Lamp(beacon_character(i)),
                    Visibility::default(),
                    Mesh3d(lamp_mesh.clone()),
                    MeshMaterial3d(lamp.clone()),
                    Transform::from_xyz(0.0, pole_h + LAMP_R, 0.0),
                    NotShadowCaster,
                    Halo {
                        color: HALO,
                        radius: HALO_R,
                    },
                    HaloReach(HALO_REACH),
                ))
                .with_child((
                    Name::new("Beacon lamp glow"),
                    DayGlow,
                    Mesh3d(glow_mesh.clone()),
                    MeshMaterial3d(glow.clone()),
                    Transform::default(),
                    NotShadowCaster,
                ));
            });
    }
}

#[cfg(test)]
mod tests {
    use extraction_core::map::{MapDef, MarkerKind};
    use extraction_core::map_landmarks::beacon_at;

    /// Every rendezvous in every valley has its beacon on open ground: not
    /// in water, not on the road the truck comes in by, not in a building.
    #[test]
    fn every_beacon_stands_clear() {
        for (id, text) in extraction_core::map::VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = extraction_core::terrain::Heightmap::build(&map);
            for rv in map.markers_of(&MarkerKind::Rendezvous) {
                let b = beacon_at(rv.at);
                assert!(
                    hm.water_level(b).is_none(),
                    "{id} {}: its beacon in water",
                    rv.name
                );
                assert!(
                    hm.slope_deg(b) < 30.0,
                    "{id} {}: its beacon on a {:.0} degree slope",
                    rv.name,
                    hm.slope_deg(b)
                );
                for p in &map.placements {
                    let f = extraction_core::map::prefab_footprint(&p.prefab);
                    let (s, c) = p.rot.to_radians().sin_cos();
                    let (dx, dz) = (b[0] - p.at[0], b[1] - p.at[1]);
                    let (lx, lz) = (dx * c + dz * s, -dx * s + dz * c);
                    assert!(
                        lx.abs() > f[0] * 0.5 + 0.5 || lz.abs() > f[2] * 0.5 + 0.5,
                        "{id} {}: its beacon inside {}",
                        rv.name,
                        p.prefab
                    );
                }
            }
        }
    }
}
