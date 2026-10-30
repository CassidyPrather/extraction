//! Why you can't run: whatever's holding you back is said, on the wrist,
//! the moment you try (playtest 28 October: "why am I so slow", ten
//! minutes in the yard and no idea why; a crouch toggled by the stick, as
//! like as not, and nothing showed it).
//!
//! What stops a run is `player::move_bodies`' own list: crouched, both
//! hands on something bulky, a broken leg, stiff with cold, too much on
//! you, too deep in water, out of breath. The first of them that holds is
//! said when you push to run and don't ([`why_not`]); again only after
//! [`AGAIN`] seconds, or for another cause. And crouching is said the first
//! time it's on, each session: the toggle is a flick of the stick, easy to
//! do by mistake.

use bevy::prelude::*;
use extraction_core::inventory::Encumbrance;

use super::Raid;
use crate::launch::LaunchOptions;
use crate::player::{Body, FLOAT_DEPTH};
use crate::rig::{LocalPlayer, RigInput};

/// The same cause isn't said again for this long (s).
pub const AGAIN: f32 = 30.0;

/// What's stopping a run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Held {
    Crouched,
    Hauling,
    Limping,
    Chilled,
    Heavy(f32),
    Deep,
    Winded,
}

/// The first thing stopping a run, in `player::move_bodies`' terms.
pub fn why_not(input: &RigInput, body: &Body) -> Option<Held> {
    let enc = Encumbrance::from_mass(body.carried_kg);
    if input.crouch {
        Some(Held::Crouched)
    } else if body.hauling > 0.0 {
        Some(Held::Hauling)
    } else if body.limping >= crate::interact::wounds::LIMP {
        Some(Held::Limping)
    } else if body.chilled >= crate::anomaly::frost::STIFF {
        Some(Held::Chilled)
    } else if !enc.can_sprint {
        Some(Held::Heavy(body.carried_kg))
    } else if body.wading >= 0.5 && body.wading < FLOAT_DEPTH {
        Some(Held::Deep)
    } else if body.stamina <= 0.05 {
        Some(Held::Winded)
    } else {
        None
    }
}

/// What's said for it; `flat` for the keyboard's key.
pub fn line(held: Held, flat: bool) -> String {
    match held {
        Held::Crouched if flat => "Crouched: slow, no running. C to stand.".into(),
        Held::Crouched => "Crouched: slow, no running. Right stick down to stand.".into(),
        Held::Hauling => "Both hands on it: no running with that.".into(),
        Held::Limping => "On a broken leg: no running. Splint it.".into(),
        Held::Chilled => "Stiff with the cold: no running.".into(),
        Held::Heavy(kg) => format!("Too heavy to run: {kg:.0} kg on you."),
        Held::Deep => "Too deep to run.".into(),
        Held::Winded => "Out of breath. A moment.".into(),
    }
}

/// The crouch, the first time it's on in a session.
pub fn crouch_line(flat: bool) -> &'static str {
    if flat {
        "Crouched: quieter, slower. C again to stand."
    } else {
        "Crouched: quieter, slower. Right stick down again to stand."
    }
}

pub struct SlowedPlugin;

impl Plugin for SlowedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tell);
    }
}

/// Tried to run and couldn't, or crouched for the first time: why.
#[allow(clippy::type_complexity)]
fn tell(
    mut raid: ResMut<Raid>,
    opts: Res<LaunchOptions>,
    time: Res<Time>,
    players: Query<(&RigInput, &Body), With<LocalPlayer>>,
    // The last said and when; and the crouch said this session.
    mut said: Local<(Option<(Held, f32)>, bool)>,
) {
    let Ok((input, body)) = players.single() else {
        return;
    };
    let now = time.elapsed_secs();
    if input.crouch && !said.1 {
        said.1 = true;
        raid.say(crouch_line(opts.flat));
        said.0 = Some((Held::Crouched, now));
        return;
    }
    if !(input.sprint && input.locomotion.y > 0.3) {
        return;
    }
    let Some(held) = why_not(input, body) else {
        return;
    };
    let same = |a: Held, b: Held| std::mem::discriminant(&a) == std::mem::discriminant(&b);
    if said
        .0
        .is_some_and(|(h, at)| same(h, held) && now - at < AGAIN)
    {
        return;
    }
    said.0 = Some((held, now));
    raid.say(line(held, opts.flat));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap()
    }

    fn said(app: &App, what: &str) -> usize {
        app.world()
            .resource::<Raid>()
            .log
            .iter()
            .filter(|(l, _)| l.contains(what))
            .count()
    }

    fn speed(app: &App, p: Entity) -> f32 {
        let v = app.world().get::<Body>(p).unwrap().velocity;
        Vec2::new(v.x, v.z).length()
    }

    /// The playtest's yard: crouched by a flick of the stick, slow and
    /// no running, and nothing said why. Now it's said; and stood up, the
    /// run comes back.
    #[test]
    fn crouched_and_trying_to_run_you_are_told_and_standing_clears_it() {
        let mut app = crate::raid::requests::tests::app();
        let p = player(&mut app);
        let push = |app: &mut App, crouch: bool, frames: u32| {
            for _ in 0..frames {
                let mut i = app.world_mut().get_mut::<RigInput>(p).unwrap();
                i.crouch = crouch;
                i.sprint = true;
                i.locomotion = Vec2::new(0.0, 1.0);
                app.update();
            }
        };
        push(&mut app, true, 60);
        assert_eq!(
            said(&app, "Crouched"),
            1,
            "{:?}",
            app.world().resource::<Raid>().log
        );
        let low = speed(&app, p);
        assert!(low < crate::player::WALK_SPEED, "crouched: {low}");
        // Still pushing: not said again so soon.
        push(&mut app, true, 60);
        assert_eq!(said(&app, "Crouched"), 1);
        // Stood: running again, nothing holding it.
        push(&mut app, false, 90);
        let i = app.world().get::<RigInput>(p).unwrap().clone();
        assert_eq!(why_not(&i, app.world().get::<Body>(p).unwrap()), None);
        let fast = speed(&app, p);
        assert!(
            fast > crate::player::WALK_SPEED,
            "standing, running: {fast}"
        );
    }

    /// Every cause that stops a run has its line, and it fits the wrist.
    #[test]
    fn every_cause_is_said() {
        for held in [
            Held::Crouched,
            Held::Hauling,
            Held::Limping,
            Held::Chilled,
            Held::Heavy(41.0),
            Held::Deep,
            Held::Winded,
        ] {
            for flat in [false, true] {
                let l = line(held, flat);
                assert!(!l.is_empty() && l.len() <= 78, "{held:?}: {l}");
            }
        }
        assert!(crouch_line(false).len() <= 78);
    }
}
