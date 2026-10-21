//! How drones fly: a position controller asks for an acceleration, an
//! attitude controller turns that into a torque, and a mixer splits thrust
//! and torque across the rotors, each pushing at its own position. Banking
//! into a turn is the physics: an orbit needs a sideways acceleration, and
//! the only way to get one is to tip the thrust.
//!
//! On top of that, what makes it read as a machine and not a marker: it
//! turns its nose toward where it's going before it gets there; it never
//! hangs perfectly still (a slow hunting drift round its station); a hit
//! is an impulse at the point struck, so it rocks and skids and recovers;
//! and one dead rotor doesn't drop it like a stone. The other three still
//! hold its tilt (three rotors, three unknowns: thrust, roll and pitch),
//! but nothing holds its yaw or its height: it lists toward the gap, turns
//! slowly round and sinks. Two dead and it falls.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::drone::{ARM, Drone, MASS, MAX_ROTOR_N, ROTORS};
use super::{Awareness, Intent, Robot, RobotPart};
use crate::world::Zone;

/// The body's own state, beside the brain's [`Drone`].
#[derive(Component, Debug, Clone, Default)]
pub struct Flight {
    /// The acceleration it's asking for, world space (visuals tilt the
    /// rotors with it).
    pub accel: Vec3,
    /// Seconds flown, for the hunting drift.
    pub age: f32,
    /// Ice on the rotors, 0..1: it builds over a minute or so in the cold
    /// (more in falling snow) and goes once it's warmer. It costs lift.
    pub frost: f32,
}

/// What full frost costs a rotor of its top thrust: still enough to hover
/// (thrust to weight 2.2 down to about 1.2), not to climb as it did.
pub const FROST_COST: f32 = 0.45;

pub struct FlightPlugin;

impl Plugin for FlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                knock_drones
                    .after(crate::weapons::ballistics::BallisticsSystems)
                    .before(PhysicsSystems::First),
                sway_dirigibles
                    .after(super::dirigible::fly_dirigibles)
                    .before(PhysicsSystems::First),
            ),
        );
    }
}

/// Splits total thrust (N) and body-frame roll/pitch torque (N·m) over the
/// living rotors. With four, the X-quad allocation; with three, the exact
/// solution (yaw is lost); with fewer, each does what it would with four.
pub fn mix(thrust: f32, torque_body: Vec3, alive: [bool; 4]) -> [f32; 4] {
    let n = alive.iter().filter(|a| **a).count();
    let mut f = [0.0f32; 4];
    if n == 3 {
        let idx: Vec<usize> = (0..4).filter(|i| alive[*i]).collect();
        let col = |i: usize| Vec3::new(1.0, -ROTORS[i].z, ROTORS[i].x);
        let m = Mat3::from_cols(col(idx[0]), col(idx[1]), col(idx[2]));
        if m.determinant().abs() > 1e-6 {
            let sol = m.inverse() * Vec3::new(thrust, torque_body.x, torque_body.z);
            for (k, i) in idx.iter().enumerate() {
                f[*i] = sol[k].clamp(0.0, MAX_ROTOR_N);
            }
            return f;
        }
    }
    let a2 = 4.0 * ARM * ARM;
    for (i, r) in ROTORS.iter().enumerate() {
        let fi = thrust / 4.0 - torque_body.x * r.z / a2 + torque_body.z * r.x / a2;
        f[i] = if alive[i] {
            fi.clamp(0.0, MAX_ROTOR_N)
        } else {
            0.0
        };
    }
    f
}

/// Position -> acceleration -> attitude -> per-rotor thrust.
#[allow(clippy::type_complexity)]
pub(crate) fn fly_drones(
    mut drones: Query<(
        &mut Drone,
        &mut Flight,
        &Robot,
        &Transform,
        &Awareness,
        Option<&Intent>,
        Has<super::Stunned>,
        Forces,
    )>,
    targets: Query<&GlobalTransform>,
    zone: Res<Zone>,
    snow: Option<Res<crate::world::snow::Snow>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let g = Vec3::new(0.0, -9.81, 0.0);
    for (mut d, mut fl, robot, t, aware, intent, stunned, mut forces) in &mut drones {
        let pos = t.translation;
        let icing = snow
            .as_ref()
            .filter(|s| s.cold(pos.y))
            .map_or(0.0, |s| 0.6 + 0.4 * s.falling);
        fl.frost += (icing - fl.frost) * (1.0 - (-dt / 45.0).exp());
        let lin = LinearVelocity(forces.linear_velocity());
        let ang = AngularVelocity(forces.angular_velocity());
        let near_ground = pos.y - zone.ground(pos.x, pos.z) < 0.6;
        let alive = d.rotor_alive.iter().filter(|a| **a).count();
        if robot.wrecked || stunned || alive < 4 && near_ground {
            d.thrust = [0.0; 4];
            fl.accel = Vec3::ZERO;
            if near_ground {
                d.grounded += dt;
            }
            continue;
        }
        fl.age += dt;
        // Never quite still: a slow drift round its station, different
        // for every drone.
        let k = fl.age + (d.phase * 7.0).sin() * 3.0;
        let drift = Vec3::new(
            (k * 0.53).sin() * 0.25,
            (k * 0.71).sin() * 0.12,
            (k * 0.37).cos() * 0.25,
        );
        // Position controller.
        let err = d.goal + drift - pos;
        let mut acc = err * 1.2 - lin.0 * 1.6;
        let pace = intent.map_or(1.0, |i| i.pace);
        let horiz = Vec2::new(acc.x, acc.z).clamp_length_max(5.0 * pace.max(0.3));
        acc = Vec3::new(horiz.x, acc.y.clamp(-4.0, 5.0), horiz.y);
        // Don't scrape the ground.
        if pos.y - zone.ground(pos.x, pos.z) < 2.5 {
            acc.y = acc.y.max(3.0);
        }
        if alive == 3 {
            // Three rotors can't hold it up: it sinks, whatever it wants.
            acc.y = acc.y.min(-1.2);
        }
        fl.accel = acc;
        let f_world = (acc - g) * MASS;
        // Desired up, tilted at most ~35 degrees.
        let mut up_des = f_world.normalize_or(Vec3::Y);
        if alive == 3 {
            // It lists toward the dead rotor.
            let dead = (0..4).find(|i| !d.rotor_alive[*i]).unwrap_or(0);
            let toward = t.rotation * Vec3::new(ROTORS[dead].x, 0.0, ROTORS[dead].z);
            up_des = (up_des + toward.normalize_or_zero() * 0.3).normalize();
        }
        let tilt = up_des.angle_between(Vec3::Y);
        if tilt > 0.6 {
            let axis = Vec3::Y.cross(up_des).normalize_or(Vec3::X);
            up_des = Quat::from_axis_angle(axis, 0.6) * Vec3::Y;
        }
        let up = t.rotation * Vec3::Y;
        let thrust_total = f_world.dot(up).max(0.0);
        // Attitude: tilt toward up_des, yaw toward the target it sees, what
        // its brain says to look at, or (leading with its nose) where it's
        // headed.
        let tilt_axis = up.cross(up_des);
        let tilt_err = tilt_axis.normalize_or_zero() * up.angle_between(up_des);
        let headed = Vec3::new(err.x, 0.0, err.z);
        let face = aware
            .target
            .filter(|_| aware.can_see_target)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation() - pos)
            .or_else(|| intent.and_then(|i| i.look).map(|l| l - pos))
            .unwrap_or(if headed.length() > 2.0 { headed } else { lin.0 });
        let fwd = t.rotation * Vec3::NEG_Z;
        let face_flat = Vec3::new(face.x, 0.0, face.z);
        let yaw_err = if face_flat.length() > 0.5 {
            let f = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
            let to = face_flat.normalize();
            // Signed angle, full turn when the target is behind (an arcsine
            // of the cross product reads zero there, and it never turned).
            f.cross(to).y.atan2(f.dot(to))
        } else {
            0.0
        };
        let torque_world = tilt_err * 9.0 - ang.0 * 2.2 + Vec3::Y * yaw_err * 2.0;
        let torque_body = t.rotation.inverse() * torque_world;
        let mut f = mix(thrust_total, torque_body, d.rotor_alive);
        let cap = MAX_ROTOR_N * (1.0 - FROST_COST * fl.frost);
        for fi in &mut f {
            *fi = fi.min(cap);
        }
        d.thrust = f;
        for (i, r) in ROTORS.iter().enumerate() {
            if f[i] > 0.0 {
                let at = t.transform_point(*r);
                forces.apply_force_at_point(up * f[i], at);
            }
        }
        if alive == 4 {
            // Yaw comes from rotor drag; apply it directly.
            forces.apply_torque(Vec3::Y * torque_world.y.clamp(-3.0, 3.0));
        } else {
            // Unbalanced drag turns it, slowly, whatever it wants.
            let dead = (0..4).find(|i| !d.rotor_alive[*i]).unwrap_or(0);
            let spin = if dead % 2 == 0 { 0.5 } else { -0.5 };
            forces.apply_torque(up * spin);
        }
        d.grounded = 0.0;
    }
}

/// An airship's attitude, on top of the heading it flies.
#[derive(Component, Debug, Default)]
pub struct Sway {
    pitch: f32,
    roll: f32,
    last_yaw: f32,
    yaw_rate: f32,
    age: f32,
}

/// Airships pitch with their climb, roll into turns, wallow a little in
/// the wind, and list toward a punctured cell as they sink (nose down if
/// it's the front one).
fn sway_dirigibles(
    mut commands: Commands,
    mut ships: Query<(
        Entity,
        &super::dirigible::Dirigible,
        &mut Transform,
        Option<&mut Sway>,
        Option<&super::RobotSeed>,
    )>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, d, mut t, sway, seed) in &mut ships {
        let Some(mut s) = sway else {
            commands.entity(e).insert(Sway {
                last_yaw: d.yaw,
                ..default()
            });
            continue;
        };
        s.age += dt;
        let mut dy = d.yaw - s.last_yaw;
        if dy > std::f32::consts::PI {
            dy -= std::f32::consts::TAU;
        } else if dy < -std::f32::consts::PI {
            dy += std::f32::consts::TAU;
        }
        s.last_yaw = d.yaw;
        s.yaw_rate += (dy / dt - s.yaw_rate) * (1.0 * dt).min(1.0);
        let speed = Vec2::new(d.velocity.x, d.velocity.z).length();
        // (Each ship's sway on its own phase.)
        let k = s.age + seed.map_or(0, |s| s.low() % 1000) as f32;
        let mut pitch = (d.velocity.y * 0.05).clamp(-0.1, 0.1) + 0.012 * (k * 0.23).sin();
        // Banking: the outside of the turn rises (a negative yaw rate turns
        // right, rolling it right).
        let mut roll = (s.yaw_rate * speed * 0.5).clamp(-0.08, 0.08) + 0.018 * (k * 0.31).sin();
        if d.sinking {
            for (i, alive) in d.cells.iter().enumerate() {
                if !alive {
                    pitch += (i as f32 - 1.0) * 0.16;
                    roll += 0.05;
                }
            }
        }
        // Heavy and slow to answer.
        let rate = (0.6 * dt).min(1.0);
        s.pitch += (pitch - s.pitch) * rate;
        s.roll += (roll - s.roll) * rate;
        t.rotation = Quat::from_rotation_y(d.yaw)
            * Quat::from_rotation_x(s.pitch)
            * Quat::from_rotation_z(s.roll);
    }
}

/// A hit is an impulse where it struck: the drone rocks and skids.
fn knock_drones(
    mut hits: MessageReader<crate::weapons::ballistics::Hit>,
    parts: Query<&RobotPart>,
    mut drones: Query<Forces, With<Drone>>,
) {
    for hit in hits.read() {
        let Ok(part) = parts.get(hit.entity) else {
            continue;
        };
        let Ok(mut forces) = drones.get_mut(part.robot) else {
            continue;
        };
        let impulse = hit.velocity.normalize_or_zero() * (hit.damage * 0.035).min(1.5);
        forces.apply_linear_impulse_at_point(impulse, hit.point);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moments(f: [f32; 4]) -> (f32, f32, f32) {
        let mut out = (0.0, 0.0, 0.0);
        for (i, r) in ROTORS.iter().enumerate() {
            out.0 += f[i];
            out.1 += -r.z * f[i];
            out.2 += r.x * f[i];
        }
        out
    }

    /// Four rotors or three, the mixer delivers the thrust and the roll and
    /// pitch asked of it (within what the rotors can push: on three, the
    /// rotor across from the gap idles at a hover, so only some torques
    /// are there to be had).
    #[test]
    fn mixer_holds_attitude_on_three_rotors() {
        let hover = MASS * 9.81;
        for alive in [
            [true; 4],
            [false, true, true, true],
            [true, true, false, true],
        ] {
            let torques = if alive == [true; 4] {
                vec![Vec3::ZERO, Vec3::new(0.4, 0.0, -0.3)]
            } else {
                vec![Vec3::ZERO]
            };
            for torque in torques {
                let f = mix(hover, torque, alive);
                assert!(f.iter().all(|v| v.is_finite() && *v >= 0.0));
                for i in 0..4 {
                    if !alive[i] {
                        assert_eq!(f[i], 0.0);
                    }
                }
                let (t, x, z) = moments(f);
                assert!((t - hover).abs() < 0.05, "{alive:?}: thrust {t}");
                assert!((x - torque.x).abs() < 0.05, "{alive:?}: roll {x}");
                assert!((z - torque.z).abs() < 0.05, "{alive:?}: pitch {z}");
            }
        }
    }

    /// How far a drone climbs in 3 s toward a goal 25 m up, warm or in
    /// the cold with its rotors iced.
    fn climb(iced: bool) -> f32 {
        // Iced: up on Rime Fell's col, above its snow line, in the cold.
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                map: if iced { "rime_fell" } else { "training_yard" }.into(),
                map_explicit: true,
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let site = if iced {
            zone.on_ground(70.0, -262.0)
        } else {
            zone.on_ground(40.0, 200.0)
        } + Vec3::Y * 8.0;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::drone::spawn_drone(
                &mut c,
                site,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
            c.entity(e)
                .remove::<super::super::Senses>()
                .remove::<super::super::Brain>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..90 {
            app.update();
        }
        if iced {
            // (A minute's icing, rather than waiting it out.)
            assert!(
                app.world()
                    .resource::<crate::world::snow::Snow>()
                    .cold(site.y),
                "it's cold up here"
            );
            app.world_mut().get_mut::<Flight>(drone).unwrap().frost = 1.0;
        }
        let y0 = app.world().get::<Transform>(drone).unwrap().translation.y;
        app.world_mut().get_mut::<Drone>(drone).unwrap().goal = site + Vec3::Y * 25.0;
        for _ in 0..(90 * 3) {
            app.update();
        }
        app.world().get::<Transform>(drone).unwrap().translation.y - y0
    }

    /// Iced rotors in the cold cost a drone its climb (it still holds
    /// itself up).
    #[test]
    fn frosted_rotors_climb_slower() {
        let warm = climb(false);
        let cold = climb(true);
        assert!(warm > 3.0, "it climbs ({warm:.1} m)");
        assert!(cold > 0.5, "iced, it still rises ({cold:.1} m)");
        assert!(cold < warm * 0.85, "iced {cold:.1} m vs warm {warm:.1} m");
    }

    /// One rotor shot off: it sinks, listing and turning, rather than
    /// dropping, and comes down somewhere near where it was hit.
    #[test]
    fn a_drone_with_a_dead_rotor_sinks_rather_than_falls() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let site = zone.on_ground(40.0, 200.0) + Vec3::Y * 12.0;
        let drone = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let e = super::super::drone::spawn_drone(
                &mut c,
                site,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site,
                    duration: 999.0,
                    exit: site,
                },
            );
            c.entity(e).remove::<super::super::Senses>();
            q.apply(app.world_mut());
            e
        };
        for _ in 0..90 {
            app.update();
        }
        app.world_mut().get_mut::<Drone>(drone).unwrap().rotor_alive[1] = false;
        let start = app.world().get::<Transform>(drone).unwrap().translation;
        let mut worst_tilt = 0.0f32;
        let mut landed_after = None;
        for tick in 0..90 * 12 {
            app.update();
            let t = app.world().get::<Transform>(drone).unwrap();
            assert!(t.translation.is_finite() && t.rotation.is_finite());
            worst_tilt = worst_tilt.max((t.rotation * Vec3::Y).angle_between(Vec3::Y));
            let over = t.translation.y - zone.ground(t.translation.x, t.translation.z);
            if over < 0.8 {
                landed_after = Some(tick as f32 / 90.0);
                break;
            }
        }
        let fell = start.y - zone.ground(start.x, start.z);
        let secs = landed_after.expect("it came down");
        assert!(
            secs > 2.5,
            "it sank over {secs:.1} s from {fell:.1} m, not dropped"
        );
        assert!(
            worst_tilt < 1.0,
            "it listed ({worst_tilt:.2} rad), not tumbled"
        );
    }
}
