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
    // Posts and plates merged a cell at a time, as the wire is: a post
    // was its own model entity every 3 m round the map (745 in Marrow
    // Creek, a third of the valley's static mesh entities), each culled,
    // bounded and specialised every frame (`budget::mesh_census`). A
    // steel stake per post until the concrete post's model is in
    // (`merge_fence_posts`).
    let stake = Mesh::from(Cuboid::new(0.08, 2.8, 0.08));
    let plate = Mesh::from(Cuboid::new(0.5, 0.35, 0.02));
    let mut wires: std::collections::HashMap<(i32, i32), Mesh> = Default::default();
    let mut posts: std::collections::HashMap<(i32, i32), Vec<Transform>> = Default::default();
    let mut plates: std::collections::HashMap<(i32, i32), Mesh> = Default::default();
    let cell_of = |x: f32, z: f32| {
        (
            (x / WIRE_CELL).floor() as i32,
            (z / WIRE_CELL).floor() as i32,
        )
    };
    let add = |into: &mut std::collections::HashMap<(i32, i32), Mesh>, cell, piece: Mesh| match into
        .get_mut(&cell)
    {
        Some(m) => {
            let _ = m.merge(&piece);
        }
        None => {
            into.insert(cell, piece);
        }
    };
    let mut k = 0usize;
    for run in fence_runs(&zone) {
        let len = run.a.distance(run.b);
        let n = (len / 3.0).ceil().max(1.0) as usize;
        let along = (run.b - run.a).normalize_or(Vec2::X);
        let yaw = Quat::from_rotation_y(-along.y.atan2(along.x));
        for i in 0..=n {
            let p = run.a.lerp(run.b, i as f32 / n as f32);
            let y = zone.ground(p.x, p.y);
            let cell = cell_of(p.x, p.y);
            posts
                .entry(cell)
                .or_default()
                .push(Transform::from_xyz(p.x, y, p.y).with_rotation(yaw));
            if i < n {
                let q = run.a.lerp(run.b, (i + 1) as f32 / n as f32);
                let yq = zone.ground(q.x, q.y);
                let span = Vec3::new(q.x - p.x, yq - y, q.y - p.y);
                let rot = Quat::from_rotation_arc(Vec3::X, span.normalize());
                for h in [0.5, 1.1, 1.7, 2.3, 2.65] {
                    let mid = Vec3::new(p.x, y + h, p.y) + span * 0.5;
                    let piece = Mesh::from(Cuboid::new(1.0, 0.012, 0.012)).transformed_by(
                        Transform::from_translation(mid)
                            .with_rotation(rot)
                            .with_scale(Vec3::new(span.length(), 1.0, 1.0)),
                    );
                    add(&mut wires, cell, piece);
                }
                k += 1;
                if k % 9 == 0 {
                    let mid = Vec3::new(p.x, y + 1.4, p.y) + span * 0.5;
                    add(
                        &mut plates,
                        cell,
                        plate
                            .clone()
                            .transformed_by(Transform::from_translation(mid).with_rotation(rot)),
                    );
                }
            }
        }
    }
    // A 12 mm strand is under a pixel past about 15 m: fade it out by the
    // time it would only shimmer (the posts carry the line further), and
    // cast no shadow (in overcast light, none you'd see).
    for (_, mesh) in wires {
        commands.spawn((
            Name::new("Fence wire"),
            crate::render::centred(&mut meshes, mesh),
            MeshMaterial3d(metal.clone()),
            bevy::light::NotShadowCaster,
            bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: (WIRE_FADE - 10.0)..(WIRE_FADE + 10.0),
                use_aabb: true,
            },
        ));
    }
    for (_, mesh) in plates {
        commands.spawn((
            Name::new("Fence plates"),
            crate::render::centred(&mut meshes, mesh),
            MeshMaterial3d(sign.clone()),
            post_range(),
        ));
    }
    let mut stakes = Vec::new();
    for (_, at) in &posts {
        let mut cell: Option<Mesh> = None;
        for t in at {
            let piece = stake
                .clone()
                .transformed_by(*t * Transform::from_xyz(0.0, 1.3, 0.0));
            match cell.as_mut() {
                Some(m) => {
                    let _ = m.merge(&piece);
                }
                None => cell = Some(piece),
            }
        }
        if let Some(mesh) = cell {
            stakes.push(
                commands
                    .spawn((
                        Name::new("Fence stakes"),
                        crate::render::centred(&mut meshes, mesh),
                        MeshMaterial3d(metal.clone()),
                        post_range(),
                    ))
                    .id(),
            );
        }
    }
    commands.insert_resource(FencePosts {
        posts,
        stakes,
        done: false,
    });
}

/// Every fence post's place, by wire cell, and the merged stakes standing
/// in for them until the post's model is in.
#[derive(Resource)]
pub struct FencePosts {
    posts: std::collections::HashMap<(i32, i32), Vec<Transform>>,
    stakes: Vec<Entity>,
    done: bool,
}

/// Client: once the concrete post's model has loaded, each cell's posts
/// become one mesh per material, and the stakes go. (A model that's
/// missing or broken leaves the stakes.)
pub fn merge_fence_posts(
    mut commands: Commands,
    posts: Option<ResMut<FencePosts>>,
    mut props: crate::render::world_props::PropModels,
    mut cache: ResMut<crate::render::world_props::PropMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut posts) = posts else {
        return;
    };
    if posts.done {
        return;
    }
    use crate::render::world_props::Resolved;
    match props.resolve("fence_post") {
        Resolved::Loading => return,
        Resolved::Missing => {
            posts.done = true;
            return;
        }
        Resolved::Ready => {}
    }
    let parts = cache.get("fence_post", &mut props, &meshes).to_vec();
    if parts.is_empty() {
        return;
    }
    for at in posts.posts.values() {
        for (mat, part) in &parts {
            let mut cell: Option<Mesh> = None;
            for t in at {
                let piece = part.clone().transformed_by(*t);
                match cell.as_mut() {
                    Some(m) => {
                        let _ = m.merge(&piece);
                    }
                    None => cell = Some(piece),
                }
            }
            if let Some(mesh) = cell {
                let material = props.material(mat, &mut materials);
                commands.spawn((
                    Name::new("Fence posts"),
                    crate::render::centred(&mut meshes, mesh),
                    MeshMaterial3d(material),
                    post_range(),
                ));
            }
        }
    }
    for e in std::mem::take(&mut posts.stakes) {
        commands.entity(e).try_despawn();
    }
    posts.done = true;
}

/// A post's or plate's fade, for a cell of them: `POST_REACH` from the
/// cell's nearest edge, as `LodReach` fades one.
fn post_range() -> bevy::camera::visibility::VisibilityRange {
    bevy::camera::visibility::VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: (POST_REACH.0 * 0.8)..POST_REACH.0,
        use_aabb: true,
    }
}

/// An 8 cm post is under a pixel wide past about 90 m (and crawls): it
/// fades out there, with the plates on the wire.
const POST_REACH: crate::render::budget::LodReach = crate::render::budget::LodReach(90.0);

/// Fence wire is merged in cells this wide (m) ...
const WIRE_CELL: f32 = 16.0;
/// ... and fades out round this far from a cell's centre.
const WIRE_FADE: f32 = 40.0;

#[cfg(test)]
mod fence_where {
    #[test]
    #[ignore = "prints where Marrow Creek's first fence run is"]
    fn fence_where() {
        let zone = super::Zone::load("marrow_creek");
        let r = &super::fence_runs(&zone)[0];
        println!(
            "FENCE {:?} {:?} ground {}",
            r.a,
            r.b,
            zone.ground(r.a.x, r.a.y)
        );
    }
}
