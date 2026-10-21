//! First aid and food with the hands, in the headset.
//!
//! - **A dressing** is wound round the other forearm. Hold it against the
//!   bare forearm of your empty hand and circle it round, one and a half
//!   turns.
//! - **An injector** goes in the thigh. Put its tip to your thigh and pull
//!   the trigger.
//! - **Food** goes to your mouth: hold it there a moment. A bottle has to
//!   be tipped up to drink.
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
/// The forearm, from the hand back (m), and how far round it a dressing
/// held to it may be.
const FOREARM: f32 = 0.26;
const ROUND: f32 = 0.12;
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
        app.add_systems(
            FixedUpdate,
            (wrap_dressings, inject, eat_and_drink).after(InteractSystems::Follow),
        );
    }
}

/// Where a dressing is in its winding: the angle it was last at round the
/// forearm, and how far round it's gone.
#[derive(Component, Debug, Default)]
pub struct Winding {
    last: Option<f32>,
    pub turns: f32,
    away: f32,
}

/// Seconds food has been at the mouth.
#[derive(Component, Debug, Default)]
pub struct Eating(pub f32);

/// A dressing circled round the other forearm.
#[allow(clippy::type_complexity)]
fn wrap_dressings(
    mut commands: Commands,
    mut items: Query<(Entity, &Item, &Held, &GlobalTransform, Option<&mut Winding>)>,
    players: Query<&Children, With<Player>>,
    hands: Query<(&RigHand, &Hand)>,
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
        // The other hand: empty, its forearm running back from it.
        let Ok(children) = players.get(held.player) else {
            continue;
        };
        let arm = children.iter().find_map(|c| {
            hands
                .get(c)
                .ok()
                .filter(|(rh, h)| rh.side != held.side && h.holding.is_none())
                .map(|(_, h)| h.world)
        });
        let Some(arm) = arm else {
            continue;
        };
        let local = arm
            .compute_affine()
            .inverse()
            .transform_point3(gt.translation());
        let along = local.z;
        let round = local.truncate();
        if !(0.0..=FOREARM).contains(&along) || round.length() > ROUND || round.length() < 0.01 {
            w.away += time.delta_secs();
            if w.away > 0.6 {
                *w = Winding::default();
            }
            continue;
        }
        w.away = 0.0;
        let a = round.y.atan2(round.x);
        if let Some(last) = w.last {
            let mut d = a - last;
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            } else if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            let before = (w.turns * 4.0).floor();
            w.turns += d.abs() / std::f32::consts::TAU;
            if (w.turns * 4.0).floor() > before {
                // A tick each quarter turn, in both hands.
                pulse(&mut haptics, held.side, TICK);
                pulse(&mut haptics, held.side.other(), TICK);
            }
        }
        w.last = Some(a);
        if w.turns >= WRAP_TURNS {
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
    mut items: Query<(Entity, &Item, &Held, &GlobalTransform, Option<&mut Eating>)>,
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
    for (e, item, held, gt, eating) in &mut items {
        let def = catalog.get(item.kind());
        if def.category != Category::Food {
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
