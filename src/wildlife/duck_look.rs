//! Ducks, drawn: the client's half of [`super::ducks`].
//!
//! Like the birds (`visuals`): one entity a duck, one mesh a pose
//! (`art/animals/duck.py`, its primitives merged into one vertex-coloured
//! mesh), drakes and hens painted from the same model, one white material
//! for all of them, no shadows. Swimming they bob a little; dabbling it's
//! tail up; in the run-up and the skid the pose carries its own splash.
//! Past [`FAR`] a duck is its far stand-in, and a whole raft past
//! [`RAFT_FAR`] goes into the animals' one far mesh (`visuals::FarParts`).

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::ducks::{DuckState, Raft};
use super::visuals::{FarParts, HIDE, nearest};
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::lod::StandIn;

/// Beyond this a duck is its far stand-in (m)...
pub const FAR: f32 = 40.0;
/// ...and beyond this from a raft's middle, the raft is in the far mesh.
pub const RAFT_FAR: f32 = 60.0;
/// Wingbeats a second: quick, a duck's.
pub const BEAT: f32 = 6.5;

const MODEL: &str = "models/animals/duck.glb";

/// A duck's colours by material, a drake's or a hen's.
fn colour(drake: bool, material: Option<&str>) -> LinearRgba {
    let c = |r, g, b| LinearRgba::rgb(r, g, b);
    match (drake, material.unwrap_or("Pale")) {
        (true, "Drake") => c(0.012, 0.075, 0.03),
        (true, "Hide") => c(0.14, 0.045, 0.02),
        (true, "Pale") => c(0.36, 0.35, 0.33),
        (true, "Feather") => c(0.13, 0.115, 0.095),
        (true, "Hoof") => c(0.012, 0.012, 0.014),
        (true, "Beak") => c(0.45, 0.36, 0.04),
        // The hen: mottled browns all over, an orange-brown bill.
        (false, "Drake") => c(0.1, 0.068, 0.04),
        (false, "Hide") => c(0.13, 0.085, 0.045),
        (false, "Pale") => c(0.16, 0.11, 0.06),
        (false, "Feather") => c(0.085, 0.06, 0.035),
        (false, "Hoof") => c(0.1, 0.07, 0.04),
        (false, "Beak") => c(0.3, 0.14, 0.03),
        (_, _) => c(0.16, 0.11, 0.06),
    }
}

/// One plumage's poses.
#[derive(Clone)]
pub struct DuckPoses {
    pub swim: Handle<Mesh>,
    pub dabble: Handle<Mesh>,
    pub swim_far: Handle<Mesh>,
    /// The wingbeat, top to bottom.
    pub beat: [Handle<Mesh>; 3],
    /// The same with the run-up's splash.
    pub run: [Handle<Mesh>; 3],
    pub glide: Handle<Mesh>,
    /// Swimming with the skid's wash.
    pub skid: Handle<Mesh>,
    pub fly_far: Handle<Mesh>,
}

#[derive(Resource, Default)]
struct DuckArt {
    /// Drake, hen.
    poses: Option<[DuckPoses; 2]>,
    material: Option<Handle<StandardMaterial>>,
}

impl DuckArt {
    fn of(&self, drake: bool) -> Option<&DuckPoses> {
        self.poses.as_ref().map(|p| &p[if drake { 0 } else { 1 }])
    }
}

#[derive(Component)]
struct DuckVisual {
    raft: Entity,
    index: usize,
    phase: f32,
}

#[derive(Component)]
struct Drawn;

pub struct DuckLookPlugin;

impl Plugin for DuckLookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DuckArt>().add_systems(
            Update,
            (load, draw, pose, far_rafts)
                .chain()
                .before(super::visuals::build_far),
        );
    }
}

fn keep(mut m: Mesh) -> Mesh {
    m.asset_usage = bevy::asset::RenderAssetUsages::default();
    m
}

/// Spray thrown up behind a duck, in duck space (-Z forward): a few
/// flattened drops about the feet and back along its wake.
fn splash() -> Mesh {
    let mut m = Mesh::from(Cuboid::new(0.001, 0.001, 0.001));
    let drops = [
        (0.12, 0.03, 0.12, 0.07),
        (-0.13, 0.04, 0.1, 0.06),
        (0.07, 0.08, 0.26, 0.05),
        (-0.08, 0.06, 0.3, 0.06),
        (0.0, 0.03, 0.42, 0.08),
        (0.18, 0.02, 0.0, 0.05),
        (-0.19, 0.02, -0.02, 0.05),
        (0.03, 0.12, 0.2, 0.04),
    ];
    for (x, y, z, s) in drops {
        let drop = Mesh::from(Cuboid::new(s, s * 0.45, s * 1.4)).translated_by(Vec3::new(x, y, z));
        m.merge(&drop).ok();
    }
    m
}

fn merged(
    tree: &[ModelNode],
    name: &str,
    drake: bool,
    extra: Option<&Mesh>,
    meshes: &mut Assets<Mesh>,
) -> Option<Handle<Mesh>> {
    let node = ModelNode::find(tree, name)?;
    let mut s = StandIn::default();
    for p in &node.primitives {
        let mesh = meshes.get(&p.mesh)?;
        s.add(mesh, colour(drake, p.material.as_deref()));
    }
    if let Some(e) = extra {
        s.add(e, LinearRgba::rgb(0.55, 0.6, 0.62));
    }
    s.build().map(|m| meshes.add(keep(m)))
}

/// Without the model: a lozenge on the water, a cross in the air.
fn blockout(drake: bool, meshes: &mut Assets<Mesh>) -> DuckPoses {
    let mut paint = |m: Mesh| {
        let mut s = StandIn::default();
        s.add(&m, colour(drake, Some("Pale")));
        meshes.add(keep(s.build().unwrap()))
    };
    let body = || Mesh::from(Cuboid::new(0.2, 0.12, 0.45)).translated_by(Vec3::Y * 0.04);
    let swim = paint(body());
    let mut wings = body();
    wings
        .merge(&Mesh::from(Cuboid::new(0.9, 0.01, 0.12)).translated_by(Vec3::Y * 0.1))
        .ok();
    let fly = paint(wings);
    DuckPoses {
        swim: swim.clone(),
        dabble: swim.clone(),
        swim_far: swim.clone(),
        beat: [fly.clone(), fly.clone(), fly.clone()],
        run: [fly.clone(), fly.clone(), fly.clone()],
        glide: fly.clone(),
        skid: swim,
        fly_far: fly,
    }
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut art: ResMut<DuckArt>,
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
            perceptual_roughness: 0.55,
            reflectance: 0.4,
            // The wings are single slabs seen from both sides.
            double_sided: true,
            cull_mode: None,
            ..default()
        }));
    }
    let blockouts = |meshes: &mut Assets<Mesh>| [blockout(true, meshes), blockout(false, meshes)];
    let Some(handle) = models.request(MODEL, &server) else {
        art.poses = Some(blockouts(&mut meshes));
        return;
    };
    match Models::state(&handle, &server, &gltfs) {
        ModelState::Loading => {}
        ModelState::Failed => art.poses = Some(blockouts(&mut meshes)),
        ModelState::Ready(gltf) => {
            let tree = model_nodes(gltf, &nodes, &gltf_meshes);
            let spray = splash();
            let mut plumage = |drake: bool| {
                let mut pose =
                    |n: &str, extra: Option<&Mesh>| merged(&tree, n, drake, extra, &mut meshes);
                Some(DuckPoses {
                    swim: pose("swim", None)?,
                    dabble: pose("dabble", None)?,
                    swim_far: pose("swim_lod1", None)?,
                    beat: [pose("up", None)?, pose("mid", None)?, pose("down", None)?],
                    run: [
                        pose("up", Some(&spray))?,
                        pose("mid", Some(&spray))?,
                        pose("down", Some(&spray))?,
                    ],
                    glide: pose("glide", None)?,
                    skid: pose("glide", Some(&spray))?,
                    fly_far: pose("fly_lod1", None)?,
                })
            };
            art.poses = Some(match (plumage(true), plumage(false)) {
                (Some(d), Some(h)) => [d, h],
                _ => {
                    warn!("{MODEL}: a pose is missing; drawing blockout ducks");
                    blockouts(&mut meshes)
                }
            });
        }
    }
}

fn draw(mut commands: Commands, art: Res<DuckArt>, rafts: Query<(Entity, &Raft), Without<Drawn>>) {
    let Some(mat) = art.material.clone() else {
        return;
    };
    if art.poses.is_none() {
        return;
    }
    for (e, raft) in &rafts {
        for (i, d) in raft.ducks.iter().enumerate() {
            let Some(poses) = art.of(d.drake) else {
                continue;
            };
            commands.spawn((
                Name::new("Duck"),
                DuckVisual {
                    raft: e,
                    index: i,
                    phase: (i as f32 * 0.37).fract(),
                },
                Mesh3d(poses.swim.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(d.pos),
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
    }
}

/// A duck's turn on the water: its heading; flying, along its flight.
fn facing(d: &super::ducks::Duck) -> Quat {
    let yaw = Quat::from_rotation_y(-d.yaw - std::f32::consts::FRAC_PI_2);
    match d.state {
        DuckState::Fly if d.vel.length_squared() > 0.01 => {
            Transform::IDENTITY.looking_to(d.vel, Vec3::Y).rotation
        }
        // Breast up, beating along the surface.
        DuckState::RunUp { .. } => yaw * Quat::from_rotation_x(0.35),
        DuckState::Skid => yaw * Quat::from_rotation_x(0.25),
        _ => yaw,
    }
}

fn pose(
    art: Res<DuckArt>,
    rafts: Query<&Raft>,
    mut ducks: Query<(
        &mut DuckVisual,
        &mut Transform,
        &mut Mesh3d,
        &mut Visibility,
    )>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let eyes: Vec<Vec3> = cams.iter().map(|g| g.translation()).collect();
    for (mut v, mut tf, mut mesh, mut vis) in &mut ducks {
        let Ok(raft) = rafts.get(v.raft) else {
            continue;
        };
        let Some(d) = raft.ducks.get(v.index) else {
            continue;
        };
        let Some(poses) = art.of(d.drake) else {
            continue;
        };
        let whole = !eyes.is_empty() && nearest(&eyes, raft.mean()) > RAFT_FAR;
        vis.set_if_neq(if whole {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        if whole {
            continue;
        }
        let far = !eyes.is_empty() && nearest(&eyes, d.pos) > FAR;
        let mut at = d.pos;
        let want = match d.state {
            DuckState::Swim => {
                // A little bob on the water.
                at.y += (t * 1.7 + v.index as f32 * 1.3).sin() * 0.008;
                if far {
                    &poses.swim_far
                } else if d.dabble > 0.0 {
                    &poses.dabble
                } else {
                    &poses.swim
                }
            }
            DuckState::Skid => {
                if far {
                    &poses.swim_far
                } else {
                    &poses.skid
                }
            }
            DuckState::RunUp { .. } | DuckState::Fly => {
                if far {
                    &poses.fly_far
                } else if d.flap < 0.2 {
                    v.phase = 0.0;
                    &poses.glide
                } else {
                    v.phase = (v.phase + BEAT * (0.6 + 0.4 * d.flap) * dt).fract();
                    let k = [0, 1, 2, 1][((v.phase * 4.0) as usize).min(3)];
                    if matches!(d.state, DuckState::RunUp { .. }) {
                        &poses.run[k]
                    } else {
                        &poses.beat[k]
                    }
                }
            }
        };
        *tf = Transform::from_translation(at).with_rotation(facing(d));
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}

/// Rafts far off: their ducks' far poses, into the animals' one far mesh.
fn far_rafts(
    art: Res<DuckArt>,
    rafts: Query<&Raft>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut far: ResMut<FarParts>,
) {
    let eyes: Vec<Vec3> = cams.iter().map(|g| g.translation()).collect();
    if eyes.is_empty() {
        return;
    }
    for raft in &rafts {
        let d = nearest(&eyes, raft.mean());
        if d <= RAFT_FAR || d > HIDE {
            continue;
        }
        for duck in &raft.ducks {
            let Some(poses) = art.of(duck.drake) else {
                continue;
            };
            let mesh = if duck.on_water() && !matches!(duck.state, DuckState::RunUp { .. }) {
                &poses.swim_far
            } else {
                &poses.fly_far
            };
            let at = Transform::from_translation(duck.pos).with_rotation(facing(duck));
            far.parts.push((mesh.clone(), at, LinearRgba::WHITE));
        }
    }
}
