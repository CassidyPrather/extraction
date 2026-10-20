//! The Zone director: posts jobs and sends robots in through the Gate to do
//! them. Nobody patrols; things *arrive*, *work*, and *leave*.
//!
//! - **Survey**: a drone flies to a point of interest, circles it a while,
//!   flies home.
//! - **Watch**: a walker picks its way to a work site and stands there.
//! - **Response**: loud noise (gunfire) is reported; after a delay a drone
//!   is dispatched to where it was heard. Noise has a cost, paid later.
//!
//! - **Logistics** and **Construction**: see `dirigible` and `pylon`.
//!
//! Attention from carried artifacts feeds the same queue (`anomaly`).
//!
//! - **Night visit**: someone sleeping behind a door with the Zone's
//!   attention on them ([`NightVisit`], sent by whoever puts them to bed)
//!   gets two walkers at that door, late: one batters it down if it's
//!   wedged (`doors`). It replaces any response already queued near there.

use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::noise::Rng;

use super::drone::spawn_drone;
use super::walker::spawn_walker;
use super::{Job, JobKind, Noise, Robot};
use crate::launch::LaunchOptions;
use crate::world::Zone;

#[derive(Resource)]
pub struct Director {
    pub enabled: bool,
    pub rng: Rng,
    /// Seconds until the next routine job.
    pub next_job: f32,
    /// Pending responses: (seconds until dispatch, where).
    pub responses: Vec<(f32, Vec3)>,
    /// Seconds before another response may be queued.
    pub response_cooldown: f32,
    pub max_active: usize,
    /// Seconds until the next logistics run (dirigible and grapplers).
    pub next_logistics: f32,
    /// Where responses were queued this tick: the robots talk about it, and
    /// a radio can hear them.
    pub chatter: Vec<Vec3>,
    /// Pending night visits: (seconds until dispatch, the door, where the
    /// sleeper lies).
    pub visits: Vec<(f32, Entity, Vec3)>,
}

impl Director {
    /// Send a response team to `at` in `delay` seconds.
    pub fn queue_response(&mut self, delay: f32, at: Vec3) {
        self.responses.push((delay, at));
        self.chatter.push(at);
    }
}

/// Send a night visit: two walkers to `door` (a door of the building
/// someone sleeps in; `at` is where they lie) in `delay` seconds.
#[derive(Message, Clone, Copy, Debug)]
pub struct NightVisit {
    pub door: Entity,
    pub at: Vec3,
    pub delay: f32,
}

/// A night visit replaces the responses queued within this of it (m).
pub const VISIT_CLEARS: f32 = 150.0;
/// The visitors look for the sleeper this long before they give up (s).
pub const VISIT_TIME: f32 = 600.0;

pub struct DirectorPlugin;

impl Plugin for DirectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<NightVisit>()
            .add_systems(Startup, init_director)
            .add_systems(
                FixedUpdate,
                (hear_reports, night_visits, run_director)
                    .chain()
                    .after(super::perceive),
            );
    }
}

fn init_director(mut commands: Commands, opts: Res<LaunchOptions>) {
    commands.insert_resource(Director {
        enabled: opts.robots,
        rng: Rng::new(opts.seed ^ 0xD1EC),
        next_job: 25.0,
        responses: Vec::new(),
        response_cooldown: 0.0,
        max_active: 6,
        next_logistics: 150.0,
        chatter: Vec::new(),
        visits: Vec::new(),
    });
}

/// Loud noises get reported; a team goes to look, late.
fn hear_reports(
    mut noises: MessageReader<Noise>,
    mut director: ResMut<Director>,
    players: Query<(), With<crate::player::Player>>,
    hushes: Option<Res<crate::anomaly::hush::Hushes>>,
) {
    for n in noises.read() {
        // Nothing made in a Hush is heard (`anomaly::hush`).
        if hushes.as_ref().is_some_and(|h| h.eats(n.at)) {
            continue;
        }
        let by_player = n.source.is_some_and(|s| players.contains(s));
        if by_player && n.radius > 150.0 && director.response_cooldown <= 0.0 {
            let delay = director.rng.range(35.0, 70.0);
            director.queue_response(delay, n.at);
            director.response_cooldown = 45.0;
        }
    }
}

/// Night visits: queued as they're sent (clearing responses near them),
/// dispatched when due: two walkers from the Gate, looking for whoever is
/// inside the building behind that door.
fn night_visits(
    mut commands: Commands,
    mut visits: MessageReader<NightVisit>,
    mut director: ResMut<Director>,
    zone: Res<Zone>,
    entrances: Res<super::doors::Entrances>,
    time: Res<Time>,
) {
    for v in visits.read() {
        director
            .responses
            .retain(|r| r.1.distance(v.at) > VISIT_CLEARS);
        director.visits.push((v.delay, v.door, v.at));
        director.chatter.push(v.at);
    }
    if !director.enabled {
        return;
    }
    let dt = time.delta_secs();
    let mut due = Vec::new();
    for v in &mut director.visits {
        v.0 -= dt;
        if v.0 <= 0.0 {
            due.push((v.1, v.2));
        }
    }
    director.visits.retain(|v| v.0 > 0.0);
    let exit = gate(&zone);
    for (door, at) in due {
        // Inside, behind that door (if it's one we know; else where they
        // lie): the walkers' way in is the door.
        let inside = entrances
            .doors
            .iter()
            .find(|e| e.door == door)
            .map(|e| e.mid - e.out * 2.0)
            .unwrap_or(at);
        info!("director: night visit to {inside:.0}");
        for k in 0..2 {
            let side = Vec3::new(k as f32 * 2.0 - 1.0, 0.0, 0.0) * 2.0;
            let w = spawn_walker(
                &mut commands,
                &zone,
                exit + side,
                Job {
                    kind: JobKind::Response,
                    site: inside,
                    duration: 40.0,
                    exit,
                },
            );
            commands.entity(w).insert(super::Brain::Investigate {
                at: inside,
                left: VISIT_TIME,
            });
        }
    }
}

pub fn gate(zone: &Zone) -> Vec3 {
    let m = zone
        .marker(&MarkerKind::RobotGate)
        .map(|m| m.at)
        .unwrap_or([0.0, -280.0]);
    zone.on_ground(m[0], m[1])
}

fn run_director(
    mut commands: Commands,
    mut director: ResMut<Director>,
    zone: Res<Zone>,
    // Robots that live in a site (underground residents) aren't the
    // director's to count: they'd use up the budget for the valley.
    robots: Query<&Robot, Without<super::indoor::LocalNav>>,
    pylons: Query<(), With<super::pylon::Pylon>>,
    time: Res<Time>,
) {
    if !director.enabled {
        return;
    }
    let dt = time.delta_secs();
    director.response_cooldown -= dt;
    director.next_job -= dt;
    let active = robots.iter().filter(|r| !r.wrecked).count();
    let exit = gate(&zone);
    // Responses first.
    let mut dispatch = Vec::new();
    for r in &mut director.responses {
        r.0 -= dt;
        if r.0 <= 0.0 {
            dispatch.push(r.1);
        }
    }
    director.responses.retain(|r| r.0 > 0.0);
    for at in dispatch {
        if active >= director.max_active {
            continue;
        }
        let job = Job {
            kind: JobKind::Response,
            site: at,
            duration: 40.0,
            exit,
        };
        // Now and then a pair of hounds instead: they hunt by ear.
        if director.rng.chance(super::hound_mind::HOUND_SHARE) && active + 2 <= director.max_active
        {
            info!("director: hounds to {at:.0}");
            for k in 0..2 {
                let side = Vec3::new(k as f32 * 2.0 - 1.0, 0.0, 0.0) * 1.5;
                super::hound::spawn_hound(&mut commands, &zone, exit + side, job.clone());
            }
            continue;
        }
        info!("director: response team to {at:.0}");
        spawn_drone(&mut commands, exit + Vec3::Y * 15.0, job);
    }
    // Logistics: a dirigible brings a cargo pod to a work site and
    // grapplers walk out to guard it, one run at a time.
    director.next_logistics -= dt;
    let airborne = robots
        .iter()
        .any(|r| r.kind == super::RobotKind::Dirigible && !r.wrecked);
    if director.next_logistics <= 0.0 && !airborne && active + 3 <= director.max_active {
        director.next_logistics = director.rng.range(360.0, 600.0);
        let sites: Vec<[f32; 2]> = zone
            .map
            .markers_of(&MarkerKind::WorkSite)
            .map(|m| m.at)
            .collect();
        if !sites.is_empty() {
            let s = sites[director.rng.below(sites.len())];
            let site = zone.on_ground(s[0], s[1]);
            // In over the map edge beyond the Gate, out the same way.
            let out = Vec3::new(exit.x, 0.0, exit.z).normalize_or(Vec3::NEG_Z);
            let edge = out * (zone.map.size * 0.5 + 60.0);
            let start = Vec3::new(edge.x, exit.y + 45.0, edge.z);
            let duration = director.rng.range(150.0, 240.0);
            info!("director: logistics run to {s:?}");
            super::dirigible::spawn_dirigible(
                &mut commands,
                start,
                Job {
                    kind: JobKind::Watch,
                    site,
                    duration,
                    exit: start,
                },
            );
            for k in 0..2 {
                let a = k as f32 * std::f32::consts::PI + 0.7;
                let post = zone.on_ground(site.x + a.cos() * 7.0, site.z + a.sin() * 7.0);
                super::grappler::spawn_grappler(
                    &mut commands,
                    &zone,
                    exit,
                    Job {
                        kind: JobKind::Watch,
                        site: post,
                        duration: duration + 60.0,
                        exit,
                    },
                );
            }
        }
    }
    if director.next_job > 0.0 || active >= director.max_active {
        return;
    }
    director.next_job = director.rng.range(50.0, 110.0);
    let pois: Vec<[f32; 2]> = zone
        .map
        .markers_of(&MarkerKind::Poi)
        .map(|m| m.at)
        .collect();
    let sites: Vec<[f32; 2]> = zone
        .map
        .markers_of(&MarkerKind::WorkSite)
        .map(|m| m.at)
        .collect();
    if director.rng.chance(0.6) && !pois.is_empty() {
        let p = pois[director.rng.below(pois.len())];
        info!("director: survey drone to {p:?}");
        spawn_drone(
            &mut commands,
            exit + Vec3::Y * 15.0,
            Job {
                kind: JobKind::Survey,
                site: zone.on_ground(p[0], p[1]),
                duration: director.rng.range(40.0, 90.0),
                exit,
            },
        );
    } else if !sites.is_empty()
        && director.rng.chance(0.5)
        && pylons.is_empty()
        && active + 3 <= director.max_active
    {
        // Construction: a pylon kit at a site, two walkers to put it up.
        let s = sites[director.rng.below(sites.len())];
        let site = zone.on_ground(s[0], s[1]);
        let run = director.rng.range(150.0, 260.0);
        let duration = super::pylon::BUILD_TIME + run + super::pylon::PACK_TIME;
        info!("director: construction at {s:?}");
        super::pylon::spawn_pylon(&mut commands, site, duration, exit);
        super::pylon::send_builders(&mut commands, &zone, site, duration, exit);
    } else if !sites.is_empty() {
        let s = sites[director.rng.below(sites.len())];
        info!("director: walker to work site {s:?}");
        spawn_walker(
            &mut commands,
            &zone,
            exit,
            Job {
                kind: JobKind::Watch,
                site: zone.on_ground(s[0], s[1]),
                duration: director.rng.range(90.0, 200.0),
                exit,
            },
        );
    }
}
