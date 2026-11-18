//! The construction job: two walkers walk out from the Gate to a work site
//! where a pylon kit sits, assemble a relay pylon (a mast that climbs out
//! of its cabinet, a sensor dish on top), run it a while, and pack it up
//! again.
//!
//! While it runs, the pylon is a sentry: what its dish sees, it reports,
//! and a response team follows. It's a robot like any other underneath:
//! shoot the dish and it's blind, wreck the cabinet and it's salvage —
//! loudly, with its builders standing next to it: its mast comes down into
//! the burst cabinet, its head on the collar and its relay coil and cell
//! out of its front, each in reach (`pylon_salvage`).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::director::Director;
use super::{
    Awareness, Brain, Job, JobKind, PartDestroyed, PartRole, Robot, RobotKind, RobotPart, Senses,
    part_bundle,
};
use crate::world::{Layer, Zone};

/// Mast height at full extension, above the cabinet.
pub const MAST: f32 = 7.0;
const CABINET: Vec3 = Vec3::new(1.0, 1.2, 1.0);
/// Seconds to put it up (with a builder there), and to take it down.
pub const BUILD_TIME: f32 = 40.0;
pub const PACK_TIME: f32 = 25.0;
/// A kit nobody comes to build is collected after this long.
const ABANDON_AFTER: f32 = 600.0;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Pylon {
    /// 0 packed .. 1 fully up.
    pub progress: f32,
    pub state: PylonState,
    pub report_cooldown: f32,
    /// Seconds since the kit was set down.
    pub age: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PylonState {
    /// A kit waiting for its builders.
    Waiting,
    Building,
    Running {
        left: f32,
    },
    Packing,
}

/// The mast: a plain box that grows with the pylon (bullets stop on it).
#[derive(Component)]
pub struct PylonMast;

pub struct PylonPlugin;

impl Plugin for PylonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                pylon_parts_destroyed,
                run_pylons,
                pose_pylons,
                report_pylons,
                pylon_salvage,
            )
                .chain()
                .after(super::perceive),
        );
    }
}

/// The kit at the site; builders are sent separately (see the director).
pub fn spawn_pylon(commands: &mut Commands, site: Vec3, run_for: f32, exit: Vec3) -> Entity {
    let robot = commands
        .spawn((
            Name::new("Relay pylon"),
            Robot {
                kind: RobotKind::Pylon,
                wrecked: false,
            },
            Pylon {
                progress: 0.0,
                state: PylonState::Waiting,
                report_cooldown: 0.0,
                age: 0.0,
            },
            Senses {
                sight: 70.0,
                fov_deg: 360.0,
                hearing: super::senses::SIGHT_ONLY,
                eye: Vec3::new(0.0, CABINET.y + 0.3, 0.0),
            },
            Awareness::default(),
            Brain::Working {
                at: site,
                left: run_for,
            },
            Job {
                kind: JobKind::Construct,
                site,
                duration: run_for,
                exit,
            },
            Transform::from_translation(site),
            Visibility::default(),
            RigidBody::Static,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Pylon cabinet"),
            part_bundle(
                robot,
                PartRole::Core,
                160.0,
                Collider::cuboid(CABINET.x, CABINET.y, CABINET.z),
                Transform::from_xyz(0.0, CABINET.y * 0.5, 0.0),
            ),
        ));
        c.spawn((
            Name::new("Pylon mast"),
            PylonMast,
            Collider::cuboid(0.25, 1.0, 0.25),
            CollisionLayers::new(Layer::Robot, [Layer::World, Layer::Item, Layer::Robot]),
            Transform::from_xyz(0.0, CABINET.y, 0.0).with_scale(Vec3::new(1.0, 0.01, 1.0)),
            Visibility::default(),
        ));
        c.spawn((
            Name::new("Pylon dish"),
            part_bundle(
                robot,
                PartRole::Sensor,
                45.0,
                Collider::cuboid(0.66, 0.36, 0.66),
                Transform::from_xyz(0.0, CABINET.y + 0.3, 0.0),
            ),
        ));
    });
    robot
}

fn pylon_parts_destroyed(
    mut events: MessageReader<PartDestroyed>,
    mut pylons: Query<&mut Robot, With<Pylon>>,
    mut raid: ResMut<crate::raid::Raid>,
) {
    for ev in events.read() {
        if ev.role != PartRole::Core {
            continue;
        }
        if let Ok(mut robot) = pylons.get_mut(ev.robot) {
            if !robot.wrecked {
                robot.wrecked = true;
                raid.say("The pylon's cabinet bursts open in a shower of sparks.");
            }
        }
    }
}

/// Builders on site put it up; it runs; they take it down.
fn run_pylons(
    mut commands: Commands,
    mut pylons: Query<(Entity, &mut Pylon, &Robot, &Transform)>,
    builders: Query<(&Job, &Brain, &Robot, &GlobalTransform), Without<Pylon>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut p, robot, t) in &mut pylons {
        p.age += dt;
        if robot.wrecked {
            // Its mast telescopes down into the burst cabinet, the head
            // coming down onto its collar, in reach (`pylon_salvage`).
            p.progress = (p.progress - dt / FALL_TIME).max(0.0);
            continue;
        }
        let site = t.translation;
        let crew = builders.iter().any(|(job, brain, r, gt)| {
            job.kind == JobKind::Construct
                && !r.wrecked
                && job.site.distance(site) < 1.0
                && matches!(brain, Brain::Working { .. })
                && gt.translation().distance(site) < 9.0
        });
        match p.state {
            PylonState::Waiting => {
                if crew {
                    p.state = PylonState::Building;
                } else if p.age > ABANDON_AFTER {
                    commands.entity(e).despawn();
                }
            }
            PylonState::Building => {
                if crew {
                    p.progress = (p.progress + dt / BUILD_TIME).min(1.0);
                }
                if p.progress >= 1.0 {
                    p.state = PylonState::Running {
                        left: builders
                            .iter()
                            .filter(|(job, ..)| job.site.distance(site) < 1.0)
                            .map(|(job, ..)| job.duration - BUILD_TIME - PACK_TIME)
                            .fold(120.0, f32::max),
                    };
                }
            }
            PylonState::Running { left } => {
                let left = left - dt;
                p.state = if left <= 0.0 {
                    PylonState::Packing
                } else {
                    PylonState::Running { left }
                };
            }
            PylonState::Packing => {
                p.progress -= dt / PACK_TIME;
                if p.progress <= 0.0 {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

/// The mast grows with the pylon; the dish (and the eye) ride on top.
fn pose_pylons(
    mut pylons: Query<(&Pylon, &mut Senses, &Children)>,
    mut masts: Query<&mut Transform, With<PylonMast>>,
    mut parts: Query<(&RobotPart, &mut Transform), Without<PylonMast>>,
) {
    for (p, mut senses, children) in &mut pylons {
        let h = MAST * p.progress.max(0.01);
        let top = CABINET.y + h;
        if (senses.eye.y - (top + 0.3)).abs() > 1e-3 {
            senses.eye.y = top + 0.3;
        }
        for c in children.iter() {
            if let Ok(mut t) = masts.get_mut(c) {
                let want = Transform::from_xyz(0.0, CABINET.y + h * 0.5, 0.0)
                    .with_scale(Vec3::new(1.0, h, 1.0));
                if *t != want {
                    *t = want;
                }
            }
            if let Ok((part, mut t)) = parts.get_mut(c) {
                if part.role == PartRole::Sensor {
                    let want = Vec3::new(0.0, top + 0.3, 0.0);
                    if t.translation != want {
                        t.translation = want;
                    }
                }
            }
        }
    }
}

/// Seconds for a wrecked pylon's mast to come down into its cabinet.
pub const FALL_TIME: f32 = 0.6;

/// Its salvage (`salvage`, #105), once its mast is down: its sensor head
/// on the cabinet's collar (three bolts), and out of the burst cabinet's
/// front its relay coil on its two leads and its cell by hand, both live.
/// (The cell's the cabinet's own health: shot through, it's scrap.) The
/// head's own look goes with its part, hidden; the item is in its place.
#[allow(clippy::type_complexity)]
fn pylon_salvage(
    mut commands: Commands,
    pylons: Query<(Entity, &Pylon, &Robot, &Children), Without<super::Salvaged>>,
    mut parts: Query<(&RobotPart, &mut Visibility)>,
    catalog: Res<crate::items::Catalog>,
    (mut meshes, mut materials): (
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
) {
    for (e, p, robot, children) in &pylons {
        if !robot.wrecked || p.progress > 0.0 {
            continue;
        }
        commands.entity(e).insert(super::Salvaged);
        let look = super::salvage::Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
        let health = |p: &RobotPart| (p.hp / p.max_hp.max(1.0)).clamp(0.0, 1.0);
        let Some(core) = children
            .iter()
            .find(|c| parts.get(*c).is_ok_and(|(p, _)| p.role == PartRole::Core))
        else {
            continue;
        };
        let mut dish = 0.0;
        for c in children.iter() {
            if let Ok((part, mut vis)) = parts.get_mut(c) {
                if part.role == PartRole::Sensor {
                    dish = health(part);
                    *vis = Visibility::Hidden;
                }
            }
        }
        let cabinet = parts.get(core).map_or(0.0, |(p, _)| health(p));
        for row in [
            wreck_row(PartRole::Sensor, dish),
            wreck_row(PartRole::Lamp(0), COIL_HEALTH),
            wreck_row(PartRole::Core, cabinet),
        ]
        .into_iter()
        .flatten()
        {
            super::salvage::fit(&mut commands, &catalog, core, row, &look);
        }
    }
}

/// Where things sit on a wrecked pylon's cabinet (local to it, its middle
/// 0.6 m up): the head's middle on its collar, mast down
/// (`art/robots/pylon.py` prints the head's middle), the coil and the
/// cell out of its burst front.
pub const DISH_AT: Vec3 = Vec3::new(0.0, 1.29, 0.025);
pub const DISH_BOLTS: usize = 3;
pub const COIL_AT: Vec3 = Vec3::new(-0.2, 0.0, -0.56);
pub const CELL_AT: Vec3 = Vec3::new(0.22, -0.15, -0.6);
/// Its relay coil comes through the cabinet bursting (it's in a can).
pub const COIL_HEALTH: f32 = 0.7;
pub const COIL_LEADS: usize = 2;

/// What a wrecked pylon gives up, and how each is held (`Lamp(0)` keys its
/// relay coil: it has no lamp).
pub fn wreck_row(role: PartRole, health: f32) -> Option<super::salvage::Row> {
    use super::salvage::{Fix, Row};
    let row = |item, at: Vec3, fix, hot| Row {
        item,
        at: Transform::from_translation(at),
        size: 1.0,
        health,
        fix,
        hot,
        replaces: None,
    };
    Some(match role {
        PartRole::Sensor => row(
            "pylon_dish",
            DISH_AT,
            Fix::Bolts {
                n: DISH_BOLTS,
                centre: Vec3::new(0.0, 0.8, 0.0),
                face: Quat::IDENTITY,
                radius: 0.17,
            },
            false,
        ),
        PartRole::Lamp(0) => row(
            "pylon_relay_coil",
            COIL_AT,
            Fix::Cables { n: COIL_LEADS },
            true,
        ),
        PartRole::Core => row("robot_cell", CELL_AT, Fix::Hand, true),
        _ => return None,
    })
}

/// A running pylon reports what its dish sees: a response team follows.
fn report_pylons(
    mut pylons: Query<(Entity, &mut Pylon, &Awareness, &Robot, &Transform)>,
    director: Option<ResMut<Director>>,
    targets: Query<&GlobalTransform>,
    time: Res<Time>,
) {
    let Some(mut director) = director else {
        return;
    };
    let dt = time.delta_secs();
    for (e, mut p, aware, robot, t) in &mut pylons {
        p.report_cooldown -= dt;
        if robot.wrecked
            || !matches!(p.state, PylonState::Running { .. })
            || !aware.can_see_target
            || aware.suspicion < 0.6
            || p.report_cooldown > 0.0
        {
            continue;
        }
        if let Some(at) = aware.target.and_then(|t| targets.get(t).ok()) {
            let delay = director.rng.range(15.0, 30.0);
            // Through the network (`network`): a cut, a jam or distrust
            // keeps it from going out.
            director.queue_report(e, t.translation, delay, at.translation());
            p.report_cooldown = 75.0;
            info!("pylon: reported someone at {:.0}", at.translation());
        }
    }
}

/// The crew for a pylon: two walkers from the Gate, standing either side.
pub fn send_builders(commands: &mut Commands, zone: &Zone, site: Vec3, duration: f32, exit: Vec3) {
    for k in 0..2 {
        let a = k as f32 * std::f32::consts::PI + 0.4;
        let post = zone.on_ground(site.x + a.cos() * 4.0, site.z + a.sin() * 4.0);
        let e = super::walker::spawn_walker(
            commands,
            zone,
            exit,
            Job {
                kind: JobKind::Construct,
                site,
                duration,
                exit,
            },
        );
        commands.entity(e).insert(Brain::Transit { to: post });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walkers_put_up_a_pylon_that_reports_what_it_sees() {
        let opts = crate::LaunchOptions {
            robots: true,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        {
            // Nothing else from the director: just this job.
            let mut d = app.world_mut().resource_mut::<Director>();
            d.next_job = 1e9;
            d.next_logistics = 1e9;
        }
        let zone = app.world().resource::<Zone>().clone();
        // The south field work site on Marrow Creek: open ground.
        let site = zone.on_ground(100.0, 210.0);
        let exit = super::super::director::gate(&zone);
        // Builders already on site, to keep the test short.
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let pylon = {
            let mut c = Commands::new(&mut queue, app.world());
            let p = spawn_pylon(&mut c, site, 200.0, exit);
            send_builders(&mut c, &zone, site, 200.0, exit);
            p
        };
        queue.apply(app.world_mut());
        app.update();
        let builders: Vec<Entity> = app
            .world_mut()
            .query::<(Entity, &Job)>()
            .iter(app.world())
            .filter(|(_, j)| j.kind == JobKind::Construct && j.site == site)
            .map(|(e, _)| e)
            .filter(|e| *e != pylon)
            .collect();
        assert_eq!(builders.len(), 2);
        for (i, b) in builders.iter().enumerate() {
            let a = i as f32 * std::f32::consts::PI + 0.4;
            let post = zone.on_ground(site.x + a.cos() * 4.0, site.z + a.sin() * 4.0);
            app.world_mut()
                .get_mut::<Transform>(*b)
                .unwrap()
                .translation = post + Vec3::Y;
        }
        let mut running = false;
        for _ in 0..90 * 90 {
            app.update();
            if matches!(
                app.world().get::<Pylon>(pylon).unwrap().state,
                PylonState::Running { .. }
            ) {
                running = true;
                break;
            }
        }
        assert!(
            running,
            "the pylon went up: {:?}",
            app.world().get::<Pylon>(pylon)
        );
        // Someone in plain view of the dish gets reported.
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let near = zone.on_ground(site.x + 25.0, site.z + 10.0);
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = near;
        // Standing (nothing drives the rig here, and its head starts on the
        // floor).
        app.world_mut()
            .get_mut::<crate::rig::RigInput>(player)
            .unwrap()
            .head
            .translation
            .y = 1.65;
        let mut reported = false;
        for _ in 0..90 * 10 {
            app.update();
            if !app.world().resource::<Director>().responses.is_empty() {
                reported = true;
                break;
            }
        }
        assert!(reported, "the pylon reported the player");
    }

    #[test]
    fn a_wrecked_pylon_is_salvage() {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<Zone>().clone();
        let site = zone.on_ground(100.0, 210.0);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let pylon = {
            let mut c = Commands::new(&mut queue, app.world());
            spawn_pylon(&mut c, site, 200.0, site)
        };
        queue.apply(app.world_mut());
        app.update();
        let near = |app: &mut App| {
            app.world_mut()
                .query_filtered::<&GlobalTransform, With<crate::items::Item>>()
                .iter(app.world())
                .filter(|g| g.translation().distance(site) < 3.0)
                .count()
        };
        let before = near(&mut app);
        let cabinet = app
            .world_mut()
            .query::<(Entity, &RobotPart)>()
            .iter(app.world())
            .find(|(_, p)| p.robot == pylon && p.role == PartRole::Core)
            .map(|(e, _)| e)
            .unwrap();
        app.world_mut().write_message(PartDestroyed {
            robot: pylon,
            part: cabinet,
            role: PartRole::Core,
            at: site,
        });
        for _ in 0..90 * 5 {
            app.update();
        }
        assert!(app.world().get::<Robot>(pylon).unwrap().wrecked);
        assert!(near(&mut app) >= before + 3, "salvage spilled out");
        // Seated in reach (`pylon_salvage`): its head on the collar, mast
        // down; its coil and cell out of the cabinet's front, live.
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let seated: Vec<(String, Vec3, bool)> = app
            .world_mut()
            .query_filtered::<(
                &crate::items::Item,
                &GlobalTransform,
                Has<super::super::salvage::Live>,
            ), With<crate::interact::InSocket>>()
            .iter(app.world())
            .filter(|(_, g, _)| g.translation().distance(site) < 3.0)
            .map(|(i, g, live)| (catalog.get(i.kind()).id.clone(), g.translation(), live))
            .collect();
        let mut ids: Vec<&str> = seated.iter().map(|(id, ..)| id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            ["pylon_dish", "pylon_relay_coil", "robot_cell"],
            "{seated:?}"
        );
        for (id, at, live) in &seated {
            let over = at.y - site.y;
            assert!((0.0..1.6).contains(&over), "{id} {over:.2} m up");
            assert_eq!(*live, id != "pylon_dish", "{id} live: {live}");
        }
        // Its own salvage, not the pylon table's rolls on top.
        assert!(app.world().get::<super::super::Salvaged>(pylon).is_some());
    }
}
