//! `--sweep FILTER`: the "looks wrong" sweep. Every model under
//! `assets/models/` and every generated prefab is put alone in a studio
//! high over the valley (a grey floor, the valley's sky) and shot from
//! eight angles at once (a 4 x 2 grid of viewports) under three lights:
//! the clearing day, dusk, and night with a lamp. Each shot goes to
//! `<capture-out>/<light>/<subject>.png`; `sweep.json` says what each
//! subject measured.
//!
//! The day is shot a second time with the subject hidden, to tell it from
//! the studio; time stands still while a subject is photographed.
//! `scripts/capture_sweep.py` looks for fallback colours, blank views,
//! absurd sizes, blockouts left standing and floating pieces, and writes
//! the report. Z-fighting is found from the geometry instead
//! (`capture::fighting`): a still picture doesn't show it.
//!
//! FILTER is `all`, `models`, `prefabs`, or text a subject's name must
//! contain (`props/bed`, `prefab:barn`). `--sweep-lights day` shoots only
//! the day.

use bevy::camera::Viewport;
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use std::time::Duration;

use crate::launch::LaunchOptions;
use crate::models::{ModelMaterials, ModelNode, ModelState, Models};
use crate::render::FreeCamera;

pub struct SweepPlugin;

impl Plugin for SweepPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sweep>()
            .add_systems(Update, (sweep, match_look).chain().run_if(sweep_run));
    }
}

fn sweep_run(opts: Res<LaunchOptions>) -> bool {
    opts.capture.sweep.is_some()
}

/// Where the studio is: high over the valley, out of everything's way.
pub const STUDIO: Vec3 = Vec3::new(0.0, 900.0, 0.0);
/// The grid of views: columns, rows.
pub const GRID: (u32, u32) = (4, 2);
/// The cameras' field of view, vertical, degrees.
pub const FOV: f32 = 40.0;
/// The cameras' near plane.
pub const NEAR: f32 = 0.03;
/// How much further than the weather's the sweep's cameras see.
const CLEAR_AIR: f32 = 8.0;
/// How far under the subjects the studio floor's top is, metres.
pub const FLOOR_GAP: f32 = 0.004;

/// The eight views: compass azimuth and elevation, degrees.
pub fn angles() -> [(f32, f32); 8] {
    let mut a = [(0.0, 0.0); 8];
    for (i, v) in a.iter_mut().enumerate() {
        *v = (i as f32 * 45.0 + 20.0, if i % 2 == 0 { 14.0 } else { 38.0 });
    }
    a
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// `<folder>/<name>` under `assets/models/`.
    Model(String),
    /// A generated prefab (buildings and underground sites).
    Prefab(String),
    /// A test piece that fights on purpose (`probe`, never in `all`): a
    /// block with a differently coloured sheet lying exactly on its top,
    /// and one 1 mm above it, to show the z-fighting check works.
    Probe,
}

impl Subject {
    pub fn name(&self) -> String {
        match self {
            Subject::Model(m) => m.clone(),
            Subject::Prefab(p) => format!("prefab:{p}"),
            Subject::Probe => "probe:zfight".into(),
        }
    }

    pub fn file(&self) -> String {
        self.name().replace([':', '/'], "__")
    }
}

/// Every subject the filter keeps.
pub fn subjects(filter: &str) -> Vec<Subject> {
    let mut out: Vec<Subject> = super::glb::all_models()
        .into_iter()
        .map(|(f, n, _)| Subject::Model(format!("{f}/{n}")))
        .collect();
    out.extend(
        crate::world::buildings::PREFABS
            .into_iter()
            .chain(crate::world::underground::ids())
            .map(|p| Subject::Prefab(p.to_string())),
    );
    out.push(Subject::Probe);
    out.retain(|s| match filter {
        _ if *s == Subject::Probe => filter.split(',').any(|f| f == "probe"),
        "all" => true,
        "models" => matches!(s, Subject::Model(_)),
        "prefabs" => matches!(s, Subject::Prefab(_)),
        f => f.split(',').any(|f| s.name().contains(f)),
    });
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Light {
    Day,
    Dusk,
    Night,
}

impl Light {
    fn dir(self) -> &'static str {
        match self {
            Light::Day => "day",
            Light::Dusk => "dusk",
            Light::Night => "night",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct Record {
    subject: String,
    file: String,
    kind: &'static str,
    /// The subject's bounds over the studio floor, metres.
    lo: [f32; 3],
    hi: [f32; 3],
    meshes: usize,
    /// Blockout boxes still standing when it was shot (a model that never
    /// arrived).
    blockouts: usize,
    /// Meshes touching no other mesh and off the floor, by name.
    floating: Vec<String>,
    /// Frames it took to load.
    load_frames: u32,
    /// Meshes spawned with it outside its hierarchy (a building's crates,
    /// window boards, wreckage), cleared when it went.
    left_behind: usize,
    left_behind_names: Vec<String>,
    failed: bool,
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
enum Stage {
    #[default]
    Start,
    /// The studio's first frames: its floor and cameras come up.
    Warm,
    Spawn,
    Load,
    Shoot,
    Waiting,
    Next,
}

#[derive(Resource, Default)]
struct Sweep {
    subjects: Vec<Subject>,
    lights: Vec<Light>,
    light: usize,
    i: usize,
    stage: Stage,
    root: Option<Entity>,
    floor: Option<Entity>,
    lamp: Option<Entity>,
    extra: Vec<Entity>,
    frames: u32,
    stable: (usize, u32),
    /// Pictures to take of this subject under this light, by file suffix.
    shots: Vec<&'static str>,
    records: Vec<Record>,
    /// The subject is hidden for its `_empty` picture.
    hidden: bool,
    /// The light the studio last settled into.
    warmed: Option<usize>,
    /// The meshes near the studio before the first subject (the sky dome
    /// and the like), not any subject's.
    baseline: Option<std::collections::HashSet<Entity>>,
    started: Option<std::time::Instant>,
}

/// Meshes of the scene round the camera, not of any subject (the sky, the
/// renderer's warm-up, the sun's shafts, and at night the moths round the
/// lamp, which wildlife spawns afresh whenever they're cleared: the night
/// sweep once cleared them for ever and never moved on).
const SCENE_MESHES: [&str; 4] = ["Sky dome", "Warm-up shelf", "Light shafts", "Moth"];

/// Frames the studio settles for when it opens and at each new light
/// (the sky and fog ease into a new weather).
const WARM: u32 = 90;
/// Frames a subject may take to load before it's shot as it is.
const LOAD_LIMIT: u32 = 600;
/// Frames the meshes must hold steady (count and bounds) before shooting.
const STEADY: u32 = 4;

#[allow(clippy::too_many_arguments)]
fn sweep(
    mut commands: Commands,
    mut opts: ResMut<LaunchOptions>,
    mut state: ResMut<Sweep>,
    zone: Option<Res<crate::world::Zone>>,
    mut cams: Query<(Entity, &mut Transform, &mut Camera, &mut Projection), With<FreeCamera>>,
    mut extra_cams: Query<
        (&mut Transform, &mut Camera, &mut Projection),
        (With<SweepCamera>, Without<FreeCamera>),
    >,
    windows: Query<&Window>,
    children: Query<&Children>,
    meshes_q: Query<(&GlobalTransform, Option<&Aabb>, Option<&Name>), With<Mesh3d>>,
    (names, parents, strays): (
        Query<&Name>,
        Query<&ChildOf>,
        Query<(Entity, &GlobalTransform, Option<&Name>), With<Mesh3d>>,
    ),
    blockouts: Query<(), With<crate::render::world_props::Blockout>>,
    pending: Query<
        (),
        Or<(
            With<crate::world::buildings::PropsPending>,
            With<crate::render::world_props::SwapIn>,
        )>,
    >,
    mut lamps: Query<&mut Transform, (With<SweepLamp>, Without<FreeCamera>, Without<SweepCamera>)>,
    (
        mut models,
        mut model_mats,
        server,
        gltfs,
        gnodes,
        gmeshes,
        mut lib,
        mut materials,
        mut meshes,
    ): (
        ResMut<Models>,
        ResMut<ModelMaterials>,
        Res<AssetServer>,
        Res<Assets<bevy::gltf::Gltf>>,
        Res<Assets<bevy::gltf::GltfNode>>,
        Res<Assets<bevy::gltf::GltfMesh>>,
        ResMut<crate::render::materials::MaterialLibrary>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<Assets<Mesh>>,
    ),
    (mut exit, mut time): (MessageWriter<AppExit>, ResMut<Time<Virtual>>),
    mut boot: Local<u32>,
) {
    let Some(_zone) = zone else {
        return;
    };
    *boot += 1;
    if *boot == 1 {
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(1.0 / 30.0),
        ));
    }
    // Let the valley come up first.
    if *boot < opts.frames {
        return;
    }
    let Ok((cam_e, mut cam_tf, mut cam, mut proj)) = cams.single_mut() else {
        return;
    };
    let out = opts.capture.out_dir();
    let st = &mut *state;
    // Time stands still while a subject is photographed, so its pictures
    // differ only in what was changed on purpose (the clouds drift).
    let still = matches!(st.stage, Stage::Shoot | Stage::Waiting) && st.frames >= 2;
    if still != time.is_paused() {
        if still {
            time.pause();
        } else {
            time.unpause();
        }
    }
    match st.stage {
        Stage::Start => {
            let filter = opts.capture.sweep.clone().unwrap_or_default();
            st.subjects = subjects(&filter);
            st.lights = match opts.capture.sweep_lights.as_deref() {
                Some(l) => l
                    .split(',')
                    .filter_map(|l| match l {
                        "day" => Some(Light::Day),
                        "dusk" => Some(Light::Dusk),
                        "night" => Some(Light::Night),
                        _ => None,
                    })
                    .collect(),
                None => vec![Light::Day, Light::Dusk, Light::Night],
            };
            st.started = Some(std::time::Instant::now());
            info!(
                "sweep: {} subjects under {} lights",
                st.subjects.len(),
                st.lights.len()
            );
            // The studio floor: a broad grey disc.
            let floor = commands
                .spawn((
                    Name::new("Sweep floor"),
                    Mesh3d(meshes.add(Cylinder::new(3000.0, 0.2))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.42, 0.42, 0.40),
                        perceptual_roughness: 0.95,
                        ..default()
                    })),
                    // Its top a few millimetres under the subjects, so a
                    // rug or a film of ash lying flat doesn't fight it.
                    Transform::from_translation(STUDIO - Vec3::Y * (0.1 + FLOOR_GAP)),
                ))
                .id();
            st.floor = Some(floor);
            // A lamp for the night, off till then.
            st.lamp = Some(
                commands
                    .spawn((
                        SweepLamp,
                        PointLight {
                            color: Color::srgb(1.0, 0.9, 0.75),
                            intensity: 0.0,
                            range: 200.0,
                            shadow_maps_enabled: true,
                            ..default()
                        },
                        Transform::from_translation(STUDIO + Vec3::new(3.0, 4.0, 3.0)),
                    ))
                    .id(),
            );
            // Seven more cameras beside the free one, each its own cell.
            let (w, h) = windows
                .iter()
                .next()
                .map_or((1280, 640), |w| (w.physical_width(), w.physical_height()));
            let (cw, ch) = (w / GRID.0, h / GRID.1);
            let cell = |i: u32| Viewport {
                physical_position: UVec2::new((i % GRID.0) * cw, (i / GRID.0) * ch),
                physical_size: UVec2::new(cw, ch),
                ..default()
            };
            cam.viewport = Some(cell(0));
            // The haze is the valley's, not the subject's: a big subject
            // framed from 200 m off was lost in it.
            commands
                .entity(cam_e)
                .insert((super::Lens(FOV.to_radians()), super::ClearAir(CLEAR_AIR)));
            let weather = crate::world::sky::Weather::default();
            for i in 1..8 {
                st.extra.push(
                    commands
                        .spawn((
                            Name::new(format!("Sweep camera {i}")),
                            SweepCamera,
                            super::Lens(FOV.to_radians()),
                            super::ClearAir(CLEAR_AIR),
                            Camera {
                                viewport: Some(cell(i)),
                                order: i as isize,
                                ..default()
                            },
                            Transform::default(),
                            crate::render::camera_bundle(&weather),
                        ))
                        .id(),
                );
            }
            st.frames = 0;
            st.stage = Stage::Warm;
        }
        Stage::Warm => {
            st.frames += 1;
            if st.frames >= WARM {
                st.stage = Stage::Spawn;
            }
        }
        Stage::Spawn => {
            let Some(light) = st.lights.get(st.light).copied() else {
                finish(st, &out, &mut exit);
                return;
            };
            // Whatever the last subject spawned outside its own hierarchy
            // (a building's crates, window boards and wreckage are world
            // entities) is cleared before the next. The scene's own meshes
            // near the camera (the sky dome and so on) aren't anyone's.
            let near: Vec<(Entity, String)> = strays
                .iter()
                .filter(|(e, g, _)| {
                    Some(*e) != st.floor && g.translation().distance(STUDIO) < 400.0
                })
                .map(|(e, _, n)| (e, n.map_or("unnamed".into(), |n| n.as_str().to_string())))
                .collect();
            let left: Vec<(Entity, String)> = match &mut st.baseline {
                Some(base) => near
                    .into_iter()
                    .filter(|(e, n)| {
                        // The scene's own, whenever they turn up.
                        if SCENE_MESHES.contains(&n.as_str()) {
                            base.insert(*e);
                        }
                        !base.contains(e)
                    })
                    .collect(),
                None => {
                    st.baseline = Some(near.into_iter().map(|(e, _)| e).collect());
                    Vec::new()
                }
            };
            if !left.is_empty() {
                // Named by their parent's name where they have no name of
                // their own (a leak's holder says where it came from).
                let label = |e: Entity, n: &str| {
                    if n != "unnamed" {
                        return n.to_string();
                    }
                    match parents.get(e) {
                        Ok(p) => format!(
                            "child of {}",
                            names
                                .get(p.parent())
                                .map_or("an unnamed entity", |n| n.as_str())
                        ),
                        Err(_) => "unparented".into(),
                    }
                };
                let mut kinds: Vec<String> = left.iter().map(|(e, n)| label(*e, n)).collect();
                kinds.sort();
                kinds.dedup();
                info!(
                    "sweep: cleared {}'s {} loose meshes: {}",
                    st.records.last().map_or("?", |r| r.subject.as_str()),
                    left.len(),
                    kinds.join(", ")
                );
                if let Some(r) = st.records.last_mut().filter(|_| st.light == 0) {
                    r.left_behind = left.len();
                    r.left_behind_names = kinds;
                }
                for (e, _) in left {
                    commands.entity(e).despawn();
                }
                return;
            }
            let Some(subject) = st.subjects.get(st.i).cloned() else {
                st.light += 1;
                st.i = 0;
                return;
            };
            if st.i == 0 && st.warmed != Some(st.light) {
                // A new light: the hour and weather, then time for the sky
                // and fog to settle into them before the first subject.
                let (hour, weather) = match light {
                    Light::Day => ("day", "clearing"),
                    Light::Dusk => ("dusk", "clearing"),
                    Light::Night => ("night", "clearing"),
                };
                opts.hour = Some(hour.into());
                opts.weather = Some(weather.into());
                let _ = std::fs::create_dir_all(out.join(light.dir()));
                st.warmed = Some(st.light);
                st.frames = 0;
                st.stage = Stage::Warm;
                return;
            }
            let root = commands
                .spawn((
                    Name::new(format!("Sweep {}", subject.name())),
                    Transform::from_translation(STUDIO),
                    Visibility::default(),
                ))
                .id();
            match &subject {
                Subject::Model(_) => {}
                Subject::Probe => {
                    let mut mat = |c: Color| {
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: c,
                            perceptual_roughness: 0.8,
                            ..default()
                        }))
                    };
                    commands.spawn((
                        Name::new("block"),
                        Mesh3d(meshes.add(Cuboid::new(1.0, 0.5, 1.0))),
                        mat(Color::srgb(0.7, 0.7, 0.68)),
                        Transform::from_xyz(0.0, 0.25, 0.0),
                        ChildOf(root),
                    ));
                    // On the top face exactly: fights.
                    commands.spawn((
                        Name::new("sheet on the face"),
                        Mesh3d(meshes.add(Plane3d::default().mesh().size(0.6, 0.6))),
                        mat(Color::srgb(0.75, 0.15, 0.1)),
                        Transform::from_xyz(-0.15, 0.5, -0.15)
                            .with_rotation(Quat::from_rotation_y(0.3)),
                        ChildOf(root),
                    ));
                    // A millimetre proud of it: from a few metres, still fights.
                    commands.spawn((
                        Name::new("sheet 1 mm up"),
                        Mesh3d(meshes.add(Plane3d::default().mesh().size(0.3, 0.3))),
                        mat(Color::srgb(0.1, 0.2, 0.7)),
                        Transform::from_xyz(0.25, 0.501, 0.25),
                        ChildOf(root),
                    ));
                }
                Subject::Prefab(p) => {
                    commands
                        .entity(root)
                        .insert(crate::world::buildings::Building { prefab: p.clone() });
                    let geo = crate::world::buildings::prefab(p);
                    for d in geo.iter().flat_map(|g| g.doors.iter()) {
                        commands.spawn((
                            Name::new("Door"),
                            crate::world::buildings::DoorLeaf(*d),
                            Transform::from_translation(d.hinge)
                                .with_rotation(Quat::from_rotation_y(d.yaw)),
                            Visibility::default(),
                            ChildOf(root),
                        ));
                    }
                }
            }
            st.root = Some(root);
            st.hidden = false;
            st.frames = 0;
            st.stable = (0, 0);
            st.stage = Stage::Load;
        }
        Stage::Load => {
            let root = st.root.unwrap();
            let subject = st.subjects[st.i].clone();
            st.frames += 1;
            let mut failed = false;
            if let Subject::Model(m) = &subject {
                // Spawned once its file has loaded.
                if children.get(root).is_err() {
                    let path = format!("models/{m}.glb");
                    match models.request(&path, &server) {
                        None => failed = true,
                        Some(h) => match Models::state(&h, &server, &gltfs) {
                            ModelState::Loading if st.frames < LOAD_LIMIT => return,
                            ModelState::Loading | ModelState::Failed => failed = true,
                            ModelState::Ready(g) => {
                                let nodes = crate::models::model_nodes(g, &gnodes, &gmeshes);
                                spawn_nodes(&mut commands, root, &nodes, &mut |name| {
                                    model_mats.get(
                                        name.unwrap_or("Paint"),
                                        &mut lib,
                                        &server,
                                        &mut materials,
                                    )
                                });
                                return;
                            }
                        },
                    }
                }
            }
            // Everything under the root with a mesh, once it has bounds.
            let mut all = Vec::new();
            collect(root, &children, &mut all);
            let busy = all.iter().any(|e| pending.contains(*e));
            let mesh_list: Vec<(Entity, Vec3, Vec3)> = all
                .iter()
                .filter_map(|e| {
                    let (gt, aabb, _) = meshes_q.get(*e).ok()?;
                    let aabb = aabb?;
                    let (lo, hi) = world_aabb(gt, aabb);
                    Some((*e, lo, hi))
                })
                .collect();
            let unbounded = all
                .iter()
                .filter(|e| meshes_q.get(**e).is_ok_and(|m| m.1.is_none()))
                .count();
            let n = mesh_list.len();
            if st.stable.0 == n && n > 0 && !busy && unbounded == 0 {
                st.stable.1 += 1;
            } else {
                st.stable = (n, 0);
            }
            if st.stable.1 < STEADY && st.frames < LOAD_LIMIT && !failed {
                return;
            }
            let Some((lo, hi)) = mesh_list
                .iter()
                .map(|m| (m.1, m.2))
                .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            else {
                warn!("sweep: {} has nothing to show", subject.name());
                if st.light == 0 {
                    st.records.push(record(
                        &subject,
                        Vec3::ZERO,
                        Vec3::ZERO,
                        0,
                        0,
                        vec![],
                        st.frames,
                        true,
                    ));
                }
                st.stage = Stage::Next;
                return;
            };
            // A model sits on the floor, its lowest point at the studio's. A
            // building stands where it was built: its origin is its footing,
            // the floor its ground (its foundation buried, as in a valley),
            // and the pieces the game spawns round it in world space
            // (crates, boarded windows, wreckage) stay where they belong.
            // An underground site is nearly all below its footing, which
            // would put it under the floor, so it sits on it like a model.
            let lift = match &subject {
                Subject::Prefab(p) if !crate::world::underground::ids().any(|u| u == p) => 0.0,
                _ => STUDIO.y - lo.y,
            };
            if lift != 0.0 {
                commands
                    .entity(root)
                    .insert(Transform::from_translation(STUDIO + Vec3::Y * lift));
            }
            let (lo, hi) = (
                (lo + Vec3::Y * lift).max(Vec3::new(f32::MIN, STUDIO.y, f32::MIN)),
                hi + Vec3::Y * lift,
            );
            if st.light == 0 {
                let blocks = all.iter().filter(|e| blockouts.contains(**e)).count();
                let label = |e: Entity| {
                    let mut at = e;
                    for _ in 0..4 {
                        if let Ok(n) = names.get(at) {
                            return n.as_str().to_string();
                        }
                        match parents.get(at) {
                            Ok(p) => at = p.parent(),
                            Err(_) => break,
                        }
                    }
                    format!("{e:?}")
                };
                let lifted: Vec<(Entity, Vec3, Vec3)> = mesh_list
                    .iter()
                    .map(|(e, a, b)| (*e, *a + Vec3::Y * lift, *b + Vec3::Y * lift))
                    .collect();
                let floating = floating(&lifted, STUDIO.y)
                    .into_iter()
                    .map(|e| label(e))
                    .collect();
                st.records.push(record(
                    &subject,
                    lo - STUDIO,
                    hi - STUDIO,
                    n,
                    blocks,
                    floating,
                    st.frames,
                    failed,
                ));
            }
            // Frame it: every view sees its bounding sphere whole.
            let centre = (lo + hi) * 0.5;
            let radius = ((hi - lo).length() * 0.5).max(0.05);
            let fov = FOV.to_radians();
            let dist = radius / (fov * 0.5).sin() * 1.05;
            let place = |i: usize, tf: &mut Transform, proj: &mut Projection| {
                let (az, el) = angles()[i];
                let (az, el) = (az.to_radians(), el.to_radians());
                let dir = Vec3::new(az.sin() * el.cos(), el.sin(), -az.cos() * el.cos());
                *tf = Transform::from_translation(centre + dir * dist).looking_at(centre, Vec3::Y);
                *proj = Projection::Perspective(PerspectiveProjection {
                    fov,
                    near: NEAR,
                    far: 4000.0,
                    ..default()
                });
            };
            place(0, &mut cam_tf, &mut proj);
            for (i, e) in st.extra.iter().enumerate() {
                if let Ok((mut tf, _, mut p)) = extra_cams.get_mut(*e) {
                    place(i + 1, &mut tf, &mut p);
                }
            }
            // The night's lamp: up and to one side of the first view.
            if let Some(Ok(mut l)) = st.lamp.map(|l| lamps.get_mut(l)) {
                let (az, _) = angles()[0];
                let side = (az + 50.0).to_radians();
                l.translation = centre
                    + Vec3::new(side.sin(), 0.0, -side.cos()) * radius * 1.6
                    + Vec3::Y * radius * 1.4;
            }
            let night = st.lights[st.light] == Light::Night;
            commands.entity(st.lamp.unwrap()).insert(PointLight {
                color: Color::srgb(1.0, 0.88, 0.7),
                // As bright on every subject: the lamp stands about 2r off,
                // so its power goes with that squared, for a cartridge as
                // for a factory (a floor of 1 m once blew small items out).
                intensity: if night {
                    20_000.0 * (radius * 2.0).powi(2)
                } else {
                    0.0
                },
                range: radius * 8.0 + 10.0,
                radius: 0.05,
                shadow_maps_enabled: true,
                ..default()
            });
            st.shots = if st.lights[st.light] == Light::Day {
                // Then without the subject, for telling it from the studio.
                vec!["", "_empty"]
            } else {
                vec![""]
            };
            // A few frames for the move to settle (shadows, the lamp).
            st.frames = 0;
            st.stage = Stage::Shoot;
        }
        Stage::Shoot => {
            st.frames += 1;
            if st.frames < 3 {
                return;
            }
            let Some(suffix) = st.shots.first().copied() else {
                st.stage = Stage::Next;
                return;
            };
            if suffix == "_empty" {
                if let Some(root) = st.root {
                    if !st.hidden {
                        commands.entity(root).insert(Visibility::Hidden);
                        st.hidden = true;
                        st.frames = 1;
                        return;
                    }
                }
            }
            let light = st.lights[st.light];
            let file = out
                .join(light.dir())
                .join(format!("{}{suffix}.png", st.subjects[st.i].file()));
            let mut save = save_to_disk(file);
            st.shots.remove(0);
            st.stage = Stage::Waiting;
            commands.spawn(Screenshot::primary_window()).observe(
                move |shot: On<ScreenshotCaptured>, mut st: ResMut<Sweep>| {
                    save(shot);
                    st.stage = Stage::Shoot;
                    st.frames = 2;
                },
            );
        }
        Stage::Waiting => {}
        Stage::Next => {
            if let Some(root) = st.root.take() {
                commands.entity(root).despawn();
            }
            st.i += 1;
            if st.i % 10 == 0 && st.light == 0 {
                write_report(st, &out);
            }
            if st.i % 25 == 0 {
                info!(
                    "sweep: {} {}/{} ({:.0} s)",
                    st.lights[st.light].dir(),
                    st.i,
                    st.subjects.len(),
                    st.started.map_or(0.0, |t| t.elapsed().as_secs_f32())
                );
            }
            st.stage = Stage::Spawn;
        }
    }
}

#[derive(Component)]
struct SweepCamera;

#[derive(Component)]
struct SweepLamp;

/// The extra cameras look like the free one: the weather's fog, the grade,
/// the exposure.
fn match_look(
    main: Query<
        (
            &DistanceFog,
            &bevy::render::view::ColorGrading,
            &bevy::camera::Exposure,
        ),
        (With<FreeCamera>, Without<SweepCamera>),
    >,
    mut extra: Query<
        (
            &mut DistanceFog,
            &mut bevy::render::view::ColorGrading,
            &mut bevy::camera::Exposure,
        ),
        With<SweepCamera>,
    >,
) {
    let Ok((fog, grade, exposure)) = main.single() else {
        return;
    };
    for (mut f, mut g, mut e) in &mut extra {
        *f = fog.clone();
        *g = grade.clone();
        *e = *exposure;
    }
}

#[allow(clippy::too_many_arguments)]
fn record(
    s: &Subject,
    lo: Vec3,
    hi: Vec3,
    meshes: usize,
    blockouts: usize,
    floating: Vec<String>,
    load_frames: u32,
    failed: bool,
) -> Record {
    Record {
        subject: s.name(),
        file: s.file(),
        kind: match s {
            Subject::Model(_) => "model",
            Subject::Prefab(_) => "prefab",
            Subject::Probe => "probe",
        },
        lo: lo.to_array(),
        hi: hi.to_array(),
        meshes,
        blockouts,
        floating,
        load_frames,
        failed,
        left_behind: 0,
        left_behind_names: Vec::new(),
    }
}

/// `sweep.json`: what each subject measured. Written as the sweep goes
/// (every ten subjects) as well as at the end, so a run that dies hours in
/// still leaves its findings.
fn write_report(st: &Sweep, out: &std::path::Path) {
    let json = serde_json::json!({
        "grid": [GRID.0, GRID.1],
        "angles": angles(),
        "lights": st.lights.iter().map(|l| l.dir()).collect::<Vec<_>>(),
        "subjects": st.records,
    });
    let _ = std::fs::write(
        out.join("sweep.json"),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    );
}

fn finish(st: &mut Sweep, out: &std::path::Path, exit: &mut MessageWriter<AppExit>) {
    write_report(st, out);
    info!(
        "sweep: done, {} subjects in {:.0} s",
        st.records.len(),
        st.started.map_or(0.0, |t| t.elapsed().as_secs_f32())
    );
    exit.write(AppExit::Success);
}

/// A model's nodes under `parent`, as the game would place them, leaving
/// out far stand-ins (`_lod` nodes), which the game only draws far off.
fn spawn_nodes(
    commands: &mut Commands,
    parent: Entity,
    nodes: &[ModelNode],
    material: &mut dyn FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    for n in nodes {
        if n.name.contains("_lod") {
            continue;
        }
        let e = commands
            .spawn((
                Name::new(n.name.clone()),
                n.transform,
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        crate::models::spawn_primitives(commands, e, n, |m| material(m));
        spawn_nodes(commands, e, &n.children, material);
    }
}

fn collect(e: Entity, children: &Query<&Children>, out: &mut Vec<Entity>) {
    out.push(e);
    if let Ok(c) = children.get(e) {
        for c in c.iter() {
            collect(c, children, out);
        }
    }
}

fn world_aabb(gt: &GlobalTransform, aabb: &Aabb) -> (Vec3, Vec3) {
    let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for k in 0..8 {
        let p = c + h * Vec3::new(
            if k & 1 == 0 { -1.0 } else { 1.0 },
            if k & 2 == 0 { -1.0 } else { 1.0 },
            if k & 4 == 0 { -1.0 } else { 1.0 },
        );
        let w = gt.transform_point(p);
        lo = lo.min(w);
        hi = hi.max(w);
    }
    (lo, hi)
}

/// How near two boxes must come to count as touching, metres.
pub const TOUCH: f32 = 0.004;

/// Meshes whose boxes touch no other mesh's box (nor the floor at
/// `floor`): pieces hanging in the air. Boxes are generous (a box touches
/// more than its mesh does), so this misses pieces near others but never
/// flags one that really touches.
pub fn floating<T: Copy>(boxes: &[(T, Vec3, Vec3)], floor: f32) -> Vec<T> {
    let near = |a: &(T, Vec3, Vec3), b: &(T, Vec3, Vec3)| {
        (0..3).all(|k| a.1[k] <= b.2[k] + TOUCH && b.1[k] <= a.2[k] + TOUCH)
    };
    // Grounded: on the floor, or touching something grounded.
    let n = boxes.len();
    let mut grounded: Vec<bool> = boxes.iter().map(|b| b.1.y <= floor + TOUCH).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            if grounded[i] {
                continue;
            }
            if (0..n).any(|j| grounded[j] && near(&boxes[i], &boxes[j])) {
                grounded[i] = true;
                changed = true;
            }
        }
    }
    boxes
        .iter()
        .zip(grounded)
        .filter(|(_, g)| !g)
        .map(|(b, _)| b.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_finds_what_hangs_free() {
        let b = |id: u32, lo: [f32; 3], hi: [f32; 3]| (id, Vec3::from(lo), Vec3::from(hi));
        let boxes = [
            b(0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            // On top of the first.
            b(1, [0.2, 1.0, 0.2], [0.8, 1.5, 0.8]),
            // Hung off the second's side.
            b(2, [0.8, 1.2, 0.3], [1.2, 1.4, 0.5]),
            // In the air, nothing near.
            b(3, [3.0, 2.0, 0.0], [3.5, 2.5, 0.5]),
            // A ladder of two in the air touching each other only.
            b(4, [5.0, 2.0, 0.0], [5.5, 2.5, 0.5]),
            b(5, [5.5, 2.0, 0.0], [6.0, 2.5, 0.5]),
        ];
        assert_eq!(floating(&boxes, 0.0), vec![3, 4, 5]);
    }

    #[test]
    fn subjects_cover_models_and_prefabs() {
        let all = subjects("all");
        assert!(all.iter().any(|s| *s == Subject::Model("guns/ak74".into())));
        assert!(all.iter().any(|s| *s == Subject::Prefab("barn".into())));
        assert!(
            subjects("prefabs")
                .iter()
                .all(|s| matches!(s, Subject::Prefab(_)))
        );
        assert_eq!(
            subjects("guns/ak74"),
            vec![Subject::Model("guns/ak74".into())]
        );
        // File names are flat.
        assert!(all.iter().all(|s| !s.file().contains('/')));
        // The probe only when asked for.
        assert!(!all.contains(&Subject::Probe));
        assert_eq!(subjects("probe"), vec![Subject::Probe]);
    }
}
