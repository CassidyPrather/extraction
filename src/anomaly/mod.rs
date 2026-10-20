//! Anomalies, artifacts and attention.
//!
//! - **Anomalies** sit where the Surge left them ([`extraction_core::anomaly`]
//!   lays them out per cycle). A *lift* floats and swirls anything loose and
//!   flings it when it pops; a *burner* cooks whoever stands in it; a
//!   *snare* discharges into whatever touches it, robots included; a
//!   *well* ([`wells`]) drags everything in toward a heart that crushes,
//!   and bends bullets; a *mirror* ([`mirrors`]) blinks whatever crosses
//!   it out the far side. Throw something in first, or carry a detector
//!   ([`detector`]).
//! - **Artifacts** grow at their hearts. Carried, they do odd, useful
//!   things (see [`artifacts`]) — and they draw **attention**.
//! - **Attention** (carried artifacts, plus recent gunfire) is what the
//!   director spends: response teams sent to where you *are*, and new
//!   anomalies blooming across your path.

pub mod artifacts;
pub mod detector;
pub mod mirrors;
pub mod offering;
pub mod visuals;
pub mod wells;

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
#[require(detector::Touched)]
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
    /// A mirror blinked something away.
    Blink,
    /// Something thrown in showed it up.
    Reveal,
    /// A well's heart started crushing something.
    Crush,
}

/// A snare's discharge reached out to something (an arc to draw): metal
/// in a hand draws it from further off.
#[derive(Message, Clone, Copy, Debug)]
pub struct ArcStrike {
    pub anomaly: Entity,
    pub from: Vec3,
    pub to: Vec3,
}

/// How much further a snare reaches for metal in a hand, metres.
pub const SNARE_METAL_REACH: f32 = 0.8;
/// A lift draws loose things in from this many radii out as it nears
/// popping.
pub const LIFT_SUCTION: f32 = 1.8;

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

    /// How it feels, 0 (nothing) to 4: there is no number, only this.
    pub fn feeling(&self) -> usize {
        match self.total() {
            t if t <= 0.05 => 0,
            t if t < 0.5 => 1,
            t if t < 1.5 => 2,
            t if t < 2.5 => 3,
            _ => 4,
        }
    }
}

/// The feelings, by [`Attention::feeling`].
pub const FEELINGS: [&str; 5] = [
    "",
    "a prickle at the back of your neck",
    "your teeth ache",
    "the air hums around you",
    "the whole Zone is looking at you",
];

/// The director's attention account.
#[derive(Resource, Debug, Default)]
pub struct AttentionBudget {
    pub spent_on_responses: f32,
    pub budget: f32,
    pub next_bloom: f32,
    pub warned: bool,
    /// The strongest feeling announced lately (see [`Attention::feeling`]).
    pub felt: usize,
}

/// Attention-seconds per response team.
pub const RESPONSE_COST: f32 = 90.0;
/// Attention-seconds per bloom.
pub const BLOOM_COST: f32 = 60.0;
pub const MAX_BLOOMS: usize = 3;
/// The most attention your own noise can draw (about a hard minute's
/// shooting; it decays in half a minute).
pub const NOISE_CAP: f32 = 1.0;
pub const BLOOM_LIFE: f32 = 240.0;

pub struct AnomalyPlugin;

impl Plugin for AnomalyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AnomalyEvent>()
            .add_message::<ArcStrike>()
            .add_message::<mirrors::Blinked>()
            .add_message::<detector::DetectorClick>()
            .init_resource::<AttentionBudget>()
            .add_observer(|add: On<Add, Player>, mut commands: Commands| {
                commands.entity(add.entity).insert(Attention::default());
            })
            .add_plugins((artifacts::ArtifactPlugin, offering::OfferingPlugin))
            .add_systems(Startup, spawn_cycle_anomalies)
            .add_systems(
                FixedUpdate,
                (
                    // Before anything can blink or fling what was thrown.
                    detector::reveal,
                    lifts,
                    burners,
                    snares,
                    wells::wells,
                    mirrors::mirrors,
                    mirrors::shards,
                    detector::detectors,
                    measure_attention,
                    spend_attention,
                    age_blooms,
                )
                    .chain()
                    .after(crate::robots::perceive),
            )
            .add_systems(
                FixedUpdate,
                wells::bend_rounds.before(crate::weapons::ballistics::BallisticsSystems),
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
    crate::world::underground::spawn_anomalies(world);
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
                charge: if matches!(kind, AnomalyKind::Snare | AnomalyKind::Mirror) {
                    1.0
                } else {
                    0.0
                },
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
        // Winding up to pop, it draws loose things in from further out:
        // leaves and junk sliding toward it is the warning.
        let windup = ((a.charge - 0.5) * 2.0).clamp(0.0, 1.0);
        let reach = if windup > 0.0 {
            a.radius * LIFT_SUCTION
        } else {
            a.radius
        };
        let hits = spatial.shape_intersections(
            &Collider::sphere(reach),
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
            let p = bt.translation();
            let rel = p - core;
            let d = Vec2::new(rel.x, rel.z).length();
            let m = mass.value().max(0.05);
            if d > a.radius {
                if !popping {
                    let inward = -Vec3::new(rel.x, 0.0, rel.z).normalize_or_zero();
                    forces.apply_force((inward * 7.0 + Vec3::Y * 3.0) * windup * m);
                }
                continue;
            }
            occupied = true;
            let k = falloff(d, a.radius);
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
                    mass_g: 0.0,
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
#[allow(clippy::too_many_arguments)]
fn snares(
    mut anomalies: Query<(Entity, &mut Anomaly, &GlobalTransform)>,
    mut players: Query<(Entity, &GlobalTransform, &mut Health), With<Player>>,
    held: Query<(&GlobalTransform, &Item, &crate::interact::Held)>,
    catalog: Res<Catalog>,
    positions: Query<&GlobalTransform>,
    mut arcs: MessageWriter<ArcStrike>,
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
            .any(|(_, pt, _)| (pt.translation() + Vec3::Y * 0.9).distance(c) < a.radius + 0.3);
        // Metal in a hand draws an arc from further off, through you.
        let metal: Vec<(Entity, Vec3)> = held
            .iter()
            .filter(|(g, item, _)| {
                g.translation().distance(c) < a.radius + SNARE_METAL_REACH
                    && catalog.get(item.kind()).tags.iter().any(|t| t == "metal")
            })
            .map(|(g, _, h)| (h.player, g.translation()))
            .collect();
        if touched.is_empty() && !player_in && metal.is_empty() {
            continue;
        }
        a.charge = 0.0;
        for (pe, pt, mut h) in &mut players {
            let inside = (pt.translation() + Vec3::Y * 0.9).distance(c) < a.radius + 0.3;
            let grounded = metal.iter().any(|(p, _)| *p == pe);
            if (inside || grounded) && !h.down {
                h.hp -= 22.0;
                h.since_hit = 0.0;
            }
        }
        for (_, to) in &metal {
            arcs.write(ArcStrike {
                anomaly: e,
                from: c,
                to: *to,
            });
        }
        for t in touched {
            if held.contains(t) {
                // Struck above, through whoever holds it.
                continue;
            }
            if let Ok(g) = positions.get(t) {
                arcs.write(ArcStrike {
                    anomaly: e,
                    from: c,
                    to: g.translation(),
                });
            }
            if parts.contains(t) {
                hits.write(Hit {
                    entity: t,
                    point: c,
                    normal: Vec3::Y,
                    velocity: Vec3::ZERO,
                    damage: 40.0,
                    shooter: None,
                    mass_g: 0.0,
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
        // Noise attention halves every half minute. It is a small share:
        // gunfire already gets a team sent where it was heard
        // (`director::hear_reports`), and a big share here turned every
        // fight into three more.
        att.noise *= 0.5f32.powf(dt / 30.0);
        for (s, r) in &loud {
            if *s == p {
                att.noise = (att.noise + r / 1500.0).min(NOISE_CAP);
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
        budget.felt = 0;
        return;
    }
    if att.carried > 0.0 && !budget.warned {
        budget.warned = true;
        raid.say("Your teeth ache. Something has noticed you.");
    }
    // It getting worse is news; easing off a step lets it be news again.
    let feeling = att.feeling();
    if feeling > budget.felt {
        budget.felt = feeling;
        match feeling {
            3 => raid.say("The air hums around you. More of them are coming."),
            4 => raid.say("The whole Zone is looking at you."),
            _ => {}
        }
    } else if feeling + 1 < budget.felt {
        budget.felt = feeling + 1;
    }
    budget.budget += att.total() * dt;
    let here = gt.translation();
    if budget.budget >= RESPONSE_COST {
        budget.budget -= RESPONSE_COST;
        budget.spent_on_responses += RESPONSE_COST;
        let delay = director.rng.range(25.0, 50.0);
        director.queue_response(delay, here);
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
            let kind = AnomalyKind::ALL[director.rng.below(AnomalyKind::ALL.len())];
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

    fn player(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap()
    }

    fn pos(app: &App, e: Entity) -> Vec3 {
        app.world().get::<GlobalTransform>(e).unwrap().translation()
    }

    #[test]
    fn a_well_drags_loose_things_into_its_heart() {
        let mut app = app(false);
        let (_, at, r) = find(&mut app, AnomalyKind::Well);
        let heart = wells::heart(at);
        let start = at + Vec3::new(r * 2.2, 0.6, 0.0);
        let tin = drop_item(&mut app, "canned_stew", start);
        let mut closest = f32::MAX;
        for _ in 0..400 {
            app.update();
            closest = closest.min(pos(&app, tin).distance(heart));
        }
        assert!(
            closest < r * 0.6,
            "dropped {:.1} m out, the tin was drawn in to {closest:.2} m of the heart",
            r * 2.2
        );
        let end = pos(&app, tin).distance(heart);
        assert!(end < r, "and circles there ({end:.2} m)");
    }

    #[test]
    fn a_wells_heart_crushes_and_its_edge_lets_you_go() {
        let mut app = app(false);
        let (_, at, _) = find(&mut app, AnomalyKind::Well);
        let p = player(&mut app);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = at;
        for _ in 0..30 {
            app.update();
        }
        let hp = app.world().get::<Health>(p).unwrap().hp;
        assert!(hp < 95.0, "the heart crushes ({hp})");
    }

    /// Fires a round along +X past `heart` at `miss` metres and returns
    /// how far it turned, degrees.
    fn shot_past(app: &mut App, heart: Vec3, miss: f32, speed: f32) -> f32 {
        let from = heart + Vec3::new(-25.0, miss, 0.0);
        let round = app
            .world_mut()
            .spawn((
                crate::weapons::ballistics::Projectile {
                    velocity: Vec3::X * speed,
                    damage: 0.0,
                    shooter: None,
                    gun: Entity::PLACEHOLDER,
                    life: 2.0,
                    mods: Default::default(),
                    mass_g: 8.0,
                    seed: 0,
                    passes: 0,
                },
                Transform::from_translation(from),
            ))
            .id();
        let mut v = Vec3::X;
        for _ in 0..12 {
            app.update();
            match app
                .world()
                .get::<crate::weapons::ballistics::Projectile>(round)
            {
                Some(p) => v = p.velocity,
                None => break,
            }
        }
        // Gravity drops it a little either way; only the turn in y counts
        // against the round fired far from any well.
        v.y.atan2(v.x).to_degrees()
    }

    #[test]
    fn a_well_bends_rounds_that_pass_close() {
        let mut app = app(false);
        let (_, at, _) = find(&mut app, AnomalyKind::Well);
        let heart = wells::heart(at);
        let far = shot_past(&mut app, heart + Vec3::new(0.0, 0.0, 60.0), 1.0, 330.0);
        let near = shot_past(&mut app, heart, 1.0, 330.0);
        assert!(
            (near - far).abs() > 2.0,
            "a pistol round a metre over the heart turns down toward it ({near:.2}° vs {far:.2}°)"
        );
        assert!(near < far, "toward the heart, below it");
    }

    #[test]
    fn a_mirror_blinks_a_thrown_bolt_out_the_far_side() {
        let mut app = app(false);
        let (m, at, r) = find(&mut app, AnomalyKind::Mirror);
        // Someone near enough to see it.
        let p = player(&mut app);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = at + Vec3::Z * 12.0;
        let bolt = drop_item(
            &mut app,
            "bolt",
            at + Vec3::new(-r - 1.0, mirrors::MIRROR_HEIGHT + 0.3, 0.0),
        );
        app.update();
        app.world_mut()
            .entity_mut(bolt)
            .insert(LinearVelocity(Vec3::new(5.0, 1.0, 0.0)));
        let mut blinked = false;
        for _ in 0..90 {
            app.update();
            if app.world().get::<Anomaly>(m).unwrap().charge < 0.5 {
                blinked = true;
            }
        }
        assert!(blinked, "the mirror fired");
        let p = pos(&app, bolt);
        assert!(
            p.x > at.x + r + 1.0,
            "thrown in from the west, the bolt came out east of it ({:.1} vs centre {:.1})",
            p.x,
            at.x
        );
        assert!(
            app.world().get::<detector::Touched>(m).unwrap().revealed > 0.0,
            "and showed it up"
        );
    }

    #[test]
    fn walking_into_a_mirror_puts_you_out_beyond_it() {
        let mut app = app(false);
        let (_, at, r) = find(&mut app, AnomalyKind::Mirror);
        let p = player(&mut app);
        let entry = at + Vec3::new(0.0, 0.0, r * 0.6);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = entry;
        for _ in 0..5 {
            app.update();
        }
        let now = pos(&app, p);
        let d = Vec2::new(now.x - at.x, now.z - at.z).length();
        assert!(d > r + 1.5, "blinked {d:.1} m from its centre");
    }

    #[test]
    fn a_thrown_bolt_shows_up_a_burner() {
        let mut app = app(false);
        let (b, at, _) = find(&mut app, AnomalyKind::Burner);
        assert_eq!(
            app.world().get::<detector::Touched>(b).unwrap().revealed,
            0.0
        );
        let p = player(&mut app);
        app.world_mut().get_mut::<Transform>(p).unwrap().translation = at + Vec3::X * 15.0;
        drop_item(&mut app, "bolt", at + Vec3::Y * 1.5);
        let mut seen = false;
        for _ in 0..60 {
            app.update();
            seen |= app.world().get::<detector::Touched>(b).unwrap().revealed > 0.0;
        }
        assert!(seen, "the bolt showed it up");
    }

    #[test]
    fn a_detector_clicks_faster_near_an_anomaly() {
        let mut app = app(false);
        let (_, at, r) = find(&mut app, AnomalyKind::Snare);
        let p = player(&mut app);
        let pack = app
            .world_mut()
            .query::<(Entity, &crate::interact::body::Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == p && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let det = drop_item(&mut app, "detector", Vec3::ZERO);
        app.world_mut()
            .get_mut::<crate::interact::body::Backpack>(pack)
            .unwrap()
            .contents
            .push(det);
        app.world_mut().entity_mut(det).insert((
            crate::interact::body::InBackpack { bag: pack },
            ChildOf(pack),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        let count = |app: &mut App, d: f32| {
            app.world_mut().get_mut::<Transform>(p).unwrap().translation =
                at + Vec3::new(r + d, 0.0, 0.0);
            for _ in 0..10 {
                app.update();
            }
            let mut cursor = app
                .world()
                .resource::<Messages<detector::DetectorClick>>()
                .get_cursor_current();
            let mut n = 0;
            for _ in 0..300 {
                app.update();
                n += cursor
                    .read(app.world().resource::<Messages<detector::DetectorClick>>())
                    .filter(|c| c.detector == det)
                    .count();
            }
            n
        };
        let far = count(&mut app, 18.0);
        let near = count(&mut app, 2.0);
        assert!(far >= 1, "a tick now and then at 18 m ({far})");
        assert!(near > far * 4, "a rattle at 2 m ({near} vs {far})");
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
        let raid = app.world().resource::<Raid>();
        assert!(
            raid.log.iter().any(|(l, _)| l.starts_with("The air hums")),
            "1.8/s is felt: {:?}",
            raid.log
        );
        assert_eq!(att.feeling(), 3);
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
