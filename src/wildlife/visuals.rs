//! What animals look like: the client's half of `wildlife`.
//!
//! Birds are one entity each, one mesh a pose (`art/animals/rook.py`: the
//! glTF's primitives merged into one vertex-coloured mesh, so a bird is a
//! single draw) and one material a species: every bird in the same pose
//! batches with the others. The wingbeat is the poses in turn (up, mid,
//! down, mid) at a rate that follows how hard it's working
//! ([`super::flock::Bird::flap`]), gliding between bursts; turning, it banks.
//! Past [`FAR`] a bird is its far stand-in (a few quads), and past
//! [`HIDE`] it isn't drawn: in the valleys' air that's a speck, if that.
//! Birds don't cast shadows: tiny, high and fast, nobody would see them.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::HashMap;

use super::flock::{BirdState, Flock, Species};
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::lod::StandIn;

/// Beyond this, a bird is its far stand-in (m).
pub const FAR: f32 = 45.0;
/// Beyond this it isn't drawn at all.
pub const HIDE: f32 = 380.0;
/// Beyond this from a flock's middle, the whole flock is one mesh (one
/// draw: the budget counts meshes), rebuilt from its birds' far poses.
pub const FLOCK_FAR: f32 = 60.0;
/// Wingbeats a second, easy and flat out.
pub const BEAT_EASY: f32 = 3.2;
pub const BEAT_HARD: f32 = 6.0;

const MODEL: &str = "models/animals/rook.glb";

/// A bird's poses, one mesh each.
#[derive(Clone)]
pub struct BirdPoses {
    pub sit: Handle<Mesh>,
    pub sit_far: Handle<Mesh>,
    /// The wingbeat, top of the stroke to the bottom.
    pub beat: [Handle<Mesh>; 3],
    pub glide: Handle<Mesh>,
    pub fly_far: Handle<Mesh>,
}

#[derive(Resource, Default)]
pub struct BirdArt {
    pub poses: Option<BirdPoses>,
    materials: HashMap<&'static str, Handle<StandardMaterial>>,
}

/// One bird of a flock, drawn.
#[derive(Component, Debug)]
pub struct BirdVisual {
    pub flock: Entity,
    pub index: usize,
    /// Through the wingbeat, in beats.
    phase: f32,
    /// Last tick's heading, for banking into turns.
    heading: f32,
    roll: f32,
}

/// This flock's birds have been spawned.
#[derive(Component)]
struct Drawn;

/// Everything of the animals' that's far off this frame, to be drawn as
/// one mesh ([`build_far`]): whole flocks past [`FLOCK_FAR`], deer and dogs
/// past `herd_look::FAR`. The budget counts meshes; this is one.
#[derive(Resource, Default)]
pub struct FarParts {
    /// A mesh, where, and a tint for its vertex colours.
    pub parts: Vec<(Handle<Mesh>, Transform, LinearRgba)>,
    /// The last build's parts, to skip rebuilding a still scene.
    built: Vec<(AssetId<Mesh>, [f32; 4])>,
    entity: Option<(Entity, Handle<Mesh>)>,
}

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BirdArt>()
            .init_resource::<FarParts>()
            .add_systems(
                Update,
                (load_birds, draw_flocks, pose_birds, far_flocks)
                    .chain()
                    .before(build_far),
            )
            .add_systems(Update, build_far);
    }
}

/// The poses from the model once it's in; blockouts if it never comes.
fn load_birds(
    mut art: ResMut<BirdArt>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if art.poses.is_some() {
        return;
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
            let mut pose = |name: &str| merged(&tree, name, &mut meshes);
            let poses = (|| {
                Some(BirdPoses {
                    sit: pose("sit")?,
                    sit_far: pose("sit_lod1")?,
                    beat: [pose("up")?, pose("mid")?, pose("down")?],
                    glide: pose("glide")?,
                    fly_far: pose("fly_lod1")?,
                })
            })();
            art.poses = Some(poses.unwrap_or_else(|| {
                warn!("{MODEL}: a pose is missing; drawing blockout birds");
                blockout(&mut meshes)
            }));
        }
    }
}

/// A node's primitives as one mesh, each painted its material's colour
/// (the bird's material is white: the colours are the colour).
fn merged(tree: &[ModelNode], name: &str, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
    let node = ModelNode::find(tree, name)?;
    let mut s = StandIn::default();
    for p in &node.primitives {
        let mesh = meshes.get(&p.mesh)?;
        let c = match p.material.as_deref() {
            Some("Beak") => LinearRgba::rgb(0.13, 0.13, 0.135),
            _ => LinearRgba::rgb(0.034, 0.034, 0.04),
        };
        s.add(mesh, c);
    }
    s.build().map(|m| meshes.add(keep(m)))
}

/// Kept in the main world too: birds swap poses all the time, and a
/// mesh's bounds are worked out there.
fn keep(mut m: Mesh) -> Mesh {
    m.asset_usage = bevy::asset::RenderAssetUsages::default();
    m
}

/// Without the model: a dark lozenge sitting, a dark cross flying.
fn blockout(meshes: &mut Assets<Mesh>) -> BirdPoses {
    let mut paint = |m: Mesh| {
        let mut s = StandIn::default();
        s.add(&m, LinearRgba::rgb(0.034, 0.034, 0.04));
        meshes.add(keep(s.build().unwrap()))
    };
    let body = || Mesh::from(Cuboid::new(0.1, 0.1, 0.3)).translated_by(Vec3::Y * 0.13);
    let sit = paint(body());
    let mut wings = body();
    wings
        .merge(&Mesh::from(Cuboid::new(0.85, 0.01, 0.12)).translated_by(Vec3::Y * 0.15))
        .ok();
    let fly = paint(wings);
    BirdPoses {
        sit: sit.clone(),
        sit_far: sit,
        beat: [fly.clone(), fly.clone(), fly.clone()],
        glide: fly.clone(),
        fly_far: fly,
    }
}

/// A species' tint on the rook's colours: a raven's plumage glossier and
/// blacker, a jackdaw's greyer.
fn tint(species: Species) -> Color {
    match species {
        Species::Rook => Color::WHITE,
        Species::Raven => Color::srgb(0.8, 0.8, 0.85),
        Species::Jackdaw => Color::srgb(1.9, 1.9, 1.95),
    }
}

fn material(
    art: &mut BirdArt,
    species: Species,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let key = match species {
        Species::Rook => "rook",
        Species::Raven => "raven",
        Species::Jackdaw => "jackdaw",
    };
    let tint = tint(species);
    art.materials
        .entry(key)
        .or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: tint,
                perceptual_roughness: if species == Species::Raven { 0.35 } else { 0.5 },
                reflectance: 0.4,
                // The wings are single slabs seen from both sides.
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        })
        .clone()
}

/// A flock's birds, once the poses are ready.
fn draw_flocks(
    mut commands: Commands,
    mut art: ResMut<BirdArt>,
    flocks: Query<(Entity, &Flock), Without<Drawn>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(poses) = art.poses.clone() else {
        return;
    };
    for (e, flock) in &flocks {
        let mat = material(&mut art, flock.species, &mut materials);
        for (i, b) in flock.birds.iter().enumerate() {
            commands.spawn((
                Name::new("Bird"),
                BirdVisual {
                    flock: e,
                    index: i,
                    phase: (i as f32 * 0.37).fract(),
                    heading: 0.0,
                    roll: 0.0,
                },
                Mesh3d(poses.sit.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(b.pos).with_scale(Vec3::splat(flock.species.size())),
                Visibility::default(),
                NotShadowCaster,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: (HIDE - 40.0)..HIDE,
                    use_aabb: false,
                },
            ));
        }
        commands.entity(e).insert(Drawn);
        debug!("drew a flock of {}", flock.birds.len());
    }
}

/// Each bird where the flock's logic has it, in its pose for the moment.
fn pose_birds(
    art: Res<BirdArt>,
    flocks: Query<&Flock>,
    mut birds: Query<(
        &mut BirdVisual,
        &mut Transform,
        &mut Mesh3d,
        &mut Visibility,
    )>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
) {
    let Some(poses) = &art.poses else {
        return;
    };
    let dt = time.delta_secs();
    let eye = cams.iter().next().map(|g| g.translation());
    for (mut v, mut tf, mut mesh, mut vis) in &mut birds {
        let Ok(flock) = flocks.get(v.flock) else {
            continue;
        };
        let Some(b) = flock.birds.get(v.index) else {
            continue;
        };
        // The whole flock far off: drawn in the one far mesh (`far_flocks`).
        let whole = eye.is_some_and(|e| e.distance(flock.mean()) > FLOCK_FAR);
        vis.set_if_neq(if whole {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        if whole {
            continue;
        }
        let far = eye.is_some_and(|e| e.distance(b.pos) > FAR);
        tf.translation = b.pos;
        let want = match b.state {
            BirdState::Sitting(_) => {
                tf.rotation = Quat::from_rotation_y(-b.yaw - std::f32::consts::FRAC_PI_2);
                v.roll = 0.0;
                if far { &poses.sit_far } else { &poses.sit }
            }
            BirdState::Flying | BirdState::Landing(_) => {
                let heading = b.vel.z.atan2(b.vel.x);
                if dt > 0.0 {
                    let turn = crate::robots::anim::wrap_angle(heading - v.heading) / dt;
                    let bank = (turn * 0.35).clamp(-0.9, 0.9);
                    v.roll += (bank - v.roll) * (dt * 4.0).min(1.0);
                }
                v.heading = heading;
                if b.vel.length_squared() > 0.01 {
                    tf.rotation = Transform::IDENTITY.looking_to(b.vel, Vec3::Y).rotation
                        * Quat::from_rotation_z(-v.roll);
                }
                if b.flap < 0.22 {
                    // Gliding; finish the beat to the glide.
                    v.phase = 0.0;
                    if far { &poses.fly_far } else { &poses.glide }
                } else {
                    let rate = BEAT_EASY + (BEAT_HARD - BEAT_EASY) * b.flap;
                    v.phase = (v.phase + rate * dt).fract();
                    if far {
                        &poses.fly_far
                    } else {
                        // Up, mid, down, mid.
                        let k = (v.phase * 4.0) as usize;
                        &poses.beat[[0, 1, 2, 1][k.min(3)]]
                    }
                }
            }
        };
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}

/// A bird's far pose and turn (no banking: from far off nobody sees it).
fn far_pose(b: &super::flock::Bird) -> (bool, Quat) {
    match b.state {
        BirdState::Sitting(_) => (
            true,
            Quat::from_rotation_y(-b.yaw - std::f32::consts::FRAC_PI_2),
        ),
        _ if b.vel.length_squared() > 0.01 => (
            false,
            Transform::IDENTITY.looking_to(b.vel, Vec3::Y).rotation,
        ),
        _ => (false, Quat::IDENTITY),
    }
}

/// Far-off flocks: their birds' far poses, for the one far mesh.
fn far_flocks(
    art: Res<BirdArt>,
    flocks: Query<&Flock>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut far: ResMut<FarParts>,
) {
    let Some(poses) = &art.poses else {
        return;
    };
    let Some(eye) = cams.iter().next().map(|g| g.translation()) else {
        return;
    };
    for flock in &flocks {
        let d = eye.distance(flock.mean());
        if d <= FLOCK_FAR || d > HIDE {
            continue;
        }
        let scale = Vec3::splat(flock.species.size());
        let tint = tint(flock.species).to_linear();
        for b in &flock.birds {
            let (sitting, rot) = far_pose(b);
            let at = Transform::from_translation(b.pos)
                .with_rotation(rot)
                .with_scale(scale);
            let mesh = if sitting {
                &poses.sit_far
            } else {
                &poses.fly_far
            };
            far.parts.push((mesh.clone(), at, tint));
        }
    }
}

/// The animals far off, as one mesh, rebuilt when anything in it moved.
pub(crate) fn build_far(
    mut commands: Commands,
    mut far: ResMut<FarParts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut vis: Query<&mut Visibility>,
) {
    let far = &mut *far;
    let sig: Vec<(AssetId<Mesh>, [f32; 4])> = far
        .parts
        .iter()
        .map(|(m, t, _)| {
            let p = t.translation;
            (m.id(), [p.x, p.y, p.z, t.rotation.y])
        })
        .collect();
    let parts = std::mem::take(&mut far.parts);
    let (e, handle) = far.entity.get_or_insert_with(|| {
        let mesh = meshes.add(keep(Mesh::from(Cuboid::new(0.01, 0.01, 0.01))));
        let mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.6,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let e = commands
            .spawn((
                Name::new("Animals far off"),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::IDENTITY,
                Visibility::Hidden,
                NotShadowCaster,
                // Its mesh changes under it; its bounds would be the first one's.
                bevy::camera::visibility::NoFrustumCulling,
            ))
            .id();
        (e, mesh)
    });
    if let Ok(mut v) = vis.get_mut(*e) {
        v.set_if_neq(if parts.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    if parts.is_empty() || sig == far.built {
        return;
    }
    let mut built = StandIn::default();
    for (m, at, tint) in &parts {
        if let Some(mesh) = meshes.get(m) {
            built.add(&mesh.clone().transformed_by(*at), *tint);
        }
    }
    if let (Some(m), Some(mut mesh)) = (built.build(), meshes.get_mut(&*handle)) {
        *mesh = keep(m);
    }
    far.built = sig;
}
