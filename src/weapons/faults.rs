//! The game side of what goes wrong, each with its own feel so a hand can
//! tell which it is before an eye does:
//!
//! - A stovepipe jolts; a failure to feed is a dead click; a double feed
//!   grinds. (The mechanism's rules are `extraction_core::firearm`.)
//! - A squib (a re-packed round with too little powder) is a soft pop and
//!   no kick: the bullet is in the bore. Push a cleaning rod down the bore
//!   from the muzzle to knock it out ([`rod_bores`]). Fire on it instead
//!   and the barrel bulges: a violent jolt, the gun mostly ruined, and the
//!   hand that held it hurt.
//! - A filthy Mosin's bolt sticks shut after the shot: lifting the handle
//!   only grinds. Slap it up hard (a palm under the handle,
//!   [`STICKY_SLAP`]) and it breaks free (in `work_handles`).
//! - A worn, dirty SKS can slam-fire: the round goes off as the bolt closes
//!   on it, trigger or no. Cleaning and repair are the cure.

use std::collections::HashMap;

use bevy::prelude::*;
use extraction_core::firearm::{Event as FireEvent, Malfunction};

use super::ballistics::Hit;
use super::{Gun, GunEvent};
use crate::interact::{Hand, Held};
use crate::items::{Catalog, Item};
use crate::rig::haptics::Haptic;

/// How fast (m/s, along the gun's up) a palm must come up under a stuck
/// bolt's handle to break it free.
pub const STICKY_SLAP: f32 = 1.2;
/// How far down the bore from the muzzle a rod's tip must go to push a
/// squib's bullet out, and how near the bore's line it must be, metres.
pub const ROD_DEPTH: f32 = 0.1;
pub const ROD_RADIUS: f32 = 0.025;
/// What a bulging barrel does to the hand on it.
pub const BULGE_HARM: f32 = 15.0;

pub struct FaultsPlugin;

impl Plugin for FaultsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (fault_feel, grind_stuck_bolts))
            .add_systems(FixedUpdate, rod_bores.before(super::triggers));
    }
}

/// Each fault's feel in the hand that holds the gun, and a bulge's harm.
fn fault_feel(
    mut events: MessageReader<GunEvent>,
    held: Query<(&Held, &GlobalTransform)>,
    bodies: Query<(Entity, &ChildOf), With<crate::player::BodyCollider>>,
    mut haptics: MessageWriter<Haptic>,
    mut hits: MessageWriter<Hit>,
) {
    for ev in events.read() {
        let feel = match ev.event {
            FireEvent::Malfunctioned(Malfunction::FailureToEject) => (0.55, 0.03),
            FireEvent::Malfunctioned(Malfunction::FailureToFeed) => (0.12, 0.015),
            FireEvent::Malfunctioned(Malfunction::DoubleFeed) => (0.85, 0.12),
            FireEvent::Squib { .. } => (0.2, 0.03),
            FireEvent::Bulged { .. } => (1.0, 0.35),
            FireEvent::BoltStuck => (0.6, 0.08),
            FireEvent::BoltFreed => (0.4, 0.02),
            FireEvent::Rodded => (0.3, 0.04),
            _ => continue,
        };
        let Ok((h, gt)) = held.get(ev.gun) else {
            continue;
        };
        haptics.write(Haptic {
            side: h.side,
            amplitude: feel.0,
            seconds: feel.1,
        });
        if matches!(ev.event, FireEvent::Bulged { .. }) {
            if let Some((body, _)) = bodies.iter().find(|(_, p)| p.parent() == h.player) {
                hits.write(Hit {
                    entity: body,
                    point: gt.translation(),
                    normal: Vec3::Y,
                    velocity: Vec3::ZERO,
                    damage: BULGE_HARM,
                    shooter: None,
                    mass_g: 0.0,
                    load: Default::default(),
                });
            }
        }
    }
}

/// A hand lifting a stuck bolt's handle without the slap feels it grind.
fn grind_stuck_bolts(
    hands: Query<(Entity, &Hand, &crate::rig::RigHand)>,
    guns: Query<(&Gun, &GlobalTransform)>,
    time: Res<Time>,
    mut last: Local<HashMap<Entity, f32>>,
    mut haptics: MessageWriter<Haptic>,
) {
    let now = time.elapsed_secs();
    for (e, hand, rig) in &hands {
        let Some((g, 0)) = hand.handle else {
            continue;
        };
        let Ok((gun, gt)) = guns.get(g) else {
            continue;
        };
        if !gun.firearm.bolt_stuck {
            continue;
        }
        let up = hand.velocity.dot(gt.rotation() * Vec3::Y);
        if up > 0.1 && last.get(&e).is_none_or(|t| now - t > 0.15) {
            last.insert(e, now);
            haptics.write(Haptic {
                side: rig.side,
                amplitude: 0.3,
                seconds: 0.03,
            });
        }
    }
}

/// The tip of a cleaning rod, pushed into a muzzle and down the bore,
/// knocks a squib's bullet out.
fn rod_bores(
    hands: Query<&Hand>,
    rods: Query<(&Item, &GlobalTransform)>,
    mut guns: Query<(Entity, &mut Gun, &GlobalTransform, &Item)>,
    catalog: Res<Catalog>,
    mut out: MessageWriter<GunEvent>,
    mut ejected: MessageWriter<super::Ejection>,
) {
    for hand in &hands {
        let Some(rod) = hand.holding else {
            continue;
        };
        let Ok((it, rgt)) = rods.get(rod) else {
            continue;
        };
        let def = catalog.get(it.kind());
        if def.id != "cleaning_rod" {
            continue;
        }
        let half = def.size_m()[2] * 0.5;
        // Either end will do.
        let tips = [
            rgt.transform_point(Vec3::Z * -half),
            rgt.transform_point(Vec3::Z * half),
        ];
        for (e, mut gun, gt, gun_item) in &mut guns {
            if gun.firearm.squib.is_none() {
                continue;
            }
            let muzzle = gt.transform_point(gun.layout.muzzle);
            let out_of_bore = gt.rotation() * Vec3::NEG_Z;
            let rodded = tips.iter().any(|t| {
                let rel = *t - muzzle;
                let depth = -rel.dot(out_of_bore);
                let off = (rel + out_of_bore * depth).length();
                depth > ROD_DEPTH && off < ROD_RADIUS
            });
            if rodded {
                let ev = gun.firearm.rod_through();
                let name = catalog.get(gun_item.kind()).id.clone();
                super::emit(e, &name, &mut gun, gt, ev, &mut out, &mut ejected);
            }
        }
    }
}
