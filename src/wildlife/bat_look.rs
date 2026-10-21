//! Bats, drawn (`bats`): one entity a bat, one mesh a pose
//! (`art/animals/bat.glb`, merged and vertex-coloured like the birds), the
//! wings up and down in turn about ten times a second, banking into its
//! swerves. Past [`FAR`] a bat is its far stand-in, past [`HIDE`] it isn't
//! drawn: at dusk that's a flicker against the sky, if that. No shadows.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::bats::Colony;
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::lod::StandIn;

/// Beyond this, a bat is its far stand-in (m).
pub const FAR: f32 = 25.0;
/// Beyond this it isn't drawn at all.
pub const HIDE: f32 = 60.0;
/// Drawn a little over life size: a noctule's 0.36 m is hard to see in
/// the dusk at any distance.
pub const SIZE: f32 = 1.3;

const MODEL: &str = "models/animals/bat.glb";

#[derive(Resource, Default)]
struct BatArt {
    /// Up, down, far.
    poses: Option<[Handle<Mesh>; 3]>,
    material: Option<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct BatVisual {
    colony: Entity,
    index: usize,
    heading: f32,
    roll: f32,
}

#[derive(Component)]
struct Drawn;

pub struct BatLookPlugin;

impl Plugin for BatLookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BatArt>()
            .add_systems(Update, (load, draw, pose).chain());
    }
}

fn keep(mut m: Mesh) -> Mesh {
    m.asset_usage = bevy::asset::RenderAssetUsages::default();
    m
}

/// The fur dark brown, the wings near black.
fn colour(material: Option<&str>) -> LinearRgba {
    match material {
        Some("Hide") => LinearRgba::rgb(0.05, 0.035, 0.025),
        _ => LinearRgba::rgb(0.02, 0.018, 0.017),
    }
}

fn merged(tree: &[ModelNode], name: &str, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
    let node = ModelNode::find(tree, name)?;
    let mut s = StandIn::default();
    for p in &node.primitives {
        let mesh = meshes.get(&p.mesh)?;
        s.add(mesh, colour(p.material.as_deref()));
    }
    s.build().map(|m| meshes.add(keep(m)))
}

/// Without the model: a dark body and a bar of wing, raised or lowered.
fn blockout(meshes: &mut Assets<Mesh>) -> [Handle<Mesh>; 3] {
    let mut pose = |lift: f32| {
        let mut s = StandIn::default();
        s.add(
            &Mesh::from(Cuboid::new(0.035, 0.03, 0.08)),
            colour(Some("Hide")),
        );
        for side in [-1.0f32, 1.0] {
            let wing = Mesh::from(Cuboid::new(0.17, 0.005, 0.06))
                .translated_by(Vec3::X * side * 0.09)
                .rotated_by(Quat::from_rotation_z(side * lift));
            s.add(&wing, colour(None));
        }
        meshes.add(keep(s.build().unwrap()))
    };
    [pose(0.9), pose(-0.7), pose(0.15)]
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut art: ResMut<BatArt>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if art.poses.is_some() {
        return;
    }
    if art.material.is_none() {
        art.material = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.7,
            reflectance: 0.3,
            // The wings are thin slabs seen from both sides.
            double_sided: true,
            cull_mode: None,
            ..default()
        }));
    }
    let Some(handle) = models.request(MODEL, &server) else {
        art.poses = Some(blockout(&mut meshes));
        return;
    };
    match Models::state(&handle, &server, &gltfs) {
        ModelState::Loading => {}
        ModelState::Failed => art.poses = Some(blockout(&mut meshes)),
        ModelState::Ready(gltf) => {
            let tree = model_nodes(gltf, &nodes, &gltf_meshes);
            let mut pose = |n: &str| merged(&tree, n, &mut meshes);
            art.poses = Some(match (pose("up"), pose("down"), pose("far")) {
                (Some(u), Some(d), Some(f)) => [u, d, f],
                _ => {
                    warn!("{MODEL}: a pose is missing; drawing blockout bats");
                    blockout(&mut meshes)
                }
            });
        }
    }
}

fn draw(
    mut commands: Commands,
    art: Res<BatArt>,
    colonies: Query<(Entity, &Colony), Without<Drawn>>,
) {
    let (Some(poses), Some(mat)) = (&art.poses, &art.material) else {
        return;
    };
    for (e, c) in &colonies {
        for (i, b) in c.bats.iter().enumerate() {
            // Children of the group: they go when it's dropped.
            commands.spawn((
                Name::new("Bat"),
                BatVisual {
                    colony: e,
                    index: i,
                    heading: 0.0,
                    roll: 0.0,
                },
                Mesh3d(poses[0].clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(b.pos).with_scale(Vec3::splat(SIZE)),
                Visibility::default(),
                NotShadowCaster,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: (HIDE - 5.0)..HIDE,
                    use_aabb: false,
                },
                ChildOf(e),
            ));
        }
        commands.entity(e).insert(Drawn);
    }
}

fn pose(
    art: Res<BatArt>,
    colonies: Query<&Colony>,
    mut bats: Query<(&mut BatVisual, &mut Transform, &mut Mesh3d, &mut Visibility)>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
) {
    let Some(poses) = &art.poses else {
        return;
    };
    let dt = time.delta_secs();
    let eyes: Vec<Vec3> = cams.iter().map(|g| g.translation()).collect();
    for (mut v, mut tf, mut mesh, mut vis) in &mut bats {
        let Ok(c) = colonies.get(v.colony) else {
            continue;
        };
        let (Some(b), true) = (c.bats.get(v.index), c.flying()) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(Visibility::Inherited);
        let heading = b.vel.z.atan2(b.vel.x);
        if dt > 0.0 {
            let turn = crate::robots::anim::wrap_angle(heading - v.heading) / dt;
            let bank = (turn * 0.12).clamp(-1.0, 1.0);
            v.roll += (bank - v.roll) * (dt * 8.0).min(1.0);
        }
        v.heading = heading;
        tf.translation = b.pos;
        if b.vel.length_squared() > 0.01 {
            tf.rotation = Transform::IDENTITY.looking_to(b.vel, Vec3::Y).rotation
                * Quat::from_rotation_z(-v.roll);
        }
        let far = !eyes.is_empty() && super::visuals::nearest(&eyes, b.pos) > FAR;
        let want = if far {
            &poses[2]
        } else if b.phase < 0.5 {
            &poses[0]
        } else {
            &poses[1]
        };
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}
