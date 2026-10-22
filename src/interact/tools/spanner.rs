//! The spanner. Hold it by the handle and put its head to a bolt: it
//! seats there. Then swing the handle round the bolt, counterclockwise
//! looking down on its head, and the bolt turns with it; swing back and
//! it ratchets, counting nothing. A bolt takes a turn or two to come out.
//!
//! The bolts are `destruction::hooks::Unboltable`, and a seated spanner
//! sends `destruction::hooks::Turned` each tick it undoes one. Whoever owns
//! the bolt counts the turns and decides when it's out (a wreck's servo
//! pack: `robots::salvage`).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{CLICK, KNOCK, TICK, pulse};
pub use crate::destruction::hooks::{Turned, Unboltable};
use crate::interact::{GrabPoint, Grabbable, GripRole, Hand, Held, InteractSystems};
use crate::items::{Catalog, Item};
use crate::rig::haptics::Haptic;

/// In the spanner's space: the handle (+Z) and the head (-Z).
pub const GRIP: Vec3 = Vec3::new(0.0, 0.0, 0.1);
pub const HEAD: Vec3 = Vec3::new(0.0, 0.0, -0.13);
/// The head seats on a bolt within this of it (m), as a crowbar's claw
/// on a seam.
const SEAT: f32 = 0.06;
/// The hand pulled further than this from the bolt takes the spanner off
/// it (m), and nearer the bolt's axis than this it can't turn it.
const SLIP: f32 = 0.45;
const NEAR_AXIS: f32 = 0.03;
/// A click in the hand every this much turned (radians).
const CLICK_EVERY: f32 = 0.5;

pub struct SpannerPlugin;

impl Plugin for SpannerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(setup_spanner)
            .add_systems(FixedUpdate, turn.after(InteractSystems::Follow));
    }
}

/// A spanner, and the bolt its head is on.
#[derive(Component, Debug, Default)]
pub struct Spanner {
    pub on: Option<OnBolt>,
}

#[derive(Clone, Copy, Debug)]
pub struct OnBolt {
    pub bolt: Entity,
    /// The way the handle last pointed from the bolt, across its axis.
    last: Vec3,
    /// Turned since the last click (radians).
    since: f32,
}

fn setup_spanner(
    add: On<Add, Item>,
    mut commands: Commands,
    items: Query<&Item>,
    catalog: Res<Catalog>,
) {
    let Ok(item) = items.get(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != "spanner" {
        return;
    }
    commands.entity(add.entity).insert((
        Spanner::default(),
        Grabbable {
            points: vec![GrabPoint::new(
                Transform::from_translation(GRIP),
                GripRole::Primary,
                0.12,
            )],
        },
    ));
}

/// How far `from` has to turn to `to` about `axis`, counterclockwise
/// looking down the axis positive (radians, -π..π).
pub fn turn_about(axis: Vec3, from: Vec3, to: Vec3) -> f32 {
    axis.dot(from.cross(to)).atan2(from.dot(to))
}

/// Seat the head on a bolt it touches; while seated, keep it on the bolt
/// with the handle toward the hand, and count the swing round it.
#[allow(clippy::type_complexity)]
fn turn(
    mut spanners: Query<(
        &mut Spanner,
        &GlobalTransform,
        Option<&Held>,
        &mut Position,
        &mut Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
    hands: Query<&Hand>,
    bolts: Query<(Entity, &Unboltable, &GlobalTransform)>,
    mut turned: MessageWriter<Turned>,
    mut haptics: MessageWriter<Haptic>,
) {
    for (mut sp, gt, held, mut pos, mut rot, mut lin, mut ang) in &mut spanners {
        let Some(held) = held else {
            sp.on = None;
            continue;
        };
        let Ok(hand) = hands.get(held.hand) else {
            continue;
        };
        let h = hand.world.translation;
        let across = |axis: Vec3, at: Vec3| {
            let arm = h - at;
            arm - axis * arm.dot(axis)
        };
        let Some(mut on) = sp.on else {
            let head = gt.transform_point(HEAD);
            let found = bolts
                .iter()
                .find(|(_, _, bgt)| bgt.translation().distance(head) < SEAT);
            if let Some((bolt, u, bgt)) = found {
                let axis = (bgt.rotation() * u.axis).normalize_or(Vec3::Y);
                let flat = across(axis, bgt.translation());
                if flat.length() >= NEAR_AXIS {
                    sp.on = Some(OnBolt {
                        bolt,
                        last: flat.normalize(),
                        since: 0.0,
                    });
                    pulse(&mut haptics, held.side, CLICK);
                }
            }
            continue;
        };
        let Ok((_, u, bgt)) = bolts.get(on.bolt) else {
            // Out.
            sp.on = None;
            pulse(&mut haptics, held.side, KNOCK);
            continue;
        };
        let bolt = bgt.translation();
        let axis = (bgt.rotation() * u.axis).normalize_or(Vec3::Y);
        let flat = across(axis, bolt);
        if (h - bolt).length() > SLIP || flat.length() < NEAR_AXIS {
            sp.on = None;
            pulse(&mut haptics, held.side, TICK);
            continue;
        }
        let dir = flat.normalize();
        let angle = turn_about(axis, on.last, dir);
        if angle > 0.0 {
            turned.write(Turned {
                target: on.bolt,
                angle,
                by: held.player,
            });
            on.since += angle;
            if on.since >= CLICK_EVERY {
                on.since = 0.0;
                pulse(&mut haptics, held.side, TICK);
            }
        }
        // Back or forward, the handle is where it is now (a ratchet).
        on.last = dir;
        sp.on = Some(on);
        // Held on the bolt, the handle toward the hand.
        let r = Quat::from_rotation_arc(Vec3::Z, (h - bolt).normalize_or(Vec3::Z));
        rot.0 = r;
        pos.0 = bolt - r * HEAD;
        lin.0 = Vec3::ZERO;
        ang.0 = Vec3::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter turn counterclockwise about +Y (looking down on it) is
    /// +π/2; clockwise, -π/2.
    #[test]
    fn turning_counts_counterclockwise() {
        let a = turn_about(Vec3::Y, Vec3::X, Vec3::NEG_Z);
        assert!((a - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{a}");
        let b = turn_about(Vec3::Y, Vec3::X, Vec3::Z);
        assert!((b + std::f32::consts::FRAC_PI_2).abs() < 1e-4, "{b}");
    }
}
