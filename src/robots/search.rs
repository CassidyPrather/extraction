//! Memory and search: what a robot does with what it half-knows.
//!
//! A robot that has heard or glimpsed something **investigates**: it goes
//! to the last-known position ([`Awareness::last_known`]). There it
//! **searches**: it stands and looks round, then sweeps the cover and the
//! doorways nearby (from the nav grid and the buildings' entrances,
//! [`plan_search`]), as far out as its uncertainty says you could have
//! got. Nothing turns up and it **gives up**: back to its job, which it
//! cuts short ([`give_up`]), or out through the Gate if it was sent to
//! find you. Anything fresh — a footstep, a glimpse — sends it there
//! instead. Lose its line of sight in a fight and it does the same,
//! starting where it last saw you.
//!
//! Robots talk: one that notices you calls the others in earshot of its
//! radio ([`CALL_RADIUS`]) to come and look, and a radio you carry picks
//! the chatter up.
//!
//! Each change is announced, for sound and the wrist: [`Investigating`],
//! [`Searching`], [`LostYou`] (and [`super::Noticed`], when it has you).

use super::doors::Entrances;
use super::indoor::NavRef;
use super::{Awareness, Brain, Job, JobKind, NOTICED, Noticed, Robot};
use bevy::prelude::*;

/// Suspicion past which a robot goes to look.
pub const INVESTIGATE_AT: f32 = 0.35;
/// How long it gives itself to get to where it's going to look (s).
pub const INVESTIGATE_TIME: f32 = 30.0;
/// On arrival it stands and looks round this long (s)...
pub const LOOK_TIME: f32 = 3.0;
/// ...then sweeps nearby spots, pausing at each this long (s)...
pub const SPOT_DWELL: f32 = 2.5;
/// ...at most this many...
pub const SEARCH_SPOTS: usize = 5;
/// ...within the uncertainty, clamped to these (m)...
pub const SEARCH_RADIUS: (f32, f32) = (6.0, 18.0);
/// ...for at most this long, all told (s).
pub const SEARCH_TIME: f32 = 40.0;
/// Fresh word this far from where it's looking (beyond the uncertainty)
/// sends it there instead (m).
pub const RETARGET: f32 = 4.0;
/// A robot that notices you calls others within this (m)...
pub const CALL_RADIUS: f32 = 45.0;
/// ...up to this suspicion: they come to look; they don't open fire
/// until they see you themselves.
pub const CALL_SUSPICION: f32 = 0.45;
/// ...and they know where only so well (m added to the caller's
/// uncertainty).
pub const CALL_SPREAD: f32 = 6.0;
/// Each robot calls at most this often (s)...
pub const CALL_COOLDOWN: f32 = 12.0;
/// ...and the radio carries it at most this often (s).
pub const RADIO_COOLDOWN: f32 = 30.0;

/// A robot went to look at something: `toward` is where.
#[derive(Message, Clone, Copy, Debug)]
pub struct Investigating {
    pub robot: Entity,
    pub at: Vec3,
    pub toward: Vec3,
}

/// A robot got there and is searching round it.
#[derive(Message, Clone, Copy, Debug)]
pub struct Searching {
    pub robot: Entity,
    pub at: Vec3,
}

/// A robot gave up looking.
#[derive(Message, Clone, Copy, Debug)]
pub struct LostYou {
    pub robot: Entity,
    pub at: Vec3,
}

/// A robot that noticed someone called the robots near it.
#[derive(Message, Clone, Copy, Debug)]
pub struct CalledOut {
    pub robot: Entity,
    pub at: Vec3,
    /// How many heard it.
    pub heard: usize,
    /// Who it noticed, if it was a player.
    pub about: Option<Entity>,
}

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Investigating>()
            .add_message::<Searching>()
            .add_message::<LostYou>()
            .add_message::<CalledOut>()
            .add_systems(FixedUpdate, call_out.after(super::perceive))
            .add_systems(FixedPostUpdate, announce)
            .add_systems(Update, wrist_lines);
    }
}

/// The alarm, the same for every kind: seen and sure enough, fight;
/// suspicious, go and look; looking and told something new, look there.
/// `engage_at` is the suspicion it opens fire at.
pub fn react(brain: &mut Brain, aware: &Awareness, engage_at: f32) {
    if matches!(brain, Brain::Down) {
        return;
    }
    if aware.can_see_target && aware.suspicion > engage_at {
        if !matches!(brain, Brain::Engage) {
            *brain = Brain::Engage;
        }
        return;
    }
    if aware.suspicion <= INVESTIGATE_AT {
        return;
    }
    let Some(at) = aware.last_known else {
        return;
    };
    let investigate = Brain::Investigate {
        at,
        left: INVESTIGATE_TIME,
    };
    match brain {
        Brain::Transit { .. } | Brain::Working { .. } => *brain = investigate,
        Brain::Investigate { at: was, .. } | Brain::Search { at: was, .. } if aware.news => {
            if flat(*was - at) > RETARGET + aware.uncertainty {
                *brain = investigate;
            } else if let Brain::Investigate { at: was, .. } = brain {
                // Close to what it was told: keep going, a little better aimed.
                *was = at;
            }
        }
        _ => {}
    }
}

/// Seconds out of sight a robot fights on before it goes to look where
/// you were instead.
pub fn lost_sight(brain: &mut Brain, aware: &Awareness, patience: f32, fallback: Vec3) -> bool {
    if matches!(brain, Brain::Engage) && aware.unseen > patience {
        *brain = Brain::Investigate {
            at: aware.last_known.unwrap_or(fallback),
            left: INVESTIGATE_TIME,
        };
        return true;
    }
    false
}

/// Arrived where it was going to look: start searching round it.
pub fn begin_search(at: Vec3, aware: &Awareness, from: Vec3, nav: NavRef, e: &Entrances) -> Brain {
    Brain::Search {
        at,
        left: SEARCH_TIME,
        spots: plan_search(at, aware.uncertainty, from, nav, e),
        dwell: LOOK_TIME,
    }
}

/// What a searching robot does this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Go here.
    Go(Vec3),
    /// Stand and look this way (a point to face).
    Look(Vec3),
    /// Nothing found.
    Done,
}

/// One tick of a search: dwell and look round, then the next spot (it
/// counts as there within `near` m), until the spots or the time run out.
/// `sweep` is an angle that turns as it looks round.
pub fn search_step(brain: &mut Brain, pos: Vec3, near: f32, dt: f32, sweep: f32) -> Step {
    let Brain::Search {
        left, spots, dwell, ..
    } = brain
    else {
        return Step::Done;
    };
    *left -= dt;
    if *left <= 0.0 {
        return Step::Done;
    }
    let look = pos + Vec3::new(sweep.cos(), 0.0, sweep.sin()) * 6.0;
    if *dwell > 0.0 {
        *dwell -= dt;
        return Step::Look(look);
    }
    let Some(next) = spots.first().copied() else {
        return Step::Done;
    };
    if flat(next - pos) < near {
        spots.remove(0);
        *dwell = SPOT_DWELL;
        return Step::Look(look);
    }
    Step::Go(next)
}

/// Nothing found: back to the job, cut short (so a robot that keeps being
/// distracted does leave), or home if finding you was the job.
pub fn give_up(brain: &mut Brain, job: &mut Job) {
    if matches!(job.kind, JobKind::Response) || job.duration < 20.0 {
        *brain = Brain::Leave { to: job.exit };
    } else {
        job.duration *= 0.5;
        *brain = Brain::Transit { to: job.site };
    }
}

/// Where to look round `at`: the doorways of buildings nearby and the
/// walkable ground hard by obstacles (behind a wall, a wreck, a trunk),
/// within `radius` (clamped to [`SEARCH_RADIUS`]); at most
/// [`SEARCH_SPOTS`], nearest-first from `from`.
pub fn plan_search(
    at: Vec3,
    radius: f32,
    from: Vec3,
    nav: NavRef,
    entrances: &Entrances,
) -> Vec<Vec3> {
    let r = radius.clamp(SEARCH_RADIUS.0, SEARCH_RADIUS.1);
    let mut picks: Vec<Vec3> = Vec::new();
    let mut doors: Vec<Vec3> = entrances
        .doors
        .iter()
        .map(|e| e.mid + e.out * 1.8)
        .filter(|p| flat(*p - at) < r + 4.0)
        .collect();
    doors.sort_by(|a, b| flat(*a - at).total_cmp(&flat(*b - at)));
    picks.extend(doors.into_iter().take(2));
    let blocked = |p: Vec3| !nav.walkable(p);
    for ring in [0.45, 1.0] {
        for k in 0..12 {
            let a = k as f32 * std::f32::consts::TAU / 12.0 + ring;
            let p = at + Vec3::new(a.cos(), 0.0, a.sin()) * r * ring;
            if blocked(p) {
                continue;
            }
            let cover = (0..8).any(|j| {
                let b = j as f32 * std::f32::consts::FRAC_PI_4;
                blocked(p + Vec3::new(b.cos(), 0.0, b.sin()) * 1.6)
            });
            if cover {
                picks.push(p);
            }
        }
    }
    // Open ground, if there's no cover at all: a few points round about.
    if picks.is_empty() {
        for k in 0..4 {
            let a = k as f32 * std::f32::consts::FRAC_PI_2 + 0.4;
            let p = at + Vec3::new(a.cos(), 0.0, a.sin()) * r * 0.6;
            if !blocked(p) {
                picks.push(p);
            }
        }
    }
    picks.dedup_by(|a, b| flat(*a - *b) < 3.0);
    // Nearest-first tour from where it stands.
    let mut tour = Vec::new();
    let mut here = from;
    while !picks.is_empty() && tour.len() < SEARCH_SPOTS {
        let i = (0..picks.len())
            .min_by(|&i, &j| flat(picks[i] - here).total_cmp(&flat(picks[j] - here)))
            .unwrap();
        let p = picks.swap_remove(i);
        if tour.iter().all(|q: &Vec3| flat(*q - p) > 3.0) {
            tour.push(p);
            here = p;
        }
    }
    tour
}

fn flat(v: Vec3) -> f32 {
    Vec2::new(v.x, v.z).length()
}

/// Robots that notice someone call the others near them.
fn call_out(
    mut noticed: MessageReader<Noticed>,
    mut robots: Query<(Entity, &Robot, &GlobalTransform, &mut Awareness, &Job)>,
    mut last: Local<std::collections::HashMap<Entity, f32>>,
    mut radio_at: Local<f32>,
    time: Res<Time>,
    director: Option<ResMut<super::director::Director>>,
    mut out: MessageWriter<CalledOut>,
) {
    let now = time.elapsed_secs();
    let mut director = director;
    for n in noticed.read() {
        if last.get(&n.robot).is_some_and(|t| now - t < CALL_COOLDOWN) {
            continue;
        }
        let Ok((_, _, gt, aware, job)) = robots.get(n.robot) else {
            continue;
        };
        // The convoy's collector keeps itself to itself.
        if job.kind == JobKind::Collect {
            continue;
        }
        let (Some(at), target, unc) = (aware.last_known, aware.target, aware.uncertainty) else {
            continue;
        };
        let me = gt.translation();
        last.insert(n.robot, now);
        let mut heard = 0;
        for (e, robot, ogt, mut other, ojob) in &mut robots {
            if e == n.robot
                || robot.wrecked
                || ojob.kind == JobKind::Collect
                || ogt.translation().distance(me) > CALL_RADIUS
                || other.suspicion >= NOTICED
            {
                continue;
            }
            other.suspicion = other.suspicion.max(CALL_SUSPICION);
            other.last_known = Some(at);
            other.uncertainty = unc + CALL_SPREAD;
            other.news = true;
            if other.target.is_none() {
                other.target = target;
            }
            heard += 1;
        }
        out.write(CalledOut {
            robot: n.robot,
            at: me,
            heard,
            about: target,
        });
        // The chatter carries to a radio you have on you.
        if heard > 0 && (*radio_at == 0.0 || now - *radio_at > RADIO_COOLDOWN) {
            if let Some(d) = director.as_deref_mut() {
                d.chatter.push(at);
                *radio_at = now;
            }
        }
    }
}

/// A line on the wrist only within this of you (m)...
pub const LINE_NEAR: f32 = 30.0;
/// ...at most once a minute per robot (s)...
pub const LINE_PER_ROBOT: f32 = 60.0;
/// ...and once in this long, all told (s).
pub const LINE_ANY: f32 = 20.0;

/// What the wrist says when a robot close by gives up on you, or calls
/// the others about you. The words are Story's (`raid::lines`); the `n`th
/// line of each pool, in turn.
fn line(lost: bool, n: u32) -> &'static str {
    // At merge: `crate::raid::lines::pick(LOST_YOU | CALLED_OUT, n)`.
    const LOST_YOU: [&str; 1] = ["The whirr fades. It's stopped looking."];
    const CALLED_OUT: [&str; 1] = ["Chirps answer each other close by."];
    let pool: &[&str] = if lost { &LOST_YOU } else { &CALLED_OUT };
    pool[n as usize % pool.len()]
}

/// Says, rarely, when a robot close by that was after you gives up, or
/// calls the others about you.
#[allow(clippy::too_many_arguments)]
fn wrist_lines(
    mut lost: MessageReader<LostYou>,
    mut called: MessageReader<CalledOut>,
    players: Query<(Entity, &GlobalTransform), With<crate::rig::LocalPlayer>>,
    aware: Query<&Awareness>,
    raid: Option<ResMut<crate::raid::Raid>>,
    time: Res<Time>,
    mut said: Local<(std::collections::HashMap<Entity, f32>, f32, u32)>,
) {
    let events: Vec<(bool, Entity, Vec3, Option<Entity>)> = lost
        .read()
        .map(|l| {
            (
                true,
                l.robot,
                l.at,
                aware.get(l.robot).ok().and_then(|a| a.target),
            )
        })
        .chain(called.read().map(|c| (false, c.robot, c.at, c.about)))
        .collect();
    let (Some(mut raid), Ok((me, gt))) = (raid, players.single()) else {
        return;
    };
    if raid.phase == crate::raid::Phase::AtBase {
        return;
    }
    let now = time.elapsed_secs();
    let (per_robot, last, n) = &mut *said;
    for (is_lost, robot, at, about) in events {
        if about != Some(me)
            || at.distance(gt.translation()) > LINE_NEAR
            || (*last > 0.0 && now - *last < LINE_ANY)
            || per_robot
                .get(&robot)
                .is_some_and(|t| now - t < LINE_PER_ROBOT)
        {
            continue;
        }
        raid.say(line(is_lost, *n));
        *n += 1;
        *last = now;
        per_robot.insert(robot, now);
    }
}

/// Says when a robot starts to look, to search, and gives up.
fn announce(
    robots: Query<(Entity, &Brain, &GlobalTransform)>,
    mut was: Local<std::collections::HashMap<Entity, u8>>,
    mut investigating: MessageWriter<Investigating>,
    mut searching: MessageWriter<Searching>,
    mut lost: MessageWriter<LostYou>,
) {
    let mut seen = std::collections::HashSet::new();
    for (e, brain, gt) in &robots {
        seen.insert(e);
        let code = match brain {
            Brain::Transit { .. } => 0,
            Brain::Working { .. } => 1,
            Brain::Investigate { .. } => 2,
            Brain::Search { .. } => 3,
            Brain::Engage => 4,
            Brain::Leave { .. } => 5,
            Brain::Down => 6,
        };
        let before = was.insert(e, code);
        let at = gt.translation();
        match (before, brain) {
            (Some(b), Brain::Investigate { at: toward, .. }) if b != 2 => {
                investigating.write(Investigating {
                    robot: e,
                    at,
                    toward: *toward,
                });
            }
            (Some(b), Brain::Search { .. }) if b != 3 => {
                searching.write(Searching { robot: e, at });
            }
            (Some(2 | 3), Brain::Transit { .. } | Brain::Leave { .. }) => {
                lost.write(LostYou { robot: e, at });
            }
            _ => {}
        }
    }
    was.retain(|e, _| seen.contains(e));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_dwells_sweeps_and_ends() {
        let mut b = Brain::Search {
            at: Vec3::ZERO,
            left: SEARCH_TIME,
            spots: vec![Vec3::new(10.0, 0.0, 0.0)],
            dwell: LOOK_TIME,
        };
        let dt = 0.1;
        assert!(matches!(
            search_step(&mut b, Vec3::ZERO, 1.0, dt, 0.0),
            Step::Look(_)
        ));
        for _ in 0..40 {
            search_step(&mut b, Vec3::ZERO, 1.0, dt, 0.0);
        }
        assert_eq!(
            search_step(&mut b, Vec3::ZERO, 1.0, dt, 0.0),
            Step::Go(Vec3::new(10.0, 0.0, 0.0))
        );
        let at = Vec3::new(9.5, 0.0, 0.0);
        assert!(matches!(
            search_step(&mut b, at, 1.0, dt, 0.0),
            Step::Look(_)
        ));
        for _ in 0..30 {
            search_step(&mut b, at, 1.0, dt, 0.0);
        }
        assert_eq!(search_step(&mut b, at, 1.0, dt, 0.0), Step::Done);
    }

    #[test]
    fn fresh_word_elsewhere_redirects_a_search() {
        let mut b = Brain::Search {
            at: Vec3::ZERO,
            left: SEARCH_TIME,
            spots: vec![],
            dwell: 0.0,
        };
        let mut aware = Awareness {
            suspicion: 0.5,
            last_known: Some(Vec3::new(2.0, 0.0, 0.0)),
            uncertainty: 2.0,
            news: true,
            ..default()
        };
        react(&mut b, &aware, 0.6);
        assert!(
            matches!(b, Brain::Search { .. }),
            "close by: keep searching"
        );
        aware.last_known = Some(Vec3::new(30.0, 0.0, 0.0));
        react(&mut b, &aware, 0.6);
        assert!(matches!(b, Brain::Investigate { .. }), "far: go there");
    }
}
