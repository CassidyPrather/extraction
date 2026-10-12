//! The walker: a sensor orb carried on four ball-jointed legs.
//!
//! Locomotion is procedural. The orb glides along an A* path; each foot
//! stays planted until the place it *should* be has drifted too far, then
//! it lifts and arcs to a new footing on the real ground. Legs step in
//! diagonal pairs. Knees come from two-bone IK. Shoot a leg off and it
//! hangs limp; lose two and the orb comes down.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::nav::{NavGrid, Obstacle};
use std::sync::Arc;

use super::{
    Awareness, Brain, Job, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::weapons::ballistics::Shot;
use crate::world::Zone;

const LEGS: usize = 4;
const BODY_HEIGHT: f32 = 1.15;
pub const UPPER: f32 = 0.8;
pub const LOWER: f32 = 0.95;
const HIP_RADIUS: f32 = 0.32;
const FOOT_RADIUS: f32 = 1.05;
const STEP_TRIGGER: f32 = 0.42;
const STEP_TIME: f32 = 0.28;
const STEP_HEIGHT: f32 = 0.28;
const SPEED: f32 = 1.7;

/// The shared navigation grid for ground robots (and bots).
#[derive(Resource, Clone)]
pub struct Nav(pub Arc<NavGrid>);

#[derive(Clone, Copy, Debug)]
pub struct Foot {
    pub planted: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    /// 0..1 through a step; 1 when planted.
    pub t: f32,
    pub alive: bool,
}

#[derive(Component, Debug)]
pub struct Walker {
    pub feet: [Foot; LEGS],
    pub path: Vec<Vec3>,
    pub repath: f32,
    pub velocity: Vec3,
    pub yaw: f32,
    pub gun_cooldown: f32,
    pub turret_yaw: f32,
    /// Which diagonal pair may step next.
    pub phase: usize,
    pub collapsed: bool,
    pub goal: Option<Vec3>,
}

/// A leg's collider and visual root, posed every tick from the IK.
#[derive(Component)]
pub struct WalkerLeg {
    pub index: usize,
}

#[derive(Component)]
pub struct WalkerTurret;

/// Joint positions of each leg, for visuals: hip, knee, foot (world).
#[derive(Component, Default, Debug)]
pub struct LegPoses(pub [[Vec3; 3]; LEGS]);

pub struct WalkerPlugin;

impl Plugin for WalkerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            build_nav.after(crate::world::trees::spawn_tree_colliders),
        )
        .add_systems(
            FixedUpdate,
            (walker_parts_destroyed, walker_brain, walk, walker_guns)
                .chain()
                .before(PhysicsSystems::First),
        );
    }
}

fn hip_offset(i: usize) -> Vec3 {
    let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
    Vec3::new(a.cos() * HIP_RADIUS, -0.12, a.sin() * HIP_RADIUS)
}

fn rest_offset(i: usize) -> Vec3 {
    let a = std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2;
    Vec3::new(a.cos() * FOOT_RADIUS, 0.0, a.sin() * FOOT_RADIUS)
}

/// Navigation for ground robots: terrain, structures and tree trunks.
pub fn build_nav(mut commands: Commands, zone: Res<Zone>) {
    let started = std::time::Instant::now();
    let mut g = NavGrid::from_heightmap(&zone.hm, 2.0, 32.0);
    for p in &zone.map.placements {
        let f = crate::world::structures::footprint(&p.prefab);
        g.block(
            Obstacle::Rect {
                at: p.at,
                half: [f.x * 0.5, f.z * 0.5],
                yaw: p.rot.to_radians(),
            },
            0.6,
        );
    }
    for t in crate::world::trees::scatter_trees(&zone) {
        g.block(
            Obstacle::Circle {
                at: [t.pos.x, t.pos.z],
                radius: 0.3 * t.scale,
            },
            0.5,
        );
    }
    for f in &zone.map.forests {
        g.scale_cost_in(&f.polygon, 1.6);
    }
    // Bridges are walkable even though the creek under them isn't.
    for rl in &zone.hm.road_lines {
        for (a, b) in &rl.bridges {
            let pa = extraction_core::geom::polyline_at(&rl.points, *a).0;
            let pb = extraction_core::geom::polyline_at(&rl.points, *b).0;
            g.open_line(pa, pb, 1);
        }
    }
    info!(
        "navigation grid built in {:.0} ms",
        started.elapsed().as_secs_f32() * 1000.0
    );
    commands.insert_resource(Nav(Arc::new(g)));
}

pub fn spawn_walker(commands: &mut Commands, zone: &Zone, at: Vec3, job: Job) -> Entity {
    spawn_walker_with(commands, zone, at, job, true, 130.0)
}

/// A walker body: `armed` gives it the turret; `core_hp` is how much it
/// takes to bring down (the convoy collector is sturdier and unarmed).
pub fn spawn_walker_with(
    commands: &mut Commands,
    zone: &Zone,
    at: Vec3,
    job: Job,
    armed: bool,
    core_hp: f32,
) -> Entity {
    let ground = zone.ground(at.x, at.z);
    let pos = Vec3::new(at.x, ground + BODY_HEIGHT, at.z);
    let feet = std::array::from_fn(|i| {
        let p = pos + rest_offset(i);
        let f = Vec3::new(p.x, zone.ground(p.x, p.z), p.z);
        Foot {
            planted: f,
            from: f,
            to: f,
            t: 1.0,
            alive: true,
        }
    });
    let robot = commands
        .spawn((
            Name::new("Walker"),
            Robot {
                kind: RobotKind::Walker,
                wrecked: false,
            },
            Walker {
                feet,
                path: Vec::new(),
                repath: 0.0,
                velocity: Vec3::ZERO,
                yaw: 0.0,
                gun_cooldown: 2.0,
                turret_yaw: 0.0,
                phase: 0,
                collapsed: false,
                goal: None,
            },
            LegPoses::default(),
            Senses {
                sight: 75.0,
                fov_deg: 200.0,
                hearing: 1.2,
                eye: Vec3::new(0.0, 0.15, -0.3),
            },
            Awareness::default(),
            Brain::Transit { to: job.site },
            job,
            Transform::from_translation(pos),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Walker orb"),
            part_bundle(
                robot,
                PartRole::Core,
                core_hp,
                Collider::sphere(0.4),
                Transform::IDENTITY,
            ),
        ));
        c.spawn((
            Name::new("Walker eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                25.0,
                Collider::cuboid(0.3, 0.08, 0.06),
                Transform::from_xyz(0.0, 0.12, -0.36),
            ),
        ));
        if armed {
            c.spawn((
                Name::new("Walker turret"),
                WalkerTurret,
                part_bundle(
                    robot,
                    PartRole::Weapon,
                    45.0,
                    Collider::cuboid(0.16, 0.12, 0.4),
                    Transform::from_xyz(0.0, 0.46, 0.0),
                ),
            ));
        }
        for i in 0..LEGS {
            c.spawn((
                Name::new(format!("Walker leg {i}")),
                WalkerLeg { index: i },
                part_bundle(
                    robot,
                    PartRole::Leg(i as u8),
                    35.0,
                    Collider::capsule(0.06, LOWER),
                    Transform::IDENTITY,
                ),
            ));
        }
    });
    robot
}

fn walker_parts_destroyed(
    mut events: MessageReader<PartDestroyed>,
    mut walkers: Query<(&mut Walker, &mut Robot)>,
) {
    for ev in events.read() {
        let Ok((mut w, mut robot)) = walkers.get_mut(ev.robot) else {
            continue;
        };
        match ev.role {
            PartRole::Leg(i) => {
                w.feet[i as usize].alive = false;
                if w.feet.iter().filter(|f| !f.alive).count() >= 2 {
                    w.collapsed = true;
                    robot.wrecked = true;
                }
            }
            PartRole::Core => {
                w.collapsed = true;
                robot.wrecked = true;
            }
            _ => {}
        }
    }
}

fn walker_brain(
    mut commands: Commands,
    mut walkers: Query<(
        Entity,
        &mut Walker,
        &mut Brain,
        &Job,
        &Awareness,
        &Transform,
        &Robot,
    )>,
    targets: Query<&GlobalTransform>,
    nav: Res<Nav>,
    time: Res<Time>,
    mut convoy: MessageWriter<super::collector::ConvoyEvent>,
) {
    let dt = time.delta_secs();
    for (e, mut w, mut brain, job, aware, t, robot) in &mut walkers {
        if robot.wrecked {
            *brain = Brain::Down;
            w.goal = None;
            continue;
        }
        let pos = t.translation;
        // The collector has one job and is not to be distracted from it.
        let single_minded = job.kind == super::JobKind::Collect;
        if !single_minded
            && aware.can_see_target
            && aware.suspicion > 0.55
            && !matches!(*brain, Brain::Engage | Brain::Down)
        {
            *brain = Brain::Engage;
        } else if !single_minded
            && aware.suspicion > 0.35
            && matches!(*brain, Brain::Transit { .. } | Brain::Working { .. })
        {
            if let Some(at) = aware.last_known {
                *brain = Brain::Investigate { at, left: 30.0 };
            }
        }
        let goal = match &mut *brain {
            Brain::Transit { to } => {
                let to = *to;
                if Vec2::new(pos.x - to.x, pos.z - to.z).length() < 3.0 {
                    *brain = Brain::Working {
                        at: to,
                        left: job.duration,
                    };
                }
                Some(to)
            }
            Brain::Working { left, .. } => {
                *left -= dt;
                if *left <= 0.0 {
                    if single_minded {
                        convoy.write(super::collector::ConvoyEvent::Loaded { robot: e });
                    }
                    *brain = Brain::Leave { to: job.exit };
                }
                None
            }
            Brain::Investigate { at, left } => {
                *left -= dt;
                let at = *at;
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
                (Vec2::new(pos.x - at.x, pos.z - at.z).length() > 4.0).then_some(at)
            }
            Brain::Engage => {
                let target = aware
                    .target
                    .and_then(|p| targets.get(p).ok())
                    .map(|g| g.translation());
                match target {
                    Some(tp) if aware.unseen < 10.0 => {
                        // Close to firing range and hold; advance if sight is lost.
                        let d = tp.distance(pos);
                        if !aware.can_see_target || d > 35.0 {
                            Some(tp)
                        } else {
                            None
                        }
                    }
                    _ => {
                        *brain = Brain::Investigate {
                            at: aware.last_known.unwrap_or(pos),
                            left: 25.0,
                        };
                        None
                    }
                }
            }
            Brain::Leave { to } => {
                let to = *to;
                // The Gate itself is blocked on the nav grid, so "there" is
                // close, or at the end of the path to the nearest open cell.
                let d = Vec2::new(pos.x - to.x, pos.z - to.z).length();
                let arrived = d < 5.0 || (d < 14.0 && w.goal == Some(to) && w.path.is_empty());
                if arrived {
                    if single_minded {
                        // The convoy system records what it carried out, then
                        // despawns it.
                        convoy.write(super::collector::ConvoyEvent::Delivered { robot: e });
                    } else {
                        commands.entity(e).despawn();
                    }
                    continue;
                }
                Some(to)
            }
            Brain::Down => None,
        };
        w.repath -= dt;
        if goal != w.goal || (goal.is_some() && w.repath <= 0.0 && w.path.is_empty()) {
            w.goal = goal;
            w.repath = 3.0;
            w.path = goal
                .and_then(|g| nav.0.path([pos.x, pos.z], [g.x, g.z]))
                .map(|p| {
                    p.into_iter()
                        .skip(1)
                        .map(|q| Vec3::new(q[0], 0.0, q[1]))
                        .collect()
                })
                .unwrap_or_default();
        }
    }
}

/// Body motion along the path, then feet, then IK.
fn walk(
    mut walkers: Query<(
        &mut Walker,
        &mut Transform,
        &mut LegPoses,
        &Awareness,
        &Children,
        &Robot,
    )>,
    mut legs: Query<(&WalkerLeg, &mut Transform), Without<Walker>>,
    targets: Query<&GlobalTransform>,
    zone: Res<Zone>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut w, mut t, mut poses, aware, children, robot) in &mut walkers {
        let pos = t.translation;
        // Steering.
        let mut desired = Vec3::ZERO;
        if let Some(next) = w.path.first().copied() {
            let to = Vec3::new(next.x - pos.x, 0.0, next.z - pos.z);
            if to.length() < 0.8 {
                w.path.remove(0);
            } else {
                desired = to.normalize() * SPEED;
            }
        }
        let alive_legs = w.feet.iter().filter(|f| f.alive).count();
        if w.collapsed || robot.wrecked {
            desired = Vec3::ZERO;
        } else if alive_legs < LEGS {
            desired *= 0.45; // limping
        }
        let v = w.velocity.lerp(desired, (3.0 * dt).min(1.0));
        w.velocity = v;
        // Face travel, or the target when fighting.
        let face = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() - pos)
            .unwrap_or(v);
        if Vec2::new(face.x, face.z).length() > 0.2 {
            let want = (-face.x).atan2(-face.z);
            let mut d = want - w.yaw;
            while d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            while d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            w.yaw += d.clamp(-1.6 * dt, 1.6 * dt);
        }
        let mut next = pos + v * dt;
        // Height: above the planted feet, with a little bob in mid-stride.
        let feet_h: f32 = w
            .feet
            .iter()
            .filter(|f| f.alive)
            .map(|f| f.planted.y)
            .sum::<f32>()
            / alive_legs.max(1) as f32;
        let ground = zone.ground(next.x, next.z);
        let base = if w.collapsed {
            ground + 0.4
        } else {
            feet_h.max(ground - 0.2) + BODY_HEIGHT
        };
        let stepping = w.feet.iter().any(|f| f.t < 1.0);
        let bob = if stepping { 0.04 } else { 0.0 };
        next.y += (base + bob - next.y) * (6.0 * dt).min(1.0);
        t.translation = next;
        // Lean into motion, tip toward a missing leg.
        let lean = Vec3::new(v.z, 0.0, -v.x) * 0.05;
        t.rotation = Quat::from_rotation_y(w.yaw) * Quat::from_scaled_axis(lean);

        // Feet.
        let rot = Quat::from_rotation_y(w.yaw);
        let lookahead = v * 0.35;
        let pair_busy = |w: &Walker, pair: usize| {
            (0..LEGS)
                .filter(|i| i % 2 == pair)
                .any(|i| w.feet[i].t < 1.0)
        };
        for i in 0..LEGS {
            if !w.feet[i].alive {
                continue;
            }
            let ideal_xz = next + rot * rest_offset(i) + lookahead;
            let ideal = Vec3::new(ideal_xz.x, zone.ground(ideal_xz.x, ideal_xz.z), ideal_xz.z);
            let f = w.feet[i];
            if f.t < 1.0 {
                let nt = (f.t + dt / STEP_TIME).min(1.0);
                let foot = &mut w.feet[i];
                foot.t = nt;
                let s = nt * nt * (3.0 - 2.0 * nt);
                foot.planted = foot.from.lerp(foot.to, s)
                    + Vec3::Y * (STEP_HEIGHT * (std::f32::consts::PI * nt).sin());
                if nt >= 1.0 {
                    foot.planted = foot.to;
                }
            } else if f.planted.distance(ideal) > STEP_TRIGGER
                && i % 2 == w.phase
                && !pair_busy(&w, 1 - w.phase)
            {
                let foot = &mut w.feet[i];
                foot.from = foot.planted;
                foot.to = ideal;
                foot.t = 0.0;
            }
        }
        // Alternate pairs once the current pair has landed.
        if !pair_busy(&w, w.phase)
            && w.feet
                .iter()
                .enumerate()
                .any(|(i, f)| i % 2 == w.phase && f.t >= 1.0)
        {
            let other = 1 - w.phase;
            let other_needs = (0..LEGS).any(|i| {
                i % 2 == other && w.feet[i].alive && {
                    let ideal_xz = next + rot * rest_offset(i);
                    w.feet[i].planted.distance(Vec3::new(
                        ideal_xz.x,
                        w.feet[i].planted.y,
                        ideal_xz.z,
                    )) > STEP_TRIGGER * 0.6
                }
            });
            if other_needs {
                w.phase = other;
            }
        }
        // IK.
        let tf = *t;
        for i in 0..LEGS {
            let hip = tf.transform_point(hip_offset(i));
            let foot = if w.feet[i].alive {
                w.feet[i].planted
            } else {
                // A dead leg dangles under its hip.
                hip + rot * (hip_offset(i).normalize_or_zero() * 0.4)
                    - Vec3::Y * (UPPER + LOWER) * 0.8
            };
            let knee = solve_knee(
                hip,
                foot,
                (hip - tf.translation).normalize_or(Vec3::X) + Vec3::Y * 0.8,
            );
            poses.0[i] = [hip, knee, foot];
        }
        // Leg colliders follow the lower segments (local to the root).
        let inv = tf.compute_affine().inverse();
        for c in children.iter() {
            if let Ok((leg, mut lt)) = legs.get_mut(c) {
                let [_, knee, foot] = poses.0[leg.index];
                let mid = (knee + foot) * 0.5;
                let dir = (knee - foot).normalize_or(Vec3::Y);
                let world = Transform::from_translation(mid)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir));
                *lt = Transform::from_matrix((inv * world.compute_affine()).into());
            }
        }
    }
}

/// Two-bone IK: the knee, bent toward `pole`.
pub fn solve_knee(hip: Vec3, foot: Vec3, pole: Vec3) -> Vec3 {
    let d = (foot - hip).length().clamp(0.05, UPPER + LOWER - 0.001);
    let dir = (foot - hip).normalize_or(Vec3::NEG_Y);
    // Distance along hip->foot to the knee's projection, and the knee's offset.
    let a = (UPPER * UPPER - LOWER * LOWER + d * d) / (2.0 * d);
    let h = (UPPER * UPPER - a * a).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Y);
    hip + dir * a + bend * h
}

fn walker_guns(
    mut walkers: Query<(Entity, &mut Walker, &Brain, &Awareness, &Robot, &Children)>,
    turrets: Query<(&GlobalTransform, &RobotPart), With<WalkerTurret>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
    mut shots: MessageWriter<Shot>,
    mut seed: Local<u64>,
) {
    let dt = time.delta_secs();
    for (e, mut w, brain, aware, robot, children) in &mut walkers {
        w.gun_cooldown -= dt;
        if robot.wrecked
            || !matches!(brain, Brain::Engage)
            || !aware.can_see_target
            || w.gun_cooldown > 0.0
        {
            continue;
        }
        let Some(target) = aware
            .target
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() + Vec3::Y * 1.1)
        else {
            continue;
        };
        let Some((gt, part)) = children.iter().find_map(|c| turrets.get(c).ok()) else {
            continue;
        };
        if !part.alive() {
            continue;
        }
        let muzzle = gt.translation() + Vec3::Y * 0.05;
        if muzzle.distance(target) > 70.0 {
            continue;
        }
        *seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        shots.write(Shot {
            gun: e,
            shooter: Some(e),
            origin: muzzle + (target - muzzle).normalize_or_zero() * 0.3,
            direction: (target - muzzle).normalize_or(Vec3::NEG_Z),
            speed: 450.0,
            damage: 14.0,
            projectiles: 1,
            spread: 0.025,
            recoil: 0.0,
            seed: *seed,
        });
        w.gun_cooldown = 1.1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knee_keeps_segment_lengths() {
        let hip = Vec3::new(0.0, 1.0, 0.0);
        let foot = Vec3::new(1.0, 0.0, 0.2);
        let knee = solve_knee(hip, foot, Vec3::Y + Vec3::X);
        assert!((knee.distance(hip) - UPPER).abs() < 1e-3);
        assert!((knee.distance(foot) - LOWER).abs() < 1e-3);
        assert!(knee.y > hip.y - 0.2, "knee bends upward, like a spider's");
    }
}
