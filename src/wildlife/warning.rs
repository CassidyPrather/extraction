//! Wildlife as warning: how often the valley tells you a machine is coming
//! before it can see you (`docs/WILDLIFE.md`, "Warning"). A sim gauge:
//! nothing here changes the game.
//!
//! Every robot that moves is watched against the local player. A **cue**
//! is something the player can read at a distance, put down to that robot:
//! - a flock it flushed, within [`READ_BIRDS`] of you;
//! - a herd that bolted from it, within [`READ_HERD`] of you;
//! - the chorus round you cut by it to [`HUSH_CUE`] or less, when the
//!   chorus would be singing otherwise (`insects::hush`, by day or night,
//!   not in a downpour or after a fright).
//!
//! An **approach** is the moment a robot first has a clear line from its
//! eye to your head, inside its sight, after [`UNSEEN_FOR`] seconds
//! without one, having come at least [`CAME`] metres in the last
//! [`UNSEEN_FOR`] (it came to you, not you to it). It was **warned** if
//! one of its cues came [`LEAD`] to [`LOOK_BACK`] seconds before.
//! `scripts/wildlife_warning.py` adds the runs up.

use std::collections::{HashMap, VecDeque};

use avian3d::prelude::*;
use bevy::prelude::*;
use serde_json::json;

use super::insects;
use super::{FlushCause, Flushed};
use crate::robots::{Breed, Robot, RobotKind, Senses};
use crate::sim::telemetry::{SimLog, round};

/// A flock going up is read this far off (m)...
pub const READ_BIRDS: f32 = insects::FRIGHT_NEAR;
/// ...a herd bolting this far.
pub const READ_HERD: f32 = 120.0;
/// The chorus cut to this by one machine is a cue.
pub const HUSH_CUE: f32 = 0.6;
/// The chorus must be at least this, the machine aside, for its going
/// quiet to tell.
pub const SINGING: f32 = 0.5;
/// A cue this long before the robot sees you warns you (s)...
pub const LEAD: f32 = 5.0;
/// ...looking back at most this far.
pub const LOOK_BACK: f32 = 120.0;
/// Unseen this long, a robot that sees you is a new approach (s)...
pub const UNSEEN_FOR: f32 = 20.0;
/// ...if it has come this far meanwhile (m).
pub const CAME: f32 = 10.0;
/// How often robots are looked at (s).
const EVERY: f32 = 0.25;

#[derive(Default)]
pub(crate) struct Track {
    los: bool,
    /// When it last had you in sight, if ever.
    seen: Option<f32>,
    /// Where it's been: (t, position), the last [`UNSEEN_FOR`] seconds.
    path: VecDeque<(f32, Vec3)>,
    /// Its cues: (t, what).
    cues: Vec<(f32, &'static str)>,
    /// Hushing the chorus now (a cue when it starts).
    hushing: bool,
    /// How near a flock and a herd it's been: (t, flock, herd), the last
    /// [`LOOK_BACK`] seconds.
    near: VecDeque<(f32, f32, f32)>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn log_warnings(
    log: Option<ResMut<SimLog>>,
    me: Query<(&GlobalTransform, &crate::player::Body), With<crate::rig::LocalPlayer>>,
    robots: Query<(Entity, &Robot, Option<&Breed>, &Senses, &GlobalTransform)>,
    chorus: Res<insects::Chorus>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    spatial: SpatialQuery,
    (mut flushed, mut startled): (MessageReader<Flushed>, MessageReader<super::herd::Startled>),
    (flocks, herds, zone): (
        Query<&super::flock::Flock>,
        Query<&super::herd::Herd>,
        Res<crate::world::Zone>,
    ),
    mut tracks: Local<HashMap<Entity, Track>>,
    mut next: Local<f32>,
) {
    let (Some(mut log), Ok((me, body))) = (log, me.single()) else {
        flushed.clear();
        startled.clear();
        return;
    };
    let t = log.t;
    let foot = me.translation();
    for f in flushed.read() {
        if let (FlushCause::Robot, Some(by)) = (f.cause, f.by)
            && f.at.distance(foot) < READ_BIRDS
        {
            tracks.entry(by).or_default().cues.push((t, "flush"));
        }
    }
    for s in startled.read() {
        if let (super::herd::Startle::Bolt, Some(by)) = (s.what, s.by)
            && robots.contains(by)
            && s.at.distance(foot) < READ_HERD
        {
            tracks.entry(by).or_default().cues.push((t, "bolt"));
        }
    }
    if t < *next {
        return;
    }
    *next = t + EVERY;
    let head = foot + Vec3::Y * body.height.max(1.0);
    // The chorus as it would be with no machine about.
    let singing = chorus.fright * conditions.as_deref().map_or(1.0, insects::weather);
    let (hour, rain) = conditions.as_deref().map_or(("Day".to_string(), 0.0), |c| {
        (format!("{:?}", c.hour), c.rain)
    });
    let filter = SpatialQueryFilter::from_mask(crate::world::Layer::World);
    let under = |p: Vec3| p.y < zone.ground(p.x, p.z) - 2.0;
    let flocks: Vec<Vec3> = flocks.iter().map(|f| f.mean()).collect();
    let herds: Vec<Vec3> = herds
        .iter()
        .filter_map(|h| h.leader().map(|b| b.pos))
        .collect();
    let nearest = |ps: &[Vec3], p: Vec3| {
        ps.iter()
            .map(|q| q.xz().distance(p.xz()))
            .fold(f32::MAX, f32::min)
    };
    let mut alive = Vec::new();
    for (e, robot, breed, senses, gt) in &robots {
        if robot.wrecked || matches!(robot.kind, RobotKind::Pylon | RobotKind::Sorter) {
            continue;
        }
        alive.push(e);
        let at = gt.translation();
        let tr = tracks.entry(e).or_default();
        tr.path.push_back((t, at));
        while tr.path.front().is_some_and(|(s, _)| t - s > UNSEEN_FOR) {
            tr.path.pop_front();
        }
        tr.cues.retain(|(s, _)| t - s < LOOK_BACK);
        tr.near
            .push_back((t, nearest(&flocks, at), nearest(&herds, at)));
        while tr.near.front().is_some_and(|(s, _, _)| t - s > LOOK_BACK) {
            tr.near.pop_front();
        }
        let near = insects::hush_near(match robot.kind {
            RobotKind::Drone => super::ThreatKind::Drone,
            RobotKind::Dirigible => super::ThreatKind::Dirigible,
            _ => super::ThreatKind::Walker,
        });
        let hushing = singing >= SINGING && insects::hush(at.distance(foot), near) <= HUSH_CUE;
        if hushing && !tr.hushing {
            tr.cues.push((t, "hush"));
        }
        tr.hushing = hushing;
        let eye = gt.transform_point(senses.eye);
        let d = eye.distance(head);
        let los = d < senses.sight
            && Dir3::new(eye - head).is_ok_and(|dir| {
                spatial
                    .cast_ray(head, dir, d - 0.5, true, &filter)
                    .is_none()
            });
        if los && !tr.los {
            let fresh = tr.seen.is_none_or(|s| t - s >= UNSEEN_FOR);
            let came = tr
                .path
                .front()
                .is_some_and(|(_, p)| p.xz().distance(at.xz()) >= CAME);
            if fresh && came {
                let lead = tr
                    .cues
                    .iter()
                    .map(|(s, _)| t - s)
                    .filter(|l| *l >= LEAD)
                    .fold(None, |m: Option<f32>, l| Some(m.map_or(l, |m| m.max(l))));
                let mut kinds: Vec<&str> = tr
                    .cues
                    .iter()
                    .filter(|(s, _)| t - s >= LEAD)
                    .map(|c| c.1)
                    .collect();
                kinds.sort_unstable();
                kinds.dedup();
                let (flock, herd) = tr
                    .near
                    .iter()
                    .fold((f32::MAX, f32::MAX), |(f, h), n| (f.min(n.1), h.min(n.2)));
                let far = |d: f32| (d < f32::MAX).then(|| round(d, 0));
                log.event(
                    "approach",
                    json!({"robot": crate::robots::robot_name(robot.kind, breed),
                           "dist": round(d, 1), "warned": lead.is_some(),
                           "lead": lead.map(|l| round(l, 1)), "cues": kinds,
                           "hour": hour, "rain": round(rain, 2),
                           "singing": round(singing, 2),
                           "under": under(at) || under(foot),
                           "flock": far(flock), "herd": far(herd),
                           "x": round(at.x, 1), "z": round(at.z, 1)}),
                );
            }
        }
        if los {
            tr.seen = Some(t);
        }
        tr.los = los;
    }
    tracks.retain(|e, _| alive.contains(e));
}
