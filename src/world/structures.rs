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
    Vec3::from_array(extraction_core::map::prefab_footprint(prefab))
}

#[derive(Component)]
pub struct Structure {
    pub prefab: String,
}

pub fn spawn_structures(mut commands: Commands, zone: Res<Zone>) {
    for p in &zone.map.placements {
        // Generated buildings handle themselves; the rest are blockouts.
        if super::buildings::prefab(&p.prefab).is_some() {
            continue;
        }
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
        commands.entity(e).insert((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(mat.clone()),
        ));
    }
}

/// A run of quarantine fence between two points (map space).
#[derive(Clone, Copy, Debug)]
pub struct FenceRun {
    pub a: Vec2,
    pub b: Vec2,
}

/// The fence: a square `fence_inset` in from the edge, broken where roads
/// cross it (those are the gates).
pub fn fence_runs(zone: &Zone) -> Vec<FenceRun> {
    let inset = zone.map.fence_inset;
    if inset <= 0.0 {
        return Vec::new();
    }
    let h = zone.map.half() - inset;
    let corners = [
        Vec2::new(-h, -h),
        Vec2::new(h, -h),
        Vec2::new(h, h),
        Vec2::new(-h, h),
    ];
    let step = 3.0;
    let mut runs = Vec::new();
    for side in 0..4 {
        let (a, b) = (corners[side], corners[(side + 1) % 4]);
        let len = a.distance(b);
        let n = (len / step).ceil() as usize;
        let mut start: Option<Vec2> = None;
        for k in 0..=n {
            let p = a.lerp(b, k as f32 / n as f32);
            let gap = zone.hm.road_lines.iter().any(|rl| {
                extraction_core::geom::polyline_closest(&rl.points, [p.x, p.y]).0
                    < rl.width * 0.5 + 3.0
            });
            match (gap, start) {
                (false, None) => start = Some(p),
                (true, Some(s)) => {
                    runs.push(FenceRun { a: s, b: p });
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            runs.push(FenceRun { a: s, b });
        }
    }
    runs
}

#[derive(Component)]
pub struct Fence;

/// Logic: fence colliders, plus invisible walls at the very edge of the map.
pub fn spawn_boundary(mut commands: Commands, zone: Res<Zone>) {
    for run in fence_runs(&zone) {
        let mid = (run.a + run.b) * 0.5;
        let len = run.a.distance(run.b);
        if len < 0.5 {
            continue;
        }
        let dir = (run.b - run.a) / len;
        let y = zone.ground(mid.x, mid.y);
        commands.spawn((
            Name::new("Fence"),
            Fence,
            Transform::from_xyz(mid.x, y + 1.5, mid.y)
                .with_rotation(Quat::from_rotation_y(-dir.y.atan2(dir.x))),
            RigidBody::Static,
            Collider::cuboid(len, 4.0, 0.25),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
    let half = zone.map.half();
    for (x, z, sx, sz) in [
        (0.0, -half, 2.0 * half, 1.0),
        (0.0, half, 2.0 * half, 1.0),
        (-half, 0.0, 1.0, 2.0 * half),
        (half, 0.0, 1.0, 2.0 * half),
    ] {
        commands.spawn((
            Name::new("World edge"),
            Transform::from_xyz(x, 50.0, z),
            RigidBody::Static,
            Collider::cuboid(sx, 200.0, sz),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
}

/// Client: posts, wire and warning plates along each fence run.
pub fn fence_visuals(
    mut commands: Commands,
    added: Query<(), Added<Fence>>,
    zone: Res<Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut done: Local<bool>,
) {
    if *done || added.is_empty() {
        return;
    }
    *done = true;
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.17, 0.16),
        metallic: 0.8,
        perceptual_roughness: 0.6,
        ..default()
    });
    let sign = materials.add(StandardMaterial {
        base_color: Color::srgb(0.65, 0.55, 0.12),
        perceptual_roughness: 0.7,
        ..default()
    });
    let post = meshes.add(Cuboid::new(0.08, 2.8, 0.08));
    let wire = meshes.add(Cuboid::new(1.0, 0.012, 0.012));
    let plate = meshes.add(Cuboid::new(0.5, 0.35, 0.02));
    let mut k = 0usize;
    for run in fence_runs(&zone) {
        let len = run.a.distance(run.b);
        let n = (len / 3.0).ceil().max(1.0) as usize;
        let along = (run.b - run.a).normalize_or(Vec2::X);
        let yaw = Quat::from_rotation_y(-along.y.atan2(along.x));
        for i in 0..=n {
            let p = run.a.lerp(run.b, i as f32 / n as f32);
            let y = zone.ground(p.x, p.y);
            // A steel stake until the concrete post's model is in.
            commands
                .spawn((
                    Transform::from_xyz(p.x, y, p.y).with_rotation(yaw),
                    Visibility::default(),
                    crate::render::world_props::SwapIn::one("fence_post"),
                ))
                .with_child((
                    Mesh3d(post.clone()),
                    MeshMaterial3d(metal.clone()),
                    Transform::from_xyz(0.0, 1.3, 0.0),
                    crate::render::world_props::Blockout,
                ));
            if i < n {
                let q = run.a.lerp(run.b, (i + 1) as f32 / n as f32);
                let yq = zone.ground(q.x, q.y);
                let span = Vec3::new(q.x - p.x, yq - y, q.y - p.y);
                let rot = Quat::from_rotation_arc(Vec3::X, span.normalize());
                for h in [0.5, 1.1, 1.7, 2.3, 2.65] {
                    let mid = Vec3::new(p.x, y + h, p.y) + span * 0.5;
                    commands.spawn((
                        Mesh3d(wire.clone()),
                        MeshMaterial3d(metal.clone()),
                        Transform::from_translation(mid)
                            .with_rotation(rot)
                            .with_scale(Vec3::new(span.length(), 1.0, 1.0)),
                    ));
                }
                k += 1;
                if k % 9 == 0 {
                    let mid = Vec3::new(p.x, y + 1.4, p.y) + span * 0.5;
                    commands.spawn((
                        Mesh3d(plate.clone()),
                        MeshMaterial3d(sign.clone()),
                        Transform::from_translation(mid).with_rotation(rot),
                    ));
                }
            }
        }
    }
}
