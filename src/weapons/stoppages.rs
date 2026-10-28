//! Every way a gun stops, told apart as it happens and cleared by what a
//! shooter would do. Each stoppage has its own sound (`audio`), feel
//! (`rig::haptics`, `faults`) and look ([`legibility::look`]): you look at
//! the port, feel the trigger and hear the action, no screen. Each has one
//! physical clear, done in the hand (`work_handles` racks, `tap_magazines`
//! taps, the mag release drops, `faults::rod_bores` rods, a palm slap frees a
//! stuck bolt), and the wrong one leaves it stopped, showing why.
//!
//! How often each happens is the gun's condition and dirt
//! (`Firearm::malfunction_chance`, the dud chance, `sticky_chance`), so
//! the fabricator bench and cleaning decide it.
//!
//! [`stoppages`] walks a gun through each it can have, on the mechanism
//! itself, with the dice set to make it happen.

use extraction_core::ammo::Load;
use extraction_core::firearm::{
    ActionKind, Event as FireEvent, FireMode, Firearm, FirearmDef, Magazine,
};

use super::legibility::{Look, look};

/// One way a gun stops: what it's left as and what the moment sent, the
/// clear and the wrong fixes, each a hand's doing on the mechanism.
pub struct Stoppage {
    pub name: &'static str,
    pub gun: Firearm,
    pub moment: Vec<FireEvent>,
    pub look: Look,
    pub clear: (&'static str, Fix),
    pub wrong: Vec<(&'static str, Fix)>,
    /// Whether it holds the gun until a hand clears it. A double-action
    /// gun's dud doesn't: the next pull strikes again (or turns a
    /// revolver's cylinder on), which is its clear.
    pub stops: bool,
}

pub type Fix = fn(&mut Firearm) -> Vec<FireEvent>;

/// The dice a pull uses: `first`, then nothing going wrong.
fn dice(first: &[f32]) -> impl FnMut() -> f32 + '_ {
    let mut i = 0;
    move || {
        i += 1;
        first.get(i - 1).copied().unwrap_or(0.99)
    }
}

/// The trigger pressed and let go, the dice `rolls`.
pub fn pull(g: &mut Firearm, rolls: &[f32]) -> Vec<FireEvent> {
    let mut r = dice(rolls);
    let mut ev = g.update(0.016, true, &mut r);
    ev.extend(g.update(0.016, false, &mut r));
    ev
}

/// A second and a bit with the action left alone.
pub fn wait(g: &mut Firearm) -> Vec<FireEvent> {
    let mut r = dice(&[]);
    let mut ev = Vec::new();
    for _ in 0..80 {
        ev.extend(g.update(0.016, false, &mut r));
    }
    ev
}

/// Whether a press of the trigger (one per barrel) would send a clean shot
/// downrange now. Tried on a copy.
pub fn fires(g: &Firearm) -> bool {
    let mut g = g.clone();
    (0..g.def.barrels.max(1)).any(|_| shot(&pull(&mut g, &[])))
}

/// A shot, and not one that wrecked the gun doing it.
pub fn shot(ev: &[FireEvent]) -> bool {
    ev.iter().any(|e| matches!(e, FireEvent::Fired { .. }))
        && !ev
            .iter()
            .any(|e| matches!(e, FireEvent::Bulged { .. } | FireEvent::OutOfBattery))
}

fn mag(def: &FirearmDef, rounds: u32, load: Load) -> Magazine {
    let mut m = Magazine {
        family: def.magazine_family.clone().unwrap_or_default(),
        capacity: 30,
        rounds,
        ..Default::default()
    };
    for i in 0..rounds {
        m.loads.push(i, load);
    }
    m
}

/// Works the action through, as a hand does on each kind: the slide or
/// bolt back and home, a turn-bolt lifted, drawn, pushed and turned down,
/// a pump run, a double broken and shut, a revolver's cylinder brought
/// round by cocking.
pub fn cycle(g: &mut Firearm) -> Vec<FireEvent> {
    let mut ev = Vec::new();
    match g.def.action {
        ActionKind::BoltAction => {
            ev.extend(g.set_handle_lift(1.0));
            ev.extend(g.set_bolt_travel(1.0));
            ev.extend(g.set_bolt_travel(0.0));
            ev.extend(g.set_handle_lift(0.0));
        }
        ActionKind::Pump => {
            ev.extend(g.release_action());
            ev.extend(g.set_bolt_travel(1.0));
            ev.extend(g.set_bolt_travel(0.0));
        }
        ActionKind::BreakAction => {
            ev.extend(g.set_break_open(true));
            ev.extend(g.set_break_open(false));
        }
        ActionKind::Revolver => ev.extend(g.cock_hammer()),
        _ => {
            ev.extend(g.set_bolt_travel(1.0));
            ev.extend(g.release_bolt());
        }
    }
    ev
}

/// A gun loaded with `load` (a double with one shell, so a dud stops it),
/// a round up, off safe.
pub fn loaded(def: &FirearmDef, load: Load) -> Firearm {
    let mut g = Firearm::new(def.clone(), 1.0);
    match def.action {
        ActionKind::SemiAuto if def.magazine_family.is_some() => {
            g.insert_magazine(mag(def, 10, load)).unwrap();
            cycle(&mut g);
        }
        ActionKind::SemiAuto => {
            g.set_bolt_travel(1.0);
            g.release_bolt();
            let mut loads = Default::default();
            for i in 0..def.internal_capacity {
                extraction_core::ammo::Loads::push(&mut loads, i, load);
            }
            g.strip_clip_of(def.internal_capacity, &mut loads);
            g.release_slide_stop();
        }
        ActionKind::BoltAction => {
            g.set_handle_lift(1.0);
            g.set_bolt_travel(1.0);
            for _ in 0..def.internal_capacity.max(1) {
                g.load_round_of(load);
            }
            g.set_bolt_travel(0.0);
            g.set_handle_lift(0.0);
        }
        ActionKind::Pump => {
            for _ in 0..4 {
                g.load_round_of(load);
            }
            cycle(&mut g);
        }
        ActionKind::BreakAction => {
            g.set_break_open(true);
            g.load_round_of(load);
            g.set_break_open(false);
        }
        ActionKind::Revolver => {
            g.set_gate(true);
            for _ in 0..g.chambers.len() {
                g.load_round_of(load);
                g.rotate_cylinder();
            }
            g.set_gate(false);
        }
        ActionKind::Crossbow => {
            g.set_bolt_travel(1.0);
            g.release_bolt();
            g.load_round_of(load);
        }
    }
    while g.fire_mode() == FireMode::Safe {
        g.cycle_mode();
    }
    g
}

/// The action worked to throw out what's in the chamber, and the gun
/// reloaded if that was its last.
fn back_in_action(g: &mut Firearm) -> Vec<FireEvent> {
    let mut ev = cycle(g);
    if g.live_rounds() == 0 {
        ev.extend(reload(g));
    }
    ev
}

/// The reload that puts a gun back in action: a fresh magazine, rounds
/// thumbed in, shells in the breech, a bolt on the rail.
fn reload(g: &mut Firearm) -> Vec<FireEvent> {
    let def = g.def.clone();
    let mut ev = Vec::new();
    match def.action {
        ActionKind::SemiAuto if def.magazine_family.is_some() => {
            g.remove_magazine();
            ev.push(FireEvent::MagazineRemoved);
            ev.extend(
                g.insert_magazine(mag(&def, 10, Load::Standard))
                    .unwrap_or_default(),
            );
            if g.locked_back {
                ev.extend(g.release_slide_stop());
            } else {
                ev.extend(cycle(g));
            }
            if def.open_bolt && !g.locked_back {
                ev.extend(cycle(g));
            }
        }
        // The same gun, not a fresh one: whatever's wrong with it stays.
        ActionKind::SemiAuto => {
            ev.extend(g.set_bolt_travel(1.0));
            ev.extend(g.release_bolt());
            let mut loads = Default::default();
            g.strip_clip_of(def.internal_capacity, &mut loads);
            ev.push(FireEvent::RoundLoaded);
            ev.extend(g.release_slide_stop());
        }
        ActionKind::BoltAction => {
            ev.extend(g.set_handle_lift(1.0));
            ev.extend(g.set_bolt_travel(1.0));
            for _ in 0..def.internal_capacity.max(1) {
                if g.load_round_of(Load::Standard) {
                    ev.push(FireEvent::RoundLoaded);
                }
            }
            ev.extend(g.set_bolt_travel(0.0));
            ev.extend(g.set_handle_lift(0.0));
        }
        ActionKind::Pump => {
            for _ in 0..4 {
                if g.load_round_of(Load::Standard) {
                    ev.push(FireEvent::RoundLoaded);
                }
            }
            ev.extend(cycle(g));
        }
        ActionKind::BreakAction => {
            ev.extend(g.set_break_open(true));
            for b in 0..def.barrels {
                g.unload_chamber(b);
            }
            if g.load_round_of(Load::Standard) {
                ev.push(FireEvent::RoundLoaded);
            }
            ev.extend(g.set_break_open(false));
        }
        ActionKind::Revolver => {
            ev.extend(g.set_gate(true));
            for _ in 0..g.chambers.len() {
                ev.extend(g.eject_at_gate());
                if g.load_round_of(Load::Standard) {
                    ev.push(FireEvent::RoundLoaded);
                }
                ev.extend(g.rotate_cylinder());
            }
            ev.extend(g.set_gate(false));
        }
        ActionKind::Crossbow => {
            if !g.locked_back {
                ev.extend(cycle(g));
            }
            if g.load_round_of(Load::Standard) {
                ev.push(FireEvent::RoundLoaded);
            }
        }
    }
    ev
}

/// Every stoppage `def` can have, each brought on and left as it would
/// be in the hand.
pub fn stoppages(id: &str, def: &FirearmDef) -> Vec<Stoppage> {
    use ActionKind::*;
    let mut out = Vec::new();
    let mut add = |name, gun: Firearm, moment: Vec<FireEvent>, clear, wrong| {
        out.push(Stoppage {
            name,
            look: look(id, &gun),
            stops: !(name == "dud" && gun.def.double_action),
            gun,
            moment,
            clear,
            wrong,
        });
    };
    let crossbow = def.action == Crossbow;
    let closed_self_loader = def.action == SemiAuto && !def.open_bolt;
    let detachable = def.magazine_family.is_some();

    if !crossbow {
        // A dud: the hammer falls, the round doesn't go.
        let mut g = loaded(def, Load::Standard);
        let moment = pull(&mut g, &[0.0]);
        let clear: (&'static str, Fix) = match def.action {
            Revolver => ("pull again: the cylinder brings the next round", |g| {
                pull(g, &[])
            }),
            _ if def.double_action => ("pull again: double action strikes it again", |g| {
                pull(g, &[])
            }),
            BreakAction => ("break it open, swap the shell, shut it", |g| {
                let mut ev = g.set_break_open(true);
                g.unload_chamber(0);
                g.load_round_of(Load::Standard);
                ev.extend(g.set_break_open(false));
                ev
            }),
            _ => (
                "work the action: the dud flies out, a fresh one in",
                back_in_action,
            ),
        };
        let mut wrong: Vec<(&'static str, Fix)> = Vec::new();
        if !def.double_action {
            wrong.push(("wait for it", wait));
        }
        if !def.double_action && def.action != BreakAction {
            wrong.push(("pull again", |g| pull(g, &[])));
        }
        if def.action == Revolver {
            wrong.push(("open the gate to look", |g| {
                let mut ev = g.set_gate(true);
                ev.extend(pull(g, &[]));
                ev
            }));
        }
        add("dud", g, moment, clear, wrong);
    }
    if !crossbow && !def.open_bolt {
        // A hang-fire: the hammer falls, the primer fizzes, it goes late.
        let mut g = loaded(def, Load::Standard);
        g.ammo_cold = 1.0;
        let moment = pull(&mut g, &[0.99, 0.0, 0.5]);
        let opens: Option<(&'static str, Fix)> = match def.action {
            Revolver => None,
            BreakAction => Some(("break it open to look", |g| g.set_break_open(true))),
            BoltAction => Some(("lift the bolt to look", |g| g.set_handle_lift(1.0))),
            _ => Some(("rack it to look", |g| g.set_bolt_travel(0.5))),
        };
        let mut wrong: Vec<(&'static str, Fix)> = opens.into_iter().collect();
        if def.action == Revolver {
            wrong.push(("pull again", |g| pull(g, &[])));
        }
        add(
            "hang-fire",
            g,
            moment,
            ("hold it on the target and wait", wait),
            wrong,
        );
    }
    if def.action == SemiAuto {
        // A stovepipe: the case caught upright in the port.
        let mut g = loaded(def, Load::Standard);
        let moment = pull(&mut g, &[0.99, 0.0, 0.1]);
        add(
            "stovepipe",
            g,
            moment,
            ("rack it: the case flies", cycle),
            vec![
                ("pull again", |g| pull(g, &[])),
                ("tap the magazine", |g| g.tap_magazine()),
            ],
        );
    }
    if closed_self_loader && detachable {
        // A failure to feed: the magazine slipped, the bolt home on nothing.
        let mut g = loaded(def, Load::Standard);
        let moment = pull(&mut g, &[0.99, 0.0, 0.6]);
        add(
            "failure to feed",
            g,
            moment,
            ("tap the magazine home, rack", |g| {
                let mut ev = g.tap_magazine();
                ev.extend(cycle(g));
                ev
            }),
            vec![("rack it", cycle), ("pull again", |g| pull(g, &[]))],
        );
        // A double feed: the case still in, the next round rammed into it.
        let mut g = loaded(def, Load::Standard);
        let moment = pull(&mut g, &[0.99, 0.0, 0.9]);
        add(
            "double feed",
            g,
            moment,
            ("drop the magazine, rack, seat it, rack", |g| {
                let def = g.def.clone();
                let m = g
                    .remove_magazine()
                    .unwrap_or_else(|| mag(&def, 8, Load::Standard));
                let mut ev = vec![FireEvent::MagazineRemoved];
                ev.extend(cycle(g));
                ev.extend(g.insert_magazine(m).unwrap_or_default());
                ev.extend(cycle(g));
                ev
            }),
            vec![
                ("rack it", cycle),
                ("tap the magazine", |g| g.tap_magazine()),
            ],
        );
    }
    if (closed_self_loader && detachable) || def.action == BoltAction {
        // A short stroke: a stiff action worked timidly. On a self-loader,
        // a fresh magazine in and the slide drawn half way: nothing fed. On
        // a turn-bolt after the shot: the case still in.
        let mut g = Firearm::new(def.clone(), 1.0);
        if def.action == SemiAuto {
            g.insert_magazine(mag(def, 10, Load::Standard)).unwrap();
            while g.fire_mode() == FireMode::Safe {
                g.cycle_mode();
            }
        } else {
            g = loaded(def, Load::Standard);
            pull(&mut g, &[]);
            g.set_handle_lift(1.0);
        }
        let mut moment = g.set_bolt_travel(0.5);
        moment.extend(g.set_bolt_travel(0.0));
        moment.extend(g.short_stroke());
        if def.action == BoltAction {
            g.set_handle_lift(0.0);
        }
        let short: Fix = |g| {
            let mut ev = g.set_handle_lift(1.0);
            ev.extend(g.set_bolt_travel(0.5));
            ev.extend(g.set_bolt_travel(0.0));
            ev.extend(g.set_handle_lift(0.0));
            ev
        };
        add(
            "short stroke",
            g,
            moment,
            ("work it fully, all the way back", back_in_action),
            vec![("work it short again", short)],
        );
    }
    if !crossbow {
        // A squib: a pop, no kick, the bullet stuck in the bore.
        let mut g = loaded(def, Load::Repacked);
        let moment = pull(&mut g, &[0.99, 0.0]);
        add(
            "squib",
            g,
            moment,
            ("rod the bore, then work the action", |g| {
                let mut ev = g.rod_through();
                ev.extend(back_in_action(g));
                ev
            }),
            vec![("work the action and fire", |g| {
                let mut ev = back_in_action(g);
                ev.extend(pull(g, &[]));
                ev
            })],
        );
    }
    if def.action == BoltAction {
        // A filthy bolt stuck shut after the shot.
        let mut g = loaded(def, Load::Standard);
        g.fouling = 1.0;
        let moment = pull(&mut g, &[0.99, 0.0]);
        add(
            "stuck bolt",
            g,
            moment,
            ("slap the handle up, work the bolt", |g| {
                let mut ev = g.free_bolt();
                ev.extend(back_in_action(g));
                ev
            }),
            vec![("lift the handle", cycle)],
        );
    }
    {
        // Empty: the last round gone. A gun that holds open says so on the
        // last shot; the rest click on the next pull.
        let mut g = loaded(def, Load::Standard);
        let mut last = Vec::new();
        for _ in 0..60 {
            if g.live_rounds() == 0 {
                break;
            }
            let ev = pull(&mut g, &[]);
            if shot(&ev) {
                last = ev;
            } else {
                cycle(&mut g);
            }
        }
        let mut moment: Vec<_> = last
            .into_iter()
            .filter(|e| *e == FireEvent::BoltLocked)
            .collect();
        if crossbow {
            // Spanned again, nothing on the rail: the latch's click, and
            // the empty groove to see.
            moment = cycle(&mut g);
        } else if moment.is_empty() {
            if def.action != SemiAuto {
                cycle(&mut g);
            }
            moment = pull(&mut g, &[]);
        }
        add(
            "empty",
            g,
            moment,
            ("reload", reload),
            vec![("work the action", cycle)],
        );
    }
    if def.modes.contains(&FireMode::Safe) {
        let mut g = loaded(def, Load::Standard);
        while g.fire_mode() != FireMode::Safe {
            g.cycle_mode();
        }
        let moment = pull(&mut g, &[]);
        add(
            "on safe",
            g,
            moment,
            ("thumb the selector off safe", |g| {
                let mut ev = Vec::new();
                while g.fire_mode() == FireMode::Safe {
                    ev.extend(g.cycle_mode());
                }
                ev
            }),
            vec![("pull harder", |g| pull(g, &[]))],
        );
    }
    if def.action == Revolver {
        // The gate left open: the cylinder won't turn under the hammer.
        let mut g = loaded(def, Load::Standard);
        g.set_gate(true);
        let moment = pull(&mut g, &[]);
        add(
            "gate open",
            g,
            moment,
            ("shut the gate", |g| g.set_gate(false)),
            vec![("pull harder", |g| pull(g, &[]))],
        );
    }
    if crossbow {
        // The string let go short of the latch: it slaps home, uncocked.
        let mut g = Firearm::new(def.clone(), 1.0);
        while g.fire_mode() == FireMode::Safe {
            g.cycle_mode();
        }
        g.set_bolt_travel(0.6);
        let moment = g.release_bolt();
        let short: Fix = |g| {
            g.set_bolt_travel(0.6);
            g.release_bolt()
        };
        add(
            "string off the latch",
            g,
            moment,
            (
                "draw it right back onto the latch, a bolt on the rail",
                |g| {
                    let mut ev = g.set_bolt_travel(1.0);
                    ev.extend(g.release_bolt());
                    g.load_round_of(Load::Standard);
                    ev
                },
            ),
            vec![("draw it short again", short), ("pull", |g| pull(g, &[]))],
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Catalog;

    /// What the ear gets of a moment (the shot itself aside: a stoppage's
    /// tell is what comes with it): each sample set and its speed.
    fn heard(action: ActionKind, id: &str, ev: &[FireEvent]) -> Vec<String> {
        let mut out = Vec::new();
        for e in ev {
            for (set, _, speed, cut) in crate::audio::fault_layers(e) {
                out.push(format!("{set} x{speed:.2} {cut:?}"));
            }
            if crate::audio::fault_layers(e).is_empty() {
                if let Some((set, _)) = crate::audio::handling_sound(e, Some(action), id) {
                    let speed = crate::audio::handling_speed(e, Some(action));
                    out.push(format!("{set} x{speed:.2}"));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// What the hand gets of a moment (the kick aside).
    fn felt(ev: &[FireEvent]) -> Vec<String> {
        let mut out: Vec<_> = ev
            .iter()
            .filter_map(|e| {
                crate::rig::haptics::handling_feel(e)
                    .map(|f| format!("{f:?}"))
                    .or_else(|| super::super::faults::fault_feel_of(e).map(|f| format!("{f:?}")))
            })
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Every gun, every way it can stop: each stoppage is heard, felt and
    /// seen as itself the moment it happens, no two of a gun's alike; it
    /// stops the gun; its own clear (a hand's doing) puts the gun back in
    /// action; and each wrong fix leaves it stopped and shows why (a tell
    /// of its own, or the stoppage still there to see).
    #[test]
    fn every_stoppage_tells_you_its_fix() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let id = def.id.as_str();
            let ready = look(id, &loaded(f, Load::Standard));
            assert!(fires(&loaded(f, Load::Standard)), "{id}: loaded, it fires");
            let all = stoppages(id, f);
            let tells: Vec<_> = all
                .iter()
                .map(|s| {
                    let flash = s
                        .moment
                        .iter()
                        .any(|e| matches!(e, FireEvent::Fired { .. }));
                    (
                        heard(f.action, id, &s.moment),
                        felt(&s.moment),
                        (s.look.clone(), flash),
                    )
                })
                .collect();
            for (s, (h, fl, _)) in all.iter().zip(&tells) {
                let n = s.name;
                if h.is_empty() {
                    bad.push(format!("{id} {n}: unheard ({:?})", s.moment));
                }
                if fl.is_empty() {
                    bad.push(format!("{id} {n}: unfelt ({:?})", s.moment));
                }
                if s.stops && fires(&s.gun) {
                    bad.push(format!("{id} {n}: it didn't stop the gun"));
                }
                let mut g = s.gun.clone();
                let ev = (s.clear.1)(&mut g);
                if !shot(&ev) && !fires(&g) {
                    bad.push(format!("{id} {n}: \"{}\" didn't clear it", s.clear.0));
                }
                for (w, fix) in &s.wrong {
                    let mut g = s.gun.clone();
                    let ev = fix(&mut g);
                    // A wrong fix that wrecks the gun clearing it (a bulge,
                    // a case burst open) is no clear.
                    let harmed = ev
                        .iter()
                        .any(|e| matches!(e, FireEvent::Bulged { .. } | FireEvent::OutOfBattery));
                    if !harmed && (shot(&ev) || fires(&g)) {
                        bad.push(format!("{id} {n}: the wrong fix \"{w}\" cleared it"));
                    }
                    let tells = !heard(f.action, id, &ev).is_empty() || !felt(&ev).is_empty();
                    if !tells && look(id, &g) == ready {
                        bad.push(format!("{id} {n}: \"{w}\" doesn't show why it failed"));
                    }
                }
                if s.wrong.is_empty() && s.stops {
                    bad.push(format!("{id} {n}: no wrong fix to try"));
                }
            }
            for (i, a) in all.iter().enumerate() {
                for (j, b) in all.iter().enumerate().skip(i + 1) {
                    let (ta, tb) = (&tells[i], &tells[j]);
                    let (an, bn) = (a.name, b.name);
                    if ta.0 == tb.0 {
                        bad.push(format!("{id}: {an} and {bn} sound alike {:?}", ta.0));
                    }
                    if ta.1 == tb.1 {
                        bad.push(format!("{id}: {an} and {bn} feel alike {:?}", ta.1));
                    }
                    if ta.2 == tb.2 {
                        bad.push(format!("{id}: {an} and {bn} look alike {:?}", ta.2));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every pull on a stopped gun says why, every pull: nobody squeezes a
    /// dead gun at a walker without knowing it's dead (the first playtest's
    /// first death: a pistol run dry, a fresh magazine in, the slide still
    /// back, and the trigger answered nothing).
    #[test]
    fn every_pull_on_a_stopped_gun_says_why() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let id = def.id.as_str();
            for s in stoppages(id, f) {
                // (A hang-fire goes off in its own time; hold on.)
                if !s.stops || s.name == "hang-fire" {
                    continue;
                }
                let mut g = s.gun.clone();
                for k in 1..=3 {
                    let ev = pull(&mut g, &[]);
                    if shot(&ev) {
                        bad.push(format!("{id} {}: pull {k} fired", s.name));
                        break;
                    }
                    // A double-action revolver fires into its squib: the
                    // bulge says it, and the gun's done saying anything.
                    if ev
                        .iter()
                        .any(|e| matches!(e, FireEvent::Bulged { .. } | FireEvent::OutOfBattery))
                    {
                        break;
                    }
                    if heard(f.action, id, &ev).is_empty() || felt(&ev).is_empty() {
                        bad.push(format!("{id} {}: pull {k} said nothing ({ev:?})", s.name));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// How often a gun stops is its condition and dirt: a worn, filthy one
    /// jams and sticks far more than one fresh off the bench.
    #[test]
    fn worn_and_dirty_guns_stop_more() {
        let catalog = Catalog::load();
        for id in ["ak74", "mosin"] {
            let (_, item) = catalog.iter().find(|(_, d)| d.id == id).unwrap();
            let def = item.firearm.clone().unwrap();
            let mut fresh = Firearm::new(def.clone(), 1.0);
            let mut worn = Firearm::new(def, 0.3);
            fresh.fouling = 0.0;
            worn.fouling = 0.9;
            assert!(
                worn.malfunction_chance() > 20.0 * fresh.malfunction_chance(),
                "{id}"
            );
            assert!(worn.sticky_chance() > fresh.sticky_chance(), "{id}");
        }
    }
}

#[cfg(test)]
mod vr {
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;
    use extraction_core::firearm::FireMode;

    fn said(r: &Range, text: &str) -> bool {
        r.vr.app
            .world()
            .resource::<crate::raid::Raid>()
            .log
            .iter()
            .any(|(t, _)| t.contains(text))
    }

    fn mark(r: &mut Range, down: bool) {
        let p = r.vr.player;
        r.vr.app
            .world_mut()
            .get_mut::<crate::rig::RigInput>(p)
            .unwrap()
            .mark = down;
    }

    /// The selector on the gun hand's stick click acts as the click lets
    /// go; both sticks clicked for a playtest mark (the gun hand's landing
    /// a frame first) leaves the safety alone.
    #[test]
    fn a_mark_with_both_sticks_leaves_the_safety_alone() {
        let mut r = hold("pm_pistol", false, false);
        let safe = |r: &mut Range| gun_mut(r).firearm.fire_mode() == FireMode::Safe;
        assert!(safe(&mut r), "it comes on safe");
        // The gun hand's stick a frame early, then both: a mark.
        r.vr.stick_click(Side::Right, true);
        r.vr.step(1);
        mark(&mut r, true);
        r.vr.step(3);
        r.vr.stick_click(Side::Right, false);
        mark(&mut r, false);
        r.vr.step(3);
        assert!(safe(&mut r), "a mark doesn't flip the safety");
        // A click on its own: off safe as it lets go.
        r.vr.stick_click(Side::Right, true);
        r.vr.step(2);
        assert!(safe(&mut r), "not yet: on the release");
        r.vr.stick_click(Side::Right, false);
        r.vr.step(2);
        assert!(!safe(&mut r), "clicked off safe");
    }

    /// The first playtest's first death, played through: the PM run dry
    /// at a walker, the empty magazine dropped and a full one seated, the
    /// slide still back. The trigger fires nothing, and says so: the open
    /// action's dry tick and give in the hand, and the wrist's line. B
    /// lets the slide go, and it fires.
    #[test]
    fn a_pistol_run_dry_and_reloaded_tells_you_the_slide_is_back() {
        run_dry_and_reload("pm_pistol", "pm_mag", 8);
    }

    /// The same for every gun that holds open on a detachable magazine
    /// (the SVD), so the cue is the class's, not the pistol's.
    #[test]
    fn every_hold_open_gun_run_dry_tells_you_the_slide_is_back() {
        let catalog = crate::items::Catalog::load();
        let mut n = 0;
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let Some(fam) = f.magazine_family.as_ref() else {
                continue;
            };
            if !f.last_round_hold_open || def.id == "pm_pistol" {
                continue;
            }
            let (_, mag) = catalog
                .iter()
                .find(|(_, d)| {
                    d.magazine
                        .as_ref()
                        .is_some_and(|m| &m.fits == fam && !m.stripper)
                })
                .expect("a magazine for it");
            let cap = mag.magazine.as_ref().unwrap().capacity;
            run_dry_and_reload(&def.id, &mag.id, cap);
            n += 1;
        }
        assert!(n > 0, "the SVD at least");
    }

    fn run_dry_and_reload(gun: &str, mag: &str, full: u32) {
        let mut r = hold(gun, false, false);
        seat_mag(&mut r, mag, 1);
        rack(&mut r);
        {
            let mut g = gun_mut(&mut r);
            while g.firearm.fire_mode() != FireMode::Semi {
                g.firearm.cycle_mode();
            }
        }
        assert_eq!(fire(&mut r, 1, true).len(), 1, "{gun}: the last round");
        assert!(gun_mut(&mut r).firearm.locked_back, "{gun}: held open");
        r.vr.primary(Side::Right, true);
        r.vr.step(3);
        r.vr.primary(Side::Right, false);
        r.vr.step(10);
        assert!(gun_mut(&mut r).firearm.magazine.is_none(), "{gun}: dropped");
        seat_mag(&mut r, mag, full);
        assert!(
            fire(&mut r, 1, true).is_empty(),
            "{gun}: the slide's back: nothing"
        );
        assert!(
            said(&r, "Slide's back"),
            "{gun}: the wrist says what lets it go"
        );
        r.vr.secondary(Side::Right, true);
        r.vr.step(3);
        r.vr.secondary(Side::Right, false);
        r.vr.step(5);
        assert_eq!(fire(&mut r, 1, true).len(), 1, "{gun}: let go, it fires");
    }
}
