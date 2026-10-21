//! The grappler: a walker body with a harpoon line instead of a gun. On a
//! logistics job grapplers stand guard round a dirigible's cargo pod. They
//! don't shoot: they spear you with a tether and reel, so you're slow and
//! dragged toward them while their friends come: the line sings as it
//! bites ([`REEL_NOISE`]), a noise at you that robots round about hear.

use bevy::prelude::*;

use super::walker::spawn_walker_with;
use super::{Awareness, Brain, Job, PartRole, Robot, RobotPart};
use crate::player::Player;
use crate::world::Zone;

#[derive(Component, Debug, Default)]
pub struct Grappler {
    pub cooldown: f32,
    /// Seconds into winding up a throw (0: not winding up).
    pub windup: f32,
}

/// A grappler winds up this long before it throws: it rears back, and
/// [`HarpoonWindup`] says so (the tell).
pub const WINDUP: f32 = 0.6;
/// A line that bites sings this far (m): its friends come.
pub const REEL_NOISE: f32 = 60.0;

/// A grappler began winding up a throw at `to` (for its tell).
#[derive(Message, Clone, Copy, Debug)]
pub struct HarpoonWindup {
    pub from: Entity,
    pub to: Entity,
}

/// A player with a harpoon line in them: slow, and pulled toward `by`.
#[derive(Component, Debug, Clone, Copy)]
pub struct Tethered {
    pub by: Entity,
    pub left: f32,
}

/// A harpoon was fired (for visuals and sound).
#[derive(Message, Clone, Copy, Debug)]
pub struct Harpoon {
    pub from: Entity,
    pub to: Entity,
    pub hit: bool,
}

pub const TETHER_RANGE: f32 = 22.0;
pub const TETHER_TIME: f32 = 4.0;
/// Walk speed while tethered.
pub const TETHER_SLOW: f32 = 0.35;
/// How hard the line drags you, m/s.
pub const TETHER_PULL: f32 = 1.6;

pub struct GrapplerPlugin;

impl Plugin for GrapplerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Harpoon>()
            .add_message::<HarpoonWindup>()
            .add_systems(
                FixedUpdate,
                (harpoons, tethers).chain().after(super::perceive),
            );
    }
}

pub fn spawn_grappler(commands: &mut Commands, zone: &Zone, at: Vec3, job: Job) -> Entity {
    let e = spawn_walker_with(commands, zone, at, job, false, 110.0);
    commands.entity(e).insert((
        Name::new("Grappler"),
        Grappler {
            cooldown: 3.0,
            windup: 0.0,
        },
    ));
    e
}

fn harpoons(
    mut commands: Commands,
    mut grapplers: Query<(
        Entity,
        &mut Grappler,
        &Brain,
        &Awareness,
        &Robot,
        &GlobalTransform,
        &Children,
    )>,
    sensors: Query<&RobotPart>,
    players: Query<&GlobalTransform, With<Player>>,
    time: Res<Time>,
    mut shots: MessageWriter<Harpoon>,
    mut windups: MessageWriter<HarpoonWindup>,
    stunned: Query<(), With<super::Stunned>>,
    mut noise: MessageWriter<super::Noise>,
    mut seed: Local<u64>,
    minds: Query<&super::senses::MindSeed>,
) {
    let dt = time.delta_secs();
    for (e, mut g, brain, aware, robot, gt, children) in &mut grapplers {
        g.cooldown -= dt;
        if robot.wrecked
            || stunned.contains(e)
            || !matches!(brain, Brain::Engage)
            || !aware.can_see_target
            || g.cooldown > 0.0
        {
            g.windup = 0.0;
            continue;
        }
        // Needs its eye to aim.
        let eye = children.iter().any(|c| {
            sensors
                .get(c)
                .is_ok_and(|p| p.role == PartRole::Sensor && p.alive())
        });
        let Some(target) = aware.target else {
            continue;
        };
        let Ok(pt) = players.get(target) else {
            continue;
        };
        if !eye || pt.translation().distance(gt.translation()) > TETHER_RANGE {
            g.windup = 0.0;
            continue;
        }
        // Tell first: it rears back, then throws.
        if g.windup < WINDUP {
            if g.windup == 0.0 {
                windups.write(HarpoonWindup {
                    from: e,
                    to: target,
                });
            }
            g.windup += dt;
            continue;
        }
        g.windup = 0.0;
        g.cooldown = 9.0;
        // Most throws land; some don't.
        *seed = super::senses::shot_seed(&minds, e, &time);
        let hit = (*seed >> 33) % 100 < 70;
        if hit {
            commands.entity(target).insert(Tethered {
                by: e,
                left: TETHER_TIME,
            });
            // The line sings as it bites: the others hear where you are.
            if let Ok(at) = players.get(target) {
                noise.write(super::Noise {
                    at: at.translation(),
                    radius: REEL_NOISE,
                    source: Some(target),
                });
            }
        }
        shots.write(Harpoon {
            from: e,
            to: target,
            hit,
        });
    }
}

/// Lines wear off, snap when their robot goes down, and drag.
fn tethers(
    mut commands: Commands,
    mut players: Query<(
        Entity,
        &mut Tethered,
        &GlobalTransform,
        &mut crate::player::Body,
    )>,
    robots: Query<(&Robot, &GlobalTransform)>,
    stunned: Query<(), With<super::Stunned>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut t, gt, mut body) in &mut players {
        t.left -= dt;
        let anchor = robots
            .get(t.by)
            .ok()
            .filter(|(r, _)| !r.wrecked && !stunned.contains(t.by));
        let Some((_, rgt)) = anchor.filter(|_| t.left > 0.0) else {
            commands.entity(e).remove::<Tethered>();
            continue;
        };
        let to = rgt.translation() - gt.translation();
        let flat = Vec3::new(to.x, 0.0, to.z);
        if flat.length() > 2.0 {
            let pull = flat.normalize() * TETHER_PULL;
            body.velocity.x = body.velocity.x * TETHER_SLOW + pull.x;
            body.velocity.z = body.velocity.z * TETHER_SLOW + pull.z;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grappler_spears_a_player_in_reach() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let (player, at) = app
            .world_mut()
            .query_filtered::<(Entity, &GlobalTransform), With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .map(|(e, g)| (e, g.translation()))
            .next()
            .unwrap();
        let zone = app.world().resource::<Zone>().clone();
        // Out in the open, not in the truck's canvas.
        let _ = at;
        let at = zone.on_ground(40.0, 255.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at;
        app.update();
        let spot = zone.on_ground(at.x + 12.0, at.z);
        {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            spawn_grappler(
                &mut c,
                &zone,
                spot,
                super::super::Job {
                    kind: super::super::JobKind::Response,
                    site: at,
                    duration: 999.0,
                    exit: spot,
                },
            );
            q.apply(app.world_mut());
        }
        #[derive(Resource, Default)]
        struct Sung(bool);
        app.init_resource::<Sung>().add_systems(
            Last,
            |mut n: MessageReader<super::super::Noise>, mut s: ResMut<Sung>| {
                s.0 |= n.read().any(|n| n.radius == REEL_NOISE);
            },
        );
        let mut speared = false;
        for _ in 0..90 * 20 {
            app.update();
            if app.world().get::<Tethered>(player).is_some() {
                speared = true;
                break;
            }
        }
        assert!(speared, "the grappler got a line into the player");
        app.update();
        assert!(app.world().resource::<Sung>().0, "and the line sang");
    }
}
