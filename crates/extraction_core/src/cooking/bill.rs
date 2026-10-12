//! What a food costs, from the ground up.
//!
//! A [`Bill`] follows a food's recipes back to the raw ingredients, adding
//! up what each step took on the way: strokes of work, ticks over heat,
//! ticks of waiting, and how many steps there were. It is the hook for
//! tuning a pipeline: price a dish from its bill, give a customer patience
//! in proportion to its time, decide whether a recipe is worth its trouble.
//! The demo prices every dish from one.

use super::food::{Food, Stage};
use super::process::Kind;
use super::recipe;
use super::stock::Stock;

/// Everything that went into one food.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bill {
    /// The raw ingredients.
    pub raw: Stock,
    /// Strokes of physical work.
    pub effort: u32,
    /// Ticks over heat.
    pub heat: u32,
    /// Ticks of waiting.
    pub time: u32,
    /// How many processes, counting each one every time it is used.
    pub steps: u32,
}

impl Bill {
    /// Add another bill to this one.
    pub fn merge(&mut self, other: &Self) {
        self.raw.merge(&other.raw);
        self.effort += other.effort;
        self.heat += other.heat;
        self.time += other.time;
        self.steps += other.steps;
    }

    /// Ticks spent on the clock, heat and waiting together.
    #[must_use]
    pub const fn clock(&self) -> u32 {
        self.heat + self.time
    }

    /// The raw ingredients' worth, at prices of the caller's choosing.
    #[must_use]
    pub fn raw_value(&self, price: impl Fn(Food) -> u32) -> u32 {
        self.raw.iter().map(|(f, n)| price(f) * n).sum()
    }
}

/// What it takes to make `food` from nothing, following the first recipe
/// that makes it at every step. A raw food's bill is itself; waste has none.
#[must_use]
pub fn bill(food: Food) -> Option<Bill> {
    match food.stage() {
        Stage::Raw => Some(Bill {
            raw: Stock::of(&[food]),
            ..Bill::default()
        }),
        Stage::Waste => None,
        Stage::Prepared | Stage::Dish => {
            let recipe = recipe::making(food).next()?;
            let mut total = Bill::default();
            for &input in recipe.inputs {
                total.merge(&bill(input)?);
            }
            match recipe.process.kind() {
                Kind::Physical => total.effort += recipe.work,
                Kind::Heat => total.heat += recipe.work,
                Kind::Time => total.time += recipe.work,
            }
            total.steps += 1;
            Some(total)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::secs;
    use super::*;

    #[test]
    fn every_dish_can_be_made_from_raw_ingredients() {
        for food in Food::ALL {
            let b = bill(food);
            assert_eq!(b.is_none(), food.stage() == Stage::Waste, "{food:?}");
            if let Some(b) = b {
                assert!(!b.raw.is_empty(), "{food:?} comes from nothing");
                for (raw, _) in b.raw.iter() {
                    assert_eq!(raw.stage(), Stage::Raw, "{food:?} bills {raw:?} as raw");
                }
            }
        }
    }

    #[test]
    fn bread_goes_all_the_way_back_to_wheat_and_water() {
        let b = bill(Food::Bread).unwrap();
        assert_eq!(b.raw, Stock::of(&[Food::Wheat, Food::Water]));
        // Grind, knead, prove, bake.
        assert_eq!(b.steps, 4);
        assert_eq!(b.effort, 8 + 8);
        assert_eq!(b.time, secs(20));
        assert_eq!(b.heat, secs(14));
        assert_eq!(b.raw_value(|f| u32::from(f == Food::Wheat) * 3), 3);
    }
}
