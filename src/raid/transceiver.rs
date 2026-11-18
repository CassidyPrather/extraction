//! The field sets: a fixed radio at every way out, to call the truck from
//! (#94: the carried radio was the one way home, and a career saved
//! without one could never end). Each pad's hangs at chest height on a
//! wall by it, facing out (a map without one: on the beacon's pole, or for
//! the short run a post of its own). Pressed, it calls
//! the truck to that pad (`RadioCall { pad }`), not to the one nearest the
//! caller: you call from where you'll wait.
//!
//! A call is heard (`robots::pressure::Transmitted`): each pad has a
//! weight ([`PadKind`]). A calm pad's call stirs the machines little; a
//! busy pad, where they're at work in sight, gets a tell at the mast and
//! an answer to it; the Gauge house's set sounds the dam's alarm horn
//! across the valley.
//!
//! Where each hangs, and the horn, are the map's (`map_extract`,
//! `siren_mast`).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::map_extract;
use extraction_core::map_landmarks::beacon_at;
use extraction_core::siren_mast;

use super::{Phase, RadioCall, Raid, SHORT_EXFIL};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::raid::truck::Pressable;
use crate::render::world_props::{Blockout, SwapIn};
use crate::robots::pressure::Transmitted;
use crate::world::{Layer, Zone};

/// A fixed field set: which pad it calls the truck to (a rendezvous's
/// index, or [`SHORT_EXFIL`]).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transceiver {
    pub pad: usize,
    pub kind: PadKind,
}

/// What a call from a pad's set costs (#94, the pads Map design surveyed):
/// how near machines at work are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKind {
    /// Nothing at work near it.
    Calm,
    /// Something within earshot, not in sight.
    Between,
    /// Machines at work in sight of the pad: a tell, then an answer.
    Busy,
    /// Sallow's Gauge house: the set's wired to the dam's alarm horn.
    Horn,
    /// The gate's short run.
    Short,
}

impl PadKind {
    /// A rendezvous's kind, by its name.
    pub fn of(name: &str) -> Self {
        match name {
            "Rail yard" | "Hunting lodge" | "Fire lookout" | "Sallow Head" | "The saddle" => {
                Self::Calm
            }
            "Ranger tower" | "Mine yard" | "North sidings" | "Tip road" => Self::Busy,
            "Gauge house" => Self::Horn,
            _ => Self::Between,
        }
    }

    /// What a call stirs the machines' account by.
    pub fn stir(self) -> f32 {
        match self {
            Self::Short => 0.1,
            Self::Calm => 0.2,
            Self::Between => 0.35,
            Self::Busy => 0.6,
            Self::Horn => 1.0,
        }
    }

    /// Whether the mast gives a tell before an answer. Not until the
    /// tell can be heard ([`MAST_WHINE_HEARD`]): an unheard tell is no
    /// warning. The horn is its own tell.
    pub fn busy(self) -> bool {
        MAST_WHINE_HEARD && matches!(self, Self::Busy | Self::Horn)
    }
}

/// Whether the mast's tell has a sound (Audio's whine, `mast.tell`). It
/// has: a busy pad's call is told at the mast, then answered. (Off again
/// would leave a busy pad its weight alone.)
pub const MAST_WHINE_HEARD: bool = true;

/// Where the dam's alarm horn is: on the gauge house's siren mast
/// (`siren_mast`, tagged `gauge_horn`), where the valley has one.
pub fn gauge_horn(zone: &Zone) -> Option<Vec3> {
    siren_mast::gauge_horn(&zone.map).map(|p| Vec3::from(siren_mast::horn_at(&zone.hm, p)))
}

/// The set's box (m): a field set bolted on, its face to you.
const SIZE: Vec3 = Vec3::new(0.32, 0.26, 0.14);
/// How far a set's back stands off its wall: on its board, on standoffs,
/// proud of the ivy (`art/props/field_set.py`, `OFF`).
const OFF_WALL: f32 = 0.13;
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
        app.add_message::<Transmitted>()
            .add_systems(Update, (spawn_sets, press.before(super::radio_calls)));
    }
}

/// Where a set stands: which pad it calls, its middle, the way its face
/// turns (a yaw), and the foot of the post it's on, if it's on one.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub set: Transceiver,
    pub at: Vec3,
    pub yaw: f32,
    pub post: Option<Vec3>,
}

/// Each pad's set: hung on the wall the map gives it (a `FieldSet`
/// marker, `map_extract`: its back to the wall, facing out); else, for a
/// rendezvous, on its beacon's pole, and for the short run on a post of
/// its own, facing the valley's middle.
pub fn sets(zone: &Zone) -> Vec<Site> {
    let walls = map_extract::field_sets(&zone.map);
    let wall = |pad: &str| {
        walls.iter().find(|w| w.0 == pad).map(|&(_, at, rot)| {
            // A marker's front is (-sin, cos) in the map; a set's face is
            // its +Z, which a yaw of -rot turns there.
            let yaw = -rot.to_radians();
            let front = Quat::from_rotation_y(yaw) * Vec3::Z;
            (
                zone.on_ground(at[0], at[1]) + front * (OFF_WALL + SIZE.z * 0.5),
                yaw,
            )
        })
    };
    let middle = |at: [f32; 2]| {
        let f = Vec3::new(-at[0], 0.0, -at[1]).normalize_or(Vec3::Z);
        (f, f.x.atan2(f.z))
    };
    let mut out = Vec::new();
    for (i, rv) in zone.map.markers_of(&MarkerKind::Rendezvous).enumerate() {
        let set = Transceiver {
            pad: i,
            kind: PadKind::of(&rv.name),
        };
        let (foot, yaw) = wall(&rv.name).unwrap_or_else(|| {
            let at = beacon_at(rv.at);
            let (f, yaw) = middle(at);
            (
                zone.on_ground(at[0], at[1]) + f * (POLE_R + SIZE.z * 0.5),
                yaw,
            )
        });
        out.push(Site {
            set,
            at: foot + Vec3::Y * CHEST,
            yaw,
            post: None,
        });
    }
    let set = Transceiver {
        pad: SHORT_EXFIL,
        kind: PadKind::Short,
    };
    let shorts: Vec<[f32; 2]> = zone
        .map
        .markers_of(&MarkerKind::ShortExfil)
        .map(|m| m.at)
        .collect();
    match wall(map_extract::SHORT) {
        Some((foot, yaw)) if !shorts.is_empty() => out.push(Site {
            set,
            at: foot + Vec3::Y * CHEST,
            yaw,
            post: None,
        }),
        _ => {
            for at in shorts {
                let (_, yaw) = middle(at);
                let foot = zone.on_ground(at[0], at[1]);
                let turn = Quat::from_rotation_y(yaw);
                out.push(Site {
                    set,
                    at: foot + turn * Vec3::new(0.0, CHEST, POST_R + SIZE.z * 0.5),
                    yaw,
                    post: Some(foot),
                });
            }
        }
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
    for Site { set, at, yaw, post } in sets(&zone) {
        let turn = Quat::from_rotation_y(yaw);
        if let Some(foot) = post {
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
            // A box and its grille until the model's in (`art/props/
            // field_set.py`: its front is -Z, the set's face +Z).
            e.insert(SwapIn(vec![(
                MODEL,
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
            )]))
            .with_children(|c| {
                c.spawn((
                    Name::new("Field set (blockout)"),
                    Blockout,
                    Mesh3d(l.body.clone()),
                    MeshMaterial3d(l.olive.clone()),
                    Transform::IDENTITY,
                ));
                c.spawn((
                    Name::new("Field set grille (blockout)"),
                    Blockout,
                    Mesh3d(l.grille.clone()),
                    MeshMaterial3d(l.dark.clone()),
                    Transform::from_xyz(0.0, 0.0, SIZE.z * 0.5 + 0.005),
                ));
            });
        }
    }
}

/// The set's model under `models/props/`.
pub const MODEL: &str = "field_set";

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

/// Pressed: the truck, to this set's pad; and, out in the valley, the
/// call is heard.
fn press(
    mut events: MessageReader<InteractEvent>,
    sets: Query<(&Transceiver, &GlobalTransform)>,
    raid: Option<Res<Raid>>,
    zone: Option<Res<Zone>>,
    mut calls: MessageWriter<RadioCall>,
    mut heard: MessageWriter<Transmitted>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, player } = ev else {
            continue;
        };
        let Ok((set, at)) = sets.get(*item) else {
            continue;
        };
        calls.write(RadioCall {
            player: *player,
            pad: Some(set.pad),
        });
        let out = raid.as_ref().is_some_and(|r| {
            matches!(
                r.phase,
                Phase::Out | Phase::TruckCalled { .. } | Phase::TruckWaiting { .. }
            )
        });
        if out {
            heard.write(transmitted(set.kind, at.translation(), zone.as_deref()));
        }
    }
}

/// What a call from a set of this kind, here, is to the machines.
pub fn transmitted(kind: PadKind, at: Vec3, zone: Option<&Zone>) -> Transmitted {
    let horn = (kind == PadKind::Horn)
        .then_some(zone)
        .flatten()
        .and_then(gauge_horn);
    Transmitted {
        at,
        stir: kind.stir(),
        busy: kind.busy(),
        horn,
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

    /// Every pad name the weights know is a rendezvous in some valley, and
    /// every valley has a calm pad: a rename can't silently make a pad
    /// "between", and there's always a quiet way home.
    #[test]
    fn the_pad_weights_name_real_pads() {
        let named = [
            "Rail yard",
            "Hunting lodge",
            "Fire lookout",
            "Sallow Head",
            "The saddle",
            "Ranger tower",
            "Mine yard",
            "North sidings",
            "Tip road",
            "Gauge house",
        ];
        let mut seen = Vec::new();
        for valley in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let text = std::fs::read_to_string(
                crate::asset_root_path().join(format!("maps/{valley}.ron")),
            )
            .unwrap();
            let map = extraction_core::map::MapDef::from_ron(&text).unwrap();
            let kinds: Vec<PadKind> = map
                .markers_of(&MarkerKind::Rendezvous)
                .map(|m| {
                    seen.push(m.name.clone());
                    PadKind::of(&m.name)
                })
                .collect();
            assert!(kinds.contains(&PadKind::Calm), "{valley}: no calm pad");
        }
        for n in named {
            assert!(seen.iter().any(|s| s == n), "no rendezvous named {n}");
        }
    }

    /// In every valley each set hangs where the map says, on its wall
    /// (`FieldSet`): at chest height off the marker, its face along the
    /// marker's front, and none on a post.
    #[test]
    fn every_set_hangs_on_the_wall_the_map_gives_it() {
        for valley in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let zone = Zone::load(valley);
            let walls = map_extract::field_sets(&zone.map);
            let names: Vec<String> = zone
                .map
                .markers_of(&MarkerKind::Rendezvous)
                .map(|m| m.name.clone())
                .collect();
            let sites = sets(&zone);
            assert_eq!(sites.len(), names.len() + 1, "{valley}");
            for site in sites {
                assert!(site.post.is_none(), "{valley}: {:?} on a post", site.set);
                let pad = if site.set.pad == SHORT_EXFIL {
                    map_extract::SHORT
                } else {
                    names[site.set.pad].as_str()
                };
                let &(_, at, rot) = walls
                    .iter()
                    .find(|w| w.0 == pad)
                    .unwrap_or_else(|| panic!("{valley}: no wall for {pad}"));
                let flat = Vec2::new(site.at.x - at[0], site.at.z - at[1]);
                let off = flat.length() - (OFF_WALL + SIZE.z * 0.5);
                assert!(off.abs() < 0.01, "{valley} {pad}: {flat} off its mark");
                let up = site.at.y - zone.ground(at[0], at[1]);
                assert!((up - CHEST).abs() < 0.3, "{valley} {pad}: {up} m up");
                let face = Quat::from_rotation_y(site.yaw) * Vec3::Z;
                let (fs, fc) = rot.to_radians().sin_cos();
                let front = Vec3::new(-fs, 0.0, fc);
                assert!(
                    face.dot(front) > 0.99,
                    "{valley} {pad}: faces {face}, not {front}"
                );
            }
        }
    }

    /// The Gauge house's call sounds the horn on the siren mast, near its
    /// top; no other valley has the horn.
    #[test]
    fn the_gauge_house_horn_is_on_its_siren_mast() {
        let sallow = Zone::load("sallow_reservoir");
        let horn = gauge_horn(&sallow).expect("Sallow's siren mast");
        let mast = siren_mast::gauge_horn(&sallow.map).unwrap();
        let up = horn.y - sallow.ground(mast.at[0], mast.at[1]);
        assert!((up - siren_mast::HORN[1]).abs() < 1.5, "{up} m up");
        let call = transmitted(PadKind::Horn, Vec3::ZERO, Some(&sallow));
        assert_eq!(call.horn, Some(horn));
        assert!(gauge_horn(&Zone::load("marrow_creek")).is_none());
    }

    /// A call from a set is heard by its pad's weight: the Gauge house
    /// sounds the horn; a busy pad gives no tell until the tell can be
    /// heard; a calm one stirs little.
    #[test]
    fn a_call_is_heard_by_its_pads_weight() {
        let gauge = transmitted(PadKind::Horn, Vec3::ZERO, None);
        assert_eq!((gauge.stir, gauge.horn), (1.0, None));
        let busy = transmitted(PadKind::Busy, Vec3::ZERO, None);
        assert_eq!((busy.stir, busy.busy), (0.6, MAST_WHINE_HEARD));
        let calm = transmitted(PadKind::Calm, Vec3::ZERO, None);
        assert!(calm.stir < busy.stir && !calm.busy && calm.horn.is_none());
        let short = transmitted(PadKind::Short, Vec3::ZERO, None);
        assert!(short.stir < calm.stir);
    }

    /// Pressed out in the valley, a set's call is heard (one
    /// `Transmitted`, at the set); pressed at base, nothing is.
    #[test]
    fn a_press_out_there_is_heard_and_one_at_base_is_not() {
        let mut app = app();
        let me = player(&mut app);
        let set = app
            .world_mut()
            .query::<(Entity, &Transceiver)>()
            .iter(app.world())
            .find(|(_, t)| t.pad == 0)
            .map(|(e, _)| e)
            .unwrap();
        let heard = |app: &mut App| {
            app.world_mut()
                .resource_mut::<Messages<Transmitted>>()
                .drain()
                .count()
        };
        app.world_mut().resource_mut::<Raid>().phase = Phase::AtBase;
        heard(&mut app);
        app.world_mut().write_message(InteractEvent::Pressed {
            item: set,
            player: me,
        });
        app.update();
        assert_eq!(heard(&mut app), 0, "at base");
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.world_mut().write_message(InteractEvent::Pressed {
            item: set,
            player: me,
        });
        app.update();
        assert_eq!(heard(&mut app), 1, "out");
    }

    /// A busy pad's call is told: pressed out there, its set sends
    /// `busy`, and the mast gives its tell (`MastTell`, the whine) before
    /// any answer.
    #[test]
    fn a_busy_pads_call_is_told_at_its_mast() {
        use crate::robots::pressure::MastTell;
        let mut app = app();
        let me = player(&mut app);
        let (set, at) = app
            .world_mut()
            .query::<(Entity, &Transceiver, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, t, _)| t.kind == PadKind::Busy)
            .map(|(e, _, g)| (e, g.translation()))
            .expect("the default valley has a busy pad");
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        let mut sent = app
            .world()
            .resource::<Messages<Transmitted>>()
            .get_cursor_current();
        let mut told = app
            .world()
            .resource::<Messages<MastTell>>()
            .get_cursor_current();
        app.world_mut().write_message(InteractEvent::Pressed {
            item: set,
            player: me,
        });
        let (mut busy, mut tell) = (false, None);
        for _ in 0..30 {
            app.update();
            let w = app.world();
            busy |= sent
                .read(w.resource::<Messages<Transmitted>>())
                .any(|t| t.busy);
            if let Some(t) = told.read(w.resource::<Messages<MastTell>>()).last() {
                tell = Some(t.at);
            }
            if busy && tell.is_some() {
                break;
            }
        }
        assert!(busy, "the call wasn't sent as busy");
        let tell = tell.expect("no tell at the mast");
        assert!(
            tell.distance(at) < 0.01,
            "told at {tell}, the set's at {at}"
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
