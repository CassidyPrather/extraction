//! The field sets: a fixed radio at every way out, to call the truck from
//! (#94: the carried radio was the one way home, and a career saved
//! without one could never end). Each rendezvous has one on its beacon's
//! pole, at chest height, facing the valley's middle where you come from;
//! the gate's short run has one on a post of its own. Pressed, it calls
//! the truck to that pad (`RadioCall { pad }`), not to the one nearest the
//! caller: you call from where you'll wait.
//!
//! (Map design moves each onto its pad's wait-cover shed wall; a set at
//! the dam's gauge house trips the alarm, `robots`. Neither yet.)

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::map_landmarks::beacon_at;

use super::{RadioCall, SHORT_EXFIL};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::raid::truck::Pressable;
use crate::world::{Layer, Zone};

/// A fixed field set: which pad it calls the truck to (a rendezvous's
/// index, or [`SHORT_EXFIL`]).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transceiver {
    pub pad: usize,
}

/// The set's box (m): a field set bolted on, its face to you.
const SIZE: Vec3 = Vec3::new(0.32, 0.26, 0.14);
/// Its middle this high off the ground.
const CHEST: f32 = 1.3;
/// The beacon pole's radius (`render::rv_beacons`), and the gap to it.
const POLE_R: f32 = 0.08;
/// The gate's post: this tall, this thick.
const POST_H: f32 = 1.6;
const POST_R: f32 = 0.06;

pub struct TransceiverPlugin;

impl Plugin for TransceiverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_sets, press.before(super::radio_calls)));
    }
}

/// Where each set stands and which pad it calls: on each rendezvous's
/// beacon pole, then at each short run's marker. (Ground position, the
/// way it faces as a yaw: toward the valley's middle.)
pub fn sets(zone: &Zone) -> Vec<(Transceiver, Vec3, f32)> {
    let face = |at: [f32; 2]| {
        let f = Vec3::new(-at[0], 0.0, -at[1]).normalize_or(Vec3::Z);
        (f, f.x.atan2(f.z))
    };
    let mut out = Vec::new();
    for (i, rv) in zone.map.markers_of(&MarkerKind::Rendezvous).enumerate() {
        let at = beacon_at(rv.at);
        let (f, yaw) = face(at);
        let foot = zone.on_ground(at[0], at[1]) + f * (POLE_R + SIZE.z * 0.5);
        out.push((Transceiver { pad: i }, foot, yaw));
    }
    for m in zone.map.markers_of(&MarkerKind::ShortExfil) {
        let (_, yaw) = face(m.at);
        out.push((
            Transceiver { pad: SHORT_EXFIL },
            zone.on_ground(m.at[0], m.at[1]),
            yaw,
        ));
    }
    out
}

/// Once the valley's built (and again for another valley): its sets. With
/// no renderer (headless runs, the bots, tests) they're there to press,
/// unseen.
fn spawn_sets(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    old: Query<Entity, With<Transceiver>>,
    posts: Query<Entity, With<SetPost>>,
    mut built_for: Local<Option<String>>,
) {
    let Some(zone) = zone else {
        return;
    };
    if built_for.as_deref() == Some(zone.map.name.as_str()) {
        return;
    }
    *built_for = Some(zone.map.name.clone());
    for e in old.iter().chain(posts.iter()) {
        commands.entity(e).despawn();
    }
    let looks = meshes
        .zip(materials)
        .map(|(mut m, mut mats)| Looks::new(&mut m, &mut mats));
    for (set, foot, yaw) in sets(&zone) {
        let turn = Quat::from_rotation_y(yaw);
        if set.pad == SHORT_EXFIL {
            let mut post = commands.spawn((
                Name::new("Field set post"),
                SetPost,
                Transform::from_translation(foot + Vec3::Y * POST_H * 0.5),
                Visibility::default(),
                RigidBody::Static,
                Collider::cylinder(POST_R, POST_H),
            ));
            if let Some(l) = &looks {
                post.insert((Mesh3d(l.post.clone()), MeshMaterial3d(l.steel.clone())));
            }
        }
        let at = if set.pad == SHORT_EXFIL {
            foot + turn * Vec3::new(0.0, CHEST, POST_R + SIZE.z * 0.5)
        } else {
            foot + Vec3::Y * CHEST
        };
        let mut e = commands.spawn((
            Name::new(format!("Field set ({})", set.pad)),
            set,
            Pressable::Setting,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(SIZE.x, SIZE.y, SIZE.z),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            Transform::from_translation(at).with_rotation(turn),
            Visibility::default(),
        ));
        if let Some(l) = &looks {
            e.insert((Mesh3d(l.body.clone()), MeshMaterial3d(l.olive.clone())))
                .with_child((
                    Name::new("Field set grille"),
                    Mesh3d(l.grille.clone()),
                    MeshMaterial3d(l.dark.clone()),
                    Transform::from_xyz(0.0, 0.0, SIZE.z * 0.5 + 0.005),
                ));
        }
    }
}

/// The gate's post.
#[derive(Component)]
struct SetPost;

/// What a set's drawn with.
struct Looks {
    body: Handle<Mesh>,
    grille: Handle<Mesh>,
    post: Handle<Mesh>,
    olive: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    steel: Handle<StandardMaterial>,
}

impl Looks {
    fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        Self {
            body: meshes.add(Cuboid::new(SIZE.x, SIZE.y, SIZE.z)),
            grille: meshes.add(Cuboid::new(SIZE.x * 0.55, SIZE.y * 0.5, 0.01)),
            post: meshes.add(Cylinder::new(POST_R, POST_H)),
            olive: materials.add(StandardMaterial {
                base_color: Color::srgb(0.27, 0.31, 0.2),
                perceptual_roughness: 0.8,
                ..default()
            }),
            dark: materials.add(StandardMaterial {
                base_color: Color::srgb(0.06, 0.06, 0.05),
                perceptual_roughness: 0.9,
                ..default()
            }),
            steel: materials.add(StandardMaterial {
                base_color: Color::srgb(0.35, 0.36, 0.37),
                metallic: 0.6,
                perceptual_roughness: 0.6,
                ..default()
            }),
        }
    }
}

/// Pressed: the truck, to this set's pad.
fn press(
    mut events: MessageReader<InteractEvent>,
    sets: Query<&Transceiver>,
    mut calls: MessageWriter<RadioCall>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        if let Ok(set) = sets.get(*item) {
            calls.write(RadioCall {
                player: *player,
                pad: Some(set.pad),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raid::{Phase, Raid};
    use crate::rig::LocalPlayer;

    fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn player(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap()
    }

    /// A set at every way out: one on each rendezvous's beacon, one at
    /// each gate's short run.
    #[test]
    fn every_way_out_has_a_set() {
        let mut app = app();
        let zone = app.world().resource::<Zone>().clone();
        let rvs = zone.map.markers_of(&MarkerKind::Rendezvous).count();
        let gates = zone.map.markers_of(&MarkerKind::ShortExfil).count();
        assert!(rvs >= 3, "{rvs}");
        let mut pads: Vec<usize> = app
            .world_mut()
            .query::<&Transceiver>()
            .iter(app.world())
            .map(|t| t.pad)
            .collect();
        pads.sort();
        let mut want: Vec<usize> = (0..rvs).collect();
        want.extend(std::iter::repeat_n(SHORT_EXFIL, gates));
        assert_eq!(pads, want);
    }

    /// Pressed with no radio anywhere, a pad's set brings the truck to
    /// that pad, though another is nearer the caller.
    #[test]
    fn a_set_calls_the_truck_to_its_own_pad() {
        let mut app = app();
        let me = player(&mut app);
        let zone = app.world().resource::<Zone>().clone();
        let rvs: Vec<[f32; 2]> = zone
            .map
            .markers_of(&MarkerKind::Rendezvous)
            .map(|m| m.at)
            .collect();
        // Stood by pad 0, pressing pad 1's set.
        app.world_mut()
            .get_mut::<Transform>(me)
            .unwrap()
            .translation = zone.on_ground(rvs[0][0], rvs[0][1]);
        app.update();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let set = app
            .world_mut()
            .query::<(Entity, &Transceiver)>()
            .iter(app.world())
            .find(|(_, t)| t.pad == 1)
            .map(|(e, _)| e)
            .unwrap();
        app.world_mut().write_message(InteractEvent::Pressed {
            item: set,
            player: me,
        });
        app.update();
        assert!(
            matches!(
                app.world().resource::<Raid>().phase,
                Phase::TruckCalled { rv: 1, .. }
            ),
            "{:?}",
            app.world().resource::<Raid>().phase
        );
    }

    /// The gate's set is the short run: on a gentle job, free.
    #[test]
    fn the_gates_set_is_the_short_run() {
        let mut app = app();
        let me = player(&mut app);
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .done = 0;
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let set = app
            .world_mut()
            .query::<(Entity, &Transceiver)>()
            .iter(app.world())
            .find(|(_, t)| t.pad == SHORT_EXFIL)
            .map(|(e, _)| e)
            .unwrap();
        app.world_mut().write_message(InteractEvent::Pressed {
            item: set,
            player: me,
        });
        app.update();
        assert!(matches!(
            app.world().resource::<Raid>().phase,
            Phase::TruckWaiting {
                rv: SHORT_EXFIL,
                ..
            }
        ));
    }
}
