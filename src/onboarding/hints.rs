//! Hints where the game would otherwise just not answer: everywhere, not
//! only in the yard. A trigger squeezed on safe does nothing; a radio used
//! in the truck does nothing; an empty chamber clicks. Someone new can't
//! tell those from a broken control (the first headset session found most
//! controls "dead" and couldn't tell why), so the wrist says what to do,
//! a few times, then trusts you. Where the gun already says it in its own
//! way (the safety's dead stop and buzz, `weapons::stoppages`), once.

use bevy::prelude::*;
use extraction_core::firearm::{Event as FireEvent, FireMode};

use crate::interact::Held;
use crate::raid::{Phase, RadioCall, Raid};
use crate::rig::{LocalPlayer, RigDriver, RigInput};
use crate::weapons::{Gun, GunEvent};

/// How many times each hint is given in a session.
const TIMES: u32 = 3;
/// Seconds between two of the same hint.
const QUIET: f32 = 20.0;

pub struct HintsPlugin;

/// What you've shown you know: done what the hint would tell you (off safe
/// by your own hand, a round chambered after a click, a call from out in
/// the Zone). It isn't said again (the playtest heard "Safety's on" three
/// minutes after it had taken the range's pistol off safe itself).
#[derive(Resource, Default, Debug)]
pub struct Known(pub std::collections::HashSet<Hint>);

impl Plugin for HintsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hints>().add_systems(
            Update,
            (on_safe, dry_click, slide_back, radio_at_base, one_up),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hint {
    Safety,
    Empty,
    SlideBack,
    RadioAtBase,
}

impl Hint {
    pub fn line(self, flat: bool) -> &'static str {
        match (self, flat) {
            (Hint::Safety, false) => "Safety's on. Click the stick on the gun hand.",
            (Hint::Safety, true) => "Safety's on. Press X.",
            (Hint::Empty, false) => "Click. Nothing in the chamber: rack the slide or bolt.",
            (Hint::Empty, true) => "Click. Nothing in the chamber: press V (R reloads).",
            (Hint::SlideBack, false) => "Slide's back. Press B to let it go.",
            (Hint::SlideBack, true) => "Slide's back. Press V to let it go.",
            (Hint::RadioAtBase, _) => "Truck: \"I'm right here. Step out first.\"",
        }
    }

    /// How many times it's said in a session: once where the gun tells it
    /// as well (the safety's dead stop and buzz).
    fn times(self) -> u32 {
        match self {
            Hint::Safety => 1,
            _ => TIMES,
        }
    }
}

/// Hints given: how many times, and when last.
#[derive(Resource, Default, Debug)]
pub struct Hints(std::collections::HashMap<Hint, (u32, f32)>);

impl Hints {
    /// Say it, unless it's been said enough or just now.
    pub fn give(&mut self, hint: Hint, now: f32, flat: bool, raid: &mut Raid) -> bool {
        let e = self.0.entry(hint).or_insert((0, f32::NEG_INFINITY));
        if e.0 >= hint.times() || now - e.1 < QUIET {
            return false;
        }
        e.0 += 1;
        e.1 = now;
        raid.say(hint.line(flat));
        true
    }

    pub fn given(&self, hint: Hint) -> u32 {
        self.0.get(&hint).map_or(0, |e| e.0)
    }
}

/// A squeeze on a gun that's on safe.
fn on_safe(
    guns: Query<(&Gun, &Held)>,
    inputs: Query<&RigInput, With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    driver: crate::rig::KnownDriver,
    known: Res<Known>,
    time: Res<Time>,
    mut was: Local<bool>,
) {
    let mut squeezed = false;
    for (gun, held) in &guns {
        if !inputs.contains(held.player) {
            continue;
        }
        if gun.pulled && gun.firearm.fire_mode() == FireMode::Safe {
            squeezed = true;
        }
    }
    // (Not till it's known which words, nor once you know.)
    if let (true, false, Some(flat)) = (squeezed, *was, driver.flat()) {
        if !known.0.contains(&Hint::Safety) {
            hints.give(Hint::Safety, time.elapsed_secs(), flat, &mut raid);
        }
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
    mut known: ResMut<Known>,
    mut clicked: Local<Vec<Entity>>,
    time: Res<Time>,
) {
    // Guns in a hand only, once each (every dry click pushed its gun
    // again, for the session).
    clicked.retain(|g| held.contains(*g));
    for ev in events.read() {
        let mine = held.get(ev.gun).is_ok_and(|h| locals.contains(h.player));
        if !mine {
            continue;
        }
        match ev.event {
            FireEvent::ModeChanged(m) if m != FireMode::Safe => {
                known.0.insert(Hint::Safety);
            }
            // Racked after it clicked: you know what the click means.
            FireEvent::Chambered if clicked.contains(&ev.gun) => {
                known.0.insert(Hint::Empty);
            }
            FireEvent::DryFire => {
                if !clicked.contains(&ev.gun) {
                    clicked.push(ev.gun);
                }
                if let (Some(flat), false) = (driver.flat(), known.0.contains(&Hint::Empty)) {
                    hints.give(Hint::Empty, time.elapsed_secs(), flat, &mut raid);
                }
            }
            _ => {}
        }
    }
}

/// The slide locked back on the last round, or the trigger pulled on an
/// open action: what lets it go.
fn slide_back(
    mut events: MessageReader<GunEvent>,
    held: Query<&Held>,
    guns: Query<&Gun>,
    locals: Query<(), With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    driver: Res<RigDriver>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let mine = held.get(ev.gun).is_ok_and(|h| locals.contains(h.player));
        let holds_open = guns
            .get(ev.gun)
            .is_ok_and(|g| g.firearm.def.last_round_hold_open && g.firearm.locked_back);
        let open =
            ev.event == FireEvent::ActionOpen || (ev.event == FireEvent::BoltLocked && holds_open);
        if mine && open && holds_open {
            let flat = *driver == RigDriver::Flat;
            hints.give(Hint::SlideBack, time.elapsed_secs(), flat, &mut raid);
        }
    }
}

/// The radio does nothing while you're still at the truck.
fn radio_at_base(
    mut calls: MessageReader<RadioCall>,
    locals: Query<(), With<LocalPlayer>>,
    mut hints: ResMut<Hints>,
    mut raid: ResMut<Raid>,
    mut known: ResMut<Known>,
    time: Res<Time>,
) {
    for c in calls.read() {
        if !locals.contains(c.player) {
            continue;
        }
        if raid.phase != Phase::AtBase {
            known.0.insert(Hint::RadioAtBase);
        } else if !known.0.contains(&Hint::RadioAtBase) {
            hints.give(Hint::RadioAtBase, time.elapsed_secs(), false, &mut raid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every hint fits the wrist whole.
    #[test]
    fn hints_fit_the_wrist() {
        for h in [
            Hint::Safety,
            Hint::Empty,
            Hint::SlideBack,
            Hint::RadioAtBase,
        ] {
            for flat in [false, true] {
                let l = h.line(flat);
                let n = crate::render::screen::wrap(l, 26, usize::MAX).len();
                assert!(n <= 3, "{n} lines: {l}");
            }
        }
    }
}

/// What the driver says as you step off the truck with nothing chambered
/// and rounds in the gun's magazine.
pub const ONE_UP: &str = "Truck: \"One up the spout before you go. Safety on till you need it.\"";
/// ...its magazine empty, a loaded one on you.
pub const MAG_IN: &str = "Truck: \"Mag in, then one up the spout. Safety on till you need it.\"";
/// ...the gun and every magazine on you empty, rounds in the crate.
pub const LOAD_UP: &str = "Truck: \"That's empty, your spares too. Load up from the crate.\"";
/// ...none on you, but a loaded gun down in your pack.
pub const IN_PACK: &str = "Truck: \"That one's empty. Your loaded one's in your pack: swap them.\"";
/// ...and none in the crate either.
pub const NO_ROUNDS: &str = "Truck: \"That's empty, and you've no rounds. Go careful.\"";

/// Off the truck's steps at base (as `progression`'s tailgate: the raid
/// begins 14 m off).
const OFF_THE_TRUCK: f32 = 3.0;

/// Stepping off the truck at base with the gun on you and nothing in its
/// chamber: the driver reminds you of what the range taught, once each
/// time home. On the first job that's always so (a new career's PM has its
/// magazine in, nothing up, and the safety on); after, only if you leave
/// that way again. Not a step: nothing waits on it. Not in the yard.
#[allow(clippy::type_complexity)]
fn one_up(
    mut raid: ResMut<Raid>,
    opts: Res<crate::launch::LaunchOptions>,
    players: Query<(Entity, &GlobalTransform), With<LocalPlayer>>,
    trucks: Query<&GlobalTransform, With<crate::raid::truck::Truck>>,
    guns: Query<(
        &Gun,
        &crate::anomaly::artifacts::CarriedBy,
        Has<crate::interact::body::InBackpack>,
    )>,
    mags: Query<(
        &crate::items::Item,
        Option<&crate::anomaly::artifacts::CarriedBy>,
        &crate::weapons::Mag,
        Option<&crate::interact::InSocket>,
    )>,
    ammo: Query<&crate::items::Item>,
    stash: Query<&crate::interact::body::Backpack, With<crate::raid::truck::Stash>>,
    catalog: Res<crate::items::Catalog>,
    // Said this time home; how far off the truck you were last frame.
    mut said: Local<(bool, Option<f32>)>,
) {
    if raid.phase != Phase::AtBase {
        *said = (false, None);
        return;
    }
    if said.0 || opts.map == crate::camp::TRAINING_YARD {
        return;
    }
    let (Some((me, at)), Some(truck)) = (players.iter().next(), trucks.iter().next()) else {
        return;
    };
    let d = at.translation().xz().distance(truck.translation().xz());
    let was = said.1.replace(d);
    // Walked off the steps, a step at a time (not put at camp).
    let stepped = was.is_some_and(|w| w < OFF_THE_TRUCK && d - w < 1.0);
    if d < OFF_THE_TRUCK || !stepped {
        return;
    }
    said.0 = true;
    // Every gun on you (two pistols: the first found was the empty one,
    // and the advice was for it, not the one drawn).
    let mine: Vec<&Gun> = guns
        .iter()
        .filter(|(_, by, packed)| by.0 == me && !packed)
        .map(|(g, ..)| g)
        .collect();
    let gun = mine.first().copied();
    let empty = !mine.is_empty()
        && mine
            .iter()
            .all(|g| g.firearm.chambers.iter().all(|c| c.is_none()));
    info!(
        "off the truck: {}",
        match (gun.is_some(), empty) {
            (false, _) => "no gun on you",
            (true, true) => "nothing chambered",
            (true, false) => "a round up",
        }
    );
    // What there is to load, by what you're holding (never advice wrong for
    // it: "one up the spout" to an empty gun with empty spares racked
    // nothing): in the gun's magazine, a loaded one on you, or rounds in
    // the crate.
    let in_gun = mine
        .iter()
        .any(|g| g.firearm.magazine.as_ref().is_some_and(|m| m.rounds > 0));
    // (One that fits it: an AK's magazine on you is no help to the PM, and
    // "Mag in" sent the newcomer out with none it could use.)
    let fits = |m: &crate::weapons::Mag| {
        mine.iter()
            .any(|g| g.firearm.def.magazine_family.as_deref() == Some(m.family.as_str()))
    };
    // (Not one seated in a gun: the loaded pistol down in the pack had its
    // magazine counted as a spare, and "Mag in" had nothing to put in.)
    let wells: Vec<Entity> = guns.iter().filter_map(|(g, ..)| g.magwell).collect();
    let spare = mags.iter().any(|(it, by, m, sock)| {
        it.rounds > 0
            && fits(m)
            && by.is_some_and(|b| b.0 == me)
            && !sock.is_some_and(|s| wells.contains(&s.socket))
    });
    // A loaded gun of your own, packed away.
    let packed = guns.iter().any(|(g, by, packed)| {
        by.0 == me && packed && g.firearm.magazine.as_ref().is_some_and(|m| m.rounds > 0)
    });
    let in_crate = stash.iter().flat_map(|b| b.contents.iter()).any(|e| {
        ammo.get(*e).is_ok_and(|it| {
            it.rounds > 0
                && (mags.get(*e).is_ok_and(|(_, _, m, _)| fits(m))
                    || catalog.get(it.kind()).id.starts_with("ammo_"))
        })
    });
    if empty {
        raid.say(match (in_gun, spare, packed, in_crate) {
            (true, ..) => ONE_UP,
            (false, true, ..) => MAG_IN,
            (false, false, true, _) => IN_PACK,
            (false, false, false, true) => LOAD_UP,
            (false, false, false, false) => NO_ROUNDS,
        });
    }
}
