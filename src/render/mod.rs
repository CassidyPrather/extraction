//! Rendering: custom materials, the camera stack, post-processing.

pub mod affordance;
pub mod arms;
pub mod bench;
pub mod blink;
pub mod body;
pub mod budget;
pub mod cards;
pub mod clear_eyes;
pub mod comfort;
pub mod cpu;
pub mod eye_projection;
pub mod first_contact;
pub mod first_sight;
pub mod fog;
pub mod gpu_lost;
pub mod grade;
pub mod gun_rest;
pub mod hand_push;
pub mod hold_rate;
pub mod kit;
pub mod kitchen;
pub mod landmarks;
pub mod legibility;
pub mod legibility_shots;
pub mod lod;
pub mod logistics;
pub mod loot_light;
pub mod materials;
pub mod memorial;
pub mod offering;
#[cfg(test)]
pub mod overdraw;
pub mod props;
pub mod pry_cue;
pub mod readback;
pub mod rv_beacons;
pub mod screen;
pub mod sound_meshes;
pub mod spikes;
pub mod stats;
pub mod stress;
pub mod tells;
pub mod terrain_material;
pub mod textures;
pub mod timeline;
pub mod tools;
pub mod upgrades;
pub mod vr_budget;
pub mod vram;
pub mod warmup;
pub mod water_material;
pub mod wayfind_shots;
pub mod world_props;
pub mod wounds;

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, RigHead};
use crate::world::sky::Weather;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        if app
            .world()
            .get_resource::<LaunchOptions>()
            .is_some_and(|o| o.stress)
        {
            app.add_plugins(stress::StressPlugin);
        }
        app.add_plugins((
            terrain_material::TerrainMaterialPlugin,
            water_material::WaterMaterialPlugin,
            fog::HeightFogPlugin,
            memorial::MemorialBoardPlugin,
            screen::ScreenPlugin,
            bench::BenchVisualsPlugin,
            kitchen::KitchenVisualsPlugin,
            offering::OfferingVisualsPlugin,
            logistics::LogisticsVisualsPlugin,
            comfort::ComfortPlugin,
            stats::RenderStatsPlugin,
            world_props::WorldPropsPlugin,
            kit::KitVisualsPlugin,
            upgrades::UpgradeVisualsPlugin,
            budget::BudgetPlugin,
        ))
        .add_plugins(crate::raid::board::BoardVisualsPlugin)
        .add_plugins(crate::raid::lev::LevVisualsPlugin)
        .add_plugins(crate::raid::epilogue::EpilogueVisualsPlugin)
        .add_plugins(cpu::CpuSplitPlugin)
        .add_plugins(warmup::WarmupPlugin)
        .add_plugins(first_sight::PreloadPlugin)
        .add_plugins(loot_light::LootLightPlugin)
        .add_plugins(timeline::TimelinePlugin)
        .add_plugins(sound_meshes::SoundMeshesPlugin)
        .add_plugins(hold_rate::HoldRatePlugin)
        .add_plugins(gpu_lost::GpuLostPlugin)
        .add_plugins(eye_projection::EyeProjectionPlugin)
        .add_plugins(textures::TexturesPlugin)
        .add_plugins(first_sight::EverythingPlugin)
        .add_plugins(gun_rest::GunRestPlugin)
        .add_plugins(wounds::WoundMarksPlugin)
        .add_plugins(clear_eyes::ClearEyesPlugin)
        .add_plugins(legibility_shots::LegibilityShotsPlugin)
        .add_plugins(wayfind_shots::WayfindShotsPlugin)
        .add_plugins(landmarks::LandmarkLampsPlugin)
        .add_plugins(rv_beacons::RvBeaconsPlugin)
        .add_plugins(pry_cue::PryCuePlugin)
        .init_resource::<lod::StandInMaterial>()
        .add_plugins(arms::ArmsPlugin)
        .add_plugins(body::BodyPlugin)
        .add_plugins(tools::ToolVisualsPlugin)
        .add_plugins(blink::BlinkPlugin)
        .add_plugins(hand_push::HandPushPlugin)
        .init_resource::<grade::Adaptation>()
        .init_resource::<grade::Below>()
        .init_resource::<grade::Dimmed>()
        .add_systems(First, grade::undim)
        .add_systems(PostUpdate, grade::emissives_to_the_eye)
        // After the underground's darkening, which takes whatever ambient it
        // finds for the sky's: run before it, the room's dimming was taken
        // for daylight and the light ran down to nothing indoors.
        .add_systems(
            PostUpdate,
            grade::dim_indoors.after(crate::world::underground::go_dark),
        )
        .add_systems(Update, attach_flat_camera)
        .add_systems(
            Update,
            grade::adapt_eyes.after(crate::world::sky::WeatherLook),
        );
    }
}

/// A camera that renders into a texture (scope lenses): not the player's
/// view, so systems looking for "the camera" skip it.
#[derive(Component)]
pub struct OffscreenCamera;

/// The camera `--camera` puts anywhere (screenshots, the tour).
#[derive(Component)]
pub struct FreeCamera;

/// The 3D cameras that might be a player's eyes, as a query filter: see
/// [`eyes`].
pub type EyeCamera = (With<Camera3d>, Without<OffscreenCamera>);

/// The player's eyes (or the flat camera): what distance-based detail
/// (near trees, shadows) is for. Not a camera drawing into an image (an
/// impostor capture's rig, 4 km down, while it waits on its picture; in
/// the GPU-less gauges it waits for ever).
pub fn eyes<'a>(
    cams: &'a Query<(&GlobalTransform, Option<&bevy::camera::RenderTarget>), EyeCamera>,
) -> impl Iterator<Item = &'a GlobalTransform> {
    cams.iter()
        .filter(|(_, t)| !matches!(t, Some(bevy::camera::RenderTarget::Image(_))))
        .map(|(g, _)| g)
}

/// In flat mode the local player's head is a camera. (The XR driver adds
/// its own per-eye cameras instead.)
fn attach_flat_camera(
    mut commands: Commands,
    zone: Res<crate::world::Zone>,
    heads: Query<(Entity, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    weather: Res<Weather>,
    opts: Res<LaunchOptions>,
    driver: Res<crate::rig::RigDriver>,
    mut frames: Local<u32>,
    mut done: Local<bool>,
) {
    // Give an XR session a moment to come up before deciding we're flat.
    *frames += 1;
    if *done || *driver == crate::rig::RigDriver::Xr || *frames < 3 {
        return;
    }
    *done = true;
    // A camera override (screenshots, flyovers) gets a free camera instead.
    if let Some((pos, yaw, pitch)) = opts.camera {
        let ground = zone.ground(pos.x, pos.z);
        // A height of 0 (or less) means standing there, eyes 1.7 m up.
        // Underground (in a site's rooms or on its stairs), as given.
        let y = if crate::world::underground::floor_under(&zone, pos).is_some() {
            pos.y
        } else if pos.y <= 0.0 {
            ground + 1.7
        } else {
            pos.y.max(ground + 0.3)
        };
        commands
            .spawn((
                Name::new("Free camera"),
                FreeCamera,
                Transform::from_xyz(pos.x, y, pos.z).with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    -yaw.to_radians(),
                    pitch.to_radians(),
                    0.0,
                )),
                camera_bundle(&weather),
            ))
            .with_children(|c| {
                // Held up by the eye and aimed where it looks, as anyone
                // looking at something by torchlight does.
                if opts.torch {
                    c.spawn((
                        crate::gadgets::visuals::torch_beam(),
                        Transform::from_xyz(0.15, -0.1, 0.0),
                    ));
                }
            });
        return;
    }
    for (head, parent) in &heads {
        if !locals.contains(parent.parent()) {
            continue;
        }
        commands.entity(head).insert(camera_bundle(&weather));
    }
}

/// Everything a player-view camera carries.
pub fn camera_bundle(weather: &Weather) -> impl Bundle {
    (
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 75f32.to_radians(),
            near: NEAR_CLIP,
            far: 2000.0,
            ..default()
        }),
        camera_look(weather),
    )
}

/// How close to the eye things still draw: iron sights sit a few
/// centimetres off it.
pub const NEAR_CLIP: f32 = 0.03;

/// The look every player-view camera shares, the headset's eyes included:
/// exposure, tonemapping and the weather's fog (which also hides where the
/// trees and grass are cut off).
pub fn camera_look(weather: &Weather) -> impl Bundle {
    (
        Hdr,
        Tonemapping::AgX,
        bevy::camera::Exposure { ev100: 10.6 },
        weather.fog(),
        // The valley's grade arrives with the weather (`sky::apply_weather`).
        bevy::render::view::ColorGrading::default(),
        Msaa::Sample4,
    )
}

/// A merged world-space mesh, re-centred on its own bounds: the handle,
/// the bounds and the transform that puts it back where it was.
///
/// Both halves matter for a `VisibilityRange`. The GPU's dither between
/// LODs measures from the mesh's origin, whatever `use_aabb` says, so a
/// mesh left at the world origin fades by the camera's distance from the
/// map centre; and a `RENDER_WORLD`-only mesh spawned late in a frame
/// loses its vertices before `calculate_bounds` sees them, so it never gets
/// an `Aabb` (nor frustum culling) unless it is given one.
pub fn centred(
    meshes: &mut Assets<Mesh>,
    mesh: Mesh,
) -> (Mesh3d, bevy::camera::primitives::Aabb, Transform) {
    let (mesh, aabb, at) = centre(mesh);
    (Mesh3d(meshes.add(mesh)), aabb, at)
}

/// `centred`'s work without the asset store, for a background task.
pub fn centre(mesh: Mesh) -> (Mesh, bevy::camera::primitives::Aabb, Transform) {
    use bevy::camera::primitives::MeshAabb;
    let c = mesh
        .compute_aabb()
        .map_or(Vec3::ZERO, |b| Vec3::from(b.center));
    let mesh = mesh.translated_by(-c);
    let aabb = mesh.compute_aabb().unwrap_or_default();
    (mesh, aabb, Transform::from_translation(c))
}
