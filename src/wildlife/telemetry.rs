//! The sim's record of the animals (`docs/SIMULATION.md`): every flock that
//! went up, and every robot an alarm sent to look. What the pacing check
//! in `docs/WILDLIFE.md` counts.

use bevy::prelude::*;
use serde_json::json;

use super::Flushed;
use crate::robots::search::Investigating;
use crate::sim::telemetry::{SimLog, round};

/// An investigation this soon after an alarm, toward where it was, was the
/// birds' doing (s, m).
const BLAME_FOR: f32 = 5.0;
const BLAME_WITHIN: f32 = 10.0;

pub(crate) fn log_wildlife(
    log: Option<ResMut<SimLog>>,
    mut flushed: MessageReader<Flushed>,
    mut investigating: MessageReader<Investigating>,
    robots: Query<&crate::robots::Robot>,
    time: Res<Time>,
    mut alarms: Local<Vec<(f32, Vec3)>>,
) {
    let Some(mut log) = log else {
        flushed.clear();
        investigating.clear();
        return;
    };
    let now = time.elapsed_secs();
    alarms.retain(|(t, _)| now - *t < BLAME_FOR);
    for f in flushed.read() {
        if f.alarm {
            alarms.push((now, f.at));
        }
        log.event(
            "flush",
            json!({"cause": format!("{:?}", f.cause), "birds": f.birds, "alarm": f.alarm,
                   "x": round(f.at.x, 1), "z": round(f.at.z, 1)}),
        );
    }
    for i in investigating.read() {
        let Some(&(_, at)) = alarms
            .iter()
            .find(|(_, a)| Vec2::new(a.x - i.toward.x, a.z - i.toward.z).length() < BLAME_WITHIN)
        else {
            continue;
        };
        let kind = robots
            .get(i.robot)
            .map(|r| format!("{:?}", r.kind))
            .unwrap_or_default();
        log.event(
            "bird_alarm",
            json!({"robot": kind, "x": round(at.x, 1), "z": round(at.z, 1),
                   "dist": round(i.at.distance(at), 1)}),
        );
    }
}
