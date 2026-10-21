//! What damage looks like on a robot, part by part: one mapping every
//! body reads, so a hurt part always shows it from 20-40 m. A part's health
//! falls into a band; each band of each part's role gives its tell: what
//! comes off the wound (sparks, smoke, coolant), whether it goes dark, and
//! how the body carries it (a limp, a wobble, a sag). The bodies (`anim`,
//! `flight`, `armour`, `dirigible`), the effects (`fx`) and the eyes
//! (`visuals`) all take their cues from here.

use super::PartRole;

/// How hurt a part is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    /// Over 70%: nothing to see.
    Whole,
    /// Over 34%: it's been hit.
    Hurt,
    /// Over 0: about to go.
    Failing,
    /// Shot away or dead.
    Gone,
}

impl Band {
    pub fn of(health: f32) -> Self {
        if health <= 0.0 {
            Band::Gone
        } else if health <= 0.34 {
            Band::Failing
        } else if health <= 0.7 {
            Band::Hurt
        } else {
            Band::Whole
        }
    }
}

/// How the body carries a hurt part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    None,
    /// A leg it favours: it limps (`anim`, from the leg's health).
    Limp,
    /// A dead leg it drags or carries.
    Drag,
    /// A rotor off true: the drone wobbles and loses lift (`flight`).
    Wobble,
    /// A rotor gone: it lists and sinks.
    Sink,
    /// An eye that stutters and drops out (`visuals`).
    Gutter,
    /// A plate hanging from its last bolt (`armour`).
    Hang,
    /// A gas cell venting or gone: the envelope sags (`dirigible`).
    Sag,
}

/// What a part shows at a band: per-second rates of what comes off the
/// wound, and how it looks and moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tell {
    /// Sparks a second.
    pub sparks: f32,
    /// Smoke puffs a second.
    pub smoke: f32,
    /// Coolant drops a second (a thin pale-green spatter).
    pub coolant: f32,
    /// Its light (an eye, a lamp, a lock flare) is out.
    pub dark: bool,
    pub motion: Motion,
}

impl Tell {
    const NONE: Tell = Tell {
        sparks: 0.0,
        smoke: 0.0,
        coolant: 0.0,
        dark: false,
        motion: Motion::None,
    };

    /// Whether there's anything to see.
    pub fn visible(&self) -> bool {
        self.sparks > 0.0
            || self.smoke > 0.0
            || self.coolant > 0.0
            || self.dark
            || self.motion != Motion::None
    }
}

/// The tell for a part of this role at this health (0..1).
pub fn tell(role: PartRole, health: f32) -> Tell {
    let band = Band::of(health);
    let t = Tell::NONE;
    match (role, band) {
        (_, Band::Whole) => t,
        (PartRole::Leg(_), Band::Hurt) => Tell {
            sparks: 0.6,
            motion: Motion::Limp,
            ..t
        },
        (PartRole::Leg(_), Band::Failing) => Tell {
            sparks: 1.5,
            coolant: 2.0,
            motion: Motion::Limp,
            ..t
        },
        (PartRole::Leg(_), Band::Gone) => Tell {
            sparks: 0.5,
            motion: Motion::Drag,
            ..t
        },
        (PartRole::Rotor(_), Band::Hurt) => Tell {
            smoke: 1.2,
            motion: Motion::Wobble,
            ..t
        },
        (PartRole::Rotor(_), Band::Failing) => Tell {
            sparks: 1.5,
            smoke: 2.5,
            motion: Motion::Wobble,
            ..t
        },
        (PartRole::Rotor(_), Band::Gone) => Tell {
            smoke: 2.5,
            motion: Motion::Sink,
            ..t
        },
        (PartRole::Sensor, Band::Hurt) => Tell {
            sparks: 0.4,
            motion: Motion::Gutter,
            ..t
        },
        (PartRole::Sensor, Band::Failing) => Tell {
            sparks: 1.2,
            motion: Motion::Gutter,
            ..t
        },
        // Out for good, and its socket spits now and then.
        (PartRole::Sensor, Band::Gone) => Tell {
            sparks: 0.6,
            dark: true,
            ..t
        },
        (PartRole::Core, Band::Hurt) => Tell { sparks: 0.8, ..t },
        (PartRole::Core, Band::Failing) => Tell {
            sparks: 1.5,
            smoke: 2.0,
            coolant: 3.0,
            motion: Motion::Gutter,
            ..t
        },
        (PartRole::Core, Band::Gone) => Tell {
            smoke: 1.5,
            dark: true,
            ..t
        },
        (PartRole::Weapon, Band::Hurt) => Tell { sparks: 0.8, ..t },
        (PartRole::Weapon, Band::Failing) => Tell {
            sparks: 1.5,
            smoke: 1.0,
            ..t
        },
        (PartRole::Weapon, Band::Gone) => Tell {
            smoke: 0.8,
            dark: true,
            ..t
        },
        (PartRole::GasCell(_), Band::Hurt) => Tell { smoke: 1.5, ..t },
        (PartRole::GasCell(_), Band::Failing) => Tell {
            smoke: 3.0,
            motion: Motion::Sag,
            ..t
        },
        (PartRole::GasCell(_), Band::Gone) => Tell {
            motion: Motion::Sag,
            ..t
        },
        (PartRole::Armour(_), Band::Hurt) => Tell { sparks: 0.6, ..t },
        (PartRole::Armour(_), Band::Failing) => Tell {
            sparks: 0.8,
            motion: Motion::Hang,
            ..t
        },
        // Fallen off: what was under it shows (and it's lying there).
        (PartRole::Armour(_), Band::Gone) => Tell { sparks: 0.4, ..t },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every damageable part role, as the bodies use them.
    fn roles() -> Vec<PartRole> {
        let mut r = vec![PartRole::Core, PartRole::Sensor, PartRole::Weapon];
        for i in 0..4 {
            r.extend([
                PartRole::Rotor(i),
                PartRole::Leg(i),
                PartRole::GasCell(i),
                PartRole::Armour(i),
            ]);
        }
        r
    }

    /// Every damageable part shows its damage: at every band past whole
    /// each role has a tell, and a whole part shows nothing.
    #[test]
    fn every_hurt_part_has_a_tell() {
        for role in roles() {
            assert!(!tell(role, 1.0).visible(), "{role:?} whole shows nothing");
            for health in [0.6, 0.2, 0.0] {
                assert!(
                    tell(role, health).visible(),
                    "{role:?} at {health} shows nothing"
                );
            }
        }
    }

    /// A shot leg limps, a hit rotor wobbles and smokes, a dead sensor
    /// goes dark: the brief's own cases.
    #[test]
    fn the_cases_read() {
        assert_eq!(tell(PartRole::Leg(1), 0.5).motion, Motion::Limp);
        let rotor = tell(PartRole::Rotor(2), 0.5);
        assert_eq!(rotor.motion, Motion::Wobble);
        assert!(rotor.smoke > 0.0);
        let eye = tell(PartRole::Sensor, 0.0);
        assert!(eye.dark && eye.sparks > 0.0, "a dead eye's socket sparks");
        let core = tell(PartRole::Core, 0.2);
        assert!(core.sparks > 0.0 && core.coolant > 0.0);
    }
}
