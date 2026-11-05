//! Every hand-worked mechanism, by hand, against what it's meant to do:
//! a flick past the end, a steady pull at the speed a hand works it, and a
//! stroke let go short. The intent per kind is in
//! `docs/shots/weapon-feel/README.md` ("Hand-worked mechanisms").

use super::*;
use crate::interact::GripRole;
use crate::rig::Side;
use crate::weapons::feel::vr_tests::*;
use extraction_core::firearm::{Magazine, Round};

/// How a hand works it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stroke {
    /// Past the end in three ticks.
    Flick,
    /// Past the end over half a second.
    Steady,
    /// Half way, then let go.
    Short,
}

/// What a kind of mechanism is meant to do, per stroke: does it complete,
/// and what does it do let go short.
struct Spec {
    flick_completes: bool,
    /// Let go short: a spring takes it home (else it stays where left).
    short_springs_home: bool,
}

fn spec(kind: HandleKind) -> Spec {
    match kind {
        // Racked hard is how a slide or a charging handle is worked; let go
        // anywhere, the spring takes it home.
        HandleKind::Slide | HandleKind::ChargingHandle => Spec {
            flick_completes: true,
            short_springs_home: true,
        },
        // A bolt thrown fast is still a bolt worked; there's no spring.
        HandleKind::TurnBolt | HandleKind::Pump => Spec {
            flick_completes: true,
            short_springs_home: false,
        },
        // A smart push down opens it; short of the latch it stays shut.
        HandleKind::BreakBarrels => Spec {
            flick_completes: true,
            short_springs_home: true,
        },
        // The prod is too heavy to flick past the latch; let go short, the
        // string slaps home.
        HandleKind::String => Spec {
            flick_completes: false,
            short_springs_home: true,
        },
    }
}

/// The left hand away from the gun.
fn away(r: &mut Range) {
    r.vr.trigger(Side::Left, 0.0);
    r.vr.grip(Side::Left, 0.0);
    r.vr.step(1);
    r.vr.move_hand(Side::Left, r.feet + Vec3::new(-0.4, 1.0, 0.0));
    r.vr.step(4);
}

/// Works `id`'s handle by `stroke` and lets go. Returns how far the part
/// came (0 to 1; open or shut for a break action), what it rested at once
/// let go, and the gun's state then.
fn work(id: &str, kind: HandleKind, stroke: Stroke) -> (f32, f32, Range) {
    // A pump is worked by the hand on its forend.
    let mut r = hold(id, kind == HandleKind::Pump, false);
    {
        let mut g = gun_mut(&mut r);
        g.firearm.condition = 1.0;
        // Fed, chamber empty: a full stroke feeds a round.
        g.firearm.chambers.iter_mut().for_each(|c| *c = None);
        if let Some(family) = g.firearm.def.magazine_family.clone() {
            let _ = g.firearm.insert_magazine(Magazine {
                family,
                capacity: 5,
                rounds: 5,
                ..Default::default()
            });
        } else {
            // A fixed magazine (the SKS's) or a tube.
            g.firearm.internal_rounds = 3;
        }
        if kind == HandleKind::Pump {
            let _ = g.firearm.release_action();
        }
    }
    let (back, up) = {
        let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
        (gt.back().as_vec3(), gt.up().as_vec3())
    };
    let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
    let from = if kind == HandleKind::Pump {
        r.vr.hand_state(Side::Left).world.translation
    } else {
        let at = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
        r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(4);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        assert!(
            r.vr.hand_state(Side::Left).handle.is_some(),
            "{id}: took the handle"
        );
        at
    };
    let mut start = from;
    if kind == HandleKind::TurnBolt {
        // Up off its lug first.
        start = from + up * 0.045;
        glide_side(&mut r, Side::Left, from, start, 6);
    }
    let k = if stroke == Stroke::Short { 0.5 } else { 1.15 };
    let to = if kind == HandleKind::BreakBarrels {
        start - up * 0.09 * k
    } else {
        start + back * travel * k
    };
    let ticks = if stroke == Stroke::Flick { 3 } else { 30 };
    let open = |r: &mut Range| gun_mut(r).firearm.bolt_travel;
    let mut most = open(&mut r);
    for i in 1..=ticks {
        r.vr.move_hand(Side::Left, start.lerp(to, i as f32 / ticks as f32));
        r.vr.step(1);
        most = most.max(open(&mut r));
    }
    away(&mut r);
    let rest = open(&mut r);
    (most, rest, r)
}

/// Every gun with a hand-worked handle, every stroke: a flick past the end
/// completes only where that's how the real gun's worked, a steady pull
/// always does, and let go short each does what its own spec says.
#[test]
fn every_hand_worked_mechanism_completes_as_its_kind_should() {
    let mut bad = Vec::new();
    let mut guns = 0;
    for id in [
        "pm_pistol",
        "ak74",
        "mosin",
        "toz_double",
        "sks",
        "nagant",
        "ppsh",
        "mp133",
        "svd",
        "toz8",
        "rpk74",
        "flare_pistol",
        "crossbow",
    ] {
        let Some(kind) = layout::layout(id).and_then(|l| l.handle).map(|h| h.kind) else {
            continue;
        };
        guns += 1;
        let want = spec(kind);
        for stroke in [Stroke::Flick, Stroke::Steady, Stroke::Short] {
            let (most, rest, mut r) = work(id, kind, stroke);
            let f = gun_mut(&mut r).firearm.clone();
            let completed = match kind {
                HandleKind::String => f.locked_back,
                HandleKind::ChargingHandle if f.def.open_bolt => f.locked_back,
                HandleKind::BreakBarrels => rest > 0.5,
                _ => most >= 0.95,
            };
            let should = match stroke {
                Stroke::Flick => want.flick_completes,
                Stroke::Steady => true,
                Stroke::Short => false,
            };
            if completed != should {
                bad.push(format!(
                    "{id} ({kind:?}) {stroke:?}: completed {completed}, want {should} \
                     (came {most:.2}, rests {rest:.2})"
                ));
            }
            // A spring-driven stroke that completes feeds a round home.
            let springs = matches!(kind, HandleKind::Slide | HandleKind::ChargingHandle);
            if springs && !f.def.open_bolt {
                let fed = f.chambers[0] == Some(Round::Live);
                if fed != completed {
                    bad.push(format!(
                        "{id} {stroke:?}: chambered {fed}, completed {completed}"
                    ));
                }
            }
            if stroke == Stroke::Short {
                let held = if kind == HandleKind::BreakBarrels {
                    // Short of the latch: still shut.
                    rest < 0.5
                } else if want.short_springs_home {
                    rest < 0.05
                } else {
                    // No spring: where the hand left it.
                    rest > 0.3
                };
                if !held {
                    bad.push(format!(
                        "{id} ({kind:?}) let go short: rests {rest:.2} (came {most:.2})"
                    ));
                }
            }
        }
    }
    assert_eq!(guns, 12, "every gun with a handle");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// What the left hand felt.
#[derive(Resource, Default)]
struct Felt(Vec<f32>);

/// A slide's recoil spring: a hand drifting back slides on it, a steady
/// rack beats it, and the hand feels it compress, harder the further it
/// comes back.
#[test]
fn a_slide_has_a_spring_a_rack_beats() {
    for id in ["pm_pistol", "ak74"] {
        // Drifting back over 3 s: it doesn't follow.
        let mut r = hold(id, false, false);
        let at = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
        let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
        let back =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .back()
                .as_vec3();
        r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(4);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        let ticks = ((travel * 1.15 / 0.02) * 90.0) as u32;
        let mut most = 0.0f32;
        for i in 1..=ticks {
            r.vr.move_hand(
                Side::Left,
                at + back * travel * 1.15 * i as f32 / ticks as f32,
            );
            r.vr.step(1);
            most = most.max(gun_mut(&mut r).firearm.bolt_travel);
        }
        assert!(
            most < 0.2,
            "{id}: drifted back at 2 cm/s, it came {most:.2}"
        );
        // A steady rack: it comes, and it's felt coming, harder as it goes.
        let mut r = hold(id, false, false);
        r.vr.app.init_resource::<Felt>().add_systems(
            Update,
            |mut ev: MessageReader<crate::rig::haptics::Haptic>, mut felt: ResMut<Felt>| {
                felt.0.extend(
                    ev.read()
                        .filter(|h| h.side == Side::Left && (h.seconds - 0.012).abs() < 1e-4)
                        .map(|h| h.amplitude),
                );
            },
        );
        let (most, _, _) = {
            let at = r.vr.point(r.gun, |p| *p == GripRole::Handle(0));
            r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(4);
            r.vr.trigger(Side::Left, 1.0);
            r.vr.step(2);
            let mut most = 0.0f32;
            for i in 1..=30 {
                r.vr.move_hand(Side::Left, at + back * travel * 1.15 * i as f32 / 30.0);
                r.vr.step(1);
                most = most.max(gun_mut(&mut r).firearm.bolt_travel);
            }
            (most, 0, 0)
        };
        assert!(most >= 0.95, "{id}: a steady rack came {most:.2}");
        let felt = r.vr.app.world().resource::<Felt>().0.clone();
        assert!(felt.len() >= 3, "{id}: felt {felt:?}");
        assert!(
            felt.last().unwrap() > felt.first().unwrap(),
            "{id}: harder as it comes: {felt:?}"
        );
    }
}
