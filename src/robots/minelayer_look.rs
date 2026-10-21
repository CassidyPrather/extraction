//! How a minelayer and its mines look (client only): dressed from
//! `assets/models/robots/minelayer.glb` (`art/robots/minelayer.py`) in
//! three LODs. Its amber lamp turns while it crawls. A laid mine sits in a
//! ring of turned earth and blinks red every couple of seconds
//! (`extraction_core::minelayer::lit`): look for it.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::minelayer as rules;

use super::heavy_look::{lod_name, lod_range};
use super::minelayer::{CHUTE, EYE, HOPPER_AT, HULL, Hopper, LaidMine, Minelayer, TREADS};
use super::{PartRole, Robot, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

const MODEL: &str = "models/robots/minelayer.glb";
/// Its lamp, local to its root.
pub const LAMP: Vec3 = Vec3::new(0.0, 0.95, 0.0);
/// Where the mine's LED sits on it. (The mine is modelled apart, at x 3,
/// but exported about its own origin like every part, so it hangs at the
/// mine itself.)
const LED: Vec3 = Vec3::new(0.1, 0.075, 0.0);

pub struct MinelayerLookPlugin;

impl Plugin for MinelayerLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress, blink_mines, turn_lamps));
    }
}

#[derive(Component)]
struct Dressed;

/// A mine's red light.
#[derive(Component)]
struct Led(Entity);

/// A crawler's lamp light.
#[derive(Component)]
struct Lamp(Entity);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress(
    mut commands: Commands,
    layers: Query<(Entity, &Children), (With<Minelayer>, Without<Dressed>)>,
    mines: Query<Entity, (With<LaidMine>, Without<Dressed>)>,
    parts: Query<(&RobotPart, Has<Hopper>, &Transform)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut extras: Local<
        Option<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<Mesh>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    if layers.is_empty() && mines.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("minelayer model failed to load");
            for (e, _) in &layers {
                commands.entity(e).insert(Dressed);
            }
            for e in &mines {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let (earth, earth_mat, led, led_mat) = extras
        .get_or_insert_with(|| {
            (
                meshes.add(Circle::new(0.5)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.17, 0.13, 0.09),
                    perceptual_roughness: 1.0,
                    ..default()
                }),
                meshes.add(Sphere::new(0.012)),
                materials.add(StandardMaterial {
                    base_color: Color::BLACK,
                    emissive: LinearRgba::rgb(30.0, 0.5, 0.3),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();
    let mut hang = |commands: &mut Commands,
                    materials: &mut Assets<StandardMaterial>,
                    parent: Entity,
                    name: &str,
                    at: Transform| {
        let holder = commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        for lod in 0..3 {
            let Some(node) = ModelNode::find(&model, &lod_name(name, lod)) else {
                continue;
            };
            for p in &node.primitives {
                let m = match p.material.as_deref() {
                    Some(n) => model_mats.get(n, &mut lib, &server, materials),
                    None => model_mats.get("Dark", &mut lib, &server, materials),
                };
                let mut ec =
                    commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(m), ChildOf(holder)));
                if let Some(r) = lod_range(lod) {
                    ec.insert(r);
                }
                match p.material.as_deref() {
                    Some("Shell") => {
                        ec.insert(super::weathering::Weather { shell: true });
                    }
                    Some("Dark") | None => {
                        ec.insert(super::weathering::Weather { shell: false });
                    }
                    _ => {}
                }
            }
        }
    };
    for (e, children) in &layers {
        hang(
            &mut commands,
            &mut materials,
            e,
            "chute",
            Transform::from_translation(CHUTE),
        );
        hang(
            &mut commands,
            &mut materials,
            e,
            "lamp",
            Transform::from_translation(LAMP),
        );
        for c in children.iter() {
            let Ok((part, hopper, pt)) = parts.get(c) else {
                continue;
            };
            let name = match part.role {
                PartRole::Core => "hull",
                PartRole::Sensor => "eye",
                _ if hopper => "hopper",
                PartRole::Armour(_) => {
                    if (pt.translation - TREADS[0]).length() < 0.01 {
                        "tread_l"
                    } else {
                        "tread_r"
                    }
                }
                _ => continue,
            };
            hang(&mut commands, &mut materials, c, name, Transform::IDENTITY);
        }
        let light = commands
            .spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.6, 0.15),
                    intensity: 0.0,
                    range: 18.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(LAMP + Vec3::Y * 0.08),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert((Lamp(light), Dressed));
        let _ = (HULL, HOPPER_AT, EYE);
    }
    for e in &mines {
        hang(
            &mut commands,
            &mut materials,
            e,
            "mine",
            Transform::IDENTITY,
        );
        // Its ring of turned earth.
        commands.spawn((
            Mesh3d(earth.clone()),
            MeshMaterial3d(earth_mat.clone()),
            Transform::from_xyz(0.0, 0.012, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            NotShadowCaster,
            ChildOf(e),
        ));
        let l = commands
            .spawn((
                Mesh3d(led.clone()),
                MeshMaterial3d(led_mat.clone()),
                Transform::from_translation(LED),
                Visibility::Hidden,
                NotShadowCaster,
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert((Led(l), Dressed));
    }
}

fn blink_mines(mines: Query<(&LaidMine, &Led)>, mut vis: Query<&mut Visibility>) {
    for (m, led) in &mines {
        if let Ok(mut v) = vis.get_mut(led.0) {
            let on = rules::lit(m.mine.age, m.phase) || m.mine.clicked.is_some();
            let want = if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != want {
                *v = want;
            }
        }
    }
}

/// Its lamp: a slow amber pulse while it crawls; dark once it's wrecked.
fn turn_lamps(layers: Query<(&Robot, &Lamp)>, mut lights: Query<&mut PointLight>, time: Res<Time>) {
    let t = time.elapsed_secs();
    for (r, lamp) in &layers {
        if let Ok(mut l) = lights.get_mut(lamp.0) {
            l.intensity = if r.wrecked {
                0.0
            } else {
                6000.0 * (0.5 + 0.5 * (t * 4.0).sin()).powi(3)
            };
        }
    }
}
