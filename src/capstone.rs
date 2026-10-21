//! The capstone: recalling the machines.
//!
//! The Quarantine Authority's terminals tell it. The white machines are its
//! Custodian, still sweeping the valleys for a review that never came, and
//! each valley's Gate is one of their depots. A depot answers only to the
//! Authority's three valley keys, presented together at any depot's lock.
//! - **The keys** (`MarkerKind::ValleyKey`): one per valley, where the
//!   notes say, at that valley's worst place (the Hollow, the quarry,
//!   the foot of the works chimney). A key lies there until someone takes
//!   it home. If the Surge sweeps up one that was dropped in the Zone, it
//!   turns up where it was again.
//! - **The lock**: a pedestal by every Gate, with a cradle and a screen. Set
//!   all three keys in the cradle in one raid (carry them out from the
//!   stash, and don't go down on the way) and the depot accepts the recall.
//!   The machines in the valley turn for home, the director stops sending
//!   them anywhere, and the career remembers: the valleys stay quiet.
//!
//! After that the game goes on, without the machines: a quiet Zone to
//! walk, the Surge still coming, an epilogue on the lock and at camp.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::raid::Raid;
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::world::{Layer, Zone};

/// The keys, in the order the lock lists them.
pub const KEYS: [(&str, &str); 3] = [
    ("key_marrow", "KEY-M  MARROW"),
    ("key_kettle", "KEY-K  KETTLE"),
    ("key_cinder", "KEY-C  CINDER"),
];

pub struct CapstonePlugin;

impl Plugin for CapstonePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (place_key, build_lock, turn_lock, quiet_valleys).chain(),
        );
    }
}

/// The lock by a Gate: where its cradle is, in the world.
#[derive(Component)]
pub struct Lock {
    pub cradle: Vec3,
}

#[derive(Component)]
struct LockScreen;

/// Dressed by the client (`render::props`): the pedestal, cradle, lamp.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapstoneProps {
    Pedestal,
    Strongbox,
}

/// How far from the cradle's middle a key still counts as set in it.
const CRADLE: Vec3 = Vec3::new(0.32, 0.25, 0.24);
const PEDESTAL_TOP: f32 = 1.0;

/// Once the Zone has settled (saves loaded, items placed), lays this
/// valley's key where it belongs if it isn't anywhere already.
fn place_key(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    catalog: Res<Catalog>,
    items: Query<&Item>,
    raid: Res<Raid>,
    mut frames: Local<u32>,
) {
    let Some(zone) = zone else {
        return;
    };
    *frames += 1;
    if *frames != 60 || raid.recalled {
        return;
    }
    for m in &zone.map.markers {
        let MarkerKind::ValleyKey { item } = &m.kind else {
            continue;
        };
        let Some(kind) = catalog.kind(item) else {
            warn!("valley key {item} isn't in the catalog");
            continue;
        };
        if items.iter().any(|i| i.kind() == kind) {
            continue;
        }
        let ground = zone.ground(m.at[0], m.at[1]);
        let yaw = Quat::from_rotation_y(-m.rot.to_radians());
        info!("capstone: {item} lies at {:?}", m.at);
        commands.spawn((
            Name::new("Key strongbox"),
            CapstoneProps::Strongbox,
            Transform::from_xyz(m.at[0], ground, m.at[1]).with_rotation(yaw),
            Visibility::default(),
            RigidBody::Static,
            Collider::compound(vec![(
                Vec3::new(0.0, 0.02, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(0.5, 0.04, 0.36),
            )]),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        spawn_item(
            &mut commands,
            &catalog,
            SpawnItem {
                kind,
                transform: Transform::from_xyz(m.at[0], ground + 0.2, m.at[1]).with_rotation(yaw),
                condition: 1.0,
            },
        );
    }
}

/// A lock beside every robot Gate.
fn build_lock(mut commands: Commands, zone: Option<Res<Zone>>, mut done: Local<bool>) {
    let Some(zone) = zone else {
        return;
    };
    if *done {
        return;
    }
    *done = true;
    for m in zone
        .map
        .markers
        .iter()
        .filter(|m| m.kind == MarkerKind::RobotGate)
    {
        // Beside the Gate's left pier, on the side the robots come out.
        let yaw = Quat::from_rotation_y(-m.rot.to_radians());
        let off = yaw * Vec3::new(-10.5, 0.0, -2.5);
        let at = Vec3::new(m.at[0] + off.x, 0.0, m.at[1] + off.z);
        let at = Vec3::new(at.x, zone.ground(at.x, at.z), at.z);
        let cradle = at + Vec3::Y * (PEDESTAL_TOP + 0.06);
        let lock = commands
            .spawn((
                Name::new("Depot lock"),
                Lock { cradle },
                CapstoneProps::Pedestal,
                Transform::from_translation(at).with_rotation(yaw),
                Visibility::default(),
                RigidBody::Static,
                Collider::compound(vec![(
                    Vec3::new(0.0, PEDESTAL_TOP * 0.5, 0.0),
                    Quat::IDENTITY,
                    Collider::cuboid(0.7, PEDESTAL_TOP, 0.55),
                )]),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
            ))
            .id();
        commands.spawn((
            LockScreen,
            crate::render::legibility::TextSource::LockScreen,
            ScreenText {
                text: String::new(),
                size: Vec2::new(0.42, 0.3),
                style: ScreenStyle::Crt,
            },
            // On a little mast behind the cradle, facing the way the lock
            // faces (the Gate's front).
            Transform::from_xyz(0.0, PEDESTAL_TOP + 0.45, -0.195),
            Visibility::default(),
            ChildOf(lock),
        ));
    }
}

/// Keys set in a lock's cradle; all three, and the depot takes the recall.
fn turn_lock(
    mut commands: Commands,
    locks: Query<(Entity, &Lock)>,
    mut screens: Query<(&ChildOf, &mut ScreenText), With<LockScreen>>,
    items: Query<(Entity, &Item, &GlobalTransform), Without<crate::interact::Held>>,
    catalog: Res<Catalog>,
    mut raid: ResMut<Raid>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.25;
    for (lock_e, lock) in &locks {
        let mut set: Vec<(usize, Entity)> = Vec::new();
        for (e, item, gt) in &items {
            let id = catalog.get(item.kind()).id.as_str();
            let Some(k) = KEYS.iter().position(|(kid, _)| *kid == id) else {
                continue;
            };
            let d = (gt.translation() - lock.cradle).abs();
            if d.x < CRADLE.x
                && d.y < CRADLE.y
                && d.z < CRADLE.z
                && !set.iter().any(|(s, _)| *s == k)
            {
                set.push((k, e));
            }
        }
        let all = set.len() == KEYS.len();
        if all && !raid.recalled {
            raid.recalled = true;
            raid.say("The lock takes the keys. The screen goes green.");
            raid.say("Across the valley, every machine stops, turns, and walks home.");
            for (_, e) in &set {
                commands.entity(*e).despawn();
            }
            // The epilogue, left on the lock.
            commands.spawn((
                Name::new("Epilogue"),
                crate::render::legibility::TextSource::Epilogue,
                ScreenText {
                    text: crate::render::screen::wrap_lines(EPILOGUE, 26),
                    size: Vec2::new(0.24, 0.32),
                    style: ScreenStyle::Paper,
                },
                Transform::from_xyz(0.18, PEDESTAL_TOP + 0.006, 0.12)
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                Visibility::default(),
                ChildOf(lock_e),
            ));
        }
        let text = if raid.recalled {
            "CUSTODIAN DEPOT\n\nRECALL ACCEPTED\nAll units returning.\nSweep: ended.\nReview: closed.".to_string()
        } else {
            let mut t = "CUSTODIAN DEPOT - LOCK\n\nRecall needs all three\nvalley keys, here:\n\n"
                .to_string();
            for (k, (_, name)) in KEYS.iter().enumerate() {
                let here = set.iter().any(|(s, _)| *s == k);
                t += &format!("{}  {name}\n", if here { "[OK]" } else { "[  ]" });
            }
            t
        };
        for (parent, mut screen) in &mut screens {
            if parent.parent() == lock_e && screen.text != text {
                screen.text = text.clone();
            }
        }
    }
}

/// With the machines recalled: no more sent out, and those out go home.
fn quiet_valleys(
    mut commands: Commands,
    raid: Res<Raid>,
    director: Option<ResMut<crate::robots::director::Director>>,
    // Residents walk back to their bays and sleep there (robots::indoor).
    robots: Query<
        Entity,
        (
            With<crate::robots::Robot>,
            Without<crate::robots::indoor::Resident>,
        ),
    >,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    if !raid.recalled {
        return;
    }
    if let Some(mut d) = director {
        if d.enabled {
            d.enabled = false;
        }
    }
    // One at a time, a few seconds apart, as if walking off.
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 3.0;
    if let Some(e) = robots.iter().next() {
        commands.entity(e).despawn();
    }
}

const EPILOGUE: &str = "For whoever comes after.

The depot took the keys. The machines stopped where they stood and walked back through the Gates, and the Gates closed behind them.

Nobody came for the review. Nobody is coming. The Surge still comes; the stones still grow where the ground is wrong.

The valleys are yours now. Walk them slowly.";

#[cfg(test)]
mod tests {
    use super::*;

    /// The three keys set in the lock: the machines are recalled, the
    /// director stops, and the keys are gone into the depot.
    #[test]
    fn three_keys_recall_the_machines() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        let cradle = {
            let w = app.world_mut();
            w.query::<&Lock>()
                .iter(w)
                .next()
                .expect("a lock by the Gate")
                .cradle
        };
        {
            let w = app.world_mut();
            let catalog = w.resource::<Catalog>().clone();
            let mut commands_queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut commands_queue, w);
            for (i, (id, _)) in KEYS.iter().enumerate() {
                spawn_item(
                    &mut commands,
                    &catalog,
                    SpawnItem {
                        kind: catalog.kind(id).unwrap(),
                        transform: Transform::from_translation(
                            cradle + Vec3::new(-0.2 + 0.2 * i as f32, 0.05, 0.0),
                        ),
                        condition: 1.0,
                    },
                );
            }
            commands_queue.apply(w);
        }
        for _ in 0..60 {
            app.update();
        }
        assert!(app.world().resource::<Raid>().recalled, "recalled");
        assert!(
            !app.world()
                .resource::<crate::robots::director::Director>()
                .enabled
        );
        let w = app.world_mut();
        let catalog = w.resource::<Catalog>().clone();
        let keys_left = w
            .query::<&Item>()
            .iter(w)
            .filter(|i| KEYS.iter().any(|(k, _)| catalog.get(i.kind()).id == *k))
            .count();
        assert_eq!(keys_left, 0, "the depot keeps the keys");
    }

    /// Every valley's map places its key, and each key is a real item.
    #[test]
    fn every_valley_hides_a_key() {
        let catalog =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        let mut found = Vec::new();
        for (name, text) in [
            ("marrow_creek", extraction_core::map::MARROW_CREEK),
            ("kettle_pass", extraction_core::map::KETTLE_PASS),
            ("cinder_works", extraction_core::map::CINDER_WORKS),
        ] {
            let map = extraction_core::map::MapDef::from_ron(text).unwrap();
            let keys: Vec<&String> = map
                .markers
                .iter()
                .filter_map(|m| match &m.kind {
                    MarkerKind::ValleyKey { item } => Some(item),
                    _ => None,
                })
                .collect();
            assert_eq!(keys.len(), 1, "{name} hides one key");
            assert!(catalog.kind(keys[0]).is_some(), "{} is an item", keys[0]);
            assert!(
                map.markers.iter().any(|m| m.kind == MarkerKind::RobotGate),
                "{name} has a Gate for the lock"
            );
            found.push(keys[0].clone());
        }
        found.sort();
        let mut want: Vec<String> = KEYS.iter().map(|(k, _)| k.to_string()).collect();
        want.sort();
        assert_eq!(found, want, "one of each key across the valleys");
    }
}
