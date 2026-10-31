//! How a minelayer and its mines look (client only): dressed from
//! `assets/models/robots/minelayer.glb` (`art/robots/minelayer.py`) in
//! three LODs. Its amber lamp turns while it crawls. A laid mine sits in a
//! ring of turned earth, dark; with someone near it blinks red every
//! couple of seconds (`extraction_core::minelayer::lit_near`): look for it.

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
    mines: Query<(Entity, &Transform), (With<LaidMine>, Without<Dressed>)>,
    zone: Option<Res<crate::world::Zone>>,
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
            for (e, _) in &mines {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let (earth, earth_mat, led, led_mat) = extras
        .get_or_insert_with(|| {
            (
                meshes.add(Circle::new(0.5)),
                // Turned earth, a little lighter than dark soil; the ring's
                // own vertex colours fade it out at its rim.
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.42, 0.34, 0.26),
                    perceptual_roughness: 1.0,
                    alpha_mode: AlphaMode::Blend,
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
                    Some(n) => {
                        model_mats.get(super::weak_points::paint(n), &mut lib, &server, materials)
                    }
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
    for (e, mt) in &mines {
        hang(
            &mut commands,
            &mut materials,
            e,
            "mine",
            Transform::IDENTITY,
        );
        // Its ring of turned earth, draped over the ground round it (a
        // flat disc sinks into a slope on one side and floats on the
        // other).
        let ring = match zone.as_ref() {
            Some(z) => meshes.add(earth_ring(|x, y| z.ground(x, y), mt.translation)),
            None => earth.clone(),
        };
        commands.spawn((
            Mesh3d(ring),
            MeshMaterial3d(earth_mat.clone()),
            Transform::IDENTITY,
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

fn blink_mines(
    mines: Query<(&LaidMine, &Led, &GlobalTransform)>,
    people: Query<&GlobalTransform, With<crate::player::Player>>,
    mut vis: Query<&mut Visibility>,
) {
    for (m, led, gt) in &mines {
        if let Ok(mut v) = vis.get_mut(led.0) {
            // Dark till someone's near (`rules::WAKE`).
            let nearest = people
                .iter()
                .map(|p| p.translation().xz().distance(gt.translation().xz()))
                .fold(f32::INFINITY, f32::min);
            let on = rules::lit_near(m.mine.age, m.phase, nearest) || m.mine.clicked.is_some();
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

/// The ring of turned earth round a mine at `at`: an annulus from just
/// under its rim out to [`EARTH_OUT`], each vertex on the ground
/// (`ground(x, z)`) a little proud of it, faded out at its outer edge.
/// Local to the mine.
pub fn earth_ring(ground: impl Fn(f32, f32) -> f32, at: Vec3) -> Mesh {
    let (pos, colours) = earth_ring_points(ground, at);
    let n = pos.len() / 2;
    let mut idx = Vec::with_capacity(n * 6);
    for k in 0..n {
        let (i0, o0) = (2 * k as u32, 2 * k as u32 + 1);
        let (i1, o1) = (2 * ((k + 1) % n) as u32, 2 * ((k + 1) % n) as u32 + 1);
        idx.extend_from_slice(&[i0, o1, o0, i0, i1, o1]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
    .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

/// The ring's inner and outer radii (m), its lift off the ground, and its
/// segments.
pub const EARTH_IN: f32 = 0.13;
pub const EARTH_OUT: f32 = 0.5;
const EARTH_LIFT: f32 = 0.012;
const EARTH_SEGS: usize = 20;

/// The ring's vertices (inner, outer, ... round it), local to the mine,
/// and their colours (opaque inside, clear at the rim).
pub fn earth_ring_points(
    ground: impl Fn(f32, f32) -> f32,
    at: Vec3,
) -> (Vec<[f32; 3]>, Vec<[f32; 4]>) {
    let mut pos = Vec::with_capacity(EARTH_SEGS * 2);
    let mut col = Vec::with_capacity(EARTH_SEGS * 2);
    for k in 0..EARTH_SEGS {
        let a = k as f32 * std::f32::consts::TAU / EARTH_SEGS as f32;
        for (r, alpha) in [(EARTH_IN, 0.85), (EARTH_OUT, 0.0)] {
            let (x, z) = (a.cos() * r, a.sin() * r);
            let y = ground(at.x + x, at.z + z) - at.y + EARTH_LIFT;
            pos.push([x, y, z]);
            col.push([1.0, 1.0, 1.0, alpha]);
        }
    }
    (pos, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a steep slope the ring lies on the ground all the way round:
    /// never under it, never more than a couple of cm off it.
    #[test]
    fn the_earth_ring_lies_on_a_slope() {
        let ground = |x: f32, z: f32| 10.0 + 0.6 * x - 0.3 * z + 0.05 * (x * 7.0).sin();
        let at = Vec3::new(4.0, ground(4.0, -2.0), -2.0);
        let (pos, col) = earth_ring_points(ground, at);
        for p in &pos {
            let g = ground(at.x + p[0], at.z + p[2]) - at.y;
            let off = p[1] - g;
            assert!(off > 0.0 && off < 0.03, "{off} off the ground at {p:?}");
        }
        assert!(col.iter().any(|c| c[3] == 0.0), "a soft rim");
        assert!(col.iter().any(|c| c[3] > 0.5), "turned earth inside");
    }
}
