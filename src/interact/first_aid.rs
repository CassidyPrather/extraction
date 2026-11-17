//! First aid and food with the hands, in the headset.
//!
//! - **An injector** (the only medicine) goes in anywhere sensible
//!   ([`on_body`]): its tip to a thigh, a shin, the chest, the belly, the
//!   neck, an arm, or the other wrist or hand (an empty one), and the
//!   trigger. Not into the gun, nor what the other hand holds, nor the air.
//! - **Food** goes to your mouth: hold it there a moment. A bottle has to
//!   be tipped up to drink, and its cap off first ([`Capped`]): the other
//!   hand takes the cap (the trigger, as a gun's slide is taken) and
//!   twists it [`UNSCREW`] of a turn.
//!
//! Each, done, uses the item as the button used to (`items::usage`). The
//! button still works for medicine with the one-handed comfort setting.

use bevy::prelude::*;

use super::tools::{CLICK, TICK, pulse};
use super::{Hand, Held, InteractSystems};
use crate::interact::body::Torso;
use crate::items::usage::UseItem;
use crate::items::{Catalog, Item};
use crate::player::Player;
use crate::rig::haptics::Haptic;
use crate::rig::stance::Stance;
use crate::rig::{RigDriver, RigHand, RigInput, Side};
use extraction_core::items::Category;

/// How far round a bottle's cap is twisted to come off, turns.
pub const UNSCREW: f32 = 0.75;

/// A bottle with its cap on: it won't drink.
#[derive(Component, Debug, Default)]
pub struct Capped {
    /// How far it's been twisted, turns.
    pub twisted: f32,
}

/// An injector's tip, in its own space.
pub const NEEDLE: Vec3 = Vec3::new(0.0, 0.0, -0.065);
/// How near a limb's line its tip must be (m): a limb's half thickness
/// and a little more (the user: "I do not want to be pedantic about where
/// the player uses it").
pub const ON_LIMB: f32 = 0.13;
/// The same for the trunk, from the spine's line (m).
pub const ON_TRUNK: f32 = 0.2;
/// The thighs, from the torso anchor (it rides mid-chest): a body with no
/// `Stance` yet.
const THIGH: Vec3 = Vec3::new(0.11, -0.62, -0.04);
/// The upper arm, as a share of the way from the shoulder to the hand.
const UPPER_ARM: f32 = 0.45;
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
            (unscrew, inject, eat_and_drink).after(InteractSystems::Follow),
        );
    }
}

/// Seconds food has been at the mouth.
#[derive(Component, Debug, Default)]
pub struct Eating(pub f32);

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

/// Whether an injector's `tip`, held in the hand on `side`, is on the
/// body (`stance`, world space): the trunk from the neck down to the hips,
/// the legs, and the other arm: its upper arm always, and on to its hand
/// (`other`) while that hand holds nothing (not into the gun it holds).
/// The hand holding the injector can't reach its own arm.
pub fn on_body(stance: &Stance, tip: Vec3, side: Side, other: Vec3, other_empty: bool) -> bool {
    let near = |a: Vec3, b: Vec3, r: f32| {
        let ab = b - a;
        let t = ((tip - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        (a + ab * t).distance(tip) < r
    };
    let hips = (stance.hips[0] + stance.hips[1]) * 0.5;
    let legs = (0..2).any(|i| {
        near(stance.hips[i], stance.knees[i], ON_LIMB)
            || near(stance.knees[i], stance.ankles[i], ON_LIMB)
    });
    let shoulder = stance.shoulders[side.other().index()];
    let upper = shoulder + (other - shoulder) * UPPER_ARM;
    near(stance.neck, hips, ON_TRUNK)
        || legs
        || near(shoulder, upper, ON_LIMB)
        || (other_empty && near(shoulder, other, ON_LIMB))
}

/// An injector's tip on the body ([`on_body`]), and the trigger.
#[allow(clippy::too_many_arguments)]
fn inject(
    items: Query<(Entity, &Item, &Held, &GlobalTransform)>,
    torsos: Query<(&GlobalTransform, &ChildOf), With<Torso>>,
    stances: Query<&Stance>,
    players: Query<&Children>,
    hands: Query<(&RigHand, &Hand)>,
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
        let was = last.get(&e).copied().unwrap_or(false);
        let pulled = crate::rig::pressed(was, input.hand(held.side).trigger);
        last.insert(e, pulled);
        if !pulled || was {
            continue;
        }
        let tip = gt.transform_point(NEEDLE);
        let other = players.get(held.player).ok().and_then(|c| {
            c.iter()
                .filter_map(|c| hands.get(c).ok())
                .find(|(rh, _)| rh.side != held.side)
                .map(|(_, h)| (h.world.translation, h.holding.is_none()))
        });
        let on = match (stances.get(held.player), other) {
            (Ok(st), Some((at, empty))) => on_body(st, tip, held.side, at, empty),
            // No body worked out yet: the thighs, from the torso.
            _ => torsos
                .iter()
                .find(|(_, c)| c.parent() == held.player)
                .is_some_and(|(torso, _)| {
                    [THIGH, THIGH * Vec3::new(-1.0, 1.0, 1.0)]
                        .iter()
                        .any(|t| torso.transform_point(*t).distance(tip) < ON_LIMB)
                }),
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A standing body, facing -Z, right-handed, the left hand out in
    /// front at waist height.
    fn standing() -> (Stance, Vec3) {
        let mut st = Stance::default();
        st.neck = Vec3::new(0.0, 1.5, 0.0);
        st.shoulders = [Vec3::new(-0.2, 1.42, 0.0), Vec3::new(0.2, 1.42, 0.0)];
        st.hips = [Vec3::new(-0.1, 0.92, 0.0), Vec3::new(0.1, 0.92, 0.0)];
        st.knees = [Vec3::new(-0.1, 0.5, 0.0), Vec3::new(0.1, 0.5, 0.0)];
        st.ankles = [Vec3::new(-0.1, 0.08, 0.0), Vec3::new(0.1, 0.08, 0.0)];
        (st, Vec3::new(-0.15, 1.0, -0.35))
    }

    /// Generous (#102): a thigh, a shin, the belly, the chest, the neck,
    /// the other upper arm, and the other wrist and hand while it holds
    /// nothing all take it; the air, the gun in the other hand, and the
    /// injecting hand's own forearm don't.
    #[test]
    fn an_injector_goes_in_anywhere_sensible() {
        let (st, other) = standing();
        let right = Side::Right;
        let on = |tip: Vec3, empty: bool| on_body(&st, tip, right, other, empty);
        for (what, tip) in [
            ("the right thigh", Vec3::new(0.1, 0.75, -0.08)),
            ("the left thigh", Vec3::new(-0.1, 0.7, -0.1)),
            ("a shin", Vec3::new(0.1, 0.3, -0.06)),
            ("the belly", Vec3::new(0.0, 1.05, -0.15)),
            ("the chest", Vec3::new(0.05, 1.3, -0.15)),
            ("the neck", Vec3::new(0.06, 1.5, -0.05)),
            ("the left upper arm", Vec3::new(-0.22, 1.3, -0.1)),
            ("the left wrist", other + Vec3::new(0.0, 0.03, 0.06)),
            ("the left hand", other + Vec3::new(0.02, 0.05, 0.0)),
        ] {
            assert!(on(tip, true), "{what} didn't take it");
        }
        // The other hand holding the gun: its wrist and the gun don't.
        assert!(
            !on(other + Vec3::new(0.0, 0.05, -0.1), false),
            "the gun took it"
        );
        // Out in front, at arm's length, and off to the side.
        for tip in [
            Vec3::new(0.25, 1.2, -0.55),
            Vec3::new(0.6, 1.0, 0.0),
            Vec3::new(0.0, 1.9, -0.3),
        ] {
            assert!(!on(tip, true), "the air at {tip} took it");
        }
    }
}
