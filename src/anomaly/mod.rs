//! Anomalies, artifacts and attention.
//!
//! - **Anomalies** sit where the Surge left them ([`extraction_core::anomaly`]
//!   lays them out per cycle). A *lift* floats and swirls anything loose and
//!   flings it when it pops; a *burner* cooks whoever stands in it; a
//!   *snare* discharges into whatever touches it, robots included. Throw
//!   something in first.
//! - **Artifacts** grow at their hearts. Carried, they do odd, useful
//!   things (see [`artifacts`]) — and they draw **attention**.
//! - **Attention** (carried artifacts, plus recent gunfire) is what the
//!   director spends: response teams sent to where you *are*, and new
//!   anomalies blooming across your path.

pub mod artifacts;
pub mod offering;
pub mod visuals;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, layout};
use extraction_core::noise::Rng;

use crate::health::Health;
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::player::{Body, Player};
use crate::raid::Raid;
use crate::robots::{JobKind, Noise};
use crate::weapons::ballistics::Hit;
use crate::world::{Layer, Zone};

#[derive(Component, Debug, Clone)]
pub struct Anomaly {
    pub kind: AnomalyKind,
    pub radius: f32,
    /// Lift: builds while things are in it, pops at 1. Snare: 1 = ready.
    pub charge: f32,
    /// Seconds left for a bloom (director-made, temporary); None for the
    /// Surge-laid ones.
    pub bloom: Option<f32>,
}

/// Something an anomaly did, for visuals and sound.
#[derive(Message, Clone, Copy, Debug)]
pub struct AnomalyEvent {
    pub anomaly: Entity,
    pub kind: AnomalyKind,
    pub at: Vec3,
    pub what: AnomalyAct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnomalyAct {
    /// A lift threw everything in it.
    Pop,
    /// A snare discharged.
    Discharge,
    /// Something wandered into a burner.
    Flare,
    /// A bloom opened up.
    Bloom,
}

/// How much the Zone has noticed a player. Per second; the director turns
/// it into budget.
#[derive(Component, Debug, Default, Clone)]
pub struct Attention {
    /// From carried artifacts.
    pub carried: f32,
    /// From recent noise, decaying.
    pub noise: f32,
}

impl Attention {
    pub fn total(&self) -> f32 {
        self.carried + self.noise
    }
}

/// The director's attention account.
#[derive(Resource, Debug, Default)]
pub struct AttentionBudget {
    pub spent_on_responses: f32,
    pub budget: f32,
    pub next_bloom: f32,
    pub warned: bool,
}

/// Attention-seconds per response team.
pub const RESPONSE_COST: f32 = 90.0;
/// Attention-seconds per bloom.
pub const BLOOM_COST: f32 = 60.0;
pub const MAX_BLOOMS: usize = 3;
pub const BLOOM_LIFE: f32 = 240.0;

pub struct AnomalyPlugin;

impl Plugin for AnomalyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AnomalyEvent>()
            .init_resource::<AttentionBudget>()
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(Attention::default());
            })
            .add_plugins((artifacts::ArtifactPlugin, offering::OfferingPlugin))
            .add_systems(Startup, spawn_cycle_anomalies)
            .add_systems(
                FixedUpdate,
                (
                    lifts,
                    burners,
                    snares,
                    measure_attention,
                    spend_attention,
                    age_blooms,
                )
                    .chain()
                    .after(crate::robots::perceive),
            );
    }
}

/// Lays out this cycle's anomalies (startup, and after a Surge reset).
pub fn spawn_cycle_anomalies(world: &mut World) {
    let opts = world.resource::<LaunchOptions>().clone();
    let cycle = world.resource::<Raid>().cycle;
    let zone = world.resource::<Zone>().clone();
    let old: Vec<Entity> = world
        .query_filtered::<Entity, With<Anomaly>>()
        .iter(world)
        .collect();
    for e in old {
        world.despawn(e);
    }
    for spec in layout(&zone.map, &zone.hm, opts.seed, cycle) {
        spawn_anomaly(
            world,
            spec.kind,
            zone.on_ground(spec.at[0], spec.at[1]),
            spec.radius,
            None,
        );
    }
}

pub fn spawn_anomaly(
    world: &mut World,
    kind: AnomalyKind,
    at: Vec3,
    radius: f32,
    bloom: Option<f32>,
) -> Entity {
    world
        .spawn((
            Name::new(format!("{kind:?} anomaly")),
            Anomaly {
                kind,
                radius,
                charge: if kind == AnomalyKind::Snare { 1.0 } else { 0.0 },
                bloom,
            },
            Transform::from_translation(at),
            Visibility::default(),
        ))
        .id()
}

/// The artifacts this cycle's anomalies hold, spawned as items (part of
/// rolling the Zone's loot, so saves keep them like any other item).
pub fn roll_artifacts(world: &mut World) {
    let opts = world.resource::<LaunchOptions>().clone();
    let cycle = world.resource::<Raid>().cycle;
    let zone = world.resource::<Zone>().clone();
    let catalog = world.resource::<Catalog>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for spec in layout(&zone.map, &zone.hm, opts.seed, cycle) {
            let Some(id) = spec.artifact else {
                continue;
            };
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let at = zone.on_ground(spec.at[0], spec.at[1]) + Vec3::Y * 0.4;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            );
        }
    }
    queue.apply(world);
}

fn falloff(d: f32, r: f32) -> f32 {
    (1.0 - d / r).clamp(0.0, 1.0)
}

/// Lifts: loose things float and circle; the longer something's in it, the
/// sooner it pops and flings everything out.
fn lifts(
    mut anomalies: Query<(Entity, &mut Anomaly, &GlobalTransform)>,
    mut bodies: Query<
        (&GlobalTransform, &RigidBody, &ComputedMass, Forces),
        Without<RigidBodyDisabled>,
    >,
    mut players: Query<(&GlobalTransform, &mut Body, &mut Health), With<Player>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut events: MessageWriter<AnomalyEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut a, gt) in &mut anomalies {
        if a.kind != AnomalyKind::Lift {
            continue;
        }
        let c = gt.translation();
        let core = c + Vec3::Y * 1.2;
        let hits = spatial.shape_intersections(
            &Collider::sphere(a.radius),
            core,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask([Layer::Item, Layer::Robot]),
        );
        let mut occupied = false;
        let popping = a.charge >= 1.0;
        for hit in hits {
            let Ok((bt, rb, mass, mut forces)) = bodies.get_mut(hit) else {
                continue;
            };
            if *rb != RigidBody::Dynamic {
                continue;
            }
            occupied = true;
            let p = bt.translation();
            let rel = p - core;
            let d = Vec2::new(rel.x, rel.z).length();
            let k = falloff(d, a.radius);
            let m = mass.value().max(0.05);
            if popping {
                let out = Vec3::new(rel.x, 0.0, rel.z).normalize_or(Vec3::X);
                forces.apply_linear_impulse((out * 4.0 + Vec3::Y * 7.0) * m);
            } else {
                // Cancel gravity and a bit more, toward a hover at the core,
                // and swirl.
                let lift = 9.81 * (1.0 + 0.3 * k) - rel.y.clamp(-1.0, 1.0) * 6.0;
                let swirl = Vec3::new(-rel.z, 0.0, rel.x).normalize_or_zero() * 3.0 * k;
                forces.apply_force((Vec3::Y * lift * k.sqrt() + swirl) * m);
            }
        }
        for (pt, mut body, mut health) in &mut players {
            let rel = pt.translation() - c;
            let d = Vec2::new(rel.x, rel.z).length();
            if d > a.radius || rel.y > 4.0 {
                continue;
            }
            occupied = true;
            if popping {
                let out = Vec3::new(rel.x, 0.0, rel.z).normalize_or(Vec3::X);
                body.velocity = out * 5.0 + Vec3::Y * 8.5;
                body.grounded = false;
                body.airborne = 1.0;
                health.hp -= 6.0;
                health.since_hit = 0.0;
            } else {
                // Light on your feet: gravity halves, and you're tugged round.
                let k = falloff(d, a.radius);
                body.velocity.y += 9.81 * 0.55 * k * dt;
                let swirl = Vec3::new(-rel.z, 0.0, rel.x).normalize_or_zero();
                body.velocity += swirl * 1.5 * k * dt;
            }
        }
        if popping {
            a.charge = 0.0;
            events.write(AnomalyEvent {
                anomaly: e,
                kind: a.kind,
                at: core,
                what: AnomalyAct::Pop,
            });
        } else if occupied {
            a.charge = (a.charge + dt / 4.0).min(1.0);
        } else {
            a.charge = (a.charge - dt / 6.0).max(0.0);
        }
    }
}

/// Burners: heat that hurts whoever stands in it, and cooks robot parts.
fn burners(
    mut anomalies: Query<(Entity, &mut Anomaly, &GlobalTransform)>,
    mut players: Query<(&GlobalTransform, &mut Health), With<Player>>,
    parts: Query<(), With<crate::robots::RobotPart>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut events: MessageWriter<AnomalyEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut a, gt) in &mut anomalies {
        if a.kind != AnomalyKind::Burner {
            continue;
        }
        let c = gt.translation();
        let mut inside = false;
        for (pt, mut h) in &mut players {
            let rel = pt.translation() - c;
            if Vec2::new(rel.x, rel.z).length() < a.radius && rel.y < 2.5 {
                inside = true;
                if !h.down {
                    h.hp -= 14.0 * dt;
                    h.since_hit = 0.0;
                }
            }
        }
        for hit in spatial.shape_intersections(
            &Collider::cylinder(a.radius, 3.0),
            c + Vec3::Y * 1.5,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask(Layer::Robot),
        ) {
            if parts.contains(hit) {
                inside = true;
                hits.write(Hit {
                    entity: hit,
                    point: c,
                    normal: Vec3::Y,
                    velocity: Vec3::ZERO,
                    damage: 18.0 * dt,
                    shooter: None,
                });
            }
        }
        // A flare when something new steps in (charge marks "occupied").
        if inside && a.charge < 0.5 {
            events.write(AnomalyEvent {
                anomaly: e,
                kind: a.kind,
                at: c,
                what: AnomalyAct::Flare,
            });
        }
        a.charge = if inside { 1.0 } else { 0.0 };
    }
}

/// Snares: discharge into everything in reach when anything touches them.
fn snares(
    mut anomalies: Query<(Entity, &mut Anomaly, &GlobalTransform)>,
    mut players: Query<(&GlobalTransform, &mut Health), With<Player>>,
    mut bodies: Query<(&RigidBody, Forces), Without<RigidBodyDisabled>>,
    parts: Query<(), With<crate::robots::RobotPart>>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut hits: MessageWriter<Hit>,
    mut events: MessageWriter<AnomalyEvent>,
    mut noises: MessageWriter<Noise>,
    mut rng: Local<Option<Rng>>,
) {
    let dt = time.delta_secs();
    let rng = rng.get_or_insert_with(|| Rng::new(0x5A4E));
    for (e, mut a, gt) in &mut anomalies {
        if a.kind != AnomalyKind::Snare {
            continue;
        }
        a.charge = (a.charge + dt / 3.5).min(1.0);
        if a.charge < 1.0 {
            continue;
        }
        let c = gt.translation() + Vec3::Y * 0.8;
        let touched: Vec<Entity> = spatial.shape_intersections(
            &Collider::sphere(a.radius),
            c,
            Quat::IDENTITY,
            &SpatialQueryFilter::from_mask([Layer::Item, Layer::Robot]),
        );
        let player_in = players
            .iter()
            .any(|(pt, _)| (pt.translation() + Vec3::Y * 0.9).distance(c) < a.radius + 0.3);
        if touched.is_empty() && !player_in {
            continue;
        }
        a.charge = 0.0;
        for (pt, mut h) in &mut players {
            if (pt.translation() + Vec3::Y * 0.9).distance(c) < a.radius + 0.3 && !h.down {
                h.hp -= 22.0;
                h.since_hit = 0.0;
            }
        }
        for t in touched {
            if parts.contains(t) {
                hits.write(Hit {
                    entity: t,
                    point: c,
                    normal: Vec3::Y,
                    velocity: Vec3::ZERO,
                    damage: 40.0,
                    shooter: None,
                });
            } else if let Ok((rb, mut f)) = bodies.get_mut(t) {
                if *rb == RigidBody::Dynamic {
                    let kick = Vec3::new(
                        rng.range(-1.0, 1.0),
                        rng.range(0.5, 1.5),
                        rng.range(-1.0, 1.0),
                    );
                    f.apply_linear_impulse(kick * 0.6);
                }
            }
        }
        events.write(AnomalyEvent {
            anomaly: e,
            kind: a.kind,
            at: c,
            what: AnomalyAct::Discharge,
        });
        noises.write(Noise {
            at: c,
            radius: 60.0,
            source: None,
        });
    }
}

/// Attention per player: the artifacts they carry, and the noise they make.
fn measure_attention(
    mut players: Query<(Entity, &mut Attention)>,
    carried: Query<(&Item, &artifacts::CarriedBy), Without<artifacts::Shielded>>,
    catalog: Res<Catalog>,
    mut noises: MessageReader<Noise>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let mut loud: Vec<(Entity, f32)> = Vec::new();
    for n in noises.read() {
        if let Some(s) = n.source {
            loud.push((s, n.radius));
        }
    }
    for (p, mut att) in &mut players {
        att.carried = carried
            .iter()
            .filter(|(_, c)| c.0 == p)
            .map(|(i, _)| catalog.get(i.kind()).attention)
            .sum();
        // Noise attention halves every minute.
        att.noise *= 0.5f32.powf(dt / 60.0);
        for (s, r) in &loud {
            if *s == p {
                att.noise = (att.noise + r / 400.0).min(3.0);
            }
        }
    }
}

/// The director spends what attention buys: response teams to where you
/// are, and anomalies blooming in your way.
fn spend_attention(
    mut world_cmds: Commands,
    mut budget: ResMut<AttentionBudget>,
    players: Query<(&Attention, &GlobalTransform, &Body), With<crate::rig::LocalPlayer>>,
    director: Option<ResMut<crate::robots::director::Director>>,
    blooms: Query<&Anomaly>,
    zone: Res<Zone>,
    mut raid: ResMut<Raid>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        return;
    };
    if !director.enabled {
        return;
    }
    let dt = time.delta_secs();
    let Some((att, gt, body)) = players.iter().next() else {
        return;
    };
    if raid.phase == crate::raid::Phase::AtBase {
        budget.warned = false;
        return;
    }
    if att.carried > 0.0 && !budget.warned {
        budget.warned = true;
        raid.say("Your teeth ache. Something has noticed you.");
    }
    budget.budget += att.total() * dt;
    let here = gt.translation();
    if budget.budget >= RESPONSE_COST {
        budget.budget -= RESPONSE_COST;
        budget.spent_on_responses += RESPONSE_COST;
        let delay = director.rng.range(25.0, 50.0);
        director.responses.push((delay, here));
        info!("director: attention buys a response to {here:.0}");
    }
    budget.next_bloom -= dt;
    let live = blooms.iter().filter(|a| a.bloom.is_some()).count();
    if att.carried > 0.0
        && budget.next_bloom <= 0.0
        && live < MAX_BLOOMS
        && budget.budget >= BLOOM_COST * 0.5
    {
        budget.next_bloom = director.rng.range(50.0, 110.0);
        budget.budget -= BLOOM_COST * 0.5;
        // Ahead of where you're going, or somewhere near if you're still.
        let heading = Vec3::new(body.velocity.x, 0.0, body.velocity.z);
        let dir = if heading.length() > 0.5 {
            heading.normalize()
        } else {
            let a = director.rng.range(0.0, std::f32::consts::TAU);
            Vec3::new(a.cos(), 0.0, a.sin())
        };
        let dist = director.rng.range(18.0, 35.0);
        let side = Vec3::new(-dir.z, 0.0, dir.x) * director.rng.range(-6.0, 6.0);
        let p = here + dir * dist + side;
        if extraction_core::anomaly::placeable(&zone.map, &zone.hm, [p.x, p.z]) {
            let kind = AnomalyKind::ALL[director.rng.below(3)];
            let (lo, hi) = kind.radius_range();
            let radius = director.rng.range(lo, hi);
            let at = zone.on_ground(p.x, p.z);
            world_cmds.queue(move |world: &mut World| {
                let e = spawn_anomaly(world, kind, at, radius, Some(BLOOM_LIFE));
                world.write_message(AnomalyEvent {
                    anomaly: e,
                    kind,
                    at,
                    what: AnomalyAct::Bloom,
                });
            });
            info!("director: a {kind:?} blooms at {at:.0}");
        }
    }
    let _ = JobKind::Response;
}

fn age_blooms(
    mut commands: Commands,
    mut anomalies: Query<(Entity, &mut Anomaly)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut a) in &mut anomalies {
        if let Some(life) = &mut a.bloom {
            *life -= dt;
            if *life <= 0.0 {
                commands.entity(e).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{SpawnItem, spawn_item};

    fn app(robots: bool) -> App {
        let opts = LaunchOptions {
            robots,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    fn find(app: &mut App, kind: AnomalyKind) -> (Entity, Vec3, f32) {
        app.world_mut()
            .query::<(Entity, &Anomaly, &GlobalTransform)>()
            .iter(app.world())
            .find(|(_, a, _)| a.kind == kind && a.bloom.is_none())
            .map(|(e, a, g)| (e, g.translation(), a.radius))
            .expect("this cycle has one")
    }

    fn drop_item(app: &mut App, id: &str, at: Vec3) -> Entity {
        let catalog = app.world().resource::<Catalog>().clone();
        let kind = catalog.kind(id).unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_item(
                &mut c,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            )
        };
        queue.apply(app.world_mut());
        e
    }

    #[test]
    fn a_thrown_bolt_sets_off_a_snare() {
        let mut app = app(false);
        let (snare, at, _) = find(&mut app, AnomalyKind::Snare);
        assert_eq!(app.world().get::<Anomaly>(snare).unwrap().charge, 1.0);
        drop_item(&mut app, "spare_parts", at + Vec3::Y * 1.5);
        let mut fired = false;
        for _ in 0..90 {
            app.update();
            if app.world().get::<Anomaly>(snare).unwrap().charge < 0.5 {
                fired = true;
                break;
            }
        }
        assert!(fired, "the snare discharged into the item");
    }

    #[test]
    fn a_lift_floats_things() {
        let mut app = app(false);
        let (_, at, _) = find(&mut app, AnomalyKind::Lift);
        let item = drop_item(&mut app, "canned_stew", at + Vec3::Y * 1.0);
        let mut highest: f32 = 0.0;
        for _ in 0..180 {
            app.update();
            let y = app
                .world()
                .get::<GlobalTransform>(item)
                .unwrap()
                .translation()
                .y
                - at.y;
            highest = highest.max(y);
        }
        assert!(highest > 1.3, "the tin rose (to {highest:.2} m)");
    }

    #[test]
    fn a_lead_case_hides_an_artifact() {
        let mut app = app(true);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let pack = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let case = drop_item(&mut app, "lead_case", Vec3::ZERO);
        let stone = drop_item(&mut app, "art_featherstone", Vec3::ZERO);
        app.update();
        crate::persist::seat_in_mount(app.world_mut(), case, stone);
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(pack)
            .unwrap()
            .contents
            .push(case);
        app.world_mut().entity_mut(case).insert((
            crate::interact::body::InBackpack { bag: pack },
            ChildOf(pack),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        app.world_mut().resource_mut::<Raid>().phase = crate::raid::Phase::Out;
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world().get::<artifacts::Shielded>(stone).is_some(),
            "the stone is sealed in"
        );
        assert!(
            app.world().get::<artifacts::CarriedBy>(stone).is_some(),
            "and still carried"
        );
        let att = app.world().get::<Attention>(player).unwrap().clone();
        assert!(
            att.carried.abs() < 1e-6,
            "no attention, got {}",
            att.carried
        );
        let kg = app.world().get::<Body>(player).unwrap().carried_kg;
        assert!(
            kg > 2.5,
            "the case is heavy and the stone lifts nothing ({kg})"
        );
    }

    #[test]
    fn carried_artifacts_draw_attention_and_lighten_the_load() {
        let mut app = app(true);
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let before = app.world().get::<Body>(player).unwrap().carried_kg;
        let pack = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        for id in ["art_featherstone", "art_hearth"] {
            let e = drop_item(&mut app, id, Vec3::ZERO);
            app.world_mut()
                .get_mut::<crate::interact::body::Backpack>(pack)
                .unwrap()
                .contents
                .push(e);
            app.world_mut().entity_mut(e).insert((
                crate::interact::body::InBackpack { bag: pack },
                ChildOf(pack),
                RigidBodyDisabled,
                ColliderDisabled,
            ));
        }
        app.world_mut().resource_mut::<Raid>().phase = crate::raid::Phase::Out;
        for _ in 0..5 {
            app.update();
        }
        let att = app.world().get::<Attention>(player).unwrap().clone();
        assert!(
            (att.carried - 1.8).abs() < 1e-3,
            "featherstone + hearth = 1.8/s, got {}",
            att.carried
        );
        let after = app.world().get::<Body>(player).unwrap().carried_kg;
        // +0.2 (stone) - 6 (its lift) + 0.8 (hearth), floored at zero.
        assert!(
            after < before + 0.01,
            "the featherstone carries itself and more ({before} -> {after})"
        );
        // A couple of minutes of that buys a response team.
        for _ in 0..90 * 70 {
            app.update();
        }
        assert!(
            app.world().resource::<AttentionBudget>().spent_on_responses >= RESPONSE_COST,
            "attention was spent"
        );
    }
}
