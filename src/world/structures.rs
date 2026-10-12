//! Placed structures: buildings, props, robot sites.
//!
//! Until the building generator and the modelled prefabs land, each
//! placement becomes a correctly-sized blockout so the map can be walked
//! and judged for scale.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Layer, Zone};

/// Rough footprint (width x depth x height) of each prefab, metres.
pub fn footprint(prefab: &str) -> Vec3 {
    match prefab {
        "quarantine_gate" => Vec3::new(14.0, 4.0, 0.6),
        "sawmill" => Vec3::new(24.0, 7.0, 12.0),
        "cabin_log" => Vec3::new(7.0, 4.5, 6.0),
        "log_pile" => Vec3::new(8.0, 1.6, 3.0),
        "farmhouse" => Vec3::new(10.0, 7.0, 8.0),
        "barn" => Vec3::new(16.0, 9.0, 12.0),
        "shed" => Vec3::new(4.0, 3.0, 3.0),
        "house_plank" => Vec3::new(8.0, 6.0, 7.0),
        "house_brick" => Vec3::new(10.0, 7.5, 8.0),
        "shop" => Vec3::new(9.0, 5.0, 7.0),
        "pump_station" => Vec3::new(14.0, 7.0, 10.0),
        "footbridge" => Vec3::new(1.6, 1.2, 24.0),
        "anomaly_core" => Vec3::new(3.0, 3.0, 3.0),
        "bus_depot" => Vec3::new(30.0, 8.0, 18.0),
        "radio_mast" => Vec3::new(2.0, 45.0, 2.0),
        "mast_hut" => Vec3::new(5.0, 3.0, 4.0),
        "rail_shed" => Vec3::new(20.0, 9.0, 12.0),
        "boxcar" => Vec3::new(3.0, 4.2, 14.0),
        "ranger_tower" => Vec3::new(4.0, 14.0, 4.0),
        "robot_gate" => Vec3::new(18.0, 12.0, 6.0),
        _ => Vec3::new(4.0, 3.0, 4.0),
    }
}

/// Radius around a placement kept free of trees.
pub fn clearance(prefab: &str) -> f32 {
    let f = footprint(prefab);
    (f.x.max(f.z) * 0.5 + 4.0).max(6.0)
}

#[derive(Component)]
pub struct Structure {
    pub prefab: String,
}

pub fn spawn_structures(mut commands: Commands, zone: Res<Zone>) {
    for p in &zone.map.placements {
        let size = footprint(&p.prefab);
        let ground = zone.ground(p.at[0], p.at[1]);
        let y = if p.absolute { p.y } else { ground + p.y };
        let rot = Quat::from_rotation_y(-p.rot.to_radians());
        commands.spawn((
            Name::new(format!("Structure {}", p.prefab)),
            Structure {
                prefab: p.prefab.clone(),
            },
            Transform::from_xyz(p.at[0], y + size.y * 0.5, p.at[1]).with_rotation(rot),
            RigidBody::Static,
            Collider::cuboid(size.x, size.y, size.z),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
}

/// Blockout visuals: grey boxes sized to the footprint, raised so their
/// base sits on the ground.
pub fn blockout_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Structure), Added<Structure>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: Local<Option<Handle<StandardMaterial>>>,
) {
    let mat = cache
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.42, 0.40, 0.38),
                perceptual_roughness: 0.9,
                ..default()
            })
        })
        .clone();
    for (e, s) in &added {
        let size = footprint(&s.prefab);
        commands
            .entity(e)
            .insert((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(mat.clone())));
    }
}
