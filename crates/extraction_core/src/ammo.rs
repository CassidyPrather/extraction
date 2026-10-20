//! What a round is loaded with, and the order rounds sit in a magazine.
//!
//! - [`Load`]: standard ball (buckshot in 12 gauge), armour-piercing (a
//!   steel core: through hard plate more, less harm to soft targets),
//!   tracer (burns a bright streak along its flight), and the 12 gauge
//!   slug.
//! - [`Loads`]: a stack of rounds by load. Only the rounds that aren't
//!   standard need remembering: the stack holds the top rounds as listed
//!   (the last one comes out first); any rounds below them are standard.
//!   A magazine thumbed full of ball has an empty stack; one with every
//!   fourth a tracer, or AP on top of ball, lists what's above the ball.

use serde::{Deserialize, Serialize};

/// What a round is loaded with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Load {
    /// Ball (full metal jacket); in 12 gauge, buckshot.
    #[default]
    Standard,
    /// Armour-piercing: a hardened steel core.
    Ap,
    /// A tracer: a burning base, seen along its path.
    Tracer,
    /// A 12 gauge slug: one heavy lead ball where buckshot is nine.
    Slug,
    /// Ball re-packed at the bench: now and then too little powder (a
    /// squib) or a tired primer (a dud).
    Repacked,
}

/// AP's cost through hard plate (armour steel, sheet, a helmet), against
/// ball's.
pub const AP_HARD: f32 = 0.5;
/// AP's harm to soft targets, against ball's (it doesn't break up).
pub const AP_SOFT: f32 = 0.85;
/// A 12 gauge slug, grams.
pub const SLUG_MASS_G: f32 = 32.0;
/// A tracer's bullet is a little lighter (the compound in its base).
pub const TRACER_MASS: f32 = 0.95;

impl Load {
    /// The bullet's mass for this load in `cartridge`, grams.
    pub fn mass_g(self, cartridge: &str) -> f32 {
        let ball = crate::firearm_terminal::bullet_mass_g(cartridge);
        match self {
            Load::Slug => SLUG_MASS_G,
            Load::Tracer => ball * TRACER_MASS,
            _ => ball,
        }
    }

    /// Harm to a soft target (a body, a robot's soft parts), against
    /// ball's.
    pub fn soft(self) -> f32 {
        match self {
            Load::Ap => AP_SOFT,
            _ => 1.0,
        }
    }

    /// The loads a cartridge comes in.
    pub fn for_cartridge(cartridge: &str) -> &'static [Load] {
        match cartridge {
            "12ga" => &[Load::Standard, Load::Slug],
            "7.62x38R" | "7.62x25" | "9x18" => &[Load::Standard],
            _ => &[Load::Standard, Load::Ap, Load::Tracer],
        }
    }
}

/// A stack of rounds by load (see the module docs): the top rounds as
/// listed, last out first; any below are [`Load::Standard`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Loads(pub Vec<Load>);

impl Loads {
    /// A round of `load` goes on top of `count` rounds already there.
    pub fn push(&mut self, count: u32, load: Load) {
        self.fit(count);
        if self.0.is_empty() && load == Load::Standard {
            return;
        }
        self.0.push(load);
    }

    /// The top round comes off a stack of `count`: its load.
    pub fn pop(&mut self, count: u32) -> Load {
        self.fit(count);
        if count == 0 {
            return Load::Standard;
        }
        let load = self.0.pop().unwrap_or_default();
        self.trim();
        load
    }

    /// The top round comes off a box or a loose round of `count` whose
    /// unlisted rounds are `base` (its own load).
    pub fn pop_or(&mut self, count: u32, base: Load) -> Load {
        let n = count as usize;
        if self.0.len() > n {
            self.0.drain(..self.0.len() - n);
        }
        if count == 0 {
            return base;
        }
        self.0.pop().unwrap_or(base)
    }

    /// The load of the round on top of a stack of `count`.
    pub fn top(&self, count: u32) -> Load {
        if count == 0 {
            return Load::Standard;
        }
        self.0.last().copied().unwrap_or_default()
    }

    /// Keeps the stack no taller than `count` rounds (if rounds were taken
    /// without saying which, they came off the bottom).
    pub fn fit(&mut self, count: u32) {
        let n = count as usize;
        if self.0.len() > n {
            self.0.drain(..self.0.len() - n);
        }
        self.trim();
    }

    /// Standard rounds at the bottom of the list are the same as unlisted.
    fn trim(&mut self) {
        let lead = self.0.iter().take_while(|l| **l == Load::Standard).count();
        self.0.drain(..lead);
    }

    /// Moves `n` rounds off the top of this stack of `count` onto `to`, a
    /// stack of `to_count`, one at a time (so they go in reversed, as when
    /// thumbing rounds from one magazine to another).
    pub fn move_to(&mut self, count: u32, to: &mut Loads, to_count: u32, n: u32) {
        for i in 0..n.min(count) {
            let l = self.pop(count - i);
            to.push(to_count + i, l);
        }
    }

    /// `n` rounds from a box of one load go on top of a stack of `count`.
    pub fn fill(&mut self, count: u32, load: Load, n: u32) {
        for i in 0..n {
            self.push(count + i, load);
        }
    }

    /// A factory-loaded magazine of `n`: every `every`-th round from the
    /// bottom a tracer (0: none).
    pub fn with_tracers(n: u32, every: u32) -> Self {
        let mut s = Self::default();
        for i in 0..n {
            let l = if every > 0 && (i + 1) % every == 0 {
                Load::Tracer
            } else {
                Load::Standard
            };
            s.push(i, l);
        }
        s
    }

    /// Whether any round in a stack of `count` is of `load`.
    pub fn any(&self, count: u32, load: Load) -> bool {
        if load == Load::Standard {
            return (self.0.len() as u32) < count || self.0.contains(&Load::Standard);
        }
        self.0.contains(&load)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thumb in ball, then AP on top: the AP comes out first, then the
    /// ball; the stack only ever lists what isn't ball.
    #[test]
    fn last_in_first_out_and_ball_is_unlisted() {
        let mut s = Loads::default();
        s.fill(0, Load::Standard, 5);
        assert!(s.0.is_empty());
        s.fill(5, Load::Ap, 2);
        assert_eq!(s.0, vec![Load::Ap, Load::Ap]);
        assert_eq!(s.pop(7), Load::Ap);
        assert_eq!(s.pop(6), Load::Ap);
        assert_eq!(s.pop(5), Load::Standard);
        assert!(s.0.is_empty());
    }

    /// A tracer every fourth: rounds 4 and 8 from the bottom of an eight,
    /// so the fifth shot is the first tracer (the top round is the 8th).
    #[test]
    fn every_fourth_a_tracer() {
        let mut s = Loads::with_tracers(8, 4);
        let shots: Vec<Load> = (0..8).rev().map(|i| s.pop(i + 1)).collect();
        assert_eq!(shots[0], Load::Tracer);
        assert_eq!(shots[4], Load::Tracer);
        assert_eq!(
            shots.iter().filter(|l| **l == Load::Tracer).count(),
            2,
            "{shots:?}"
        );
    }

    /// Rounds taken without saying which leave the listed top intact.
    #[test]
    fn untracked_takes_come_off_the_bottom() {
        let mut s = Loads::default();
        s.fill(0, Load::Ap, 3);
        s.fit(2);
        assert_eq!(s.0.len(), 2);
        assert_eq!(s.top(2), Load::Ap);
    }
}
