//! Each gun's recoil signature: how its muzzle moves over the half second
//! after the trigger goes down, held as a competent player holds it (two
//! hands where it has a place for the second, shouldered where it has a
//! stock), trigger held on its most automatic setting. A gauge holds every
//! gun to being its own (each pair's features differ by at least
//! [`DISTINCT`]) and
//! to staying in hand (no more than [`CONTROL_RISE`] of climb with two
//! hands, [`CONTROL_RISE_ONE`] for a pistol in one, and settled by the end
//! unless it's still firing).
//!
//! Recorded through the headset path: poses, grip and trigger into
//! `RigInput`; the trace is the gun as the physics moves it.

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use extraction_core::firearm::{FireMode, Round};

use crate::rig::Side;
use crate::weapons::Gun;
use crate::weapons::feel::vr_tests::{Range, gun_mut, hold, load};

/// Ticks in a signature (half a second at 90 Hz).
pub const TICKS: usize = 45;
/// How far apart two guns' features ([`Signature::apart`]) must be for
/// them to feel like different guns.
pub const DISTINCT: f32 = 8.0;
/// The most the muzzle may climb (or swing) within the half second with
/// two hands, radians.
pub const CONTROL_RISE: f32 = 0.14;
/// A pistol, in the one hand the game gives it: the same, by this.
pub const CONTROL_RISE_ONE: f32 = 0.4;
/// A gun that fired once is back within this share of its peak rise by
/// the end of the half second.
pub const SETTLED: f32 = 0.35;
/// Within this of where it pointed counts as settled whatever the peak,
/// mrad (a hand's own drift).
pub const SETTLED_FLOOR: f32 = 5.0;

/// The muzzle's path: per tick, rise and yaw (mrad, up and left of where
/// it pointed) and its displacement (mm, back along the bore and up), in
/// the gun's frame before the shot.
#[derive(Clone, Debug, Default)]
pub struct Signature {
    pub id: String,
    pub hands: u8,
    pub shouldered: bool,
    pub shots: usize,
    /// The action's slams heard (a carrier home, an open bolt).
    pub slams: usize,
    /// It cycles itself.
    pub self_loading: bool,
    /// The bore's height over where the gun bears (the butt shouldered,
    /// else the grip), mm.
    pub lever: f32,
    /// The gun's mass, kg.
    pub mass: f32,
    pub rise: Vec<f32>,
    pub yaw: Vec<f32>,
    pub back: Vec<f32>,
    pub up: Vec<f32>,
}

impl Signature {
    /// The largest rise or swing, mrad.
    pub fn peak(&self) -> f32 {
        self.rise
            .iter()
            .zip(&self.yaw)
            .map(|(r, y)| r.hypot(*y))
            .fold(0.0, f32::max)
    }

    /// What a hand and an eye tell apart: the peak rise (mrad) and when it
    /// comes (per 10 ms), the peak push back (per half mm), the climb and
    /// walk left at the end (mrad; per half mrad of walk), and the shots
    /// (each worth 5).
    pub fn features(&self) -> [f32; 6] {
        let (t, rise) = self
            .rise
            .iter()
            .copied()
            .enumerate()
            .fold((0, f32::MIN), |b, (i, r)| if r > b.1 { (i, r) } else { b });
        [
            rise,
            t as f32 * 100.0 / 90.0,
            self.back.iter().copied().fold(f32::MIN, f32::max) * 2.0,
            self.rise.last().copied().unwrap_or(0.0),
            self.yaw.last().copied().unwrap_or(0.0) * 2.0,
            self.shots as f32 * 5.0,
        ]
    }

    /// How far apart two guns' features are.
    pub fn apart(&self, other: &Signature) -> f32 {
        self.features()
            .iter()
            .zip(other.features())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// RMS difference from `other` over the whole trace.
    pub fn distance(&self, other: &Signature) -> f32 {
        let mut sum = 0.0;
        let mut n = 0;
        for (a, b) in [
            (&self.rise, &other.rise),
            (&self.yaw, &other.yaw),
            (&self.back, &other.back),
            (&self.up, &other.up),
        ] {
            for (x, y) in a.iter().zip(b) {
                sum += (x - y).powi(2);
                n += 1;
            }
        }
        (sum / n.max(1) as f32).sqrt()
    }

    /// One line: peak rise, when, peak push, yaw at the end, rise at the end.
    pub fn line(&self) -> String {
        let (t, rise) = self
            .rise
            .iter()
            .copied()
            .enumerate()
            .fold((0, f32::MIN), |b, (i, r)| if r > b.1 { (i, r) } else { b });
        let back = self.back.iter().copied().fold(f32::MIN, f32::max);
        format!(
            "{:<13} {}h{} lever {:>5.1} mm {:>4.1} kg shots {:>2}  rise {:>6.1} mrad at {:>3.0} ms  back {:>5.1} mm  up {:>5.1} mm  yaw end {:>6.1}  rise end {:>6.1}",
            self.id,
            self.hands,
            if self.shouldered { "+s" } else { "  " },
            self.lever,
            self.mass,
            self.shots,
            rise,
            t as f32 * 1000.0 / 90.0,
            back,
            self.up.iter().copied().fold(f32::MIN, f32::max),
            self.yaw.last().copied().unwrap_or(0.0),
            self.rise.last().copied().unwrap_or(0.0),
        )
    }
}

/// Every gun in the catalog, by id.
pub fn guns() -> Vec<String> {
    let catalog = crate::items::Catalog::load();
    catalog
        .iter()
        .filter(|(_, d)| d.firearm.is_some())
        .map(|(_, d)| d.id.clone())
        .collect()
}

/// A round in every chamber, cocked and closed, on its most automatic
/// setting (a magazine full where it takes one).
fn ready(r: &mut Range) {
    let def = gun_mut(r).firearm.def.clone();
    let best = if def.modes.contains(&FireMode::Auto) {
        FireMode::Auto
    } else {
        FireMode::Semi
    };
    if let Some(family) = &def.magazine_family {
        load(r, family, 30, best);
        return;
    }
    let mut g = gun_mut(r);
    let f = &mut g.firearm;
    f.bolt_travel = 0.0;
    for c in f.chambers.iter_mut() {
        *c = Some(Round::Live);
    }
    for h in f.hammers_cocked.iter_mut() {
        *h = true;
    }
    while f.fire_mode() != best {
        f.cycle_mode();
    }
    f.condition = 1.0;
    f.fouling = 0.0;
}

/// Records `id`'s signature.
pub fn record(id: &str) -> Signature {
    let (two, shoulder) = {
        let probe = hold(id, false, false);
        let g = probe.vr.app.world().get::<Gun>(probe.gun).unwrap();
        let two = g.layout.support.is_some();
        (two, two && crate::weapons::feel::butt(g).is_some())
    };
    let mut r = hold(id, two, shoulder);
    ready(&mut r);
    r.vr.step(30);
    let pose = |r: &Range| {
        let w = r.vr.app.world();
        let g = w.get::<Gun>(r.gun).unwrap();
        let gt = w.get::<GlobalTransform>(r.gun).unwrap();
        (gt.transform_point(g.layout.muzzle), gt.rotation())
    };
    let (m0, q0) = pose(&r);
    let inv = q0.inverse();
    let mut sig = Signature {
        id: id.into(),
        ..default()
    };
    let mut cursor =
        r.vr.app
            .world()
            .resource::<Messages<crate::weapons::GunEvent>>()
            .get_cursor_current();
    let gun = r.gun;
    r.vr.trigger(Side::Right, 1.0);
    for _ in 0..TICKS {
        r.vr.step(1);
        let w = r.vr.app.world();
        use extraction_core::firearm::Event as E;
        for ev in cursor.read(w.resource::<Messages<crate::weapons::GunEvent>>()) {
            match ev.event {
                _ if ev.gun != gun => {}
                E::Fired { .. } => sig.shots += 1,
                E::CarrierHome | E::OpenBoltSlam => sig.slams += 1,
                _ => {}
            }
        }
        let (m, q) = pose(&r);
        let d = inv * (q * Vec3::NEG_Z);
        sig.rise.push(d.y.atan2(-d.z) * 1000.0);
        sig.yaw.push((-d.x).atan2(-d.z) * 1000.0);
        let off = inv * (m - m0);
        sig.back.push(off.z * 1000.0);
        sig.up.push(off.y * 1000.0);
    }
    r.vr.trigger(Side::Right, 0.0);
    let h =
        r.vr.app
            .world()
            .get::<crate::weapons::feel::Handling>(r.gun)
            .unwrap()
            .hold;
    sig.hands = h.hands;
    sig.shouldered = h.shouldered;
    let w = r.vr.app.world();
    let g = w.get::<Gun>(r.gun).unwrap();
    sig.self_loading = g.firearm.def.action == extraction_core::firearm::ActionKind::SemiAuto;
    sig.lever = crate::weapons::feel::lever(g, h.shouldered) * 1000.0;
    let catalog = w.resource::<crate::items::Catalog>();
    sig.mass = catalog
        .get(w.get::<crate::items::Item>(r.gun).unwrap().kind())
        .mass;
    sig
}

mod tests {
    use super::*;
    use crate::interact::GripRole;
    use crate::weapons::armoury::{BIPOD_HANDLE, Bipod};
    use crate::weapons::feel::Handling;
    use crate::weapons::feel::vr_tests::glide_side;

    fn handling(r: &Range) -> &Handling {
        r.vr.app.world().get::<Handling>(r.gun).unwrap()
    }

    /// Working the MP-133's pump hard shoves the gun in the hands: back
    /// against the rear stop, forward going home.
    #[test]
    fn the_pump_shoves_the_gun_at_its_stops() {
        let mut r = hold("mp133", true, true);
        r.vr.step(30);
        let from = r.vr.hand_state(Side::Left).world.translation;
        let travel = gun_mut(&mut r).layout.handle.as_ref().unwrap().travel;
        let back =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .back()
                .as_vec3();
        let to = from + back * travel * 1.2;
        let (mut most_back, mut most_fore) = (0.0f32, 0.0f32);
        let mut stroke = |r: &mut Range, a: Vec3, b: Vec3| {
            for i in 0..=6 {
                r.vr.move_hand(Side::Left, a.lerp(b, i as f32 / 6.0));
                r.vr.step(1);
                let p = handling(r).push;
                most_back = most_back.max(p);
                most_fore = most_fore.min(p);
            }
            for _ in 0..10 {
                r.vr.step(1);
                let p = handling(r).push;
                most_back = most_back.max(p);
                most_fore = most_fore.min(p);
            }
        };
        stroke(&mut r, from, to);
        assert!(gun_mut(&mut r).firearm.bolt_travel > 0.9, "pumped back");
        stroke(&mut r, to, from);
        assert!(gun_mut(&mut r).firearm.bolt_travel < 0.1, "home");
        assert!(most_back > 0.0005, "a shove back: {most_back} m");
        assert!(most_fore < -0.0005, "a jolt forward: {most_fore} m");
    }

    /// The RPK on its deployed bipod, its feet on a wall top: a shot hops
    /// the muzzle and it drops back past where it was before it settles,
    /// as the legs give and spring back. Off the bipod it just settles.
    #[test]
    fn the_rpk_hops_on_its_bipod() {
        let mut r = hold("rpk74", false, false);
        let legs = r.vr.point(r.gun, |p| *p == GripRole::Handle(BIPOD_HANDLE));
        r.vr.hand(Side::Left, legs, Quat::IDENTITY, 0.0, 0.0);
        r.vr.step(20);
        r.vr.trigger(Side::Left, 1.0);
        r.vr.step(2);
        let down =
            r.vr.app
                .world()
                .get::<GlobalTransform>(r.gun)
                .unwrap()
                .down()
                .as_vec3();
        glide_side(&mut r, Side::Left, legs, legs + down * 0.07, 10);
        r.vr.grip(Side::Left, 0.0);
        r.vr.trigger(Side::Left, 0.0);
        r.vr.step(3);
        r.vr.move_hand(Side::Left, r.feet + Vec3::new(-0.4, 1.0, 0.0));
        r.vr.step(30);
        let b = *r.vr.app.world().get::<Bipod>(r.gun).unwrap();
        assert!(b.swing >= 1.0, "down: {b:?}");
        let feet = {
            let gt = r.vr.app.world().get::<GlobalTransform>(r.gun).unwrap();
            gt.transform_point(b.feet_now())
        };
        r.vr.app.world_mut().spawn((
            Transform::from_translation(feet - Vec3::Y * 0.11),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(0.3, 0.2, 0.12),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
        ));
        r.vr.step(10);
        assert!(handling(&r).hold.braced, "rested on its feet");
        let hop = |r: &mut Range, rested: bool| {
            gun_mut(r).firearm.remove_magazine();
            load(r, "ak545", 30, FireMode::Semi);
            r.vr.step(5);
            assert_eq!(handling(r).hold.bipod && handling(r).hold.braced, rested);
            r.vr.trigger(Side::Right, 1.0);
            let (mut peak, mut under) = (0.0f32, 0.0f32);
            for _ in 0..45 {
                r.vr.step(1);
                let x = handling(r).rise.x;
                peak = peak.max(x);
                if peak > 0.0 && x < under {
                    under = x;
                }
            }
            r.vr.trigger(Side::Right, 0.0);
            r.vr.step(30);
            (peak, under)
        };
        let (peak, under) = hop(&mut r, true);
        assert!(peak > 0.0, "it kicked");
        assert!(under < -0.08 * peak, "dropped back past: {under} of {peak}");
        r.vr.app
            .world_mut()
            .get_mut::<Bipod>(r.gun)
            .unwrap()
            .deployed = false;
        r.vr.step(90);
        let (peak, under) = hop(&mut r, false);
        assert!(
            under > -0.05 * peak,
            "off the bipod, no hop: {under} of {peak}"
        );
    }

    /// Prints every gun's signature (for tuning; `--nocapture`), and holds
    /// them to the gauge: each its own, each in hand.
    #[test]
    fn every_gun_has_its_own_kick_and_stays_in_hand() {
        let sigs: Vec<Signature> = guns().iter().map(|id| record(id)).collect();
        for s in &sigs {
            println!("{}", s.line());
        }
        let mut pairs = Vec::new();
        for (i, a) in sigs.iter().enumerate() {
            for b in &sigs[i + 1..] {
                pairs.push((a.apart(b), &a.id, &b.id, a.distance(b)));
            }
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (d, a, b, rms) in pairs.iter().take(10) {
            println!("{a:<13} {b:<13} {d:>6.1} (trace RMS {rms:.1})");
        }
        let close: Vec<String> = pairs
            .iter()
            .filter(|p| p.0 < DISTINCT)
            .map(|(d, a, b, _)| format!("{a} ~ {b}: {d:.1}"))
            .collect();
        let mut wild = Vec::new();
        for s in &sigs {
            assert!(s.shots >= 1, "{} fired: {s:?}", s.id);
            // Every self-loader's action is heard slamming home for each
            // shot (the last may come after the half second); a gun worked
            // by hand makes no such sound.
            if s.self_loading {
                assert!(
                    s.slams + 1 >= s.shots && s.slams >= 1,
                    "{}: {} slams for {} shots",
                    s.id,
                    s.slams,
                    s.shots
                );
            } else {
                assert_eq!(s.slams, 0, "{}", s.id);
            }
            let limit = if s.hands >= 2 {
                CONTROL_RISE
            } else {
                CONTROL_RISE_ONE
            };
            if s.peak() > limit * 1000.0 {
                wild.push(format!("{}: peak {:.0} mrad", s.id, s.peak()));
            }
            let end = s.rise.last().unwrap().hypot(*s.yaw.last().unwrap());
            if s.shots == 1 && end > (SETTLED * s.peak()).max(SETTLED_FLOOR) {
                wild.push(format!("{}: {end:.0} of {:.0} mrad left", s.id, s.peak()));
            }
        }
        assert!(close.is_empty(), "too alike: {close:?}");
        assert!(wild.is_empty(), "out of hand: {wild:?}");
    }
}
