//! The small life, drawn: fish in the creeks (`fish`), and moths round the
//! lamps at dusk and at night.
//!
//! Fish are one mesh each (`art/animals/fish.glb`, merged and
//! vertex-coloured like the birds), the tail's beat a sway of the body,
//! drawn only within [`FISH_REACH`]: you see them from the bank or not at
//! all. Moths are visual only: a few round every warm light near you
//! (any `PointLight`: house lamps, lanterns, the camp's), flitting in and
//! out of it, two poses of wing; never in the day.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::noise::Rng;

use super::fish::Shoal;
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::lod::StandIn;

pub const FISH_REACH: f32 = 30.0;
/// Moths round lights within this of you...
pub const MOTH_REACH: f32 = 35.0;
/// ...this many a light...
pub const MOTHS: usize = 7;
/// ...and at most this many lights at once.
pub const MOTH_LIGHTS: usize = 6;

const MODEL: &str = "models/animals/fish.glb";
const RAT: &str = "models/animals/rat.glb";

#[derive(Resource, Default)]
struct SmallArt {
    fish: Option<Handle<Mesh>>,
    moth: Option<[Handle<Mesh>; 2]>,
    /// Sitting, and the scurry's two frames.
    rat: Option<[Handle<Mesh>; 3]>,
    material: Option<Handle<StandardMaterial>>,
    done: bool,
}

#[derive(Component)]
struct FishVisual {
    shoal: Entity,
    index: usize,
}

#[derive(Component)]
struct Moth {
    light: Entity,
    phase: f32,
    seed: f32,
}

#[derive(Component)]
struct Drawn;

#[derive(Component)]
struct RatVisual {
    house: Entity,
    index: usize,
}

pub struct SmallLookPlugin;

impl Plugin for SmallLookPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SmallArt>().add_systems(
            Update,
            (
                load, load_rats, draw_fish, pose_fish, draw_rats, pose_rats, moths,
            )
                .chain(),
        );
    }
}

fn keep(mut m: Mesh) -> Mesh {
    m.asset_usage = bevy::asset::RenderAssetUsages::default();
    m
}

fn merged(
    tree: &[ModelNode],
    name: &str,
    meshes: &mut Assets<Mesh>,
    colours: &[(&str, LinearRgba)],
) -> Option<Handle<Mesh>> {
    let node = ModelNode::find(tree, name)?;
    let mut s = StandIn::default();
    for p in &node.primitives {
        let mesh = meshes.get(&p.mesh)?;
        let c = colours
            .iter()
            .find(|(n, _)| Some(*n) == p.material.as_deref())
            .map_or(colours[0].1, |(_, c)| *c);
        s.add(mesh, c);
    }
    s.build().map(|m| meshes.add(keep(m)))
}

#[allow(clippy::too_many_arguments)]
fn load(
    mut art: ResMut<SmallArt>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if art.done {
        return;
    }
    if art.material.is_none() {
        art.material = Some(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.5,
            reflectance: 0.4,
            double_sided: true,
            cull_mode: None,
            ..default()
        }));
    }
    let blockout = |meshes: &mut Assets<Mesh>| {
        let mut s = StandIn::default();
        s.add(
            &Mesh::from(Cuboid::new(0.04, 0.05, 0.26)),
            LinearRgba::rgb(0.03, 0.03, 0.025),
        );
        let fish = meshes.add(keep(s.build().unwrap()));
        let mut s = StandIn::default();
        s.add(
            &Mesh::from(Cuboid::new(0.04, 0.002, 0.015)),
            LinearRgba::rgb(0.5, 0.47, 0.4),
        );
        let moth = meshes.add(keep(s.build().unwrap()));
        (fish, [moth.clone(), moth])
    };
    let Some(handle) = models.request(MODEL, &server) else {
        let (f, m) = blockout(&mut meshes);
        (art.fish, art.moth, art.done) = (Some(f), Some(m), true);
        return;
    };
    match Models::state(&handle, &server, &gltfs) {
        ModelState::Loading => {}
        ModelState::Failed => {
            let (f, m) = blockout(&mut meshes);
            (art.fish, art.moth, art.done) = (Some(f), Some(m), true);
        }
        ModelState::Ready(gltf) => {
            let tree = model_nodes(gltf, &nodes, &gltf_meshes);
            let fish_c = [
                ("Hide", LinearRgba::rgb(0.035, 0.035, 0.025)),
                ("Pale", LinearRgba::rgb(0.2, 0.19, 0.15)),
            ];
            let moth_c = [("Pale", LinearRgba::rgb(0.5, 0.46, 0.38))];
            let fish = merged(&tree, "fish", &mut meshes, &fish_c);
            let up = merged(&tree, "moth_up", &mut meshes, &moth_c);
            let down = merged(&tree, "moth_down", &mut meshes, &moth_c);
            match (fish, up, down) {
                (Some(f), Some(u), Some(d)) => {
                    (art.fish, art.moth) = (Some(f), Some([u, d]));
                }
                _ => {
                    warn!("{MODEL}: a part is missing; drawing blockouts");
                    let (f, m) = blockout(&mut meshes);
                    (art.fish, art.moth) = (Some(f), Some(m));
                }
            }
            art.done = true;
        }
    }
}

fn draw_fish(
    mut commands: Commands,
    art: Res<SmallArt>,
    shoals: Query<(Entity, &Shoal), Without<Drawn>>,
) {
    let (Some(mesh), Some(mat)) = (art.fish.clone(), art.material.clone()) else {
        return;
    };
    for (e, s) in &shoals {
        for (i, f) in s.fish.iter().enumerate() {
            // Children of the shoal: they go when it's dropped.
            commands.spawn((
                Name::new("Fish"),
                FishVisual { shoal: e, index: i },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(f.pos),
                Visibility::default(),
                NotShadowCaster,
                bevy::camera::visibility::VisibilityRange {
                    start_margin: 0.0..0.0,
                    end_margin: (FISH_REACH - 4.0)..FISH_REACH,
                    use_aabb: false,
                },
                ChildOf(e),
            ));
        }
        commands.entity(e).insert(Drawn);
    }
}

fn pose_fish(shoals: Query<&Shoal>, mut fish: Query<(&FishVisual, &mut Transform)>) {
    for (v, mut t) in &mut fish {
        let Some(f) = shoals.get(v.shoal).ok().and_then(|s| s.fish.get(v.index)) else {
            continue;
        };
        // The tail's beat as a sway of the whole fish.
        let sway = (f.phase * std::f32::consts::TAU).sin() * 0.25;
        let rot = Quat::from_rotation_y(-f.yaw - std::f32::consts::FRAC_PI_2 + sway);
        *t = Transform::from_translation(f.pos)
            .with_rotation(rot)
            .with_scale(Vec3::splat(f.size));
    }
}

/// Moths round the lamps near you, at dusk and at night.
#[allow(clippy::too_many_arguments)]
fn moths(
    mut commands: Commands,
    art: Res<SmallArt>,
    lights: Query<(Entity, &GlobalTransform, &PointLight)>,
    mut moths: Query<(Entity, &mut Moth, &mut Transform, &mut Mesh3d), Without<PointLight>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    time: Res<Time>,
    mut rng: Local<Option<Rng>>,
) {
    use crate::world::weather::Hour;
    let (Some(wings), Some(mat)) = (art.moth.clone(), art.material.clone()) else {
        return;
    };
    let dark = conditions
        .as_ref()
        .is_some_and(|c| matches!(c.hour, Hour::Dusk | Hour::Night) && c.rain < 0.6);
    let Some(eye) = cams.iter().next().map(|g| g.translation()) else {
        return;
    };
    // Which lights have moths: the nearest few, in reach, after dark.
    let mut near: Vec<(Entity, f32)> = if dark {
        lights
            .iter()
            .filter(|(_, _, l)| l.intensity > 1000.0)
            .map(|(e, g, _)| (e, g.translation().distance(eye)))
            .filter(|(_, d)| *d < MOTH_REACH)
            .collect()
    } else {
        Vec::new()
    };
    near.sort_by(|a, b| a.1.total_cmp(&b.1));
    near.truncate(MOTH_LIGHTS);
    let rng = rng.get_or_insert_with(|| Rng::new(0x307));
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let mut have: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for (e, mut m, mut tf, mut mesh) in &mut moths {
        let Some(at) = near
            .iter()
            .find(|(l, _)| *l == m.light)
            .and_then(|(l, _)| lights.get(*l).ok())
            .map(|(_, g, _)| g.translation())
        else {
            commands.entity(e).despawn();
            continue;
        };
        *have.entry(m.light).or_default() += 1;
        // Loops in and out of the light, a different one each.
        let s = m.seed;
        let r = 0.18 + 0.35 * (0.5 + 0.5 * (t * (0.7 + s) + s * 9.0).sin());
        let a = t * (2.0 + s * 3.0) + s * 20.0;
        let p = at
            + Vec3::new(
                a.cos() * r,
                (t * (1.3 + s) + s * 5.0).sin() * 0.25 - 0.1,
                (a * 1.3).sin() * r,
            );
        let dir = (p - tf.translation).normalize_or(Vec3::X);
        tf.translation = p;
        tf.rotation = Transform::IDENTITY.looking_to(dir, Vec3::Y).rotation;
        m.phase = (m.phase + dt * 18.0).fract();
        let want = &wings[(m.phase * 2.0) as usize % 2];
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
    for (light, _) in &near {
        let n = have.get(light).copied().unwrap_or(0);
        for _ in n..MOTHS {
            commands.spawn((
                Name::new("Moth"),
                Moth {
                    light: *light,
                    phase: rng.f32(),
                    seed: rng.f32(),
                },
                Mesh3d(wings[0].clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(eye + Vec3::Y * -100.0),
                Visibility::default(),
                NotShadowCaster,
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn load_rats(
    mut art: ResMut<SmallArt>,
    mut models: ResMut<Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tried: Local<bool>,
) {
    if art.rat.is_some() || *tried {
        return;
    }
    let blockout = |meshes: &mut Assets<Mesh>| {
        let mut s = StandIn::default();
        s.add(
            &Mesh::from(Cuboid::new(0.07, 0.06, 0.2)).translated_by(Vec3::Y * 0.03),
            LinearRgba::rgb(0.04, 0.035, 0.03),
        );
        let m = meshes.add(keep(s.build().unwrap()));
        [m.clone(), m.clone(), m]
    };
    let Some(handle) = models.request(RAT, &server) else {
        art.rat = Some(blockout(&mut meshes));
        return;
    };
    match Models::state(&handle, &server, &gltfs) {
        ModelState::Loading => {}
        ModelState::Failed => art.rat = Some(blockout(&mut meshes)),
        ModelState::Ready(gltf) => {
            *tried = true;
            let tree = model_nodes(gltf, &nodes, &gltf_meshes);
            let c = [
                ("Hide", LinearRgba::rgb(0.035, 0.03, 0.026)),
                ("Pale", LinearRgba::rgb(0.16, 0.12, 0.11)),
            ];
            let mut pose = |n: &str| merged(&tree, n, &mut meshes, &c);
            art.rat = Some(match (pose("sit"), pose("run_a"), pose("run_b")) {
                (Some(a), Some(b), Some(c)) => [a, b, c],
                _ => {
                    warn!("{RAT}: a pose is missing; drawing blockout rats");
                    blockout(&mut meshes)
                }
            });
        }
    }
}

fn draw_rats(
    mut commands: Commands,
    art: Res<SmallArt>,
    houses: Query<(Entity, &super::rats::Rats), Without<Drawn>>,
) {
    let (Some(rat), Some(mat)) = (art.rat.clone(), art.material.clone()) else {
        return;
    };
    for (e, h) in &houses {
        for (i, r) in h.rats.iter().enumerate() {
            commands.spawn((
                Name::new("Rat"),
                RatVisual { house: e, index: i },
                Mesh3d(rat[0].clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(r.pos),
                Visibility::default(),
                NotShadowCaster,
                ChildOf(e),
            ));
        }
        commands.entity(e).insert(Drawn);
    }
}

fn pose_rats(
    art: Res<SmallArt>,
    houses: Query<&super::rats::Rats>,
    mut rats: Query<(&RatVisual, &mut Transform, &mut Mesh3d, &mut Visibility)>,
) {
    use super::rats::RatState;
    let Some(frames) = &art.rat else {
        return;
    };
    for (v, mut t, mut mesh, mut vis) in &mut rats {
        let Some(r) = houses.get(v.house).ok().and_then(|h| h.rats.get(v.index)) else {
            continue;
        };
        let want = match r.state {
            RatState::Gone => {
                *vis = Visibility::Hidden;
                continue;
            }
            RatState::Sniff => &frames[0],
            RatState::Run { .. } => &frames[1 + (r.phase * 2.0) as usize % 2],
        };
        // Sniffing: the nose bobs a little.
        let bob = if r.state == RatState::Sniff {
            (r.phase * 40.0).sin().max(0.0) * 0.08
        } else {
            0.0
        };
        *t = Transform::from_translation(r.pos).with_rotation(
            Quat::from_rotation_y(-r.yaw - std::f32::consts::FRAC_PI_2)
                * Quat::from_rotation_x(bob),
        );
        if mesh.0 != *want {
            mesh.0 = want.clone();
        }
    }
}
