//! The gunsmith's bench: one gun at hand distance under the game's light.
//!
//! ```text
//! cargo run -p gunsmith --example bench -- ak74
//! cargo run -p gunsmith --example bench -- pm_pistol --shots docs/craft/guns/pm_pistol/shots
//! cargo run -p gunsmith --example bench -- ak74 --screenshot x.png --view left --dist 0.3
//! ```
//!
//! Interactive keys: `Space` stops and starts the turntable; `1`-`7` the
//! views (right, left, top, front three-quarter, shooter's eye, front,
//! underneath); `E` explodes it; `B` `S` `M` `C` `T` cycle the bolt, the
//! safety, the magazine release, the dust cover and the trigger, slowly;
//! `0` stops the motion; `-` and `=` move in and out.
//!
//! `--shots DIR` takes the review's standard set (`docs/craft/GUNSMITHING.md`)
//! and exits; `--screenshot FILE` takes one (`--view`, `--dist`, `--focus
//! x,y,z`, `--motion bolt|safety|mag|cover|trigger --t 0..1`, `--exploded`).
//! Headless: run it under `xvfb-run` with `WGPU_BACKEND=vulkan`, as
//! `scripts/gun_bench.sh` does.
//!
//! The light is the game's calm daytime (`world::sky`'s `Weather::default`,
//! `render::camera_look`): the overcast sun, the grey-blue sky light, AgX
//! at EV100 10.6. The finishes are the game's (`models::ModelMaterials`),
//! from the same CC0 scans under `assets/textures/materials/`.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::camera::Exposure;
use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::gltf::{Gltf, GltfMaterial, GltfMesh, GltfNode};
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;

use gunsmith::firearm::{FireMode, Firearm, FirearmDef};
use gunsmith::parts::{Axis, Delta, Motion, Moves, delta, moves_for};
use gunsmith::spec::{Spec, spec};

// --- What to show -----------------------------------------------------------

/// A slow cycle of one of the gun's motions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
    Bolt,
    Safety,
    Mag,
    Cover,
    Trigger,
}

impl Work {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "bolt" => Work::Bolt,
            "safety" => Work::Safety,
            "mag" => Work::Mag,
            "cover" => Work::Cover,
            "trigger" => Work::Trigger,
            _ => return None,
        })
    }
}

/// Where the camera stands, by name: (yaw from the right side, pitch).
fn view(name: &str) -> Option<(f32, f32)> {
    Some(match name {
        // The right side, the ejection port's: square on.
        "right" => (0.0, 0.02),
        "left" => (180.0, 0.02),
        // Dead level, for orthographic overlays.
        "level_right" => (0.0, 0.0),
        "level_left" => (180.0, 0.0),
        "top" => (0.0, 89.0),
        // Ahead of the right side and above, the way a gun is shown.
        "threeq" => (-40.0, 25.0),
        // Behind the gun, a little to the right and above it, as the
        // shooter looks down at it in the hands: the selector, the safety,
        // the grip, the top of the receiver.
        "eye" => (60.0, 30.0),
        "front" => (-88.0, 5.0),
        "under" => (-30.0, -60.0),
        _ => return None,
    })
}

#[derive(Clone, Debug)]
struct Shot {
    name: String,
    view: String,
    dist: f32,
    focus: Option<Vec3>,
    work: Option<(Work, f32)>,
    exploded: bool,
    /// An orthographic frame this many metres high (for overlays on a
    /// scaled reference photograph), instead of the 50-degree lens.
    ortho: Option<f32>,
}

impl Shot {
    fn new(name: &str, view: &str, dist: f32) -> Self {
        Self {
            name: name.into(),
            view: view.into(),
            dist,
            focus: None,
            work: None,
            exploded: false,
            ortho: None,
        }
    }

    fn working(mut self, w: Work, t: f32) -> Self {
        self.work = Some((w, t));
        self
    }
}

/// The review's standard set: every view at 30 cm and at 1 m, exploded, and
/// each motion worked at 30 cm.
fn standard_shots() -> Vec<Shot> {
    let mut v = Vec::new();
    for view in ["right", "left", "threeq", "eye", "top", "front", "under"] {
        v.push(Shot::new(&format!("{view}_30cm"), view, 0.3));
    }
    for view in ["right", "left", "threeq", "eye"] {
        v.push(Shot::new(&format!("{view}_1m"), view, 1.0));
    }
    v.push(Shot {
        exploded: true,
        ..Shot::new("exploded_1m", "threeq", 1.0)
    });
    v.push(Shot::new("bolt_back_30cm", "threeq", 0.3).working(Work::Bolt, 1.0));
    v.push(Shot::new("safety_fire_30cm", "eye", 0.3).working(Work::Safety, 1.0));
    v.push(Shot::new("mag_release_30cm", "threeq", 0.3).working(Work::Mag, 1.0));
    v.push(Shot::new("cover_off_30cm", "threeq", 0.3).working(Work::Cover, 1.0));
    v.push(Shot::new("trigger_pulled_30cm", "right", 0.3).working(Work::Trigger, 1.0));
    v
}

#[derive(Resource)]
struct Bench {
    spec: &'static Spec,
    def: FirearmDef,
    /// The turntable's yaw (degrees), and whether it turns.
    yaw: f32,
    pitch: f32,
    turning: bool,
    dist: f32,
    focus: Option<Vec3>,
    ortho: Option<f32>,
    exploded: f32,
    exploding: bool,
    work: Option<Work>,
    /// Seconds into the current motion's cycle (one cycle: 4 s).
    clock: f32,
    /// A fixed point of the motion (shots), instead of the cycle.
    hold: Option<f32>,
    /// Shots still to take, and where they go.
    shots: Vec<Shot>,
    out: Option<PathBuf>,
    /// Frames left before the next shot (the model and its textures in,
    /// then a few frames for each change to settle).
    wait: u32,
    taken: u32,
    /// Set up and settling: the next frame past `wait` takes it.
    armed: bool,
    closing: bool,
}

const CYCLE: f32 = 4.0;

// --- Model ------------------------------------------------------------------

#[derive(Resource)]
struct Loading {
    gun: Handle<Gltf>,
    mag: Option<Handle<Gltf>>,
    done: bool,
}

/// A part of the gun on the bench and its rest pose.
#[derive(Component)]
struct Piece {
    moves: Moves,
    rest: Transform,
    name: String,
}

/// The magazine, seated at the model's `magwell`.
#[derive(Component)]
struct Magazine {
    seat: Transform,
}

#[derive(Component)]
struct Gun;

#[derive(Component)]
struct BenchCamera;

/// Gun-space bounds of what's drawn, for framing.
#[derive(Resource, Default)]
struct Bounds {
    lo: Vec3,
    hi: Vec3,
}

fn magazine_for(id: &str) -> Option<&'static str> {
    Some(match id {
        "ak74" => "ak_mag",
        "rpk74" => "rpk_mag",
        "pm_pistol" => "pm_mag",
        "ppsh" => "ppsh_drum",
        "svd" => "svd_mag",
        _ => return None,
    })
}

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(serde::Deserialize)]
struct Catalog {
    items: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    id: String,
    #[serde(default)]
    dims: Option<(f32, f32, f32)>,
    #[serde(default)]
    firearm: Option<FirearmDef>,
}

fn catalog() -> Catalog {
    let path = workspace().join("assets/data/items.ron");
    let text = std::fs::read_to_string(&path).expect("read assets/data/items.ron");
    ron::from_str(&text).expect("parse items.ron")
}

/// The gun's mechanism as the catalog describes it.
fn firearm_def(id: &str) -> FirearmDef {
    catalog()
        .items
        .into_iter()
        .find(|e| e.id == id)
        .and_then(|e| e.firearm)
        .unwrap_or_else(|| panic!("{id} is no firearm in the catalog"))
}

/// A magazine's height: its model is centred on its middle, and hangs that
/// half from the gun's `magwell` (as `weapons::visuals::seat_mag_visuals`).
fn mag_height(id: &str) -> f32 {
    catalog()
        .items
        .into_iter()
        .find(|e| e.id == id)
        .and_then(|e| e.dims)
        .map_or(0.1, |d| d.1)
}

// --- Finishes ---------------------------------------------------------------

/// The game's finishes for the model's material names, kept in step with
/// `src/models.rs` (`ModelMaterials::get`) and `src/render/materials.rs`.
#[derive(Resource, Default)]
struct Finishes(HashMap<String, Handle<StandardMaterial>>);

fn repeat_sampler() -> ImageSamplerDescriptor {
    ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    }
}

/// A scanned set (`materials/<dir>`) as the game builds it, or its flat
/// fallback colour when the scan hasn't been fetched.
fn scanned(
    dir: &str,
    tile: f32,
    color: Color,
    rough: f32,
    metal_flat: f32,
    server: &AssetServer,
) -> StandardMaterial {
    let mut m = StandardMaterial {
        base_color: color,
        perceptual_roughness: rough,
        metallic: metal_flat,
        uv_transform: Affine2::from_scale(Vec2::splat(1.0 / tile)),
        ..default()
    };
    let root = workspace().join("assets/textures/materials").join(dir);
    if root.join("albedo.ktx2").exists() {
        let load = |name: &str, srgb: bool| -> Handle<Image> {
            server
                .load_builder()
                .with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = srgb;
                    s.sampler = ImageSampler::Descriptor(repeat_sampler());
                })
                .load(format!("textures/materials/{dir}/{name}.ktx2"))
        };
        m.base_color = Color::WHITE;
        m.base_color_texture = Some(load("albedo", true));
        m.normal_map_texture = Some(load("normal", false));
        let arm = load("arm", false);
        m.occlusion_texture = Some(arm.clone());
        m.metallic_roughness_texture = Some(arm);
        m.perceptual_roughness = 1.0;
        m.metallic = 1.0;
    } else {
        warn!("materials/{dir} not fetched (scripts/fetch_assets.py materials): flat colour");
    }
    m
}

/// A gun-steel finish on the gun-metal scan, as the game's `models::steel`.
fn steel(mut m: StandardMaterial, f: &gunsmith::finish::SteelFinish) -> StandardMaterial {
    m.uv_transform = Affine2::from_scale(Vec2::splat(1.0 / gunsmith::finish::TILE));
    let scanned = m.base_color_texture.is_some();
    match f.gain {
        Some([r, g, b]) if scanned => m.base_color = Color::linear_rgb(r, g, b),
        Some([r, g, b]) => m.base_color = Color::linear_rgb(0.045 * r, 0.045 * g, 0.045 * b),
        None => {
            m.base_color_texture = None;
            let [r, g, b] = f.flat;
            m.base_color = Color::linear_rgb(r, g, b);
        }
    }
    if f.keep_scan_metal && m.metallic_roughness_texture.is_some() {
        m.metallic = f.metallic;
        m.perceptual_roughness = f.roughness;
    } else {
        m.metallic_roughness_texture = None;
        m.metallic = f.metallic;
        m.perceptual_roughness = if f.keep_scan_metal {
            (0.31 * f.roughness).min(1.0)
        } else {
            f.roughness
        };
    }
    m
}

fn flat(color: Color, metallic: f32, rough: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        metallic,
        perceptual_roughness: rough,
        ..default()
    }
}

fn finish(name: &str, server: &AssetServer) -> StandardMaterial {
    let gun_metal = || {
        scanned(
            "gun_metal",
            0.15,
            Color::srgb(0.08, 0.08, 0.09),
            0.4,
            0.8,
            server,
        )
    };
    match name {
        // As the game's (src/models.rs, `steel`).
        "Blued" => steel(gun_metal(), &gunsmith::finish::BLUED),
        "Stamped" => steel(gun_metal(), &gunsmith::finish::STAMPED),
        "Wood" => scanned(
            "gun_wood",
            0.25,
            Color::srgb(0.36, 0.17, 0.08),
            0.5,
            0.0,
            server,
        ),
        "Polymer" => scanned(
            "polymer_plum",
            0.4,
            Color::srgb(0.27, 0.1, 0.11),
            0.55,
            0.0,
            server,
        ),
        "Steel" => {
            let mut m = gun_metal();
            m.base_color_texture = None;
            m.base_color = Color::srgb(0.46, 0.46, 0.47);
            m.metallic = 1.0;
            m
        }
        "Bakelite" => {
            let mut m = scanned(
                "polymer_plum",
                0.4,
                Color::srgb(0.27, 0.1, 0.11),
                0.55,
                0.0,
                server,
            );
            m.base_color_texture = None;
            m.base_color = Color::srgb(0.27, 0.105, 0.055);
            m.metallic = 0.0;
            m
        }
        "Black" => steel(gun_metal(), &gunsmith::finish::BLACK),
        "Brass" => flat(Color::srgb(0.72, 0.52, 0.24), 1.0, 0.35),
        "Worn" => flat(Color::srgb(0.5, 0.5, 0.5), 1.0, 0.45),
        "Rubber" => flat(Color::srgb(0.07, 0.07, 0.07), 0.0, 0.88),
        "Red" => flat(Color::srgb(0.5, 0.06, 0.04), 0.0, 0.6),
        "Dark" => flat(Color::srgb(0.08, 0.085, 0.09), 0.7, 0.4),
        "Iron" => flat(Color::srgb(0.12, 0.12, 0.12), 0.8, 0.55),
        "Copper" => flat(Color::srgb(0.78, 0.44, 0.28), 1.0, 0.32),
        "Rope" => flat(Color::srgb(0.55, 0.47, 0.32), 0.0, 0.9),
        "Lens" => StandardMaterial {
            base_color: Color::srgb(0.05, 0.07, 0.1),
            metallic: 0.3,
            perceptual_roughness: 0.05,
            reflectance: 0.9,
            ..default()
        },
        "Glass" => StandardMaterial {
            base_color: Color::srgba(0.12, 0.14, 0.15, 0.55),
            perceptual_roughness: 0.05,
            alpha_mode: AlphaMode::Blend,
            reflectance: 0.8,
            ..default()
        },
        other => {
            warn!("finish {other:?} isn't on the bench; drawing it grey");
            flat(Color::srgb(0.3, 0.3, 0.3), 0.0, 0.6)
        }
    }
}

// --- App --------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut id = None;
    let mut one = Shot::new("shot", "threeq", 0.3);
    let mut out = None;
    let mut shots_dir = None;
    let mut t = 1.0;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || {
            i += 1;
            args.get(i).cloned().unwrap_or_default()
        };
        match a {
            "--screenshot" => out = Some(PathBuf::from(val())),
            "--shots" => shots_dir = Some(PathBuf::from(val())),
            "--view" => one.view = val(),
            "--dist" => one.dist = val().parse().expect("--dist metres"),
            "--focus" => {
                let v: Vec<f32> = val().split(',').map(|s| s.parse().unwrap()).collect();
                one.focus = Some(Vec3::new(v[0], v[1], v[2]));
            }
            "--motion" => one.work = Some((Work::parse(&val()).expect("--motion"), 1.0)),
            "--t" => t = val().parse().expect("--t 0..1"),
            "--exploded" => one.exploded = true,
            "--ortho" => one.ortho = Some(val().parse().expect("--ortho metres")),
            _ if !a.starts_with("--") => id = Some(a.to_string()),
            _ => panic!("unknown argument {a}"),
        }
        i += 1;
    }
    if let Some((w, _)) = one.work {
        one.work = Some((w, t));
    }
    let id = id.unwrap_or_else(|| "ak74".into());
    let spec = spec(&id).unwrap_or_else(|| panic!("no gun {id} on the bench (gunsmith::spec)"));
    let def = firearm_def(&id);

    let (shots, out) = match (shots_dir, out) {
        (Some(dir), _) => (standard_shots(), Some(dir)),
        (None, Some(file)) => {
            one.name = file.file_stem().unwrap().to_string_lossy().into_owned();
            let dir = file.parent().map(PathBuf::from).unwrap_or_default();
            (vec![one], Some(dir))
        }
        (None, None) => (Vec::new(), None),
    };
    let headless = out.is_some();

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: workspace().join("assets").to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("gunsmith's bench: {id}"),
                    resolution: WindowResolution::new(1280, 800),
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(Color::srgb(0.50, 0.53, 0.55)))
    .insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.62, 0.68, 0.76),
        brightness: 1100.0,
        ..default()
    })
    .insert_resource(Bench {
        spec,
        def,
        yaw: if headless { 0.0 } else { -40.0 },
        pitch: if headless { 0.0 } else { 25.0 },
        turning: !headless,
        dist: 0.6,
        focus: None,
        ortho: None,
        exploded: 0.0,
        exploding: false,
        work: None,
        clock: 0.0,
        hold: None,
        shots,
        out,
        wait: 90,
        taken: 0,
        armed: false,
        closing: false,
    })
    .init_resource::<Finishes>()
    .init_resource::<Bounds>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            spawn_model,
            keys,
            take_shots,
            animate,
            measure,
            frame_camera,
        )
            .chain(),
    );
    app.run();
}

fn setup(mut commands: Commands, server: Res<AssetServer>, bench: Res<Bench>) {
    let id = bench.spec.id;
    let gun = server.load(format!("models/guns/{id}.glb"));
    let mag = magazine_for(id).map(|m| server.load(format!("models/mags/{m}.glb")));
    commands.insert_resource(Loading {
        gun,
        mag,
        done: false,
    });
    // The game's calm daytime sun: high in the south-west behind the
    // cloud (`Weather::default`).
    commands.spawn((
        DirectionalLight {
            illuminance: 1350.0,
            color: Color::srgb(0.92, 0.93, 0.95),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::IDENTITY.looking_to(Vec3::new(0.35, -0.8, 0.5).normalize(), Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 4.0,
            ..default()
        }
        .build(),
    ));
    commands.spawn((
        BenchCamera,
        Camera3d::default(),
        Hdr,
        Tonemapping::AgX,
        Exposure { ev100: 10.6 },
        Msaa::Sample4,
        Projection::Perspective(PerspectiveProjection {
            // Narrower than the game's 75 degrees: at 30 cm a wide lens
            // bends the gun out of shape, and the eye in the headset looks
            // at the middle of its view.
            fov: 50f32.to_radians(),
            near: 0.01,
            far: 50.0,
            ..default()
        }),
        Transform::from_xyz(0.5, 0.2, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((Gun, Transform::IDENTITY, Visibility::default()));
}

#[allow(clippy::too_many_arguments)]
fn spawn_model(
    mut commands: Commands,
    mut loading: ResMut<Loading>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    meshes: Res<Assets<GltfMesh>>,
    server: Res<AssetServer>,
    mut finishes: ResMut<Finishes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    gun: Single<Entity, With<Gun>>,
    bench: Res<Bench>,
) {
    if loading.done {
        return;
    }
    let Some(g) = gltfs.get(&loading.gun) else {
        return;
    };
    let mag = match &loading.mag {
        Some(h) => match gltfs.get(h) {
            Some(m) => Some(m),
            None => return,
        },
        None => None,
    };
    loading.done = true;
    let mut fin = |name: &str| -> Handle<StandardMaterial> {
        finishes
            .0
            .entry(name.to_string())
            .or_insert_with(|| materials.add(finish(name, &server)))
            .clone()
    };
    let mut seat = None;
    for (node, transform) in roots(g, &nodes) {
        if node.name == "magwell" {
            seat = Some(transform);
            continue;
        }
        // What the game won't draw, the bench doesn't either.
        let Some(moves) = moves_for(&node.name) else {
            continue;
        };
        let piece = commands
            .spawn((
                Piece {
                    moves,
                    rest: transform,
                    name: node.name.clone(),
                },
                transform,
                Visibility::default(),
                ChildOf(*gun),
            ))
            .id();
        spawn_meshes(&mut commands, piece, node, g, &nodes, &meshes, &mut fin);
    }
    if let (Some(seat), Some(m)) = (seat, mag) {
        let mag_id = magazine_for(bench.spec.id).unwrap_or_default();
        let seat = seat.mul_transform(Transform::from_xyz(0.0, -mag_height(mag_id) * 0.5, 0.0));
        let holder = commands
            .spawn((
                Magazine { seat },
                seat,
                Visibility::default(),
                ChildOf(*gun),
            ))
            .id();
        for (node, transform) in roots(m, &nodes) {
            let part = commands
                .spawn((transform, Visibility::default(), ChildOf(holder)))
                .id();
            spawn_meshes(&mut commands, part, node, m, &nodes, &meshes, &mut fin);
        }
    }
}

/// A model's top-level nodes and their transforms.
fn roots<'a>(g: &'a Gltf, nodes: &'a Assets<GltfNode>) -> Vec<(&'a GltfNode, Transform)> {
    let children: std::collections::HashSet<AssetId<GltfNode>> = g
        .nodes
        .iter()
        .filter_map(|h| nodes.get(h))
        .flat_map(|n| n.children.iter().map(|c| c.id()))
        .collect();
    g.nodes
        .iter()
        .filter(|h| !children.contains(&h.id()))
        .filter_map(|h| nodes.get(h))
        .map(|n| (n, n.transform))
        .collect()
}

fn spawn_meshes(
    commands: &mut Commands,
    parent: Entity,
    node: &GltfNode,
    g: &Gltf,
    nodes: &Assets<GltfNode>,
    meshes: &Assets<GltfMesh>,
    fin: &mut impl FnMut(&str) -> Handle<StandardMaterial>,
) {
    let names: HashMap<AssetId<GltfMaterial>, String> = g
        .named_materials
        .iter()
        .map(|(n, h)| (h.id(), n.to_string()))
        .collect();
    if let Some(m) = node.mesh.as_ref().and_then(|m| meshes.get(m)) {
        for p in &m.primitives {
            let name = p
                .material
                .as_ref()
                .and_then(|h| names.get(&h.id()).cloned())
                .unwrap_or_else(|| "Blued".into());
            commands.spawn((
                Mesh3d(p.mesh.clone()),
                MeshMaterial3d(fin(&name)),
                ChildOf(parent),
            ));
        }
    }
    for c in &node.children {
        if let Some(child) = nodes.get(c) {
            let e = commands
                .spawn((child.transform, Visibility::default(), ChildOf(parent)))
                .id();
            spawn_meshes(commands, e, child, g, nodes, meshes, fin);
        }
    }
}

// --- Motion -----------------------------------------------------------------

/// Where the motion stands, 0..1: out and back over one cycle, with a
/// pause at each end so the eye can read both.
fn phase(clock: f32) -> f32 {
    let x = (clock / CYCLE).fract();
    let k = ((x - 0.1) / 0.35).clamp(0.0, 1.0) - ((x - 0.55) / 0.35).clamp(0.0, 1.0);
    // Eased: hands slow at the ends of a stroke.
    k * k * (3.0 - 2.0 * k)
}

fn animate(
    time: Res<Time>,
    mut bench: ResMut<Bench>,
    mut pieces: Query<(&Piece, &mut Transform), Without<Magazine>>,
    mut mags: Query<(&Magazine, &mut Transform), Without<Piece>>,
) {
    let dt = time.delta_secs();
    bench.clock += dt;
    let target = if bench.exploding { 1.0 } else { 0.0 };
    bench.exploded += (target - bench.exploded).clamp(-dt * 1.5, dt * 1.5);
    if bench.hold.is_some() {
        bench.exploded = target;
    }
    let k = bench.hold.unwrap_or_else(|| phase(bench.clock));

    let mut f = Firearm::new(bench.def.clone(), 1.0);
    let mut cocked = true;
    let mut mag_out = 0.0;
    match bench.work {
        Some(Work::Bolt) => {
            // Off safe: the AK's lever blocks the handle on safe.
            f.mode = f
                .def
                .modes
                .iter()
                .position(|m| *m != FireMode::Safe)
                .unwrap_or(0);
            f.bolt_travel = k;
        }
        Some(Work::Safety) => {
            // Safe to the last position on the lever, through any between.
            let n = f.def.modes.len();
            f.mode = ((k * (n - 1) as f32).round() as usize).min(n.saturating_sub(1));
        }
        Some(Work::Trigger) => {
            f.mode = f
                .def
                .modes
                .iter()
                .position(|m| *m != FireMode::Safe)
                .unwrap_or(0);
            if k > 0.95 {
                cocked = false;
            }
        }
        Some(Work::Cover) | Some(Work::Mag) | None => {}
    }
    if bench.work.is_none() {
        f.mode = f
            .def
            .modes
            .iter()
            .position(|m| *m == FireMode::Safe)
            .unwrap_or(0);
    }
    for h in f.hammers_cocked.iter_mut() {
        *h = cocked;
    }
    let mut motion = Motion::new(&f, bench.spec.travel);
    motion.hinge = bench.spec.hinge;
    match bench.work {
        Some(Work::Trigger) => motion.trigger = k,
        Some(Work::Mag) => {
            // The catch first, then the magazine rocks out after it.
            motion.mag_catch = (k * 3.0).min(1.0);
            mag_out = ((k - 0.3) / 0.7).clamp(0.0, 1.0);
        }
        Some(Work::Cover) => motion.dust_cover = k,
        _ => {}
    }
    let x = bench.exploded;
    for (piece, mut t) in &mut pieces {
        let d = delta(
            piece.moves,
            piece.rest.translation.to_array(),
            piece.rest.translation.to_array(),
            &motion,
        );
        *t = apply(piece.rest, &d);
        t.translation += explode(piece) * x;
    }
    for (m, mut t) in &mut mags {
        // Out of the well along its own axis, down and a little forward.
        let mut s = m.seat;
        s.translation += s.rotation * Vec3::new(0.0, -0.06, -0.01) * mag_out;
        s.translation += Vec3::new(0.0, -0.12, 0.0) * x;
        *t = s;
    }
}

/// Where an exploded view puts a part, against its rest (gun space).
fn explode(p: &Piece) -> Vec3 {
    match p.moves {
        Moves::WithBolt | Moves::TurnBolt | Moves::CockingPiece => Vec3::new(0.0, 0.10, 0.0),
        Moves::DustCover => Vec3::new(0.0, 0.18, 0.0),
        Moves::Selector | Moves::Crossbolt | Moves::Gate | Moves::Cylinder => {
            Vec3::new(0.07, 0.0, 0.0)
        }
        Moves::Trigger => Vec3::new(0.0, -0.07, 0.0),
        Moves::MagCatch => Vec3::new(0.0, -0.05, -0.03),
        Moves::HeelCatch => Vec3::new(0.0, -0.03, 0.04),
        Moves::Hammer => Vec3::new(0.0, 0.06, 0.04),
        Moves::Barrels => Vec3::new(0.0, 0.0, -0.1),
        Moves::Bipod | Moves::Bayonet => Vec3::new(0.0, -0.08, 0.0),
        Moves::Static if p.name.starts_with("static_butt") => Vec3::new(0.0, 0.0, 0.12),
        _ => Vec3::ZERO,
    }
}

/// A part's rest pose moved by a gunsmith's [`Delta`] (as the game's
/// `weapons::visuals::apply`).
fn apply(rest: Transform, d: &Delta) -> Transform {
    let mut r = rest;
    if let Some((axis, angle)) = d.turn {
        let turn = match axis {
            Axis::X => Quat::from_rotation_x(angle),
            Axis::Y => Quat::from_rotation_y(angle),
            Axis::Z => Quat::from_rotation_z(angle),
        };
        let pivot = Vec3::from_array(d.pivot);
        r.translation = pivot + turn * (r.translation - pivot);
        r.rotation = turn * r.rotation;
    }
    r.translation += Vec3::from_array(d.shift);
    r.scale *= Vec3::from_array(d.scale);
    r
}

// --- Camera -------------------------------------------------------------------

fn measure(
    mut bounds: ResMut<Bounds>,
    meshes: Query<(&GlobalTransform, &bevy::camera::primitives::Aabb), With<Mesh3d>>,
) {
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for (g, aabb) in &meshes {
        let c = Vec3::from(aabb.center);
        let h = Vec3::from(aabb.half_extents);
        for s in [-1.0, 1.0] {
            for t in [-1.0, 1.0] {
                for u in [-1.0, 1.0] {
                    let p = g.transform_point(c + h * Vec3::new(s, t, u));
                    lo = lo.min(p);
                    hi = hi.max(p);
                }
            }
        }
    }
    if lo.x <= hi.x {
        bounds.lo = lo;
        bounds.hi = hi;
    }
}

fn frame_camera(
    bench: Res<Bench>,
    bounds: Res<Bounds>,
    cam: Single<(&mut Transform, &mut Projection), With<BenchCamera>>,
) {
    let (mut cam, mut projection) = cam.into_inner();
    if let Some(h) = bench.ortho {
        if !matches!(*projection, Projection::Orthographic(_)) {
            *projection = Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical { viewport_height: h },
                near: -5.0,
                far: 5.0,
                ..OrthographicProjection::default_3d()
            });
        }
    }
    let centre = (bounds.lo + bounds.hi) / 2.0;
    // Close up, the action: the receiver over the grip, where the hands
    // and the eye are. Further off, the whole gun.
    let focus = bench.focus.unwrap_or(if bench.dist < 0.6 {
        Vec3::new(0.0, 0.05, -0.07_f32.max(bounds.lo.z * 0.15))
    } else {
        centre
    });
    let (yaw, pitch) = (bench.yaw.to_radians(), bench.pitch.to_radians());
    // Yaw 0 looks at the right side (+X) square on; positive turns toward
    // the butt.
    let dir = Vec3::new(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        yaw.sin() * pitch.cos(),
    );
    let up = if pitch.abs() > 80f32.to_radians() {
        Vec3::NEG_Z
    } else {
        Vec3::Y
    };
    *cam = Transform::from_translation(focus + dir * bench.dist).looking_at(focus, up);
}

// --- Input and shots ------------------------------------------------------------

fn keys(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    bounds: Res<Bounds>,
    mut bench: ResMut<Bench>,
) {
    if bench.out.is_some() {
        return;
    }
    if input.just_pressed(KeyCode::KeyP) {
        report(&bench, &bounds);
    }
    if bench.turning {
        bench.yaw += 12.0 * time.delta_secs();
    }
    let views = [
        (KeyCode::Digit1, "right"),
        (KeyCode::Digit2, "left"),
        (KeyCode::Digit3, "top"),
        (KeyCode::Digit4, "threeq"),
        (KeyCode::Digit5, "eye"),
        (KeyCode::Digit6, "front"),
        (KeyCode::Digit7, "under"),
    ];
    for (k, name) in views {
        if input.just_pressed(k) {
            let (y, p) = view(name).unwrap();
            bench.yaw = y;
            bench.pitch = p;
            bench.turning = false;
        }
    }
    let works = [
        (KeyCode::KeyB, Work::Bolt),
        (KeyCode::KeyS, Work::Safety),
        (KeyCode::KeyM, Work::Mag),
        (KeyCode::KeyC, Work::Cover),
        (KeyCode::KeyT, Work::Trigger),
    ];
    for (k, w) in works {
        if input.just_pressed(k) {
            bench.work = Some(w);
            bench.clock = 0.0;
        }
    }
    if input.just_pressed(KeyCode::Digit0) {
        bench.work = None;
    }
    if input.just_pressed(KeyCode::Space) {
        bench.turning = !bench.turning;
    }
    if input.just_pressed(KeyCode::KeyE) {
        bench.exploding = !bench.exploding;
    }
    if input.pressed(KeyCode::Minus) {
        bench.dist = (bench.dist - 0.4 * time.delta_secs()).max(0.12);
    }
    if input.pressed(KeyCode::Equal) {
        bench.dist = (bench.dist + 0.4 * time.delta_secs()).min(3.0);
    }
}

/// The model's size against the real gun's (with the magazine in, at rest).
fn report(bench: &Bench, b: &Bounds) {
    let size = (b.hi - b.lo) * 1000.0;
    println!(
        "bench: {} drawn {:.0} long x {:.0} high x {:.0} wide (mm)",
        bench.spec.id, size.z, size.y, size.x
    );
    if let Some(r) = bench.spec.real {
        let pct = |m: f32, r: f32| (m / r - 1.0) * 100.0;
        println!(
            "bench: real length {:.0} mm: the model is {:+.1}%",
            r.length,
            pct(size.z, r.length)
        );
        if let Some(h) = r.height {
            println!(
                "bench: real height {h:.0} mm: the model is {:+.1}%",
                pct(size.y, h)
            );
        }
        if let Some(w) = r.width {
            println!(
                "bench: real width {w:.1} mm: the model is {:+.1}%",
                pct(size.x, w)
            );
        }
    }
}

fn take_shots(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    loading: Res<Loading>,
    bounds: Res<Bounds>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(dir) = bench.out.clone() else {
        return;
    };
    if !loading.done {
        return;
    }
    if bench.wait > 0 {
        bench.wait -= 1;
        return;
    }
    let n = bench.taken as usize;
    // Each shot: set up, let it settle, take it, let the capture land.
    if bench.armed {
        let shot = bench.shots[n].clone();
        let path = dir.join(format!("{}.png", shot.name));
        let _ = std::fs::create_dir_all(&dir);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path.clone()));
        println!("bench: {}", path.display());
        bench.armed = false;
        bench.taken += 1;
        bench.wait = 4;
        return;
    }
    if n >= bench.shots.len() {
        if !bench.closing {
            report(&bench, &bounds);
            bench.closing = true;
            bench.wait = 20;
        } else {
            exit.write(AppExit::Success);
        }
        return;
    }
    let shot = bench.shots[n].clone();
    let (yaw, pitch) = view(&shot.view).unwrap_or_else(|| panic!("no view {}", shot.view));
    bench.yaw = yaw;
    bench.pitch = pitch;
    bench.dist = shot.dist;
    bench.focus = shot.focus;
    bench.ortho = shot.ortho;
    bench.exploding = shot.exploded;
    bench.work = shot.work.map(|(w, _)| w);
    bench.hold = Some(shot.work.map_or(0.0, |(_, t)| t));
    bench.armed = true;
    bench.wait = 6;
}
