//! Scripted bot runs that double as integration tests.
//!
//! `extraction --scenario pistol_drill [--headless]` runs one; `all` runs
//! them one after another in the same world. The process exits non-zero if
//! any expectation failed, so CI can run them.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use super::{Bot, Expect, Task};
use crate::launch::LaunchOptions;
use crate::progression::contracts::scenario::SCENARIOS as CONTRACTS;
use crate::rig::{LocalPlayer, Side};
use crate::weapons::flat::MacroKind;
use crate::world::{Layer, Zone};

/// Stage a scenario's world (`Task::Setup`): the module under test reads
/// it and builds what the scenario needs round `at`, the bot's feet.
#[derive(Message, Clone, Copy, Debug)]
pub struct Setup {
    pub name: &'static str,
    pub at: Vec3,
    pub facing: Vec3,
}

/// A steel plate on a post, for shooting at. Counts its hits.
#[derive(Component, Default)]
pub struct Target {
    pub hits: u32,
}

pub const NAMES: &[&str] = &[
    "pistol_drill",
    "rifle_drill",
    "bolt_drill",
    "carbine_drill",
    "revolver_drill",
    "smg_drill",
    "shotgun_drill",
    "pump_drill",
    "marksman_drill",
    "smallbore_drill",
    "lmg_drill",
    "flare_drill",
    "crossbow_drill",
    "pack_mule",
    "drone_duel",
    "walker_hunt",
    crate::anomaly::scenarios::NAMES[0],
    crate::anomaly::scenarios::NAMES[1],
    crate::anomaly::scenarios::NAMES[2],
    crate::anomaly::scenarios::NAMES[3],
    crate::anomaly::scenarios::NAMES[4],
    crate::anomaly::scenarios::NAMES[5],
    crate::anomaly::scenarios::NAMES[6],
    crate::onboarding::bot::SCENARIO,
    crate::robot_voice::SCENARIO,
    crate::robots::heavy_mind::SCENARIO,
    crate::robots::sentry::SCENARIO,
    crate::robots::spotter::SCENARIO,
    crate::robots::minelayer::SCENARIO,
    crate::robots::mender::SCENARIO,
    crate::robots::network::SCENARIO,
    crate::robots::devices::SCENARIOS[0].0,
    crate::robots::devices::SCENARIOS[1].0,
    crate::robots::devices::SCENARIOS[2].0,
    crate::robots::devices::SCENARIOS[3].0,
    crate::robots::devices::SCENARIOS[4].0,
    crate::robots::plough::SCENARIO,
    crate::world::cage::SCENARIO,
    crate::world::echo::SCENARIO,
    crate::world::lockdown::SCENARIO,
    crate::world::live::SCENARIO,
];

/// Longer runs, not in `all`: run them by name.
/// - `second_hour`: a short career past the yard (`onboarding::second_hour`).
/// - `safehouse`: claim Marrow Creek's cabin, sleep, see off the visit
///   (`progression::safehouse::scenario`).
/// - `door_shot`: a walker shot through a shut plank door
///   (`destruction::scenario`).
/// - `crossing`: a raid's crossing to the truck, Sallow Reservoir's by
///   default (`bot::crossing`).
/// - `contract_<id>`: one of the driver's contracts from the card to its
///   pay, on its valley's map (`progression::contracts::scenario`).
/// - `first_hour`, `first_hour_<persona>`: newcomers from the yard to the
///   end of a first raid (`onboarding::first_hour`); each runs its two
///   halves, `first_yard` and `first_raid`, as two sessions.
pub const OPT_IN: &[&str] = &[
    crate::onboarding::second_hour::SCENARIO,
    crate::progression::safehouse::scenario::SCENARIO,
    crate::destruction::scenario::SCENARIO,
    super::crossing::SCENARIO,
    CONTRACTS[0].0,
    CONTRACTS[1].0,
    CONTRACTS[2].0,
    CONTRACTS[3].0,
    CONTRACTS[4].0,
    CONTRACTS[5].0,
    CONTRACTS[6].0,
    CONTRACTS[7].0,
    crate::onboarding::first_hour::SCENARIO,
    crate::onboarding::first_hour::PERSONAS[0].scenario,
    crate::onboarding::first_hour::PERSONAS[1].scenario,
    crate::onboarding::first_hour::PERSONAS[2].scenario,
    crate::onboarding::first_hour::PERSONAS[3].scenario,
    crate::onboarding::first_hour::PERSONAS[4].scenario,
];

/// Targets sit north of the insertion point at these distances.
const TARGET_DISTANCES: [f32; 3] = [10.0, 25.0, 50.0];

fn origin(zone: &Zone) -> Vec2 {
    let m = zone
        .marker(&MarkerKind::Insertion)
        .map(|m| m.at)
        .unwrap_or([0.0, 0.0]);
    Vec2::new(m[0], m[1])
}

pub fn target_point(zone: &Zone, i: usize) -> Vec3 {
    let o = origin(zone);
    let x = o.x + 6.0 + i as f32 * 1.5;
    let z = o.y - TARGET_DISTANCES[i];
    Vec3::new(x, zone.ground(x, z) + 1.3, z)
}

pub fn tasks(name: &str, zone: &Zone) -> Option<Vec<Task>> {
    let t0 = target_point(zone, 0);
    let t1 = target_point(zone, 1);
    use Task::*;
    let s = |x: &str| x.to_string();
    // A device set down running by a machine: noticed, and shot if its row
    // says so (`robots::devices`).
    if let Some(&(_, id, _, shot)) = crate::robots::devices::SCENARIOS
        .iter()
        .find(|x| x.0 == name)
    {
        use crate::bot::Expect::Counter;
        use crate::robots::devices as dv;
        let mut t = vec![
            Log(format!("device: a running {id} set down by a machine")),
            Setup(
                crate::robots::devices::SCENARIOS
                    .iter()
                    .find(|x| x.0 == name)
                    .unwrap()
                    .0,
            ),
            // (A task may take 30 s at most: two waits.)
            Wait(15.0),
            Wait(15.0),
            Expect(Counter(dv::NOTICED, 1, 100)),
        ];
        if shot {
            t.push(Expect(Counter(dv::WRECKED, 1, 100)));
        }
        return Some(t);
    }
    Some(match name {
        "pistol_drill" => vec![
            Log(s("pistol drill: load two magazines, shoot one dry, reload")),
            PickUp {
                id: s("pm_pistol"),
                side: Side::Right,
            },
            PickUp {
                id: s("pm_mag"),
                side: Side::Left,
            },
            Stow(Side::Left),
            PickUp {
                id: s("pm_mag"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Expect(super::Expect::Holding(Side::Right, s("pm_pistol"))),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(8, 8)),
            Gun(MacroKind::Mode),
            Fire { at: t0, shots: 8 },
            Expect(super::Expect::GunRounds(0, 0)),
            Expect(super::Expect::Hits(5)),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(8, 8)),
            Stow(Side::Right),
            Expect(super::Expect::CarryingAtLeast(0.9)),
        ],
        "rifle_drill" => vec![
            Log(s("rifle drill: two-handed AK, full auto burst")),
            PickUp {
                id: s("ak74"),
                side: Side::Right,
            },
            PickUp {
                id: s("ak_mag"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(30, 30)),
            Support,
            Gun(MacroKind::Mode),
            Fire { at: t1, shots: 10 },
            Expect(super::Expect::GunRounds(17, 23)),
            Expect(super::Expect::Hits(4)),
            Release(Side::Left),
            Stow(Side::Right),
        ],
        "bolt_drill" => vec![
            Log(s(
                "bolt drill: thumb five rounds into a Mosin, work the bolt",
            )),
            PickUp {
                id: s("mosin"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_762r"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(5, 5)),
            Fire { at: t1, shots: 1 },
            Gun(MacroKind::Rack),
            Fire { at: t1, shots: 1 },
            Expect(super::Expect::GunRounds(3, 3)),
            Expect(super::Expect::Hits(1)),
            Stow(Side::Right),
        ],
        "carbine_drill" => vec![
            Log(s(
                "carbine drill: load an SKS's fixed magazine, shoot it dry, it locks back",
            )),
            PickUp {
                id: s("sks"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_762x39"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(10, 10)),
            Gun(MacroKind::Mode),
            Fire { at: t1, shots: 10 },
            Expect(super::Expect::GunRounds(0, 0)),
            Expect(super::Expect::Hits(5)),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(10, 10)),
            Stow(Side::Right),
        ],
        "revolver_drill" => vec![
            Log(s(
                "revolver drill: load a Nagant through the gate, shoot it dry double-action, reload",
            )),
            PickUp {
                id: s("nagant"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_762x38r"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(7, 7)),
            Fire { at: t0, shots: 7 },
            Expect(super::Expect::GunRounds(0, 0)),
            Expect(super::Expect::Hits(4)),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(7, 7)),
            Stow(Side::Right),
        ],
        "smg_drill" => vec![
            Log(s(
                "smg drill: seat the drum, rack to the sear, a burst on auto",
            )),
            PickUp {
                id: s("ppsh"),
                side: Side::Right,
            },
            PickUp {
                id: s("ppsh_drum"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(71, 71)),
            Support,
            Gun(MacroKind::Mode),
            Fire { at: t0, shots: 10 },
            Expect(super::Expect::GunRounds(40, 66)),
            Expect(super::Expect::Hits(4)),
            Release(Side::Left),
            Stow(Side::Right),
        ],
        "shotgun_drill" => vec![
            Log(s("shotgun drill: break, load two, fire two")),
            PickUp {
                id: s("toz_double"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_12ga"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(2, 2)),
            Fire { at: t0, shots: 2 },
            Expect(super::Expect::GunRounds(0, 0)),
            Expect(super::Expect::Hits(6)),
        ],
        "pump_drill" => vec![
            Log(s("pump drill: thumb the tube full, pump, fire, pump, fire")),
            PickUp {
                id: s("mp133"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_12ga"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(5, 5)),
            Gun(MacroKind::Mode),
            Fire { at: t0, shots: 1 },
            Gun(MacroKind::Rack),
            Fire { at: t0, shots: 1 },
            Expect(super::Expect::GunRounds(3, 3)),
            Expect(super::Expect::Hits(6)),
            Stow(Side::Right),
        ],
        "marksman_drill" => vec![
            Log(s(
                "marksman drill: an SVD's ten-round box, five aimed shots at 25 m",
            )),
            PickUp {
                id: s("svd"),
                side: Side::Right,
            },
            PickUp {
                id: s("svd_mag"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(10, 10)),
            Support,
            Gun(MacroKind::Mode),
            Fire { at: t1, shots: 5 },
            Expect(super::Expect::GunRounds(5, 5)),
            Expect(super::Expect::Hits(4)),
            Release(Side::Left),
            Stow(Side::Right),
        ],
        "smallbore_drill" => vec![
            Log(s(
                "smallbore drill: a .22 into the TOZ-8's chamber, fire, again",
            )),
            PickUp {
                id: s("toz8"),
                side: Side::Right,
            },
            PickUp {
                id: s("ammo_22lr"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(1, 1)),
            Fire { at: t0, shots: 1 },
            Gun(MacroKind::Reload),
            Fire { at: t0, shots: 1 },
            Expect(super::Expect::GunRounds(0, 0)),
            Expect(super::Expect::Hits(1)),
            Stow(Side::Right),
        ],
        "lmg_drill" => vec![
            Log(s("lmg drill: the RPK's forty-five, a burst on auto")),
            PickUp {
                id: s("rpk74"),
                side: Side::Right,
            },
            PickUp {
                id: s("rpk_mag"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(45, 45)),
            // The legs down (bots fire it off them as they would a rest).
            Gun(MacroKind::Bipod),
            Support,
            Gun(MacroKind::Mode),
            Fire { at: t1, shots: 10 },
            Expect(super::Expect::GunRounds(20, 38)),
            Expect(super::Expect::Hits(4)),
            Release(Side::Left),
            Stow(Side::Right),
        ],
        "flare_drill" => vec![
            Log(s(
                "flare drill: break the signal pistol, a white star in, fire it",
            )),
            PickUp {
                id: s("flare_pistol"),
                side: Side::Right,
            },
            PickUp {
                id: s("flare_white"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(1, 1)),
            Fire { at: t1, shots: 1 },
            Expect(super::Expect::GunRounds(0, 0)),
            Stow(Side::Right),
        ],
        "crossbow_drill" => vec![
            Log(s(
                "crossbow drill: draw to the latch, a bolt in the groove, loose; again",
            )),
            PickUp {
                id: s("crossbow"),
                side: Side::Right,
            },
            PickUp {
                id: s("bolt_bundle"),
                side: Side::Left,
            },
            Stow(Side::Left),
            Gun(MacroKind::Reload),
            Expect(super::Expect::GunRounds(1, 1)),
            Gun(MacroKind::Mode),
            Fire { at: t0, shots: 1 },
            Gun(MacroKind::Reload),
            Fire { at: t0, shots: 1 },
            Expect(super::Expect::GunRounds(0, 0)),
            Stow(Side::Right),
        ],
        "pack_mule" => vec![
            Log(s("pack mule: stow everything that fits")),
            PickUp {
                id: s("canned_stew"),
                side: Side::Right,
            },
            Stow(Side::Right),
            PickUp {
                id: s("cleaning_kit"),
                side: Side::Right,
            },
            Stow(Side::Right),
            PickUp {
                id: s("flashlight"),
                side: Side::Right,
            },
            Stow(Side::Right),
            // The kit is Medium: no slot takes it, so it goes in the pack.
            Expect(super::Expect::PackItems(1, 4)),
            Expect(super::Expect::CarryingAtLeast(2.0)),
            // A featherstone in the pack carries itself and then some.
            PickUp {
                id: s("art_featherstone"),
                side: Side::Right,
            },
            Stow(Side::Right),
            Expect(super::Expect::CarryingAtMost(0.5)),
        ],
        "drone_duel" => {
            let o = origin(zone);
            vec![
                Log(s(
                    "drone duel: an AK against a drone that's already seen you",
                )),
                PickUp {
                    id: s("ak74"),
                    side: Side::Right,
                },
                PickUp {
                    id: s("ak_mag"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                PickUp {
                    id: s("ak_mag"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                Gun(MacroKind::Mode),
                Gun(MacroKind::Mode),
                Support,
                Spawn {
                    kind: crate::robots::RobotKind::Drone,
                    at: Vec3::new(o.x, zone.ground(o.x, o.y - 40.0) + 12.0, o.y - 40.0),
                },
                Wait(3.0),
                Engage { shots: 29 },
                Gun(MacroKind::Reload),
                Engage { shots: 30 },
                Wait(4.0),
                Expect(super::Expect::Wrecked(1)),
            ]
        }
        "walker_hunt" => {
            let o = origin(zone);
            vec![
                Log(s(
                    "walker hunt: a Mosin against a walker coming up the road",
                )),
                PickUp {
                    id: s("mosin"),
                    side: Side::Right,
                },
                PickUp {
                    id: s("ammo_762r"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                Spawn {
                    kind: crate::robots::RobotKind::Walker,
                    at: Vec3::new(o.x, 0.0, o.y - 30.0),
                },
                Wait(2.0),
                Engage { shots: 1 },
                Gun(MacroKind::Rack),
                Engage { shots: 1 },
                Gun(MacroKind::Rack),
                Engage { shots: 1 },
                Gun(MacroKind::Rack),
                Engage { shots: 1 },
                Gun(MacroKind::Rack),
                Engage { shots: 1 },
                Wait(5.0),
                Expect(super::Expect::Wrecked(1)),
            ]
        }
        n if crate::anomaly::scenarios::NAMES.contains(&n) => {
            crate::anomaly::scenarios::tasks(n, zone)?
        }
        crate::robot_voice::SCENARIO => {
            // Standing by the truck while the machines come: a far drone
            // answers as it sets out, a walker and a hound come up the
            // road and find you. What would you hear?
            use crate::robots::RobotKind;
            let o = origin(zone);
            let at = |dx: f32, dz: f32, up: f32| {
                Vec3::new(o.x + dx, zone.ground(o.x + dx, o.y + dz) + up, o.y + dz)
            };
            vec![
                Log(s("listening: by the truck, while the machines come")),
                Spawn {
                    kind: RobotKind::Drone,
                    at: at(40.0, -180.0, 15.0),
                },
                Spawn {
                    kind: RobotKind::Walker,
                    at: at(-10.0, -45.0, 0.0),
                },
                Wait(25.0),
                Expect(super::Expect::Heard(
                    Some(crate::robot_voice::Utterance::Answering),
                    2,
                )),
                Expect(super::Expect::Heard(None, 4)),
            ]
        }
        crate::robots::heavy_mind::SCENARIO => vec![
            Log(s(
                "heavy well: a heavy on your trail, and a well between you and it",
            )),
            Setup(crate::robots::heavy_mind::SCENARIO),
            // It comes on; the well takes its feet and its heart the knees.
            Wait(25.0),
            Wait(25.0),
            Wait(25.0),
            Expect(super::Expect::Wrecked(1)),
            Expect(super::Expect::Alive),
        ],
        crate::robots::network::SCENARIO => {
            use crate::bot::Expect::Counter;
            use crate::robots::network as net;
            vec![
                Log(s("relay cut: cut a pylon's cable, then stand in its view")),
                Setup(net::SCENARIO),
                Wait(1.0),
                PickUp {
                    id: s("bolt_cutters"),
                    side: Side::Right,
                },
                Cut { within: 25.0 },
                Wait(1.0),
                WalkTo {
                    to: net::VIEW_PLACEHOLDER,
                    within: 2.0,
                },
                Wait(15.0),
                Expect(Counter(net::SILENCED, 1, 1)),
                Expect(Counter(net::BLOCKED, 1, 1000)),
            ]
        }
        crate::robots::sentry::SCENARIO => {
            vec![
                Log(s(
                    "sentry cable: get behind a sentry, cut its cable, walk through its arc",
                )),
                Setup(crate::robots::sentry::SCENARIO),
                Wait(1.0),
                PickUp {
                    id: s("bolt_cutters"),
                    side: Side::Right,
                },
                Cut { within: 25.0 },
                Wait(1.0),
                // (The setup fills in where: past the sentry, through its arc.)
                WalkTo {
                    to: crate::robots::sentry::PAST_PLACEHOLDER,
                    within: 2.0,
                },
                Wait(2.0),
                Expect(super::Expect::HealthAtLeast(99.0)),
                Expect(super::Expect::Alive),
            ]
        }
        crate::world::live::SCENARIO => {
            // The service tunnel: the vault's breaker in (the overflow's
            // water live), the switch room's isolator pulled, and into the
            // overflow chamber unhurt.
            use crate::bot::Expect::{Counter, HealthAtLeast};
            use crate::world::live as lv;
            vec![
                Log(s(
                    "live water: the breaker, the isolator, then the overflow chamber",
                )),
                Setup(lv::SCENARIO),
                Wait(1.0),
                Press(Entity::PLACEHOLDER),
                Wait(1.0),
                WalkTo {
                    to: lv::TO_ISOLATOR,
                    within: 0.8,
                },
                Press(Entity::PLACEHOLDER),
                Wait(1.0),
                WalkTo {
                    to: lv::INTO_IT,
                    within: 1.0,
                },
                Wait(2.0),
                Expect(Counter("live_isolated", 1, 1)),
                Expect(Counter("live_jolts", 0, 0)),
                Expect(HealthAtLeast(99.0)),
            ]
        }
        crate::world::lockdown::SCENARIO => {
            // The shelter: the generator started, the klaxon, the gas lock
            // sealed; out by the escape tunnel.
            use crate::bot::Expect::{Counter, HealthAtLeast, Near};
            use crate::world::lockdown as lk;
            let wait = extraction_core::underground::layout("bunker_marrow")
                .and_then(|l| l.lockdown.as_ref())
                .map_or(20.0, |k| k.after)
                + 2.0;
            vec![
                Log(s(
                    "shelter lockdown: start the generator, then out the escape tunnel",
                )),
                Setup(lk::SCENARIO),
                Wait(1.0),
                Press(Entity::PLACEHOLDER),
                Wait(wait),
                Expect(Counter("shelter_sealed", 1, 1)),
                WalkTo {
                    to: lk::OUT,
                    within: 1.0,
                },
                Expect(Near(lk::OUT, 1.5)),
                Expect(HealthAtLeast(50.0)),
            ]
        }
        crate::world::echo::SCENARIO => {
            // The dam's gallery: its length on foot, upright; every step
            // comes back.
            use crate::bot::Expect::{Counter, Near};
            use crate::world::echo as ec;
            vec![
                Log(s("gallery echo: the inspection gallery's length on foot")),
                Setup(ec::SCENARIO),
                Wait(1.0),
                WalkTo {
                    to: ec::FAR_END,
                    within: 1.0,
                },
                Expect(Near(ec::FAR_END, 2.0)),
                Expect(Counter("gallery_echoes", 10, 1000)),
            ]
        }
        crate::world::cage::SCENARIO => {
            // The adit's winze: into the cage at the shaft station and rung
            // down; at the foot, sent up and called back; ridden up and out.
            use crate::bot::Expect::{Counter, HealthAtLeast, Near};
            use crate::world::cage as cg;
            let ride = cg::BELL_WAIT + 12.0 / cg::CAGE_SPEED + 3.0;
            vec![
                Log(s("cage ride: down the adit's winze and back up")),
                Setup(cg::SCENARIO),
                Wait(1.0),
                WalkTo {
                    to: cg::IN_CAGE,
                    within: 0.4,
                },
                Press(Entity::PLACEHOLDER),
                Wait(ride),
                WalkTo {
                    to: cg::AT_FOOT,
                    within: 0.8,
                },
                Press(Entity::PLACEHOLDER),
                Wait(ride),
                Press(Entity::PLACEHOLDER),
                Wait(ride),
                WalkTo {
                    to: cg::IN_CAGE,
                    within: 0.4,
                },
                Press(Entity::PLACEHOLDER),
                Wait(ride),
                WalkTo {
                    to: cg::AT_TOP,
                    within: 0.8,
                },
                Expect(Near(cg::AT_TOP, 1.5)),
                Expect(HealthAtLeast(99.0)),
                Expect(Counter("cage_rides", 2, 2)),
                Expect(Counter("cage_crushed", 0, 0)),
            ]
        }
        crate::robots::plough::SCENARIO => {
            // A plough coming along a snowy road. A tin in its way goes in
            // the bank. The bot drops a crowbar in its lane and steps back:
            // it jams, its hatch springs, and the bot shoots the cell.
            use crate::robots::plough as pl;
            vec![
                Log(s("plough jam: a crowbar in its lane, then its cell")),
                Setup(pl::SCENARIO),
                Wait(1.0),
                PickUp {
                    id: s("ak74"),
                    side: Side::Right,
                },
                PickUp {
                    id: s("ak_mag"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                Gun(MacroKind::Mode),
                PickUp {
                    id: s("crowbar"),
                    side: Side::Left,
                },
                WalkTo {
                    to: pl::LAY_PLACEHOLDER,
                    within: 0.6,
                },
                Release(Side::Left),
                WalkTo {
                    to: pl::BACK_PLACEHOLDER,
                    within: 1.0,
                },
                Support,
                // Till it reaches the crowbar and jams.
                Wait(10.0),
                Expect(super::Expect::Counter(pl::JAMS, 1, 9)),
                Shoot {
                    target: Entity::PLACEHOLDER,
                    shots: 25,
                },
                Wait(3.0),
                Expect(super::Expect::RobotHits(1)),
                Expect(super::Expect::Counter(pl::BURIED, 1, 9)),
                Expect(super::Expect::Wrecked(1)),
                Expect(super::Expect::Alive),
            ]
        }
        crate::robots::mender::SCENARIO | crate::robots::mender::CONTROL => {
            // A hurt pylon ahead, a mender coming to weld it. The bot
            // waits for the arc and shoots the mender off it; the control
            // lets it work.
            use crate::robots::mender as md;
            let shoot = name == md::SCENARIO;
            let mut t = vec![
                Log(s(if shoot {
                    "mender: shoot the mender off the machine it's welding"
                } else {
                    "mender control: let it weld"
                })),
                Setup(md::SCENARIO),
                Wait(1.0),
                PickUp {
                    id: s("ak74"),
                    side: Side::Right,
                },
                PickUp {
                    id: s("ak_mag"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                Expect(super::Expect::GunRounds(20, 31)),
                // Off safe.
                Gun(MacroKind::Mode),
                Support,
                // Till it's there and welding.
                Wait(8.0),
            ];
            if shoot {
                t.extend([
                    Shoot {
                        target: Entity::PLACEHOLDER,
                        shots: 30,
                    },
                    Wait(20.0),
                    Expect(super::Expect::RobotHits(1)),
                    Expect(super::Expect::Wrecked(1)),
                    // Shot off before it's done (left to it, it welds 130 or so).
                    Expect(super::Expect::Counter(md::WELDED, 0, 80)),
                ]);
            } else {
                t.extend([
                    Wait(20.0),
                    Expect(super::Expect::Counter(md::WELDED, 100, 100_000)),
                ]);
            }
            t
        }
        crate::robots::minelayer::SCENARIO | crate::robots::minelayer::CONTROL => {
            // A field of mines across the way. The bot walks up to the
            // first, cuts its lead, goes round the end of the rest and on.
            // The control walks straight through.
            use crate::robots::minelayer as ml;
            let careful = name == ml::SCENARIO;
            let mut t = vec![
                Log(s(if careful {
                    "minefield: spot the first mine, cut it, go round the rest"
                } else {
                    "minefield control: straight through"
                })),
                Setup(ml::SCENARIO),
                Wait(1.0),
            ];
            if careful {
                t.extend([
                    PickUp {
                        id: s("bolt_cutters"),
                        side: Side::Right,
                    },
                    WalkTo {
                        to: ml::MINE_PLACEHOLDER,
                        within: 0.6,
                    },
                    Cut { within: 2.0 },
                    Wait(1.0),
                    WalkTo {
                        to: ml::ROUND_PLACEHOLDER,
                        within: 1.0,
                    },
                ]);
            }
            t.extend([
                WalkTo {
                    to: ml::PAST_PLACEHOLDER,
                    within: 1.5,
                },
                Wait(2.0),
            ]);
            if careful {
                t.extend([
                    Expect(super::Expect::Counter(ml::DISARMED, 1, 1)),
                    Expect(super::Expect::Counter(ml::BLOWN, 0, 0)),
                    Expect(super::Expect::HealthAtLeast(99.0)),
                ]);
            } else {
                t.push(Expect(super::Expect::Counter(ml::BLOWN, 1, 9)));
            }
            t
        }
        crate::robots::spotter::SCENARIO | crate::robots::spotter::CONTROL => {
            // Under a spotter's balloon (straight under it is blind), cut
            // its tether and walk out across the ground it watched: no
            // call. The control walks out without the cut, and is called.
            let cut = name == crate::robots::spotter::SCENARIO;
            let mut t = vec![
                Log(s(if cut {
                    "spotter tether: in under a spotter, cut it loose, walk out"
                } else {
                    "spotter control: walk out under a spotter's watch"
                })),
                Setup(crate::robots::spotter::SCENARIO),
                Wait(1.0),
            ];
            if cut {
                t.extend([
                    PickUp {
                        id: s("bolt_cutters"),
                        side: Side::Right,
                    },
                    Cut { within: 20.0 },
                    Wait(3.0),
                ]);
            }
            t.extend([
                WalkTo {
                    to: crate::robots::spotter::PAST_PLACEHOLDER,
                    within: 2.0,
                },
                Wait(12.0),
                Expect(if cut {
                    super::Expect::Counter(crate::robots::spotter::CALLS, 0, 0)
                } else {
                    super::Expect::Counter(crate::robots::spotter::CALLS, 1, 9)
                }),
                Expect(super::Expect::Alive),
            ]);
            t
        }
        crate::onboarding::bot::SCENARIO | crate::onboarding::first_hour::YARD => {
            crate::onboarding::bot::opening()
        }
        crate::onboarding::first_hour::RAID => crate::onboarding::first_hour::opening(),
        crate::onboarding::second_hour::SCENARIO => crate::onboarding::second_hour::opening(),
        crate::destruction::scenario::SCENARIO => crate::destruction::scenario::tasks(zone),
        crate::progression::safehouse::scenario::SCENARIO => {
            crate::progression::safehouse::scenario::opening()
        }
        super::crossing::SCENARIO => super::crossing::opening(),
        n if crate::progression::contracts::scenario::is(n) => {
            crate::progression::contracts::scenario::opening(n)
        }
        _ => return None,
    })
}

pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Setup>()
            .add_systems(Startup, spawn_targets)
            .add_systems(PostStartup, attach_scenario)
            .add_systems(Update, (finish_scenario, count_target_hits));
    }
}

fn spawn_targets(mut commands: Commands, zone: Res<Zone>, opts: Res<LaunchOptions>) {
    // Bare colliders, drawn by nothing: only for the drills, or they stand
    // invisible on every valley's road out of the truck.
    if opts.scenario.is_none() {
        return;
    }
    for i in 0..TARGET_DISTANCES.len() {
        let p = target_point(&zone, i);
        commands.spawn((
            Name::new(format!("Target {}m", TARGET_DISTANCES[i])),
            Target::default(),
            Transform::from_translation(p),
            RigidBody::Static,
            Collider::cuboid(0.45, 0.6, 0.03),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
        let ground = zone.ground(p.x, p.z);
        commands.spawn((
            Name::new("Target post"),
            Transform::from_xyz(p.x, (ground + p.y - 0.3) * 0.5, p.z),
            RigidBody::Static,
            Collider::cuboid(0.06, (p.y - 0.3 - ground).max(0.1), 0.06),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
}

fn count_target_hits(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    mut targets: Query<&mut Target>,
) {
    for hit in hits.read() {
        if let Ok(mut t) = targets.get_mut(hit.entity) {
            t.hits += 1;
        }
    }
}

fn attach_scenario(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    players: Query<Entity, With<LocalPlayer>>,
) {
    let Some(name) = &opts.scenario else {
        return;
    };
    let list: Vec<&str> = if name == "all" {
        NAMES.to_vec()
    } else {
        vec![name.as_str()]
    };
    let mut all = Vec::new();
    for n in list.iter().copied() {
        match tasks(n, &zone) {
            Some(t) => all.extend(t),
            None => {
                error!("unknown scenario `{n}`; known: {NAMES:?}");
                commands.queue(|w: &mut World| {
                    w.write_message(AppExit::error());
                });
                return;
            }
        }
    }
    if list.contains(&crate::robot_voice::SCENARIO) {
        commands.init_resource::<crate::robot_voice::Heard>();
    }
    for p in &players {
        commands.entity(p).insert(Bot::new(all.clone()));
    }
}

fn finish_scenario(
    bots: Query<(&Bot, &Transform), With<LocalPlayer>>,
    opts: Res<LaunchOptions>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
    mut frames: Local<u32>,
    time: Res<Time>,
    robots: Query<(
        &crate::robots::Robot,
        &GlobalTransform,
        &crate::robots::Brain,
        &crate::robots::Awareness,
        &Children,
    )>,
    parts: Query<&crate::robots::RobotPart>,
) {
    if opts.scenario.is_none() || *done {
        return;
    }
    let Some((bot, t)) = bots.iter().next() else {
        return;
    };
    *frames += 1;
    if *frames % 180 == 0 {
        for (r, gt, brain, aware, children) in &robots {
            let hp: Vec<String> = children
                .iter()
                .filter_map(|c| parts.get(c).ok())
                .map(|p| format!("{:?}:{:.0}", p.role, p.hp))
                .collect();
            info!(
                "robot {:?} at {:.1?} wrecked={} brain={:?} sees={} susp={:.2} hp=[{}] bot_hits={}",
                r.kind,
                gt.translation(),
                r.wrecked,
                brain,
                aware.can_see_target,
                aware.suspicion,
                hp.join(" "),
                bot.hits
            );
        }
    }
    if *frames % 900 == 0 {
        info!(
            "scenario heartbeat: frame {} t={:.1}s at {:.1?} task {:?}",
            *frames,
            time.elapsed_secs(),
            t.translation,
            bot.tasks.front()
        );
    }
    if !bot.done() {
        return;
    }
    *done = true;
    let ok = bot.failed == 0;
    println!(
        "scenario {}: {} passed, {} failed -> {}",
        opts.scenario.as_deref().unwrap_or(""),
        bot.passed,
        bot.failed,
        if ok { "OK" } else { "FAILED" }
    );
    for line in &bot.log {
        println!("  {line}");
    }
    // Leave the screenshot tool to quit when it's done, if it's running.
    if opts.screenshot.is_none() {
        exit.write(if ok {
            AppExit::Success
        } else {
            AppExit::error()
        });
    }
    let _ = Expect::Hits(0);
}
