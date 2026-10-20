//! Finding anomalies before they find you.
//!
//! - **Throw something in.** A bolt, a nut, a tin: the moment a loose
//!   thing first crosses into an anomaly, it shows itself (a flare of its
//!   light and a burst of its particles for a few seconds, see
//!   `visuals`), whatever kind it is. A snare discharges into it, a lift
//!   floats it, a well drags it in, a mirror blinks it away.
//! - **The detector.** Carry one and it clicks, faster the nearer the
//!   closest anomaly's edge: a lazy tick at 25 m, a rattle at the edge.
//!   It clicks for the nearest one only, so two close together sound like
//!   one; sweep it about (held, it listens the way it points).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::{DETECTOR_RANGE, detector_interval};

use super::artifacts::CarriedBy;
use super::{Anomaly, AnomalyAct, AnomalyEvent};
use crate::interact::Held;
use crate::items::{Catalog, Item};
use crate::player::Player;
use crate::world::Layer;

/// Seconds an anomaly stays shown after something is thrown into it.
pub const REVEAL_TIME: f32 = 6.0;

/// What's crossed into an anomaly lately, and how long it stays shown.
#[derive(Component, Debug, Default, Clone)]
pub struct Touched {
    pub inside: Vec<Entity>,
    /// Seconds left showing itself (0: back to lurking).
    pub revealed: f32,
}

/// One click of a detector (for sound and its lamp).
#[derive(Message, Clone, Copy, Debug)]
pub struct DetectorClick {
    pub detector: Entity,
    pub at: Vec3,
    /// In a hand (else on the body somewhere).
    pub held: bool,
    /// 0 at the edge of its range, 1 at an anomaly's edge.
    pub near: f32,
}

/// A detector's count to its next click.
#[derive(Component, Debug, Default)]
pub struct Ticking {
    pub next: f32,
}

/// Loose things crossing into anomalies show them up.
pub fn reveal(
    mut anomalies: Query<(Entity, &Anomaly, &GlobalTransform, &mut Touched)>,
    loose: Query<&RigidBody, (With<Item>, Without<Held>, Without<RigidBodyDisabled>)>,
    players: Query<&GlobalTransform, With<Player>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut events: MessageWriter<AnomalyEvent>,
) {
    let dt = time.delta_secs();
    let eyes: Vec<Vec3> = players.iter().map(|g| g.translation()).collect();
    for (e, a, gt, mut touched) in &mut anomalies {
        touched.revealed = (touched.revealed - dt).max(0.0);
        let c = gt.translation();
        // Only where someone could see it.
        if !eyes.iter().any(|p| p.distance(c) < 90.0) {
            touched.inside.clear();
            continue;
        }
        let core = c + Vec3::Y * 1.0;
        let reach = a.radius.max(1.0);
        let now: Vec<Entity> = spatial
            .shape_intersections(
                &Collider::cylinder(reach, 3.0),
                core + Vec3::Y * 0.3,
                Quat::IDENTITY,
                &SpatialQueryFilter::from_mask(Layer::Item),
            )
            .into_iter()
            .filter(|h| loose.get(*h).is_ok_and(|rb| *rb == RigidBody::Dynamic))
            .collect();
        let new = now.iter().any(|h| !touched.inside.contains(h));
        if new {
            touched.revealed = REVEAL_TIME;
            events.write(AnomalyEvent {
                anomaly: e,
                kind: a.kind,
                at: core,
                what: AnomalyAct::Reveal,
            });
        }
        touched.inside = now;
    }
}

/// How near the nearest anomaly's edge is to `p`, metres (negative
/// inside one); `listen` (a held detector's pointing) favours the ones
/// ahead, so sweeping it about finds them.
pub fn nearest_edge(p: Vec3, listen: Option<Vec3>, anomalies: &[(Vec3, f32)]) -> Option<f32> {
    anomalies
        .iter()
        .map(|(c, r)| {
            let to = *c - p;
            let edge = Vec2::new(to.x, to.z).length() - r;
            match listen {
                // Pointed at, it hears it as it is; behind, as if half as
                // far again.
                Some(dir) if edge > 0.0 => {
                    let ahead = to.normalize_or_zero().dot(dir).clamp(-1.0, 1.0);
                    edge * (1.25 - 0.25 * ahead)
                }
                _ => edge,
            }
        })
        .filter(|d| *d < DETECTOR_RANGE)
        .min_by(|a, b| a.total_cmp(b))
}

/// Carried detectors click for the nearest anomaly.
#[allow(clippy::type_complexity)]
pub fn detectors(
    mut commands: Commands,
    mut found: Query<
        (
            Entity,
            &Item,
            &GlobalTransform,
            Has<Held>,
            Option<&mut Ticking>,
        ),
        With<CarriedBy>,
    >,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    catalog: Res<Catalog>,
    time: Res<Time>,
    mut clicks: MessageWriter<DetectorClick>,
) {
    let dt = time.delta_secs();
    let all: Vec<(Vec3, f32)> = anomalies
        .iter()
        .map(|(a, g)| (g.translation(), a.kind.reach(a.radius)))
        .collect();
    for (e, item, gt, held, ticking) in &mut found {
        if catalog.get(item.kind()).id != "detector" {
            continue;
        }
        let Some(mut t) = ticking else {
            commands.entity(e).insert(Ticking::default());
            continue;
        };
        let p = gt.translation();
        // Held, its antenna (the item's -Z) is what it listens along.
        let listen = held.then(|| gt.forward().as_vec3());
        let Some(edge) = nearest_edge(p, listen, &all) else {
            t.next = t.next.max(0.3);
            continue;
        };
        let Some(interval) = detector_interval(edge) else {
            continue;
        };
        t.next = t.next.min(interval) - dt;
        if t.next <= 0.0 {
            t.next = interval;
            clicks.write(DetectorClick {
                detector: e,
                at: p,
                held,
                near: (1.0 - edge / DETECTOR_RANGE).clamp(0.0, 1.0),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointing_at_it_hears_it_nearer() {
        let one = [(Vec3::new(0.0, 0.0, -12.0), 2.0)];
        let ahead = nearest_edge(Vec3::ZERO, Some(Vec3::NEG_Z), &one).unwrap();
        let behind = nearest_edge(Vec3::ZERO, Some(Vec3::Z), &one).unwrap();
        let worn = nearest_edge(Vec3::ZERO, None, &one).unwrap();
        assert!((ahead - 10.0).abs() < 1e-4, "{ahead}");
        assert!((worn - 10.0).abs() < 1e-4);
        assert!(behind > 14.0, "{behind}");
        assert!(nearest_edge(Vec3::ZERO, None, &[(Vec3::X * 40.0, 2.0)]).is_none());
        // Inside one, it's inside wherever it points.
        let inside = nearest_edge(Vec3::ZERO, Some(Vec3::Z), &[(Vec3::X, 2.0)]).unwrap();
        assert!(inside < 0.0);
    }
}
