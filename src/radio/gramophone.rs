//! A wind-up gramophone: put a 78 on the platter, push the crank round a
//! few times, push the arm over onto the record. It plays as long as the
//! spring lasts, and as the spring runs down the record slows and sags
//! flat and stops, unless you wind it again while it plays. Push the arm
//! back to stop. The robots hear it.
//!
//! The model is `art/props/music.py`'s `gramophone` (platter, tonearm,
//! crank and horn as nodes); the records are items (`record_*`), their
//! music `extraction_core::music::tapes::RECORDS`.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::items::Mount;
use extraction_core::music::tapes;
use std::f32::consts::FRAC_PI_2;

use crate::interact::poke::PokeButton;
use crate::interact::{InteractEvent, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};

/// Seconds of running one push of the crank winds in, and the most the
/// spring holds.
pub const PER_TURN: f32 = 20.0;
pub const SPRING: f32 = 150.0;
/// Below this much spring the turntable starts to slow.
pub const SLOWS_BELOW: f32 = 12.0;
/// The platter's spindle, the crank's axle, the tonearm's pivot (the
/// model's nodes, in its space).
pub const SPINDLE: Vec3 = Vec3::new(-0.025, 0.16, 0.0);
pub const CRANK: Vec3 = Vec3::new(0.16, 0.075, 0.05);
pub const ARM: Vec3 = Vec3::new(0.11, 0.245, 0.13);
/// How far a robot hears it (m).
pub const HEARD_AT: f32 = 24.0;

#[derive(Component, Debug, Default)]
pub struct Gramophone {
    /// Seconds of running wound into the spring.
    pub spring: f32,
    pub arm_down: bool,
    /// The record on the platter, and how far into it the needle is (s).
    pub record: Option<Entity>,
    pub at: f32,
    /// How fast the platter turns, 0..1 of 78 rpm.
    pub speed: f32,
    pub noise_in: f32,
}

/// The crank (pushed round) and the arm (pushed over), as push buttons.
#[derive(Component, Debug, Clone, Copy)]
pub enum GramoPart {
    Crank(Entity),
    Arm(Entity),
}

/// The platter's socket, for records.
#[derive(Component)]
pub struct Platter;

/// How fast a spring with this much left turns the platter.
pub fn spring_speed(spring: f32) -> f32 {
    if spring <= 0.0 {
        0.0
    } else if spring >= SLOWS_BELOW {
        1.0
    } else {
        (spring / SLOWS_BELOW).sqrt()
    }
}

/// A gramophone standing here (on a table or the floor), its parts and
/// its body.
pub fn spawn(commands: &mut Commands, at: Transform) -> Entity {
    let g = commands
        .spawn((
            Name::new("Gramophone"),
            Gramophone::default(),
            at,
            Visibility::default(),
            RigidBody::Static,
            Collider::compound(vec![(
                Vec3::new(0.0, 0.08, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(0.32, 0.16, 0.32),
            )]),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
        ))
        .id();
    commands.spawn((
        Name::new("Platter"),
        Platter,
        Socket {
            kind: SocketKind::Mount(Mount::Record),
            occupant: None,
            radius: 0.16,
            seat: Transform::IDENTITY,
        },
        SocketOwner(g),
        Transform::from_translation(SPINDLE + Vec3::Y * 0.004),
        Visibility::default(),
        ChildOf(g),
    ));
    // The crank's knob, facing out of the case's side.
    let crank = commands
        .spawn((
            Name::new("Crank"),
            Transform::from_translation(CRANK + Vec3::new(0.06, 0.0, 0.0))
                .with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
            Visibility::default(),
            ChildOf(g),
        ))
        .id();
    commands.entity(crank).insert((
        GramoPart::Crank(g),
        PokeButton {
            target: crank,
            depth: 0.0,
            down: false,
        },
    ));
    // The arm's soundbox, pushed from above to swing it over.
    let arm = commands
        .spawn((
            Name::new("Arm"),
            Transform::from_translation(ARM + Vec3::new(-0.05, 0.03, -0.05))
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            Visibility::default(),
            ChildOf(g),
        ))
        .id();
    commands.entity(arm).insert((
        GramoPart::Arm(g),
        PokeButton {
            target: arm,
            depth: 0.0,
            down: false,
        },
    ));
    g
}

/// Records onto and off the platter; the crank and the arm pushed.
pub(super) fn work_gramophones(
    mut events: MessageReader<InteractEvent>,
    parts: Query<&GramoPart>,
    platters: Query<&SocketOwner, With<Platter>>,
    mut grams: Query<&mut Gramophone>,
) {
    for ev in events.read() {
        match *ev {
            InteractEvent::Pressed { item, .. } => match parts.get(item) {
                Ok(GramoPart::Crank(g)) => {
                    if let Ok(mut gr) = grams.get_mut(*g) {
                        gr.spring = (gr.spring + PER_TURN).min(SPRING);
                    }
                }
                Ok(GramoPart::Arm(g)) => {
                    if let Ok(mut gr) = grams.get_mut(*g) {
                        gr.arm_down = !gr.arm_down;
                    }
                }
                Err(_) => {}
            },
            InteractEvent::Socketed { item, socket } => {
                if let Ok(owner) = platters.get(socket) {
                    if let Ok(mut gr) = grams.get_mut(owner.0) {
                        gr.record = Some(item);
                        gr.at = 0.0;
                    }
                }
            }
            InteractEvent::Unsocketed { item, socket } => {
                if let Ok(owner) = platters.get(socket) {
                    if let Ok(mut gr) = grams.get_mut(owner.0) {
                        if gr.record == Some(item) {
                            gr.record = None;
                            gr.arm_down = false;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// The spring runs the platter; the needle runs along the record.
pub(super) fn run_gramophones(
    mut grams: Query<(&mut Gramophone, &GlobalTransform)>,
    records: Query<&Item>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    let dt = time.delta_secs();
    for (mut g, gt) in &mut grams {
        let len = g
            .record
            .and_then(|r| records.get(r).ok())
            .and_then(|i| tapes::record(&catalog.get(i.kind()).id))
            .map(tapes::record_length);
        // The motor runs whenever it's wound and the brake (the arm off
        // the record) is off; with nothing under the needle, it doesn't.
        let running = g.arm_down && len.is_some() && g.spring > 0.0;
        let want = if running { spring_speed(g.spring) } else { 0.0 };
        // The platter's weight: up to speed in about a second, down a bit
        // slower.
        let k = if want > g.speed { 1.5 } else { 1.0 };
        g.speed += (want - g.speed) * (k * dt).min(1.0);
        if g.speed < 0.01 && want == 0.0 {
            g.speed = 0.0;
        }
        g.spring = (g.spring - g.speed * dt).max(0.0);
        if let Some(len) = len {
            if g.arm_down {
                g.at = (g.at + g.speed * dt).min(len);
            }
        }
        g.noise_in -= dt;
        if g.speed > 0.3 && g.noise_in <= 0.0 && len.is_some_and(|l| g.at < l) {
            g.noise_in = 3.0;
            noise.write(crate::robots::Noise {
                at: gt.translation(),
                radius: HEARD_AT,
                source: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spring_slows_it_then_stops_it() {
        assert_eq!(spring_speed(SPRING), 1.0);
        assert_eq!(spring_speed(SLOWS_BELOW), 1.0);
        let low = spring_speed(SLOWS_BELOW * 0.25);
        assert!((low - 0.5).abs() < 1e-4);
        assert_eq!(spring_speed(0.0), 0.0);
    }
}
