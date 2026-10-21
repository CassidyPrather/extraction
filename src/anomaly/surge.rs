//! The Surge, as the anomalies feel it. Through the five minutes' warning
//! every anomaly stirs: it swells (up to a third again), charges and
//! recharges faster, burns hotter, and flares (see `visuals::pulse`). And
//! a handful of new ones bloom round whoever is still out, to the side of
//! the way they're going and never on a road: each spends its first few
//! seconds forming, lit up and harmless, so you see it open before it can
//! hurt you. Heading out along the road, you get through.

use bevy::prelude::*;
use extraction_core::anomaly::{
    AnomalyKind, SURGE_FORMING, SURGE_SWELL, bloom_ok, surge_agitation, surge_bloom_spot,
    surge_blooms_due,
};
use extraction_core::noise::Rng;

use super::detector::{REVEAL_TIME, Touched};
use super::{Anomaly, AnomalyAct, AnomalyEvent, BLOOM_LIFE, spawn_anomaly};
use crate::player::Body;
use crate::raid::{Phase, Raid, SURGE_LENGTH, SURGE_WARNING};
use crate::rig::LocalPlayer;
use crate::world::Zone;

/// How stirred up the anomalies are, 0..1 (see
/// `extraction_core::anomaly::surge_agitation`).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct Agitation(pub f32);

/// An anomaly's size before the Surge swelled it.
#[derive(Component, Debug, Clone, Copy)]
pub struct BaseRadius(pub f32);

/// A Surge bloom still opening: seconds left before it can hurt anyone.
#[derive(Component, Debug, Clone, Copy)]
pub struct Forming(pub f32);

/// One of the Surge's own blooms.
#[derive(Component, Debug, Clone, Copy)]
pub struct SurgeBloom;

pub fn remember_size(add: On<Add, Anomaly>, anomalies: Query<&Anomaly>, mut commands: Commands) {
    if let Ok(a) = anomalies.get(add.entity) {
        commands.entity(add.entity).insert(BaseRadius(a.radius));
    }
}

/// Reads the Zone clock: how stirred up everything is, and swells them.
pub fn stir(
    raid: Res<Raid>,
    mut agitation: ResMut<Agitation>,
    mut anomalies: Query<(&mut Anomaly, &BaseRadius)>,
    mut forming: Query<(Entity, &mut Forming)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let a = if matches!(raid.phase, Phase::AtBase) {
        0.0
    } else {
        surge_agitation(raid.surge_in(), SURGE_WARNING, SURGE_LENGTH)
    };
    agitation.0 = a;
    for (mut anomaly, base) in &mut anomalies {
        let want = base.0 * (1.0 + SURGE_SWELL * a);
        if (anomaly.radius - want).abs() > 1e-3 {
            anomaly.radius = want;
        }
    }
    let dt = time.delta_secs();
    for (e, mut f) in &mut forming {
        f.0 -= dt;
        if f.0 <= 0.0 {
            commands.entity(e).remove::<Forming>();
        }
    }
}

/// Opens the Surge's blooms round the local player as they fall due.
#[allow(clippy::too_many_arguments)]
pub fn surge_blooms(
    mut commands: Commands,
    mut raid: ResMut<Raid>,
    players: Query<(&GlobalTransform, &Body), With<LocalPlayer>>,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    zone: Res<Zone>,
    mut last: Local<Option<f32>>,
    mut rng: Local<Option<Rng>>,
) {
    let now = raid.surge_in();
    let before = last.replace(now).unwrap_or(now);
    let out = matches!(
        raid.phase,
        Phase::Out | Phase::TruckCalled { .. } | Phase::TruckWaiting { .. }
    );
    let due = surge_blooms_due(before, now);
    if due == 0 || !out {
        return;
    }
    let Some((gt, body)) = players.iter().next() else {
        return;
    };
    let rng = rng.get_or_insert_with(|| Rng::new(0x5_0A6E ^ raid.cycle as u64));
    let here = gt.translation();
    // Underground there's nowhere for them to open.
    if crate::world::underground::floor_under(&zone, here).is_some() {
        return;
    }
    let taken: Vec<(Vec2, f32)> = anomalies
        .iter()
        .map(|(a, g)| (g.translation().xz(), a.kind.danger_radius(a.radius)))
        .collect();
    for _ in 0..due {
        let kind = AnomalyKind::ALL[rng.below(AnomalyKind::ALL.len())];
        let (lo, hi) = kind.radius_range();
        let radius = rng.range(lo, hi);
        let danger = kind.danger_radius(radius);
        let ok = |p: [f32; 2]| {
            bloom_ok(&zone.map, &zone.hm, p, kind, radius)
                && taken
                    .iter()
                    .all(|(c, r)| c.distance(Vec2::from_array(p)) > r + danger + 1.0)
        };
        let mut roll = || rng.f32();
        let heading = [body.velocity.x, body.velocity.z];
        let Some(p) = surge_bloom_spot([here.x, here.z], heading, &mut roll, ok) else {
            continue;
        };
        let at = zone.on_ground(p[0], p[1]);
        let stone = rng.chance(extraction_core::anomaly::STORMGLASS_CHANCE);
        commands.queue(move |world: &mut World| {
            let e = spawn_anomaly(world, kind, at, radius, Some(BLOOM_LIFE));
            world.entity_mut(e).insert((
                SurgeBloom,
                Forming(SURGE_FORMING),
                Touched {
                    inside: Vec::new(),
                    revealed: REVEAL_TIME,
                },
            ));
            world.write_message(AnomalyEvent {
                anomaly: e,
                kind,
                at,
                what: AnomalyAct::Bloom,
            });
            // The only place a stormglass grows: where the Surge itself
            // broke the ground.
            if stone {
                grow(world, "art_stormglass", at + Vec3::Y * 0.4);
            }
        });
        info!("surge: a {kind:?} blooms at {at:.0}");
    }
    if before > first_bloom() && now <= first_bloom() {
        raid.say("The ground is stirring. New ones are opening around you. Keep to the road.");
    }
}

/// Spawns an artifact at `at` (a bloom's heart).
pub fn grow(world: &mut World, id: &str, at: Vec3) -> Option<Entity> {
    let catalog = world.resource::<crate::items::Catalog>().clone();
    let kind = catalog.kind(id)?;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let e = {
        let mut c = Commands::new(&mut queue, world);
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_translation(at),
                condition: 1.0,
            },
        )
    };
    queue.apply(world);
    Some(e)
}

/// When the first Surge bloom opens, seconds before the Surge.
pub fn first_bloom() -> f32 {
    extraction_core::anomaly::SURGE_BLOOMS[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::LaunchOptions;

    /// Out in the open while the Surge rages: deadly, unless a stormglass
    /// is on you.
    #[test]
    fn a_stormglass_is_a_roof_in_the_surge() {
        let hurt = |carry: bool| {
            let mut app = crate::headless_app(
                LaunchOptions {
                    robots: false,
                    ..Default::default()
                },
                false,
            );
            app.finish();
            app.cleanup();
            app.update();
            let p = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            let open = app.world().resource::<Zone>().on_ground(120.0, -60.0);
            app.world_mut().get_mut::<Transform>(p).unwrap().translation = open;
            if carry {
                let bead = grow(app.world_mut(), "art_stormglass", open).unwrap();
                app.update();
                let hand = app
                    .world_mut()
                    .query_filtered::<Entity, With<crate::rig::RigHand>>()
                    .iter(app.world())
                    .next()
                    .unwrap();
                app.world_mut().entity_mut(bead).insert((
                    crate::interact::Held {
                        player: p,
                        hand,
                        side: crate::rig::Side::Right,
                        point: 0,
                        offset: Transform::IDENTITY,
                    },
                    avian3d::prelude::RigidBodyDisabled,
                ));
                // The hand holds it too: a hold its hand doesn't know of
                // drops (`interact::drop_orphaned_holds`).
                app.world_mut()
                    .get_mut::<crate::interact::Hand>(hand)
                    .unwrap()
                    .holding = Some(bead);
            }
            {
                let mut raid = app.world_mut().resource_mut::<Raid>();
                raid.phase = Phase::Out;
                raid.clock = crate::raid::SURGE_AFTER + 10.0;
            }
            for _ in 0..3 {
                app.update();
            }
            let hp = app.world().get::<crate::health::Health>(p).unwrap().hp;
            for _ in 0..90 * 2 {
                app.update();
            }
            hp - app.world().get::<crate::health::Health>(p).unwrap().hp
        };
        let bare = hurt(false);
        let sheltered = hurt(true);
        assert!(bare > 20.0, "the open Surge tears at you ({bare:.1})");
        assert!(
            sheltered < 3.0,
            "the stormglass keeps it off ({sheltered:.1})"
        );
    }

    #[test]
    fn the_surge_swells_them_and_opens_new_ones_beside_you() {
        let mut app = crate::headless_app(
            LaunchOptions {
                robots: false,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let first = app
            .world_mut()
            .query::<(Entity, &Anomaly)>()
            .iter(app.world())
            .find(|(_, a)| a.bloom.is_none())
            .map(|(e, a)| (e, a.radius))
            .unwrap();
        // Out in the Hollow, a few seconds before the first bloom.
        let p = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let open = app.world().resource::<Zone>().on_ground(120.0, -60.0);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = open;
        {
            let mut raid = app.world_mut().resource_mut::<Raid>();
            raid.phase = Phase::Out;
            raid.clock = crate::raid::SURGE_AFTER - first_bloom() - 2.0;
        }
        let mut bloomed = None;
        for _ in 0..400 {
            app.update();
            let mut q = app
                .world_mut()
                .query_filtered::<(Entity, &GlobalTransform, Has<Forming>), With<SurgeBloom>>();
            if let Some((e, g, forming)) = q.iter(app.world()).next() {
                bloomed.get_or_insert((e, g.translation(), forming));
            }
        }
        let (_, at, forming) = bloomed.expect("a bloom opened");
        assert!(forming, "it opened forming, harmless at first");
        let d = at.xz().distance(open.xz());
        assert!(
            (9.0..=26.0).contains(&d),
            "beside you, not on you ({d:.1} m)"
        );
        // Twenty seconds out, everything has swollen by about a quarter.
        app.world_mut().resource_mut::<Raid>().clock = crate::raid::SURGE_AFTER - 20.0;
        for _ in 0..3 {
            app.update();
        }
        let now = app.world().get::<Anomaly>(first.0).unwrap().radius;
        assert!(
            now > first.1 * 1.2,
            "the others swell ({} -> {now})",
            first.1
        );
        let raid = app.world().resource::<Raid>();
        assert!(
            raid.log
                .iter()
                .any(|(l, _)| l.starts_with("The ground is stirring"))
        );
    }
}
