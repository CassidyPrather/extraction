//! The field radio's tuning knob. Hold the radio in one hand, take the
//! knob with the other (squeeze, or the trigger) and turn your wrist: the
//! dial runs up the AM band, through the static, past whatever is on the
//! air here (`extraction_core::music::stations`). Turned all the way down
//! it clicks off. Let go and take it again to keep turning, as you would a
//! real one.
//!
//! The robots' chatter (`raid::chatter`) breaks in whatever it's tuned to:
//! it's the radio's job.

use bevy::prelude::*;
use extraction_core::music::stations::{self, BAND, Listen, Reception};
use std::collections::HashMap;

use crate::interact::{GrabPoint, Grabbable, GripRole, Hand, InSocket};
use crate::items::{Catalog, Item};

/// The knob's id among the radio's grab points.
pub const DIAL_HANDLE: u8 = 31;
/// Where the knob is on the radio: the model's `dial` node, the larger
/// top knob, turning about the radio's +Y (`art/items/gadgets.py`).
pub const KNOB: Vec3 = Vec3::new(-0.0195, 0.023, 0.0017);
/// How close the hand must come to take it (m).
pub const KNOB_REACH: f32 = 0.05;
/// kHz per radian of wrist turn: a quarter turn of the wrist is about
/// 60 kHz, and a station (`stations::SPREAD`) is a few degrees either way.
pub const KHZ_PER_RAD: f32 = 40.0;
/// Below the band: off.
pub const OFF: f32 = BAND.0 - 10.0;
/// While it plays out loud (not in a pocket), how far a robot hears it.
pub const HEARD_AT: f32 = 14.0;

/// A radio's dial, and what it picks up there.
#[derive(Component, Debug, Clone)]
pub struct Tuner {
    pub khz: f32,
    pub reception: Reception,
    /// Out loud: in a hand or set down (in a pocket or the pack it's
    /// muffled to nothing; it still takes the chatter).
    pub aloud: bool,
    pub noise_in: f32,
}

impl Tuner {
    pub fn on(&self) -> bool {
        self.khz >= BAND.0
    }
}

impl Default for Tuner {
    fn default() -> Self {
        Self {
            khz: OFF,
            reception: Reception {
                stations: Vec::new(),
                static_gain: 0.0,
            },
            aloud: false,
            noise_in: 0.0,
        }
    }
}

/// The dial clicked past a detent: on, off.
#[derive(Message, Clone, Copy, Debug)]
pub struct DialClick {
    pub radio: Entity,
    pub on: bool,
}

pub(super) fn tuner_parts(
    add: On<Add, Item>,
    mut commands: Commands,
    mut items: Query<(&Item, &mut Grabbable)>,
    catalog: Res<Catalog>,
) {
    let Ok((item, mut grab)) = items.get_mut(add.entity) else {
        return;
    };
    if catalog.get(item.kind()).id != "radio" {
        return;
    }
    grab.points.push(GrabPoint::new(
        Transform::from_translation(KNOB),
        GripRole::Handle(DIAL_HANDLE),
        KNOB_REACH,
    ));
    commands.entity(add.entity).insert(Tuner::default());
}

/// The hand's turn about `axis`: its right-hand direction flattened onto
/// the knob's face.
fn wrist(rot: Quat, axis: Vec3) -> Vec3 {
    let r = rot * Vec3::X;
    (r - axis * r.dot(axis)).normalize_or_zero()
}

/// Clockwise from above, up the band.
pub(super) fn turn_dials(
    hands: Query<(Entity, &Hand)>,
    mut radios: Query<(&GlobalTransform, &mut Tuner)>,
    mut taken: Local<HashMap<Entity, (Vec3, f32)>>,
    mut clicks: MessageWriter<DialClick>,
) {
    taken.retain(|h, _| {
        hands
            .get(*h)
            .is_ok_and(|(_, hand)| matches!(hand.handle, Some((_, DIAL_HANDLE))))
    });
    for (hand_e, hand) in &hands {
        let Some((radio, DIAL_HANDLE)) = hand.handle else {
            continue;
        };
        let Ok((gt, mut tuner)) = radios.get_mut(radio) else {
            continue;
        };
        let axis = gt.rotation() * Vec3::Y;
        let now = wrist(hand.world.rotation, axis);
        let (from, start) = *taken.entry(hand_e).or_insert((now, tuner.khz));
        let turned = from.cross(now).dot(-axis).atan2(from.dot(now));
        let khz = (start + turned * KHZ_PER_RAD).clamp(OFF, BAND.1);
        if (khz - tuner.khz).abs() > 1e-3 {
            let was = tuner.on();
            tuner.khz = khz;
            if was != tuner.on() {
                clicks.write(DialClick {
                    radio,
                    on: tuner.on(),
                });
            }
        }
    }
}

/// What each radio picks up where it is: the valley, how high, under
/// ground or not, the story so far.
pub(super) fn receive(
    mut radios: Query<(&GlobalTransform, &mut Tuner, Option<&InSocket>)>,
    sockets: Query<&crate::interact::Socket>,
    zone: Option<Res<crate::world::Zone>>,
    raid: Option<Res<crate::raid::Raid>>,
    progression: Option<Res<crate::progression::Progression>>,
    launch: Option<Res<crate::LaunchOptions>>,
    mut floor: Local<Option<f32>>,
) {
    let Some(zone) = zone else {
        return;
    };
    // The valley floor: the low tenth of its ground.
    let floor = *floor.get_or_insert_with(|| {
        let mut h: Vec<f32> = zone.hm.heights.iter().copied().filter(|h| h.is_finite()).collect();
        h.sort_by(f32::total_cmp);
        h.get(h.len() / 10).copied().unwrap_or(0.0)
    });
    let valley = launch.as_ref().map_or("", |l| l.map.as_str());
    let recalled = raid.as_ref().is_some_and(|r| r.recalled);
    let rank = progression.as_ref().map_or(0, |p| p.0.tier);
    for (gt, mut tuner, in_socket) in &mut radios {
        let at = gt.translation();
        let ground = zone.on_ground(at.x, at.z).y;
        let listen = Listen {
            valley,
            height: at.y - floor,
            underground: at.y < ground - 2.5,
            recalled,
            rank,
        };
        tuner.reception = if tuner.on() {
            stations::reception(tuner.khz, &listen)
        } else {
            Reception {
                stations: Vec::new(),
                static_gain: 0.0,
            }
        };
        // Out loud unless it's in a pouch, holster or the pack.
        tuner.aloud = in_socket
            .and_then(|s| sockets.get(s.socket).ok())
            .is_none_or(|s| !matches!(s.kind, crate::interact::SocketKind::Body(_)));
    }
}

/// A radio playing out loud is heard, a little: a station's music more
/// than a hiss.
pub(super) fn radio_noise(
    mut radios: Query<(&GlobalTransform, &mut Tuner)>,
    time: Res<Time>,
    mut noise: MessageWriter<crate::robots::Noise>,
) {
    for (gt, mut t) in &mut radios {
        if !t.on() || !t.aloud {
            continue;
        }
        t.noise_in -= time.delta_secs();
        if t.noise_in > 0.0 {
            continue;
        }
        t.noise_in = super::deck::NOISE_EVERY;
        let carried: f32 = t.reception.stations.iter().map(|s| s.gain).sum();
        noise.write(crate::robots::Noise {
            at: gt.translation(),
            radius: HEARD_AT * (0.5 + 0.5 * carried.min(1.0)),
            source: None,
        });
    }
}
