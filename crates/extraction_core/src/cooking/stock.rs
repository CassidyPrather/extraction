//! A count of every food: a pantry, a shopping list, a bill's ingredients.

use super::food::Food;

/// How many of each food.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stock {
    counts: [u32; Food::COUNT],
}

impl Default for Stock {
    fn default() -> Self {
        Self::new()
    }
}

impl Stock {
    /// Nothing at all.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            counts: [0; Food::COUNT],
        }
    }

    /// One of each food in `foods`, counting repeats.
    #[must_use]
    pub fn of(foods: &[Food]) -> Self {
        let mut stock = Self::new();
        for &f in foods {
            stock.add(f, 1);
        }
        stock
    }

    /// How many of `food`.
    #[must_use]
    pub const fn count(&self, food: Food) -> u32 {
        self.counts[food.index()]
    }

    pub const fn add(&mut self, food: Food, n: u32) {
        self.counts[food.index()] = self.counts[food.index()].saturating_add(n);
    }

    /// Take `n` of `food` away, or nothing if there are fewer than `n`.
    pub const fn remove(&mut self, food: Food, n: u32) -> bool {
        let have = self.counts[food.index()];
        if have < n {
            return false;
        }
        self.counts[food.index()] = have - n;
        true
    }

    /// Add every count of `other` to this one.
    pub fn merge(&mut self, other: &Self) {
        for food in Food::ALL {
            self.add(food, other.count(food));
        }
    }

    /// Whether every count in `other` is covered here.
    #[must_use]
    pub fn covers(&self, other: &Self) -> bool {
        Food::ALL.iter().all(|&f| self.count(f) >= other.count(f))
    }

    /// Take everything in `other` away, or nothing if it is not all here.
    pub fn spend(&mut self, other: &Self) -> bool {
        if !self.covers(other) {
            return false;
        }
        for food in Food::ALL {
            self.counts[food.index()] -= other.count(food);
        }
        true
    }

    /// How many things in all.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.counts.iter().sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// Every food present, with its count, in [`Food::ALL`] order.
    pub fn iter(&self) -> impl Iterator<Item = (Food, u32)> + '_ {
        Food::ALL
            .iter()
            .map(|&f| (f, self.count(f)))
            .filter(|&(_, n)| n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spending_is_all_or_nothing() {
        let mut pantry = Stock::of(&[Food::Egg, Food::Egg, Food::Milk]);
        let custard = Stock::of(&[Food::Egg, Food::Milk, Food::Sugar]);
        assert!(!pantry.spend(&custard), "spent sugar it did not have");
        assert_eq!(pantry.total(), 3, "a refused spend took something");
        pantry.add(Food::Sugar, 1);
        assert!(pantry.spend(&custard));
        assert_eq!(pantry.iter().collect::<Vec<_>>(), vec![(Food::Egg, 1)]);
    }

    #[test]
    fn removing_more_than_there_is_takes_nothing() {
        let mut stock = Stock::of(&[Food::Salt]);
        assert!(!stock.remove(Food::Salt, 2));
        assert_eq!(stock.count(Food::Salt), 1);
        assert!(stock.remove(Food::Salt, 1));
        assert!(stock.is_empty());
    }
}
