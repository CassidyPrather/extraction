//! Reloading without looking: every step of a reload is heard and felt as
//! itself (`audio::handling_sound`, `rig::haptics::handling_feel`), and
//! the steps that matter (is the magazine in? is one chambered?) unlike
//! anything else.
//!
//! A magazine brought up into the well too gently hangs on its catch
//! (`Firearm::half_seat_magazine`): it scrapes rather than snaps, it hangs
//! a few millimetres low, it feeds nothing, and the rack's jolt drops it
//! out. A palm slapped up under it (`tap_magazines`) seats it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::firearm::Event as FireEvent;
use std::collections::HashMap;

use super::{Gun, GunEvent, Mag};
use crate::interact::{Hand, Held, InSocket, InteractEvent, Socket, physics_on};

/// Under this speed up into the well, m/s relative to the gun, a magazine
/// hangs on its catch instead of seating.
pub const SEAT_SPEED: f32 = 0.3;
/// How far a half-seated magazine hangs below home, metres.
pub const HALF_DROP: f32 = 0.008;
/// How fast a dropped magazine leaves the well, m/s down the gun.
const DROP_SPEED: f32 = 0.6;
/// How far a magazine hangs out of the well while the release is held:
/// out of the catch, held for the other hand to take, metres.
pub const PRESENT_DROP: f32 = 0.035;

/// A magazine let go by the release and held out of the well while the
/// button's down, for the other hand to take (the first playtest wanted
/// it held, not dropped). Let go of the button, and it falls.
#[derive(Component, Debug)]
pub struct Presented;

/// The mag release pressed: the magazine comes out of the catch and hangs
/// half out of the well.
#[derive(Message, Clone, Copy, Debug)]
pub struct PresentMagazine {
    pub gun: Entity,
}

pub(crate) fn present_magazines(
    mut commands: Commands,
    mut requests: MessageReader<PresentMagazine>,
    mut guns: Query<(&mut Gun, &GlobalTransform)>,
    sockets: Query<&Socket>,
    mut mags: Query<&mut crate::items::Item, With<Mag>>,
    mut out: MessageWriter<GunEvent>,
) {
    for req in requests.read() {
        let Ok((mut gun, gt)) = guns.get_mut(req.gun) else {
            continue;
        };
        let Some(m) = gun
            .magwell
            .and_then(|w| sockets.get(w).ok())
            .and_then(|s| s.occupant)
        else {
            continue;
        };
        let Some(removed) = gun.firearm.remove_magazine() else {
            continue;
        };
        if let Ok(mut it) = mags.get_mut(m) {
            it.rounds = removed.rounds;
            it.loads = removed.loads;
        }
        commands.entity(m).insert(Presented);
        out.write(GunEvent {
            gun: req.gun,
            item: String::new(),
            event: FireEvent::MagazineRemoved,
            at: gt.translation(),
        });
    }
}

/// Out of the well (taken, or dropped): no longer held out.
pub(crate) fn unpresent(mut events: MessageReader<InteractEvent>, mut commands: Commands) {
    for ev in events.read() {
        if let InteractEvent::Unsocketed { item, .. } = ev {
            commands.entity(*item).try_remove::<Presented>();
        }
    }
}

/// Every magazine in a hand and its hand's velocity, as of the last tick
/// (rebuilt every tick: the well reads it the tick the hand lets go).
#[derive(Resource, Default)]
pub struct MagApproach(pub HashMap<Entity, Vec3>);

pub(crate) fn note_approach(
    mags: Query<(Entity, &Held), With<Mag>>,
    hands: Query<&Hand>,
    mut approach: ResMut<MagApproach>,
) {
    approach.0.clear();
    for (e, held) in &mags {
        if let Ok(hand) = hands.get(held.hand) {
            approach.0.insert(e, hand.velocity);
        }
    }
}

/// A seated magazine at home in its well, a half-seated or slipped one
/// hanging short.
pub(crate) fn hang_mags(
    guns: Query<&Gun>,
    sockets: Query<&Socket>,
    mut mags: Query<(&Mag, &mut Transform, Has<Presented>), With<InSocket>>,
) {
    for gun in &guns {
        let Some(m) = gun
            .magwell
            .and_then(|w| sockets.get(w).ok())
            .and_then(|s| s.occupant)
        else {
            continue;
        };
        let Ok((mag, mut t, presented)) = mags.get_mut(m) else {
            continue;
        };
        // Held out by the release; half-seated, or slipped down the well
        // (a failure to feed).
        let short = if presented {
            PRESENT_DROP
        } else if gun.firearm.mag_unseated {
            HALF_DROP
        } else {
            0.0
        };
        let want = -mag.height * 0.5 - short;
        if (t.translation.y - want).abs() > 1e-5 {
            t.translation.y = want;
        }
    }
}

/// The rack shook a half-seated magazine out: it falls from the well.
pub(crate) fn drop_half_seated(
    mut commands: Commands,
    mut events: MessageReader<GunEvent>,
    guns: Query<(&Gun, &GlobalTransform, Option<&LinearVelocity>)>,
    mut sockets: Query<&mut Socket>,
    gts: Query<&GlobalTransform>,
    mut out: MessageWriter<InteractEvent>,
) {
    for ev in events.read() {
        if ev.event != FireEvent::MagazineDropped {
            continue;
        }
        let Ok((gun, gun_gt, vel)) = guns.get(ev.gun) else {
            continue;
        };
        let Some(well) = gun.magwell else {
            continue;
        };
        let Ok(mut socket) = sockets.get_mut(well) else {
            continue;
        };
        let Some(item) = socket.occupant.take() else {
            continue;
        };
        let world = gts
            .get(item)
            .map_or(Transform::IDENTITY, |g| g.compute_transform());
        let v = vel.map_or(Vec3::ZERO, |v| v.0) - gun_gt.up().as_vec3() * DROP_SPEED;
        physics_on(&mut commands, item, world, v, Vec3::ZERO, true);
        out.write(InteractEvent::Unsocketed { item, socket: well });
    }
}

/// One gun's reload, step by step, as the mechanism tells it: what each
/// step sends (`GunEvent`s, the game's own: a magazine pulled, a round
/// thumbed in). Steps that send nothing (a turn-bolt drawn back on an
/// empty chamber) are felt only in the handle's travel and aren't listed.
pub fn reload_steps(
    def: &extraction_core::firearm::FirearmDef,
) -> Vec<(&'static str, Vec<FireEvent>)> {
    use extraction_core::firearm::{ActionKind::*, Firearm, Magazine};
    let new = || Firearm::new(def.clone(), 1.0);
    let mag = |rounds| Magazine {
        family: def.magazine_family.clone().unwrap_or_default(),
        capacity: 30,
        rounds,
        ..Default::default()
    };
    let mut steps = Vec::new();
    match def.action {
        SemiAuto if def.magazine_family.is_some() => {
            let mut g = new();
            steps.push(("half-seated", g.half_seat_magazine(mag(5)).unwrap()));
            steps.push(("racked, it drops", g.set_bolt_travel(1.0)));
            let mut g = new();
            steps.push(("seated", g.insert_magazine(mag(5)).unwrap()));
            steps.push(("racked back", g.set_bolt_travel(1.0)));
            let home = g.release_bolt();
            if def.open_bolt {
                steps.push(("caught on the sear", home));
            } else {
                steps.push(("home, chambered", home));
                g.remove_magazine();
                steps.push(("released", vec![FireEvent::MagazineRemoved]));
                let mut g = new();
                g.set_bolt_travel(1.0);
                steps.push(("home, empty", g.release_bolt()));
            }
            if def.last_round_hold_open {
                let mut g = new();
                g.insert_magazine(mag(0)).unwrap();
                g.set_bolt_travel(1.0);
                steps.push(("held open", g.release_bolt()));
            }
        }
        SemiAuto => {
            let mut g = new();
            g.set_bolt_travel(1.0);
            steps.push(("held open", g.release_bolt()));
            let n = g.strip_clip(def.internal_capacity);
            steps.push((
                "clip stripped",
                vec![FireEvent::RoundLoaded; n.min(1) as usize],
            ));
            steps.push(("home, chambered", g.release_slide_stop()));
        }
        BoltAction => {
            let mut g = new();
            steps.push(("lifted", g.set_handle_lift(1.0)));
            g.set_bolt_travel(1.0);
            g.load_round();
            let home = g.set_bolt_travel(0.0);
            if home.contains(&FireEvent::Chambered) {
                steps.push(("round in", vec![FireEvent::RoundLoaded]));
                steps.push(("home, chambered", home));
                let mut g = new();
                g.set_handle_lift(1.0);
                g.set_bolt_travel(1.0);
                steps.push(("home, empty", g.set_bolt_travel(0.0)));
            } else {
                // A single-shot (a TOZ-8): the round goes into the chamber
                // by hand, felt there, and the bolt only closes on it.
                steps.push(("chambered by hand", vec![FireEvent::RoundLoaded]));
                steps.push(("home", home));
            }
        }
        Pump => {
            let mut g = new();
            g.load_round();
            steps.push(("shell in", vec![FireEvent::RoundLoaded]));
            steps.push(("pumped back", g.set_bolt_travel(1.0)));
            steps.push(("home, chambered", g.set_bolt_travel(0.0)));
            let mut g = new();
            g.set_bolt_travel(1.0);
            steps.push(("home, empty", g.set_bolt_travel(0.0)));
        }
        BreakAction => {
            let mut g = new();
            steps.push(("broken open", g.set_break_open(true)));
            g.load_round();
            steps.push(("shell in", vec![FireEvent::RoundLoaded]));
            steps.push(("shut", g.set_break_open(false)));
        }
        Revolver => {
            let mut g = new();
            steps.push(("gate open", g.set_gate(true)));
            g.load_round();
            steps.push(("round in", vec![FireEvent::RoundLoaded]));
            steps.push(("turned", g.rotate_cylinder()));
            steps.push(("gate shut", g.set_gate(false)));
        }
        Crossbow => {
            let mut g = new();
            g.set_bolt_travel(1.0);
            steps.push(("spanned", g.release_bolt()));
        }
    }
    steps.retain(|(_, ev)| !ev.is_empty());
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::Catalog;
    use crate::rig::haptics::handling_feel;

    /// Each step's sounds and feels.
    fn cues(
        action: extraction_core::firearm::ActionKind,
        id: &str,
        ev: &[FireEvent],
    ) -> (Vec<String>, Vec<String>) {
        // What the ear gets: the sample set and how fast it plays (two cue
        // names on one sample at one speed are one sound).
        let mut heard: Vec<_> = ev
            .iter()
            .filter_map(|e| {
                crate::audio::handling_sound(e, Some(action), id).map(|(set, _)| {
                    format!(
                        "{set} x{:.2}",
                        crate::audio::handling_speed(e, Some(action))
                    )
                })
            })
            .collect();
        let mut felt: Vec<_> = ev
            .iter()
            .filter_map(|e| handling_feel(e).map(|f| format!("{f:?}")))
            .collect();
        heard.sort();
        heard.dedup();
        felt.sort();
        felt.dedup();
        (heard, felt)
    }

    /// Every gun's reload, step by step, is heard and felt: each step has
    /// a sound and a feel, and no two steps of one gun's reload sound or
    /// feel alike (seated from half-seated, chambered from empty, held
    /// open from home), so a hand working it unseen knows where it is.
    #[test]
    fn every_reload_step_is_heard_and_felt_as_itself() {
        let catalog = Catalog::load();
        let mut bad = Vec::new();
        for (_, def) in catalog.iter() {
            let Some(f) = def.firearm.as_ref() else {
                continue;
            };
            let id = def.id.as_str();
            let steps: Vec<_> = reload_steps(f)
                .into_iter()
                .map(|(name, ev)| {
                    let (heard, felt) = cues(f.action, id, &ev);
                    (name, ev, heard, felt)
                })
                .collect();
            for (name, ev, heard, felt) in &steps {
                if heard.is_empty() {
                    bad.push(format!("{id} {name}: unheard ({ev:?})"));
                }
                if felt.is_empty() {
                    bad.push(format!("{id} {name}: unfelt ({ev:?})"));
                }
            }
            for (i, a) in steps.iter().enumerate() {
                for b in &steps[i + 1..] {
                    if a.2 == b.2 {
                        bad.push(format!("{id}: {} and {} sound alike {:?}", a.0, b.0, a.2));
                    }
                    if a.3 == b.3 {
                        bad.push(format!("{id}: {} and {} feel alike {:?}", a.0, b.0, a.3));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}

#[cfg(test)]
mod vr {
    use super::*;
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;

    fn well(r: &mut Range) -> Vec3 {
        let w = gun_mut(r).magwell.unwrap();
        r.vr.app
            .world()
            .get::<GlobalTransform>(w)
            .unwrap()
            .translation()
    }

    /// An AK in the right hand, a full magazine eased up into its well by
    /// the left over `steps` ticks and let go.
    fn ease_in(steps: usize) -> (Range, Entity) {
        let mut r = hold("ak74", false, false);
        let mag = take(&mut r, "ak_mag", 30);
        let w = well(&mut r);
        let away = r.vr.hand_state(Side::Left).world.translation;
        glide(&mut r, away, w - Vec3::Y * 0.25, 15);
        glide(&mut r, w - Vec3::Y * 0.25, w, steps);
        r.vr.step(5);
        r.vr.grip(Side::Left, 0.0);
        r.vr.hand(
            Side::Left,
            w + Vec3::new(-0.3, -0.2, 0.0),
            Quat::IDENTITY,
            0.0,
            0.0,
        );
        r.vr.step(5);
        (r, mag)
    }

    fn in_well(r: &Range, mag: Entity) -> bool {
        r.vr.app
            .world()
            .get::<crate::interact::InSocket>(mag)
            .is_some()
    }

    fn hangs(r: &Range, mag: Entity) -> f32 {
        let w = r.vr.app.world();
        -w.get::<Transform>(mag).unwrap().translation.y - w.get::<Mag>(mag).unwrap().height * 0.5
    }

    /// Eased up into the well, the magazine hangs on its catch: in the
    /// well, but low, and it feeds nothing; the rack shakes it out.
    #[test]
    fn a_magazine_eased_in_hangs_and_drops_on_the_rack() {
        let (mut r, mag) = ease_in(240);
        assert!(in_well(&r, mag), "it's in the well");
        assert!(gun_mut(&mut r).firearm.mag_half, "half-seated");
        let low = hangs(&r, mag);
        assert!((low - HALF_DROP).abs() < 1e-4, "it hangs short: {low}");
        rack(&mut r);
        assert!(!in_well(&r, mag), "the rack dropped it");
        r.vr.step(20);
        assert!(gun_mut(&mut r).firearm.magazine.is_none());
        assert!(gun_mut(&mut r).firearm.chambers[0].is_none(), "nothing fed");
        let fell = well(&mut r).y
            - r.vr
                .app
                .world()
                .get::<GlobalTransform>(mag)
                .unwrap()
                .translation()
                .y;
        assert!(fell > 0.1, "it fell clear: {fell}");
    }

    /// The same magazine pushed home briskly seats, and a half-seated one
    /// slapped home seats and feeds.
    #[test]
    fn a_magazine_pushed_home_or_tapped_feeds() {
        let (mut r, mag) = ease_in(30);
        assert!(
            in_well(&r, mag) && !gun_mut(&mut r).firearm.mag_half,
            "pushed home"
        );
        assert!(hangs(&r, mag).abs() < 1e-4, "at home");

        let (mut r, mag) = ease_in(240);
        assert!(gun_mut(&mut r).firearm.mag_half);
        let floor = {
            let w = r.vr.app.world();
            let gt = w.get::<GlobalTransform>(mag).unwrap();
            gt.translation() - gt.rotation() * Vec3::Y * w.get::<Mag>(mag).unwrap().height * 0.5
        };
        for i in 0..=8 {
            let at = floor - Vec3::Y * (0.12 - 0.015 * i as f32);
            r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(1);
        }
        r.vr.step(2);
        assert!(!gun_mut(&mut r).firearm.mag_half, "the tap seated it");
        assert!(hangs(&r, mag).abs() < 1e-4, "at home");
        rack(&mut r);
        assert!(in_well(&r, mag), "it rode out the rack");
        {
            let mut g = gun_mut(&mut r);
            while g.firearm.fire_mode() != extraction_core::firearm::FireMode::Semi {
                g.firearm.cycle_mode();
            }
        }
        assert_eq!(fire(&mut r, 1, true).len(), 1, "it feeds");
    }
}

#[cfg(test)]
mod grab {
    use super::*;
    use crate::interact::{GripRole, InSocket};
    use crate::rig::Side;
    use crate::weapons::feel::vr_tests::*;

    /// Every gun with a magazine well, and the magazine that fits it.
    fn guns_and_mags() -> Vec<(String, String)> {
        let catalog = crate::items::Catalog::load();
        catalog
            .iter()
            .filter_map(|(_, d)| {
                let fam = d.firearm.as_ref()?.magazine_family.clone()?;
                let (_, m) = catalog.iter().find(|(_, m)| {
                    m.magazine
                        .as_ref()
                        .is_some_and(|m| m.fits == fam && !m.stripper)
                })?;
                Some((d.id.clone(), m.id.clone()))
            })
            .collect()
    }

    /// Six ways in to a point, 3 cm off it.
    const APPROACHES: [Vec3; 6] = [
        Vec3::new(0.03, 0.0, 0.0),
        Vec3::new(-0.03, 0.0, 0.0),
        Vec3::new(0.0, 0.03, 0.0),
        Vec3::new(0.0, -0.03, 0.0),
        Vec3::new(0.0, 0.0, 0.03),
        Vec3::new(0.0, 0.0, -0.03),
    ];

    fn holding(r: &mut Range, side: Side) -> Option<Entity> {
        r.vr.hand_state(side).holding
    }

    /// The first playtest's top complaint: grabbing the pistol took its
    /// magazine. For every gun with a seated magazine, a grab at its grip
    /// from any side takes the gun, lying loose or held in the other hand
    /// (a pass between hands); only a deliberate pull from under the well
    /// of a gun already in the other hand takes the magazine.
    #[test]
    fn a_grab_takes_the_gun_never_the_magazine_in_it() {
        let mut bad = Vec::new();
        for (gun, mag) in guns_and_mags() {
            for (k, off) in APPROACHES.iter().enumerate() {
                // Held in the right hand: the left closes on its grip.
                let mut r = hold(&gun, false, false);
                let m = seat_mag(&mut r, &mag, 3);
                let grip = r.vr.point(r.gun, |p| *p == GripRole::Primary);
                r.vr.hand(Side::Left, grip + *off, Quat::IDENTITY, 0.0, 0.0);
                r.vr.step(2);
                r.vr.grip(Side::Left, 1.0);
                r.vr.step(3);
                if holding(&mut r, Side::Left) != Some(r.gun) {
                    bad.push(format!(
                        "{gun}: a pass from approach {k} took {:?}",
                        holding(&mut r, Side::Left)
                    ));
                }
                if r.vr.app.world().get::<InSocket>(m).is_none() {
                    bad.push(format!(
                        "{gun}: a pass from approach {k} pulled the magazine"
                    ));
                }
                // Let go of everything, the gun left lying still: the right
                // hand comes back for it.
                r.vr.grip(Side::Left, 0.0);
                r.vr.grip(Side::Right, 0.0);
                r.vr.app
                    .world_mut()
                    .entity_mut(r.gun)
                    .insert(avian3d::prelude::RigidBody::Static);
                r.vr.step(3);
                let grip = r.vr.point(r.gun, |p| *p == GripRole::Primary);
                r.vr.hand(Side::Right, grip + *off, Quat::IDENTITY, 0.0, 0.0);
                r.vr.step(2);
                r.vr.grip(Side::Right, 1.0);
                r.vr.step(3);
                if holding(&mut r, Side::Right) != Some(r.gun) {
                    bad.push(format!(
                        "{gun}: lying, approach {k} took {:?}",
                        holding(&mut r, Side::Right)
                    ));
                }
                if r.vr.app.world().get::<InSocket>(m).is_none() {
                    bad.push(format!("{gun}: lying, approach {k} pulled the magazine"));
                }
            }
            // The deliberate pull: the gun in the right hand, the left
            // closing under the well on the magazine's foot.
            let mut r = hold(&gun, false, false);
            let m = seat_mag(&mut r, &mag, 3);
            let foot = {
                let w = r.vr.app.world();
                let gt = w.get::<GlobalTransform>(m).unwrap();
                gt.translation() - gt.rotation() * Vec3::Y * w.get::<Mag>(m).unwrap().height * 0.5
            };
            r.vr.hand(Side::Left, foot - Vec3::Y * 0.01, Quat::IDENTITY, 0.0, 0.0);
            r.vr.step(2);
            r.vr.grip(Side::Left, 1.0);
            r.vr.step(3);
            if holding(&mut r, Side::Left) != Some(m) {
                bad.push(format!(
                    "{gun}: a pull at the magazine's foot took {:?}",
                    holding(&mut r, Side::Left)
                ));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Held, the mag release lets the magazine out of its catch and holds
    /// it half out of the well for the other hand; let go of the button,
    /// and it falls.
    #[test]
    fn the_release_held_hangs_the_magazine_out_for_the_other_hand() {
        let mut r = hold("pm_pistol", false, false);
        let m = seat_mag(&mut r, "pm_mag", 8);
        r.vr.primary(Side::Right, true);
        r.vr.step(4);
        let w = r.vr.app.world();
        assert!(w.get::<InSocket>(m).is_some(), "still in the well");
        assert!(w.get::<Presented>(m).is_some(), "held out");
        let low =
            -w.get::<Transform>(m).unwrap().translation.y - w.get::<Mag>(m).unwrap().height * 0.5;
        assert!((low - PRESENT_DROP).abs() < 1e-4, "half out: {low}");
        assert!(
            gun_mut(&mut r).firearm.magazine.is_none(),
            "out of the catch"
        );
        // The other hand takes it by its middle.
        let at =
            r.vr.app
                .world()
                .get::<GlobalTransform>(m)
                .unwrap()
                .translation();
        r.vr.hand(Side::Left, at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(Side::Left, 1.0);
        r.vr.step(3);
        assert_eq!(holding(&mut r, Side::Left), Some(m), "taken");
        r.vr.primary(Side::Right, false);
        r.vr.step(5);
        assert_eq!(holding(&mut r, Side::Left), Some(m), "still in the hand");
        assert_eq!(
            r.vr.app
                .world()
                .get::<crate::items::Item>(m)
                .unwrap()
                .rounds,
            8
        );
        // Pressed and let go with nothing taking it: it falls.
        let mut r = hold("pm_pistol", false, false);
        let m = seat_mag(&mut r, "pm_mag", 8);
        r.vr.primary(Side::Right, true);
        r.vr.step(3);
        r.vr.primary(Side::Right, false);
        r.vr.step(10);
        assert!(r.vr.app.world().get::<InSocket>(m).is_none(), "dropped");
    }

    /// A box held at a magazine's lips (not only its middle) loads it, a
    /// round at a time while it's held there.
    #[test]
    fn a_box_at_the_lips_loads_a_round_at_a_time() {
        let mut r = hold("pm_pistol", false, false);
        // The gun away; a box in the right hand, an empty magazine in the left.
        r.vr.grip(Side::Right, 0.0);
        r.vr.step(2);
        let mag = take(&mut r, "pm_mag", 0);
        let box_at = r.feet + Vec3::new(0.35, 1.0, -0.3);
        let bx = r.vr.spawn("ammo_9x18", Transform::from_translation(box_at));
        r.vr.app
            .world_mut()
            .entity_mut(bx)
            .insert(avian3d::prelude::RigidBody::Static);
        r.vr.hand(Side::Right, box_at, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(2);
        r.vr.grip(Side::Right, 1.0);
        r.vr.step(3);
        assert_eq!(holding(&mut r, Side::Right), Some(bx), "the box");
        r.vr.app
            .world_mut()
            .entity_mut(bx)
            .insert(avian3d::prelude::RigidBody::Dynamic);
        // To the lips: past the old reach of the middle.
        let lips = {
            let w = r.vr.app.world();
            let gt = w.get::<GlobalTransform>(mag).unwrap();
            gt.translation() + gt.up().as_vec3() * (w.get::<Mag>(mag).unwrap().height * 0.5)
        };
        r.vr.hand(Side::Right, lips + Vec3::Y * 0.13, Quat::IDENTITY, 1.0, 0.0);
        let rounds = |r: &Range| {
            r.vr.app
                .world()
                .get::<crate::items::Item>(mag)
                .unwrap()
                .rounds
        };
        let mut seen = Vec::new();
        for _ in 0..40 {
            r.vr.step(1);
            seen.push(rounds(&r));
        }
        assert!(seen[0] <= 1, "not all at once: {seen:?}");
        assert!(
            seen.windows(2).all(|w| w[1] <= w[0] + 1),
            "a round at a time: {seen:?}"
        );
        assert!(
            *seen.last().unwrap() >= 3,
            "and on while it's held there: {seen:?}"
        );
    }
}
