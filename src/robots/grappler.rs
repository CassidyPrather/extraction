//! The grappler: a walker body with a harpoon line instead of a gun. On a
//! logistics job grapplers stand guard round a dirigible's cargo pod. They
//! don't shoot: they spear you with a tether and reel, so you're slow and
//! dragged toward them while their friends come: the line sings as it
//! bites ([`REEL_NOISE`]), a noise at you that robots round about hear,
//! and it calls them (its "calling" voice, `search::Mind::Calling`).
//!
//! **One grappler's line at a time.** The first to spear you has you
//! ([`CLAIM`] after each throw); the others close in, and throw only if
//! it's down, stunned or gone. Two that took turns pulled you back and
//! forth, with no goal you could read (playtest, 28 October, 1:03:28).

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
/// A grappler that's thrown at you has you this long (s): a throw's
/// cooldown and a little; another grappler waits it out.
pub const CLAIM: f32 = 12.0;

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
/// With its line on you, a grappler walks back to the nearest of its armed
/// friends within this (m), hauling you in to them: what it's for, to read
/// (playtest, 28 October, 1:03:34: "what are you gonna do when I get
/// close?"). Within [`HAUL_NEAR`] of one, it holds.
pub const HAUL_FROM: f32 = 45.0;
pub const HAUL_NEAR: f32 = 5.0;

pub struct GrapplerPlugin;

impl Plugin for GrapplerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Harpoon>()
            .add_message::<HarpoonWindup>()
            .add_systems(
                FixedUpdate,
                (harpoons, tethers).chain().after(super::perceive),
            )
            .add_systems(
                FixedUpdate,
                // (Only before the walkers think: tying it after `perceive`
                // as well put every walker's thinking after its seeing, and
                // walkers saw a tick sooner than their tests hold them to.)
                haul_to_friends.before(super::walker::walker_brain),
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
    (mut claims, mut voice, mut log): (
        Local<std::collections::HashMap<Entity, (Entity, f32)>>,
        MessageWriter<super::search::MindShift>,
        ResMut<super::search::MindLog>,
    ),
) {
    let now = time.elapsed_secs();
    // Whose you are: a claim lapses with its time, or its grappler.
    let alive: std::collections::HashSet<Entity> = grapplers
        .iter()
        .filter(|(e, _, _, _, r, ..)| !r.wrecked && !stunned.contains(*e))
        .map(|(e, ..)| e)
        .collect();
    claims.retain(|_, (by, until)| *until > now && alive.contains(by));
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
        // Another grappler has them: leave them to it.
        if claims.get(&target).is_some_and(|(by, _)| *by != e) {
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
        claims.insert(target, (e, now + CLAIM));
        // Most throws land; some don't.
        *seed = super::senses::shot_seed(&minds, e, &time);
        let hit = (*seed >> 33) % 100 < 70;
        if hit {
            commands.entity(target).insert(Tethered {
                by: e,
                left: TETHER_TIME,
            });
            // It has you: it calls the others in.
            log.shift(
                &mut voice,
                e,
                super::search::Mind::Calling,
                gt.translation(),
                time.elapsed_secs(),
            );
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

/// A grappler with its line on someone makes for its nearest armed friend
/// (a walker with a gun, not another grappler), hauling them along: it
/// holds you for the guns. None near, it stands and reels.
fn haul_to_friends(
    mut commands: Commands,
    mut grapplers: Query<
        (Entity, &GlobalTransform, &mut Brain, &Robot, Has<Hauling>),
        With<Grappler>,
    >,
    held: Query<&Tethered>,
    friends: Query<
        (Entity, &GlobalTransform, &Robot),
        (With<super::walker::Walker>, Without<Grappler>),
    >,
    parts: Query<&super::RobotPart>,
) {
    for (e, gt, mut brain, robot, was) in &mut grapplers {
        if robot.wrecked || !held.iter().any(|t| t.by == e) {
            if was {
                commands.entity(e).remove::<Hauling>();
            }
            continue;
        }
        let here = gt.translation();
        // Armed: a gun on it still working.
        let armed = |f: Entity| {
            parts
                .iter()
                .any(|p| p.robot == f && p.role == super::PartRole::Weapon && p.alive())
        };
        let friend = friends
            .iter()
            .filter(|(f, _, r)| !r.wrecked && armed(*f))
            .map(|(_, g, _)| g.translation())
            .filter(|p| p.distance(here) < HAUL_FROM)
            .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
        match friend.filter(|to| to.distance(here) > HAUL_NEAR) {
            Some(to) => {
                *brain = Brain::Transit { to };
                if !was {
                    commands.entity(e).insert(Hauling);
                }
            }
            None if was => {
                commands.entity(e).remove::<Hauling>();
            }
            None => {}
        }
    }
}

/// A grappler hauling someone in to its friends (`haul_to_friends`): its
/// brain goes where the haul says, not where its eyes would take it.
#[derive(Component, Debug)]
pub struct Hauling;

#[cfg(test)]
mod tests {
    use super::*;

    /// With its line on you, a grappler walks back to its armed friend,
    /// hauling you in to the gun: its goal, to read (playtest, 28 October,
    /// 1:03:34, "what are you gonna do when I get close?": it reeled you in
    /// to itself and did nothing).
    #[test]
    fn a_grappler_hauls_you_to_its_friends_gun() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
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
        let at = zone.on_ground(40.0, 255.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = at;
        app.update();
        let spot = zone.on_ground(at.x + 12.0, at.z);
        let gun_at = zone.on_ground(at.x + 27.0, at.z);
        let job = |site: Vec3, exit: Vec3| super::super::Job {
            kind: super::super::JobKind::Response,
            site,
            duration: 999.0,
            exit,
        };
        let (grappler, friend) = {
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
            let mut c = q.get_mut(app.world_mut()).unwrap();
            let g = spawn_grappler(&mut c, &zone, spot, job(at, spot));
            let f = super::super::walker::spawn_walker(&mut c, &zone, gun_at, job(gun_at, gun_at));
            // It stands where it's put: the gun the grappler's for.
            c.entity(f)
                .remove::<Brain>()
                .remove::<super::super::Senses>();
            q.apply(app.world_mut());
            (g, f)
        };
        let gap = |app: &App| {
            let g = app
                .world()
                .get::<GlobalTransform>(grappler)
                .unwrap()
                .translation();
            let f = app
                .world()
                .get::<GlobalTransform>(friend)
                .unwrap()
                .translation();
            g.xz().distance(f.xz())
        };
        let mut bit = None;
        for _ in 0..90 * 20 {
            app.update();
            if app.world().get::<Tethered>(player).is_some() {
                bit = Some(gap(&app));
                break;
            }
        }
        let start = bit.expect("its line bit");
        let mut least = start;
        for _ in 0..(90.0 * (TETHER_TIME - 0.5)) as usize {
            app.update();
            if app.world().get::<Tethered>(player).is_none() {
                break;
            }
            least = least.min(gap(&app));
        }
        assert!(
            least < start - 1.0,
            "it didn't make for its friend's gun: {start:.1} m off when its line bit, {least:.1} at nearest"
        );
    }

    #[test]
    fn a_grappler_spears_a_player_in_reach() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
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

    /// Two grapplers either side of you, both in reach: one line, from one
    /// of them, the whole while (they took turns, and pulled you back and
    /// forth); and when it bites, it calls the others.
    #[test]
    fn two_grapplers_dont_pull_you_back_and_forth() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: extraction_core::map::TEST_FIELD_ID.into(),
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
        let at = zone.on_ground(40.0, 255.0);
        #[derive(Resource, Default)]
        struct Called(usize);
        app.init_resource::<Called>().add_systems(
            Last,
            |mut m: MessageReader<super::super::search::MindShift>, mut c: ResMut<Called>| {
                c.0 += m
                    .read()
                    .filter(|m| m.state == super::super::search::Mind::Calling)
                    .count();
            },
        );
        for side in [-1.0, 1.0] {
            let spot = zone.on_ground(at.x + 12.0 * side, at.z);
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
        let mut by = std::collections::HashSet::new();
        for _ in 0..90 * 40 {
            // Held where they stand (the line's drag aside): both stay
            // in reach.
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = at;
            app.update();
            if let Some(t) = app.world().get::<Tethered>(player) {
                by.insert(t.by);
            }
        }
        assert!(!by.is_empty(), "speared");
        assert_eq!(by.len(), 1, "one grappler's line, not turns: {by:?}");
        assert!(
            app.world().resource::<Called>().0 > 0,
            "it called the others"
        );
    }
}
