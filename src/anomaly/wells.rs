//! Wells: a knot of weight hanging in the air. Everything loose within
//! its reach is dragged toward its heart and circles there in a tightening
//! ball of junk; whoever walks in is dragged too, gently at the edge and
//! hard near the heart, which crushes. Bullets passing close bend (slow
//! ones most), so a robot behind a well is hard to hit and a shot across
//! one goes wide.
//!
//! A well grows a **lodestone**, which hangs at its heart unmoved: dash
//! in for it and out again before the heart has you. Carried, it shoves
//! rounds fired at you a little aside, as a small well of your own.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::{
    AnomalyKind, WELL_CRUSH, WELL_DRAG, WELL_HEART, WELL_HEIGHT, WELL_PULL, well_impulse,
    well_in_reach, well_pull,
};

use super::artifacts::{CarriedBy, Shielded};
use super::{Anomaly, AnomalyAct, AnomalyEvent};
use crate::health::Health;
use crate::interact::Held;
use crate::items::{Catalog, Item};
use crate::player::{Body, Player};
use crate::weapons::ballistics::{Hit, Projectile};
use crate::world::Layer;

/// A lodestone's own little well round its carrier: radius of the lens it
/// works like, and how far from the carrier's chest it reaches.
pub const LODESTONE_RADIUS: f32 = 0.6;
/// A lodestone's lens, as a share of a well's: enough to turn a graze
/// into a miss, not to wrap a round round you.
pub const LODESTONE_STRENGTH: f32 = 0.15;

/// A player's chest crossed into a well's reach (for a first-time hint:
/// sent each time they go in, not while they stay).
#[derive(Message, Clone, Copy, Debug)]
pub struct EnteredWell {
    pub player: Entity,
    pub well: Entity,
}

/// Where a well's heart hangs, from its ground position.
pub fn heart(at: Vec3) -> Vec3 {
    at + Vec3::Y * WELL_HEIGHT
}

/// Wells pull loose bodies and walkers in, and crush what reaches the
/// heart.
#[allow(clippy::type_complexity)]
pub fn wells(
    mut anomalies: Query<(Entity, &mut Anomaly, &GlobalTransform), Without<super::surge::Forming>>,
    mut bodies: Query<
        (
            &GlobalTransform,
            &RigidBody,
            &ComputedMass,
            Forces,
            Has<Held>,
            Option<&Item>,
        ),
        Without<RigidBodyDisabled>,
    >,
    mut players: Query<(Entity, &GlobalTransform, &mut Body, &mut Health), With<Player>>,
    parts: Query<(), With<crate::robots::RobotPart>>,
    catalog: Res<Catalog>,
    mut entered: MessageWriter<EnteredWell>,
    mut inside: Local<Vec<(Entity, Entity)>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut events: MessageWriter<AnomalyEvent>,
) {
    let dt = time.delta_secs();
    let mut now_inside = Vec::new();
    for (e, mut a, gt) in &mut anomalies {
        if a.kind != AnomalyKind::Well {
            continue;
        }
        let core = heart(gt.translation());
        let reach = a.kind.reach(a.radius);
        let mut crushing = false;
        for hit in spatial.shape_intersections(
            &Collider::sphere(reach),
            core,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask([Layer::Item, Layer::Robot]),
        ) {
            if parts.contains(hit) {
                // Robots are too heavy to drag; the heart still crushes them.
                if let Ok((bt, ..)) = bodies.get(hit)
                    && bt.translation().distance(core) < a.radius * WELL_HEART + 0.4
                {
                    crushing = true;
                    hits.write(Hit {
                        entity: hit,
                        point: core,
                        normal: Vec3::Y,
                        velocity: Vec3::ZERO,
                        damage: 20.0 * dt,
                        shooter: None,
                        mass_g: 0.0,
                        load: Default::default(),
                    });
                }
                continue;
            }
            let Ok((bt, rb, mass, mut forces, held, item)) = bodies.get_mut(hit) else {
                continue;
            };
            // Its lodestone hangs at the heart, unmoved: reach in for it.
            let artifact = item.is_some_and(|i| {
                catalog.get(i.kind()).category == extraction_core::items::Category::Artifact
            });
            if *rb != RigidBody::Dynamic || held || artifact {
                continue;
            }
            let rel = bt.translation() - core;
            let m = mass.value().max(0.05);
            let pull = Vec3::from_array(well_pull(rel.to_array(), a.radius, WELL_PULL));
            // Hold it up against gravity as it falls in, and damp it near
            // the heart so things settle into a circling knot instead of
            // slingshotting through.
            let k = extraction_core::anomaly::well_falloff(rel.length(), a.radius);
            let v = forces.linear_velocity();
            let swirl = Vec3::new(-rel.z, 0.0, rel.x).normalize_or_zero() * 4.0 * k;
            let damp = -v * 2.5 * k * k;
            forces.apply_force((pull + Vec3::Y * 9.81 * k.sqrt() + swirl + damp) * m);
        }
        for (pe, pt, mut body, mut health) in &mut players {
            let chest = pt.translation() + Vec3::Y * 1.0;
            let rel = chest - core;
            let flat = Vec3::new(rel.x, 0.0, rel.z);
            if flat.length() > reach || rel.y.abs() > 3.0 {
                continue;
            }
            now_inside.push((pe, e));
            if !inside.contains(&(pe, e)) {
                entered.write(EnteredWell {
                    player: pe,
                    well: e,
                });
            }
            let pull = Vec3::from_array(well_pull(flat.to_array(), a.radius, WELL_DRAG));
            body.velocity += pull * dt;
            if flat.length() < a.radius * WELL_HEART && !health.down {
                crushing = true;
                health.hp -= WELL_CRUSH * dt;
                health.since_hit = 0.0;
            }
        }
        // `charge` marks crushing, so the event fires as it starts.
        if crushing && a.charge < 0.5 {
            events.write(AnomalyEvent {
                anomaly: e,
                kind: a.kind,
                at: core,
                what: AnomalyAct::Crush,
            });
        }
        a.charge = if crushing { 1.0 } else { 0.0 };
    }
    *inside = now_inside;
}

/// Bends rounds in flight: every well's lens over the tick each round is
/// about to fly, and a carried lodestone's push away from its carrier.
#[allow(clippy::type_complexity)]
pub fn bend_rounds(
    mut rounds: Query<(&mut Projectile, &Transform)>,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    stones: Query<(&Item, &CarriedBy), Without<Shielded>>,
    players: Query<&GlobalTransform, With<Player>>,
    catalog: Res<Catalog>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let wells: Vec<(Vec3, f32)> = anomalies
        .iter()
        .filter(|(a, _)| a.kind == AnomalyKind::Well)
        .map(|(a, g)| (heart(g.translation()), a.radius))
        .collect();
    let carriers: Vec<(Entity, Vec3)> = stones
        .iter()
        .filter(|(i, _)| catalog.get(i.kind()).id == "art_lodestone")
        .filter_map(|(_, c)| {
            players
                .get(c.0)
                .ok()
                .map(|g| (c.0, g.translation() + Vec3::Y * 1.2))
        })
        .collect();
    if wells.is_empty() && carriers.is_empty() {
        return;
    }
    for (mut p, tf) in &mut rounds {
        let from = tf.translation;
        let step = p.velocity * dt;
        let mut dv = Vec3::ZERO;
        for (c, r) in &wells {
            let rel = (from - *c).to_array();
            if well_in_reach(rel, step.to_array(), *r) {
                dv += Vec3::from_array(well_impulse(rel, p.velocity.to_array(), dt, *r));
            }
        }
        // A lodestone is a well turned inside out: it pushes.
        for (who, chest) in &carriers {
            if p.shooter == Some(*who) {
                continue;
            }
            let rel = (from - *chest).to_array();
            if well_in_reach(rel, step.to_array(), LODESTONE_RADIUS) {
                dv -= Vec3::from_array(well_impulse(
                    rel,
                    p.velocity.to_array(),
                    dt,
                    LODESTONE_RADIUS,
                )) * LODESTONE_STRENGTH;
            }
        }
        if dv != Vec3::ZERO {
            // Keep the round's speed: the lens turns it, it doesn't slow it.
            let speed = p.velocity.length();
            p.velocity = (p.velocity + dv).normalize_or(p.velocity) * speed;
        }
    }
}
