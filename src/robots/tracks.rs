//! Robots read footprints in the snow (`world::snow::Tracks`).
//!
//! A robot that can't see you, coming within sight of a fresh print of
//! yours ([`FRESH`]), reads the trail on from it, as far as it can make
//! out ([`LEAD`] along it, while the prints are warmer than [`COLD`]),
//! and goes to look there: it isn't sure (the lead is a guess, with
//! [`TRAIL_UNCERTAINTY`]), so it investigates, not fights. Arrived, it
//! sees the next prints and reads on, so it walks your trail to you,
//! unless the snow has filled it in. A trail that stops at a plough's
//! fresh lane went on along it (`signs::along_lane`). Prints are small: it makes them out
//! at [`TRACK_SIGHT`] by day, less as the light goes or the weather
//! closes in (`Conditions::sight`).
//!
//! Cost: at most one look a robot every [`LOOK_EVERY`] seconds, a scan
//! of the prints (at most `snow::MAX_PRINTS`) and one ray.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Awareness, Robot, Senses};
use crate::world::Layer;
use crate::world::snow::{Print, Tracks};

/// A print fresher than this (fill, 0 new .. 1 gone) catches its eye...
pub const FRESH: f32 = 0.5;
/// ...and it reads the trail on while the prints are fresher than this...
pub const COLD: f32 = 0.7;
/// ...as far along as this (m)...
pub const LEAD: f32 = 15.0;
/// ...not sure of it: this uncertain (m).
pub const TRAIL_UNCERTAINTY: f32 = 2.5;
/// It makes a print out at this far in clear daylight (m)...
pub const TRACK_SIGHT: f32 = 12.0;
/// ...looking every this many seconds.
pub const LOOK_EVERY: f32 = 0.5;
/// A trail raises its suspicion to this (enough to go and look).
pub const TRAIL_SUSPICION: f32 = super::search::INVESTIGATE_AT + 0.1;

pub struct TracksPlugin;

impl Plugin for TracksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, read_tracks.after(super::perceive));
    }
}

/// Where the trail from `start` leads, as far as a robot reads it: on
/// along the same walker's prints, while they're fresher than [`COLD`],
/// up to [`LEAD`] metres of trail.
pub fn read_on(tracks: &Tracks, start: &Print) -> Print {
    let mut at = *start;
    let mut walked = 0.0;
    while let Some(next) = tracks.after(&at) {
        if next.fill >= COLD {
            break;
        }
        walked += next.at.distance(at.at);
        if walked > LEAD {
            break;
        }
        at = *next;
    }
    at
}

fn read_tracks(
    mut robots: Query<(Entity, &Robot, &Senses, &GlobalTransform, &mut Awareness)>,
    players: Query<(), With<crate::player::Player>>,
    tracks: Option<Res<Tracks>>,
    (lanes, snow): (
        Option<Res<super::plough::Lanes>>,
        Option<Res<crate::world::snow::Snow>>,
    ),
    conditions: Res<crate::world::weather::Conditions>,
    spatial: SpatialQuery,
    time: Res<Time>,
    mut last: Local<std::collections::HashMap<Entity, f32>>,
) {
    let Some(tracks) = tracks.filter(|t| !t.is_empty()) else {
        return;
    };
    let now = time.elapsed_secs();
    let reach = conditions.sight(TRACK_SIGHT);
    let world = SpatialQueryFilter::from_mask([Layer::World]);
    last.retain(|e, _| robots.contains(*e));
    for (e, robot, senses, gt, mut aware) in &mut robots {
        if robot.wrecked || aware.can_see_target {
            continue;
        }
        if last.get(&e).is_some_and(|t| now - t < LOOK_EVERY) {
            continue;
        }
        last.insert(e, now);
        let eye = gt.transform_point(senses.eye);
        let forward = gt.forward().as_vec3();
        // The freshest print of yours it can make out from here.
        let seen = tracks
            .prints()
            .rev()
            .filter(|p| p.fill < FRESH && players.contains(p.by))
            .find(|p| {
                let to = p.at + Vec3::Y * 0.05 - eye;
                let d = to.length();
                if d > reach || d < 0.01 {
                    return false;
                }
                let angle = forward.angle_between(to / d).to_degrees();
                if angle > senses.fov_deg * 0.5 {
                    return false;
                }
                Dir3::new(to)
                    .ok()
                    .is_some_and(|dir| spatial.cast_ray(eye, dir, d - 0.2, true, &world).is_none())
            })
            .copied();
        let Some(print) = seen else {
            continue;
        };
        let lead = read_on(&tracks, &print);
        // Where the trail stops at a fresh lane, on along the lane.
        let heading = if lead.at.distance(print.at) > 0.5 {
            (lead.at - print.at).xz()
        } else {
            Vec2::new(-lead.yaw.sin(), -lead.yaw.cos())
        };
        let lane = lanes
            .as_ref()
            .filter(|_| tracks.after(&lead).is_none())
            .and_then(|l| {
                let falling = snow.as_ref().map_or(0.0, |s| s.falling);
                super::signs::along_lane(l, now, falling, lead.at, heading)
            });
        let (guess, uncertainty) = match lane {
            Some(at) => (at, super::signs::LANE_UNCERTAINTY),
            None => (lead.at, TRAIL_UNCERTAINTY),
        };
        // A lead further along than what it had, or a first one.
        let better = aware
            .last_known
            .is_none_or(|lk| lk.distance(guess) > 1.0 || aware.uncertainty > uncertainty);
        if !better {
            continue;
        }
        aware.last_known = Some(guess);
        aware.uncertainty = uncertainty;
        aware.suspicion = aware.suspicion.max(TRAIL_SUSPICION);
        aware.news = true;
        if aware.target.is_none() {
            aware.target = Some(lead.by);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn print(at: Vec3, by: Entity, fill: f32) -> Print {
        Print {
            at,
            yaw: 0.0,
            left: false,
            by,
            fill,
        }
    }

    /// The trail is read on from a print, along the same walker's, until
    /// it's cold or LEAD long.
    #[test]
    fn a_trail_reads_on_until_cold_or_far() {
        let mut w = World::new();
        let (you, other) = (w.spawn_empty().id(), w.spawn_empty().id());
        let mut t = Tracks::default();
        for k in 0..30 {
            t.add(print(Vec3::new(k as f32, 0.0, 0.0), you, 0.3));
            t.add(print(Vec3::new(k as f32, 0.0, 9.0), other, 0.3));
        }
        let start = *t.prints().next().unwrap();
        let lead = read_on(&t, &start);
        assert!(lead.by == you, "your trail, not another's");
        assert!((lead.at.x - LEAD).abs() <= 1.0, "LEAD on: {}", lead.at);
        // Cold further on: it stops there.
        let mut t = Tracks::default();
        for k in 0..30 {
            t.add(print(
                Vec3::new(k as f32, 0.0, 0.0),
                you,
                if k > 5 { 0.9 } else { 0.3 },
            ));
        }
        let start = *t.prints().next().unwrap();
        assert_eq!(read_on(&t, &start).at.x, 5.0);
    }
}
