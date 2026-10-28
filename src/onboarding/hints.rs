//! Hints where the game would otherwise just not answer: everywhere, not
//! only in the yard. A trigger squeezed on safe does nothing; a radio used
//! in the truck does nothing; an empty chamber clicks. Someone new can't
//! tell those from a broken control (the first headset session found most
//! controls "dead" and couldn't tell why), so the wrist says what to do,
//! a few times, then trusts you; and not at all once you've shown you
//! know (the playtest heard "Safety's on" three minutes after it had
//! taken the range's pistol off safe itself).

use bevy::prelude::*;
use extraction_core::firearm::{Event as FireEvent, FireMode};

use crate::interact::Held;
use crate::raid::{Phase, RadioCall, Raid};
use crate::rig::{LocalPlayer, RigInput};
use crate::weapons::{Gun, GunEvent};

/// How many times each hint is given in a session.
const TIMES: u32 = 3;
/// Seconds between two of the same hint.
const QUIET: f32 = 20.0;

pub struct HintsPlugin;

impl Plugin for HintsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hints>()
            .add_systems(Update, (on_safe, dry_click, radio_at_base));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hint {
    Safety,
    Empty,
    RadioAtBase,
}

impl Hint {
    pub fn line(self, flat: bool) -> &'static str {
        match (self, flat) {
            (Hint::Safety, false) => "Safety's on. Click the stick on the gun hand.",
            (Hint::Safety, true) => "Safety's on. Press X.",
            (Hint::Empty, false) => "Click. Nothing in the chamber: rack the slide or bolt.",
            (Hint::Empty, true) => "Click. Nothing in the chamber: press V (R reloads).",
            (Hint::RadioAtBase, _) => "Truck: \"I'm right here. Step out first.\"",
        }
    }
}

/// Hints given: how many times, and when last; and what you've shown you
/// know (done what the hint would tell you).
#[derive(Resource, Default, Debug)]
pub struct Hints {
    given: std::collections::HashMap<Hint, (u32, f32)>,
    known: std::collections::HashSet<Hint>,
}

impl Hints {
    /// Say it, unless it's been said enough or just now, or you know.
    pub fn give(&mut self, hint: Hint, now: f32, flat: bool, raid: &mut Raid) -> bool {
        if self.known.contains(&hint) {
            return false;
        }
        let e = self.given.entry(hint).or_insert((0, f32::NEG_INFINITY));
        if e.0 >= TIMES || now - e.1 < QUIET {
            return false;
        }
        e.0 += 1;
        e.1 = now;
        raid.say(hint.line(flat));
        true
    }

    pub fn given(&self, hint: Hint) -> u32 {
        self.given.get(&hint).map_or(0, |e| e.0)
    }

    /// You've done what it tells you: it's not said again.
    pub fn known(&mut self, hint: Hint) {
        self.known.insert(hint);
    }
}

/// A squeeze on a gun that's on safe.
fn on_safe(
    guns: Query<(&Gun, &Held)>,
    inputs: Query<&RigInput, With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    driver: crate::rig::KnownDriver,
    time: Res<Time>,
    mut was: Local<bool>,
) {
    let mut squeezed = false;
    for (gun, held) in &guns {
        let Ok(input) = inputs.get(held.player) else {
            continue;
        };
        if input.hand(held.side).trigger > 0.8 && gun.firearm.fire_mode() == FireMode::Safe {
            squeezed = true;
        }
    }
    if let (true, false, Some(flat)) = (squeezed, *was, driver.flat()) {
        hints.give(Hint::Safety, time.elapsed_secs(), flat, &mut raid);
    }
    *was = squeezed;
}

/// The hammer fell on nothing.
fn dry_click(
    mut events: MessageReader<GunEvent>,
    held: Query<&Held>,
    locals: Query<(), With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    driver: crate::rig::KnownDriver,
    time: Res<Time>,
) {
    for ev in events.read() {
        let mine = held.get(ev.gun).is_ok_and(|h| locals.contains(h.player));
        // Taken off safe, or a round chambered, by your own hand.
        match ev.event {
            FireEvent::ModeChanged(m) if mine && m != FireMode::Safe => hints.known(Hint::Safety),
            FireEvent::Chambered if mine => hints.known(Hint::Empty),
            _ => {}
        }
        if let (true, Some(flat)) = (mine && ev.event == FireEvent::DryFire, driver.flat()) {
            hints.give(Hint::Empty, time.elapsed_secs(), flat, &mut raid);
        }
    }
}

/// The radio does nothing while you're still at the truck.
fn radio_at_base(
    mut calls: MessageReader<RadioCall>,
    locals: Query<(), With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    time: Res<Time>,
) {
    for c in calls.read() {
        if !locals.contains(c.player) {
            continue;
        }
        if raid.phase == Phase::AtBase {
            hints.give(Hint::RadioAtBase, time.elapsed_secs(), false, &mut raid);
        } else {
            hints.known(Hint::RadioAtBase);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every hint fits the wrist whole.
    #[test]
    fn hints_fit_the_wrist() {
        for h in [Hint::Safety, Hint::Empty, Hint::RadioAtBase] {
            for flat in [false, true] {
                let l = h.line(flat);
                let n = crate::render::screen::wrap(l, 26, usize::MAX).len();
                assert!(n <= 3, "{n} lines: {l}");
            }
        }
    }
}
