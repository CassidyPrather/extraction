//! The dirigible: a long pale airship that does the Zone's heavy lifting.
//!
//! On a *logistics* job it comes in over the map edge high up, crosses to a
//! work site, and lowers a cargo pod on a cable. Grapplers walk out from
//! the Gate to stand guard while the pod is down. After a while it reels
//! the pod back up — with whatever is still inside — and leaves the way it
//! came. The pod is the prize; the grapplers are the price.
//!
//! It's unarmed, but it sees a long way down, and what it sees it reports:
//! a response team follows. Three gas cells hold it up: puncture two and it
//! sinks, slowly, to a big wreck full of salvage.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::director::Director;
use super::{
    Awareness, Brain, Job, PartDestroyed, PartRole, Robot, RobotKind, Senses, part_bundle,
};
use crate::interact::body::Backpack;
use crate::world::{Layer, Zone};

/// Envelope size (length along Z).
pub const LENGTH: f32 = 16.0;
pub const RADIUS: f32 = 2.6;
const CRUISE: f32 = 42.0;
const HOVER: f32 = 26.0;
const SPEED: f32 = 5.0;

#[derive(Component, Debug)]
pub struct Dirigible {
    pub cells: [bool; 3],
    /// Height of the pod below the gondola, metres (0 = stowed).
    pub cable: f32,
    pub pod: Option<Entity>,
    pub velocity: Vec3,
    pub yaw: f32,
    pub report_cooldown: f32,
    pub sinking: bool,
}

/// A cargo pod: a container with good loot, down on the ground only while
/// the dirigible works. Its contents are never saved: if it's reeled back
/// up, whatever you didn't take is gone.
#[derive(Component, Debug)]
pub struct CargoPod {
    pub carrier: Entity,
}

pub struct DirigiblePlugin;

impl Plugin for DirigiblePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                dirigible_damage,
                dirigible_brain,
                fly_dirigibles,
                cargo,
                report_sightings,
            )
                .chain()
                .before(PhysicsSystems::First),
        );
    }
}

/// Spawns a dirigible at `at` (already at altitude).
pub fn spawn_dirigible(commands: &mut Commands, at: Vec3, job: Job) -> Entity {
    let robot = commands
        .spawn((
            Name::new("Dirigible"),
            Robot {
                kind: RobotKind::Dirigible,
                wrecked: false,
            },
            Dirigible {
                cells: [true; 3],
                cable: 0.0,
                pod: None,
                velocity: Vec3::ZERO,
                yaw: 0.0,
                report_cooldown: 0.0,
                sinking: false,
            },
            Senses {
                sight: 170.0,
                fov_deg: 300.0,
                hearing: 0.6,
                eye: Vec3::new(0.0, -RADIUS - 0.9, -2.0),
            },
            Awareness::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(at),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        for i in 0..3 {
            let z = (i as f32 - 1.0) * LENGTH * 0.3;
            c.spawn((
                Name::new(format!("Gas cell {i}")),
                part_bundle(
                    robot,
                    PartRole::GasCell(i as u8),
                    120.0,
                    Collider::sphere(RADIUS),
                    Transform::from_xyz(0.0, 0.0, z),
                ),
            ));
        }
        c.spawn((
            Name::new("Gondola"),
            part_bundle(
                robot,
                PartRole::Core,
                260.0,
                Collider::cuboid(1.4, 1.1, 4.0),
                Transform::from_xyz(0.0, -RADIUS - 0.5, 0.0),
            ),
        ));
        c.spawn((
            Name::new("Dirigible eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                40.0,
                Collider::sphere(0.3),
                Transform::from_xyz(0.0, -RADIUS - 1.1, -2.0),
            ),
        ));
    });
    robot
}

fn dirigible_damage(
    mut events: MessageReader<PartDestroyed>,
    mut ships: Query<(&mut Dirigible, &mut Robot)>,
) {
    for ev in events.read() {
        let Ok((mut d, mut robot)) = ships.get_mut(ev.robot) else {
            continue;
        };
        match ev.role {
            PartRole::GasCell(i) => {
                d.cells[i as usize] = false;
                if d.cells.iter().filter(|c| !**c).count() >= 2 {
                    d.sinking = true;
                }
            }
            // A dead gondola still floats; it just drifts off.
            PartRole::Core => {
                robot.wrecked = true;
                d.sinking = true;
            }
            _ => {}
        }
    }
}

fn dirigible_brain(
    mut commands: Commands,
    mut ships: Query<(Entity, &mut Dirigible, &mut Brain, &Job, &Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut d, mut brain, job, t) in &mut ships {
        if d.sinking {
            *brain = Brain::Down;
            continue;
        }
        let pos = t.translation;
        match &mut *brain {
            Brain::Transit { to } => {
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 3.0 {
                    *brain = Brain::Working {
                        at: *to,
                        left: job.duration,
                    };
                }
            }
            Brain::Working { left, .. } => {
                // The clock only runs once the pod is down.
                if d.cable > 0.0 && d.pod.is_some() {
                    *left -= dt;
                }
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
            }
            Brain::Leave { to } => {
                if d.cable <= 0.0 && Vec2::new(pos.x - to.x, pos.z - to.z).length() < 8.0 {
                    if let Some(p) = d.pod.take() {
                        commands.entity(p).try_despawn();
                    }
                    commands.entity(e).despawn();
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn fly_dirigibles(
    mut ships: Query<(&mut Dirigible, &Brain, &mut Transform, &mut Robot)>,
    zone: Res<Zone>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut d, brain, mut t, mut robot) in &mut ships {
        let pos = t.translation;
        let ground = zone.ground(pos.x, pos.z);
        if d.sinking {
            // Down slowly, drifting with what way it had.
            d.velocity = d.velocity * (1.0 - 0.2 * dt) + Vec3::NEG_Y * 1.4 * dt;
            d.velocity.y = d.velocity.y.max(-1.6);
            let mut next = pos + d.velocity * dt;
            if next.y - RADIUS - 1.0 <= ground {
                next.y = ground + RADIUS + 1.0;
                d.velocity = Vec3::ZERO;
                robot.wrecked = true;
            }
            t.translation = next;
            continue;
        }
        let (target, height) = match brain {
            Brain::Transit { to } => (*to, CRUISE),
            Brain::Working { at, .. } => (*at, HOVER),
            Brain::Leave { to } => (*to, if d.cable > 0.0 { HOVER } else { CRUISE }),
            _ => (pos, CRUISE),
        };
        // Hold still while the pod is on the cable.
        let reeling = d.cable > 0.0;
        let flat = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z);
        let want_h = if reeling {
            Vec3::ZERO
        } else {
            flat.normalize_or_zero() * SPEED * (flat.length() / 20.0).min(1.0)
        };
        let want_y =
            (ground.max(zone.ground(target.x, target.z)) + height - pos.y).clamp(-2.0, 2.0);
        let want = want_h + Vec3::Y * want_y;
        d.velocity = d.velocity.lerp(want, (0.4 * dt).min(1.0));
        t.translation = pos + d.velocity * dt;
        let v = Vec2::new(d.velocity.x, d.velocity.z);
        if v.length() > 0.3 {
            let yaw = (-v.x).atan2(-v.y);
            let mut dy = yaw - d.yaw;
            while dy > std::f32::consts::PI {
                dy -= std::f32::consts::TAU;
            }
            while dy < -std::f32::consts::PI {
                dy += std::f32::consts::TAU;
            }
            d.yaw += dy.clamp(-0.15 * dt, 0.15 * dt);
        }
        t.rotation = Quat::from_rotation_y(d.yaw);
    }
}

/// Lowering, landing and reeling up the cargo pod.
fn cargo(
    mut commands: Commands,
    mut ships: Query<(
        Entity,
        &mut Dirigible,
        &Brain,
        &GlobalTransform,
        &mut super::LootRng,
    )>,
    mut pods: Query<&mut Transform, With<CargoPod>>,
    zone: Res<Zone>,
    loot: Option<Res<crate::items::loot::Loot>>,
    catalog: Res<crate::items::Catalog>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut d, brain, gt, mut rng) in &mut ships {
        let pos = gt.translation();
        let gondola = pos.y - RADIUS - 1.1;
        let drop = (gondola - zone.ground(pos.x, pos.z) - 0.6).max(0.0);
        let lowering = matches!(brain, Brain::Working { .. }) && !d.sinking;
        let target = if lowering { drop } else { 0.0 };
        d.cable += (target - d.cable).clamp(-2.0 * dt, 2.0 * dt);
        if d.pod.is_none() && lowering {
            // The pod rides out with the ship; it's made when it starts down.
            let pod = commands
                .spawn((
                    Name::new("Cargo pod"),
                    CargoPod { carrier: e },
                    Backpack {
                        owner: Entity::PLACEHOLDER,
                        open: true,
                        model: extraction_core::inventory::Backpack {
                            capacity: 200.0,
                            mass: 0.0,
                        },
                        contents: Vec::new(),
                        carried: false,
                        half: Vec3::new(0.55, 0.35, 0.8),
                    },
                    Transform::from_translation(pos),
                    Visibility::default(),
                ))
                .with_child((
                    Collider::cuboid(1.3, 0.9, 1.8),
                    CollisionLayers::new(Layer::World, LayerMask::ALL),
                    Transform::from_xyz(0.0, -0.1, 0.0),
                ))
                .id();
            if let Some(loot) = &loot {
                let mut queue = Vec::new();
                for drop in loot.0.roll("cargo_pod", &catalog, &mut rng.0) {
                    queue.push(drop);
                }
                for (i, drop) in queue.into_iter().enumerate() {
                    let item = crate::items::spawn_item(
                        &mut commands,
                        &catalog,
                        crate::items::SpawnItem {
                            kind: drop.kind,
                            transform: Transform::IDENTITY,
                            condition: drop.condition,
                        },
                    );
                    let x = (i as f32 * 0.37 % 1.0 - 0.5) * 0.9;
                    let z = (i as f32 * 0.61 % 1.0 - 0.5) * 1.4;
                    commands.entity(item).insert((
                        crate::interact::body::InBackpack { bag: pod },
                        ChildOf(pod),
                        Transform::from_xyz(x, -0.2, z),
                        RigidBodyDisabled,
                        ColliderDisabled,
                    ));
                    commands.queue(move |world: &mut World| {
                        if let Some(mut b) = world.get_mut::<Backpack>(pod) {
                            b.contents.push(item);
                        }
                    });
                }
            }
            d.pod = Some(pod);
        }
        // Going down: the cable parts and the pod drops where it hangs.
        if d.sinking {
            if let Some(p) = d.pod.take() {
                if let Ok(mut t) = pods.get_mut(p) {
                    let g = zone.ground(t.translation.x, t.translation.z);
                    t.translation.y = g + 0.45;
                }
            }
            continue;
        }
        if let Some(p) = d.pod {
            if let Ok(mut t) = pods.get_mut(p) {
                t.translation = Vec3::new(pos.x, gondola - d.cable - 0.45, pos.z);
            }
            // Reeled all the way back up after the job: it goes with the ship.
            if !lowering && d.cable <= 0.0 && matches!(brain, Brain::Leave { .. }) {
                commands.entity(p).try_despawn();
                d.pod = None;
            }
        }
    }
}

/// What the dirigible sees, it reports: a response team to the spot.
fn report_sightings(
    mut ships: Query<(&mut Dirigible, &Awareness)>,
    director: Option<ResMut<Director>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        return;
    };
    let dt = time.delta_secs();
    for (mut d, aware) in &mut ships {
        d.report_cooldown -= dt;
        if d.sinking || !aware.can_see_target || aware.suspicion < 0.6 || d.report_cooldown > 0.0 {
            continue;
        }
        if let Some(p) = aware.target.and_then(|t| targets.get(t).ok()) {
            let delay = director.rng.range(20.0, 40.0);
            director.queue_response(delay, p.translation());
            d.report_cooldown = 90.0;
            info!("dirigible: spotted someone at {:.0}", p.translation());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_logistics_run_lands_a_full_pod() {
        let opts = crate::LaunchOptions {
            robots: true,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        {
            let mut d = app.world_mut().resource_mut::<Director>();
            d.next_job = 1e9;
            d.next_logistics = 0.0;
        }
        let zone = app.world().resource::<Zone>().clone();
        let mut landed = None;
        for _ in 0..90 * 200 {
            app.update();
            let pod = app
                .world_mut()
                .query::<(&CargoPod, &GlobalTransform, &Backpack)>()
                .iter(app.world())
                .map(|(_, g, b)| (g.translation(), b.contents.len()))
                .next();
            if let Some((p, n)) = pod {
                if p.y - zone.ground(p.x, p.z) < 1.2 {
                    landed = Some(n);
                    break;
                }
            }
        }
        let n = landed.expect("the pod came down");
        assert!(n >= 4, "with cargo in it ({n})");
        let grapplers = app
            .world_mut()
            .query::<&crate::robots::grappler::Grappler>()
            .iter(app.world())
            .count();
        assert_eq!(grapplers, 2, "and two grapplers walked out to guard it");
    }
}
