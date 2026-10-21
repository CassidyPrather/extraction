//! How a plough looks (client only): dressed from
//! `assets/models/robots/plough.glb` (`art/robots/plough.py`) in three
//! LODs, each part hung at its own origin. Its auger spins and its beacon
//! turns while it runs; ploughing, a plume of snow arcs out of its chute
//! (the tell you see from across the valley); jammed, the auger stops and
//! the engine hatch stands open over the cell. Its bank is laid in
//! sections along the road's downhill side.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::heavy_look::{lod_name, lod_range};
use super::plough::{
    AUGER, BANK_STEP, BankPiece, CHUTE_TOP, HATCH, Lanes, Plough, PloughCell, PloughHatch,
};
use super::{PartRole, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;
use extraction_core::plough as rules;

const MODEL: &str = "models/robots/plough.glb";
/// Where the model's pieces sit on its root (each exported about its own
/// origin).
const TRACKS: [(&str, Vec3); 2] = [
    ("track_l", Vec3::new(-0.85, 0.45, 0.4)),
    ("track_r", Vec3::new(0.85, 0.45, 0.4)),
];
const HOUSING: Vec3 = Vec3::new(0.0, 0.85, -1.95);
const CHUTE: Vec3 = Vec3::new(0.6, 2.0, -1.2);
const BEACON: Vec3 = Vec3::new(0.0, 2.75, 0.0);
/// Where the housing part sits (see `plough::spawn_plough`).
const HOUSING_PART: Vec3 = Vec3::new(0.0, 0.85, -2.0);

pub struct PloughLookPlugin;

impl Plugin for PloughLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress, run_parts, gallery));
    }
}

#[derive(Component)]
struct Dressed;

/// Its moving pieces: the auger drum, the beacon's light, the hatch and
/// the plume.
#[derive(Component)]
struct Moving {
    auger: Entity,
    beacon: Entity,
    hatch: Option<Entity>,
    plume: Entity,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress(
    mut commands: Commands,
    ploughs: Query<(Entity, &Children), (With<Plough>, Without<Dressed>)>,
    banks: Query<Entity, (With<BankPiece>, Without<Dressed>)>,
    parts: Query<&RobotPart>,
    hatches: Query<(), With<PloughHatch>>,
    cells: Query<(), With<PloughCell>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plume: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    if ploughs.is_empty() && banks.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("plough model failed to load");
            for (e, _) in &ploughs {
                commands.entity(e).insert(Dressed);
            }
            for e in &banks {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let (plume_mesh, plume_mat) = plume
        .get_or_insert_with(|| {
            (
                meshes.add(Cone::new(0.9, 5.0)),
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.95, 0.97, 1.0, 0.45),
                    emissive: LinearRgba::rgb(0.25, 0.25, 0.28),
                    alpha_mode: AlphaMode::Blend,
                    unlit: false,
                    perceptual_roughness: 1.0,
                    ..default()
                }),
            )
        })
        .clone();
    let mut hang = |commands: &mut Commands,
                    materials: &mut Assets<StandardMaterial>,
                    parent: Entity,
                    name: &str,
                    at: Transform,
                    lods: usize|
     -> Entity {
        let holder = commands
            .spawn((at, Visibility::default(), ChildOf(parent)))
            .id();
        for lod in 0..lods {
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
        holder
    };
    for (e, children) in &ploughs {
        for (name, at) in TRACKS {
            hang(
                &mut commands,
                &mut materials,
                e,
                name,
                Transform::from_translation(at),
                3,
            );
        }
        let auger = hang(
            &mut commands,
            &mut materials,
            e,
            "auger",
            Transform::from_translation(AUGER),
            3,
        );
        hang(
            &mut commands,
            &mut materials,
            e,
            "chute",
            Transform::from_translation(CHUTE),
            3,
        );
        hang(
            &mut commands,
            &mut materials,
            e,
            "beacon",
            Transform::from_translation(BEACON),
            3,
        );
        let mut hatch = None;
        for c in children.iter() {
            if hatches.contains(c) {
                hatch = Some(hang(
                    &mut commands,
                    &mut materials,
                    c,
                    "hatch",
                    Transform::IDENTITY,
                    3,
                ));
                continue;
            }
            if cells.contains(c) {
                hang(
                    &mut commands,
                    &mut materials,
                    c,
                    "cell",
                    Transform::IDENTITY,
                    3,
                );
                continue;
            }
            match parts.get(c).map(|p| p.role) {
                Ok(PartRole::Armour(0)) => {
                    hang(
                        &mut commands,
                        &mut materials,
                        c,
                        "hull",
                        Transform::IDENTITY,
                        3,
                    );
                }
                Ok(PartRole::Armour(1)) => {
                    hang(
                        &mut commands,
                        &mut materials,
                        c,
                        "housing",
                        Transform::from_translation(HOUSING - HOUSING_PART),
                        3,
                    );
                }
                Ok(PartRole::Sensor) => {
                    hang(
                        &mut commands,
                        &mut materials,
                        c,
                        "eye",
                        Transform::IDENTITY,
                        3,
                    );
                }
                _ => {}
            }
        }
        let beacon = commands
            .spawn((
                SpotLight {
                    color: Color::srgb(1.0, 0.6, 0.1),
                    intensity: 0.0,
                    range: 40.0,
                    outer_angle: 0.5,
                    inner_angle: 0.3,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(BEACON + Vec3::Y * 0.12),
                ChildOf(e),
            ))
            .id();
        // The plume: a cone of thrown snow from the chute's hood, out and
        // up to the downhill side.
        let plume = commands
            .spawn((
                Mesh3d(plume_mesh.clone()),
                MeshMaterial3d(plume_mat.clone()),
                Transform::from_translation(CHUTE_TOP + Vec3::new(2.0, 0.8, 0.0))
                    .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2 + 0.5)),
                Visibility::Hidden,
                NotShadowCaster,
                ChildOf(e),
            ))
            .id();
        commands.entity(e).insert((
            Moving {
                auger,
                beacon,
                hatch,
                plume,
            },
            Dressed,
        ));
        let _ = HATCH;
    }
    for e in &banks {
        hang(
            &mut commands,
            &mut materials,
            e,
            "bank",
            Transform::IDENTITY,
            2,
        );
        commands.entity(e).insert(Dressed);
    }
}

/// The auger turns and the beacon sweeps while it runs; the plume while
/// it ploughs; jammed, the hatch stands open.
fn run_parts(
    ploughs: Query<(&Plough, &Moving, &super::Robot)>,
    mut transforms: Query<&mut Transform>,
    mut lights: Query<&mut SpotLight>,
    mut vis: Query<&mut Visibility>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (p, m, robot) in &ploughs {
        let running = !robot.wrecked && p.jammed <= 0.0;
        if running {
            if let Ok(mut a) = transforms.get_mut(m.auger) {
                a.rotate_local_x(-9.0 * dt);
            }
        }
        if let Ok(mut b) = transforms.get_mut(m.beacon) {
            b.rotation = Quat::from_rotation_y(t * 4.0) * Quat::from_rotation_x(-0.25);
        }
        if let Ok(mut l) = lights.get_mut(m.beacon) {
            l.intensity = if robot.wrecked { 0.0 } else { 400_000.0 };
        }
        if let Ok(mut v) = vis.get_mut(m.plume) {
            let want = if p.ploughing && running {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != want {
                *v = want;
            }
        }
        if let Ok(mut pt) = transforms.get_mut(m.plume) {
            let k = 1.0 + 0.12 * (t * 7.0).sin();
            pt.scale = Vec3::new(k, 1.0, k);
        }
        if let Some(h) = m.hatch {
            if let Ok(mut ht) = transforms.get_mut(h) {
                // Hinged at its front edge: up and open while jammed.
                let open = if p.jammed > 0.0 || robot.wrecked {
                    -1.1
                } else {
                    0.0
                };
                let now = ht.rotation.to_euler(EulerRot::XYZ).0;
                let a = now + (open - now) * (6.0 * dt).min(1.0);
                ht.rotation = Quat::from_rotation_x(a);
            }
        }
    }
}

/// `--gallery plough[:after]`: Rime Fell's plough on its road in front of
/// the camera. `plough` puts it a little up the road, coming, a tin lying
/// in its way; `plough:after` has it gone past, its lane and bank behind
/// it and the tin buried.
fn gallery(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut stage: Local<u32>,
    mut held: Local<Option<(Entity, f32)>>,
    mut ploughs: Query<&mut Plough>,
    mut lanes: ResMut<Lanes>,
    time: Res<Time>,
) {
    let (Some(spec), Some((cam, _, _)), Some(zone)) = (&opts.gallery, opts.camera, zone) else {
        return;
    };
    let Some(state) = spec
        .split(',')
        .next()
        .and_then(|s| s.strip_prefix("plough"))
    else {
        return;
    };
    *stage += 1;
    // Held where it was put for the picture, its auger and plume going.
    if let Some((e, at)) = *held {
        if let Ok(mut p) = ploughs.get_mut(e) {
            p.run.leg = rules::Leg::Up(at);
            p.banked = at;
        }
    }
    if *stage != 3 {
        return;
    }
    let Some(road) = zone
        .map
        .roads
        .iter()
        .find(|r| r.name == super::plough::ROAD)
    else {
        warn!("gallery: no {} here", super::plough::ROAD);
        return;
    };
    let points = road.points.clone();
    // The road's nearest point to the camera.
    let len = rules::length(&points);
    let mut best = (0.0, f32::MAX);
    let mut s = 0.0;
    while s < len {
        let (p, _) = rules::along(&points, s);
        let d = Vec2::new(p[0] - cam.x, p[1] - cam.z).length();
        if d < best.1 {
            best = (s, d);
        }
        s += 1.0;
    }
    let here = best.0;
    let after = state.contains("after");
    let start = if after { here + 25.0 } else { here - 30.0 };
    let e = super::plough::spawn_plough(&mut commands, &zone, points.clone());
    *held = Some((e, start.max(0.0)));
    commands.queue(move |w: &mut World| {
        if let Some(mut p) = w.get_mut::<Plough>(e) {
            p.run.leg = rules::Leg::Up(start.max(0.0));
            p.banked = start.max(0.0);
            p.needs_snow = false;
            // Stopped for the picture (its plume and auger still going).
            p.jammed = 0.0;
        }
    });
    let (tin, _) = rules::along(&points, here);
    let now = time.elapsed_secs();
    if after {
        // What it left: the lane and the bank from where it came to here,
        // and the tin under it.
        let mut s = (here - 40.0).max(0.0);
        while s < start {
            let (p, d) = rules::along(&points, s);
            let g = zone.on_ground(p[0], p[1]);
            let right = Vec3::new(-d[1], 0.0, d[0]);
            let lo = |v: Vec3| {
                let q = g + v * rules::BANK_OUT;
                zone.ground(q.x, q.z)
            };
            let side = right * rules::bank_side(lo(-right), lo(right));
            let b = g + side * rules::BANK_OUT;
            let rot = Quat::from_rotation_y(
                -extraction_core::sentry::bearing([0.0, 0.0], d).to_radians(),
            );
            let (next, _) = rules::along(&points, s + BANK_STEP);
            lanes.clear(Vec2::new(p[0], p[1]), Vec2::new(next[0], next[1]), now);
            commands.spawn((
                BankPiece,
                Transform::from_translation(zone.on_ground(b.x, b.z)).with_rotation(rot),
                Visibility::default(),
            ));
            s += BANK_STEP;
        }
    } else if let Some(kind) = catalog.kind("canned_stew") {
        crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(
                    zone.on_ground(tin[0], tin[1]) + Vec3::Y * 0.1,
                ),
                condition: 1.0,
            },
        );
    }
}
