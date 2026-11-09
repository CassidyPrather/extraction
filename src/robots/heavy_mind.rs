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
//! and walks home; or when its hunt is over; or when you walk away and
//! keep out of its reach ([`LET_GO_RANGE`], [`LET_GO_AFTER`]): it's slow,
//! and walking off is meant to work.

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
/// Walk away and keep out of reach and it lets you go: more than this far
/// off (m), the gap held or opening for this long (s), it gives up and goes
/// back to its work; and for [`LET_GO_FOR`] it leaves you be unless you
/// come back within [`LET_GO_NEAR`].
pub const LET_GO_RANGE: f32 = 10.0;
pub const LET_GO_AFTER: f32 = 8.0;
pub const LET_GO_FOR: f32 = 30.0;
pub const LET_GO_NEAR: f32 = 6.0;
/// How far the gap may close from its widest and the count still run (m):
/// a step back, a sidestep, a headset's bob isn't coming back for it.
pub const LET_GO_GIVE: f32 = 1.0;
/// Let go of you, it first backs off this far straight away from you (m),
/// its back to you, then heads home.
pub const RETREAT: f32 = 15.0;
/// Heading home while it's stood down, it holds where it is while you
/// stand within this (m) and in its way (within [`IN_THE_WAY`] degrees of
/// its line home): it doesn't walk past you to get there.
pub const KEEP_CLEAR: f32 = 25.0;
pub const IN_THE_WAY: f32 = 60.0;
/// Between blows, s (after the arm's back at rest).
pub const STRIKE_GAP: f32 = 1.2;
/// A door: it stands this far out from the doorway (it's wide).
pub const DOOR_STAND: f32 = 3.2;
/// Tearing at a wall, this often, s.
pub const TEAR_EVERY: f32 = 6.0;
/// Someone this close to a Gate with a valley key threatens the depot.
pub const GATE_RADIUS: f32 = 60.0;
/// How far its horn from the Gate is heard (m), calm / in a downpour, and
/// its hum (`heavy.idle`, on it, walking or standing), by the mix's
/// masking model (`scripts/audio_mix.py`, `audible_range`; read 8
/// November). The gauge `every_heavy_reads_before_it_reaches_you` holds
/// them to the valleys and to how far it notices you.
/// (Walls only low-pass a sound here, `audio::occlude`: the horn is D1
/// and A1 and the hum is slowed to 0.55, so neither is muted by them.)
pub const HORN_HEARD: (f32, f32) = (811.0, 811.0);
pub const HUM_HEARD: (f32, f32) = (201.0, 201.0);
/// The horn carries this much further than the furthest ground of any
/// valley, so a map edit doesn't take a corner out of its reach at once.
pub const HORN_MARGIN: f32 = 1.05;

/// Between calm and downpour heard ranges at `rain`, as the dB go (as
/// `senses::heard_coming`).
pub fn heard_at(range: (f32, f32), rain: f32) -> f32 {
    range.0 * (range.1 / range.0).powf(rain.clamp(0.0, 1.0))
}

/// The heavy's own thinking, beside the shared [`Brain`].
#[derive(Component, Debug, Clone)]
pub struct HeavyMind {
    /// Seconds of hunting left.
    pub hunt: f32,
    /// Seconds before it may strike again.
    pub strike_gap: f32,
    /// Seconds before it tears at a wall again.
    pub tear_gap: f32,
    /// Seconds the one it's after has kept out of reach and walked on, and
    /// the widest the gap has been since that count began.
    pub out_of_reach: f32,
    pub best_gap: f32,
    /// Seconds it'll leave someone be after letting them go.
    pub let_go: f32,
    /// It has let someone go and stood down (until it takes someone up
    /// again): its eye low, its back turned (`heavy_look`).
    pub stood_down: bool,
    /// Where it backs off to first, standing down.
    pub retreat: Option<Vec3>,
    /// Where the one it let go was last tick (to tell them coming back
    /// from its own way home bringing it near).
    pub was_at: Option<Vec3>,
}

impl Default for HeavyMind {
    fn default() -> Self {
        Self {
            hunt: HUNT_TIME,
            strike_gap: 0.0,
            tear_gap: 0.0,
            out_of_reach: 0.0,
            best_gap: 0.0,
            let_go: 0.0,
            stood_down: false,
            retreat: None,
            was_at: None,
        }
    }
}

/// A heavy let someone go: walked away from and kept out of its reach, it
/// stands down (for its sigh: Audio's cue; its look is `heavy_look`'s).
#[derive(Message, Clone, Copy, Debug)]
pub struct HeavyStandDown {
    pub heavy: Entity,
    pub at: Vec3,
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

/// The Gate woke: someone with a valley key crossed into its reach
/// ([`GATE_RADIUS`]) and it's counting them ([`rules::counting`]); in
/// [`rules::GATE_HOLD`] it calls a heavy. For its lamp (Visual design) and
/// its rising call (Audio): the Gate saying you're being counted, at the
/// line, while there's time to step back out.
#[derive(Message, Clone, Copy, Debug)]
pub struct GateWake {
    pub gate: Vec3,
}

/// The Gate went quiet: they stepped back out (`called` false), or the
/// count ran out and it called (`called` true; [`HeavyCalled`] follows on
/// the same tick).
#[derive(Message, Clone, Copy, Debug)]
pub struct GateQuiet {
    pub gate: Vec3,
    pub called: bool,
}

/// The Gate's count, for whatever shows it while it lasts: `counting`
/// from its [`GateWake`] to its [`GateQuiet`], and how far through
/// [`rules::GATE_HOLD`] it is (0..1) for a call that rises as it goes.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct GateCount {
    pub gate: Vec3,
    pub counting: bool,
    pub held: f32,
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
            .add_message::<HeavyStandDown>()
            .add_message::<GateWake>()
            .add_message::<GateQuiet>()
            .init_resource::<HeavyCalls>()
            .init_resource::<GateCount>()
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
    mut breaches: MessageWriter<crate::destruction::hooks::Breach>,
    mut stand_downs: MessageWriter<HeavyStandDown>,
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
        mind.let_go = (mind.let_go - dt).max(0.0);
        // Let go of: it leaves you be unless you come back up close: you,
        // not its own way home bringing it by you.
        let them = aware
            .target
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation());
        let came_back = them.filter(|_| aware.can_see_target).is_some_and(|p| {
            let closer = mind
                .was_at
                .is_some_and(|w| flat(pos, p) < flat(pos, w) - 1.0e-4);
            flat(pos, p) < LET_GO_NEAR && closer
        });
        mind.was_at = them;
        if !matches!(*brain, Brain::Leave { .. }) && (mind.let_go <= 0.0 || came_back) {
            search::react(&mut brain, aware, ENGAGE_AT);
            search::lost_sight(&mut brain, aware, PATIENCE, pos);
            if matches!(*brain, Brain::Engage) {
                mind.stood_down = false;
                mind.retreat = None;
            }
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
                        // Walked away and kept out of reach: it lets you go.
                        let gap = flat(pos, tp);
                        if gap > LET_GO_RANGE && gap >= mind.best_gap - LET_GO_GIVE {
                            mind.out_of_reach += dt;
                            mind.best_gap = mind.best_gap.max(gap);
                        } else {
                            mind.out_of_reach = 0.0;
                            mind.best_gap = gap;
                        }
                        if mind.out_of_reach >= LET_GO_AFTER {
                            mind.out_of_reach = 0.0;
                            mind.best_gap = 0.0;
                            mind.let_go = LET_GO_FOR;
                            mind.stood_down = true;
                            let away =
                                Vec3::new(pos.x - tp.x, 0.0, pos.z - tp.z).normalize_or(Vec3::X);
                            let back = pos + away * RETREAT;
                            mind.retreat = Some(
                                nav.0
                                    .nearest_walkable([back.x, back.z])
                                    .map_or(back, |p| Vec3::new(p[0], pos.y, p[1])),
                            );
                            stand_downs.write(HeavyStandDown { heavy: e, at: pos });
                            search::give_up(&mut brain, &mut job);
                            continue;
                        }
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
        // Stood down: its back to you, off and away first; then home, but
        // not past you: while you're near and in its way, it holds.
        let goal = if mind.stood_down && !matches!(*brain, Brain::Engage | Brain::Down) {
            match mind.retreat {
                Some(r) if flat(pos, r) > 2.5 => {
                    intent.pace = 1.0;
                    intent.look = Some(r);
                    Some(r)
                }
                _ => {
                    mind.retreat = None;
                    let blocked = goal.zip(them).is_some_and(|(home, p)| {
                        let to_home = Vec2::new(home.x - pos.x, home.z - pos.z);
                        let to_them = Vec2::new(p.x - pos.x, p.z - pos.z);
                        to_them.length() < KEEP_CLEAR
                            && to_home.angle_to(to_them).abs().to_degrees() < IN_THE_WAY
                    });
                    if blocked {
                        // Facing home, waiting for you to be on your way.
                        intent.look = goal;
                        None
                    } else {
                        if goal.is_some() {
                            intent.look = goal;
                        }
                        goal
                    }
                }
            }
        } else {
            goal
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
                            // A doorway-sized hole where it pulls (wood,
                            // plaster and tin tear; brick and concrete don't).
                            breaches.write(crate::destruction::hooks::Breach { at, out });
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
    mut wakes: MessageWriter<GateWake>,
    mut quiets: MessageWriter<GateQuiet>,
    mut count: ResMut<GateCount>,
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
        // (Not while a night visit is on its way: one at a time.)
        enabled: director.enabled && director.visits.is_empty(),
    };
    // Where it's sent: the one the Zone is looking at hardest.
    let target = players
        .iter()
        .max_by_key(|(_, a)| a.map_or(0, |a| a.feeling()))
        .map(|(g, _)| g.translation());
    let call = rules::call_step(&mut calls.book, input);
    // The Gate counting you: awake from the line, quiet when you step out
    // or it calls.
    let counting = rules::counting(&calls.book, input);
    if counting != count.counting {
        if counting {
            wakes.write(GateWake { gate });
        } else {
            quiets.write(GateQuiet {
                gate,
                called: call.is_some(),
            });
        }
    }
    let held = if counting {
        (calls.book.at_gate / rules::GATE_HOLD).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let now = GateCount {
        gate,
        counting,
        held,
    };
    if *count != now {
        *count = now;
    }
    if let Some(why) = call {
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

    /// It has you; you walk away and keep walking. It lets you go and goes
    /// back to its work, and doesn't take you up again from afar.
    #[test]
    fn walk_away_and_it_lets_you_go() {
        let mut vr = crate::vr_harness::Vr::new(extraction_core::map::TEST_FIELD_ID);
        let zone = vr.app.world().resource::<Zone>().clone();
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let home = zone.on_ground(40.0, 230.0);
        let mut at = Vec2::new(40.0, 218.0);
        vr.stand(zone.on_ground(at.x, at.y), 0.0);
        let heavy = {
            let mut c = vr.app.world_mut().commands();
            let e = super::super::heavy::spawn_heavy(
                &mut c,
                &zone,
                &catalog,
                home,
                Job {
                    kind: JobKind::Watch,
                    site: home,
                    duration: 999.0,
                    exit: home,
                },
            );
            vr.app.world_mut().flush();
            e
        };
        let engaged = |vr: &crate::vr_harness::Vr| {
            matches!(vr.app.world().get::<Brain>(heavy), Some(Brain::Engage))
        };
        // It sees you and comes for you.
        let mut came = false;
        for _ in 0..(90 * 20) {
            vr.step(1);
            if engaged(&vr) {
                came = true;
                break;
            }
        }
        assert!(came, "it came for you");
        // You walk away, a brisk walk, and keep walking.
        let mut let_go_at = None;
        for k in 0..(90 * 40) {
            at.y -= 2.0 / 90.0;
            vr.stand(zone.on_ground(at.x, at.y), 0.0);
            vr.step(1);
            if engaged(&vr) {
                let_go_at = None;
            } else {
                let_go_at.get_or_insert(k);
            }
        }
        let k = let_go_at.expect("it let you go");
        assert!(
            k < 90 * 30,
            "within half a minute: {:.0} s",
            k as f32 / 90.0
        );
        // Standing off at a distance now: it leaves you be.
        vr.step(90 * 10);
        assert!(!engaged(&vr), "it doesn't take you up again from afar");
    }

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

    /// Every heavy reads before it reaches you. Heavies have no posts or
    /// patrols: each is called out of its valley's Gate (`call_heavy`),
    /// so its approach starts there, and its horn has to reach every spot
    /// you can stand in every valley, calm or in a downpour, with
    /// [`HORN_MARGIN`] to spare. Then, under
    /// every weather and hour (the storm too), it's heard coming (its
    /// footfalls, walking; its hum, standing) from further than its eye
    /// is good for, so it never notices you before you could know it's
    /// there.
    #[test]
    fn every_heavy_reads_before_it_reaches_you() {
        use super::super::heavy::SIGHT;
        use super::super::senses::{Coming, heard_coming};
        use crate::world::weather::{Conditions, Hour, WeatherKind};
        use extraction_core::map::{MapDef, VALLEYS};
        use extraction_core::map_heavy;
        use extraction_core::terrain::Heightmap;
        let mut short = Vec::new();
        for (id, text) in VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let r = map_heavy::reach(&map, &hm).unwrap_or_else(|| panic!("{id}: no Gate"));
            let (_, far) = r.furthest().unwrap();
            for (weather, rain) in [("calm", 0.0), ("downpour", 1.0)] {
                let horn = heard_at(HORN_HEARD, rain) / HORN_MARGIN;
                let beyond = r.beyond(horn);
                println!(
                    "{id:<17} {weather:<8} horn {horn:3.0} m, ground to {far:3.0} m: {} of {} spots beyond",
                    beyond.len(),
                    r.ground.len()
                );
                if let Some((p, d)) = beyond.first() {
                    let (name, off) = map_heavy::near(&map, *p).unwrap_or(("?", 0.0));
                    short.push(format!(
                        "{id} in a {weather}: {} spots beyond the horn ({horn:.0} m with its margin), the furthest {d:.0} m at ({:.0}, {:.0}), {off:.0} m from {name}",
                        beyond.len(),
                        p[0],
                        p[1]
                    ));
                }
            }
        }
        let kinds = [
            WeatherKind::Clearing,
            WeatherKind::Drizzle,
            WeatherKind::PeaSouper,
            WeatherKind::Downpour,
        ];
        for kind in kinds {
            for hour in [Hour::Dawn, Hour::Day, Hour::Dusk, Hour::Night] {
                for storm in [false, true] {
                    if storm && kind != WeatherKind::Downpour {
                        continue;
                    }
                    for cycle in 0..8 {
                        let mut c = Conditions::compose(kind, hour, cycle);
                        if storm {
                            c = c.stormy();
                        }
                        // How far it notices you, uncapped by how it's heard.
                        let eye = c.sight(SIGHT);
                        let steps = heard_coming(Coming::Heavy, c.rain);
                        let hum = heard_at(HUM_HEARD, c.rain);
                        if cycle == 0 {
                            println!(
                                "{kind:?}{} {hour:?}: notices you to {eye:2.0} m; heard walking {steps:3.0} m, standing {hum:3.0} m",
                                if storm { " (storm)" } else { "" }
                            );
                        }
                        if eye > steps.min(hum) {
                            short.push(format!(
                                "{kind:?} {hour:?} cycle {cycle}: it notices you to {eye:.0} m, heard only to {:.0} m",
                                steps.min(hum)
                            ));
                        }
                    }
                }
            }
        }
        assert!(short.is_empty(), "{}", short.join("\n"));
    }

    /// The Gate reads before it calls (a key at a Gate is the fastest way
    /// a heavy comes: [`rules::GATE_HOLD`] after the line, the horn; then
    /// [`rules::LEAD`] and it walks out). Walking in with a key and at a
    /// sprint, the Gate wakes on the tick you cross its line
    /// ([`GateWake`]); seen ([`rules::NOTICE`]) and turned from
    /// ([`rules::TURN`]), you're back out before it calls, and it goes
    /// quiet ([`GateQuiet`]) with nothing called. Stay, and it calls on
    /// time, quiet as it does. (Heard as its lamp and call will hear them:
    /// by a system in `Update`.) Its ring of stakes is
    /// [`gate_line_is_staked`].
    #[test]
    fn gate_reads_before_it_calls() {
        use crate::player::{SPRINT_SPEED, WALK_SPEED};
        /// What the Gate said, as an `Update` system hears it: (fixed
        /// seconds, woke or quiet, called).
        #[derive(Resource, Default)]
        struct Heard(Vec<(f32, bool, bool)>);
        fn hear(
            mut heard: ResMut<Heard>,
            mut w: MessageReader<GateWake>,
            mut q: MessageReader<GateQuiet>,
            time: Res<Time<Fixed>>,
        ) {
            let t = time.elapsed_secs();
            heard.0.extend(w.read().map(|_| (t, true, false)));
            heard.0.extend(q.read().map(|q| (t, false, q.called)));
        }
        let mut vr = crate::vr_harness::Vr::new("marrow_creek");
        vr.app.init_resource::<Heard>().add_systems(Update, hear);
        vr.app
            .world_mut()
            .resource_mut::<super::super::director::Director>()
            .enabled = true;
        let zone = vr.app.world().resource::<Zone>().clone();
        let gate = super::super::director::gate(&zone);
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let key = catalog.kind("key_marrow").expect("the Marrow key");
        let player = vr.player;
        let k = crate::items::spawn_item(
            &mut vr.app.world_mut().commands(),
            &catalog,
            crate::items::SpawnItem {
                kind: key,
                transform: Transform::from_translation(gate + Vec3::Z * 300.0),
                condition: 1.0,
            },
        );
        vr.app.world_mut().flush();
        vr.app
            .world_mut()
            .entity_mut(k)
            .insert(crate::interact::body::InBackpack { bag: player });
        let now =
            |vr: &crate::vr_harness::Vr| vr.app.world().resource::<Time<Fixed>>().elapsed_secs();
        let take = |vr: &mut crate::vr_harness::Vr| {
            std::mem::take(&mut vr.app.world_mut().resource_mut::<Heard>().0)
        };
        let calls = |vr: &crate::vr_harness::Vr| vr.app.world().resource::<HeavyCalls>().log.len();
        // In from the meadow side, straight at the Gate.
        let out = Vec3::new(1.0, 0.0, 1.0).normalize();
        let at = |d: f32| zone.on_ground(gate.x + out.x * d, gate.z + out.z * d);
        // On at `speed` (m/s, toward the Gate if positive) since the last
        // step, by the game's clock (standing steps it too), then a frame.
        let last = std::cell::Cell::new(0.0f32);
        let walk = |vr: &mut crate::vr_harness::Vr, d: &mut f32, speed: f32| {
            let t = now(vr);
            *d -= speed * (t - last.get()).clamp(0.0, 0.1);
            last.set(t);
            vr.stand(at(*d), 0.0);
            vr.step(1);
        };
        for speed in [WALK_SPEED, SPRINT_SPEED] {
            let mut d = GATE_RADIUS + 6.0;
            vr.stand(at(d), 0.0);
            vr.step(10);
            take(&mut vr);
            last.set(now(&vr));
            // In to the line and over it, on till it's noticed.
            let mut crossed = None;
            let mut woke = None;
            while crossed.is_none_or(|c| now(&vr) < c + rules::NOTICE) {
                walk(&mut vr, &mut d, speed);
                if crossed.is_none() && d < GATE_RADIUS {
                    crossed = Some(now(&vr));
                }
                for (t, wake, _) in take(&mut vr) {
                    assert!(wake, "quiet before it woke");
                    assert!(crossed.is_some(), "woke {d:.1} m out, before the line");
                    woke.get_or_insert(t);
                }
            }
            let (c, w) = (crossed.unwrap(), woke.expect("the Gate woke"));
            let lag = w - c;
            let inside = GATE_RADIUS - d;
            // Stop, turn, and walk back out the way you came.
            let turn = now(&vr);
            while now(&vr) < turn + rules::TURN {
                walk(&mut vr, &mut d, 0.0);
            }
            let mut heard = take(&mut vr);
            while d <= GATE_RADIUS + 0.5 {
                walk(&mut vr, &mut d, -speed);
                heard.extend(take(&mut vr));
                assert!(now(&vr) < w + 30.0, "never got out");
            }
            vr.step(3);
            heard.extend(take(&mut vr));
            let back = now(&vr) - w;
            println!(
                "at {speed} m/s: awake {lag:.2} s after the line, {inside:.1} m in when seen; back out {back:.1} s after it woke (it calls at {})",
                rules::GATE_HOLD
            );
            assert!(lag <= 0.05, "it woke {lag:.2} s after the line");
            let quiet: Vec<bool> = heard.iter().map(|h| h.2).collect();
            assert!(heard.iter().all(|h| !h.1), "{heard:?}");
            assert_eq!(quiet, vec![false], "quiet as you leave, nothing called");
            assert_eq!(calls(&vr), 0, "no heavy called at {speed} m/s");
        }
        // Stay, and it calls on time.
        let mut d = GATE_RADIUS + 3.0;
        vr.stand(at(d), 0.0);
        vr.step(10);
        take(&mut vr);
        last.set(now(&vr));
        let mut heard = Vec::new();
        while calls(&vr) == 0 {
            let pace = if d > GATE_RADIUS - 10.0 {
                WALK_SPEED
            } else {
                0.0
            };
            walk(&mut vr, &mut d, pace);
            heard.extend(take(&mut vr));
            assert!(
                heard.is_empty() || now(&vr) < heard[0].0 + 20.0,
                "it never called"
            );
        }
        vr.step(3);
        heard.extend(take(&mut vr));
        let flags: Vec<(bool, bool)> = heard.iter().map(|h| (h.1, h.2)).collect();
        assert_eq!(
            flags,
            vec![(true, false), (false, true)],
            "woke, then quiet as it called"
        );
        let held = heard[1].0 - heard[0].0;
        println!("stayed: called {held:.2} s after it woke");
        assert!(
            (held - rules::GATE_HOLD).abs() < 0.05,
            "called after {held:.2} s"
        );
        assert!(!vr.app.world().resource::<GateCount>().counting);
    }

    /// Each Gate's line is staked where you'd cross it: every walkable
    /// spot of it within 25 m of a machine marker standing on it (Map
    /// design's and Fourth valley's rings).
    #[test]
    fn gate_line_is_staked() {
        use extraction_core::map::{MapDef, VALLEYS};
        use extraction_core::terrain::Heightmap;
        let mut short = Vec::new();
        for (id, text) in VALLEYS {
            let map = MapDef::from_ron(text).unwrap();
            let hm = Heightmap::build(&map);
            let bare = extraction_core::map_heavy::unmarked_line(
                &map,
                &hm,
                GATE_RADIUS,
                "machine_marker",
                25.0,
            );
            if let Some(p) = bare.first() {
                short.push(format!(
                    "{id}: {} spots of the Gate's line unmarked, e.g. ({:.0}, {:.0})",
                    bare.len(),
                    p[0],
                    p[1]
                ));
            }
        }
        assert!(short.is_empty(), "{}", short.join("\n"));
    }

    /// It lets you go, and shows it. Hunted by a heavy sent from a Gate
    /// that lies past you, you walk away at a walk with a headset's bob
    /// (your head swinging 15 cm back and forth twice a second): it lets
    /// you go within [`LET_GO_AFTER`] and a little of being out of reach;
    /// within a second its eye banks below its glow at work (stood down,
    /// not a machine that never saw you); within two it has turned its
    /// back on you; and for the half minute after, going home, it never
    /// comes within [`LET_GO_NEAR`] of you, though its way home is past
    /// you, nor takes you up again.
    #[test]
    fn the_heavy_lets_you_go_and_shows_it() {
        use crate::player::WALK_SPEED;
        let mut vr = crate::vr_harness::Vr::new(extraction_core::map::TEST_FIELD_ID);
        let zone = vr.app.world().resource::<Zone>().clone();
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let home = zone.on_ground(40.0, 230.0);
        // Its Gate, far past where you'll walk to.
        let gate = zone.on_ground(40.0, 90.0);
        let mut at = Vec2::new(40.0, 218.0);
        vr.stand(zone.on_ground(at.x, at.y), 0.0);
        let heavy = {
            let mut c = vr.app.world_mut().commands();
            let e = super::super::heavy::spawn_heavy(
                &mut c,
                &zone,
                &catalog,
                home,
                Job {
                    kind: JobKind::Response,
                    site: home,
                    duration: HUNT_TIME,
                    exit: gate,
                },
            );
            vr.app.world_mut().flush();
            vr.app
                .world_mut()
                .entity_mut(e)
                .insert(HeavyMind::default());
            e
        };
        let player = vr.player;
        let now =
            |vr: &crate::vr_harness::Vr| vr.app.world().resource::<Time<Fixed>>().elapsed_secs();
        let place = |vr: &mut crate::vr_harness::Vr, p: Vec2| {
            let g = zone.on_ground(p.x, p.y);
            vr.app
                .world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = g;
        };
        let brain = |vr: &crate::vr_harness::Vr| vr.app.world().get::<Brain>(heavy).cloned();
        let eye = |vr: &crate::vr_harness::Vr| {
            let w = vr.app.world();
            let l = w.get::<super::super::heavy_look::Lights>(heavy)?;
            let m = w.resource::<Assets<StandardMaterial>>().get(&l.eye)?;
            Some(m.emissive.red / 9.0)
        };
        let facing_away = |vr: &crate::vr_harness::Vr, me: Vec2| {
            let w = vr.app.world();
            let wk = w.get::<super::super::walker::Walker>(heavy).unwrap();
            let pos = w.get::<Transform>(heavy).unwrap().translation;
            let face = Vec2::new(-wk.yaw.sin(), -wk.yaw.cos());
            let to_me = (me - Vec2::new(pos.x, pos.z)).normalize_or_zero();
            face.dot(to_me).clamp(-1.0, 1.0).acos().to_degrees()
        };
        let gap = |vr: &crate::vr_harness::Vr, me: Vec2| {
            let pos = vr
                .app
                .world()
                .get::<Transform>(heavy)
                .map(|t| t.translation);
            pos.map(|p| (me - Vec2::new(p.x, p.z)).length())
        };
        // It sees you and comes.
        let t0 = now(&vr);
        while !matches!(brain(&vr), Some(Brain::Engage)) {
            vr.step(1);
            assert!(now(&vr) < t0 + 20.0, "it never came for you");
        }
        // You walk away, bobbing; out of its reach at 10 m.
        let mut out_at = None;
        let mut let_go_at = None;
        let mut path = at.y;
        let start = now(&vr);
        while let_go_at.is_none() {
            let t = now(&vr) - start;
            path -= WALK_SPEED / 90.0;
            at.y = path + 0.15 * (std::f32::consts::TAU * 2.0 * t).sin();
            place(&mut vr, at);
            vr.step(1);
            if out_at.is_none() && gap(&vr, at).is_some_and(|g| g > LET_GO_RANGE) {
                out_at = Some(now(&vr));
            }
            if vr
                .app
                .world()
                .get::<HeavyMind>(heavy)
                .is_some_and(|m| m.stood_down)
            {
                let_go_at = Some(now(&vr));
            }
            assert!(t < 40.0, "it never let you go, walking off with a bob");
        }
        let took = let_go_at.unwrap() - out_at.unwrap();
        println!("let go {took:.1} s after you were out of its reach");
        assert!(
            took <= LET_GO_AFTER + 2.0,
            "it let go {took:.1} s after you were out of reach"
        );
        // You stop where you are and watch it. Within 1 s its eye is low;
        // within 2 s its back is to you.
        let let_go = let_go_at.unwrap();
        let (mut low, mut away) = (None, None);
        let mut nearest = f32::MAX;
        while now(&vr) < let_go + 30.0 {
            vr.step(1);
            let t = now(&vr) - let_go;
            if low.is_none() && eye(&vr).is_some_and(|e| e < 0.5) {
                low = Some(t);
            }
            if away.is_none() && facing_away(&vr, at) > 120.0 {
                away = Some(t);
            }
            if let Some(g) = gap(&vr, at) {
                nearest = nearest.min(g);
            } else {
                break;
            }
            assert!(
                !matches!(brain(&vr), Some(Brain::Engage)),
                "it took you up again {t:.1} s after letting you go"
            );
        }
        println!("eye low {low:?} s, back turned {away:?} s, nearest after {nearest:.1} m");
        assert!(low.is_some_and(|t| t <= 1.0), "its eye: {low:?}");
        assert!(away.is_some_and(|t| t <= 2.0), "its back: {away:?}");
        assert!(
            nearest > LET_GO_NEAR,
            "it came within {nearest:.1} m going home"
        );
    }
}
