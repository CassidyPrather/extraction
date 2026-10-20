//! `--scenario safehouse` (run by name, not in `all`: it's a long walk):
//! a bot kits up, walks to Marrow Creek's safehouse, claims it (in, door
//! shut, a wedge at its foot, the stove lit with matches), lies down loud,
//! and wakes to a visit: it unbars the door and sees the walker off from
//! the doorway. Everything through the bot's hands and the same presses a
//! player makes.

use bevy::prelude::*;

use super::{Safehouse, SafehouseThing};
use crate::bot::{Bot, Expect, Task};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, Side};
use crate::weapons::flat::MacroKind;

pub const SCENARIO: &str = "safehouse";

/// The last task while the scenario runs: the driver adds each step's.
const IDLE: f32 = 4.5;

pub fn opening() -> Vec<Task> {
    let s = |x: &str| x.to_string();
    vec![
        Task::Log(s(
            "safehouse: claim the cabin, sleep loud, see off the visit",
        )),
        Task::PickUp {
            id: s("ak74"),
            side: Side::Right,
        },
        Task::PickUp {
            id: s("ak_mag"),
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::PickUp {
            id: s("ak_mag"),
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::Gun(MacroKind::Reload),
        Task::Gun(MacroKind::Mode),
        Task::Gun(MacroKind::Mode),
        Task::Wait(IDLE),
    ]
}

#[derive(Default)]
pub struct Steps {
    step: u8,
}

#[allow(clippy::too_many_arguments)]
pub fn direct(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    house: Res<Safehouse>,
    mut bots: Query<(Entity, &mut Bot), With<LocalPlayer>>,
    places: Query<&GlobalTransform>,
    leaves: Query<&crate::world::buildings::DoorLeaf>,
    things: Query<(Entity, &SafehouseThing)>,
    catalog: Res<Catalog>,
    zone: Res<crate::world::Zone>,
    mut attention: Query<&mut crate::anomaly::Attention>,
    raid: Res<crate::raid::Raid>,
    mut steps: Local<Steps>,
) {
    if opts.scenario.as_deref() != Some(SCENARIO) {
        return;
    }
    let Some((player, mut bot)) = bots.iter_mut().next() else {
        return;
    };
    let idle =
        bot.tasks.len() == 1 && matches!(bot.tasks.front(), Some(Task::Wait(s)) if *s == IDLE);
    if !idle || bot.elapsed < 0.6 {
        return;
    }
    let (Some(cabin), Some(door)) = (house.cabin, house.door) else {
        return;
    };
    let (Ok(c), Ok(d), Ok(leaf)) = (places.get(cabin), places.get(door), leaves.get(door)) else {
        return;
    };
    let centre = c.translation();
    // The door's middle at the floor, and the ways in and out of it.
    let mid = d.transform_point(Vec3::X * leaf.0.width * 0.5);
    // Straight through the doorway: the wall's normal, the way in.
    let normal = (d.rotation() * Vec3::Z).with_y(0.0).normalize_or_zero();
    let inward = if normal.dot(centre - mid) > 0.0 {
        normal
    } else {
        -normal
    };
    let thing = |which| things.iter().find(|(_, t)| **t == which).map(|(e, _)| e);
    let push = |bot: &mut Bot, tasks: Vec<Task>| {
        let wait = bot.tasks.pop_back();
        bot.tasks.extend(tasks);
        bot.tasks.extend(wait);
    };
    match steps.step {
        0 => {
            // On the table inside: matches and a door wedge.
            for (id, off) in [("matches", -0.3), ("door_wedge", 0.3)] {
                let at = c.transform_point(Vec3::new(off, 1.25, 0.5));
                if let Some(kind) = catalog.kind(id) {
                    spawn_item(
                        &mut commands,
                        &catalog,
                        SpawnItem {
                            kind,
                            transform: Transform::from_translation(at),
                            condition: 1.0,
                        },
                    );
                }
            }
            let out = mid - inward * 1.3;
            let inside = mid + inward * 1.8;
            push(&mut bot, vec![Task::Log("to the cabin".into())]);
            // In legs a walk task can do, to its front (straight from the
            // road walks into its back wall).
            let front = (mid - inward * 7.0).xz();
            let from = places.get(player).map_or(front, |p| p.translation().xz());
            let legs = (from.distance(front) / 30.0).ceil().max(1.0) as usize;
            push(
                &mut bot,
                (1..=legs)
                    .map(|i| Task::WalkTo {
                        to: from.lerp(front, i as f32 / legs as f32),
                        within: 1.5,
                    })
                    .collect(),
            );
            push(
                &mut bot,
                vec![
                    Task::WalkTo {
                        to: (mid - inward * 7.0).xz(),
                        within: 1.0,
                    },
                    Task::WalkTo {
                        to: out.xz(),
                        within: 0.4,
                    },
                    Task::OpenDoor(door),
                    Task::WalkTo {
                        to: inside.xz(),
                        within: 0.6,
                    },
                    Task::PickUp {
                        id: "matches".into(),
                        side: Side::Left,
                    },
                    Task::Stow(Side::Left),
                    Task::PickUp {
                        id: "door_wedge".into(),
                        side: Side::Left,
                    },
                    // Shut it, then the wedge down at its foot.
                    Task::Press(door),
                    Task::Wait(1.5),
                    Task::WalkTo {
                        to: (mid + inward * 0.75).xz(),
                        within: 0.25,
                    },
                    Task::LookAt(mid),
                    Task::Release(Side::Left),
                    Task::Wait(2.0),
                ],
            );
            steps.step = 1;
        }
        1 => {
            if !house.state.barred {
                bot.failed += 1;
                bot.log.push("FAIL the wedge didn't bar the door".into());
            }
            if let Some(stove) = thing(SafehouseThing::Stove) {
                // In front of it, round the table's front (the table's in
                // the middle; the door's on the left, the stove at the back
                // right, in cabin space).
                let round = c.transform_point(Vec3::new(1.6, 0.0, -1.2));
                let by = c.transform_point(Vec3::new(1.6, 0.0, 1.3));
                push(
                    &mut bot,
                    vec![
                        Task::WalkTo {
                            to: round.xz(),
                            within: 0.4,
                        },
                        Task::WalkTo {
                            to: by.xz(),
                            within: 0.4,
                        },
                        Task::Press(stove),
                        Task::Wait(1.0),
                    ],
                );
            }
            steps.step = 2;
        }
        2 => {
            if house.state.claimed() {
                bot.passed += 1;
                bot.log.push("PASS claimed: cleared, barred, lit".into());
            } else {
                bot.failed += 1;
                bot.log.push(format!("FAIL not claimed: {:?}", house.state));
            }
            // A loud night: it's been shooting.
            if let Ok(mut a) = attention.get_mut(player) {
                a.noise = 2.0;
            }
            if let Some(bed) = thing(SafehouseThing::Bed) {
                // Round the table's back to the bed's side.
                let round = c.transform_point(Vec3::new(0.0, 0.0, 1.9));
                let by = c.transform_point(Vec3::new(-1.7, 0.0, 1.3));
                push(
                    &mut bot,
                    vec![
                        Task::WalkTo {
                            to: round.xz(),
                            within: 0.4,
                        },
                        Task::WalkTo {
                            to: by.xz(),
                            within: 0.4,
                        },
                        Task::Press(bed),
                        Task::Wait(1.0),
                    ],
                );
            }
            steps.step = 3;
        }
        3 => {
            if house.nights_out == 1
                && raid
                    .log
                    .iter()
                    .any(|l| l.0 == super::lines::SAFE_NIGHT_VISIT)
            {
                bot.passed += 1;
                bot.log
                    .push("PASS slept, and something came to the door".into());
            } else {
                bot.failed += 1;
                bot.log
                    .push(format!("FAIL the night: {} nights out", house.nights_out));
            }
            // The visit: unbar the door and meet it in the doorway (robots
            // may be off in a scenario, so it's put there, already on the
            // bot's trail, square in the doorway's sights).
            let path = [(0.0, 1.9), (1.6, 1.3), (1.6, -1.2)]
                .map(|(x, z)| c.transform_point(Vec3::new(x, 0.0, z)));
            let mut tasks: Vec<Task> = path
                .iter()
                .map(|p| Task::WalkTo {
                    to: p.xz(),
                    within: 0.4,
                })
                .collect();
            let at = mid - inward * 9.0;
            tasks.extend([
                Task::WalkTo {
                    to: (mid + inward * 0.75).xz(),
                    within: 0.3,
                },
                Task::PickUp {
                    id: "door_wedge".into(),
                    side: Side::Left,
                },
                Task::Stow(Side::Left),
                Task::WalkTo {
                    to: (mid + inward * 1.8).xz(),
                    within: 0.4,
                },
                Task::OpenDoor(door),
                Task::WalkTo {
                    to: (mid + inward * 2.4).xz(),
                    within: 0.4,
                },
                Task::Spawn {
                    kind: crate::robots::RobotKind::Walker,
                    at: Vec3::new(at.x, zone.ground(at.x, at.z), at.z),
                },
                Task::LookAt(mid - inward * 4.0),
                Task::Wait(2.0),
                Task::Engage { shots: 30 },
                Task::Wait(4.0),
                Task::Expect(Expect::Wrecked(1)),
                Task::Expect(Expect::Alive),
            ]);
            push(&mut bot, tasks);
            steps.step = 4;
        }
        _ => {
            // Done: let the bot finish.
            if bot.tasks.len() == 1 {
                bot.tasks.clear();
            }
        }
    }
}
