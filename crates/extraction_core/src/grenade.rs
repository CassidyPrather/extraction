//! Hand grenades as plain state machines (an RGD-5, an F-1).
//!
//! Like the guns, the game drives a [`Grenade`] the way hands drive the
//! real thing: the pin pulled by the other hand, the spoon held down by the
//! gripping hand, released when the grenade leaves the hand (or thumbed
//! off while it's still held: cooking it). The spoon going is what starts
//! the fuse; a grenade with its pin in never goes off, whatever happens to
//! it.
//!
//! What it does when it goes off is numbers for the game: fragments (real
//! rounds, so walls stop them and concrete turns them, by
//! [`crate::firearm_terminal`]) and a blast that falls off with distance.

use serde::{Deserialize, Serialize};

/// Static description of a grenade model, authored in `items.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrenadeDef {
    /// Seconds from the spoon leaving to the bang.
    pub fuse: f32,
    /// Fragments thrown, and each one's mass (grams) and speed (m/s).
    pub fragments: u32,
    pub fragment_g: f32,
    pub fragment_speed: f32,
    /// Damage a fragment does at full speed.
    pub fragment_damage: f32,
    /// The blast: damage at the centre, and the radius it falls to nothing.
    pub blast_damage: f32,
    pub blast_radius: f32,
}

/// Where a grenade is in its short life.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fuse {
    /// Pin in: safe, whatever happens.
    Pinned,
    /// Pin out, the spoon held down by a hand.
    Held,
    /// The spoon's gone: seconds left.
    Burning(f32),
    /// Gone off.
    Spent,
}

/// Something that happened, for the game to show, sound and do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    PinPulled,
    /// The spoon flew: the fuse is lit.
    SpoonOff,
    Exploded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grenade {
    pub def: GrenadeDef,
    pub fuse: Fuse,
}

impl Grenade {
    pub fn new(def: GrenadeDef) -> Self {
        Self {
            def,
            fuse: Fuse::Pinned,
        }
    }

    /// The other hand pulls the ring. Only a held grenade can lose its
    /// pin (the hand on it holds the spoon down).
    pub fn pull_pin(&mut self, held: bool) -> Vec<Event> {
        if self.fuse != Fuse::Pinned || !held {
            return Vec::new();
        }
        self.fuse = Fuse::Held;
        vec![Event::PinPulled]
    }

    /// The spoon is let go: thrown, dropped or thumbed off. With the pin in
    /// it stays put and nothing happens.
    pub fn release_spoon(&mut self) -> Vec<Event> {
        if self.fuse != Fuse::Held {
            return Vec::new();
        }
        self.fuse = Fuse::Burning(self.def.fuse);
        vec![Event::SpoonOff]
    }

    /// Advances the fuse.
    pub fn update(&mut self, dt: f32) -> Vec<Event> {
        if let Fuse::Burning(t) = self.fuse {
            let left = t - dt;
            if left <= 0.0 {
                self.fuse = Fuse::Spent;
                return vec![Event::Exploded];
            }
            self.fuse = Fuse::Burning(left);
        }
        Vec::new()
    }

    pub fn live(&self) -> bool {
        matches!(self.fuse, Fuse::Held | Fuse::Burning(_))
    }
}

/// Blast damage at `distance` metres in the open: full at the heart,
/// falling with the square to nothing at the radius.
pub fn blast_at(def: &GrenadeDef, distance: f32) -> f32 {
    let k = (1.0 - distance / def.blast_radius.max(0.1)).max(0.0);
    def.blast_damage * k * k
}

/// A fragment's damage at `speed` (it falls with the energy left).
pub fn fragment_damage(def: &GrenadeDef, speed: f32) -> f32 {
    let k = speed / def.fragment_speed.max(1.0);
    def.fragment_damage * k * k
}

/// Directions for `n` fragments spread evenly over the sphere (a golden
/// spiral, turned by `twist` so two grenades don't throw the same
/// pattern).
pub fn spray(n: u32, twist: f32) -> Vec<[f32; 3]> {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = golden * i as f32 + twist;
            [r * a.cos(), y, r * a.sin()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgd5() -> GrenadeDef {
        GrenadeDef {
            fuse: 3.6,
            fragments: 120,
            fragment_g: 0.5,
            fragment_speed: 1100.0,
            fragment_damage: 22.0,
            blast_damage: 120.0,
            blast_radius: 5.0,
        }
    }

    /// The pin in, nothing sets it off: dropped, thrown, left for a minute.
    #[test]
    fn a_pinned_grenade_never_goes_off() {
        let mut g = Grenade::new(rgd5());
        assert!(g.release_spoon().is_empty());
        for _ in 0..6000 {
            assert!(g.update(0.01).is_empty());
        }
        assert_eq!(g.fuse, Fuse::Pinned);
        // Nor can the pin come out of one lying on the ground.
        assert!(g.pull_pin(false).is_empty());
    }

    /// Pin out and the spoon held: safe as long as the hand holds it. Let
    /// go and it goes off after the fuse, not before.
    #[test]
    fn the_spoon_starts_the_fuse() {
        let mut g = Grenade::new(rgd5());
        assert_eq!(g.pull_pin(true), vec![Event::PinPulled]);
        for _ in 0..1000 {
            assert!(g.update(0.01).is_empty(), "held: safe");
        }
        assert_eq!(g.release_spoon(), vec![Event::SpoonOff]);
        let mut t = 0.0;
        loop {
            t += 0.01;
            if g.update(0.01).contains(&Event::Exploded) {
                break;
            }
            assert!(t < 10.0);
        }
        assert!((t - 3.6).abs() < 0.02, "{t}");
        assert!(g.update(0.01).is_empty(), "once");
    }

    #[test]
    fn harm_falls_off_with_distance() {
        let d = rgd5();
        assert!(blast_at(&d, 0.5) > blast_at(&d, 2.0));
        assert_eq!(blast_at(&d, 5.0), 0.0);
        assert!(fragment_damage(&d, 1100.0) > fragment_damage(&d, 300.0) * 10.0);
        // The spray covers the sphere: as many fragments up as down.
        let s = spray(120, 0.3);
        let up = s.iter().filter(|v| v[1] > 0.0).count();
        assert_eq!(up, 60);
    }
}
