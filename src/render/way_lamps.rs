//! The yard lamps' light (`world::way_lamps`): a warm bulb under the hood,
//! a glow that carries a valley's width after dark, and a small pool of
//! light on the ground under it. Warm amber: the landmarks' lamps are red
//! and the way-out beacons green, so a yard lamp is neither.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::world::buildings::Building;
use crate::world::halos::{Halo, HaloReach};
use crate::world::way_lamps::BULB_AT;

pub struct WayLampsPlugin;

impl Plugin for WayLampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, light_way_lamps);
    }
}

/// The bulb as the tonemapper gets it (`render::grade::ToTheEye`), its
/// halo's colour and size, and how far off the halo shows (m).
const BULB: LinearRgba = LinearRgba::rgb(0.9, 0.55, 0.18);
const HALO: LinearRgba = LinearRgba::rgb(1.6, 0.8, 0.22);
const HALO_R: f32 = 0.8;
const HALO_REACH: f32 = 240.0;

fn light_way_lamps(
    mut commands: Commands,
    lamps: Query<(Entity, &Building), Added<Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for (e, b) in &lamps {
        if b.prefab != "way_lamp" {
            continue;
        }
        let (mesh, bulb) = made
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(0.08).mesh().uv(10, 6)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.5, 0.3, 0.1),
                        // Set each frame against the eyes' adaptation.
                        emissive: BULB,
                        ..default()
                    }),
                )
            })
            .clone();
        commands.entity(e).insert(crate::render::grade::ToTheEye {
            material: bulb.clone(),
            emissive: BULB,
        });
        commands.entity(e).with_child((
            Name::new("Way lamp bulb"),
            Mesh3d(mesh),
            MeshMaterial3d(bulb),
            Transform::from_translation(BULB_AT),
            NotShadowCaster,
            Halo {
                color: HALO,
                radius: HALO_R,
            },
            HaloReach(HALO_REACH),
            PointLight {
                color: Color::srgb(1.0, 0.75, 0.45),
                intensity: 40_000.0,
                range: 10.0,
                radius: 0.05,
                shadow_maps_enabled: false,
                ..default()
            },
        ));
    }
}
