//! The recipe table: which inputs, under which process, make what.
//!
//! Inputs are a multiset — order does not matter, duplicates count — and a
//! recipe makes exactly one of its output, so a food's cost from the ground
//! up (its [`bill`](super::bill())) is always a whole number of everything.
//!
//! `work` is strokes for a physical process and ticks for the others. Tune
//! here; the tests below keep the table well-formed (every made food has a
//! recipe, no two recipes are ambiguous, nothing cooks into itself).

use super::food::Food::{
    self, Batter, BeanStew, Beans, BoiledEgg, Bread, Butter, Cake, Carrot, ChoppedCarrot,
    ChoppedOnion, ChoppedTomato, Custard, Dough, Egg, Flatbread, Flour, FriedEgg, FriedRice,
    HotWater, Milk, MilkTea, Omelette, Onion, Pancake, Rice, RicePudding, RisenDough, Salad, Salt,
    SoakedBeans, Soup, Spice, SteamedRice, Sugar, Tea, TeaLeaf, Tomato, Water, Wheat,
};
use super::process::Process::{
    self, Bake, Boil, Chop, Churn, Fry, Grind, Knead, Mix, Prove, Soak, Steep,
};
use super::secs;

/// One way to make one food.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub process: Process,
    /// What goes in. Order does not matter.
    pub inputs: &'static [Food],
    /// What comes out, one of it.
    pub output: Food,
    /// Strokes (physical) or ticks (heat and time) to finish.
    pub work: u32,
}

const fn r(process: Process, inputs: &'static [Food], output: Food, work: u32) -> Recipe {
    Recipe {
        process,
        inputs,
        output,
        work,
    }
}

/// Every recipe.
pub const RECIPES: &[Recipe] = &[
    // Physical: strokes of work.
    r(Chop, &[Carrot], ChoppedCarrot, 4),
    r(Chop, &[Onion], ChoppedOnion, 4),
    r(Chop, &[Tomato], ChoppedTomato, 3),
    r(Grind, &[Wheat], Flour, 8),
    r(Churn, &[Milk], Butter, 10),
    r(Knead, &[Flour, Water], Dough, 8),
    r(Mix, &[Flour, Egg, Milk], Batter, 6),
    r(Mix, &[ChoppedTomato, ChoppedCarrot, Salt], Salad, 4),
    // Time: ticks, and it waits for you.
    r(Prove, &[Dough], RisenDough, secs(20)),
    r(Soak, &[Beans, Water], SoakedBeans, secs(25)),
    r(Steep, &[HotWater, TeaLeaf], Tea, secs(6)),
    r(Steep, &[HotWater, TeaLeaf, Milk, Sugar], MilkTea, secs(8)),
    // Heat: ticks, and it burns.
    r(Boil, &[Water], HotWater, secs(5)),
    r(Boil, &[Water, Egg], BoiledEgg, secs(8)),
    r(Boil, &[Water, Rice], SteamedRice, secs(12)),
    r(
        Boil,
        &[Water, ChoppedCarrot, ChoppedOnion, Salt],
        Soup,
        secs(15),
    ),
    r(
        Boil,
        &[SoakedBeans, ChoppedTomato, Spice],
        BeanStew,
        secs(18),
    ),
    r(Boil, &[Milk, Egg, Sugar], Custard, secs(10)),
    r(Boil, &[Milk, Rice, Sugar], RicePudding, secs(14)),
    r(Fry, &[Egg], FriedEgg, secs(5)),
    r(Fry, &[Egg, ChoppedOnion, Butter], Omelette, secs(7)),
    r(Fry, &[Batter, Butter], Pancake, secs(6)),
    r(Fry, &[SteamedRice, Egg, ChoppedOnion], FriedRice, secs(8)),
    r(Bake, &[Dough], Flatbread, secs(8)),
    r(Bake, &[RisenDough], Bread, secs(14)),
    r(Bake, &[Batter, Sugar, Butter], Cake, secs(16)),
];

/// Whether two lists hold the same foods the same number of times.
#[must_use]
pub fn same_foods(a: &[Food], b: &[Food]) -> bool {
    a.len() == b.len() && Food::ALL.iter().all(|f| count(a, *f) == count(b, *f))
}

/// Whether every food in `part`, counted, is also in `whole`.
#[must_use]
pub fn within(part: &[Food], whole: &[Food]) -> bool {
    part.iter().all(|f| count(part, *f) <= count(whole, *f))
}

fn count(list: &[Food], food: Food) -> usize {
    list.iter().filter(|f| **f == food).count()
}

/// The recipe `process` follows for exactly these `contents`, if any.
#[must_use]
pub fn find(process: Process, contents: &[Food]) -> Option<&'static Recipe> {
    RECIPES
        .iter()
        .find(|r| r.process == process && same_foods(r.inputs, contents))
}

/// Every recipe that makes `food`.
pub fn making(food: Food) -> impl Iterator<Item = &'static Recipe> {
    RECIPES.iter().filter(move |r| r.output == food)
}

/// Every recipe that takes `food` in.
pub fn using(food: Food) -> impl Iterator<Item = &'static Recipe> {
    RECIPES.iter().filter(move |r| r.inputs.contains(&food))
}

/// Recipes among `processes` that `contents` are on the way to: everything
/// in `contents` is an input, and the rest could still be added. For
/// showing a cook what a half-filled pot could become.
pub fn toward<'a>(
    processes: &'a [Process],
    contents: &'a [Food],
) -> impl Iterator<Item = &'static Recipe> + 'a {
    RECIPES
        .iter()
        .filter(move |r| processes.contains(&r.process) && within(contents, r.inputs))
}

/// What `recipe` still needs once `contents` are in.
#[must_use]
pub fn missing(recipe: &Recipe, contents: &[Food]) -> Vec<Food> {
    let mut left: Vec<Food> = contents.to_vec();
    let mut out = Vec::new();
    for &f in recipe.inputs {
        if let Some(at) = left.iter().position(|g| *g == f) {
            left.swap_remove(at);
        } else {
            out.push(f);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::food::Stage;
    use super::*;

    #[test]
    fn every_made_food_has_a_recipe_and_nothing_else_does() {
        for food in Food::ALL {
            let made = making(food).count() > 0;
            let should = matches!(food.stage(), Stage::Prepared | Stage::Dish);
            assert_eq!(made, should, "{food:?} ({:?})", food.stage());
        }
    }

    #[test]
    fn every_raw_and_prepared_food_is_used_by_something() {
        for food in Food::ALL {
            if matches!(food.stage(), Stage::Raw | Stage::Prepared) {
                assert!(using(food).count() > 0, "nothing uses {food:?}");
            }
        }
    }

    #[test]
    fn no_two_recipes_of_a_process_take_the_same_inputs() {
        for (i, a) in RECIPES.iter().enumerate() {
            for b in &RECIPES[i + 1..] {
                assert!(
                    !(a.process == b.process && same_foods(a.inputs, b.inputs)),
                    "{:?} and {:?} are ambiguous",
                    a.output,
                    b.output
                );
            }
        }
    }

    #[test]
    fn no_recipe_takes_in_what_it_makes_or_waste() {
        for r in RECIPES {
            assert!(!r.inputs.is_empty(), "{:?} takes nothing", r.output);
            assert!(!r.inputs.contains(&r.output), "{:?} needs itself", r.output);
            assert!(r.work > 0, "{:?} takes no work", r.output);
            for f in r.inputs {
                assert_ne!(f.stage(), Stage::Waste, "{:?} wants {f:?}", r.output);
            }
        }
    }

    #[test]
    fn matching_ignores_order() {
        let found = find(Boil, &[Salt, ChoppedOnion, Water, ChoppedCarrot]);
        assert_eq!(found.map(|r| r.output), Some(Soup));
        assert!(find(Boil, &[Water, Water]).is_none());
        assert!(find(Fry, &[Water]).is_none());
    }

    #[test]
    fn a_half_filled_pot_knows_where_it_is_going() {
        let ways: Vec<Food> = toward(&[Boil], &[Water, Salt]).map(|r| r.output).collect();
        assert_eq!(ways, vec![Soup]);
        let soup = find(Boil, &[Water, ChoppedCarrot, ChoppedOnion, Salt]).unwrap();
        assert_eq!(
            missing(soup, &[Salt, Water]),
            vec![ChoppedCarrot, ChoppedOnion]
        );
        // An empty pot could be anything it boils.
        assert_eq!(
            toward(&[Boil], &[]).count(),
            RECIPES.iter().filter(|r| r.process == Boil).count()
        );
    }
}
