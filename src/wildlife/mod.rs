//! Animals: the valleys' life besides the machines, as atmosphere and as
//! information, the way a hunter reads a wood (`docs/WILDLIFE.md`).
//!
//! - [`flock`]: rooks and crows that sit on roofs and masts and burst up,
//!   loud and seen from far off, when something comes near: a robot, or
//!   you. A flock going up 200 m away says something is there; your own
//!   careless sprint past one tells the machines ([`Flushed`], and a
//!   [`crate::robots::Noise`] at the flock).
//! - [`perches`]: where birds can sit, from the buildings' own pieces.
//! - [`bats`]: at dusk and at night, over the water and round the
//!   buildings; off a light, and gone when a machine's near.
//! - [`ducks`]: mallards on open water, that swim off from you and go up
//!   splashing when you're close (the same [`Flushed`] as a flock).
//!
//! Logic here runs headless (in [`WildlifePlugin`], part of `GamePlugin`)
//! and is deterministic for a seed; what you see is in [`visuals`]
//! (`ClientPlugin`).

use bevy::prelude::*;

pub mod bat_look;
pub mod bats;
pub mod bodies;
pub mod duck_look;
pub mod ducks;
pub mod fish;
pub mod flock;
pub mod herd;
pub mod herd_look;
pub mod insects;
pub mod perches;
pub mod rats;
pub mod small_look;
mod telemetry;
pub mod visuals;

#[cfg(test)]
mod app_tests;

/// Why a flock went up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlushCause {
    /// Someone came too close (`Flushed::by`).
    Player,
    /// A machine came too close.
    Robot,
    /// A loud noise (a shot, a blast) close enough to scare it.
    Noise,
    /// Nothing: the flock moved on of its own accord, unhurried.
    Restless,
}

/// A flock went up: where, how many birds, why, and who did it. Sound and
/// the robots' ears read it.
#[derive(Message, Clone, Copy, Debug)]
pub struct Flushed {
    pub flock: Entity,
    pub at: Vec3,
    pub birds: u32,
    pub cause: FlushCause,
    /// The player or robot that was too close, if it was one.
    pub by: Option<Entity>,
    /// A player flushed it and the machines will hear of it (at most once a
    /// minute a flock: `flock::ALARM_EVERY`).
    pub alarm: bool,
}

/// Something animals keep away from, this tick.
#[derive(Clone, Copy, Debug)]
pub struct Threat {
    pub entity: Entity,
    pub at: Vec3,
    /// Horizontal speed, m/s.
    pub speed: f32,
    pub kind: ThreatKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreatKind {
    /// A player; `crouched` from their body's height.
    Player {
        crouched: bool,
    },
    Drone,
    Walker,
    Dirigible,
}

/// Every threat in the valley, gathered once a tick for all the animals.
#[derive(Resource, Default, Debug)]
pub struct Threats(pub Vec<Threat>);

fn gather_threats(
    mut threats: ResMut<Threats>,
    players: Query<(Entity, &GlobalTransform, &crate::player::Body), With<crate::player::Player>>,
    robots: Query<(Entity, &GlobalTransform, &crate::robots::Robot)>,
    mut last: Local<std::collections::HashMap<Entity, Vec3>>,
    time: Res<Time>,
    mut calm: Local<Option<bool>>,
) {
    use crate::robots::RobotKind;
    threats.0.clear();
    // `EXTRACTION_WILDLIFE=calm`: nothing scares them (for looking at
    // them in screenshots).
    if *calm.get_or_insert_with(|| std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "calm"))
    {
        return;
    }
    for (e, gt, body) in &players {
        threats.0.push(Threat {
            entity: e,
            at: gt.translation(),
            speed: Vec2::new(body.velocity.x, body.velocity.z).length(),
            kind: ThreatKind::Player {
                crouched: body.height > 0.0 && body.height < crate::robots::senses::CROUCHED,
            },
        });
    }
    let dt = time.delta_secs().max(1e-4);
    let mut seen = std::collections::HashMap::new();
    for (e, gt, robot) in &robots {
        if robot.wrecked {
            continue;
        }
        let kind = match robot.kind {
            RobotKind::Drone => ThreatKind::Drone,
            RobotKind::Walker => ThreatKind::Walker,
            RobotKind::Dirigible => ThreatKind::Dirigible,
            // A pylon stands still at its site; the sorter is indoors.
            RobotKind::Pylon | RobotKind::Sorter => continue,
        };
        let at = gt.translation();
        let speed = last.get(&e).map_or(0.0, |p: &Vec3| {
            Vec2::new(at.x - p.x, at.z - p.z).length() / dt
        });
        seen.insert(e, at);
        threats.0.push(Threat {
            entity: e,
            at,
            speed,
            kind,
        });
    }
    *last = seen;
}

/// Ordering for animal logic: threats first, then the animals.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct WildlifeSystems;

/// Animal logic. Headless-safe.
pub struct WildlifePlugin;

impl Plugin for WildlifePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bodies::BodiesPlugin)
            .add_message::<Flushed>()
            .add_message::<herd::Startled>()
            .add_message::<rats::Scurried>()
            .init_resource::<Threats>()
            .init_resource::<insects::Chorus>()
            .add_systems(Startup, setup)
            .add_systems(
                FixedUpdate,
                (
                    scene_perches,
                    gather_threats,
                    (
                        flock::flocks,
                        herd::herds,
                        fish::shoals,
                        rats::rats,
                        bats::bats,
                        ducks::rafts,
                    ),
                )
                    .chain()
                    .in_set(WildlifeSystems),
            )
            .add_systems(
                FixedUpdate,
                (flock::alarm_robots, herd::bark_alarm, insects::chorus).after(WildlifeSystems),
            )
            .add_systems(Last, telemetry::log_wildlife);
    }
}

/// The valley's perches and its animals, once the map is in.
fn setup(mut commands: Commands, zone: Res<crate::world::Zone>, opts: Res<crate::LaunchOptions>) {
    let started = std::time::Instant::now();
    let (obstacles, mut perches) = perches::survey(&zone);
    // Somewhere for a heron to stand, where there's one.
    if opts.map == "sallow_reservoir" {
        perches.sites.extend(perches::wading(&zone));
    }
    // `EXTRACTION_WILDLIFE=off`: no animals (to compare sims with and without).
    let off = std::env::var("EXTRACTION_WILDLIFE").is_ok_and(|v| v == "off");
    let flocks = if off {
        0
    } else {
        flock::spawn_flocks(&mut commands, &opts.map, &perches, opts.seed)
    };
    info!(
        "wildlife: {} perch sites ({} perches), {flocks} flocks, in {:.0} ms",
        perches.sites.len(),
        perches.sites.iter().map(|s| s.spots.len()).sum::<usize>(),
        started.elapsed().as_secs_f32() * 1000.0
    );
    commands.insert_resource(obstacles);
    commands.insert_resource(perches);
}

/// Story's staged scenes (`world::scenes::Staged`: a row of kept buses, a
/// yard of luggage) are somewhere to perch too, and to keep out of, once
/// they're staged: they're added as they appear.
fn scene_perches(
    staged: Option<Res<crate::world::scenes::Staged>>,
    zone: Res<crate::world::Zone>,
    opts: Res<crate::LaunchOptions>,
    perches: Option<ResMut<perches::Perches>>,
    obstacles: Option<ResMut<perches::Obstacles>>,
    mut done: Local<std::collections::HashSet<String>>,
) {
    let (Some(staged), Some(mut perches), Some(mut obstacles)) = (staged, perches, obstacles)
    else {
        return;
    };
    if staged.0.len() == done.len() {
        return;
    }
    for (id, scene) in crate::world::scenes::of(&opts.map) {
        if !staged.0.contains(&id) || !done.insert(id.clone()) {
            continue;
        }
        let (solids, site) = perches::scene(&zone, &id, scene);
        for s in solids {
            obstacles.push(s);
        }
        if let Some(site) = site {
            debug!("perches on the scene {}: {}", scene.name, site.spots.len());
            perches.sites.push(site);
        }
    }
}

/// Animal visuals: the client's half.
pub struct WildlifeVisualsPlugin;

impl Plugin for WildlifeVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            visuals::VisualsPlugin,
            herd_look::HerdLookPlugin,
            small_look::SmallLookPlugin,
            bat_look::BatLookPlugin,
            duck_look::DuckLookPlugin,
        ));
    }
}

#[cfg(test)]
mod doc_tests {
    /// docs/WILDLIFE.md stays a document: a script's bad edit once wrote one
    /// paragraph into it thirteen thousand times (10 MB, every heading
    /// gone), and nothing noticed until a reader did.
    #[test]
    fn the_wildlife_doc_is_whole() {
        let text = std::fs::read_to_string(crate::asset_root_path().join("../docs/WILDLIFE.md"))
            .expect("docs/WILDLIFE.md");
        assert!(text.len() < 100_000, "{} bytes", text.len());
        for h in [
            "## Rooks",
            "## Deer and dogs",
            "## Fish",
            "## Bats",
            "## The chorus",
            "## Checks",
            "## Pacing",
        ] {
            assert_eq!(
                text.matches(h).count(),
                1,
                "{h} appears {} times",
                text.matches(h).count()
            );
        }
    }
}
