//! The Zone director: posts jobs and sends robots in through the Gate to do
//! them. Nobody patrols; things *arrive*, *work*, and *leave*.
//!
//! - **Survey**: a drone flies to a point of interest, circles it a while,
//!   flies home.
//! - **Watch**: a walker picks its way to a work site and stands there.
//! - **Response**: loud noise (gunfire) is reported; after a delay a drone
//!   is dispatched to where it was heard. Noise has a cost, paid later.
//!
//! Attention from carried artifacts (later) feeds the same queue.

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
}

pub struct DirectorPlugin;

impl Plugin for DirectorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_director).add_systems(
            FixedUpdate,
            (hear_reports, run_director).chain().after(super::perceive),
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
    });
}

/// Loud noises get reported; a team goes to look, late.
fn hear_reports(
    mut noises: MessageReader<Noise>,
    mut director: ResMut<Director>,
    players: Query<(), With<crate::player::Player>>,
) {
    for n in noises.read() {
        let by_player = n.source.is_some_and(|s| players.contains(s));
        if by_player && n.radius > 150.0 && director.response_cooldown <= 0.0 {
            let delay = director.rng.range(35.0, 70.0);
            director.responses.push((delay, n.at));
            director.response_cooldown = 45.0;
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
    robots: Query<&Robot>,
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
        info!("director: response team to {at:.0}");
        spawn_drone(
            &mut commands,
            exit + Vec3::Y * 15.0,
            Job {
                kind: JobKind::Response,
                site: at,
                duration: 40.0,
                exit,
            },
        );
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
