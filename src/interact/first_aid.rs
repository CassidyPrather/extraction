//! First aid and food with the hands, in the headset.
//!
//! - **A dressing** is wound round the wounded limb (`wounds`): hold it
//!   against the limb (a thigh, a shin, an arm, the chest, the belly, the
//!   head) and circle it round, one and a half turns. It binds that limb's
//!   wounds; the others bleed on.
//! - **A splint** goes along a broken shin (`wounds::Fracture`): hold it
//!   along the shin, the other hand at the leg strapping it, for
//!   [`STRAP`] seconds, and you walk on it (limping less).
//! - **An injector** goes in the thigh. Put its tip to your thigh and pull
//!   the trigger.
//! - **Food** goes to your mouth: hold it there a moment. A bottle has to
//!   be tipped up to drink, and its cap off first ([`Capped`]): the other
//!   hand takes the cap (the trigger, as a gun's slide is taken) and
//!   twists it [`UNSCREW`] of a turn.
//!
//! Each, done, uses the item as the button used to (`items::usage`). The
//! button still works for medicine with the one-handed comfort setting,
//! since a dressing takes both arms.

use bevy::prelude::*;

use super::tools::{CLICK, TICK, pulse};
use super::{Hand, Held, InteractSystems};
use crate::interact::body::Torso;
use crate::items::usage::UseItem;
use crate::items::{Catalog, Item};
use crate::player::Player;
use crate::rig::haptics::Haptic;
use crate::rig::{RigDriver, RigHand, RigInput};
use extraction_core::items::Category;

/// Turns round the forearm a dressing takes.
const WRAP_TURNS: f32 = 1.5;
/// How far round a bottle's cap is twisted to come off, turns.
pub const UNSCREW: f32 = 0.75;

/// A bottle with its cap on: it won't drink.
#[derive(Component, Debug, Default)]
pub struct Capped {
    /// How far it's been twisted, turns.
    pub twisted: f32,
}

/// How long a splint takes to strap on, s.
pub const STRAP: f32 = 1.5;

/// How far out from a limb's surface a dressing going round it may be
/// (m): a hand circling the other forearm can't come closer than its
/// controller lets it, and a circle by hand is never round.
const ROUND: f32 = 0.12;
/// How long a dressing drifted off its limb keeps what's wound (s), and
/// how fast it unwinds after (turns a second).
const KEEP: f32 = 1.5;
const UNWIND: f32 = 1.0;
/// An injector's tip, in its own space, and how near the thigh it must be.
pub const NEEDLE: Vec3 = Vec3::new(0.0, 0.0, -0.065);
const ON_THIGH: f32 = 0.12;
/// The thighs, from the torso anchor (it rides mid-chest).
const THIGH: Vec3 = Vec3::new(0.11, -0.62, -0.04);
/// The mouth, from the eyes, and how near food must come; how long it's
/// held there.
const MOUTH: Vec3 = Vec3::new(0.0, -0.08, -0.07);
const AT_MOUTH: f32 = 0.12;
const BITE: f32 = 0.8;

pub struct FirstAidPlugin;

impl Plugin for FirstAidPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(cap_bottles).add_systems(
            FixedUpdate,
            (
                wrap_dressings,
                strap_splints,
                unscrew,
                inject,
                eat_and_drink,
            )
                .after(InteractSystems::Follow),
        );
    }
}

/// Where a dressing is in its winding: for each limb it's near, how far
/// round that limb it's gone (net: back and forth doesn't wind), and the
/// one furthest round.
#[derive(Component, Debug, Default)]
pub struct Winding {
    rounds: Vec<Round>,
    /// The furthest round any limb, turns.
    pub turns: f32,
    /// That limb.
    pub limb: Option<super::wounds::Limb>,
}

/// A dressing's way round one limb.
#[derive(Debug)]
struct Round {
    limb: super::wounds::Limb,
    /// The angle it was last at round the limb, while it's near it.
    last: Option<f32>,
    /// Signed turns.
    net: f32,
    /// Seconds since it was last near.
    away: f32,
}

/// Seconds food has been at the mouth.
#[derive(Component, Debug, Default)]
pub struct Eating(pub f32);

/// The dressing a hand winds round a limb, for `items::usage`: it binds
/// that limb's wounds (`wounds::Wounds::dress`), not every bleed.
#[derive(Component, Debug)]
pub struct OnLimb(pub super::wounds::Limb);

/// A dressing circled round a limb: the wounded one (where the hit was),
/// or any. One and a half turns round it binds it.
#[allow(clippy::type_complexity)]
fn wrap_dressings(
    mut commands: Commands,
    mut items: Query<(Entity, &Item, &Held, &GlobalTransform, Option<&mut Winding>)>,
    players: Query<
        (
            &Transform,
            &RigInput,
            &crate::rig::stance::Stance,
            &Children,
        ),
        With<crate::rig::LocalPlayer>,
    >,
    hands: Query<(&RigHand, &Hand)>,
    mut patients: Query<(&mut crate::health::Health, &mut super::wounds::Wounds)>,
    catalog: Res<Catalog>,
    driver: Res<RigDriver>,
    mut uses: MessageWriter<UseItem>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    for (e, item, held, gt, winding) in &mut items {
        let def = catalog.get(item.kind());
        if def.id != "field_dressing" {
            continue;
        }
        let Some(mut w) = winding else {
            commands.entity(e).insert(Winding::default());
            continue;
        };
        let Some(segs) = super::wounds::player_segments(held.player, &players, &hands) else {
            continue;
        };
        // Round a limb, not the arm holding it. Every limb it's near
        // keeps its own count, so a forearm held across the chest winds the
        // forearm (a circle round it doesn't go round the chest), and the
        // first wound round far enough is the one dressed.
        let at = gt.translation();
        let dt = time.delta_secs();
        let before = (w.turns * 4.0).floor();
        for s in &segs {
            if matches!(s.limb, super::wounds::Limb::Forearm(side) if side == held.side) {
                continue;
            }
            let (t, a, d) = s.locate(at);
            let near = (-0.1..=1.1).contains(&t) && d <= s.r + ROUND;
            let i = match w.rounds.iter().position(|r| r.limb == s.limb) {
                Some(i) => i,
                None if near => {
                    w.rounds.push(Round {
                        limb: s.limb,
                        last: None,
                        net: 0.0,
                        away: 0.0,
                    });
                    w.rounds.len() - 1
                }
                None => continue,
            };
            let r = &mut w.rounds[i];
            if !near {
                // Drifted off (a circle by hand dips out under the arm):
                // what's wound stays a while, and the way back in counts
                // the arc it went round outside; then it unwinds.
                r.away += dt;
                if r.away > KEEP {
                    r.last = None;
                }
                if r.away > KEEP {
                    r.net -= r.net.signum() * (dt * UNWIND).min(r.net.abs());
                }
                continue;
            }
            r.away = 0.0;
            if let Some(last) = r.last {
                let mut d = a - last;
                if d > std::f32::consts::PI {
                    d -= std::f32::consts::TAU;
                } else if d < -std::f32::consts::PI {
                    d += std::f32::consts::TAU;
                }
                r.net += d / std::f32::consts::TAU;
            }
            r.last = Some(a);
        }
        w.rounds.retain(|r| r.last.is_some() || r.net.abs() > 1e-3);
        let lead = w
            .rounds
            .iter()
            .max_by(|a, b| a.net.abs().total_cmp(&b.net.abs()))
            .map(|r| (r.limb, r.net.abs()));
        w.turns = lead.map_or(0.0, |l| l.1);
        w.limb = lead.map(|l| l.0);
        if (w.turns * 4.0).floor() > before {
            // A tick each quarter turn, in both hands: it's going on.
            pulse(&mut haptics, held.side, TICK);
            pulse(&mut haptics, held.side.other(), TICK);
        }
        let Some(limb) = w.limb else {
            continue;
        };
        if w.turns >= WRAP_TURNS {
            // That limb's wounds bound; the rest bleed on.
            if let Ok((mut h, mut wounds)) = patients.get_mut(held.player) {
                let stopped = wounds.dress(limb);
                h.bleeding = (h.bleeding - stopped).max(0.0);
            }
            commands.entity(e).insert(OnLimb(limb));
            uses.write(UseItem {
                player: held.player,
                item: e,
            });
            pulse(&mut haptics, held.side, CLICK);
            pulse(&mut haptics, held.side.other(), CLICK);
            *w = Winding::default();
        }
    }
}

/// How long a splint's been held along the broken shin.
#[derive(Component, Debug, Default)]
pub struct Strapping(pub f32);

/// A splint held along the broken shin, the other hand at it: strapped
/// on, after a while.
#[allow(clippy::type_complexity)]
fn strap_splints(
    mut commands: Commands,
    mut items: Query<(
        Entity,
        &Item,
        &Held,
        &GlobalTransform,
        Option<&mut Strapping>,
    )>,
    players: Query<
        (
            &Transform,
            &RigInput,
            &crate::rig::stance::Stance,
            &Children,
        ),
        With<crate::rig::LocalPlayer>,
    >,
    hands: Query<(&RigHand, &Hand)>,
    mut broken: Query<&mut super::wounds::Fracture>,
    catalog: Res<Catalog>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    for (e, item, held, gt, strapping) in &mut items {
        if catalog.get(item.kind()).id != "splint" {
            continue;
        }
        let Ok(mut fracture) = broken.get_mut(held.player) else {
            continue;
        };
        if fracture.splinted {
            continue;
        }
        let Some(segs) = super::wounds::player_segments(held.player, &players, &hands) else {
            continue;
        };
        let Some(shin) = segs
            .iter()
            .find(|s| s.limb == super::wounds::Limb::Shin(fracture.leg))
        else {
            continue;
        };
        let along = (shin.b - shin.a).normalize_or(Vec3::NEG_Y);
        let axis = gt.rotation() * Vec3::Z;
        let (t, _, d) = shin.locate(gt.translation());
        let mid = shin.a.lerp(shin.b, 0.5);
        let other = players.get(held.player).ok().and_then(|(.., children)| {
            children.iter().find_map(|c| {
                hands
                    .get(c)
                    .ok()
                    .filter(|(rh, _)| rh.side != held.side)
                    .map(|(_, h)| h.world.translation)
            })
        });
        let laid = axis.dot(along).abs() > 0.8
            && (0.1..=0.9).contains(&t)
            && d < shin.r + 0.08
            && other.is_some_and(|o| o.distance(mid) < 0.25);
        let Some(mut s) = strapping else {
            commands.entity(e).insert(Strapping::default());
            continue;
        };
        if !laid {
            s.0 = 0.0;
            continue;
        }
        let before = (s.0 * 2.0).floor();
        s.0 += time.delta_secs();
        if (s.0 * 2.0).floor() > before {
            // A strap pulled tight, each half second.
            pulse(&mut haptics, held.side.other(), TICK);
        }
        if s.0 >= STRAP {
            fracture.splinted = true;
            pulse(&mut haptics, held.side, CLICK);
            pulse(&mut haptics, held.side.other(), CLICK);
            // On the leg now: out of the hand, and gone.
            let hand = held.hand;
            commands.queue(move |world: &mut World| {
                if let Some(mut h) = world.get_mut::<Hand>(hand)
                    && h.holding == Some(e)
                {
                    h.holding = None;
                }
                world.despawn(e);
            });
        }
    }
}

/// A water bottle comes with its cap on, and a handle at it to twist.
fn cap_bottles(
    add: On<Add, Item>,
    items: Query<&Item>,
    catalog: Res<Catalog>,
    mut commands: Commands,
) {
    let e = add.entity;
    let Ok(item) = items.get(e) else {
        return;
    };
    let def = catalog.get(item.kind());
    if def.id != "water_bottle" {
        return;
    }
    let top = def.size_m()[1] / 2.0 - 0.012;
    commands.queue(move |world: &mut World| {
        let Ok(mut ent) = world.get_entity_mut(e) else {
            return;
        };
        if let Some(mut g) = ent.get_mut::<super::Grabbable>() {
            g.points.push(super::GrabPoint::new(
                Transform::from_xyz(0.0, top, 0.0),
                super::GripRole::Handle(0),
                0.06,
            ));
        }
        ent.insert(Capped::default());
    });
}

/// The other hand on a held bottle's cap, twisting it round the bottle's
/// axis: off, once it's gone round far enough.
fn unscrew(
    mut commands: Commands,
    mut hands: Query<(&RigHand, &mut Hand)>,
    mut bottles: Query<(&GlobalTransform, &mut Capped)>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (rh, mut hand) in &mut hands {
        let Some((bottle, 0)) = hand.handle else {
            continue;
        };
        let Ok((gt, mut cap)) = bottles.get_mut(bottle) else {
            continue;
        };
        let axis = gt.rotation() * Vec3::Y;
        let before = (cap.twisted * 4.0).floor();
        cap.twisted += hand.angular_velocity.dot(axis).abs() * dt / std::f32::consts::TAU;
        if (cap.twisted * 4.0).floor() > before {
            pulse(&mut haptics, rh.side, TICK);
        }
        if cap.twisted >= UNSCREW {
            commands.entity(bottle).remove::<Capped>();
            hand.handle = None;
            pulse(&mut haptics, rh.side, CLICK);
        }
    }
}

/// An injector's tip on the thigh, and the trigger.
fn inject(
    items: Query<(Entity, &Item, &Held, &GlobalTransform)>,
    torsos: Query<(&GlobalTransform, &ChildOf), With<Torso>>,
    inputs: Query<&RigInput>,
    catalog: Res<Catalog>,
    driver: Res<RigDriver>,
    mut uses: MessageWriter<UseItem>,
    mut haptics: MessageWriter<Haptic>,
    mut last: Local<std::collections::HashMap<Entity, bool>>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    last.retain(|e, _| items.contains(*e));
    for (e, item, held, gt) in &items {
        let def = catalog.get(item.kind());
        if def.id != "regen_injector" {
            continue;
        }
        let Ok(input) = inputs.get(held.player) else {
            continue;
        };
        let pulled = input.hand(held.side).trigger > 0.6;
        let was = last.insert(e, pulled).unwrap_or(false);
        if !pulled || was {
            continue;
        }
        let Some((torso, _)) = torsos.iter().find(|(_, c)| c.parent() == held.player) else {
            continue;
        };
        let tip = gt.transform_point(NEEDLE);
        let on = [THIGH, THIGH * Vec3::new(-1.0, 1.0, 1.0)]
            .iter()
            .any(|t| torso.transform_point(*t).distance(tip) < ON_THIGH);
        if on {
            uses.write(UseItem {
                player: held.player,
                item: e,
            });
            pulse(&mut haptics, held.side, CLICK);
        }
    }
}

/// Food held at the mouth for a moment (a bottle tipped up) is eaten.
#[allow(clippy::type_complexity)]
fn eat_and_drink(
    mut commands: Commands,
    mut items: Query<(
        Entity,
        &Item,
        &Held,
        &GlobalTransform,
        Option<&mut Eating>,
        Has<Capped>,
    )>,
    players: Query<(&GlobalTransform, &RigInput), With<Player>>,
    catalog: Res<Catalog>,
    driver: Res<RigDriver>,
    mut uses: MessageWriter<UseItem>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    for (e, item, held, gt, eating, capped) in &mut items {
        let def = catalog.get(item.kind());
        // (A capped bottle doesn't pour.)
        if def.category != Category::Food || capped {
            continue;
        }
        let Ok((root, input)) = players.get(held.player) else {
            continue;
        };
        let head = root.mul_transform(input.head);
        let mouth = head.transform_point(MOUTH);
        let drink = def.tags.iter().any(|t| t == "food:Water");
        let tipped = (gt.rotation() * Vec3::Y).dot(Vec3::Y) < 0.6;
        let near = gt.translation().distance(mouth) < AT_MOUTH && (!drink || tipped);
        let Some(mut eating) = eating else {
            if near {
                commands.entity(e).insert(Eating::default());
            }
            continue;
        };
        if !near {
            eating.0 = 0.0;
            continue;
        }
        let before = (eating.0 / 0.3).floor();
        eating.0 += time.delta_secs();
        if (eating.0 / 0.3).floor() > before {
            // A bite (a swallow).
            pulse(&mut haptics, held.side, TICK);
        }
        if eating.0 >= BITE {
            uses.write(UseItem {
                player: held.player,
                item: e,
            });
            eating.0 = 0.0;
        }
    }
}
