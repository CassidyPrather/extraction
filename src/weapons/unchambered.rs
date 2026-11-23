//! A gun clicked on an empty chamber with rounds still in its feed (a clip
//! stripped in and the bolt closed short of picking one up, a magazine in
//! and never racked): the player's mistake, said without words, since the
//! hint's text may not be read (playtest 2026-11-22: a Mosin's "dead
//! trigger" was an empty chamber, its hint unseen). The gun is marked
//! [`NeedsWorking`]: the hand feels a firmer double tap
//! (`rig::haptics`, [`crate::rig::shapes::UNCHAMBERED`]) and the action's
//! handle glows the glove's hover green (`render::handle_glow`) till the
//! bolt, slide or forend is worked or a round is in. It answers the
//! mistake: it's never lit otherwise.

use bevy::prelude::*;
use extraction_core::firearm::{ActionKind, Event as FireEvent, Round};

use super::{Gun, GunEvent};
use crate::interact::{GrabPoint, Grabbable, GripRole};

/// On a gun clicked empty with rounds in its feed, till it's worked.
#[derive(Component, Debug)]
pub struct NeedsWorking;

pub struct UnchamberedPlugin;

impl Plugin for UnchamberedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (mark_unchambered, clear_worked).chain());
    }
}

/// Rounds waiting to be chambered: in its magazine or its own (a clip's,
/// a tube's) feed.
pub fn rounds_in_feed(gun: &Gun) -> u32 {
    gun.firearm.magazine.as_ref().map_or(0, |m| m.rounds) + gun.firearm.internal_rounds
}

/// Whether a dry click on this gun means "work the action": it has a
/// handle to work and rounds to feed. (A revolver's or a double's empty
/// chamber is the next pull's business, or the hand's.)
pub fn answers_a_dry_click(gun: &Gun, grab: Option<&Grabbable>) -> bool {
    !matches!(
        gun.firearm.def.action,
        ActionKind::Revolver | ActionKind::BreakAction | ActionKind::Crossbow
    ) && rounds_in_feed(gun) > 0
        && grab.is_some_and(|g| action_point(gun, g).is_some())
}

/// The part that works the action: the bolt, slide or charging handle
/// (the first `Handle` grab point), or a pump's forend (its support grip:
/// a pump has no handle of its own).
pub fn action_point<'a>(gun: &Gun, g: &'a Grabbable) -> Option<&'a GrabPoint> {
    action_point_of(gun.firearm.def.action == ActionKind::Pump, g)
}

/// [`action_point`], told whether it's a pump.
pub fn action_point_of(pump: bool, g: &Grabbable) -> Option<&GrabPoint> {
    g.points.iter().find(|p| {
        if pump {
            p.role == GripRole::Support
        } else {
            matches!(p.role, GripRole::Handle(_))
        }
    })
}

fn mark_unchambered(
    mut commands: Commands,
    mut events: MessageReader<GunEvent>,
    guns: Query<(&Gun, Option<&Grabbable>), Without<NeedsWorking>>,
) {
    for ev in events.read() {
        if ev.event != FireEvent::DryFire {
            continue;
        }
        if let Ok((gun, grab)) = guns.get(ev.gun) {
            if answers_a_dry_click(gun, grab) {
                commands.entity(ev.gun).insert(NeedsWorking);
            }
        }
    }
}

/// Worked (the bolt's handle lifted, the bolt or slide drawn), a round in,
/// or nothing left to feed: the mark goes.
fn clear_worked(mut commands: Commands, guns: Query<(Entity, &Gun), With<NeedsWorking>>) {
    for (e, gun) in &guns {
        let f = &gun.firearm;
        let worked = f.bolt_travel > 0.1 || f.handle_lift > 0.3 || f.locked_back;
        if worked || f.chambers.first() == Some(&Some(Round::Live)) || rounds_in_feed(gun) == 0 {
            commands.entity(e).remove::<NeedsWorking>();
        }
    }
}
