//! What your hands feel. Game events become [`Haptic`] pulses on a side;
//! the headset driver plays them on the controllers (flat mode and bots
//! ignore them).
//!
//! One scale for everything, so a pulse means the same thing wherever it
//! comes from (`docs/AUDIO.md`, "Haptics"):
//!
//! - a tick (0.1-0.15) is "this is here": a hand in reach of something, a
//!   button coming back out, letting go;
//! - a click (0.25-0.35) is "that happened": a button pressed, a thing
//!   seated in a pouch or holster, the action worked, a far grab caught;
//! - a knock (0.4-0.6) is "that took force": a magazine seating, a heavy
//!   thing taken up, a refusal;
//! - a kick (0.6-1.0) is a shot or a hit, and only those.
//!
//! Taking something up is felt by its weight: a tin is a tick, a rifle a
//! knock. Pulses on one controller in the same frame merge into one (the
//! runtime plays only the last it was given), so a shot isn't cut short by
//! its own casing.
//!
//! Index controllers barely register a pulse under about 0.1 or shorter
//! than 10 ms, so nothing here goes below either.

use bevy::prelude::*;
use extraction_core::firearm::Event as FireEvent;
use std::collections::HashMap;

use super::{LocalPlayer, RigHand, Side};
use crate::interact::{Hand, Held, InteractEvent, Supported};
use crate::weapons::GunEvent;

/// A vibration on one controller.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Haptic {
    pub side: Side,
    /// 0..1.
    pub amplitude: f32,
    pub seconds: f32,
}

/// A step of something being taught or checked off is done (the
/// onboarding's "that's it, next"): a soft chime and a tick in both hands.
/// Write it from game logic; it does nothing headless.
#[derive(Message, Clone, Copy, Debug)]
pub struct StepDone {
    /// Where the chime comes from; `None` plays it in your head (not
    /// placed), which is what a confirmation usually wants.
    pub at: Option<Vec3>,
    /// It finished a whole part (a training station): a fuller chime and
    /// a longer click.
    pub station: bool,
}

/// Write a [`StepDone`] (for systems that would rather not hold a writer).
pub fn step_done(commands: &mut Commands, at: Option<Vec3>) {
    commands.write_message(StepDone { at, station: false });
}

pub struct HapticsPlugin;

impl Plugin for HapticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Haptic>()
            .add_message::<StepDone>()
            .add_systems(
                Update,
                (
                    gun_haptics,
                    hand_haptics,
                    button_haptics,
                    hurt_haptics,
                    step_haptics,
                    grenade_haptics,
                    machine_tick,
                    zone_haptics,
                ),
            );
    }
}

const TICK: (f32, f32) = (0.12, 0.012);
const CLICK: (f32, f32) = (0.3, 0.025);

fn pulse(out: &mut MessageWriter<Haptic>, side: Side, (amplitude, seconds): (f32, f32)) {
    out.write(Haptic {
        side,
        amplitude,
        seconds,
    });
}

/// Taking something up, by its weight (kg): a 50 g tin is a tick, a
/// 1 kg pistol a click, a 3.5 kg rifle a knock.
pub fn grab_pulse(mass: f32) -> (f32, f32) {
    let m = mass.max(0.0);
    (
        (0.12 + 0.12 * (1.0 + m).log2()).clamp(0.12, 0.55),
        (0.012 + 0.008 * m).clamp(0.012, 0.045),
    )
}

/// Which hands hold a gun: (the one on the grip, the one supporting it).
fn hands_on(
    gun: Entity,
    held: &Query<(&Held, Option<&Supported>)>,
    locals: &Query<(), With<LocalPlayer>>,
) -> Option<(Side, Option<Side>)> {
    let (h, s) = held.get(gun).ok()?;
    locals
        .contains(h.player)
        .then_some((h.side, s.map(|s| s.side)))
}

/// A shot's kick, by the gun's recoil (`FirearmDef::recoil`: a pistol
/// 1, an AK 2.2, a Mosin 4.5) and how it's held: (grip hand, seconds,
/// support hand). Always in the kick band; a shouldered gun puts some of
/// it into the shoulder instead of the hands.
pub fn shot_kick(recoil: f32, shouldered: bool) -> (f32, f32, f32) {
    let a = 0.6 + 0.4 * (recoil / 5.0).clamp(0.0, 1.0);
    let a = if shouldered { (a * 0.85).max(0.6) } else { a };
    (a, (0.05 + 0.006 * recoil).clamp(0.05, 0.08), a * 0.6)
}

/// How a gun's handling is felt (the shot aside): a shape, or a plain
/// pulse (amplitude, seconds, and the supporting hand's amplitude). Each
/// step of a reload feels unlike the others (`weapons::reload`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandlingFeel {
    Shape(super::shapes::Shape),
    Pulse(f32, f32, f32),
}

pub fn handling_feel(event: &FireEvent) -> Option<HandlingFeel> {
    use super::shapes::{BOLT_CATCH, BOLT_HOME, CHAMBER, MAG_HALF, MAG_SEAT};
    use HandlingFeel::*;
    Some(match event {
        FireEvent::MagazineInserted => Shape(MAG_SEAT),
        FireEvent::MagazineHalfSeated => Shape(MAG_HALF),
        FireEvent::BoltClosed => Shape(BOLT_HOME),
        FireEvent::BoltLocked => Shape(BOLT_CATCH),
        FireEvent::Chambered | FireEvent::RoundLoaded => Shape(CHAMBER),
        FireEvent::DryFire => Pulse(0.35, 0.02, 0.0),
        // The hammer stopped hard by a primer that didn't go: firmer than
        // falling on nothing.
        FireEvent::Dud => Pulse(0.5, 0.03, 0.0),
        // The finger meeting the safety, or a Nagant's open gate: a soft
        // dead stop.
        FireEvent::TriggerBlocked => Pulse(0.15, 0.06, 0.0),
        // The hammer already down: the trigger goes back slack, no break.
        FireEvent::TriggerDead => Pulse(0.1, 0.012, 0.0),
        FireEvent::MagazineRemoved => Pulse(0.3, 0.03, 0.0),
        // The magazine's weight going out of the well all at once.
        FireEvent::MagazineDropped => Pulse(0.45, 0.05, 0.2),
        FireEvent::Cocked => Pulse(0.25, 0.02, 0.0),
        // The safety on: a firm click; off: a lighter one.
        FireEvent::ModeChanged(extraction_core::firearm::FireMode::Safe) => Pulse(0.35, 0.02, 0.0),
        FireEvent::ModeChanged(_) => Pulse(0.2, 0.015, 0.0),
        // The trigger back on an open action: a hollow give and stop.
        FireEvent::ActionOpen => Pulse(0.25, 0.04, 0.0),
        _ => return None,
    })
}

fn gun_haptics(
    mut events: MessageReader<GunEvent>,
    held: Query<(&Held, Option<&Supported>)>,
    guns: Query<(
        &crate::weapons::Gun,
        Option<&crate::weapons::feel::Handling>,
    )>,
    locals: Query<(), With<LocalPlayer>>,
    mut out: MessageWriter<Haptic>,
    mut shapes: MessageWriter<super::shapes::HapticShape>,
) {
    use super::shapes::HapticShape;
    for ev in events.read() {
        let Some((grip, support)) = hands_on(ev.gun, &held, &locals) else {
            continue;
        };
        let (a, t, support_a) = match handling_feel(&ev.event) {
            // The moments of loading have shapes of their own
            // (`rig::shapes`), the supporting hand feeling them fainter.
            Some(HandlingFeel::Shape(shape)) => {
                shapes.write(HapticShape {
                    side: grip,
                    shape,
                    scale: 1.0,
                });
                if let Some(s) = support {
                    shapes.write(HapticShape {
                        side: s,
                        shape,
                        scale: 0.5,
                    });
                }
                continue;
            }
            Some(HandlingFeel::Pulse(a, t, support_a)) => (a, t, support_a),
            None => match ev.event {
                FireEvent::Fired { .. } => guns.get(ev.gun).map_or((1.0, 0.07, 0.6), |(g, h)| {
                    shot_kick(g.firearm.def.recoil, h.is_some_and(|h| h.hold.shouldered))
                }),
                _ => continue,
            },
        };
        out.write(Haptic {
            side: grip,
            amplitude: a,
            seconds: t,
        });
        if let Some(s) = support.filter(|_| support_a > 0.0) {
            out.write(Haptic {
                side: s,
                amplitude: support_a,
                seconds: t,
            });
        }
    }
}

/// A grenade in your hand: the pin coming out is a click in the hand on
/// the ring (the one not holding it), the spoon flying a tick in the
/// hand that held it down. The bang is felt as a hit, if it hurts you.
pub fn grenade_pulse(event: extraction_core::grenade::Event) -> Option<(bool, (f32, f32))> {
    use extraction_core::grenade::Event as E;
    match event {
        E::PinPulled => Some((true, CLICK)),
        E::SpoonOff => Some((false, (0.15, 0.015))),
        E::Exploded => None,
    }
}

fn grenade_haptics(
    mut events: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    held: Query<(&Held, Option<&Supported>)>,
    locals: Query<(), With<LocalPlayer>>,
    mut out: MessageWriter<Haptic>,
) {
    for ev in events.read() {
        let Some((ring, p)) = grenade_pulse(ev.event) else {
            continue;
        };
        let Some((grip, _)) = hands_on(ev.grenade, &held, &locals) else {
            continue;
        };
        pulse(&mut out, if ring { grip.other() } else { grip }, p);
    }
}

/// How near a machine turning certain of you is felt, m.
pub const MACHINE_TICK_NEAR: f32 = 25.0;
/// The tick: under a tick's usual strength, a whisper on the wrist.
pub const MACHINE_TICK: (f32, f32) = (0.1, 0.012);

/// A machine nearby turns certain (`robot_voice::Spoke` engaging): a
/// faint tick on the watch wrist (the off hand: `Comfort::gun_side`'s
/// other). Off with the comfort setting `machine_tick`.
fn machine_tick(
    mut spoke: MessageReader<crate::robot_voice::Spoke>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut out: MessageWriter<Haptic>,
) {
    let on = comfort.as_deref().is_none_or(|c| c.machine_tick);
    let wrist = comfort
        .as_deref()
        .map_or(Side::Left, |c| c.gun_side().other());
    let Some(me) = players.iter().next().map(|g| g.translation()) else {
        spoke.clear();
        return;
    };
    let certain = spoke.read().any(|s| {
        s.said == crate::robot_voice::Utterance::Engaging && s.at.distance(me) < MACHINE_TICK_NEAR
    });
    if on && certain {
        pulse(&mut out, wrist, MACHINE_TICK);
    }
}

/// The Zone biting: a wet aegis's shock is a kick in both hands
/// (`anomaly::costs::ArtifactBite`); a hot stone held is a tick in the
/// hand holding it every half second (`anomaly::harvest::Burned`).
pub const SHOCK: (f32, f32) = (0.7, 0.06);
pub const BURN_EVERY: f32 = 0.5;

fn zone_haptics(
    mut bites: MessageReader<crate::anomaly::costs::ArtifactBite>,
    mut burns: MessageReader<crate::anomaly::harvest::Burned>,
    locals: Query<(), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand, &ChildOf)>,
    time: Res<Time>,
    mut next_burn: Local<f32>,
    mut out: MessageWriter<Haptic>,
) {
    for b in bites.read() {
        if b.what == crate::anomaly::costs::Bite::Shock && locals.contains(b.player) {
            for side in Side::BOTH {
                pulse(&mut out, side, SHOCK);
            }
        }
    }
    let now = time.elapsed_secs();
    for b in burns.read() {
        if !locals.contains(b.player) || now < *next_burn {
            continue;
        }
        *next_burn = now + BURN_EVERY;
        if let Some(side) = nearest_hand(&hands, b.player, b.at) {
            pulse(&mut out, side, TICK);
        }
    }
}

/// The local hand nearest a point (for presses, which don't say which
/// hand did it).
fn nearest_hand(
    hands: &Query<(&RigHand, &Hand, &ChildOf)>,
    player: Entity,
    at: Vec3,
) -> Option<Side> {
    hands
        .iter()
        .filter(|(_, _, p)| p.parent() == player)
        .min_by(|a, b| {
            a.1.world
                .translation
                .distance(at)
                .total_cmp(&b.1.world.translation.distance(at))
        })
        .map(|(rh, ..)| rh.side)
}

#[allow(clippy::too_many_arguments)]
fn hand_haptics(
    mut events: MessageReader<InteractEvent>,
    held: Query<&Held>,
    items: Query<&crate::items::Item>,
    catalog: Option<Res<crate::items::Catalog>>,
    locals: Query<(), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand, &ChildOf)>,
    poses: Query<&GlobalTransform>,
    buttons: Query<&crate::interact::poke::PokeButton>,
    time: Res<Time>,
    // What each local hand holds (a socketed or stowed item has left the
    // hand by the time its event is read), and what's on its way to one.
    mut in_hand: Local<HashMap<Entity, Side>>,
    mut pulled: Local<HashMap<Entity, f32>>,
    mut out: MessageWriter<Haptic>,
) {
    let now = time.elapsed_secs();
    pulled.retain(|_, t| now - *t < 2.0);
    // Food eaten from the hand is gone without a Released.
    in_hand.retain(|e, _| items.contains(*e));
    let mass = |item: Entity| {
        items
            .get(item)
            .ok()
            .zip(catalog.as_ref())
            .map_or(0.3, |(i, c)| c.get(i.kind()).mass)
    };
    for ev in events.read() {
        match ev {
            InteractEvent::Grabbed { item, side } => {
                let mine = held.get(*item).map_or(true, |h| locals.contains(h.player));
                if !mine {
                    continue;
                }
                in_hand.insert(*item, *side);
                let (a, t) = grab_pulse(mass(*item));
                // Caught out of the air: it lands in the palm.
                let caught = pulled.remove(item).is_some();
                let a = if caught { (a + 0.15).min(0.6) } else { a };
                pulse(&mut out, *side, (a, t));
            }
            InteractEvent::Hover { side, .. } => pulse(&mut out, *side, TICK),
            InteractEvent::Pulled { item, side } => {
                pulled.insert(*item, now);
                pulse(&mut out, *side, (0.3, 0.04));
            }
            InteractEvent::Released { item, side, .. } => {
                if in_hand.remove(item).is_some() {
                    pulse(&mut out, *side, (0.1, 0.012));
                }
            }
            InteractEvent::Socketed { item, .. } => {
                if let Some(side) = in_hand.remove(item) {
                    pulse(&mut out, side, CLICK);
                }
            }
            InteractEvent::Stowed { item } => {
                if let Some(side) = in_hand.remove(item) {
                    pulse(&mut out, side, (0.25, 0.02));
                }
            }
            // A push button clicks the finger that pushed it
            // (`button_haptics`); anything pressed with the grip (the bell
            // cord, a door, the bench) clicks the hand nearest it.
            InteractEvent::Pressed { item, player } if locals.contains(*player) => {
                if buttons.iter().any(|b| b.target == *item && b.down) {
                    continue;
                }
                let at = poses.get(*item).map(|g| g.translation()).ok();
                match at.and_then(|at| nearest_hand(&hands, *player, at)) {
                    Some(side) => pulse(&mut out, side, CLICK),
                    None => {
                        for side in [Side::Left, Side::Right] {
                            pulse(&mut out, side, CLICK);
                        }
                    }
                }
            }
            InteractEvent::Refused { item } => {
                if let Ok(h) = held.get(*item) {
                    if locals.contains(h.player) {
                        pulse(&mut out, h.side, (0.4, 0.05));
                    }
                }
            }
            _ => {}
        }
    }
}

/// Push buttons: a click in the finger as it goes home, a lighter tick as
/// it comes back out.
fn button_haptics(
    buttons: Query<(Entity, &crate::interact::poke::PokeButton, &GlobalTransform)>,
    hands: Query<(&RigHand, &Hand, &ChildOf)>,
    locals: Query<Entity, With<LocalPlayer>>,
    mut was: Local<HashMap<Entity, bool>>,
    mut out: MessageWriter<Haptic>,
) {
    let Some(player) = locals.iter().next() else {
        return;
    };
    for (e, b, gt) in &buttons {
        let before = was.insert(e, b.down).unwrap_or(false);
        if before == b.down {
            continue;
        }
        let tip = crate::interact::poke::FINGERTIP;
        let side = hands
            .iter()
            .filter(|(_, _, p)| p.parent() == player)
            .min_by(|a, b2| {
                let d = |h: &Hand| h.world.transform_point(tip).distance(gt.translation());
                d(a.1).total_cmp(&d(b2.1))
            })
            .map(|(rh, ..)| rh.side);
        if let Some(side) = side {
            pulse(&mut out, side, if b.down { CLICK } else { TICK });
        }
    }
}

/// Any harm to the local player shakes both hands, harder for more: a
/// bullet, a fall, an anomaly, the Surge. Steady harm (bleeding, standing
/// in a burner) pulses no faster than four times a second.
fn hurt_haptics(
    players: Query<&crate::health::Health, With<LocalPlayer>>,
    time: Res<Time>,
    mut last: Local<Option<(f32, f32)>>,
    mut owed: Local<f32>,
    mut quiet: Local<f32>,
    mut out: MessageWriter<Haptic>,
) {
    let Some(h) = players.iter().next() else {
        return;
    };
    *quiet -= time.delta_secs();
    let (hp, since) = (h.hp, h.since_hit);
    if let Some((was_hp, was_since)) = *last {
        // A new hit restarts `since_hit` (steady harm keeps it near zero);
        // bleeding only drains.
        if since < was_since || (since < 0.1 && hp < was_hp) {
            *owed += (was_hp - hp).max(0.5);
        }
    }
    *last = Some((hp, since));
    if *owed > 0.0 && *quiet <= 0.0 {
        let a = hurt_amplitude(*owed);
        for side in [Side::Left, Side::Right] {
            pulse(&mut out, side, (a, 0.1));
        }
        *owed = 0.0;
        *quiet = 0.25;
    }
}

/// How hard a hit of `damage` hp shakes the hands.
pub fn hurt_amplitude(damage: f32) -> f32 {
    (0.45 + damage / 40.0).min(1.0)
}

fn step_haptics(mut steps: MessageReader<StepDone>, mut out: MessageWriter<Haptic>) {
    for s in steps.read() {
        let feel = if s.station { (0.35, 0.1) } else { (0.25, 0.04) };
        for side in [Side::Left, Side::Right] {
            pulse(&mut out, side, feel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grenade's pin is a click on the ring hand, the spoon a tick on
    /// the grip; the bang isn't a pulse of its own.
    #[test]
    fn a_grenade_is_felt_in_the_right_hand() {
        use extraction_core::grenade::Event as E;
        assert_eq!(grenade_pulse(E::PinPulled), Some((true, CLICK)));
        let (ring, (a, _)) = grenade_pulse(E::SpoonOff).unwrap();
        assert!(!ring && (0.1..=0.15).contains(&a));
        assert_eq!(grenade_pulse(E::Exploded), None);
    }

    #[derive(Resource, Default)]
    struct Felt(Vec<Haptic>);

    fn feel(mut pulses: MessageReader<Haptic>, mut felt: ResMut<Felt>) {
        felt.0.extend(pulses.read().copied());
    }

    #[test]
    fn a_shot_kicks_the_trigger_hand_hardest() {
        let mut app = crate::headless_app(Default::default(), false);
        app.init_resource::<Felt>().add_systems(Last, feel);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let gun = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(gun).insert((
            Held {
                player,
                hand: player,
                side: Side::Right,
                point: 0,
                offset: Transform::IDENTITY,
            },
            Supported {
                hand: player,
                side: Side::Left,
                point: 1,
            },
        ));
        app.world_mut().write_message(GunEvent {
            gun,
            item: "ak74".into(),
            event: FireEvent::Fired { barrel: 0 },
            at: Vec3::ZERO,
        });
        app.update();
        let felt = &app.world().resource::<Felt>().0;
        let right = felt
            .iter()
            .find(|h| h.side == Side::Right)
            .expect("right hand");
        let left = felt
            .iter()
            .find(|h| h.side == Side::Left)
            .expect("left hand");
        assert!(right.amplitude > left.amplitude);
    }

    /// Harm that isn't a bullet (a fall, a burner) is felt too, and steady
    /// harm pulses no faster than four times a second.
    #[test]
    fn every_kind_of_harm_is_felt() {
        let mut app = crate::headless_app(Default::default(), false);
        app.init_resource::<Felt>().add_systems(Last, feel);
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
        app.world_mut().resource_mut::<Felt>().0.clear();
        // Thirty frames of a burner: a little harm every frame.
        for _ in 0..30 {
            if let Some(mut h) = app.world_mut().get_mut::<crate::health::Health>(player) {
                h.hp -= 0.5;
                h.since_hit = 0.0;
            }
            app.update();
        }
        let felt = &app.world().resource::<Felt>().0;
        let left = felt.iter().filter(|h| h.side == Side::Left).count();
        assert!(left >= 1, "felt: {felt:?}");
        assert!(left <= 4, "no faster than 4 Hz over 0.5 s: {left}");
    }

    /// The scale holds: heavier is stronger, nothing an Index can't feel,
    /// nothing but a shot or a hit at full strength.
    #[test]
    fn weight_grades_the_grab() {
        let tin = grab_pulse(0.05);
        let pistol = grab_pulse(0.8);
        let rifle = grab_pulse(3.5);
        assert!(tin.0 < pistol.0 && pistol.0 < rifle.0);
        for (a, t) in [tin, pistol, rifle, grab_pulse(40.0), TICK, CLICK] {
            assert!((0.1..0.6).contains(&a), "{a}");
            assert!(t >= 0.01, "{t}");
        }
        assert!(hurt_amplitude(1.0) > CLICK.0);
        // A shot: harder for more recoil, always a kick, never over 1.
        let (pistol, rifle) = (shot_kick(1.0, false), shot_kick(4.5, false));
        assert!(pistol.0 < rifle.0 && pistol.1 <= rifle.1);
        for k in [pistol, rifle, shot_kick(9.0, false), shot_kick(4.5, true)] {
            assert!((0.6..=1.0).contains(&k.0) && k.2 < k.0, "{k:?}");
        }
    }

    fn felt_by(vr: &mut crate::vr_harness::Vr) -> Vec<Haptic> {
        std::mem::take(&mut vr.app.world_mut().resource_mut::<Felt>().0)
    }

    /// Through the headset path: a rifle taken up is felt harder than a
    /// magazine, in the hand that took it.
    #[test]
    fn a_rifle_weighs_more_in_the_hand_than_a_magazine() {
        use crate::vr_harness::Vr;
        let mut vr = Vr::new("training_yard");
        vr.app.init_resource::<Felt>().add_systems(Last, feel);
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        let grab = |vr: &mut Vr, id: &str, dx: f32| {
            let at = feet + Vec3::new(dx, 1.0, -0.4);
            let item = vr.spawn(id, Transform::from_translation(at));
            vr.app
                .world_mut()
                .entity_mut(item)
                .insert(avian3d::prelude::RigidBody::Static);
            let point = vr.point(item, |r| {
                use crate::interact::GripRole::*;
                matches!(r, Primary | Free)
            });
            vr.hand(Side::Right, point, Quat::IDENTITY, 0.0, 0.0);
            vr.step(3);
            felt_by(vr);
            vr.grip(Side::Right, 1.0);
            vr.step(3);
            assert_eq!(vr.hand_state(Side::Right).holding, Some(item), "{id} taken");
            let felt = felt_by(vr);
            vr.grip(Side::Right, 0.0);
            vr.step(3);
            assert!(felt.iter().all(|h| h.side == Side::Right), "{felt:?}");
            felt.iter().map(|h| h.amplitude).fold(0.0, f32::max)
        };
        let mag = grab(&mut vr, "pm_mag", 0.2);
        let rifle = grab(&mut vr, "ak74", -0.3);
        assert!(rifle > mag + 0.1, "rifle {rifle} vs magazine {mag}");
    }

    /// A push button clicks the finger that pushed it, not both hands, and
    /// ticks as it comes back out. (In a valley: in the training yard the
    /// first button is also a lesson, and a lesson done is felt in both
    /// hands.)
    #[test]
    fn a_button_clicks_the_finger_that_pushed_it() {
        use crate::interact::poke::{CAP, FINGERTIP, PokeButton};
        use crate::vr_harness::Vr;
        let mut vr = Vr::new("marrow_creek");
        vr.app.init_resource::<Felt>().add_systems(Last, feel);
        // Loose things (a cassette player on a tower's floor) are still
        // settling for the first frames: aim at the world at rest.
        vr.step(90);
        // A card's button (fixed in place, not on a loose radio), the same
        // one every time: which a query meets first follows spawn order.
        let button = *vr
            .app
            .world_mut()
            .query::<(&PokeButton, &GlobalTransform, &Name)>()
            .iter(vr.app.world())
            .filter(|(_, _, n)| n.as_str() == "Button")
            .map(|(_, gt, _)| gt)
            .min_by(|a, b| a.translation().x.total_cmp(&b.translation().x))
            .expect("a card's button");
        let normal = button.rotation() * Vec3::Z;
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, -normal);
        let tip_at = |d: f32| button.translation() + normal * d - rot * FINGERTIP;
        vr.hand(Side::Right, tip_at(0.08), rot, 0.0, 0.0);
        vr.step(2);
        for k in 0..10 {
            vr.move_hand(Side::Right, tip_at(0.08 - k as f32 * 0.006));
            vr.step(1);
        }
        felt_by(&mut vr);
        for k in 0..6 {
            vr.move_hand(Side::Right, tip_at(CAP - k as f32 * 0.003));
            vr.step(1);
        }
        vr.step(2);
        let pressed = felt_by(&mut vr);
        assert!(
            pressed
                .iter()
                .any(|h| h.side == Side::Right && h.amplitude >= CLICK.0),
            "a click in the right hand: {pressed:?}"
        );
        assert!(
            pressed.iter().all(|h| h.side == Side::Right),
            "the left hand did nothing: {pressed:?}"
        );
        vr.move_hand(Side::Right, tip_at(0.08));
        vr.step(3);
        let out = felt_by(&mut vr);
        assert!(
            out.iter()
                .any(|h| h.side == Side::Right && h.amplitude < CLICK.0),
            "a tick coming out: {out:?}"
        );
    }
}
