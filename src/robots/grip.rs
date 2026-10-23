//! Robots fight a hand (`interact::fight`, Hands' side: the shove, the
//! grip, the pull to the hand). Held or shoved, a machine knows who did it
//! and turns on them; held, it fights the grip its own way:
//!
//! - a **drone** thrusts away from the hand, harder the longer it's held
//!   ([`DRONE_THRUST`]), until the arm's pull (`fight::HOLD`) loses and it
//!   tears free;
//! - a **walker** held by a leg kicks it free after [`KICK_AFTER`] (a
//!   crawler sooner, and its emitter charges at whoever held it): the hand
//!   is knocked off, hard;
//! - the **heavy** doesn't notice.

use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashMap;

use super::{Awareness, Robot};
use crate::interact::fight::{Gripped, RobotGrip, Shoved};
use crate::rig::RigHand;
use crate::rig::haptics::Haptic;

/// A held drone's thrust away from the hand, N: this at first, rising by
/// [`DRONE_RAMP`] a second held, at most [`DRONE_MOST`]. Past the arm's
/// pull (400 N) within a second or so, and it's gone.
pub const DRONE_THRUST: f32 = 220.0;
pub const DRONE_RAMP: f32 = 400.0;
pub const DRONE_MOST: f32 = 1200.0;
/// Seconds a walker lets a leg be held before kicking it free; a crawler,
/// [`CRAWLER_KICK_AFTER`].
pub const KICK_AFTER: f32 = 0.6;
pub const CRAWLER_KICK_AFTER: f32 = 0.3;

pub struct GripPlugin;

impl Plugin for GripPlugin {
    fn build(&self, app: &mut App) {
        // (Hands' messages, registered here too: an app with the robots
        // but not the hands still runs.)
        app.add_message::<KickedFree>()
            .add_message::<Gripped>()
            .add_message::<Shoved>()
            .add_systems(
                FixedUpdate,
                (turn_on_hands, fight_grips)
                    .chain()
                    .after(crate::interact::InteractSystems::Follow),
            );
    }
}

/// A walker kicked a held leg free of a hand.
#[derive(Message, Clone, Copy, Debug)]
pub struct KickedFree {
    pub robot: Entity,
    pub at: Vec3,
}

/// How a robot fights being held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fights {
    /// Thrusts away (a drone).
    Thrust,
    /// Kicks free after this long (a walker, a crawler).
    Kick(f32),
    /// Doesn't notice (the heavy).
    Ignores,
}

/// How a robot of this make fights a grip.
pub fn fights(drone: bool, heavy: bool, crawler: bool) -> Fights {
    if heavy {
        Fights::Ignores
    } else if drone {
        Fights::Thrust
    } else if crawler {
        Fights::Kick(CRAWLER_KICK_AFTER)
    } else {
        Fights::Kick(KICK_AFTER)
    }
}

/// A held drone's thrust away after `held` seconds, N.
pub fn drone_thrust(held: f32) -> f32 {
    (DRONE_THRUST + DRONE_RAMP * held).min(DRONE_MOST)
}

/// Held or shoved, it knows who and where, and turns on them.
fn turn_on_hands(
    mut gripped: MessageReader<Gripped>,
    mut shoved: MessageReader<Shoved>,
    mut aware: Query<(&mut Awareness, &Robot)>,
    heavies: Query<(), With<super::heavy::Heavy>>,
) {
    let felt = gripped
        .read()
        .map(|g| (g.robot, g.by, g.at))
        .chain(shoved.read().map(|s| (s.robot, s.by, s.at)));
    for (robot, by, at) in felt {
        if heavies.contains(robot) {
            continue;
        }
        let Ok((mut a, r)) = aware.get_mut(robot) else {
            continue;
        };
        if r.wrecked {
            continue;
        }
        a.target = Some(by);
        a.last_known = Some(at);
        a.uncertainty = a.uncertainty.min(0.5);
        a.suspicion = 1.0;
        a.news = true;
    }
}

/// Held, it fights: a drone thrusts away, a walker kicks the leg free.
#[allow(clippy::type_complexity)]
fn fight_grips(
    mut commands: Commands,
    mut gripped: MessageReader<Gripped>,
    robots: Query<(
        &Robot,
        &GlobalTransform,
        Has<super::drone::Drone>,
        Has<super::heavy::Heavy>,
        Has<super::indoor::Crawler>,
        Option<&ComputedMass>,
    )>,
    mut velocities: Query<&mut LinearVelocity>,
    hands: Query<(Entity, &RobotGrip, &RigHand)>,
    mut held: Local<HashMap<Entity, f32>>,
    mut kicked: MessageWriter<KickedFree>,
    mut charges: MessageWriter<super::indoor::CrawlerCharge>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    // Each robot held this tick, once (two hands on it: one fight), and
    // which way the hands would pull it.
    let mut this_tick: HashMap<Entity, (Vec3, Vec3)> = HashMap::new();
    for g in gripped.read() {
        let e = this_tick.entry(g.robot).or_insert((Vec3::ZERO, g.at));
        e.0 += g.strain;
    }
    // Let go of: the count starts again.
    held.retain(|e, _| this_tick.contains_key(e));
    for (robot, (strain, at)) in this_tick {
        let Ok((r, gt, drone, heavy, crawler, mass)) = robots.get(robot) else {
            continue;
        };
        if r.wrecked {
            continue;
        }
        let t = held.entry(robot).or_insert(0.0);
        *t += dt;
        match fights(drone, heavy, crawler) {
            Fights::Ignores => {}
            Fights::Thrust => {
                // Away from the hands and up, harder the longer it's held.
                let away = (-strain).normalize_or(Vec3::ZERO) + Vec3::Y * 0.4;
                let m = mass.map_or(6.0, |m| m.value()).max(0.5);
                if let Ok(mut v) = velocities.get_mut(robot) {
                    v.0 += away.normalize_or(Vec3::Y) * drone_thrust(*t) / m * dt;
                }
            }
            Fights::Kick(after) => {
                if *t < after {
                    continue;
                }
                // Kicked free: every hand on it knocked off, hard.
                for (hand, grip, rig) in &hands {
                    if grip.robot != robot {
                        continue;
                    }
                    commands.entity(hand).remove::<RobotGrip>();
                    haptics.write(Haptic {
                        side: rig.side,
                        amplitude: 0.9,
                        seconds: 0.15,
                    });
                }
                kicked.write(KickedFree { robot, at });
                if crawler {
                    // And its emitter charges at whoever held it.
                    charges.write(super::indoor::CrawlerCharge {
                        crawler: robot,
                        at: gt.translation(),
                    });
                }
                held.remove(&robot);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each make fights its own way: a drone thrusts, a walker kicks, a
    /// crawler kicks sooner, the heavy doesn't notice.
    #[test]
    fn each_fights_its_own_way() {
        assert_eq!(fights(true, false, false), Fights::Thrust);
        assert_eq!(fights(false, false, false), Fights::Kick(KICK_AFTER));
        assert_eq!(fights(false, false, true), Fights::Kick(CRAWLER_KICK_AFTER));
        assert!(CRAWLER_KICK_AFTER < KICK_AFTER);
        assert_eq!(fights(false, true, false), Fights::Ignores);
    }

    /// A held drone's thrust beats the arm's pull within a second or so.
    #[test]
    fn a_held_drone_out_thrusts_the_arm() {
        let hold = crate::interact::fight::HOLD;
        assert!(drone_thrust(0.0) < hold, "not at once");
        assert!(drone_thrust(1.2) > hold, "within a second or so");
        assert!(drone_thrust(100.0) <= DRONE_MOST);
    }
}
