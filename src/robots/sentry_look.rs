//! How a sentry looks (client only): dressed from
//! `assets/models/robots/sentry.glb` (`art/robots/sentry.py`) in three
//! LODs. Its strips glow cyan and its lens red while it has power; they go
//! dark when its cable's cut or it's wrecked. The chirp flares its lens.
//! At night (and in the dark of a storm) its searchlight burns: a
//! spotlight down its line of sight, so it gets the lit-air cone
//! (`world::beams`) and a halo (`world::halos`) like any lamp, and you
//! can watch it sweep.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::sentry as rules;

use super::heavy_look::{lod_name, lod_range};
use super::sentry::{CableLength, MAST, OfSentry, Sentry, SentryAct, SentryEvent, SentryPiece};
use super::{PartRole, Robot, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

const MODEL: &str = "models/robots/sentry.glb";
/// The searchlight on its brow, local to the head.
pub const LAMP: Vec3 = Vec3::new(0.17, 0.13, -0.27);
/// Its searchlight, lumens.
pub const LAMP_LUMENS: f32 = 600_000.0;

pub struct SentryLookPlugin;

impl Plugin for SentryLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (dress_sentries, light_sentries, searchlights, gallery),
        );
    }
}

#[derive(Component)]
struct Dressed;

/// A sentry's own lights.
#[derive(Component)]
struct Lights {
    glow: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
    lamp: Handle<StandardMaterial>,
    /// Seconds of the chirp's flare left.
    flare: f32,
}

/// Its searchlight (a child of the head).
#[derive(Component)]
struct Searchlight;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress_sentries(
    mut commands: Commands,
    heads: Query<(Entity, &Children), (With<Sentry>, Without<Dressed>)>,
    pieces: Query<(Entity, &SentryPiece, &OfSentry, Option<&CableLength>), Without<Dressed>>,
    parts: Query<(&RobotPart, &Transform)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lights: Query<&Lights>,
) {
    if heads.is_empty() && pieces.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("sentry model failed to load");
            for (e, _) in &heads {
                commands.entity(e).insert(Dressed);
            }
            for (e, ..) in &pieces {
                commands.entity(e).insert(Dressed);
            }
            return;
        }
    };
    let base_glow = model_mats.get("Glow", &mut lib, &server, &mut materials);
    let base_eye = model_mats.get("Eye", &mut lib, &server, &mut materials);
    let base_lamp = model_mats.get("Lamp", &mut lib, &server, &mut materials);
    let mut hang = |commands: &mut Commands,
                    materials: &mut Assets<StandardMaterial>,
                    own: Option<&Lights>,
                    parent: Entity,
                    name: &str,
                    at: Transform| {
        for lod in 0..3 {
            let Some(node) = ModelNode::find(&model, &lod_name(name, lod)) else {
                continue;
            };
            let holder = commands
                .spawn((at, Visibility::default(), ChildOf(parent)))
                .id();
            for p in &node.primitives {
                let m = match (p.material.as_deref(), own) {
                    (Some("Glow"), Some(l)) => l.glow.clone(),
                    (Some("Eye"), Some(l)) => l.eye.clone(),
                    (Some("Lamp"), Some(l)) => l.lamp.clone(),
                    (Some("Shell"), _) => {
                        let base = model_mats.get("Shell", &mut lib, &server, materials);
                        model_mats.derived("RobotShell", materials, &base, |m| {
                            m.emissive = LinearRgba::rgb(0.22, 0.22, 0.23);
                        })
                    }
                    (Some(n), _) => model_mats.get(n, &mut lib, &server, materials),
                    (None, _) => model_mats.get("Dark", &mut lib, &server, materials),
                };
                let mut ec =
                    commands.spawn((Mesh3d(p.mesh.clone()), MeshMaterial3d(m), ChildOf(holder)));
                if name != "cable" {
                    if let Some(r) = lod_range(lod) {
                        ec.insert(r);
                    }
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
    for (head, children) in &heads {
        let own = |m: &Handle<StandardMaterial>, materials: &mut Assets<StandardMaterial>| {
            let copy = materials.get(m).cloned().unwrap_or_default();
            materials.add(copy)
        };
        let (glow, eye, lamp) = (base_glow.clone(), base_eye.clone(), base_lamp.clone());
        let l = Lights {
            glow: own(&glow, &mut materials),
            eye: own(&eye, &mut materials),
            lamp: own(&lamp, &mut materials),
            flare: 0.0,
        };
        hang(
            &mut commands,
            &mut materials,
            Some(&l),
            head,
            "head",
            Transform::IDENTITY,
        );
        hang(
            &mut commands,
            &mut materials,
            Some(&l),
            head,
            "lamp",
            Transform::from_translation(LAMP),
        );
        for c in children.iter() {
            let Ok((part, pt)) = parts.get(c) else {
                continue;
            };
            // The part sits at its offset already.
            match part.role {
                PartRole::Sensor => hang(
                    &mut commands,
                    &mut materials,
                    Some(&l),
                    c,
                    "eye",
                    Transform::IDENTITY,
                ),
                PartRole::Weapon => hang(
                    &mut commands,
                    &mut materials,
                    Some(&l),
                    c,
                    "gun",
                    Transform::IDENTITY,
                ),
                _ => {}
            }
            let _ = pt;
        }
        commands.entity(head).insert((l, Dressed));
    }
    for (e, piece, of, len) in &pieces {
        let own = lights.get(of.0).ok();
        match piece {
            SentryPiece::Mast => hang(
                &mut commands,
                &mut materials,
                own,
                e,
                "mast",
                // (The mast's collider sits half way up; the model's foot.)
                Transform::from_xyz(0.0, -(MAST * 0.5 - 0.1), 0.0),
            ),
            SentryPiece::Box => hang(
                &mut commands,
                &mut materials,
                own,
                e,
                "box",
                Transform::IDENTITY,
            ),
            SentryPiece::Cable => hang(
                &mut commands,
                &mut materials,
                own,
                e,
                "cable",
                Transform::from_scale(Vec3::new(1.0, 1.0, len.map_or(1.0, |l| l.0))),
            ),
        }
        commands.entity(e).insert(Dressed);
    }
}

/// Powered: strips cyan, lens red, lamp lit; the chirp flares the lens.
/// Without power, or wrecked: dark.
fn light_sentries(
    mut sentries: Query<(Entity, &Sentry, &Robot, &mut Lights)>,
    mut events: MessageReader<SentryEvent>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let chirps: Vec<Entity> = events
        .read()
        .filter(|e| e.what == SentryAct::Chirp)
        .map(|e| e.sentry)
        .collect();
    for (e, s, robot, mut l) in &mut sentries {
        if chirps.contains(&e) {
            l.flare = rules::CHIRP;
        }
        l.flare = (l.flare - dt).max(0.0);
        let on = s.powered && !robot.wrecked;
        let (glow, eye, lamp) = if on {
            let f = 1.0 + 4.0 * l.flare / rules::CHIRP;
            (
                LinearRgba::rgb(1.2, 4.8, 6.0),
                LinearRgba::rgb(9.0 * f, 1.0, 0.5),
                LinearRgba::rgb(20.0, 18.0, 14.0),
            )
        } else {
            (LinearRgba::BLACK, LinearRgba::BLACK, LinearRgba::BLACK)
        };
        for (h, want) in [(&l.glow, glow), (&l.eye, eye), (&l.lamp, lamp)] {
            if let Some(mut m) = materials.get_mut(h) {
                if m.emissive != want {
                    m.emissive = want;
                }
            }
        }
    }
}

/// At night, a powered sentry's searchlight burns down its line of sight.
fn searchlights(
    mut commands: Commands,
    sentries: Query<(Entity, &Sentry, &Robot, Option<&Children>)>,
    lamps: Query<(), With<Searchlight>>,
    conditions: Res<crate::world::weather::Conditions>,
) {
    let night = conditions.light < 0.5;
    for (e, s, robot, children) in &sentries {
        let lamp = children
            .into_iter()
            .flatten()
            .copied()
            .find(|c| lamps.contains(*c));
        let want = night && s.powered && !robot.wrecked;
        match (want, lamp) {
            (true, None) => {
                commands.spawn((
                    Name::new("Searchlight"),
                    Searchlight,
                    SpotLight {
                        color: Color::srgb(1.0, 0.96, 0.88),
                        intensity: LAMP_LUMENS,
                        range: rules::BEAM_REACH,
                        outer_angle: rules::BEAM_HALF.to_radians(),
                        inner_angle: rules::BEAM_HALF.to_radians() * 0.6,
                        ..default()
                    },
                    crate::world::halos::Halo {
                        color: LinearRgba::rgb(1.0, 0.95, 0.85),
                        radius: crate::world::halos::halo_radius(LAMP_LUMENS * 0.05),
                    },
                    NotShadowCaster,
                    Transform::from_translation(LAMP + Vec3::NEG_Z * 0.06),
                    ChildOf(e),
                ));
            }
            (false, Some(l)) => {
                commands.entity(l).despawn();
            }
            _ => {}
        }
    }
}

/// `--gallery sentry[:STATE[+STATE..]][,TURN[,DIST]]`: a sentry in front of
/// the `--camera`, its head held looking toward the camera turned by
/// `TURN` degrees (its arc pinned there). `cut` cuts its cable, `down`
/// wrecks it, `walker` stands a walker beside it for scale; `--hour
/// night` shows its searchlight.
///
/// ```text
/// scripts/shot.sh docs/shots/sentry/front.png 40,0,262,0,6 90 --gallery sentry,20,12
/// ```
#[allow(clippy::too_many_arguments)]
fn gallery(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    mut stage: Local<u32>,
    mut made: Local<Option<(Entity, String)>>,
    mut robots: Query<&mut Robot>,
    cables: Query<(Entity, &SentryPiece, &OfSentry)>,
    mut cuts: MessageWriter<crate::interact::tools::cutters::Cut>,
) {
    let (Some(spec), Some((cam, yaw, _)), Some(zone)) = (&opts.gallery, opts.camera, zone) else {
        return;
    };
    let mut fields = spec.split(',');
    let first = fields.next().unwrap_or("");
    let Some(state) = first.strip_prefix("sentry") else {
        return;
    };
    *stage += 1;
    if *stage == 3 {
        let turn: f32 = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let dist: f32 = fields.next().and_then(|v| v.parse().ok()).unwrap_or(12.0);
        let fwd = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::NEG_Z;
        let at = Vec2::new(cam.x + fwd.x * dist, cam.z + fwd.z * dist);
        let toward_cam = rules::bearing([at.x, at.y], [cam.x, cam.z]);
        let side = Vec3::new(-fwd.z, 0.0, fwd.x);
        let power = at + Vec2::new(side.x, side.z) * 6.0 - Vec2::new(fwd.x, fwd.z) * 4.0;
        let e =
            super::sentry::spawn_sentry(&mut commands, &zone, at, toward_cam + turn, 0.0, power);
        if state.contains("walker") {
            let w_at = zone.on_ground(at.x - side.x * 4.0, at.y - side.z * 4.0);
            let w = super::walker::spawn_walker(
                &mut commands,
                &zone,
                w_at,
                super::Job {
                    kind: super::JobKind::Survey,
                    site: w_at,
                    duration: 1.0e6,
                    exit: w_at,
                },
            );
            commands
                .entity(w)
                .remove::<super::Senses>()
                .insert(super::Brain::Working {
                    at: w_at,
                    left: 1.0e6,
                });
        }
        commands.entity(e).remove::<super::Senses>();
        *made = Some((e, state.trim_start_matches(':').to_owned()));
    }
    let Some((e, state)) = made.as_ref() else {
        return;
    };
    let has = |s: &str| state.split('+').any(|t| t == s);
    if *stage == 30 {
        if has("down") {
            if let Ok(mut r) = robots.get_mut(*e) {
                r.wrecked = true;
            }
        }
        if has("cut") {
            if let Some((c, ..)) = cables
                .iter()
                .find(|(_, p, o)| matches!(p, SentryPiece::Cable) && o.0 == *e)
            {
                cuts.write(crate::interact::tools::cutters::Cut {
                    target: c,
                    by: Entity::PLACEHOLDER,
                });
            }
        }
    }
}
