//! Each gun's trigger as a finger meets it: where it breaks along the
//! controller's travel, a double action's long pull, and a two-stage
//! trigger's wall.
//!
//! Every gun once broke at the same 0.6 of the controller's travel
//! (`rig::PRESS_AT`): a PM's long double-action pull was as short as its
//! single-action one, an SKS had no take-up, and a Mosin's long pull and a
//! TOZ-8's light one felt the same. Now:
//!
//! - **The break** ([`TriggerFeel::single`]): a light .22 breaks early, a
//!   service rifle's long military pull late.
//! - **A double action** ([`TriggerFeel::double`]): with the hammer down,
//!   the pull that cocks it and lets it go is longer.
//! - **A wall** ([`TriggerFeel::wall`]): a two-stage trigger's slack taken
//!   up, felt as a tick ([`WALL_TICK`]) in the finger before the break.
//!
//! Every break resets [`RESET_GAP`] under itself, as every analog press
//! does (`rig::pressed`): a finger resting on it or a squeeze wavering as
//! it's held neither fires nor resets. Every break leaves [`HEADROOM`] of
//! travel before the controller's end ([`FULL_TRAVEL`], logged at start):
//! Index and Quest triggers differ in how near their end they read 1.

use bevy::prelude::*;
use extraction_core::firearm::Firearm;

/// Where the rig takes a trigger's mechanical end to read (0..1).
pub const FULL_TRAVEL: f32 = 1.0;
/// Travel every break leaves before that end.
pub const HEADROOM: f32 = 0.1;
/// How far under its break a trigger must come back to reset: the shared
/// analog press's gap.
pub const RESET_GAP: f32 = crate::rig::PRESS_AT - crate::rig::RELEASE_AT;
/// A two-stage trigger's wall in the finger (amplitude, seconds): a tick.
pub const WALL_TICK: (f32, f32) = (0.15, 0.012);
/// How far back under its wall a finger must come to feel it again.
pub const WALL_RESET: f32 = 0.1;

/// A gun's trigger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerFeel {
    /// Where it breaks with the hammer cocked (or on a striker or an open
    /// bolt), as the controller reads.
    pub single: f32,
    /// Where it breaks cocking the hammer itself, on a double action.
    pub double: Option<f32>,
    /// A two-stage trigger's wall: the slack taken up, felt, before the
    /// break.
    pub wall: Option<f32>,
}

/// Every gun not named breaks where every analog press does.
pub const PLAIN: TriggerFeel = TriggerFeel {
    single: crate::rig::PRESS_AT,
    double: None,
    wall: None,
};

/// A gun's trigger, by its catalog id.
pub fn of(id: &str) -> TriggerFeel {
    match id {
        // A PM: a long, heavy double-action pull off a lowered hammer, a
        // short single-action one cocked.
        "pm_pistol" => TriggerFeel {
            single: 0.55,
            double: Some(0.85),
            wall: None,
        },
        // A Nagant: its double action is the long, heavy one of legend.
        "nagant" => TriggerFeel {
            single: 0.55,
            double: Some(0.88),
            wall: None,
        },
        // An SKS: a military two-stage, the slack up to a wall and then
        // the break.
        "sks" => TriggerFeel {
            single: 0.65,
            double: None,
            wall: Some(0.4),
        },
        // A Mosin: a long, creeping single-stage pull.
        "mosin" => TriggerFeel {
            single: 0.75,
            double: None,
            wall: None,
        },
        // A TOZ-8: a light target .22's.
        "toz8" => TriggerFeel {
            single: 0.45,
            double: None,
            wall: None,
        },
        _ => PLAIN,
    }
}

/// Where a gun's trigger breaks now: a double action's long pull with its
/// hammer down.
pub fn break_at(feel: &TriggerFeel, firearm: &Firearm) -> f32 {
    match feel.double {
        Some(d) if firearm.def.double_action && !firearm.hammers_cocked.iter().any(|h| *h) => d,
        _ => feel.single,
    }
}

/// Whether a trigger breaking at `at` is pulled, from the finger's reading
/// and whether it was: it breaks past `at` and resets under it by
/// [`RESET_GAP`].
pub fn pulled(was: bool, value: f32, at: f32) -> bool {
    if was {
        value >= at - RESET_GAP
    } else {
        value > at
    }
}

/// Whether a finger at `value` comes onto a two-stage trigger's wall:
/// once a pull, felt again only after easing back off it.
pub fn onto_wall(feel: &TriggerFeel, on: &mut bool, value: f32) -> bool {
    let Some(w) = feel.wall else {
        return false;
    };
    if *on {
        if value < w - WALL_RESET {
            *on = false;
        }
        false
    } else if value >= w {
        *on = true;
        true
    } else {
        false
    }
}

/// Says at start what the triggers take the controller's end to be.
pub fn log_travel() {
    info!(
        "triggers: breaks assume a trigger reads {FULL_TRAVEL} at its end, each leaving {HEADROOM} before it"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Catalog;

    /// Every gun's breaks (single and double) leave the controller
    /// [`HEADROOM`] before its end, and enough under them to reset; walls
    /// come before breaks.
    #[test]
    fn every_break_leaves_travel_before_the_end() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        let mut seen = 0;
        for (_, def) in catalog.iter() {
            if def.firearm.is_none() {
                continue;
            }
            seen += 1;
            let t = of(&def.id);
            for at in [Some(t.single), t.double].into_iter().flatten() {
                if at > FULL_TRAVEL - HEADROOM {
                    bad.push(format!(
                        "{}: breaks at {at}, past {}",
                        def.id,
                        FULL_TRAVEL - HEADROOM
                    ));
                }
                if at - RESET_GAP < 0.1 {
                    bad.push(format!("{}: breaks at {at}, too light to reset", def.id));
                }
            }
            if let Some(w) = t.wall {
                if w >= t.single || w - WALL_RESET < 0.05 {
                    bad.push(format!(
                        "{}: a wall at {w}, the break at {}",
                        def.id, t.single
                    ));
                }
            }
        }
        assert!(seen >= 10, "only {seen} guns");
        assert!(bad.is_empty(), "{bad:#?}");
    }

    /// The first hour's guns each break where their own trigger does: a
    /// .22 lightest, a PM cocked, an SKS past its wall, a Mosin late, a PM
    /// off a lowered hammer latest; no two the same.
    #[test]
    fn the_first_hours_guns_break_apart() {
        let order = [
            of("toz8").single,
            of("pm_pistol").single,
            of("sks").single,
            of("mosin").single,
            of("pm_pistol").double.unwrap(),
        ];
        assert!(order.windows(2).all(|w| w[1] - w[0] >= 0.08), "{order:?}");
        assert!(of("sks").wall.is_some(), "the SKS's two stages");
    }
}

#[cfg(test)]
mod vr_tests {
    use super::*;
    use crate::rig::Side;
    use crate::rig::haptics::Haptic;
    use crate::weapons::feel::vr_tests::*;

    #[derive(Resource, Default)]
    struct Felt(Vec<Haptic>);

    /// The ticks of the wall's size felt in the gun hand as the trigger
    /// goes through `trace`.
    fn wall_ticks(r: &mut Range, trace: &[f32]) -> usize {
        r.vr.app.world_mut().resource_mut::<Felt>().0.clear();
        for v in trace {
            r.vr.trigger(Side::Right, *v);
            r.vr.step(1);
        }
        r.vr.app
            .world()
            .resource::<Felt>()
            .0
            .iter()
            .filter(|h| h.side == Side::Right && (h.amplitude, h.seconds) == WALL_TICK)
            .count()
    }

    /// An SKS in the hand: the slack taken up to its wall is felt as a
    /// tick, once a pull, and the finger resting there (or wavering on the
    /// wall) neither fires nor ticks again; eased off and taken up again,
    /// it ticks again. A Mosin's single stage has no wall.
    #[test]
    fn an_sks_has_a_wall_before_its_break() {
        let mut r = hold("sks", false, false);
        r.vr.app.init_resource::<Felt>().add_systems(
            Update,
            |mut h: MessageReader<Haptic>, mut f: ResMut<Felt>| f.0.extend(h.read().copied()),
        );
        r.vr.step(5);
        let up: Vec<f32> = (0..=10).map(|k| k as f32 * 0.05).collect();
        let waver = [0.48, 0.42, 0.5, 0.38, 0.47, 0.41];
        let ticks = wall_ticks(&mut r, &[up.as_slice(), &waver].concat());
        assert_eq!(ticks, 1, "taken up to the wall and resting on it");
        assert!(
            !gun_mut(&mut r).pulled,
            "resting on the wall doesn't break it"
        );
        let again = wall_ticks(&mut r, &[0.1, 0.1, 0.45, 0.45]);
        assert_eq!(again, 1, "eased off and taken up again");
        drop(r);
        let mut m = hold("mosin", false, false);
        m.vr.app.init_resource::<Felt>().add_systems(
            Update,
            |mut h: MessageReader<Haptic>, mut f: ResMut<Felt>| f.0.extend(h.read().copied()),
        );
        m.vr.step(5);
        let up: Vec<f32> = (0..=14).map(|k| k as f32 * 0.05).collect();
        assert_eq!(wall_ticks(&mut m, &up), 0, "a Mosin's single stage");
    }
}
