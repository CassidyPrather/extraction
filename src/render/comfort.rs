//! Headset comfort and diegetic readouts.
//!
//! - A **vignette** closes in at the edges of the view while the stick is
//!   moving or turning you (not when you walk for real), the standard cure
//!   for artificial-locomotion queasiness. Strength from [`Comfort`].
//! - A **wrist panel** on the left glove takes the flat HUD's job in a
//!   headset: the Surge, your load, the last thing said on the radio.

use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::rig::comfort::Comfort;
use crate::rig::{LocalPlayer, RigDriver, RigHead};

pub struct ComfortPlugin;

impl Plugin for ComfortPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (add_vignette, vignette, add_wrist_panel, wrist_panel),
        );
    }
}

#[derive(Component)]
struct Vignette {
    material: Handle<StandardMaterial>,
    strength: f32,
    last_yaw: f32,
}

fn ring_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 128u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = ((r - 0.32) / 0.38).clamp(0.0, 1.0);
            let a = a * a * (3.0 - 2.0 * a);
            data.extend_from_slice(&[0, 0, 0, (a * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

fn add_vignette(
    mut commands: Commands,
    heads: Query<(Entity, &ChildOf), Added<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (head, parent) in &heads {
        if !locals.contains(parent.parent()) {
            continue;
        }
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            base_color_texture: Some(ring_texture(&mut images)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            fog_enabled: false,
            ..default()
        });
        commands.entity(head).with_child((
            Vignette {
                material: material.clone(),
                strength: 0.0,
                last_yaw: 0.0,
            },
            Mesh3d(meshes.add(Rectangle::new(0.5, 0.5))),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, 0.0, -0.07),
            NotShadowCaster,
            Visibility::Hidden,
        ));
    }
}

/// Darkens with stick speed and turning, only in a headset.
fn vignette(
    mut vignettes: Query<(&mut Vignette, &mut Visibility)>,
    players: Query<(&crate::player::Body, &GlobalTransform), With<LocalPlayer>>,
    comfort: Res<Comfort>,
    driver: Res<RigDriver>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().max(1e-4);
    let Some((body, gt)) = players.iter().next() else {
        return;
    };
    let yaw = gt.rotation().to_euler(EulerRot::YXZ).0;
    for (mut v, mut vis) in &mut vignettes {
        let mut turn = (yaw - v.last_yaw).abs();
        if turn > std::f32::consts::PI {
            turn = std::f32::consts::TAU - turn;
        }
        v.last_yaw = yaw;
        let speed = Vec2::new(body.velocity.x, body.velocity.z).length();
        let want = if *driver == RigDriver::Xr {
            comfort.vignette * ((speed / 2.5) + (turn / dt / 1.5)).min(1.0)
        } else {
            0.0
        };
        // Close in quickly, open up gently.
        let rate = if want > v.strength { 8.0 } else { 2.5 };
        v.strength += (want - v.strength) * (rate * dt).min(1.0);
        *vis = if v.strength > 0.01 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if let Some(mut m) = materials.get_mut(&v.material) {
            m.base_color = Color::srgba(0.0, 0.0, 0.0, v.strength);
        }
    }
}

#[derive(Component)]
struct WristPanel;

/// The panel rides on the back of the left glove.
fn add_wrist_panel(
    mut commands: Commands,
    hands: Query<(Entity, &crate::rig::RigHand, &ChildOf), Added<crate::rig::RigHand>>,
    locals: Query<(), With<LocalPlayer>>,
) {
    for (e, hand, parent) in &hands {
        if hand.side != crate::rig::Side::Left || !locals.contains(parent.parent()) {
            continue;
        }
        commands.entity(e).with_child((
            WristPanel,
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.075, 0.045),
                style: ScreenStyle::Lcd,
            },
            // On top of the wrist, tilted toward the eyes when you look.
            Transform::from_xyz(0.0, 0.03, 0.07).with_rotation(Quat::from_rotation_x(-1.2)),
            Visibility::default(),
        ));
    }
}

fn wrist_panel(
    mut panels: Query<&mut ScreenText, With<WristPanel>>,
    raid: Res<crate::raid::Raid>,
    players: Query<(&crate::player::Body, &crate::health::Health), With<LocalPlayer>>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.5;
    let Some((body, health)) = players.iter().next() else {
        return;
    };
    let surge = raid.surge_in();
    let clock = if raid.phase == crate::raid::Phase::AtBase {
        "AT BASE".to_string()
    } else if surge > 0.0 {
        format!("SURGE {:>2.0}m", (surge / 60.0).ceil())
    } else {
        "SURGE NOW".to_string()
    };
    let mut text = format!(
        "{clock}  {:>4.1}kg\nHP {:>3.0}{}",
        body.carried_kg,
        health.hp.max(0.0),
        if health.bleeding > 0.05 { " BLEED" } else { "" }
    );
    if let Some((msg, _)) = raid.log.last() {
        for line in crate::render::screen::wrap(msg, WRIST_COLUMNS, 3) {
            text += "\n";
            text += &line;
        }
    }
    for mut p in &mut panels {
        if p.text != text {
            p.text = text.clone();
        }
    }
}

/// Characters per line on the wrist: narrow keeps the type big.
const WRIST_COLUMNS: usize = 26;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::screen::wrap;

    #[test]
    fn wrist_messages_wrap_rather_than_lose_their_end() {
        let msg =
            "Truck: \"No fuel in the stores for the short run. Head for an RV.\" And more after.";
        let lines = wrap(msg, 26, 3);
        assert!(lines.len() <= 3);
        assert!(lines.iter().all(|l| l.chars().count() <= 26), "{lines:?}");
        assert!(lines.last().unwrap().ends_with("..."), "{lines:?}");
        assert_eq!(wrap("Short one.", 26, 3), vec!["Short one."]);
        let fits = wrap("Truck: \"Get in the back.\"", 26, 3);
        assert_eq!(fits.join(" "), "Truck: \"Get in the back.\"");
    }
}
