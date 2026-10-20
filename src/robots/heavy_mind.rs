//! What the heavy thinks, and when it's sent.
//!
//! **Sent** (`extraction_core::heavy::call_step`): when the whole Zone has
//! been looking at someone a while (attention felt at its worst, held),
//! or when someone stands near a Gate with a valley key. Never two at
//! once, never soon after the last, at most two a raid, never with the
//! director off (the recall, `--no-robots`). A horn sounds from the Gate
//! and the radio hears it ([`HeavyCalled`]); twenty seconds later it walks
//! out, sent to where you *are*.
//!
//! **Hunting** it is slow and doesn't hide: no cover, no gun. It comes
//! straight at you at its best pace; in reach, its arm rises (the tell,
//! [`super::heavy::RAISE_TIME`]) and comes down where you stood. Out of
//! sight it goes to where it last saw you and searches like any robot
//! (`search`), patiently. Indoors is no refuge: it goes to the door, tears
//! it off, and reaches in; it can't get in itself, and it tears at the
//! walls ([`super::heavy::HeavyTear`]). It gives up when it can't find you
//! and walks home; or when its hunt is over.

use bevy::prelude::*;
use extraction_core::heavy::{self as rules, Call, CallBook, CallInput};

use super::heavy::{ARM_REACH, Arm, ArmOrder, ArmState, Heavy, HeavyTear, SHOULDER, spawn_heavy};
use super::indoor::NavRef;
use super::search::{self, Step};
use super::walker::{Nav, Walker};
use super::{Awareness, Brain, Intent, Job, JobKind, Robot};
use crate::world::Zone;

/// Suspicion at which it goes for you.
pub const ENGAGE_AT: f32 = 0.5;
/// Out of sight this long in a fight, it goes to where it last saw you.
pub const PATIENCE: f32 = 12.0;
/// How long it hunts before it goes home, s.
pub const HUNT_TIME: f32 = 300.0;
/// Its pace hunting (against its walk).
pub const HUNT_PACE: f32 = 1.3;
/// Between blows, s (after the arm's back at rest).
pub const STRIKE_GAP: f32 = 1.2;
/// A door: it stands this far out from the doorway (it's wide).
pub const DOOR_STAND: f32 = 3.2;
/// Tearing at a wall, this often, s.
pub const TEAR_EVERY: f32 = 6.0;
/// Someone this close to a Gate with a valley key threatens the depot.
pub const GATE_RADIUS: f32 = 60.0;

/// The heavy's own thinking, beside the shared [`Brain`].
#[derive(Component, Debug, Clone)]
pub struct HeavyMind {
    /// Seconds of hunting left.
    pub hunt: f32,
    /// Seconds before it may strike again.
    pub strike_gap: f32,
    /// Seconds before it tears at a wall again.
    pub tear_gap: f32,
}

impl Default for HeavyMind {
    fn default() -> Self {
        Self {
            hunt: HUNT_TIME,
            strike_gap: 0.0,
            tear_gap: 0.0,
        }
    }
}

/// The director called a heavy (for the horn from the Gate, the radio, the
/// wrist). It walks out [`rules::LEAD`] seconds later.
#[derive(Message, Clone, Copy, Debug)]
pub struct HeavyCalled {
    pub gate: Vec3,
    /// Where it's being sent.
    pub to: Vec3,
    pub why: Call,
}

/// The director's book on heavies, and one waiting to walk out.
#[derive(Resource, Debug, Default)]
pub struct HeavyCalls {
    pub book: CallBook,
    /// Seconds until it walks out, and where it's sent.
    pub pending: Option<(f32, Vec3)>,
    /// Every call this raid (raid seconds, why): for the sims.
    pub log: Vec<(f32, Call)>,
}

pub struct HeavyMindPlugin;

impl Plugin for HeavyMindPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HeavyCalled>()
            .init_resource::<HeavyCalls>()
            .add_systems(
                FixedUpdate,
                (
                    heavy_brain.after(super::perceive).before(super::anim::walk),
                    call_heavy.after(super::perceive),
                    stage_scenario,
                ),
            );
    }
}

/// Distance on the ground.
fn flat(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn heavy_brain(
    mut commands: Commands,
    mut heavies: Query<
        (
            Entity,
            &mut Walker,
            &mut Brain,
            &mut Job,
            &Awareness,
            &Transform,
            &Robot,
            &mut Intent,
            &Arm,
            Option<&mut HeavyMind>,
            Has<super::Stunned>,
        ),
        With<Heavy>,
    >,
    targets: Query<&GlobalTransform>,
    nav: Res<Nav>,
    entrances: Res<super::doors::Entrances>,
    doors: Query<&crate::world::buildings::Door>,
    mut orders: MessageWriter<ArmOrder>,
    mut tears: MessageWriter<HeavyTear>,
    spatial: avian3d::prelude::SpatialQuery,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let sweep = time.elapsed_secs() * 0.6;
    let navr = NavRef::Zone(&nav.0);
    for (e, mut w, mut brain, mut job, aware, t, robot, mut intent, arm, mind, stunned) in
        &mut heavies
    {
        let Some(mut mind) = mind else {
            commands.entity(e).insert(HeavyMind::default());
            continue;
        };
        if robot.wrecked {
            *brain = Brain::Down;
            w.goal = None;
            w.path.clear();
            continue;
        }
        *intent = Intent::default();
        if stunned {
            w.goal = None;
            w.path.clear();
            continue;
        }
        let pos = t.translation;
        mind.strike_gap -= dt;
        mind.tear_gap -= dt;
        if !matches!(*brain, Brain::Leave { .. }) {
            mind.hunt -= dt;
            if mind.hunt <= 0.0 {
                *brain = Brain::Leave { to: job.exit };
            }
        }
        if !matches!(*brain, Brain::Leave { .. }) {
            search::react(&mut brain, aware, ENGAGE_AT);
            search::lost_sight(&mut brain, aware, PATIENCE, pos);
        }
        let arm_free = arm.alive && arm.state == ArmState::Rest && mind.strike_gap <= 0.0;
        if arm.state != ArmState::Rest {
            // (The gap runs from when the arm's back.)
            mind.strike_gap = STRIKE_GAP;
        }
        // Where the one it wants is, if indoors.
        let mut indoors: Option<Vec3> = None;
        let goal = match &mut *brain {
            Brain::Transit { to } => {
                let to = *to;
                intent.pace = 1.0;
                if flat(pos, to) < 4.0 {
                    *brain = search::begin_search(to, aware, pos, navr, &entrances);
                    None
                } else {
                    Some(to)
                }
            }
            Brain::Working { left, .. } => {
                *left -= dt;
                if *left <= 0.0 {
                    *brain = Brain::Leave { to: job.exit };
                }
                None
            }
            Brain::Investigate { at, left } => {
                *left -= dt;
                let at = *at;
                intent.pace = 1.0;
                intent.look = Some(at);
                indoors = Some(at);
                let there = flat(pos, at) < 4.0 && entrances.building_at(at).is_none();
                if there || *left <= 0.0 {
                    *brain = search::begin_search(at, aware, pos, navr, &entrances);
                    None
                } else {
                    Some(at)
                }
            }
            Brain::Search { .. } => {
                intent.pace = 0.8;
                match search::search_step(&mut brain, pos, 3.0, dt, sweep) {
                    Step::Go(p) => Some(p),
                    Step::Look(l) => {
                        intent.look = Some(l);
                        None
                    }
                    Step::Done => {
                        search::give_up(&mut brain, &mut job);
                        None
                    }
                }
            }
            Brain::Engage => {
                let seen = aware
                    .target
                    .filter(|_| aware.can_see_target)
                    .and_then(|p| targets.get(p).ok())
                    .map(|g| g.translation());
                match (seen, aware.last_known) {
                    (Some(tp), _) => {
                        intent.pace = HUNT_PACE;
                        intent.look = Some(tp);
                        indoors = Some(tp);
                        let shoulder = t.transform_point(SHOULDER);
                        if arm_free
                            && flat(shoulder, tp) < ARM_REACH * 0.9
                            && entrances.building_at(tp).is_none()
                        {
                            orders.write(ArmOrder::Strike { heavy: e, at: tp });
                            mind.strike_gap = STRIKE_GAP;
                        }
                        // Right on top of you, it stands and swings.
                        (flat(pos, tp) > 2.5).then_some(tp)
                    }
                    (None, Some(lk)) => {
                        indoors = Some(lk);
                        intent.pace = HUNT_PACE;
                        intent.look = Some(lk);
                        if flat(pos, lk) < 4.0 && entrances.building_at(lk).is_none() {
                            *brain = search::begin_search(lk, aware, pos, navr, &entrances);
                            None
                        } else {
                            Some(lk)
                        }
                    }
                    (None, None) => {
                        *brain = Brain::Investigate {
                            at: pos,
                            left: search::INVESTIGATE_TIME,
                        };
                        None
                    }
                }
            }
            Brain::Leave { to } => {
                let to = *to;
                intent.pace = 1.0;
                let d = flat(pos, to);
                if d < 6.0 || (d < 16.0 && w.goal == Some(to) && w.path.is_empty()) {
                    // Back through the Gate.
                    commands.entity(e).despawn();
                    continue;
                }
                Some(to)
            }
            Brain::Down => None,
        };
        // Someone indoors: the door. Tear it off and reach in; tear at the
        // walls.
        let door = indoors
            .and_then(|at| entrances.building_at(at).map(|b| (at, b)))
            .and_then(|(at, b)| {
                entrances
                    .approach(b, pos, |d| super::doors::is_open(doors.get(d).ok()))
                    .map(|(ent, _)| (at, b, ent))
            });
        let goal = match door {
            Some((at, building, ent)) => {
                let stand = ent.mid + ent.out * DOOR_STAND;
                let close = flat(pos, stand) < 2.5;
                if close {
                    intent.look = Some(ent.mid - ent.out * 3.0);
                    let shoulder = t.transform_point(SHOULDER);
                    if arm_free {
                        if doors.get(ent.door).is_ok() {
                            orders.write(ArmOrder::Tear {
                                heavy: e,
                                door: ent.door,
                                at: ent.mid,
                            });
                            mind.strike_gap = STRIKE_GAP;
                        } else if aware.can_see_target && flat(shoulder, at) < ARM_REACH * 0.9 {
                            orders.write(ArmOrder::Strike { heavy: e, at });
                            mind.strike_gap = STRIKE_GAP;
                        }
                    }
                    if mind.tear_gap <= 0.0 {
                        mind.tear_gap = TEAR_EVERY;
                        // The wall between it and you.
                        if let Some((at, out)) = entrances
                            .shells
                            .iter()
                            .find(|s| s.building == building)
                            .and_then(|shell| tear_point(shell, at, pos, &spatial))
                        {
                            tears.write(HeavyTear {
                                heavy: e,
                                building,
                                at,
                                out,
                            });
                        }
                    }
                    None
                } else {
                    Some(stand)
                }
            }
            None => goal,
        };
        w.repath -= dt;
        if goal != w.goal || (goal.is_some() && w.repath <= 0.0 && w.path.is_empty()) {
            w.goal = goal;
            w.repath = 3.0;
            w.path = goal.and_then(|g| navr.path(pos, g)).unwrap_or_default();
        }
    }
}

/// Where to tear at `shell` to get at `target` inside it, from `heavy`
/// outside: the outer wall the line from the target toward it leaves the
/// building by, at chest height, a few centimetres into the wall (where a
/// ray from outside meets it). And the way to pull: out toward the heavy.
pub fn tear_point(
    shell: &super::doors::Shell,
    target: Vec3,
    heavy: Vec3,
    spatial: &avian3d::prelude::SpatialQuery,
) -> Option<(Vec3, Vec3)> {
    use avian3d::prelude::SpatialQueryFilter;
    let inv = shell.xf.compute_affine().inverse();
    let local = inv.transform_point3(target);
    let toward = inv
        .transform_vector3((heavy - target).with_y(0.0))
        .normalize_or_zero();
    if toward == Vec3::ZERO {
        return None;
    }
    // Leave the box's footprint along `toward`.
    let mut t = f32::INFINITY;
    for (p, d, h) in [
        (local.x, toward.x, shell.half.x),
        (local.z, toward.z, shell.half.y),
    ] {
        if d.abs() > 1e-5 {
            t = t.min(((h * d.signum()) - p) / d);
        }
    }
    if !t.is_finite() {
        return None;
    }
    let exit = shell
        .xf
        .compute_affine()
        .transform_point3(local + toward * t);
    let out = (heavy - target).with_y(0.0).normalize_or_zero();
    let chest = Vec3::new(exit.x, target.y + 1.0, exit.z);
    let dir = Dir3::new(-out).ok()?;
    let filter = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
    // From a metre outside, back in, onto the wall's face; where that's an
    // opening (the doorway it came in by), beside it along the same wall.
    let along = out.cross(Vec3::Y);
    // (The box can run past the wall, a porch: go in as far as the target.)
    let reach = 1.0 + Vec2::new(chest.x - target.x, chest.z - target.z).length();
    [0.0, 1.6, -1.6, 2.6, -2.6].into_iter().find_map(|side| {
        let from = chest + out * 1.0 + along * side;
        let hit = spatial.cast_ray(from, dir, reach, true, &filter)?;
        Some((from - out * (hit.distance + 0.08), out))
    })
}

/// Whether someone carrying a valley key stands near a Gate.
fn gate_threat(gate: Vec3, players: &[Vec3], key_carried: bool) -> bool {
    key_carried && players.iter().any(|p| flat(*p, gate) < GATE_RADIUS)
}

/// The director's side: watch for a call, sound the horn, send it out.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn call_heavy(
    mut commands: Commands,
    mut calls: ResMut<HeavyCalls>,
    mut director: ResMut<super::director::Director>,
    zone: Res<Zone>,
    catalog: Res<crate::items::Catalog>,
    players: Query<
        (&GlobalTransform, Option<&crate::anomaly::Attention>),
        With<crate::player::Player>,
    >,
    keys: Query<(
        &crate::items::Item,
        Has<crate::interact::Held>,
        Has<crate::interact::body::InBackpack>,
        Has<crate::interact::InSocket>,
    )>,
    heavies: Query<&Robot, With<Heavy>>,
    mut called: MessageWriter<HeavyCalled>,
    mut raid: ResMut<crate::raid::Raid>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let gate = super::director::gate(&zone);
    let spots: Vec<Vec3> = players.iter().map(|(g, _)| g.translation()).collect();
    let feeling = players
        .iter()
        .filter_map(|(_, a)| a.map(|a| a.feeling()))
        .max()
        .unwrap_or(0);
    let key_kinds: Vec<_> = crate::capstone::KEYS
        .iter()
        .filter_map(|(id, _)| catalog.kind(id))
        .collect();
    let carried = keys
        .iter()
        .any(|(it, held, bag, socket)| key_kinds.contains(&it.kind()) && (held || bag || socket));
    let input = CallInput {
        dt,
        feeling,
        gate_threat: gate_threat(gate, &spots, carried),
        out: calls.pending.is_some() || heavies.iter().any(|r| !r.wrecked),
        enabled: director.enabled,
    };
    // Where it's sent: the one the Zone is looking at hardest.
    let target = players
        .iter()
        .max_by_key(|(_, a)| a.map_or(0, |a| a.feeling()))
        .map(|(g, _)| g.translation());
    if let Some(why) = rules::call_step(&mut calls.book, input) {
        let to = match why {
            Call::Gate => spots
                .iter()
                .copied()
                .min_by(|a, b| flat(*a, gate).total_cmp(&flat(*b, gate)))
                .unwrap_or(gate),
            Call::Attention => target.unwrap_or(gate),
        };
        info!("director: a heavy called ({why:?}) to {to:.0}");
        let t = calls.book.raid_time;
        calls.log.push((t, why));
        calls.pending = Some((rules::LEAD, to));
        director.chatter.push(to);
        called.write(HeavyCalled { gate, to, why });
        raid.say("A horn sounds from the Gate, long and low. Something big is coming out.");
    }
    if let Some((left, to)) = calls.pending.as_mut() {
        *left -= dt;
        // It's sent to where you are when it walks out.
        if let Some(p) = target {
            *to = p;
        }
        if *left <= 0.0 {
            let to = *to;
            calls.pending = None;
            if director.enabled {
                info!("director: the heavy walks out toward {to:.0}");
                let e = spawn_heavy(
                    &mut commands,
                    &zone,
                    &catalog,
                    gate,
                    Job {
                        kind: JobKind::Response,
                        site: to,
                        duration: HUNT_TIME,
                        exit: gate,
                    },
                );
                commands.entity(e).insert(HeavyMind::default());
            }
        }
    }
}

/// The `heavy_well` scenario (`bot::scenarios`): a heavy on the bot's
/// trail, a well between them. Laid out along whichever way from the bot
/// is open ground the length of it.
pub const SCENARIO: &str = "heavy_well";
/// How far out the well and the heavy are put.
pub const STAGE_WELL: f32 = 13.0;
pub const STAGE_HEAVY: f32 = 42.0;

fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<Nav>,
    catalog: Res<crate::items::Catalog>,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        // The bot's facing first, then round the compass: open ground all
        // the way out, so the heavy walks straight through the well.
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * f)
            .find(|d| {
                (2..=(STAGE_HEAVY as usize + 4)).step_by(2).all(|m| {
                    nav.0
                        .walkable([s.at.x + d.x * m as f32, s.at.z + d.z * m as f32])
                })
            })
            .unwrap_or(f);
        let well = zone.on_ground(s.at.x + dir.x * STAGE_WELL, s.at.z + dir.z * STAGE_WELL);
        let from = zone.on_ground(s.at.x + dir.x * STAGE_HEAVY, s.at.z + dir.z * STAGE_HEAVY);
        info!("scenario: a well at {well:.0}, a heavy at {from:.0}");
        commands.queue(move |world: &mut World| {
            crate::anomaly::spawn_anomaly(
                world,
                extraction_core::anomaly::AnomalyKind::Well,
                well,
                2.0,
                Some(600.0),
            );
        });
        let e = spawn_heavy(
            &mut commands,
            &zone,
            &catalog,
            from,
            Job {
                kind: JobKind::Response,
                site: s.at,
                duration: HUNT_TIME,
                exit: from + (from - s.at).with_y(0.0).normalize_or_zero() * 100.0,
            },
        );
        commands.entity(e).insert(HeavyMind::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_near_the_gate_is_a_threat() {
        let gate = Vec3::new(0.0, 0.0, -280.0);
        let near = [gate + Vec3::X * 30.0];
        let far = [gate + Vec3::Z * 200.0];
        assert!(gate_threat(gate, &near, true));
        assert!(!gate_threat(gate, &far, true));
        assert!(!gate_threat(gate, &near, false));
        assert!(!gate_threat(gate, &[], true));
    }

    /// Whole raids through the call rules, a second a tick, each heavy out
    /// for as long as it hunts: what a career of them looks like. A quiet
    /// raid never sees one; a raid spent carrying artifacts sees one, then
    /// (if it stays out long enough) a second, a long while apart, never
    /// more; a raid that goes for the recall meets one at the Gate.
    #[test]
    fn heavies_keep_their_distance_over_a_raid() {
        // (raid seconds, feeling at that second, someone at the Gate with a key)
        let run = |len: usize, feeling: &dyn Fn(usize) -> usize, gate: &dyn Fn(usize) -> bool| {
            let mut book = CallBook::default();
            let mut out_until = 0usize;
            let mut calls = Vec::new();
            for t in 0..len {
                let out = t < out_until;
                let input = CallInput {
                    dt: 1.0,
                    feeling: feeling(t),
                    gate_threat: gate(t),
                    out,
                    enabled: true,
                };
                if let Some(c) = rules::call_step(&mut book, input) {
                    calls.push((t, c));
                    out_until = t + rules::LEAD as usize + HUNT_TIME as usize;
                }
            }
            calls
        };
        let raid = 45 * 60;
        // Quiet: a prickle now and then, a firefight or two (the hum).
        let quiet = run(raid, &|t| if t % 600 < 60 { 3 } else { 1 }, &|_| false);
        assert!(quiet.is_empty(), "{quiet:?}");
        // An artifact field from minute 10, the case open the whole way.
        let hoard = run(raid, &|t| if t > 600 { 4 } else { 1 }, &|_| false);
        assert_eq!(hoard.len(), 2, "{hoard:?}");
        assert!(hoard[0].0 >= 600 + rules::PEAK_HOLD as usize);
        let gap = hoard[1].0 - hoard[0].0;
        assert!(
            gap as f32 >= HUNT_TIME + rules::COOLDOWN,
            "{gap} s between them"
        );
        // A short raid with artifacts: one at most.
        let short = run(20 * 60, &|t| if t > 300 { 4 } else { 1 }, &|_| false);
        assert_eq!(short.len(), 1, "{short:?}");
        // The recall run: three keys to the Gate, half an hour in.
        let recall = run(raid, &|_| 1, &|t| (1800..1900).contains(&t));
        assert_eq!(recall.len(), 1);
        assert_eq!(recall[0].1, Call::Gate);
        assert!(recall[0].0 < 1800 + 10, "at once: {}", recall[0].0);
        // A long career of hoarding raids never sees more than two a raid,
        // and the first never in the opening minutes.
        for start in (0..raid).step_by(97) {
            let calls = run(raid, &|t| if t >= start { 4 } else { 0 }, &|_| false);
            assert!(calls.len() <= rules::PER_RAID as usize);
            if let Some((t, _)) = calls.first() {
                assert!(*t as f32 + 1.0 >= rules::EARLY);
            }
        }
    }

    /// On every building with a door: from the door, someone in the middle
    /// of it, the tear lands on a wall between them (not in the doorway),
    /// inside the wall's thickness (a wall face just outside it), at chest
    /// height, pulling outward.
    #[test]
    fn it_tears_at_the_wall_between_you() {
        use bevy::ecs::system::RunSystemOnce;
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        let checked = app
            .world_mut()
            .run_system_once(
                |entrances: Res<super::super::doors::Entrances>,
                 spatial: avian3d::prelude::SpatialQuery| {
                    let mut checked = 0;
                    let filter = avian3d::prelude::SpatialQueryFilter::from_mask([
                        crate::world::Layer::World,
                    ]);
                    for shell in &entrances.shells {
                        let Some(door) = entrances
                            .doors
                            .iter()
                            .find(|d| d.building == shell.building)
                        else {
                            continue;
                        };
                        let target = shell.xf.translation + Vec3::Y * 0.2;
                        let heavy = door.mid + door.out * DOOR_STAND;
                        let Some((at, out)) = tear_point(shell, target, heavy, &spatial) else {
                            panic!("no tear point on {:?}", shell.building);
                        };
                        // Between you and it, short of you.
                        let flat = |v: Vec3| Vec2::new(v.x, v.z);
                        assert!(
                            flat(at - target).length() > 0.5
                                && flat(at - target).length() < flat(heavy - target).length(),
                            "{at:.1} not between {target:.1} and {heavy:.1}"
                        );
                        assert!((at.y - target.y - 1.0).abs() < 0.01);
                        assert!(out.dot(heavy - target) > 0.0, "pulls outward");
                        // A wall face just outside it.
                        let face = spatial.cast_ray(
                            at + out * 0.5,
                            Dir3::new(-out).unwrap(),
                            0.6,
                            true,
                            &filter,
                        );
                        assert!(face.is_some(), "no wall at {at:.1}");
                        checked += 1;
                    }
                    checked
                },
            )
            .unwrap();
        assert!(checked >= 5, "checked {checked} buildings");
    }
}
