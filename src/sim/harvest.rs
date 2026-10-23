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
//! - Frost: in only with a hearth on you (it keeps the chill off): live,
//!   or sealed in a lead case and taken out at the edge (sealed it draws
//!   no drones on the way); without one, it's left.

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
    /// And another.
    pub spare: Option<Entity>,
    /// Its gun, slung or in hand.
    pub gun: Option<Entity>,
    /// A hearth on it, and how.
    pub hearth: Hearth,
}

/// A hearth in a raider's kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hearth {
    None,
    /// Live: in the pack or a hand.
    Live,
    /// Sealed in a lead case: `case`, and the stone in it; the case in the
    /// pack, or in a body slot (a stowed case goes to a free one first).
    Sealed {
        case: Entity,
        stone: Entity,
        packed: bool,
    },
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
    // A step in to just outside it (a throw falls short from further: a
    // bolt at the edge winds nothing), out of the pack, and thrown. Clear
    // of a snare's reach for metal in a hand (0.8 m).
    let near = centre.xz() + (edge - centre.xz()).normalize_or(Vec2::X) * (rad + 1.2);
    let throw = |junk: Entity, up: f32| {
        [
            Task::WalkTo {
                to: near,
                within: 0.4,
            },
            Task::ToggleBackpack,
            Task::Wait(0.6),
            Task::GrabFromHere {
                item: junk,
                side: Side::Left,
            },
            Task::ToggleBackpack,
            Task::Wait(0.3),
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
            // Something has to stay in it to wind it up: dropped in at its
            // base (a lob at the core sails through), two to be sure.
            t.extend(throw(kit.junk?, 0.0));
            if let Some(second) = kit.spare {
                t.extend(throw(second, 0.0));
            }
            // Clear of the pop, then the stone, wherever it lands.
            let out = (edge - centre.xz()).normalize_or(Vec2::X);
            t.push(Task::WalkTo {
                to: centre.xz() + out * (rad + 6.0),
                within: 1.0,
            });
            // It pops about four seconds after it's wound.
            t.push(Task::Wait(7.0));
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
                // At its top: a stone settled in the coals has its middle at
                // the ground, and a shot from standing height at that meets
                // the ground just short of it.
                Task::Fire {
                    at: at + Vec3::Y * 0.04,
                    shots: 6,
                },
                Task::Release(Side::Left),
                Task::Wait(HOT_SECS + 2.0),
                // Out of the coals and cool, it's taken; still in them (the
                // shots missed), it's left: never a hand into the fire.
                Task::GrabClear {
                    item: stone,
                    side: Side::Left,
                    clear_of: centre,
                    radius: rad + 0.6,
                },
                Task::Stow(Side::Left),
                Task::Wait(0.2),
            ]);
            t.push(back);
        }
        AnomalyKind::Frost => {
            // The hearth keeps the chill from ever biting or stiffening
            // you much: straight in and out. Sealed, it's out of its case
            // at the edge (into the right hand, the case back in the
            // pack), and live from there on.
            match kit.hearth {
                Hearth::None => return None,
                Hearth::Live => {}
                Hearth::Sealed {
                    case,
                    stone: hearth,
                    packed,
                } => {
                    if packed {
                        t.extend([
                            Task::ToggleBackpack,
                            Task::Wait(0.6),
                            Task::GrabFromHere {
                                item: case,
                                side: Side::Left,
                            },
                            Task::ToggleBackpack,
                        ]);
                    } else {
                        t.push(Task::Grab {
                            item: case,
                            side: Side::Left,
                        });
                    }
                    t.extend([
                        Task::Wait(0.3),
                        Task::Grab {
                            item: hearth,
                            side: Side::Right,
                        },
                        Task::Wait(0.3),
                        Task::Stow(Side::Left),
                        Task::Wait(0.3),
                    ]);
                }
            }
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

    /// The throw the ways start with: a bolt stowed in the pack, out of it
    /// and thrown at a point, lands near it.
    #[test]
    fn a_packed_bolt_comes_out_and_is_thrown() {
        let mut app = crate::headless_app(Default::default(), false);
        app.finish();
        app.cleanup();
        app.update();
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let me = app.world().get::<GlobalTransform>(p).unwrap().translation();
        let bolt =
            crate::anomaly::surge::grow(app.world_mut(), "bolt", me + Vec3::new(0.6, 0.3, 0.0))
                .unwrap();
        app.update();
        let target = me + Vec3::new(0.0, 0.0, -4.5);
        let mut tasks = vec![
            Task::PickUp {
                id: "bolt".into(),
                side: Side::Left,
            },
            Task::Stow(Side::Left),
            Task::Wait(0.5),
        ];
        // The throw half of a snare's way, from the edge.
        let kit = Kit {
            junk: Some(bolt),
            spare: None,
            gun: None,
            hearth: Hearth::None,
        };
        let way = plan(
            AnomalyKind::Snare,
            target,
            2.0,
            Vec2::new(me.x, me.z),
            Entity::PLACEHOLDER,
            target,
            &kit,
        )
        .unwrap();
        tasks.extend(
            way.into_iter()
                .take_while(|t| !matches!(t, Task::Wait(w) if *w < 0.25)),
        );
        app.world_mut()
            .entity_mut(p)
            .insert(crate::bot::Bot::new(tasks));
        // Its closest pass: a snare goes off at the touch (a lift needs it
        // to stay, which a slope can roll it out of).
        let mut closest = f32::INFINITY;
        for _ in 0..90 * 8 {
            app.update();
            let at = app
                .world()
                .get::<GlobalTransform>(bolt)
                .unwrap()
                .translation();
            closest = closest.min(Vec2::new(at.x - target.x, at.z - target.z).length());
        }
        assert!(
            app.world()
                .get::<crate::interact::body::InBackpack>(bolt)
                .is_none(),
            "out of the pack"
        );
        assert!(
            closest < 2.0,
            "passed through the snare ({closest:.1} m from its core at closest, radius 2)"
        );
    }

    #[test]
    fn every_kind_has_a_way_with_the_kit_and_none_without_it() {
        let e = Entity::from_raw_u32(7).unwrap();
        let full = Kit {
            junk: Some(e),
            spare: Some(e),
            gun: Some(e),
            hearth: Hearth::Live,
        };
        let bare = Kit {
            junk: None,
            spare: None,
            gun: None,
            hearth: Hearth::None,
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
                    .any(|t| matches!(t, Task::Grab { item, .. } | Task::GrabClear { item, .. } if *item == e))
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

    /// A frost's bands, measured: a novice walks in for the stone and out;
    /// a skilled raider does the same with a hearth in its pack, live or
    /// sealed in a lead case (out of it at the edge). All come out with
    /// it; the novice pays for it.
    #[test]
    fn a_hearth_takes_the_frost_out_of_a_frost_dive() {
        use crate::interact::body::{Backpack, InBackpack};
        use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};
        // 0: none, 1: live, 2: sealed in the pack, 3: sealed, picked up
        // and stowed (wherever stowing puts it: a belt slot first).
        let dive = |how: u8| -> (f32, bool, f32) {
            let mut app = crate::headless_app(Default::default(), false);
            app.finish();
            app.cleanup();
            app.update();
            let w = app.world_mut();
            let p = w
                .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
                .iter(w)
                .next()
                .unwrap();
            w.resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
            let me = w.get::<GlobalTransform>(p).unwrap().translation();
            let centre = w
                .resource::<crate::world::Zone>()
                .on_ground(me.x, me.z - 6.0);
            crate::anomaly::spawn_anomaly(w, AnomalyKind::Frost, centre, 3.0, None);
            let stone =
                crate::anomaly::surge::grow(w, "art_rimestone", centre + Vec3::Y * 0.15).unwrap();
            let mut hearth = Hearth::None;
            if how == 3 {
                let case =
                    crate::anomaly::surge::grow(w, "lead_case", me + Vec3::new(0.5, 0.3, 0.0))
                        .unwrap();
                let h = crate::anomaly::surge::grow(w, "art_hearth", Vec3::ZERO).unwrap();
                app.update();
                crate::persist::seat_in_mount(app.world_mut(), case, h);
                app.world_mut()
                    .entity_mut(p)
                    .insert(crate::bot::Bot::new(vec![
                        Task::Grab {
                            item: case,
                            side: Side::Left,
                        },
                        Task::Stow(Side::Left),
                        Task::Wait(0.5),
                    ]));
                for _ in 0..90 * 6 {
                    app.update();
                }
                let packed = app.world().get::<InBackpack>(case).is_some();
                assert!(
                    packed || app.world().get::<crate::interact::InSocket>(case).is_some(),
                    "stowed somewhere"
                );
                hearth = Hearth::Sealed {
                    case,
                    stone: h,
                    packed,
                };
            } else if how > 0 {
                let pack = w
                    .query::<(Entity, &Backpack)>()
                    .iter(w)
                    .find(|(_, b)| b.owner == p && b.carried)
                    .map(|(e, _)| e)
                    .unwrap();
                let h = crate::anomaly::surge::grow(w, "art_hearth", Vec3::ZERO).unwrap();
                let packed = if how == 1 {
                    h
                } else {
                    crate::anomaly::surge::grow(w, "lead_case", Vec3::ZERO).unwrap()
                };
                w.get_mut::<Backpack>(pack).unwrap().contents.push(packed);
                w.entity_mut(packed).insert((
                    InBackpack { bag: pack },
                    ChildOf(pack),
                    RigidBodyDisabled,
                    ColliderDisabled,
                ));
                hearth = Hearth::Live;
                if how >= 2 {
                    app.update();
                    crate::persist::seat_in_mount(app.world_mut(), packed, h);
                    app.update();
                    hearth = Hearth::Sealed {
                        case: packed,
                        stone: h,
                        packed: true,
                    };
                }
            }
            let edge = Vec2::new(me.x, me.z);
            let kit = Kit {
                junk: None,
                spare: None,
                gun: None,
                hearth,
            };
            let tasks = if how > 0 {
                plan(AnomalyKind::Frost, centre, 3.0, edge, stone, centre, &kit).unwrap()
            } else {
                assert!(
                    plan(AnomalyKind::Frost, centre, 3.0, edge, stone, centre, &kit).is_none(),
                    "no hearth, no skilled way in"
                );
                vec![
                    Task::Grab {
                        item: stone,
                        side: Side::Left,
                    },
                    Task::Stow(Side::Left),
                    Task::Wait(0.2),
                    Task::WalkTo {
                        to: edge,
                        within: 0.8,
                    },
                ]
            };
            app.update();
            let full = app.world().get::<crate::health::Health>(p).unwrap().hp;
            app.world_mut()
                .entity_mut(p)
                .insert(crate::bot::Bot::new(tasks));
            let mut peak: f32 = 0.0;
            for _ in 0..90 * 25 {
                app.update();
                peak = peak.max(
                    app.world()
                        .get::<crate::world::cold::Chill>(p)
                        .map_or(0.0, |c| c.level),
                );
            }
            let hp = app.world().get::<crate::health::Health>(p).unwrap().hp;
            let taken = app
                .world()
                .get::<crate::anomaly::artifacts::CarriedBy>(stone)
                .is_some_and(|c| c.0 == p);
            (full - hp, taken, peak)
        };
        let (novice, n_took, n_peak) = dive(0);
        let (skilled, s_took, s_peak) = dive(1);
        let (sealed, c_took, c_peak) = dive(2);
        let (worn, w_took, _) = dive(3);
        println!(
            "frost dive: novice lost {novice:.0} hp, chill to {n_peak:.2}; \
             with a hearth {skilled:.0} hp, chill to {s_peak:.2}; \
             with one sealed {sealed:.0} hp, chill to {c_peak:.2}"
        );
        assert!(n_took && s_took && c_took && w_took, "all come out with it");
        assert_eq!(
            worn, 0.0,
            "a case stowed on the belt: its hearth out, never bitten"
        );
        assert_eq!(skilled, 0.0, "a hearth: never bitten");
        assert_eq!(sealed, 0.0, "a hearth out of its case: never bitten");
        assert!(
            (5.0..60.0).contains(&novice),
            "a novice is bitten, not killed ({novice:.0} hp)"
        );
    }
}
