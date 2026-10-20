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
            (
                add_vignette,
                vignette,
                add_wrist_panel,
                wrist_panel,
                add_radio_screens,
                radio_screens,
            ),
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
            crate::render::legibility::TextSource::Wrist,
            ScreenText {
                text: String::new(),
                size: WRIST_SIZE,
                style: ScreenStyle::Lcd,
            },
            // On the back of the wrist, on the glove's display housing, like
            // a watch: turn the wrist to read it.
            crate::render::props::wrist_mount(),
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
        format!("SURGE {:>2.0}M", (surge / 60.0).ceil())
    } else {
        "SURGE NOW".to_string()
    };
    let text = wrist_text(
        &clock,
        body.carried_kg,
        health.hp,
        health.bleeding > 0.05,
        raid.log.last().map(|(m, _)| m.as_str()),
        page(&time),
    );
    for mut p in &mut panels {
        if p.text != text {
            p.text = text.clone();
        }
    }
}

/// The field radio's own little screen: what it last heard.
#[derive(Component)]
pub struct RadioScreen;

/// Gives every field radio a backlit screen on its face.
fn add_radio_screens(
    mut commands: Commands,
    items: Query<(Entity, &crate::items::Item), Added<crate::items::Item>>,
    catalog: Res<crate::items::Catalog>,
) {
    for (e, item) in &items {
        if catalog.get(item.kind()).id != "radio" {
            continue;
        }
        commands.entity(e).with_child((
            RadioScreen,
            crate::render::legibility::TextSource::Radio,
            ScreenText {
                text: "CH4  ----".into(),
                size: RADIO_SIZE,
                style: ScreenStyle::Lcd,
            },
            // Over the tuning window, above the speaker grille (the
            // model's origin is near its top).
            Transform::from_xyz(0.0, 0.024, 0.0245),
            Visibility::default(),
        ));
    }
}

/// The last two transmissions, newest at the bottom; a channel line when
/// it's quiet.
fn radio_screens(
    mut screens: Query<&mut ScreenText, With<RadioScreen>>,
    raid: Res<crate::raid::Raid>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 || screens.is_empty() {
        return;
    }
    *timer = 0.5;
    let n = raid.log.len();
    let text = if n == 0 {
        format!(
            "CH4  {}",
            if (time.elapsed_secs() as u32) % 2 == 0 {
                "...."
            } else {
                " ..."
            }
        )
    } else {
        let last: Vec<String> = raid.log[n.saturating_sub(2)..]
            .iter()
            .map(|(m, _)| m.clone())
            .collect();
        radio_text(&last, page(&time))
    };
    for mut s in &mut screens {
        if s.text != text {
            s.text = text.clone();
        }
    }
}

/// Characters a line and lines a page on the radio's screen.
const RADIO_COLUMNS: usize = 15;
const RADIO_LINES: usize = 4;

/// Lines of the driver's words a page on the wrist.
const WRIST_LINES: usize = 3;

/// Seconds a page of a long message stays up.
const PAGE_SECS: f32 = 3.0;

/// The wrist readout's panel (as big as the cuff's housing takes) and the
/// radio's screen (m).
pub const WRIST_SIZE: Vec2 = Vec2::new(0.078, 0.048);
pub const RADIO_SIZE: Vec2 = Vec2::new(0.056, 0.032);

/// Which page of a long message shows now.
fn page(time: &Time) -> usize {
    (time.elapsed_secs() / PAGE_SECS) as usize
}

/// Page `page` (cycling) of `msg` in capitals, wrapped to `columns` and
/// `lines` a page: the little LCDs show capitals (30% taller than lower
/// case for the same type) and page a long message rather than shrink it
/// (`docs/LEGIBILITY.md`).
fn paged(msg: &str, columns: usize, lines: usize, page: usize) -> Vec<String> {
    let all = crate::render::screen::wrap(&msg.to_uppercase(), columns, usize::MAX);
    let pages = all.len().div_ceil(lines).max(1);
    let p = page % pages;
    all.into_iter().skip(p * lines).take(lines).collect()
}

/// What the wrist shows: the clock, the load, health, and the driver's
/// last words (a page at a time).
pub fn wrist_text(
    clock: &str,
    kg: f32,
    hp: f32,
    bleeding: bool,
    said: Option<&str>,
    page: usize,
) -> String {
    let mut text = format!(
        "{clock} {:>5.1}KG\nHP {:>3.0}{}",
        kg,
        hp.max(0.0),
        if bleeding { " BLEED" } else { "" }
    );
    if let Some(msg) = said {
        for line in paged(msg, WRIST_COLUMNS, WRIST_LINES, page) {
            text += "\n";
            text += &line;
        }
    }
    text
}

/// What the radio shows: the latest transmission, a page at a time.
pub fn radio_text(heard: &[String], page: usize) -> String {
    heard
        .last()
        .map(|m| paged(m, RADIO_COLUMNS, RADIO_LINES, page).join("\n"))
        .unwrap_or_default()
}

/// Characters per line on the wrist: narrow keeps the type big.
const WRIST_COLUMNS: usize = 18;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::screen::wrap;

    #[test]
    fn long_messages_page_rather_than_lose_their_end() {
        let msg = "Truck: \"No fuel in the stores for the short run. Head for an RV.\" And more.";
        let mut seen = String::new();
        for p in 0..4 {
            let lines = paged(msg, WRIST_COLUMNS, WRIST_LINES, p);
            assert!(lines.len() <= WRIST_LINES);
            assert!(lines.iter().all(|l| l.chars().count() <= WRIST_COLUMNS));
            if p < 2 {
                seen += &lines.join(" ");
                seen += " ";
            }
        }
        assert_eq!(seen.trim(), msg.to_uppercase());
        // It cycles.
        assert_eq!(
            paged(msg, WRIST_COLUMNS, WRIST_LINES, 0),
            paged(msg, WRIST_COLUMNS, WRIST_LINES, 2)
        );
    }

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
