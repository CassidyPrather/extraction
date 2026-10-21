//! Affordances: can someone new tell, in the headset, what can be taken,
//! pressed, pulled, turned or climbed? Every kind of interactable is listed
//! here with the cues it gives (`docs/LEGIBILITY.md`, "Affordances"), and
//! tests hold each to them:
//!
//! - at least two cues, at least one seen from across a room (a contrasting
//!   colour, a light, a shape you know), not only felt or shown on touch;
//! - the part you put your hand to at least [`MIN_PART_DEG`] across from
//!   [`SEEN_FROM`];
//! - its claims true in the game: a kind that says the near ring shows
//!   round it is on the item layer, where the hand's hover finds it.
//!
//! Every interactable in every valley, the yard and the camp is classified
//! by its components ([`kind_of`]); one that isn't a known kind fails, and
//! a new `Pressable` doesn't compile until it has a kind.

use crate::raid::truck::Pressable;

/// Where an interactable should read from: across a small room.
pub const SEEN_FROM: f32 = 1.5;
/// The part you take hold of, as an angle from [`SEEN_FROM`] (degrees):
/// about 14 eye pixels at the auto render scale.
pub const MIN_PART_DEG: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    PushButton,
    /// A card on a wall or board, squeezed (or its button pushed).
    Card,
    /// Paper to take up and read (`interact::read`).
    Readable,
    /// A note pinned on the cork board: taken down, pinned back.
    PinnedNote,
    BellPull,
    BenchKnob,
    BenchLever,
    /// A pull lever on a wall (the furnace's).
    WallLever,
    /// Run-controls with a look of their own: a breaker, a valve wheel, a
    /// pump, a mine tub.
    FixtureControl,
    KitchenControl,
    Door,
    FoldedCover,
    Ladder,
    Rope,
    /// Anything loose you pick up.
    LooseItem,
    /// A safehouse fixture (stove, bed, radio) pressed for its upgrade.
    SafehouseFixture,
    /// A mine tub on its rails, pushed.
    MineCart,
    /// Your own pack, reached for over the shoulder.
    OwnPack,
}

/// A cue an interactable gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Painted or made to stand out from what it's on (the colour, srgb).
    Contrast([u8; 3]),
    /// Glows (a screen, an emissive cap or knob).
    Lit,
    /// A shape that says what to do (a handle, a knob, a rung).
    Shape(&'static str),
    /// The ring round it when an empty hand is near enough to take it, or
    /// points at it from afar (`render::props::far_cue`).
    Ring,
    /// The controller ticks on hover and clicks on use.
    Haptic,
}

impl Cue {
    /// Seen from across a room, before you touch it.
    pub fn at_a_distance(self) -> bool {
        matches!(self, Cue::Contrast(_) | Cue::Lit | Cue::Shape(_))
    }
}

pub struct Spec {
    pub cues: &'static [Cue],
    /// The size of the part you put your hand to (m).
    pub part: f32,
}

impl Kind {
    pub const ALL: [Kind; 18] = [
        Kind::PushButton,
        Kind::Card,
        Kind::Readable,
        Kind::PinnedNote,
        Kind::BellPull,
        Kind::BenchKnob,
        Kind::BenchLever,
        Kind::WallLever,
        Kind::FixtureControl,
        Kind::KitchenControl,
        Kind::Door,
        Kind::FoldedCover,
        Kind::Ladder,
        Kind::Rope,
        Kind::LooseItem,
        Kind::SafehouseFixture,
        Kind::MineCart,
        Kind::OwnPack,
    ];

    pub fn spec(self) -> Spec {
        use Cue::*;
        let (cues, part): (&'static [Cue], f32) = match self {
            // Amber cap, faintly lit, 4.4 cm, standing proud of the wall.
            Kind::PushButton => (
                &[Contrast([217, 115, 20]), Lit, Shape("a round cap"), Haptic],
                0.044,
            ),
            Kind::Card => (&[Contrast([217, 207, 176]), Ring, Haptic], 0.26),
            Kind::Readable => (&[Contrast([214, 207, 184]), Ring, Haptic], 0.24),
            Kind::PinnedNote => (&[Contrast([217, 207, 176]), Ring, Haptic], 0.15),
            // A red toggle on its rope (it was rope-coloured).
            Kind::BellPull => (
                &[
                    Contrast([173, 20, 13]),
                    Shape("a toggle on a rope"),
                    Ring,
                    Haptic,
                ],
                0.11,
            ),
            Kind::BenchKnob => (
                &[Contrast([140, 20, 15]), Shape("a knob"), Ring, Haptic],
                0.07,
            ),
            Kind::BenchLever => (&[Shape("a lever"), Lit, Ring, Haptic], 0.16),
            // Yellow plate, red knob (they had the bench's black stub).
            Kind::WallLever => (
                &[
                    Contrast([204, 153, 20]),
                    Shape("a pull lever"),
                    Ring,
                    Haptic,
                ],
                0.3,
            ),
            Kind::FixtureControl => (
                &[
                    Shape("a breaker, a valve wheel, a pump handle"),
                    Ring,
                    Haptic,
                ],
                0.3,
            ),
            Kind::KitchenControl => (&[Shape("a knob, a knife, a spoon"), Ring, Haptic], 0.044),
            // Handle and latch plate; a squeeze anywhere on the leaf.
            Kind::Door => (&[Shape("a door with a handle"), Haptic], 0.9),
            Kind::FoldedCover => (&[Shape("a folding shield"), Haptic], 1.0),
            Kind::Ladder => (&[Shape("rungs"), Ring, Haptic], 0.4),
            // Climbing-rope orange, 3.2 cm (it was tan, 2.2 cm).
            Kind::Rope => (
                &[Contrast([199, 84, 20]), Shape("a rope"), Ring, Haptic],
                2.0 * crate::gadgets::visuals::ROPE_R,
            ),
            Kind::LooseItem => (&[Shape("its own model"), Ring, Haptic], 0.05),
            Kind::SafehouseFixture => (&[Shape("a stove, a bed, a radio"), Ring, Haptic], 0.25),
            // On the World layer (it rolls and blocks): no ring; a tub on
            // rails says push.
            Kind::MineCart => (&[Shape("a tub on rails"), Haptic], 0.8),
            // Behind you: taught in the yard, felt and heard.
            Kind::OwnPack => (
                &[Shape("your pack, seen when you take it off"), Haptic],
                0.4,
            ),
        };
        Spec { cues, part }
    }
}

/// What a `Pressable` is. Exhaustive: a new one needs a kind.
pub fn pressable_kind(p: Pressable, readable: bool, fixture: bool, on_bench: bool) -> Kind {
    match p {
        Pressable::BellPull => Kind::BellPull,
        Pressable::Door => Kind::Door,
        Pressable::BenchSelect => Kind::BenchKnob,
        Pressable::BenchRun if on_bench => Kind::BenchLever,
        Pressable::BenchRun if fixture => Kind::FixtureControl,
        Pressable::BenchRun => Kind::WallLever,
        Pressable::Cook => Kind::KitchenControl,
        Pressable::FoldCover => Kind::FoldedCover,
        Pressable::Note => Kind::PinnedNote,
        Pressable::Logbook | Pressable::Request if readable => Kind::Readable,
        Pressable::Destination
        | Pressable::Setting
        | Pressable::Request
        | Pressable::Terminal
        | Pressable::Camp
        | Pressable::Logbook => Kind::Card,
    }
}

/// What an entity is to the hand, from its components ([`kind_of`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct Traits {
    pub pressable: Option<Pressable>,
    pub on_bench: bool,
    pub grabbable: bool,
    pub readable: bool,
    pub item: bool,
    pub ladder: bool,
    pub rope: bool,
    /// A breaker or a valve: a run-control with its own look.
    pub fixture: bool,
    pub cart: bool,
    pub button: bool,
    pub pack: bool,
}

/// Its kind, if it's an interactable at all; `Err` for something
/// grabbable that isn't a known kind.
pub fn kind_of(t: &Traits) -> Result<Option<Kind>, ()> {
    Ok(Some(if t.button {
        Kind::PushButton
    } else if t.cart {
        Kind::MineCart
    } else if t.pack {
        Kind::OwnPack
    } else if let Some(p) = t.pressable {
        pressable_kind(p, t.readable, t.fixture, t.on_bench)
    } else if t.readable {
        Kind::Readable
    } else if t.ladder {
        Kind::Ladder
    } else if t.rope {
        Kind::Rope
    } else if t.item {
        Kind::LooseItem
    } else if t.grabbable {
        return Err(());
    } else {
        return Ok(None);
    }))
}

/// The traits of every entity in `world` that has any, with the entity.
pub fn traits(world: &mut bevy::prelude::World) -> Vec<(bevy::prelude::Entity, Traits)> {
    use bevy::prelude::*;
    let benches: std::collections::HashSet<Entity> = world
        .query_filtered::<Entity, With<crate::raid::bench::Bench>>()
        .iter(world)
        .collect();
    let mut q = world.query::<(
        Entity,
        Option<&Pressable>,
        Option<&ChildOf>,
        Has<crate::interact::Grabbable>,
        (
            Has<crate::interact::read::Readable>,
            Has<crate::items::Item>,
            Has<crate::interact::hand_climb::Ladder>,
            Has<crate::gadgets::climbing::Rope>,
            Has<crate::world::underground::PowerSwitch>,
            Has<crate::world::underground::DrainControl>,
            Has<crate::world::workings::MineCart>,
            Has<crate::interact::poke::PokeButton>,
            Has<crate::interact::body::Backpack>,
        ),
    )>();
    q.iter(world)
        .map(|(e, p, parent, grabbable, f)| {
            let (readable, item, ladder, rope, power, drain, cart, button, pack) = f;
            (
                e,
                Traits {
                    pressable: p.copied(),
                    on_bench: parent.is_some_and(|c| benches.contains(&c.parent())),
                    grabbable,
                    readable,
                    item,
                    ladder,
                    rope,
                    fixture: power || drain,
                    cart,
                    button,
                    pack,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::legibility::angle;
    use bevy::prelude::*;

    #[test]
    fn every_kind_gives_enough_cues() {
        let mut bad = Vec::new();
        for k in Kind::ALL {
            let s = k.spec();
            if s.cues.len() < 2 {
                bad.push(format!("{k:?}: one cue"));
            }
            if !s.cues.iter().any(|c| c.at_a_distance()) {
                bad.push(format!("{k:?}: nothing seen before you touch it"));
            }
            let deg = angle(s.part, SEEN_FROM, 0.0);
            if deg < MIN_PART_DEG {
                bad.push(format!(
                    "{k:?}: its part is {:.0} mm, {deg:.2} deg from {SEEN_FROM} m",
                    s.part * 1000.0
                ));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every interactable in the game is a known kind, and what its kind
    /// claims holds: the near ring finds it (item layer, grabbable).
    #[test]
    fn every_interactable_is_a_known_kind_with_true_cues() {
        use avian3d::prelude::CollisionLayers;
        let mut bad = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (map, camp) in [
            ("training_yard", false),
            ("marrow_creek", false),
            ("kettle_pass", false),
            ("cinder_works", false),
            ("marrow_creek", true),
        ] {
            let dir = std::env::temp_dir().join(format!(
                "extraction-afford-{map}-{camp}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.into(),
                camp,
                data_dir: Some(dir.clone()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..60 {
                app.update();
            }
            let w = app.world_mut();
            // A collider's layers, on the entity or a child of it (hover
            // walks up from a collider to what it belongs to).
            let item_layer: std::collections::HashSet<Entity> = w
                .query::<(Entity, &CollisionLayers, Option<&ChildOf>)>()
                .iter(w)
                .filter(|(_, l, _)| l.memberships.has_all(crate::world::Layer::Item))
                .flat_map(|(e, _, p)| [Some(e), p.map(|p| p.parent())])
                .flatten()
                .collect();
            for (e, t) in traits(w) {
                let what = w.get::<Name>(e).map_or(format!("{e}"), |n| n.to_string());
                let kind = match kind_of(&t) {
                    Ok(Some(k)) => k,
                    Ok(None) => continue,
                    Err(()) => {
                        bad.push(format!("{map}: {what}: grabbable, but no affordance kind"));
                        continue;
                    }
                };
                seen.insert(kind);
                if kind.spec().cues.contains(&Cue::Ring)
                    && !(item_layer.contains(&e) && (t.grabbable || t.ladder || t.rope))
                {
                    bad.push(format!(
                        "{map}: {what} ({kind:?}): says the ring shows, but hover can't find it"
                    ));
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        bad.sort();
        bad.dedup();
        assert!(seen.len() >= 8, "kinds seen: {seen:?}");
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
