//! Mirrors: a fold in space you can barely see, a shimmer in the air at
//! waist height. Whatever crosses it is blinked out the far side, a few
//! metres past its edge, still moving: a bolt thrown in winks out and
//! lands beyond it, and so do you. It needs a moment to fold back up, and
//! the far side may be somewhere you'd rather not be (in another anomaly,
//! say, or in front of a robot).
//!
//! A mirror grows a **mirror shard**, which hangs in the fold unmoved
//! (it is of the mirror): throw something in, and reach in for it while
//! the fold is still closing. Held and used, it steps you a few metres
//! ahead, once a minute.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, MIRROR_RECHARGE, mirror_exit};

use super::artifacts::Shielded;
use super::{Anomaly, AnomalyAct, AnomalyEvent};
use crate::items::usage::UseItem;
use crate::items::{Catalog, Item};
use crate::player::{Body, Player};
use crate::robots::Noise;
use crate::world::{Layer, Zone};

/// Height of a mirror's fold above the ground.
pub const MIRROR_HEIGHT: f32 = 0.9;
/// The heaviest thing a mirror can blink, kg (not a robot).
pub const MIRROR_MAX_KG: f32 = 120.0;
/// How far a shard steps you, metres, and how often.
pub const SHARD_STEP: f32 = 4.0;
pub const SHARD_RECHARGE: f32 = 60.0;

/// Something was blinked from `from` to `to` (for visuals and sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct Blinked {
    pub entity: Entity,
    pub from: Vec3,
    pub to: Vec3,
}

/// A shard's time to recharge.
#[derive(Component, Default)]
pub struct ShardCharge(pub f32);

/// Somewhere a mirror can set something down: open ground in the valley,
/// and not inside another anomaly's heart.
fn landing_ok(zone: &Zone, others: &[(Vec2, f32)], p: [f32; 2]) -> bool {
    extraction_core::anomaly::placeable(&zone.map, &zone.hm, p)
        && others
            .iter()
            .all(|(c, r)| c.distance(Vec2::from_array(p)) > r * 0.5)
}

#[allow(clippy::type_complexity)]
pub fn mirrors(
    mut anomalies: Query<(
        Entity,
        &mut Anomaly,
        &GlobalTransform,
        Has<super::surge::Forming>,
    )>,
    mut bodies: Query<
        (
            Entity,
            &GlobalTransform,
            &mut Transform,
            &RigidBody,
            &ComputedMass,
            Option<&Item>,
        ),
        // Nothing carried blinks on its own: it goes with whoever has it.
        (
            Without<RigidBodyDisabled>,
            crate::interact::NotCarried,
            Without<Player>,
        ),
    >,
    parts: Query<(), With<crate::robots::RobotPart>>,
    mut players: Query<(Entity, &GlobalTransform, &mut Transform), With<Player>>,
    spatial: SpatialQuery,
    zone: Res<Zone>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut events: MessageWriter<AnomalyEvent>,
    mut blinks: MessageWriter<Blinked>,
    mut noises: MessageWriter<Noise>,
) {
    let dt = time.delta_secs();
    let all: Vec<(Vec2, f32)> = anomalies
        .iter()
        .map(|(_, a, g, _)| (g.translation().xz(), a.radius))
        .collect();
    let underground = |p: Vec3| crate::world::underground::floor_under(&zone, p).is_some();
    for (e, mut a, gt, forming) in &mut anomalies {
        if a.kind != AnomalyKind::Mirror || forming {
            continue;
        }
        a.charge = (a.charge + dt / MIRROR_RECHARGE).min(1.0);
        if a.charge < 1.0 {
            continue;
        }
        let base = gt.translation();
        let fold = base + Vec3::Y * MIRROR_HEIGHT;
        let others: Vec<(Vec2, f32)> = all
            .iter()
            .copied()
            .filter(|(c, _)| c.distance(base.xz()) > 0.01)
            .collect();
        // Underground a fold sends you along the floor; the valley's
        // ground rules don't apply down there.
        let below = underground(base);
        let ok = |p: [f32; 2]| below || landing_ok(&zone, &others, p);
        let ground = |p: [f32; 2]| {
            if below {
                base.y
            } else {
                zone.ground(p[0], p[1])
            }
        };
        let mut fired = None;
        for hit in spatial.shape_intersections(
            &Collider::sphere(a.radius),
            fold,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask([Layer::Item, Layer::Robot]),
        ) {
            if parts.contains(hit) {
                continue;
            }
            let Ok((be, bt, mut tf, rb, mass, item)) = bodies.get_mut(hit) else {
                continue;
            };
            let artifact = item.is_some_and(|i| {
                catalog.get(i.kind()).category == extraction_core::items::Category::Artifact
            });
            if *rb != RigidBody::Dynamic || mass.value() > MIRROR_MAX_KG || artifact {
                continue;
            }
            let p = bt.translation();
            let Some(exit) = mirror_exit(base.xz().to_array(), p.xz().to_array(), a.radius, &ok)
            else {
                continue;
            };
            let height = (p.y - base.y).max(0.3);
            let to = Vec3::new(exit[0], ground(exit) + height, exit[1]);
            // Physics picks the move up from the transform.
            tf.translation = to;
            blinks.write(Blinked {
                entity: be,
                from: p,
                to,
            });
            fired = Some(p);
            break;
        }
        if fired.is_none() {
            for (pe, pt, mut root) in &mut players {
                let feet = pt.translation();
                let chest = feet + Vec3::Y * 1.0;
                if chest.distance(fold) > a.radius + 0.2 {
                    continue;
                }
                let Some(exit) =
                    mirror_exit(base.xz().to_array(), feet.xz().to_array(), a.radius, &ok)
                else {
                    continue;
                };
                let to = Vec3::new(exit[0], ground(exit), exit[1]);
                root.translation += to - feet;
                blinks.write(Blinked {
                    entity: pe,
                    from: feet,
                    to,
                });
                fired = Some(chest);
                break;
            }
        }
        if let Some(at) = fired {
            a.charge = 0.0;
            events.write(AnomalyEvent {
                anomaly: e,
                kind: a.kind,
                at,
                what: AnomalyAct::Blink,
            });
            noises.write(Noise {
                at,
                radius: 25.0,
                source: None,
            });
        }
    }
}

/// A held mirror shard, used: a step through, a few metres the way you
/// face, onto open ground.
#[allow(clippy::type_complexity)]
pub fn shards(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    mut stones: Query<(&Item, Option<&mut ShardCharge>, Has<Shielded>)>,
    mut players: Query<(&GlobalTransform, &mut Transform, &mut Body), With<Player>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    catalog: Res<Catalog>,
    zone: Res<Zone>,
    time: Res<Time>,
    spatial: SpatialQuery,
    mut blinks: MessageWriter<Blinked>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    let dt = time.delta_secs();
    for (_, c, _) in &mut stones {
        if let Some(mut c) = c {
            c.0 = (c.0 - dt).max(0.0);
        }
    }
    for u in uses.read() {
        let Ok((item, charge, sealed)) = stones.get_mut(u.item) else {
            continue;
        };
        if catalog.get(item.kind()).id != "art_shard" || sealed {
            continue;
        }
        if let Some(c) = &charge
            && c.0 > 0.0
        {
            raid.say("The shard is dull. Not yet.");
            continue;
        }
        let Ok((pt, mut root, mut body)) = players.get_mut(u.player) else {
            continue;
        };
        let feet = pt.translation();
        let facing = heads
            .iter()
            .find(|(_, p)| p.parent() == u.player)
            .map(|(g, _)| g.forward().as_vec3())
            .unwrap_or(pt.forward().as_vec3());
        let flat = Vec3::new(facing.x, 0.0, facing.z).normalize_or(Vec3::NEG_Z);
        // As far as it'll go, up to a step: short of a wall or water.
        let clear = spatial
            .cast_ray(
                feet + Vec3::Y * 1.0,
                Dir3::new(flat).unwrap_or(Dir3::NEG_Z),
                SHARD_STEP + 0.5,
                true,
                &SpatialQueryFilter::from_mask([Layer::World, Layer::Robot]),
            )
            .map_or(SHARD_STEP, |h| (h.distance - 0.5).max(0.0));
        let Some(to) = (1..=8).rev().find_map(|k| {
            let d = SHARD_STEP * k as f32 / 8.0;
            if d > clear {
                return None;
            }
            let p = feet + flat * d;
            extraction_core::anomaly::placeable(&zone.map, &zone.hm, [p.x, p.z])
                .then(|| Vec3::new(p.x, zone.ground(p.x, p.z), p.z))
        }) else {
            continue;
        };
        root.translation += to - feet;
        body.velocity = Vec3::ZERO;
        match charge {
            Some(mut c) => c.0 = SHARD_RECHARGE,
            None => {
                commands.entity(u.item).insert(ShardCharge(SHARD_RECHARGE));
            }
        }
        blinks.write(Blinked {
            entity: u.player,
            from: feet,
            to,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Side;
    use crate::vr_harness::Vr;

    /// Held in the right hand, the shard's use button steps you ahead the
    /// way you face; a second press straight after does nothing.
    #[test]
    fn a_mirror_shard_steps_you_ahead_once_a_minute() {
        let mut vr = Vr::new("marrow_creek");
        let (feet, yaw) = {
            let zone = vr.app.world().resource::<Zone>();
            // Open ground with open ground 4 m north of it.
            let p = (0..400)
                .map(|i| [60.0 + (i % 20) as f32 * 7.0, 120.0 - (i / 20) as f32 * 7.0])
                .find(|p| {
                    (0..=8).all(|k| {
                        extraction_core::anomaly::placeable(
                            &zone.map,
                            &zone.hm,
                            [p[0], p[1] - k as f32 * 0.6],
                        )
                    })
                })
                .expect("somewhere open");
            (zone.on_ground(p[0], p[1]), 0.0)
        };
        vr.stand(feet, yaw);
        vr.head(Vec3::new(0.0, 1.65, 0.0));
        vr.step(10);
        let hand = feet + Vec3::new(0.25, 1.1, -0.35);
        let shard = vr.spawn("art_shard", Transform::from_translation(hand));
        vr.app
            .world_mut()
            .entity_mut(shard)
            .insert(RigidBody::Static);
        vr.hand(Side::Right, hand, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(4);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(shard), "held");
        let before = vr
            .app
            .world()
            .get::<Transform>(vr.player)
            .unwrap()
            .translation;
        vr.primary(Side::Right, true);
        vr.step(3);
        vr.primary(Side::Right, false);
        vr.step(3);
        let after = vr
            .app
            .world()
            .get::<Transform>(vr.player)
            .unwrap()
            .translation;
        let moved = after - before;
        assert!(
            moved.z < -SHARD_STEP * 0.7 && moved.x.abs() < 0.5,
            "stepped about {SHARD_STEP} m north (-Z): {moved:.2}"
        );
        vr.primary(Side::Right, true);
        vr.step(3);
        vr.primary(Side::Right, false);
        vr.step(3);
        let again = vr
            .app
            .world()
            .get::<Transform>(vr.player)
            .unwrap()
            .translation;
        assert!(
            again.distance(after) < 0.5,
            "not again so soon ({:.2} m)",
            again.distance(after)
        );
    }
}
