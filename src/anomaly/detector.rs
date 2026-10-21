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
use extraction_core::anomaly::{
    BEAR_RANGE, DETECTOR_RANGE, DetectorTier, STONE_RANGE, SWEEP_CONE_DEG, bear_blip,
    detector_interval, in_cone, stone_ping_interval,
};
use extraction_core::items::Category;

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

/// A detector's count to its next click (and a sweep's to its next
/// stone ping).
#[derive(Component, Debug, Default)]
pub struct Ticking {
    pub next: f32,
    pub stone: f32,
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

/// A sweep's ping for a loose stone in its cone (for its tone and lamp).
#[derive(Message, Clone, Copy, Debug)]
pub struct StonePing {
    pub detector: Entity,
    pub at: Vec3,
    /// 0 at the edge of its range, 1 on top of the stone.
    pub near: f32,
}

/// What a held Bear's screen shows: blips in screen space (-1..1, up is
/// the way it faces), refreshed a few times a second.
#[derive(Component, Debug, Default, Clone)]
pub struct BearScreen {
    pub stones: Vec<[f32; 2]>,
    /// Anomalies: centre and radius, screen units.
    pub anomalies: Vec<([f32; 2], f32)>,
    pub refresh: f32,
}

/// Yaw of a horizontal facing (0 = -Z), as `bear_blip` takes it.
fn yaw_of(fwd: Vec3) -> f32 {
    (-fwd.x).atan2(-fwd.z)
}

/// Carried detectors, by their tier: the clicker ticks for the nearest
/// anomaly; a held sweep hears only what it points at, and pings for a
/// loose stone in its cone; a held Bear shows the stones and anomalies
/// round you on its screen.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn detectors(
    mut commands: Commands,
    mut found: Query<
        (
            Entity,
            &Item,
            &GlobalTransform,
            Has<Held>,
            Option<&mut Ticking>,
            Option<&mut BearScreen>,
        ),
        With<CarriedBy>,
    >,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    stones: Query<(&Item, &GlobalTransform), (Without<Held>, Without<CarriedBy>)>,
    catalog: Res<Catalog>,
    hushes: Res<super::hush::Hushes>,
    time: Res<Time>,
    mut clicks: MessageWriter<DetectorClick>,
    mut pings: MessageWriter<StonePing>,
) {
    let dt = time.delta_secs();
    let all: Vec<(Vec3, f32)> = anomalies
        .iter()
        .map(|(a, g)| (g.translation(), a.kind.reach(a.radius)))
        .collect();
    let loose: Vec<Vec3> = stones
        .iter()
        .filter(|(i, _)| catalog.get(i.kind()).category == Category::Artifact)
        .map(|(_, g)| g.translation())
        .collect();
    for (e, item, gt, held, ticking, screen) in &mut found {
        let Some(tier) = DetectorTier::of(&catalog.get(item.kind()).id) else {
            continue;
        };
        let Some(mut t) = ticking else {
            commands.entity(e).insert(Ticking::default());
            continue;
        };
        let p = gt.translation();
        // In a Hush (or on a hushstone's carrier) it's mute.
        if hushes.eats(p) {
            t.next = t.next.max(0.3);
            continue;
        }
        // Held, its antenna (the item's -Z) is what it listens along.
        let fwd = gt.forward().as_vec3();
        let listen = held.then_some(fwd);
        // A held sweep hears only what's in its cone.
        let heard: Vec<(Vec3, f32)> = if tier == DetectorTier::Sweep && held {
            all.iter()
                .copied()
                .filter(|(c, r)| {
                    (*c - p).xz().length() < *r
                        || in_cone(fwd.to_array(), (*c - p).to_array(), SWEEP_CONE_DEG)
                })
                .collect()
        } else {
            all.clone()
        };
        match nearest_edge(p, listen, &heard)
            .and_then(|edge| detector_interval(edge).map(|i| (edge, i)))
        {
            Some((edge, interval)) => {
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
            None => t.next = t.next.max(0.3),
        }
        // The sweep's second voice: a stone in its cone.
        if tier == DetectorTier::Sweep && held {
            let stone = loose
                .iter()
                .filter(|s| in_cone(fwd.to_array(), (**s - p).to_array(), SWEEP_CONE_DEG))
                .map(|s| s.distance(p))
                .min_by(|a, b| a.total_cmp(b));
            match stone.and_then(|d| stone_ping_interval(d).map(|i| (d, i))) {
                Some((d, interval)) => {
                    t.stone = t.stone.min(interval) - dt;
                    if t.stone <= 0.0 {
                        t.stone = interval;
                        pings.write(StonePing {
                            detector: e,
                            at: p,
                            near: (1.0 - d / STONE_RANGE).clamp(0.0, 1.0),
                        });
                    }
                }
                None => t.stone = t.stone.max(0.3),
            }
        }
        // The Bear's screen, held.
        if tier == DetectorTier::Bear {
            let Some(mut screen) = screen else {
                commands.entity(e).insert(BearScreen::default());
                continue;
            };
            screen.refresh -= dt;
            if screen.refresh > 0.0 {
                continue;
            }
            screen.refresh = 0.25;
            if !held {
                screen.stones.clear();
                screen.anomalies.clear();
                continue;
            }
            let yaw = yaw_of(fwd);
            screen.stones = loose
                .iter()
                .filter_map(|s| bear_blip((*s - p).xz().to_array(), yaw, STONE_RANGE))
                .collect();
            screen.anomalies = all
                .iter()
                .filter_map(|(c, r)| {
                    bear_blip((*c - p).xz().to_array(), yaw, BEAR_RANGE)
                        .map(|b| (b, r / BEAR_RANGE))
                })
                .collect();
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

    /// The headset path: a detector in the right hand, then the hand
    /// turned until the detector's antenna points `dir`.
    mod held {
        use super::super::*;
        use crate::rig::Side;
        use crate::vr_harness::Vr;

        fn hold(vr: &mut Vr, id: &str, feet: Vec3) -> (Entity, Vec3) {
            let hand = feet + Vec3::new(0.25, 1.2, -0.3);
            let catalog = vr.app.world().resource::<Catalog>().clone();
            let kind = catalog.kind(id).unwrap();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, vr.app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::from_translation(hand),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(vr.app.world_mut());
            vr.app.world_mut().entity_mut(e).insert(RigidBody::Static);
            vr.step(2);
            vr.hand(Side::Right, hand, Quat::IDENTITY, 0.0, 0.0);
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(4);
            assert_eq!(
                vr.hand_state(Side::Right).holding,
                Some(e),
                "holding the {id}"
            );
            (e, hand)
        }

        fn point(vr: &mut Vr, item: Entity, hand: Vec3, dir: Vec3) {
            let mut rot = Quat::IDENTITY;
            for _ in 0..4 {
                vr.hand(Side::Right, hand, rot, 1.0, 0.0);
                vr.step(4);
                let fwd = vr
                    .app
                    .world()
                    .get::<GlobalTransform>(item)
                    .unwrap()
                    .forward()
                    .as_vec3();
                rot = Quat::from_rotation_arc(fwd, dir.normalize()) * rot;
            }
            vr.hand(Side::Right, hand, rot, 1.0, 0.0);
            vr.step(4);
        }

        fn count<M: Message>(vr: &mut Vr, frames: usize, pick: impl Fn(&M) -> bool) -> usize {
            let mut cursor = vr
                .app
                .world()
                .resource::<Messages<M>>()
                .get_cursor_current();
            let mut n = 0;
            for _ in 0..frames {
                vr.step(1);
                n += cursor
                    .read(vr.app.world().resource::<Messages<M>>())
                    .filter(|m| pick(m))
                    .count();
            }
            n
        }

        /// A direction from `from` whose cone holds no anomaly in range
        /// and no loose stone.
        fn empty_way(vr: &mut Vr, from: Vec3) -> Vec3 {
            let catalog = vr.app.world().resource::<Catalog>().clone();
            let mut things: Vec<Vec3> = vr
                .app
                .world_mut()
                .query::<(&Anomaly, &GlobalTransform)>()
                .iter(vr.app.world())
                .map(|(_, g)| g.translation())
                .filter(|p| p.distance(from) < DETECTOR_RANGE + 10.0)
                .collect();
            things.extend(
                vr.app
                    .world_mut()
                    .query_filtered::<(&Item, &GlobalTransform), Without<Held>>()
                    .iter(vr.app.world())
                    .filter(|(i, _)| catalog.get(i.kind()).category == Category::Artifact)
                    .map(|(_, g)| g.translation()),
            );
            (0..16)
                .map(|k| {
                    let a = k as f32 / 16.0 * std::f32::consts::TAU;
                    Vec3::new(a.sin(), 0.0, -a.cos())
                })
                .find(|d| {
                    things.iter().all(|t| {
                        !in_cone(d.to_array(), (*t - from).to_array(), SWEEP_CONE_DEG + 10.0)
                    })
                })
                .expect("somewhere quiet to point")
        }

        fn snare(vr: &mut Vr) -> (Vec3, f32) {
            let mut q = vr.app.world_mut().query::<(&Anomaly, &GlobalTransform)>();
            q.iter(vr.app.world())
                .find(|(a, _)| a.kind == extraction_core::anomaly::AnomalyKind::Snare)
                .map(|(a, g)| (g.translation(), a.radius))
                .unwrap()
        }

        #[test]
        fn a_held_sweep_hears_what_it_points_at_and_pings_for_stones() {
            let mut vr = Vr::new("marrow_creek");
            let (c, r) = snare(&mut vr);
            // Ten metres south of the snare's edge.
            let feet = vr
                .app
                .world()
                .resource::<crate::world::Zone>()
                .on_ground(c.x, c.z + r + 10.0);
            vr.stand(feet, 0.0);
            vr.step(10);
            let (det, hand) = hold(&mut vr, "detector_sweep", feet);
            // Pointed somewhere quiet, it hears nothing of the snare.
            let quiet = empty_way(&mut vr, hand);
            point(&mut vr, det, hand, quiet);
            let away = count::<DetectorClick>(&mut vr, 90 * 3, |m| m.detector == det);
            point(&mut vr, det, hand, (c - hand).with_y(0.0));
            let at = count::<DetectorClick>(&mut vr, 90 * 3, |m| m.detector == det);
            assert!(at >= 3 && away == 0, "pointed at it {at}, away {away}");
            // A loose stone 8 m east: a ping only when it points there.
            let stone = vr
                .app
                .world()
                .resource::<crate::world::Zone>()
                .on_ground(feet.x + 8.0, feet.z)
                + Vec3::Y * 0.2;
            crate::anomaly::surge::grow(vr.app.world_mut(), "art_hearth", stone);
            let quiet = empty_way(&mut vr, hand);
            point(&mut vr, det, hand, quiet);
            let blind = count::<StonePing>(&mut vr, 90 * 2, |m| m.detector == det);
            point(&mut vr, det, hand, (stone - hand).with_y(0.0));
            let pings = count::<StonePing>(&mut vr, 90 * 2, |m| m.detector == det);
            assert!(
                pings >= 2 && blind == 0,
                "pointed at the stone {pings}, away {blind}"
            );
        }

        #[test]
        fn a_held_bear_shows_the_stone_the_way_you_face() {
            let mut vr = Vr::new("marrow_creek");
            let (c, r) = snare(&mut vr);
            let feet = vr
                .app
                .world()
                .resource::<crate::world::Zone>()
                .on_ground(c.x, c.z + r + 12.0);
            vr.stand(feet, 0.0);
            vr.step(10);
            let stone = feet + Vec3::new(8.0, 0.2, 0.0);
            crate::anomaly::surge::grow(vr.app.world_mut(), "art_aegis", stone);
            let (det, hand) = hold(&mut vr, "detector_bear", feet);
            // Facing north (-Z): the stone east shows right of centre, and
            // the snare north up the screen.
            point(&mut vr, det, hand, Vec3::NEG_Z);
            vr.step(60);
            let screen = vr.app.world().get::<BearScreen>(det).unwrap().clone();
            assert!(
                screen
                    .stones
                    .iter()
                    .any(|s| s[0] > 0.25 && s[1].abs() < 0.2),
                "the stone shows to the right: {:?}",
                screen.stones
            );
            assert!(
                screen
                    .anomalies
                    .iter()
                    .any(|(a, _)| a[1] > 0.3 && a[0].abs() < 0.25),
                "the snare shows ahead: {:?}",
                screen.anomalies
            );
        }
    }
}
