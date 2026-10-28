//! What the Zone's hazards look like on a machine they strike (Robot
//! minds' `hazards`: the table of what each does, and `HazardStruck` when
//! it does). One mapping ([`mark`]) every look reads, as `damage::tell` is
//! for wounds: each hazard its own burst on the machine, so at 10 m in a
//! headset you can tell what got it, again each second it's in it.
//!
//! - **lift**: dust blown out from under it, and a drone thrown tumbles
//!   ([`TUMBLE`]);
//! - **burner**: embers and soot;
//! - **snare**: a bolt's arcs crawling over it, and smoke;
//! - **well**: a dark haze dragged in round it; at the heart, crushed,
//!   sparks;
//! - **mirror**: a shimmer, and its eye stutters (`visuals`) and its turret
//!   wanders (`anim`) while it's dazzled;
//! - **hush**: a still, dark haze (nothing heard, nothing more to see);
//! - **gas**: a yellow-green cloud, and the flash's embers;
//! - **mine**: sparks and soot;
//! - **EMP**: arcs, nothing else;
//! - **frost**: ice glittering off it and a frost breath; and its gait goes
//!   heavy while it's hampered (`anim`), as in a well or a lift winding up;
//! - **heat**: embers in a haze.
//!
//! The bursts are `fx::hazard_bursts`; this module is the mapping, and the
//! drone's tumble.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::hazards::{Effect, Hazard, HazardStruck};

/// Streaks thrown off it (each a spark's flight).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bits {
    /// Hot metal: a crush, a blast.
    Sparks,
    /// Electric: a snare's bolt, an EMP.
    Arcs,
    /// Orange embers: fire, gas, heat.
    Embers,
    /// Ice: a frost.
    Crystals,
}

/// A cloud on it (puffs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cloud {
    Smoke,
    Soot,
    Dust,
    /// Yellow-green: gas.
    Gas,
    /// White: a frost's breath.
    Frost,
    /// A pale glint in the air: a mirror's fold, heat haze.
    Shimmer,
    /// A dark, still haze: a hush.
    Still,
}

/// What a hazard shows on a machine it strikes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub bits: Option<(Bits, u8)>,
    pub cloud: Option<(Cloud, u8)>,
    /// A drone thrown tumbles this fast (rad/s); others stagger by their
    /// gait (`hazards` dazes them).
    pub spin: f32,
}

impl Mark {
    const NONE: Mark = Mark {
        bits: None,
        cloud: None,
        spin: 0.0,
    };

    /// Whether there's anything to see.
    pub fn visible(&self) -> bool {
        self.bits.is_some_and(|(_, n)| n > 0) || self.cloud.is_some_and(|(_, n)| n > 0)
    }

    /// What tells it from the others: its bits and its cloud.
    pub fn kind(&self) -> (Option<Bits>, Option<Cloud>) {
        (self.bits.map(|b| b.0), self.cloud.map(|c| c.0))
    }
}

/// A thrown drone's tumble (rad/s).
pub const TUMBLE: f32 = 9.0;

/// What `hazard` doing `effect` shows on the machine it struck.
pub fn mark(hazard: Hazard, effect: Effect) -> Mark {
    use Bits as B;
    use Cloud as C;
    let m = |bits: Option<(Bits, u8)>, cloud: Option<(Cloud, u8)>| Mark {
        bits,
        cloud,
        spin: 0.0,
    };
    match (hazard, effect) {
        (_, Effect::Unaffected) => Mark::NONE,
        (Hazard::Lift, Effect::Thrown) => Mark {
            spin: TUMBLE,
            ..m(None, Some((C::Dust, 5)))
        },
        // Winding up: the air pulling at it.
        (Hazard::Lift, _) => m(None, Some((C::Dust, 3))),
        (Hazard::Burner, _) => m(Some((B::Embers, 10)), Some((C::Soot, 3))),
        (Hazard::Snare, _) => m(Some((B::Arcs, 18)), Some((C::Smoke, 2))),
        // At the heart, crushed.
        (Hazard::Well, Effect::Hurt) => m(Some((B::Sparks, 10)), Some((C::Dust, 2))),
        // Dragged: a dark haze pulled in round it (a lift's is pale dust).
        (Hazard::Well, _) => m(None, Some((C::Soot, 4))),
        (Hazard::Mirror, _) => m(None, Some((C::Shimmer, 4))),
        (Hazard::Hush, _) => m(None, Some((C::Still, 4))),
        (Hazard::Gas, _) => m(Some((B::Embers, 6)), Some((C::Gas, 5))),
        (Hazard::Mine, _) => m(Some((B::Sparks, 14)), Some((C::Soot, 4))),
        (Hazard::Emp, _) => m(Some((B::Arcs, 24)), None),
        (Hazard::Frost, _) => m(Some((B::Crystals, 14)), Some((C::Frost, 4))),
        (Hazard::Heat, _) => m(Some((B::Embers, 6)), Some((C::Shimmer, 2))),
    }
}

pub struct HazardLookPlugin;

impl Plugin for HazardLookPlugin {
    fn build(&self, app: &mut App) {
        // (`hazards` registers it; here too, so an app without it runs.)
        app.add_message::<HazardStruck>()
            .add_systems(FixedUpdate, tumble_thrown);
    }
}

/// A drone a lift throws tumbles: spun about a seeded axis as it goes up,
/// its flight righting it after (it's dazzled a while: `hazards`).
fn tumble_thrown(
    mut struck: MessageReader<HazardStruck>,
    mut drones: Query<(&mut AngularVelocity, Option<&super::RobotSeed>), With<super::drone::Drone>>,
) {
    for s in struck.read() {
        let spin = mark(s.hazard, s.effect).spin;
        if spin <= 0.0 {
            continue;
        }
        let Ok((mut w, seed)) = drones.get_mut(s.robot) else {
            continue;
        };
        let k = seed.map_or(0, |s| s.low()) as f32;
        let axis = Vec3::new(k.sin(), 0.35, k.cos()).normalize_or(Vec3::X);
        w.0 += axis * spin;
    }
}

#[cfg(test)]
mod tests {
    use super::super::hazards::{HAZARDS, MACHINES, effect};
    use super::*;

    /// Every hazard that does anything to any machine marks it, and what
    /// each shows tells it from the others: you can read what got it.
    #[test]
    fn every_hazard_marks_what_it_strikes_its_own_way() {
        let mut seen: Vec<(Hazard, (Option<Bits>, Option<Cloud>))> = Vec::new();
        for h in HAZARDS {
            for m in MACHINES {
                let fx = effect(m, h);
                let look = mark(h, fx);
                if fx == Effect::Unaffected {
                    assert!(!look.visible(), "{h:?} marks {m:?}, which it doesn't touch");
                    continue;
                }
                assert!(look.visible(), "{h:?} on {m:?} ({fx:?}) shows nothing");
                if !seen.iter().any(|(s, k)| *s == h && *k == look.kind()) {
                    seen.push((h, look.kind()));
                }
            }
        }
        for (i, (h, k)) in seen.iter().enumerate() {
            for (g, j) in &seen[i + 1..] {
                assert!(h == g || k != j, "{h:?} and {g:?} look alike: {k:?}");
            }
        }
    }

    /// A lift throwing a drone tumbles it; nothing else spins anything.
    #[test]
    fn only_a_thrown_drone_tumbles() {
        for h in HAZARDS {
            for m in MACHINES {
                let fx = effect(m, h);
                let spins = mark(h, fx).spin > 0.0;
                assert_eq!(
                    spins,
                    h == Hazard::Lift && fx == Effect::Thrown,
                    "{h:?} {m:?}"
                );
            }
        }
    }
}
