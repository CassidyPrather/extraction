//! Rendering: custom materials, the camera stack, post-processing.

pub mod bench;
pub mod fog;
pub mod kitchen;
pub mod materials;
pub mod memorial;
pub mod offering;
pub mod props;
pub mod screen;
pub mod terrain_material;
pub mod water_material;

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;

use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, RigHead};
use crate::world::sky::Weather;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            terrain_material::TerrainMaterialPlugin,
            water_material::WaterMaterialPlugin,
            fog::HeightFogPlugin,
            memorial::MemorialBoardPlugin,
            screen::ScreenPlugin,
            bench::BenchVisualsPlugin,
            kitchen::KitchenVisualsPlugin,
            offering::OfferingVisualsPlugin,
        ))
        .add_systems(Update, attach_flat_camera);
    }
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
        commands.spawn((
            Name::new("Free camera"),
            Transform::from_xyz(pos.x, pos.y.max(ground + 0.3), pos.z).with_rotation(
                Quat::from_euler(EulerRot::YXZ, -yaw.to_radians(), pitch.to_radians(), 0.0),
            ),
            camera_bundle(&weather),
        ));
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
        Hdr,
        Projection::Perspective(PerspectiveProjection {
            fov: 75f32.to_radians(),
            near: 0.03,
            far: 2000.0,
            ..default()
        }),
        Tonemapping::AgX,
        bevy::camera::Exposure { ev100: 10.6 },
        weather.fog(),
        Msaa::Sample4,
    )
}
