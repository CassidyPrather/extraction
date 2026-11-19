//! How a spotter looks (client only): dressed from
//! `assets/models/robots/spotter.glb` (`art/robots/spotter.py`) in three
//! LODs, its tether a thin dark line. Its beacon is the tell: a slow amber
//! blink while it watches, quicker and red as it builds a fix on you, a
//! hard white flash with the klaxon when it calls. Holed, its envelope
//! goes slack and lies flat on the ground.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
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
    pieces: Query<
        (
            Entity,
            &SpotterPiece,
            &OfSpotter,
            Has<crate::destruction::hooks::Cuttable>,
        ),
        Without<Dressed>,
    >,
    parts: Query<&RobotPart>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
                    (Some(n), _) => {
                        model_mats.get(super::weak_points::paint(n), &mut lib, &server, materials)
                    }
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
                    let eye = hang(
                        &mut commands,
                        &mut materials,
                        None,
                        c,
                        "eye",
                        Transform::IDENTITY,
                    );
                    // (Hidden once its camera is seated in its place,
                    // `spotter_salvage`.)
                    commands.entity(eye).insert(super::salvage::SeatLook {
                        robot: e,
                        role: PartRole::Sensor,
                    });
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
    for (e, piece, of, cuttable) in &pieces {
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
                // Its drum and motor, bolted to the skid: hidden once
                // they're seated as salvage in their place.
                let drum = hang(
                    &mut commands,
                    &mut materials,
                    None,
                    e,
                    "winch_drum",
                    Transform::from_xyz(0.0, -WINCH_PART_UP, 0.0),
                );
                commands.entity(drum).insert(super::salvage::SeatLook {
                    robot: of.0,
                    role: PartRole::Armour(40),
                });
            }
            SpotterPiece::Tether(_) => {
                // Steel cable (`lines`, #101): a grip sleeve on a stretch
                // in a hand's reach, where the cutters bite.
                let line = super::lines::Line::new(super::lines::Gauge::Cable);
                let line = if cuttable { line.with_grip(0.5) } else { line };
                commands.entity(e).insert(line);
            }
        }
        commands.entity(e).insert(Dressed);
    }
}

/// Each stretch of tether's line runs along it (its Z), its length.
fn tether_lines(
    mut commands: Commands,
    stretches: Query<
        (
            Entity,
            &Transform,
            &TetherLength,
            Option<&super::lines::LineSpan>,
        ),
        (
            With<super::lines::Line>,
            Or<(
                Changed<Transform>,
                Changed<TetherLength>,
                Added<super::lines::Line>,
            )>,
        ),
    >,
) {
    for (e, t, len, had) in &stretches {
        let half = t.rotation * Vec3::Z * (len.0.max(0.01) * 0.5);
        let span = super::lines::LineSpan {
            from: t.translation - half,
            to: t.translation + half,
            slack: 0.0,
        };
        if had != Some(&span) {
            commands.entity(e).insert(span);
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

/// Holed, the envelope goes slack: it sags as it sinks, and once it's down
/// it lies draped on the ground (`drape`), its fabric spread and creased,
/// not a balloon squashed.
#[derive(Component)]
struct Draped;

#[allow(clippy::type_complexity)]
fn slack_envelopes(
    mut commands: Commands,
    spotters: Query<(&Spotter, &Children)>,
    mut envelopes: Query<(Entity, &mut Transform, &Children, Has<Draped>), With<EnvelopeMesh>>,
    mut prims: Query<&mut Mesh3d>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let mut meshes = meshes;
    for (s, children) in &spotters {
        for c in children.iter() {
            let Ok((e, mut t, prim_children, draped)) = envelopes.get_mut(c) else {
                continue;
            };
            match s.float {
                Float::Sinking => {
                    let want = Vec3::new(1.05, 0.7, 1.05);
                    t.scale = t.scale.lerp(want, (0.6 * dt).min(1.0));
                }
                Float::Down if !draped => {
                    t.scale = Vec3::ONE;
                    if let Some(meshes) = meshes.as_deref_mut() {
                        for p in prim_children.iter() {
                            let Ok(mut m) = prims.get_mut(p) else {
                                continue;
                            };
                            if let Some(d) = meshes.get(&m.0).map(drape) {
                                m.0 = meshes.add(d);
                            }
                        }
                    }
                    commands.entity(e).insert(Draped);
                }
                _ => {}
            }
        }
    }
}

/// An envelope's mesh, slack on the ground (local to the spotter, whose
/// middle is `ENVELOPE_R * 0.5` over the ground once it's down).
fn drape(mesh: &Mesh) -> Mesh {
    let mut out = mesh.clone();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) =
        out.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for p in ps.iter_mut() {
            *p = draped(Vec3::from(*p)).to_array();
        }
    }
    out.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
    if out.indices().is_some() {
        out.compute_smooth_normals();
    } else {
        out.compute_flat_normals();
    }
    out
}

/// Where a point of the inflated envelope (local, centred on its middle)
/// lies once it's slack on the ground: pressed down to a fraction of its
/// height, spread wider at its foot as fabric falls outward, creased along
/// its gores, drooping at its rim, and sagging more on one side, where the
/// last of the gas sits.
pub fn draped(p: Vec3) -> Vec3 {
    use super::spotter::ENVELOPE_R;
    let ground = -ENVELOPE_R * 0.5 + 0.015;
    // (Its foot, the load collar, to the crown's valve cap.)
    let (base, tall) = (-1.8, 3.6);
    let up = ((p.y - base) / tall).clamp(0.0, 1.0);
    let spread = 1.22 + 0.2 * (1.0 - up);
    let (x, z) = (p.x * spread, p.z * spread);
    let r = (x * x + z * z).sqrt();
    let a = z.atan2(x);
    let mut y = ground + up * DRAPED_TALL;
    // Creases along the gores, and across them, deeper toward the rim.
    y +=
        (0.05 * (a * 12.0 + p.y * 2.5).sin() + 0.025 * (r * 7.0).sin()) * (r / ENVELOPE_R).min(1.0);
    // The rim droops to the ground.
    let rim = ((r - ENVELOPE_R) / (0.6 * ENVELOPE_R)).clamp(0.0, 1.0);
    y -= 0.2 * rim * rim * (3.0 - 2.0 * rim);
    // One side fuller than the other.
    y += 0.05 * (x - 0.4 * z);
    Vec3::new(x, y.max(ground), z)
}

/// A slack envelope's height off the ground at its crown (m).
pub const DRAPED_TALL: f32 = 0.42;

#[cfg(test)]
mod drape_tests {
    use super::*;
    use crate::robots::spotter::ENVELOPE_R;

    /// Down, its envelope lies on the ground, not hovering: every point of
    /// the inflated shape lands between the ground and a hand's height, its
    /// rim on the ground, and it spreads wider than it flew.
    #[test]
    fn a_slack_envelope_lies_on_the_ground() {
        let ground = -ENVELOPE_R * 0.5;
        let mut lowest_rim = f32::MAX;
        let mut widest = 0.0f32;
        for i in 0..24 {
            for j in 0..=12 {
                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                let v = j as f32 / 12.0 * std::f32::consts::PI;
                let p = Vec3::new(
                    ENVELOPE_R * v.sin() * a.cos(),
                    -1.6 * v.cos(),
                    ENVELOPE_R * v.sin() * a.sin(),
                );
                let d = draped(p);
                assert!(
                    (ground..ground + 0.65).contains(&d.y),
                    "{p} lies at {} over the ground",
                    d.y - ground
                );
                if j == 6 {
                    lowest_rim = lowest_rim.min(d.y - ground);
                }
                widest = widest.max(d.xz().length());
            }
        }
        assert!(lowest_rim < 0.1, "its rim on the ground: {lowest_rim}");
        assert!(widest > ENVELOPE_R * 1.2, "spread: {widest}");
    }
}
