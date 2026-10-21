//! How the music's things move: a cassette player's door swinging, its
//! keys going down (play stays latched while it plays), its reels turning
//! behind the window. Client only; the model's named nodes
//! (`art/items/music.py`) are what moves.

use bevy::prelude::*;
use extraction_core::music::deck::{Key, Mode};

use super::deck::{DOOR_OPEN, DeckKey, TapeDeck};
use crate::interact::poke::{PokeButton, TRAVEL};

/// A moving node's pose as modelled.
#[derive(Component, Clone, Copy)]
struct Rest(Transform);

/// How far a latched key sits down (m).
const LATCHED: f32 = 0.006;

/// Turns per second of a reel playing (a cassette's hub at 4.76 cm/s
/// turns between about a half and two turns a second).
const REEL_PLAYING: f32 = 0.8;

fn key_node(key: Key) -> &'static str {
    match key {
        Key::Play => "btn_play",
        Key::Stop => "btn_stop",
        Key::Rewind => "btn_rew",
        Key::FastForward => "btn_ff",
    }
}

fn animate_decks(
    mut commands: Commands,
    decks: Query<(Entity, &TapeDeck)>,
    keys: Query<(&DeckKey, &PokeButton)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, d) in &decks {
        // How far each key is down: pushed by a finger, or latched.
        let mut down = [
            (Key::Play, 0.0f32),
            (Key::Stop, 0.0),
            (Key::Rewind, 0.0),
            (Key::FastForward, 0.0),
        ];
        for (k, b) in &keys {
            if k.deck != e {
                continue;
            }
            let latched = matches!(
                (k.key, d.deck.mode),
                (Key::Play, Mode::Playing)
                    | (Key::Rewind, Mode::Rewinding)
                    | (Key::FastForward, Mode::Winding)
            );
            let depth = b.depth.max(if latched { LATCHED } else { 0.0 });
            if let Some(slot) = down.iter_mut().find(|(key, _)| *key == k.key) {
                slot.1 = depth.min(TRAVEL);
            }
        }
        let reel_rate = match d.deck.mode {
            Mode::Playing => REEL_PLAYING,
            Mode::Rewinding => -REEL_PLAYING * 8.0,
            Mode::Winding => REEL_PLAYING * 8.0,
            Mode::Stopped => 0.0,
        } * std::f32::consts::TAU;
        for n in children.iter_descendants(e) {
            let Ok((name, mut t, rest)) = nodes.get_mut(n) else {
                continue;
            };
            let name = name.as_str();
            let moving =
                name == "door" || name.starts_with("btn_") || name == "reel_l" || name == "reel_r";
            if !moving {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*t));
                    *t
                }
            };
            if name == "door" {
                // Hinged on its bottom edge, the top swinging out (+Z).
                t.rotation = rest.rotation * Quat::from_rotation_x(DOOR_OPEN * d.door);
            } else if let Some((_, depth)) = down.iter().find(|(k, _)| key_node(*k) == name) {
                t.translation = rest.translation - Vec3::Y * *depth;
            } else if reel_rate != 0.0 {
                t.rotate_local_z(reel_rate * dt);
            }
        }
    }
}

/// The radio's knob shows where its dial is: as far round as the wrists
/// that turned it (`dial::KHZ_PER_RAD`), clockwise up the band.
fn turn_knobs(
    mut commands: Commands,
    radios: Query<(Entity, &super::dial::Tuner)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
) {
    for (e, t) in &radios {
        let turn = -(t.khz - super::dial::OFF) / super::dial::KHZ_PER_RAD;
        for n in children.iter_descendants(e) {
            let Ok((name, mut tf, rest)) = nodes.get_mut(n) else {
                continue;
            };
            if name.as_str() != "dial" {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*tf));
                    *tf
                }
            };
            let want = rest.rotation * Quat::from_rotation_y(turn);
            if tf.rotation.angle_between(want) > 1e-4 {
                tf.rotation = want;
            }
        }
    }
}

/// A prop whose model is still loading.
#[derive(Component)]
struct PendingProp(Handle<bevy::gltf::Gltf>);

/// Props drawn from their model (`models/props/<name>.glb`): the
/// gramophone and the piano. Their nodes, named, are what moves.
fn dress_props(
    mut commands: Commands,
    server: Res<AssetServer>,
    grams: Query<Entity, Added<super::gramophone::Gramophone>>,
    pianos: Query<Entity, Added<super::piano::Piano>>,
) {
    for (e, name) in grams
        .iter()
        .map(|e| (e, "gramophone"))
        .chain(pianos.iter().map(|e| (e, "upright_piano")))
    {
        commands
            .entity(e)
            .insert(PendingProp(server.load(format!("models/props/{name}.glb"))));
    }
}

fn spawn_nodes(
    commands: &mut Commands,
    parent: Entity,
    nodes: Vec<crate::models::ModelNode>,
    mat: &mut impl FnMut(Option<&str>) -> Handle<StandardMaterial>,
) {
    for mut node in nodes {
        let children = std::mem::take(&mut node.children);
        let n = commands
            .spawn((
                Name::new(node.name.clone()),
                node.transform,
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        crate::models::spawn_primitives_ranged(commands, n, &node, None, &mut *mat);
        spawn_nodes(commands, n, children, mat);
    }
}

fn finish_props(
    mut commands: Commands,
    pending: Query<(Entity, &PendingProp)>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    nodes: Res<Assets<bevy::gltf::GltfNode>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    mut model_mats: ResMut<crate::models::ModelMaterials>,
    mut lib: ResMut<crate::render::materials::MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::models::{ModelState, Models, model_nodes};
    for (e, p) in &pending {
        let gltf = match Models::state(&p.0, &server, &gltfs) {
            ModelState::Loading => continue,
            ModelState::Failed => {
                warn!("a music prop's model failed to load");
                commands.entity(e).remove::<PendingProp>();
                continue;
            }
            ModelState::Ready(g) => g,
        };
        commands.entity(e).remove::<PendingProp>();
        let model = model_nodes(gltf, &nodes, &gltf_meshes);
        let mut mat = |name: Option<&str>| {
            model_mats.get(name.unwrap_or("Wood"), &mut lib, &server, &mut materials)
        };
        spawn_nodes(&mut commands, e, model, &mut mat);
    }
}

/// The platter turns with the spring, the arm swings over the record when
/// it's down, the crank turns a little each push.
fn animate_gramophones(
    mut commands: Commands,
    grams: Query<(Entity, &super::gramophone::Gramophone)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
    time: Res<Time>,
    mut arm: Local<std::collections::HashMap<Entity, f32>>,
    mut crank: Local<std::collections::HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    for (e, g) in &grams {
        let a = arm.entry(e).or_default();
        let want = if g.arm_down { 0.55 } else { 0.0 };
        *a += (want - *a) * (dt * 4.0).min(1.0);
        let a = *a;
        // The crank shows the spring: a turn per push's worth.
        let c = crank.entry(e).or_default();
        let target = g.spring / super::gramophone::PER_TURN * std::f32::consts::TAU;
        *c += (target - *c) * (dt * 6.0).min(1.0);
        let c = *c;
        for n in children.iter_descendants(e) {
            let Ok((name, mut t, rest)) = nodes.get_mut(n) else {
                continue;
            };
            let name = name.as_str();
            if !matches!(name, "platter" | "tonearm" | "crank") {
                continue;
            }
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*t));
                    *t
                }
            };
            match name {
                // 78 turns a minute at full speed.
                "platter" => t.rotate_local_y(-g.speed * 78.0 / 60.0 * std::f32::consts::TAU * dt),
                "tonearm" => t.rotation = rest.rotation * Quat::from_rotation_y(a),
                _ => t.rotation = rest.rotation * Quat::from_rotation_x(c),
            }
        }
    }
}

/// Keys go down under the fingers that push them, pivoting at the back.
fn animate_pianos(
    mut commands: Commands,
    pianos: Query<(Entity, &super::piano::Piano)>,
    children: Query<&Children>,
    mut nodes: Query<(&Name, &mut Transform, Option<&Rest>)>,
) {
    for (e, p) in &pianos {
        for n in children.iter_descendants(e) {
            let Ok((name, mut t, rest)) = nodes.get_mut(n) else {
                continue;
            };
            let Some(key) = name
                .as_str()
                .strip_prefix("key_")
                .and_then(|k| k.parse::<u8>().ok())
            else {
                continue;
            };
            let depth = p
                .depth
                .get(key.wrapping_sub(super::piano::LOW) as usize)
                .copied()
                .unwrap_or(0.0);
            let rest = match rest {
                Some(r) => r.0,
                None => {
                    commands.entity(n).insert(Rest(*t));
                    *t
                }
            };
            // The front goes down: about the back pivot, front at -Z.
            let want = rest.rotation * Quat::from_rotation_x(-depth / 0.15);
            if t.rotation.angle_between(want) > 1e-4 {
                t.rotation = want;
            }
        }
    }
}

pub struct MusicVisualsPlugin;

impl Plugin for MusicVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                animate_decks,
                turn_knobs,
                dress_props,
                finish_props,
                animate_gramophones,
                animate_pianos,
            ),
        );
    }
}
