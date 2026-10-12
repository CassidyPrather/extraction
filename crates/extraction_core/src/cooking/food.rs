//! Every food there is.
//!
//! One flat enum, because a flat enum is the easiest thing to copy, prune,
//! extend and index an array by. A food's [`Stage`] says where it sits in
//! the pipeline; its [`Group`]s say what sort of thing it is, for a game
//! that wants to store milk in the cold or let a crow steal crops.
//!
//! The raw ingredients are what a game has to supply from outside: grown,
//! gathered, bought, milked. Everything else is made by a
//! [`Recipe`](super::Recipe) from them.

/// One food.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Food {
    // Raw: what the world provides.
    Water,
    Milk,
    Egg,
    Carrot,
    Onion,
    Tomato,
    Wheat,
    Rice,
    Beans,
    Sugar,
    TeaLeaf,
    Spice,
    Salt,

    // Prepared: made, and only good for making something else.
    Flour,
    Dough,
    RisenDough,
    Batter,
    Butter,
    HotWater,
    ChoppedCarrot,
    ChoppedOnion,
    ChoppedTomato,
    SoakedBeans,

    // Dishes: made, and good to eat.
    Tea,
    MilkTea,
    Salad,
    BoiledEgg,
    SteamedRice,
    Soup,
    BeanStew,
    Custard,
    RicePudding,
    FriedEgg,
    Omelette,
    Pancake,
    FriedRice,
    Flatbread,
    Bread,
    Cake,

    // Waste: what happens when it goes wrong.
    /// Whatever a process makes of inputs no recipe wants.
    Mush,
    /// What heat makes of anything left on it too long.
    Charcoal,
}

/// Where a food sits in the pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Supplied by the world; no recipe makes it.
    Raw,
    /// Made, and only useful as an input.
    Prepared,
    /// Made, and worth eating or selling.
    Dish,
    /// A failure. No recipe makes it on purpose, and none wants it.
    Waste,
}

/// What sort of thing a food is. A food can be several.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// A liquid other things are cooked in.
    FluidBase,
    /// From an animal.
    AnimalProduce,
    /// Grown in the ground.
    Crop,
    /// A cereal crop.
    Grain,
    /// A vegetable.
    Vegetable,
    /// Used a pinch at a time.
    Seasoning,
    /// Dug or dried, not grown.
    Mineral,
    /// Sweet.
    Sweet,
    /// A drink.
    Drink,
    /// Baked.
    Baked,
}

impl Food {
    /// Every food, in declaration order. `Food::ALL[f.index()] == f`.
    pub const ALL: [Self; 41] = [
        Self::Water,
        Self::Milk,
        Self::Egg,
        Self::Carrot,
        Self::Onion,
        Self::Tomato,
        Self::Wheat,
        Self::Rice,
        Self::Beans,
        Self::Sugar,
        Self::TeaLeaf,
        Self::Spice,
        Self::Salt,
        Self::Flour,
        Self::Dough,
        Self::RisenDough,
        Self::Batter,
        Self::Butter,
        Self::HotWater,
        Self::ChoppedCarrot,
        Self::ChoppedOnion,
        Self::ChoppedTomato,
        Self::SoakedBeans,
        Self::Tea,
        Self::MilkTea,
        Self::Salad,
        Self::BoiledEgg,
        Self::SteamedRice,
        Self::Soup,
        Self::BeanStew,
        Self::Custard,
        Self::RicePudding,
        Self::FriedEgg,
        Self::Omelette,
        Self::Pancake,
        Self::FriedRice,
        Self::Flatbread,
        Self::Bread,
        Self::Cake,
        Self::Mush,
        Self::Charcoal,
    ];

    /// How many foods there are.
    pub const COUNT: usize = Self::ALL.len();

    /// Position in [`Food::ALL`], for indexing arrays of per-food data.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Where it sits in the pipeline.
    #[must_use]
    pub const fn stage(self) -> Stage {
        use Food::{
            Batter, Beans, Butter, Carrot, Charcoal, ChoppedCarrot, ChoppedOnion, ChoppedTomato,
            Dough, Egg, Flour, HotWater, Milk, Mush, Onion, Rice, RisenDough, Salt, SoakedBeans,
            Spice, Sugar, TeaLeaf, Tomato, Water, Wheat,
        };
        match self {
            Water | Milk | Egg | Carrot | Onion | Tomato | Wheat | Rice | Beans | Sugar
            | TeaLeaf | Spice | Salt => Stage::Raw,
            Flour | Dough | RisenDough | Batter | Butter | HotWater | ChoppedCarrot
            | ChoppedOnion | ChoppedTomato | SoakedBeans => Stage::Prepared,
            Mush | Charcoal => Stage::Waste,
            _ => Stage::Dish,
        }
    }

    /// What sort of thing it is.
    #[must_use]
    pub const fn groups(self) -> &'static [Group] {
        use Group::{
            AnimalProduce, Baked, Crop, Drink, FluidBase, Grain, Mineral, Seasoning, Sweet,
            Vegetable,
        };
        match self {
            Self::Water | Self::HotWater => &[FluidBase],
            Self::Milk => &[FluidBase, AnimalProduce],
            Self::Egg
            | Self::BoiledEgg
            | Self::FriedEgg
            | Self::Omelette
            | Self::Batter
            | Self::Butter => &[AnimalProduce],
            Self::Carrot | Self::Onion | Self::Tomato => &[Crop, Vegetable],
            Self::ChoppedCarrot | Self::ChoppedOnion | Self::ChoppedTomato | Self::Salad => {
                &[Vegetable]
            }
            Self::Wheat | Self::Rice => &[Crop, Grain],
            Self::Flour | Self::Dough | Self::RisenDough | Self::SteamedRice | Self::FriedRice => {
                &[Grain]
            }
            Self::Beans | Self::TeaLeaf => &[Crop],
            Self::Sugar => &[Crop, Seasoning, Sweet],
            Self::Spice => &[Crop, Seasoning],
            Self::Salt => &[Mineral, Seasoning],
            Self::Tea | Self::MilkTea => &[Drink],
            Self::Custard | Self::RicePudding => &[Sweet],
            Self::Pancake | Self::Flatbread | Self::Bread => &[Baked, Grain],
            Self::Cake => &[Baked, Sweet],
            Self::SoakedBeans | Self::Soup | Self::BeanStew | Self::Mush | Self::Charcoal => &[],
        }
    }

    /// Whether it belongs to `group`.
    #[must_use]
    pub fn is(self, group: Group) -> bool {
        self.groups().contains(&group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_in_declaration_order() {
        for (i, food) in Food::ALL.iter().enumerate() {
            assert_eq!(food.index(), i, "{food:?} is out of place in ALL");
        }
    }

    #[test]
    fn milk_is_both_a_fluid_base_and_animal_produce() {
        assert!(Food::Milk.is(Group::FluidBase));
        assert!(Food::Milk.is(Group::AnimalProduce));
        assert!(!Food::Water.is(Group::AnimalProduce));
    }

    #[test]
    fn every_stage_has_members() {
        for stage in [Stage::Raw, Stage::Prepared, Stage::Dish, Stage::Waste] {
            assert!(
                Food::ALL.iter().any(|f| f.stage() == stage),
                "nothing is {stage:?}"
            );
        }
    }
}
