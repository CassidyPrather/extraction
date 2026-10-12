//! What can be done to food.
//!
//! Every process is one of three kinds, and the kind decides how a
//! [`Station`](super::Station) running it behaves:
//!
//! | Kind | Driven by | Left alone once done |
//! | --- | --- | --- |
//! | [`Kind::Physical`] | work: one [`Station::work`](super::Station::work) per stroke | waits |
//! | [`Kind::Heat`] | the clock: one [`Station::tick`](super::Station::tick) per tick | burns |
//! | [`Kind::Time`] | the clock | waits |
//!
//! That is the whole tension in three rows: physical processes cost the
//! cook's attention, heat costs nothing until it costs everything, and time
//! is free but slow.

use super::recipe::RECIPES;

/// One thing that can be done to food.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Process {
    // Physical.
    /// Cut on a board.
    Chop,
    /// Ground between stones.
    Grind,
    /// Beaten until it turns.
    Churn,
    /// Worked by hand.
    Knead,
    /// Stirred, whisked or tossed together.
    Mix,
    // Heat.
    /// In water, in a pot.
    Boil,
    /// In fat, in a pan.
    Fry,
    /// In dry heat, in an oven.
    Bake,
    // Time.
    /// Left somewhere warm to rise.
    Prove,
    /// Left in water.
    Soak,
    /// Left in hot water.
    Steep,
}

/// The three kinds of process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Cutting, kneading: costs work.
    Physical,
    /// Boiling, frying, baking: costs time, and burns.
    Heat,
    /// Rising, soaking, steeping: costs time, and waits.
    Time,
}

/// What moves a process along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Drive {
    /// Strokes of work.
    Effort,
    /// Ticks of the clock.
    Clock,
}

impl Kind {
    /// What moves it along.
    #[must_use]
    pub const fn drive(self) -> Drive {
        match self {
            Self::Physical => Drive::Effort,
            Self::Heat | Self::Time => Drive::Clock,
        }
    }

    /// Whether a finished result spoils if it is not taken out.
    #[must_use]
    pub const fn burns(self) -> bool {
        matches!(self, Self::Heat)
    }
}

impl Process {
    /// Every process.
    pub const ALL: [Self; 11] = [
        Self::Chop,
        Self::Grind,
        Self::Churn,
        Self::Knead,
        Self::Mix,
        Self::Boil,
        Self::Fry,
        Self::Bake,
        Self::Prove,
        Self::Soak,
        Self::Steep,
    ];

    /// Its kind.
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::Chop | Self::Grind | Self::Churn | Self::Knead | Self::Mix => Kind::Physical,
            Self::Boil | Self::Fry | Self::Bake => Kind::Heat,
            Self::Prove | Self::Soak | Self::Steep => Kind::Time,
        }
    }

    /// What moves it along.
    #[must_use]
    pub const fn drive(self) -> Drive {
        self.kind().drive()
    }

    /// The most inputs any of its recipes takes: how much a station running
    /// it needs to hold. Derived from the table, so adding a bigger recipe
    /// makes room for it.
    #[must_use]
    pub fn capacity(self) -> usize {
        RECIPES
            .iter()
            .filter(|r| r.process == self)
            .map(|r| r.inputs.len())
            .max()
            .unwrap_or(1)
    }

    /// How long it takes to make [`Food::Mush`](super::Food::Mush) of inputs
    /// no recipe wants: strokes for physical processes, ticks for the rest.
    /// Long enough to notice, short enough not to punish curiosity much.
    #[must_use]
    pub const fn mush_work(self) -> u32 {
        match self.kind() {
            Kind::Physical => 4,
            Kind::Heat | Kind::Time => super::secs(6),
        }
    }

    /// For heat: ticks a finished dish survives before it burns.
    #[must_use]
    pub const fn grace(self) -> u32 {
        match self {
            Self::Boil => super::secs(12),
            Self::Fry => super::secs(6),
            Self::Bake => super::secs(9),
            _ => u32::MAX,
        }
    }

    /// For heat: ticks before burning when [`Event::Smoking`](super::Event)
    /// warns that it is about to.
    #[must_use]
    pub const fn warning(self) -> u32 {
        let grace = self.grace();
        if grace == u32::MAX { 0 } else { grace / 2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_burns_and_nothing_else_does() {
        for p in Process::ALL {
            assert_eq!(p.kind().burns(), p.kind() == Kind::Heat, "{p:?}");
            assert_eq!(p.grace() < u32::MAX, p.kind() == Kind::Heat, "{p:?}");
        }
    }

    #[test]
    fn work_drives_physical_and_the_clock_drives_the_rest() {
        assert_eq!(Process::Chop.drive(), Drive::Effort);
        assert_eq!(Process::Bake.drive(), Drive::Clock);
        assert_eq!(Process::Steep.drive(), Drive::Clock);
    }

    #[test]
    fn every_process_holds_at_least_one_thing() {
        for p in Process::ALL {
            assert!(p.capacity() >= 1, "{p:?}");
        }
    }
}
