//! Gestures in the headset that the world can read: a hand over your mouth
//! to quieten your breathing, and a raised open palm.
//!
//! [`Gestures`] on each player says what they're doing, for robots now and
//! other players later, and [`Gestured`] fires as one starts. What a
//! robot makes of a raised palm is its own business (`robots`).
//!
//! Breathing: winded (stamina low after a sprint) you breathe hard, and a
//! robot near enough hears it. A hand over your mouth cuts that to a third.

use bevy::prelude::*;

use super::tools::{TICK, pulse};
use super::{Hand, InteractSystems};
use crate::player::{Body, Player};
use crate::rig::haptics::Haptic;
use crate::rig::{LocalPlayer, RigDriver, RigHand, RigInput, Side};
use crate::robots::Noise;

/// The mouth, from the eyes, and how near a palm must be to cover it (the
/// hand's origin is the middle of the fist, a few centimetres off the
/// palm).
const MOUTH: Vec3 = Vec3::new(0.0, -0.08, -0.1);
const OVER_MOUTH: f32 = 0.09;
/// A raised palm: the hand this far over the eyes at least, pointing up,
/// open, for this long.
const RAISED: f32 = -0.05;
const HOLD: f32 = 0.3;
/// How far hard breathing carries when quite winded, metres; and how
/// often it's heard.
pub const BREATH: f32 = 6.0;
const BREATH_EVERY: f32 = 1.0;
/// A hand over the mouth leaves this share of it.
pub const HUSHED: f32 = 1.0 / 3.0;

pub struct GesturesPlugin;

impl Plugin for GesturesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Gestured>().add_systems(
            FixedUpdate,
            (add_gestures, read_gestures, breathe)
                .chain()
                .after(InteractSystems::Track),
        );
    }
}

/// What a player's hands are saying.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Gestures {
    /// A hand over the mouth.
    pub hush: bool,
    /// An open palm raised over the head.
    pub palm: bool,
    /// Seconds the palm has been up (it counts after `HOLD`).
    raised_for: f32,
    /// Seconds to the next breath heard.
    next_breath: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Hush,
    RaisedPalm,
}

/// A gesture began.
#[derive(Message, Clone, Copy, Debug)]
pub struct Gestured {
    pub player: Entity,
    pub gesture: Gesture,
    pub side: Side,
}

fn add_gestures(mut commands: Commands, players: Query<Entity, (With<Player>, Without<Gestures>)>) {
    for p in &players {
        commands.entity(p).insert(Gestures::default());
    }
}

/// The headset player's hands, read for gestures.
#[allow(clippy::type_complexity)]
fn read_gestures(
    mut players: Query<
        (
            Entity,
            &GlobalTransform,
            &RigInput,
            &Children,
            &mut Gestures,
        ),
        With<LocalPlayer>,
    >,
    hands: Query<(&RigHand, &Hand)>,
    driver: Res<RigDriver>,
    mut said: MessageWriter<Gestured>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    if *driver != RigDriver::Xr {
        return;
    }
    for (player, root, input, children, mut g) in &mut players {
        let head = root.mul_transform(input.head);
        let mouth = head.transform_point(MOUTH);
        let mut hush = None;
        let mut palm = None;
        for c in children.iter() {
            let Ok((rh, hand)) = hands.get(c) else {
                continue;
            };
            if hand.holding.is_some() || hand.handle.is_some() {
                continue;
            }
            let p = hand.world.translation;
            if p.distance(mouth) < OVER_MOUTH {
                hush = Some(rh.side);
            }
            let open = input.hand(rh.side).grip < 0.3;
            let up = (hand.world.rotation * Vec3::NEG_Z).dot(Vec3::Y) > 0.6;
            if open && up && p.y > head.translation().y + RAISED {
                palm = Some(rh.side);
            }
        }
        if let (Some(side), false) = (hush, g.hush) {
            said.write(Gestured {
                player,
                gesture: Gesture::Hush,
                side,
            });
            pulse(&mut haptics, side, TICK);
        }
        g.hush = hush.is_some();
        g.raised_for = if palm.is_some() {
            g.raised_for + time.delta_secs()
        } else {
            0.0
        };
        let raised = g.raised_for >= HOLD;
        if let (Some(side), true, false) = (palm, raised, g.palm) {
            said.write(Gestured {
                player,
                gesture: Gesture::RaisedPalm,
                side,
            });
            pulse(&mut haptics, side, TICK);
        }
        g.palm = raised;
    }
}

/// Winded, you're heard breathing; less with a hand over your mouth.
fn breathe(
    mut players: Query<(Entity, &GlobalTransform, &RigInput, &Body, &mut Gestures)>,
    mut noise: MessageWriter<Noise>,
    time: Res<Time>,
) {
    for (player, root, input, body, mut g) in &mut players {
        g.next_breath -= time.delta_secs();
        if g.next_breath > 0.0 {
            continue;
        }
        g.next_breath = BREATH_EVERY;
        let winded = (1.0 - body.stamina / 0.6).clamp(0.0, 1.0);
        if winded <= 0.0 {
            continue;
        }
        let radius = BREATH * winded * if g.hush { HUSHED } else { 1.0 };
        noise.write(Noise {
            at: root.mul_transform(input.head).translation(),
            radius,
            source: Some(player),
        });
    }
}
