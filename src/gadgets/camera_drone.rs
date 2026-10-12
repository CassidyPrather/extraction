//! The camera drone: a palm-sized quadcopter. Use it to spin it up, then
//! let go: it climbs and circles the spot where it left your hand, and any
//! robot it can see is *spotted* — marked through walls for a while. Its
//! camera feed shows on your left wrist (client side). Use it again from
//! the wrist… or simply wait: when its battery runs low it comes home and
//! drops at your feet. It recharges itself between flights.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::interact::body::InBackpack;
use crate::interact::{Held, InSocket};
use crate::items::usage::UseItem;
use crate::robots::Robot;
use crate::world::{Layer, Zone};

/// Seconds of flight on a full battery.
pub const FLIGHT: f32 = 90.0;
/// Seconds to recharge from flat.
pub const RECHARGE: f32 = 150.0;
pub const ORBIT_HEIGHT: f32 = 22.0;
pub const ORBIT_RADIUS: f32 = 16.0;
/// How far it sees robots in clear air (fog and rain cut it).
pub const SPOT_RANGE: f32 = 90.0;
/// How long a robot stays marked after the drone last saw it.
pub const SPOT_TIME: f32 = 40.0;
/// Battery left when it turns for home.
const RESERVE: f32 = 20.0;
const SPEED: f32 = 7.0;

#[derive(Component, Debug)]
pub struct CameraDrone {
    pub flight: Flight,
    /// The centre of its circle: where it was let go.
    pub home: Vec3,
    /// Whoever spun it up; it comes back to them.
    pub owner: Option<Entity>,
    /// Seconds of flight left.
    pub battery: f32,
    /// Seconds aloft this flight.
    pub aloft: f32,
}

impl Default for CameraDrone {
    fn default() -> Self {
        Self {
            flight: Flight::Idle,
            home: Vec3::ZERO,
            owner: None,
            battery: FLIGHT,
            aloft: 0.0,
        }
    }
}

impl CameraDrone {
    pub fn airborne(&self) -> bool {
        matches!(self.flight, Flight::Up | Flight::Returning)
    }

    /// Where it wants to be this tick.
    fn target(&self, zone: &Zone, owner_at: Option<Vec3>) -> Vec3 {
        match (self.flight, owner_at) {
            (Flight::Returning, Some(o)) => o + Vec3::Y * 2.2,
            _ => {
                let a = self.aloft * 0.22;
                let p = self.home + Vec3::new(a.cos(), 0.0, a.sin()) * ORBIT_RADIUS;
                let floor = zone.ground(p.x, p.z) + ORBIT_HEIGHT * 0.7;
                Vec3::new(p.x, (self.home.y + ORBIT_HEIGHT).max(floor), p.z)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flight {
    #[default]
    Idle,
    /// Rotors running in your hand: let go and it flies.
    SpunUp,
    Up,
    Returning,
}

/// A robot the camera drone has seen: seconds the mark lasts.
#[derive(Component, Clone, Copy, Debug)]
pub struct Spotted(pub f32);

pub struct CameraDronePlugin;

impl Plugin for CameraDronePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (launch, fly, spot, fade_spots)
                .chain()
                .after(crate::interact::InteractSystems::Follow),
        );
    }
}

fn launch(
    mut uses: MessageReader<UseItem>,
    mut drones: Query<&mut CameraDrone>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for u in uses.read() {
        let Ok(mut d) = drones.get_mut(u.item) else {
            continue;
        };
        match d.flight {
            Flight::Idle if d.battery < RESERVE + 10.0 => {
                raid.say("The drone's battery is flat. Give it a few minutes.");
            }
            Flight::Idle => {
                d.flight = Flight::SpunUp;
                d.owner = Some(u.player);
                raid.say("The rotors whine up. Let it go.");
            }
            Flight::SpunUp => {
                d.flight = Flight::Idle;
            }
            Flight::Up => {
                d.flight = Flight::Returning;
                raid.say("Camera drone: coming home.");
            }
            Flight::Returning => {}
        }
    }
}

fn fly(
    mut commands: Commands,
    mut drones: Query<(
        Entity,
        &mut CameraDrone,
        &Position,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Has<Held>,
        Has<InSocket>,
        Has<InBackpack>,
    )>,
    players: Query<&GlobalTransform>,
    zone: Res<Zone>,
    time: Res<Time>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    let dt = time.delta_secs();
    for (e, mut d, pos, mut lin, mut ang, held, socketed, stowed) in &mut drones {
        let caught = held || socketed || stowed;
        match d.flight {
            Flight::Idle => {
                d.battery = (d.battery + dt * FLIGHT / RECHARGE).min(FLIGHT);
                continue;
            }
            Flight::SpunUp => {
                if stowed || socketed {
                    d.flight = Flight::Idle;
                } else if !held {
                    d.flight = Flight::Up;
                    d.home = pos.0;
                    d.aloft = 0.0;
                    commands.entity(e).insert(GravityScale(0.0));
                }
                continue;
            }
            Flight::Up | Flight::Returning => {}
        }
        if caught {
            d.flight = Flight::Idle;
            commands.entity(e).remove::<GravityScale>();
            continue;
        }
        d.battery -= dt;
        d.aloft += dt;
        if d.battery <= 0.0 {
            d.flight = Flight::Idle;
            commands.entity(e).remove::<GravityScale>();
            raid.say("Camera drone: battery dead. It falls out of the sky.");
            continue;
        }
        if d.flight == Flight::Up && d.battery < RESERVE {
            d.flight = Flight::Returning;
            raid.say("Camera drone: battery low, coming home.");
        }
        let owner_at = d
            .owner
            .and_then(|o| players.get(o).ok())
            .map(|g| g.translation());
        if d.flight == Flight::Returning {
            let Some(o) = owner_at else {
                // Nobody to come home to: it lands where it is.
                d.flight = Flight::Idle;
                commands.entity(e).remove::<GravityScale>();
                continue;
            };
            let flat = Vec2::new(pos.0.x - o.x, pos.0.z - o.z).length();
            if flat < 1.2 && (pos.0.y - o.y).abs() < 3.5 {
                // Over your head: the rotors stop and it drops to you.
                d.flight = Flight::Idle;
                commands.entity(e).remove::<GravityScale>();
                lin.0 = Vec3::ZERO;
                continue;
            }
        }
        let to = d.target(&zone, owner_at) - pos.0;
        let speed = (to.length() * 1.2).min(SPEED);
        lin.0 = to.normalize_or_zero() * speed;
        ang.0 = Vec3::Y * 2.0;
    }
}

/// Every half second, an airborne drone marks the robots it can see.
fn spot(
    mut commands: Commands,
    drones: Query<(&CameraDrone, &GlobalTransform)>,
    robots: Query<(Entity, &Robot, &GlobalTransform, Option<&Spotted>)>,
    spatial: SpatialQuery,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    time: Res<Time>,
    mut timer: Local<f32>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.5;
    let range = conditions.map_or(SPOT_RANGE, |c| c.sight(SPOT_RANGE));
    let mut fresh = 0;
    for (d, gt) in &drones {
        if !d.airborne() {
            continue;
        }
        let eye = gt.translation() - Vec3::Y * 0.1;
        for (r, robot, rgt, spotted) in &robots {
            if robot.wrecked {
                continue;
            }
            let at = rgt.translation() + Vec3::Y * 0.5;
            let to = at - eye;
            let dist = to.length();
            if dist > range {
                continue;
            }
            let blocked = Dir3::new(to).ok().and_then(|dir| {
                spatial.cast_ray(
                    eye,
                    dir,
                    dist - 0.5,
                    true,
                    &SpatialQueryFilter::from_mask([Layer::World]),
                )
            });
            if blocked.is_some() {
                continue;
            }
            if spotted.is_none() {
                fresh += 1;
            }
            commands.entity(r).insert(Spotted(SPOT_TIME));
        }
    }
    if fresh > 0 {
        raid.say(if fresh == 1 {
            "Camera drone: one robot spotted.".to_string()
        } else {
            format!("Camera drone: {fresh} robots spotted.")
        });
    }
}

fn fade_spots(mut commands: Commands, mut spots: Query<(Entity, &mut Spotted)>, time: Res<Time>) {
    for (e, mut s) in &mut spots {
        s.0 -= time.delta_secs();
        if s.0 <= 0.0 {
            commands.entity(e).remove::<Spotted>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drone_spots_a_robot_and_comes_home() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let zone = app.world().resource::<Zone>().clone();
        // Out in the field south of the village, away from the truck.
        let at = zone.on_ground(100.0, 200.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at;
        app.update();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let (drone, walker) = {
            let mut c = Commands::new(&mut queue, app.world());
            let drone = crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind: catalog.kind("camera_drone").unwrap(),
                    transform: Transform::from_translation(at + Vec3::Y * 1.2),
                    condition: 1.0,
                },
            );
            let site = zone.on_ground(at.x + 30.0, at.z - 20.0);
            let walker = crate::robots::walker::spawn_walker_with(
                &mut c,
                &zone,
                site,
                crate::robots::Job {
                    kind: crate::robots::JobKind::Response,
                    site,
                    duration: 999.0,
                    exit: site,
                },
                false,
                100.0,
            );
            (drone, walker)
        };
        queue.apply(app.world_mut());
        app.update();
        app.world_mut().write_message(UseItem {
            player,
            item: drone,
        });
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world().get::<CameraDrone>(drone).unwrap().flight,
            Flight::Up,
            "let go after spinning up, it flies"
        );
        let mut spotted = false;
        let mut highest = 0.0f32;
        for _ in 0..90 * 12 {
            app.update();
            highest = highest.max(app.world().get::<Position>(drone).unwrap().0.y - at.y);
            spotted |= app.world().get::<Spotted>(walker).is_some();
        }
        assert!(highest > 12.0, "it climbed ({highest:.1} m)");
        assert!(spotted, "it saw the walker");
        app.world_mut().write_message(UseItem {
            player,
            item: drone,
        });
        let mut home = false;
        for _ in 0..90 * 20 {
            app.update();
            let d = app.world().get::<CameraDrone>(drone).unwrap();
            if d.flight == Flight::Idle {
                home = true;
                break;
            }
        }
        assert!(home, "it came home");
        let p = app.world().get::<Position>(drone).unwrap().0;
        assert!(
            Vec2::new(p.x - at.x, p.z - at.z).length() < 3.0,
            "and stopped over the player ({p:.1} vs {at:.1})"
        );
    }
}
