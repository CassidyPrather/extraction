//! A raider who knows how each anomaly gives up its stone (the harvest
//! scenarios' ways, `anomaly::scenarios`), as tasks from the edge it's
//! standing at. A novice just walks in, grabs and walks out (and a
//! careful one tests a snare or a mirror first): `SIM_HARVEST=novice`
//! keeps every raider a novice, so a sim batch can measure both bands
//! (`scripts/harvest_band.py`).
//!
//! - Snare: ground it with something thrown, reach in while it recharges.
//! - Mirror: throw something through, reach in while it folds back up.
//! - Lift: wind it up with something thrown, stand clear, take the stone
//!   from where the pop throws it.
//! - Burner: shoot the stone out of the coals, let it cool, then take it.
//! - Well: in for the stone at its heart and straight out, healthy or not
//!   at all.
//! - Hush: in and out before the silence presses (anyone does that).

use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, HOT_SECS};

use crate::bot::Task;
use crate::rig::Side;

/// Whether raiders harvest with skill: everyone but the reckless, unless
/// `SIM_HARVEST=novice`.
pub fn skilled(personality: &str) -> bool {
    personality != "reckless" && std::env::var("SIM_HARVEST").map_or(true, |v| v != "novice")
}

/// What a skilled raider takes from the truck to work anomalies with:
/// bolts to throw (a novice brings none).
pub fn kit(personality: &str) -> Vec<&'static str> {
    if skilled(personality) {
        vec!["bolt"; 4]
    } else {
        Vec::new()
    }
}

/// What a skilled raider has to hand.
pub struct Kit {
    /// Something worthless to throw.
    pub junk: Option<Entity>,
    /// Its gun, slung or in hand.
    pub gun: Option<Entity>,
}

/// The tasks to take `stone` (at `at`) from the anomaly of `kind` centred
/// at `centre` (radius `rad`), standing at `edge` outside it, and back out
/// to `edge`. None when the way needs what it hasn't got (nothing to throw,
/// no gun): it goes in as a novice.
pub fn plan(
    kind: AnomalyKind,
    centre: Vec3,
    rad: f32,
    edge: Vec2,
    stone: Entity,
    at: Vec3,
    kit: &Kit,
) -> Option<Vec<Task>> {
    let take = [
        Task::Grab {
            item: stone,
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::Wait(0.2),
    ];
    let back = Task::WalkTo {
        to: edge,
        within: 0.8,
    };
    let throw = |junk: Entity, up: f32| {
        [
            Task::Grab {
                item: junk,
                side: Side::Left,
            },
            Task::Throw {
                side: Side::Left,
                at: centre + Vec3::Y * up,
            },
        ]
    };
    let mut t = Vec::new();
    match kind {
        AnomalyKind::Snare => {
            t.extend(throw(kit.junk?, 0.8));
            t.push(Task::Wait(0.2));
            t.extend(take);
            t.push(back);
        }
        AnomalyKind::Mirror => {
            t.extend(throw(kit.junk?, 0.9));
            t.extend(take);
            t.push(back);
        }
        AnomalyKind::Lift => {
            t.extend(throw(kit.junk?, 1.2));
            // Clear of the pop, then the stone, wherever it lands.
            let out = (edge - centre.xz()).normalize_or(Vec2::X);
            t.push(Task::WalkTo {
                to: centre.xz() + out * (rad + 6.0),
                within: 1.0,
            });
            t.push(Task::Wait(9.0));
            t.extend(take);
        }
        AnomalyKind::Burner => {
            // Both hands on the gun, the stone out of the coals, the gun
            // left in hand, and long enough for the stone to cool.
            let gun = kit.gun?;
            t.extend([
                Task::Release(Side::Left),
                Task::Grab {
                    item: gun,
                    side: Side::Right,
                },
                Task::Support,
                Task::Fire { at, shots: 3 },
                Task::Release(Side::Left),
                Task::Wait(HOT_SECS + 2.0),
            ]);
            t.extend(take);
            t.push(back);
        }
        AnomalyKind::Well | AnomalyKind::Hush => {
            t.extend(take);
            t.push(back);
        }
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_way_with_the_kit_and_none_without_it() {
        let e = Entity::from_raw_u32(7).unwrap();
        let full = Kit {
            junk: Some(e),
            gun: Some(e),
        };
        let bare = Kit {
            junk: None,
            gun: None,
        };
        for kind in AnomalyKind::ALL {
            let t = plan(
                kind,
                Vec3::ZERO,
                2.0,
                Vec2::new(4.0, 0.0),
                e,
                Vec3::ZERO,
                &full,
            );
            assert!(t.is_some_and(|t| {
                t.iter()
                    .any(|t| matches!(t, Task::Grab { item, .. } if *item == e))
            }));
            let needs = !matches!(kind, AnomalyKind::Well | AnomalyKind::Hush);
            let none = plan(
                kind,
                Vec3::ZERO,
                2.0,
                Vec2::new(4.0, 0.0),
                e,
                Vec3::ZERO,
                &bare,
            );
            assert_eq!(none.is_none(), needs, "{kind:?}");
        }
    }
}
