//! The Sorter's mind: the Depot gantry that hears, watches, rings the
//! hall and throws.
//!
//! It can't leave its rails, so its mind is short: calm, it does its own
//! work (the body's sort loop). A sound it hears turns its head there
//! ([`SorterOrder::Watch`]). Seeing you, it bangs the hall once (a
//! [`Noise`] of [`ALARM`]: loud enough to wake what sleeps in the Depot's
//! bays) and throws a tote at you while you're within
//! [`THROW_RANGE`] (the body winds up first, its tell: `SorterWindup`,
//! and throws where you stood then, so a step aside beats it). Out of
//! range, it tracks you. Lose it and it watches where you went
//! ([`WATCH_TIME`]), then gropes round there with its arm, spot by spot
//! ([`GROPE_TIME`]), then goes back to work. Its brain carries the same
//! states as any robot's (Investigate, Search, Engage) so what it would
//! voice (`search::MindShift`) and its radio calls come for free.
//!
//! The body (Robot bodies', `sorter`) does the moving; this only gives it
//! orders, and leaves it alone while calm.

use bevy::prelude::*;

use super::sorter::{Sorter, SorterOrder, SorterScript, SorterState, THROW_RANGE};
use super::{Awareness, Brain, Noise, Robot};

/// It bangs the hall this loud (m) when it first sees you.
pub const ALARM: f32 = 30.0;
/// Having lost you, it watches where you were this long (s)...
pub const WATCH_TIME: f32 = 8.0;
/// ...then gropes round there this long...
pub const GROPE_TIME: f32 = 20.0;
/// ...a spot every this many seconds...
pub const GROPE_EVERY: f32 = 2.5;
/// ...this far round (m, at least; more if it was unsure where you were).
pub const GROPE_RADIUS: f32 = 2.5;
/// A sound turns its head when its suspicion is past this.
pub const HEARD_AT: f32 = super::search::INVESTIGATE_AT;

/// Where the Sorter's mind is.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub enum SorterMind {
    #[default]
    Calm,
    /// It has you in sight.
    Fight,
    /// Watching `at` (heard, or where it lost you), this long left.
    Watch { at: Vec3, left: f32 },
    /// Groping round `at` this far out, this long left; `k` counts spots.
    Grope {
        at: Vec3,
        radius: f32,
        left: f32,
        k: u32,
    },
}

/// It has banged the hall since it last calmed down.
#[derive(Component, Debug, Default)]
pub struct Alarmed(pub bool);

pub struct SorterMindPlugin;

impl Plugin for SorterMindPlugin {
    fn build(&self, app: &mut App) {
        // The mind gives the orders: the body's own throw-on-sight is off.
        app.insert_resource(SorterScript(false)).add_systems(
            FixedUpdate,
            (wake_sorters, sorter_mind).chain().after(super::perceive),
        );
    }
}

fn wake_sorters(mut commands: Commands, sorters: Query<Entity, Added<Sorter>>) {
    for e in &sorters {
        commands.entity(e).insert((
            SorterMind::default(),
            Alarmed::default(),
            super::Breed("Sorter"),
        ));
    }
}

/// The spot it gropes at `k`: round `at` on the floor, a golden-angle
/// spiral out to `radius`.
pub fn grope_spot(at: Vec3, radius: f32, k: u32) -> Vec3 {
    let a = k as f32 * 2.399;
    let r = radius * ((k % 5) as f32 + 1.0) / 5.0;
    at + Vec3::new(a.cos() * r, 0.0, a.sin() * r)
}

fn sorter_mind(
    mut commands: Commands,
    mut sorters: Query<(
        Entity,
        &Robot,
        &Sorter,
        &mut Awareness,
        &mut Brain,
        &mut SorterMind,
        &mut Alarmed,
        Option<&SorterState>,
        &GlobalTransform,
    )>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut noise: MessageWriter<Noise>,
) {
    let dt = time.delta_secs();
    for (e, robot, s, mut aware, mut brain, mut mind, mut alarmed, state, gt) in &mut sorters {
        if robot.wrecked {
            continue;
        }
        let seen = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation());
        let news = std::mem::take(&mut aware.news);
        let heard = aware
            .last_known
            .filter(|_| news && aware.suspicion > HEARD_AT);
        // What next.
        let next = match (*mind, seen) {
            (_, Some(_)) => SorterMind::Fight,
            (SorterMind::Fight, None) => SorterMind::Watch {
                at: aware.last_known.unwrap_or(gt.translation()),
                left: WATCH_TIME,
            },
            (SorterMind::Calm, None) => match heard {
                Some(at) => SorterMind::Watch {
                    at,
                    left: WATCH_TIME,
                },
                None => SorterMind::Calm,
            },
            (SorterMind::Watch { at, left }, None) => {
                let (at, left) = match heard {
                    Some(h) => (h, WATCH_TIME),
                    None => (at, left - dt),
                };
                if left > 0.0 {
                    SorterMind::Watch { at, left }
                } else {
                    SorterMind::Grope {
                        at,
                        radius: aware.uncertainty.max(GROPE_RADIUS),
                        left: GROPE_TIME,
                        k: 0,
                    }
                }
            }
            (
                SorterMind::Grope {
                    at,
                    radius,
                    left,
                    k,
                },
                None,
            ) => match heard {
                Some(h) => SorterMind::Watch {
                    at: h,
                    left: WATCH_TIME,
                },
                None if left - dt <= 0.0 => SorterMind::Calm,
                None => {
                    // A new spot every GROPE_EVERY.
                    let k = if ((left - dt) / GROPE_EVERY).floor() != (left / GROPE_EVERY).floor() {
                        k + 1
                    } else {
                        k
                    };
                    SorterMind::Grope {
                        at,
                        radius,
                        left: left - dt,
                        k,
                    }
                }
            },
        };
        let calmed = next == SorterMind::Calm && *mind != SorterMind::Calm;
        *mind = next;
        // The hall rings once, the first time it sees you.
        if seen.is_some() && !alarmed.0 {
            alarmed.0 = true;
            noise.write(Noise {
                at: gt.translation(),
                radius: ALARM,
                source: Some(e),
            });
        }
        // Orders, and the brain's state for what it would voice.
        let order = match next {
            SorterMind::Calm => {
                if calmed {
                    alarmed.0 = false;
                    *brain = Brain::Working {
                        at: s.world(Vec3::ZERO),
                        left: f32::INFINITY,
                    };
                    commands.entity(e).insert(SorterOrder::Idle);
                }
                continue;
            }
            SorterMind::Fight => {
                let tp = seen.unwrap();
                *brain = Brain::Engage;
                let flat = s.local(tp) - s.shoulder();
                if Vec2::new(flat.x, flat.z).length() < THROW_RANGE {
                    // At its chest.
                    SorterOrder::Throw {
                        at: tp + Vec3::Y * 1.1,
                    }
                } else {
                    SorterOrder::Watch { at: tp }
                }
            }
            SorterMind::Watch { at, left } => {
                if !matches!(*brain, Brain::Investigate { .. }) {
                    *brain = Brain::Investigate { at, left };
                }
                SorterOrder::Watch { at }
            }
            SorterMind::Grope {
                at,
                radius,
                left,
                k,
            } => {
                if !matches!(*brain, Brain::Search { .. }) {
                    *brain = Brain::Search {
                        at,
                        left,
                        spots: Vec::new(),
                        dwell: 0.0,
                    };
                }
                let spot = grope_spot(at, radius, k);
                // Its grab can't get everywhere: where it can't, it looks.
                let reach = state.map_or(f32::INFINITY, |st| st.reach);
                let under = s.local(spot);
                if under.x.abs() <= s.half.x + reach && under.z.abs() <= s.half.y + reach {
                    SorterOrder::Reach { at: spot }
                } else {
                    SorterOrder::Watch { at: spot }
                }
            }
        };
        commands.entity(e).insert(order);
    }
}
