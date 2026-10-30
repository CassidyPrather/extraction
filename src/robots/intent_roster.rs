//! Every machine says what it's about toward you (playtest, 28 October,
//! F3: "what is this machine doing to me?"). Spawned in front of you, each
//! one says it's seen you (`search::MindShift`: noticed, fighting or
//! calling the others) within seconds, as `robot_voice` voices it. Every
//! `spawn_*` in `robots/` is on [`ROSTER`]: tried here, or excused with a
//! reason, so the next machine can't come in mute.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use super::search::{Mind, MindShift};
use super::{Job, JobKind};
use crate::rig::{LocalPlayer, RigInput};
use crate::world::Zone;

/// How each machine is tried: `None`, excused (and why).
const ROSTER: &[(&str, Option<&str>)] = &[
    ("walker", None),
    ("drone", None),
    ("grappler", None),
    ("hound", None),
    ("heavy", None),
    ("minelayer", None),
    ("sentry", None),
    ("spotter", None),
    ("dirigible", None),
    (
        "walker_with",
        Some("a walker (unarmed, or the collector): walker_brain, tried as the walker"),
    ),
    (
        "walker_sized",
        Some("a walker at another size: walker_brain, tried as the walker"),
    ),
    (
        "crawler",
        Some("indoors only, on walker_brain (tried as the walker): its zap's windup is its tell"),
    ),
    (
        "warden",
        Some("indoors only, on walker_brain (tried as the walker)"),
    ),
    (
        "mender",
        Some("never acts on you: it mends machines; its torch is its tell (mender docs)"),
    ),
    (
        "plough",
        Some("never looks for you: it drives its lane, beacon turning and auger roaring"),
    ),
    ("pylon", Some("a relay mast: no mind, nothing toward you")),
    (
        "sorter",
        Some("the Depot's gantry: its own windup tell and alarm (sorter_mind)"),
    ),
    ("debris", Some("a wreck's pieces, not a machine")),
];

/// Within this, it's said it's seen you (s).
const SAYS_WITHIN: f32 = 12.0;
const HZ: f32 = 90.0;

#[derive(Resource, Default)]
struct Said(Vec<(Entity, Mind)>);

fn said(mut m: MessageReader<MindShift>, mut s: ResMut<Said>) {
    s.0.extend(m.read().map(|m| (m.robot, m.state)));
}

/// Every `spawn_*` in `robots/` is on the roster.
#[test]
fn every_machine_is_on_the_intent_roster() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/robots");
    let mut found = vec![];
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let text = std::fs::read_to_string(e.path()).unwrap();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("pub fn spawn_") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                found.push(name);
            }
        }
    }
    found.sort();
    found.dedup();
    let missing: Vec<&String> = found
        .iter()
        .filter(|f| !ROSTER.iter().any(|(r, _)| r == f))
        .collect();
    assert!(
        missing.is_empty(),
        "machines not on intent_roster::ROSTER (try them here, or excuse them with a \
         reason): {missing:?}"
    );
}

fn app() -> (App, Entity, Zone) {
    let mut app = crate::headless_app(
        crate::LaunchOptions {
            robots: false,
            ..Default::default()
        },
        false,
    );
    app.init_resource::<Said>().add_systems(Last, said);
    app.finish();
    app.cleanup();
    for _ in 0..3 {
        app.update();
    }
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<LocalPlayer>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut()
        .get_mut::<RigInput>(player)
        .unwrap()
        .head
        .translation
        .y = 1.65;
    let zone = app.world().resource::<Zone>().clone();
    app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
    (app, player, zone)
}

/// Spawns the machine `name` near `me`, in front of it and facing it.
fn spawn(app: &mut App, zone: &Zone, name: &str, me: Vec3) -> Entity {
    let ahead = zone.on_ground(me.x, me.z + 14.0);
    let job = Job {
        kind: JobKind::Watch,
        site: ahead,
        duration: 999.0,
        exit: zone.on_ground(me.x, me.z + 200.0),
    };
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let mut queue = CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, app.world());
        match name {
            "walker" => super::walker::spawn_walker_with(&mut c, zone, ahead, job, false, 130.0),
            "drone" => super::drone::spawn_drone(&mut c, ahead + Vec3::Y * 8.0, job),
            "grappler" => super::grappler::spawn_grappler(&mut c, zone, ahead, job),
            "hound" => super::hound::spawn_hound(&mut c, zone, ahead, job),
            "heavy" => super::heavy::spawn_heavy(&mut c, zone, &catalog, ahead, job),
            "minelayer" => {
                super::minelayer::spawn_minelayer(&mut c, zone, &nav, ahead, vec![me], job.exit)
            }
            "sentry" => {
                // Its sweep centred on you.
                let to = me - ahead;
                let centre = to.x.atan2(-to.z).to_degrees();
                super::sentry::spawn_sentry(
                    &mut c,
                    zone,
                    ahead.xz(),
                    centre,
                    60.0,
                    ahead.xz() + Vec2::X * 3.0,
                )
            }
            // A watcher from afar, up on its tether: not overhead.
            "spotter" => super::spotter::spawn_spotter(&mut c, zone, Vec2::new(me.x, me.z + 50.0)),
            "dirigible" => super::dirigible::spawn_dirigible(
                &mut c,
                zone.on_ground(me.x, me.z + 60.0) + Vec3::Y * 40.0,
                Job {
                    site: zone.on_ground(me.x, me.z + 60.0),
                    ..job
                },
            ),
            other => panic!("no way to try {other}"),
        }
    };
    queue.apply(app.world_mut());
    e
}

/// Each machine on the roster that's tried: in front of you, in the open,
/// by day, it says it's seen you within [`SAYS_WITHIN`].
#[test]
fn every_machine_says_it_has_seen_you() {
    let mut mute = vec![];
    for (name, excused) in ROSTER {
        if excused.is_some() {
            continue;
        }
        let (mut app, player, zone) = app();
        let me = zone.on_ground(40.0, 255.0);
        let robot = spawn(&mut app, &zone, name, me);
        let mut heard = None;
        // The spotter's balloon rises to its watch first.
        let rise = if *name == "spotter" { 30.0 } else { 0.0 };
        for i in 0..((SAYS_WITHIN + rise) * HZ) as usize {
            // Standing, shifting about: plain to see.
            let sway = Vec3::X * ((i as f32 / HZ).sin() * 0.5);
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = me + sway + Vec3::Y * 0.05;
            if let Some(mut b) = app.world_mut().get_mut::<crate::player::Body>(player) {
                b.velocity.x = (i as f32 / HZ).cos() * 0.5;
            }
            app.update();
            heard = app
                .world()
                .resource::<Said>()
                .0
                .iter()
                .find(|(r, m)| {
                    *r == robot && matches!(m, Mind::Noticed | Mind::Engaging | Mind::Calling)
                })
                .map(|(_, m)| *m);
            if heard.is_some() {
                break;
            }
        }
        if heard.is_none() {
            let all: Vec<Mind> = app
                .world()
                .resource::<Said>()
                .0
                .iter()
                .filter(|(r, _)| *r == robot)
                .map(|(_, m)| *m)
                .collect();
            mute.push(format!("{name} (said {all:?})"));
        }
    }
    assert!(
        mute.is_empty(),
        "machines that didn't say they'd seen you within {SAYS_WITHIN} s: {mute:?}"
    );
}
