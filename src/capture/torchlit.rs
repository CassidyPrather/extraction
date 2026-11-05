//! `--torchlit FLOORS`: every item by torchlight, as most looting is done:
//! lying on a floor in a dark room, lit only by the hand torch held at
//! the eye (`gadgets::visuals::torch_beam`). For each floor (`wood`,
//! `concrete`, `ruin`: a burnt house's charred planks) and each eye
//! (an arm's length off, crouched over it, and standing 2 m off), the
//! empty floor is shot, then each item alone in its rest pose, its middle
//! at the same spot. The view is the headset's: its pixels a degree
//! (`render::legibility::eye_px_per_deg`), so an item covers as many
//! pixels as it would in the eye.
//!
//! Shots go to `<capture-out>/torchlit/<floor>/<eye>/<item>.png` (and
//! `_empty.png`), with `torchlit.json` listing the items.
//! `scripts/torchlit.py` takes each item from its empty floor and measures
//! how it stands off it: the readability report and the contact sheets.
//! `--torchlit-only a,b` shoots only the items whose ids contain one of
//! those.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use std::time::Duration;

use crate::launch::LaunchOptions;
use crate::models::{ModelMaterials, ModelState, Models};
use crate::render::FreeCamera;
use crate::render::materials::Mat;

pub struct TorchlitPlugin;

impl Plugin for TorchlitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Torchlit>()
            .add_systems(Update, torchlit.run_if(run));
    }
}

fn run(opts: Res<LaunchOptions>) -> bool {
    opts.capture.torchlit.is_some()
}

/// The dark room: high over the valley, out of everything's way.
pub const ROOM: Vec3 = Vec3::new(0.0, 1400.0, 0.0);
/// Its floor's side, its walls' height (m).
const SIDE: f32 = 8.0;
const HIGH: f32 = 3.0;
/// The view's side, pixels (square).
pub const VIEW_PX: u32 = 800;

/// The floors: name, material.
pub const FLOORS: [(&str, Mat); 3] = [
    ("wood", Mat::WoodFloor),
    ("concrete", Mat::Concrete),
    ("ruin", Mat::Charred),
];

/// The eyes: name, distance to the item's middle (m), elevation (deg).
pub const EYES: [(&str, f32, f32); 2] = [("arm", 0.65, 55.0), ("2m", 2.0, 40.0)];
/// The eyes look from this compass bearing (deg), at the floor by the
/// item's middle.
const BEARING: f32 = 200.0;

/// Frames the room settles for when it opens (the night, the exposure).
const WARM: u32 = 120;
/// Frames a shot settles for (the move, the light, the shadows).
const SETTLE: u32 = 8;
/// Frames an item may take to load.
const LOAD_LIMIT: u32 = 400;

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
enum Stage {
    #[default]
    Start,
    Warm,
    Pose,
    Spawn,
    Load,
    Place,
    Shoot,
    Waiting,
    Next,
}

#[derive(Clone, Debug, serde::Serialize)]
struct Record {
    id: String,
    name: String,
    category: String,
    folder: &'static str,
    /// Its catalogue size (m).
    dims: [f32; 3],
    /// Its drawn bounds lying on the floor (m), and whether it loaded.
    lo: [f32; 3],
    hi: [f32; 3],
    failed: bool,
}

#[derive(Resource, Default)]
struct Torchlit {
    items: Vec<(String, &'static str)>,
    floors: Vec<usize>,
    floor: usize,
    eye: usize,
    /// -1: the empty floor; then each item.
    i: isize,
    stage: Stage,
    frames: u32,
    stable: (usize, u32),
    room: Vec<Entity>,
    root: Option<Entity>,
    torch: Option<Entity>,
    records: Vec<Record>,
    started: Option<std::time::Instant>,
}

#[allow(clippy::too_many_arguments)]
fn torchlit(
    mut commands: Commands,
    mut opts: ResMut<LaunchOptions>,
    mut st: ResMut<Torchlit>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut cams: Query<(Entity, &mut Transform), With<FreeCamera>>,
    children: Query<&Children>,
    meshes_q: Query<(&GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
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
    mut exit: MessageWriter<AppExit>,
    mut boot: Local<u32>,
) {
    if zone.is_none() {
        return;
    }
    *boot += 1;
    if *boot == 1 {
        commands.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(1.0 / 30.0),
        ));
        // Dark: a clear night (the room shuts the moon out).
        opts.hour = Some("night".into());
        opts.weather = Some("clearing".into());
    }
    if *boot < opts.frames {
        return;
    }
    let Ok((cam_e, mut cam_tf)) = cams.single_mut() else {
        return;
    };
    let out = opts.capture.out_dir().join("torchlit");
    let st = &mut *st;
    match st.stage {
        Stage::Start => {
            let only: Vec<String> = opts
                .capture
                .torchlit_only
                .as_deref()
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            let root = crate::asset_root_path().join("models");
            for (_, def) in catalog.0.iter() {
                let folder = match def.category {
                    extraction_core::items::Category::Magazine => "mags",
                    _ if def.firearm.is_some() => "guns",
                    _ => "items",
                };
                if !root.join(folder).join(format!("{}.glb", def.id)).exists() {
                    continue;
                }
                if !only.is_empty() && !only.iter().any(|o| def.id.contains(o.as_str())) {
                    continue;
                }
                st.items.push((def.id.clone(), folder));
                st.records.push(Record {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    category: format!("{:?}", def.category),
                    folder,
                    dims: def.size_m(),
                    lo: [0.0; 3],
                    hi: [0.0; 3],
                    failed: false,
                });
            }
            let wanted = opts.capture.torchlit.clone().unwrap_or_default();
            st.floors = FLOORS
                .iter()
                .enumerate()
                .filter(|(_, (n, _))| wanted == "all" || wanted.split(',').any(|w| w == *n))
                .map(|(i, _)| i)
                .collect();
            info!(
                "torchlit: {} items on {} floors",
                st.items.len(),
                st.floors.len()
            );
            // The torch, held by the eye and aimed where it looks.
            let torch = commands
                .spawn((
                    Name::new("Torchlit torch"),
                    crate::gadgets::visuals::torch_beam(),
                    Transform::from_xyz(0.15, -0.1, 0.0),
                    ChildOf(cam_e),
                ))
                .id();
            st.torch = Some(torch);
            // The headset's pixels a degree, at this view's size.
            let fov = (VIEW_PX as f32 / crate::render::legibility::eye_px_per_deg()).to_radians();
            commands.entity(cam_e).insert((
                super::Lens(fov),
                Projection::Perspective(PerspectiveProjection {
                    fov,
                    near: 0.05,
                    far: 200.0,
                    ..default()
                }),
            ));
            st.started = Some(std::time::Instant::now());
            st.floor = 0;
            st.eye = 0;
            st.i = -1;
            build_room(
                st,
                &mut commands,
                &mut lib,
                &server,
                &mut materials,
                &mut meshes,
            );
            st.frames = 0;
            st.stage = Stage::Warm;
        }
        Stage::Warm => {
            st.frames += 1;
            if st.frames >= WARM {
                st.stage = Stage::Pose;
            }
        }
        Stage::Pose => {
            let (_, dist, el) = EYES[st.eye];
            let (az, el) = (BEARING.to_radians(), el.to_radians());
            let dir = Vec3::new(az.sin() * el.cos(), el.sin(), -az.cos() * el.cos());
            let aim = ROOM + Vec3::Y * 0.03;
            *cam_tf = Transform::from_translation(aim + dir * dist).looking_at(aim, Vec3::Y);
            // The torch at the eye, on what the eye looks at (as anyone
            // looking at a thing by torchlight holds it): its shadow falls
            // behind it, so what an item hides of the floor is only itself.
            if let Some(t) = st.torch {
                let held = Vec3::new(0.0, -0.01, 0.0);
                let target = cam_tf.compute_affine().inverse().transform_point3(aim);
                commands
                    .entity(t)
                    .insert(Transform::from_translation(held).looking_at(target, Vec3::Y));
            }
            st.stage = Stage::Spawn;
        }
        Stage::Spawn => {
            if st.i < 0 {
                // The empty floor.
                st.frames = 0;
                st.stage = Stage::Shoot;
                return;
            }
            let root = commands
                .spawn((
                    Name::new("Torchlit item"),
                    Transform::from_translation(ROOM),
                    Visibility::default(),
                ))
                .id();
            st.root = Some(root);
            st.frames = 0;
            st.stable = (0, 0);
            st.stage = Stage::Load;
        }
        Stage::Load => {
            let root = st.root.unwrap();
            let (id, folder) = st.items[st.i as usize].clone();
            st.frames += 1;
            let mut failed = false;
            if children.get(root).is_err() {
                let path = format!("models/{folder}/{id}.glb");
                match models.request(&path, &server) {
                    None => failed = true,
                    Some(h) => match Models::state(&h, &server, &gltfs) {
                        ModelState::Loading if st.frames < LOAD_LIMIT => return,
                        ModelState::Loading | ModelState::Failed => failed = true,
                        ModelState::Ready(g) => {
                            let nodes = crate::models::model_nodes(g, &gnodes, &gmeshes);
                            super::sweep::spawn_nodes(&mut commands, root, &nodes, &mut |name| {
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
            let mut all = Vec::new();
            super::sweep::collect(root, &children, &mut all);
            let n = all
                .iter()
                .filter(|e| meshes_q.get(**e).is_ok_and(|m| m.1.is_some()))
                .count();
            if st.stable.0 == n && n > 0 {
                st.stable.1 += 1;
            } else {
                st.stable = (n, 0);
            }
            if st.stable.1 < 4 && st.frames < LOAD_LIMIT && !failed {
                return;
            }
            if failed || n == 0 {
                warn!("torchlit: {id} didn't load");
                if st.floor == 0 && st.eye == 0 {
                    st.records[st.i as usize].failed = true;
                }
                st.stage = Stage::Next;
                return;
            }
            st.stage = Stage::Place;
        }
        Stage::Place => {
            // Lying with its lowest point on the floor, its middle on the
            // spot the eye looks at.
            let root = st.root.unwrap();
            let mut all = Vec::new();
            super::sweep::collect(root, &children, &mut all);
            let Some((lo, hi)) = all
                .iter()
                .filter_map(|e| {
                    let (gt, aabb) = meshes_q.get(*e).ok()?;
                    Some(super::sweep::world_aabb(gt, aabb?))
                })
                .reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))
            else {
                st.stage = Stage::Next;
                return;
            };
            let mid = (lo + hi) * 0.5;
            let shift = Vec3::new(ROOM.x - mid.x, ROOM.y - lo.y, ROOM.z - mid.z);
            commands
                .entity(root)
                .insert(Transform::from_translation(ROOM + shift));
            if st.floor == 0 && st.eye == 0 {
                let r = &mut st.records[st.i as usize];
                r.lo = (lo + shift - ROOM).to_array();
                r.hi = (hi + shift - ROOM).to_array();
            }
            st.frames = 0;
            st.stage = Stage::Shoot;
        }
        Stage::Shoot => {
            // A few frames for the move and the light to settle.
            st.frames += 1;
            if st.frames < SETTLE {
                return;
            }
            let (floor, _) = FLOORS[st.floors[st.floor]];
            let (eye, _, _) = EYES[st.eye];
            let name = if st.i < 0 {
                "_empty".to_string()
            } else {
                st.items[st.i as usize].0.clone()
            };
            let file = out.join(floor).join(eye).join(format!("{name}.png"));
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut save = save_to_disk(file);
            st.stage = Stage::Waiting;
            commands.spawn(Screenshot::primary_window()).observe(
                move |shot: On<ScreenshotCaptured>, mut st: ResMut<Torchlit>| {
                    save(shot);
                    st.stage = Stage::Next;
                },
            );
        }
        Stage::Waiting => {}
        Stage::Next => {
            if let Some(root) = st.root.take() {
                commands.entity(root).despawn();
            }
            st.i += 1;
            if st.i as usize >= st.items.len() {
                st.i = -1;
                st.eye += 1;
                if st.eye >= EYES.len() {
                    st.eye = 0;
                    st.floor += 1;
                    write_report(st, &out);
                    if st.floor >= st.floors.len() {
                        info!(
                            "torchlit: done, {} items in {:.0} s",
                            st.items.len(),
                            st.started.map_or(0.0, |t| t.elapsed().as_secs_f32())
                        );
                        exit.write(AppExit::Success);
                        return;
                    }
                    build_room(
                        st,
                        &mut commands,
                        &mut lib,
                        &server,
                        &mut materials,
                        &mut meshes,
                    );
                    st.frames = WARM - 20;
                    st.stage = Stage::Warm;
                    return;
                }
                st.stage = Stage::Pose;
                return;
            }
            if st.i % 25 == 0 {
                info!(
                    "torchlit: {} {} {}/{} ({:.0} s)",
                    FLOORS[st.floors[st.floor]].0,
                    EYES[st.eye].0,
                    st.i,
                    st.items.len(),
                    st.started.map_or(0.0, |t| t.elapsed().as_secs_f32())
                );
            }
            st.stage = Stage::Spawn;
        }
    }
}

/// The room for this floor: its floor of the game's material, tiled as a
/// building's is, and plastered walls and a ceiling shutting the night out.
fn build_room(
    st: &mut Torchlit,
    commands: &mut Commands,
    lib: &mut crate::render::materials::MaterialLibrary,
    server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
    meshes: &mut Assets<Mesh>,
) {
    for e in st.room.drain(..) {
        commands.entity(e).despawn();
    }
    let (_, floor_mat) = FLOORS[st.floors[st.floor]];
    let slab = |size: Vec3, mat: Mat, meshes: &mut Assets<Mesh>| {
        let tile = mat.info().1.max(0.1);
        let mut m = Mesh::from(Cuboid::from_size(size));
        // Its faces' UVs from 0..1 to their size in tiles (the largest two
        // sides of a thin slab: near enough for the floor's top).
        if let Some(bevy::mesh::VertexAttributeValues::Float32x2(uvs)) =
            m.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        {
            let span = size.x.max(size.z) / tile;
            for uv in uvs.iter_mut() {
                uv[0] *= span;
                uv[1] *= span;
            }
        }
        let _ = m.generate_tangents();
        meshes.add(m)
    };
    let floor_h = lib.get(floor_mat, server, materials);
    let wall_h = lib.get(Mat::Plaster, server, materials);
    let floor = commands
        .spawn((
            Name::new("Torchlit floor"),
            Mesh3d(slab(Vec3::new(SIDE, 0.2, SIDE), floor_mat, meshes)),
            MeshMaterial3d(floor_h),
            Transform::from_translation(ROOM - Vec3::Y * (0.1 + 0.002)),
        ))
        .id();
    st.room.push(floor);
    let wall = |at: Vec3, size: Vec3| (at, size);
    let h = SIDE * 0.5;
    for (at, size) in [
        wall(Vec3::new(0.0, HIGH * 0.5, -h), Vec3::new(SIDE, HIGH, 0.2)),
        wall(Vec3::new(0.0, HIGH * 0.5, h), Vec3::new(SIDE, HIGH, 0.2)),
        wall(Vec3::new(-h, HIGH * 0.5, 0.0), Vec3::new(0.2, HIGH, SIDE)),
        wall(Vec3::new(h, HIGH * 0.5, 0.0), Vec3::new(0.2, HIGH, SIDE)),
        wall(Vec3::new(0.0, HIGH + 0.1, 0.0), Vec3::new(SIDE, 0.2, SIDE)),
    ] {
        st.room.push(
            commands
                .spawn((
                    Name::new("Torchlit wall"),
                    Mesh3d(slab(size, Mat::Plaster, meshes)),
                    MeshMaterial3d(wall_h.clone()),
                    Transform::from_translation(ROOM + at),
                ))
                .id(),
        );
    }
}

fn write_report(st: &Torchlit, out: &std::path::Path) {
    let json = serde_json::json!({
        "floors": st.floors.iter().map(|f| FLOORS[*f].0).collect::<Vec<_>>(),
        "eyes": EYES.iter().map(|e| serde_json::json!({"name": e.0, "dist": e.1, "elevation": e.2})).collect::<Vec<_>>(),
        "view_px": VIEW_PX,
        "px_per_deg": crate::render::legibility::eye_px_per_deg(),
        "items": st.records,
    });
    let _ = std::fs::create_dir_all(out);
    let _ = std::fs::write(
        out.join("torchlit.json"),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    );
}
