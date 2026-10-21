//! `SIM_OVERNIGHT=1` in a sim run: the raider walks to the safehouse as a
//! job (`sim::raider`), and here, once it's inside, it claims the cabin and
//! sleeps the night through the bed's own press. The wedge and the matches
//! are taken as read (the `safehouse` scenario walks those through); the
//! night, the visit and the morning after are the game's.

use bevy::prelude::*;
use extraction_core::safehouse::NIGHT;

use super::{Safehouse, SafehouseThing};
use crate::interact::InteractEvent;
use crate::launch::LaunchOptions;
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;
use crate::robots::doors::Wedged;
use crate::world::buildings::Door;

/// How close to the cabin's middle counts as in, for a raider (it stops at
/// the plan's edge, not the bed).
const IN: f32 = 5.0;

#[derive(Default, PartialEq)]
pub enum Night {
    #[default]
    Awake,
    Asleep,
    Done,
}

#[allow(clippy::too_many_arguments)]
pub fn sim_overnight(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    mut house: ResMut<Safehouse>,
    raid: Res<Raid>,
    player: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    places: Query<&GlobalTransform>,
    things: Query<(Entity, &SafehouseThing)>,
    mut doors: Query<&mut Door>,
    mut events: MessageWriter<InteractEvent>,
    mut night: Local<Night>,
) {
    if opts.sim_run.is_none() || std::env::var_os("SIM_OVERNIGHT").is_none() {
        return;
    }
    let (Some(cabin), Some(door)) = (house.cabin, house.door) else {
        return;
    };
    match *night {
        Night::Awake => {
            let (Ok((me, at)), Ok(c)) = (player.single(), places.get(cabin)) else {
                return;
            };
            if at.translation().xz().distance(c.translation().xz()) > IN
                || raid.phase == Phase::AtBase
                || raid.surge_in() < NIGHT + 120.0
            {
                return;
            }
            let Some((bed, _)) = things.iter().find(|(_, t)| **t == SafehouseThing::Bed) else {
                return;
            };
            house.state.cleared = true;
            house.state.barred = true;
            house.state.lit = true;
            if let Ok(mut d) = doors.get_mut(door) {
                d.open = false;
            }
            commands.entity(door).insert(Wedged(door));
            events.write(InteractEvent::Pressed {
                item: bed,
                player: me,
            });
            *night = Night::Asleep;
        }
        Night::Asleep if house.nights_out > 0 => {
            // Morning: the wedge up, the door open, back to the raid.
            commands.entity(door).remove::<Wedged>();
            if let Ok(mut d) = doors.get_mut(door) {
                d.open = true;
            }
            *night = Night::Done;
        }
        _ => {}
    }
}
