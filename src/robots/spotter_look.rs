//! How a spotter looks (client only): dressed from
//! `assets/models/robots/spotter.glb` (`art/robots/spotter.py`) in three
//! LODs, its tether a thin dark line. Its beacon is the tell: a slow amber
//! blink while it watches, quicker and red as it builds a fix on you, a
//! hard white flash with the klaxon when it calls. Holed, its envelope
//! goes slack and lies flat on the ground.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::heavy_look::{lod_name, lod_range};
use super::spotter::{
    Float, GONDOLA, OfSpotter, Spotter, SpotterAct, SpotterEvent, SpotterPiece, TetherLength,
};
use super::{PartRole, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

const MODEL: &str = "models/robots/spotter.glb";
/// Its beacon, under the gondola (local to the envelope).
pub const LAMP: Vec3 = Vec3::new(0.0, -2.25, 0.0);
/// The winch's model sits on the ground; its part is centred 0.4 m up.
const WINCH_PART_UP: f32 = 0.4;

pub struct SpotterLookPlugin;

impl Plugin for SpotterLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (dress_spotters, beacons, slack_envelopes, tether_lines),
        );
    }
}

#[derive(Component)]
struct Dressed;

/// A spotter's beacon: its material and light, and its blink.
#[derive(Component)]
struct Beacon {
    lamp: Handle<StandardMaterial>,
    light: Entity,
    clock: f32,
    /// Seconds of the call's flash left.
    flash: f32,
}

/// The envelope's mesh holder (it goes slack when holed).
#[derive(Component)]
struct EnvelopeMesh;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress_spotters(
    mut commands: Commands,
    spotters: Query<(Entity, &Children), (With<Spotter>, Without<Dressed>)>,
    pieces: Query<(Entity, &SpotterPiece, &OfSpotter), Without<Dressed>>,
    parts: Query<&RobotPart>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tether: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    if spotters.is_empty() && pieces.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("spotter model failed to load");
            for (e, _) in &spotters {
                commands.entity(e).insert(Dressed);
            }
            for (e, ..) in &pieces {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let (line, line_mat) = tether
        .get_or_insert_with(|| {
            (
                meshes.add(Cylinder::new(0.014, 1.0)),
                model_mats.get("Rubber", &mut lib, &server, &mut materials),
            )
        })
        .clone();
    let base_lamp = model_mats.get("Lamp", &mut lib, &server, &mut materials);
    let mut hang = |commands: &mut Commands,
                    materials: &mut Assets<StandardMaterial>,
                    lamp: Option<&Handle<StandardMaterial>>,
                    parent: Entity,
                    name: &str,
                    at: Transform|
     -> Entity {
        let holder = commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        for lod in 0..3 {
            let Some(node) = ModelNode::find(&model, &lod_name(name, lod)) else {
                continue;
            };
            for p in &node.primitives {
                let m = match (p.material.as_deref(), lamp) {
                    (Some("Lamp"), Some(l)) => l.clone(),
                    (Some(n), _) => model_mats.get(n, &mut lib, &server, materials),
                    (None, _) => model_mats.get("Dark", &mut lib, &server, materials),
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
        holder
    };
    for (e, children) in &spotters {
        let lamp = {
            let copy = materials.get(&base_lamp).cloned().unwrap_or_default();
            materials.add(copy)
        };
        let env = hang(
            &mut commands,
            &mut materials,
            Some(&lamp),
            e,
            "envelope",
            Transform::IDENTITY,
        );
        commands.entity(env).insert(EnvelopeMesh);
        hang(
            &mut commands,
            &mut materials,
            Some(&lamp),
            e,
            "lamp",
            Transform::from_translation(LAMP),
        );
        for c in children.iter() {
            match parts.get(c).map(|p| p.role) {
                Ok(PartRole::Sensor) => {
                    hang(
                        &mut commands,
                        &mut materials,
                        None,
                        c,
                        "eye",
                        Transform::IDENTITY,
                    );
                }
                Ok(PartRole::Armour(_)) => {
                    hang(
                        &mut commands,
                        &mut materials,
                        None,
                        c,
                        "gondola",
                        Transform::IDENTITY,
                    );
                }
                _ => {}
            }
        }
        let light = commands
            .spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.6, 0.15),
                    intensity: 0.0,
                    range: 30.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(LAMP - Vec3::Y * 0.15),
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert((
            Beacon {
                lamp,
                light,
                clock: 0.0,
                flash: 0.0,
            },
            Dressed,
        ));
        let _ = GONDOLA;
    }
    for (e, piece, _) in &pieces {
        match piece {
            SpotterPiece::Winch => {
                hang(
                    &mut commands,
                    &mut materials,
                    None,
                    e,
                    "winch",
                    Transform::from_xyz(0.0, -WINCH_PART_UP, 0.0),
                );
            }
            SpotterPiece::Tether(_) => {
                // A unit cylinder along Y, turned onto the stretch's Z.
                commands.spawn((
                    Mesh3d(line.clone()),
                    MeshMaterial3d(line_mat.clone()),
                    Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    NotShadowCaster,
                    ChildOf(e),
                ));
            }
        }
        commands.entity(e).insert(Dressed);
    }
}

/// Each stretch of tether drawn its length.
fn tether_lines(
    stretches: Query<(&TetherLength, &Children), Changed<TetherLength>>,
    mut lines: Query<&mut Transform, With<Mesh3d>>,
) {
    for (len, children) in &stretches {
        for c in children.iter() {
            if let Ok(mut t) = lines.get_mut(c) {
                t.scale = Vec3::new(1.0, len.0.max(0.01), 1.0);
            }
        }
    }
}

/// The beacon: slow amber, quickening red with a fix, a white flash with
/// the call; dark when it's blinded, loose or down.
fn beacons(
    mut spotters: Query<(Entity, &Spotter, &mut Beacon, Has<super::Stunned>)>,
    mut lights: Query<&mut PointLight>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut events: MessageReader<SpotterEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let calls: Vec<Entity> = events
        .read()
        .filter(|e| e.what == SpotterAct::Call)
        .map(|e| e.spotter)
        .collect();
    for (e, s, mut b, stunned) in &mut spotters {
        if calls.contains(&e) {
            b.flash = 2.0;
        }
        b.flash = (b.flash - dt).max(0.0);
        b.clock += dt;
        let alive = s.float == Float::Aloft && !stunned;
        let period = s.watch.blink().max(0.15);
        let on = alive && (b.clock % period) < period * 0.35;
        let (color, power) = if !alive {
            (LinearRgba::BLACK, 0.0)
        } else if b.flash > 0.0 {
            let pulse = if (b.flash * 8.0) as i32 % 2 == 0 {
                1.0
            } else {
                0.2
            };
            (LinearRgba::rgb(12.0, 11.0, 10.0) * pulse, 60_000.0 * pulse)
        } else if !on {
            (LinearRgba::rgb(0.05, 0.03, 0.0), 0.0)
        } else if s.watch.fix > 0.0 {
            (LinearRgba::rgb(9.0, 0.6, 0.2), 25_000.0)
        } else {
            (LinearRgba::rgb(6.0, 3.0, 0.4), 12_000.0)
        };
        if let Some(mut m) = materials.get_mut(&b.lamp) {
            m.emissive = color;
        }
        if let Ok(mut l) = lights.get_mut(b.light) {
            l.intensity = power;
            l.color = if s.watch.fix > 0.0 || b.flash > 0.0 {
                Color::srgb(1.0, 0.3, 0.2)
            } else {
                Color::srgb(1.0, 0.6, 0.15)
            };
        }
    }
}

/// Holed, the envelope goes slack: it sags as it sinks and lies flat once
/// it's down.
fn slack_envelopes(
    spotters: Query<(&Spotter, &Children)>,
    mut envelopes: Query<&mut Transform, With<EnvelopeMesh>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (s, children) in &spotters {
        let want = match s.float {
            Float::Sinking => Vec3::new(1.05, 0.7, 1.05),
            Float::Down => Vec3::new(1.4, 0.28, 1.4),
            _ => continue,
        };
        for c in children.iter() {
            if let Ok(mut t) = envelopes.get_mut(c) {
                t.scale = t.scale.lerp(want, (0.6 * dt).min(1.0));
            }
        }
    }
}
