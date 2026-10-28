//! The Zone hurts them too, and you can use it.
//!
//! What every hazard in the world does to every kind of machine is one
//! table ([`effect`]): hurt, slowed, staggered or thrown, dazzled (its
//! lock lost), deafened, stunned, or nothing. Some of that the hazards do
//! themselves (a burner's heat, a snare's bolt, a gas pocket's flash, a
//! mine, an EMP: `anomaly`, `world::workings`, `robots::minelayer`,
//! `gadgets`); the rest is done here, from where the hazard is
//! (`anomaly::Anomaly`, `anomaly::hush::Hushes`):
//!
//! - **a lift** throws a drone when it pops, and staggers a walker in it
//!   (slowed while it winds up);
//! - **a well** drags whatever walks in toward its heart, and crushes it
//!   there;
//! - **a mirror** dazzles: a machine passing its fold loses you a while;
//! - **a hush** deafens a machine standing in it, and a drone in one loses
//!   its lock.
//!
//! Whatever happens to a machine is told ([`HazardStruck`], once a hazard
//! per machine per [`TELL_EVERY`]), for its look and sound, and it shows
//! on the machine while it lasts ([`Dazzled`], [`Deafened`],
//! [`Hampered`]).
//!
//! Machines go round what hurts them (`walker::Nav` costs round live
//! anomalies, [`avoid_anomalies`]), unless they're chasing hard: running
//! at you, you in sight, they come straight, and that's the lure (a hound
//! across a snare, a walker into a burner's edge). One holding a post or
//! searching goes round.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::anomaly::AnomalyKind;

use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::anomaly::Anomaly;

/// A kind of machine, as hazards tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Machine {
    Drone,
    Walker,
    Crawler,
    Hound,
    Heavy,
    Grappler,
    Dirigible,
}

pub const MACHINES: [Machine; 7] = [
    Machine::Drone,
    Machine::Walker,
    Machine::Crawler,
    Machine::Hound,
    Machine::Heavy,
    Machine::Grappler,
    Machine::Dirigible,
];

/// A hazard of the Zone's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hazard {
    Lift,
    Burner,
    Snare,
    Well,
    Mirror,
    Hush,
    /// A gas pocket going up (`world::workings`).
    Gas,
    /// A mine going off (`robots::minelayer`).
    Mine,
    /// An EMP going off (`gadgets`).
    Emp,
    /// A frost (Rime Fell's): still cold that stiffens.
    Frost,
    /// A hot gallery underground (`world::furnace`).
    Heat,
}

pub const HAZARDS: [Hazard; 11] = [
    Hazard::Lift,
    Hazard::Burner,
    Hazard::Snare,
    Hazard::Well,
    Hazard::Mirror,
    Hazard::Hush,
    Hazard::Gas,
    Hazard::Mine,
    Hazard::Emp,
    Hazard::Frost,
    Hazard::Heat,
];

impl Hazard {
    pub fn of(kind: AnomalyKind) -> Self {
        match kind {
            AnomalyKind::Lift => Hazard::Lift,
            AnomalyKind::Burner => Hazard::Burner,
            AnomalyKind::Snare => Hazard::Snare,
            AnomalyKind::Well => Hazard::Well,
            AnomalyKind::Mirror => Hazard::Mirror,
            AnomalyKind::Hush => Hazard::Hush,
            // The frost (newer than some trees this builds in: there it's
            // the only kind not named above).
            #[allow(unreachable_patterns)]
            _ => Hazard::Frost,
        }
    }
}

/// What a hazard does to a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// It takes damage.
    Hurt,
    /// It's slowed, or dragged.
    Slowed,
    /// It's knocked about: a stagger, or (a drone) thrown.
    Thrown,
    /// It loses you: its lock, its sight, a while.
    Dazzled,
    /// It hears nothing.
    Deafened,
    /// It's out: no senses, no legs, no rotors, a while.
    Stunned,
    /// Nothing: too heavy, too high, never there.
    Unaffected,
}

/// The table: what `hazard` does to `machine`.
pub const fn effect(machine: Machine, hazard: Hazard) -> Effect {
    use Effect::*;
    use Hazard as H;
    use Machine as M;
    match (hazard, machine) {
        // A lift throws what's light enough and low enough.
        (H::Lift, M::Drone) => Thrown,
        (H::Lift, M::Walker | M::Crawler | M::Hound | M::Grappler) => Thrown,
        (H::Lift, M::Heavy | M::Dirigible) => Unaffected,
        // A burner burns whatever's in it; the airship rides above.
        (H::Burner, M::Dirigible) => Unaffected,
        (H::Burner, _) => Hurt,
        // A snare's bolt finds anything metal in it.
        (H::Snare, M::Dirigible) => Unaffected,
        (H::Snare, M::Heavy) => Stunned,
        (H::Snare, _) => Hurt,
        // A well drags in and crushes; the airship's too high.
        (H::Well, M::Dirigible) => Unaffected,
        (H::Well, _) => Slowed,
        // A mirror's fold dazzles what looks through it.
        (H::Mirror, M::Dirigible) => Unaffected,
        (H::Mirror, _) => Dazzled,
        // A hush: nothing heard in it; a drone flies blind in it.
        (H::Hush, M::Drone) => Dazzled,
        (H::Hush, M::Dirigible) => Unaffected,
        (H::Hush, _) => Deafened,
        // Gas: only crawlers go where it lies.
        (H::Gas, M::Crawler) => Hurt,
        (H::Gas, _) => Unaffected,
        // A mine: anything heavy enough on the plate, and anything by it.
        (H::Mine, M::Drone | M::Dirigible) => Unaffected,
        (H::Mine, _) => Hurt,
        // An EMP stops them all.
        (H::Emp, _) => Stunned,
        // A frost stiffens joints and ices rotors; the airship's above it.
        (H::Frost, M::Dirigible) => Unaffected,
        (H::Frost, _) => Slowed,
        // Hot galleries: only crawlers go down there.
        (H::Heat, M::Crawler) => Hurt,
        (H::Heat, _) => Unaffected,
    }
}

/// What kind of machine a robot is, if it's one hazards touch.
pub fn machine_of(
    robot: &Robot,
    hound: bool,
    crawler: bool,
    heavy: bool,
    grappler: bool,
) -> Option<Machine> {
    Some(match robot.kind {
        RobotKind::Drone => Machine::Drone,
        RobotKind::Dirigible => Machine::Dirigible,
        RobotKind::Walker if heavy => Machine::Heavy,
        RobotKind::Walker if hound => Machine::Hound,
        RobotKind::Walker if crawler => Machine::Crawler,
        RobotKind::Walker if grappler => Machine::Grappler,
        RobotKind::Walker => Machine::Walker,
        RobotKind::Pylon | RobotKind::Sorter => return None,
    })
}

/// A hazard did something to a machine: for its tell (a look, a sound).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct HazardStruck {
    pub robot: Entity,
    pub hazard: Hazard,
    pub effect: Effect,
    pub at: Vec3,
}

/// Told at most this often per machine per hazard (s).
pub const TELL_EVERY: f32 = 1.0;

/// Its lock and sight lost, this many seconds more.
#[derive(Component, Debug, Clone, Copy)]
pub struct Dazzled(pub f32);

/// It hears nothing, this many seconds more.
#[derive(Component, Debug, Clone, Copy)]
pub struct Deafened(pub f32);

/// Its optics frosted (it sees [`FROST_SIGHT`] as far), this many seconds
/// more: in a frost, and a while after.
#[derive(Component, Debug, Clone, Copy)]
pub struct Frosted(pub f32);

/// Slowed to this share of its pace, this many seconds more.
#[derive(Component, Debug, Clone, Copy)]
pub struct Hampered {
    pub pace: f32,
    pub left: f32,
}

/// A mirror's fold dazzles this long (s)...
pub const MIRROR_DAZZLE: f32 = 3.0;
/// ...a hush, for as long as it's in one, and a moment after.
pub const HUSH_AFTER: f32 = 0.5;
/// A lift winding up slows a walker in it to this...
pub const LIFT_SLOW: f32 = 0.6;
/// ...and its pop staggers it with this much (as a hit's weight), and
/// throws a drone this hard (m/s, out and up).
pub const LIFT_STAGGER: f32 = 45.0;
pub const LIFT_THROW: (f32, f32) = (6.0, 7.0);
/// A drone thrown loses you this long (s).
pub const THROWN_DAZZLE: f32 = 2.0;
/// A well drags at this (m/s toward its heart), slows to this...
pub const WELL_DRAG: f32 = 2.4;
pub const WELL_SLOW: f32 = 0.45;
/// ...and crushes at its heart (hp/s, on the core).
pub const WELL_CRUSH: f32 = 25.0;
/// A frost stiffens a walker to this share of its pace, and takes this
/// share of a drone's way a second.
pub const FROST_PACE: f32 = 0.55;
pub const FROST_ICE: f32 = 0.8;
/// A drone in one sinks at this (m/s²)...
pub const FROST_SINK: f32 = 3.0;
/// ...and any machine's optics frost: it sees this share as far, until
/// this long after it's out (s).
pub const FROST_SIGHT: f32 = 0.4;
pub const FROST_THAW: f32 = 3.0;
/// A gas flash, a mine, an EMP: a machine within this of it is told (m).
pub const BLAST_TOLD: f32 = 6.0;

/// The Zone's walking grid made wary of anomalies: what hurts a walker is
/// closed to it (and [`AVOID_MARGIN`] past its reach), so it goes round.
/// Built from `walker::Nav` when that or the anomalies change.
#[derive(Resource, Clone)]
pub struct Wary(pub std::sync::Arc<extraction_core::nav::NavGrid>);

/// What hurts it is closed this far past its edge (m).
pub const AVOID_MARGIN: f32 = 1.5;
/// Running at you (in a fight, you in sight, closing on you faster than
/// this, m/s) within this (m), it comes straight. Holding a post, or
/// searching, it goes round.
pub const CHASE_HARD: f32 = 40.0;
pub const CLOSING: f32 = 0.8;

/// Chasing hard: running at you, it risks what's in the way. `gap` is how
/// far you are, `closing` how fast that's shrinking (m/s).
pub fn chasing_hard(
    brain: &super::Brain,
    aware: &super::Awareness,
    gap: Option<f32>,
    closing: f32,
) -> bool {
    matches!(brain, super::Brain::Engage)
        && aware.can_see_target
        && gap.is_some_and(|g| g < CHASE_HARD)
        && closing > CLOSING
}

/// Whether walkers go round this kind of anomaly.
pub fn avoided(kind: AnomalyKind) -> bool {
    matches!(
        effect(Machine::Walker, Hazard::of(kind)),
        Effect::Hurt | Effect::Slowed | Effect::Thrown | Effect::Stunned
    )
}

/// The wary grid: the plain one, with what hurts a walker made dear.
fn avoid_anomalies(
    mut commands: Commands,
    nav: Option<Res<super::walker::Nav>>,
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    mut last: Local<(usize, Vec<[i32; 4]>)>,
) {
    let Some(nav) = nav else {
        return;
    };
    let mut sig: Vec<[i32; 4]> = anomalies
        .iter()
        .filter(|(a, _)| avoided(a.kind))
        .map(|(a, g)| {
            let p = g.translation();
            [
                a.kind as i32,
                p.x.round() as i32,
                p.z.round() as i32,
                (a.kind.reach(a.radius) * 2.0).round() as i32,
            ]
        })
        .collect();
    sig.sort();
    let ptr = std::sync::Arc::as_ptr(&nav.0) as usize;
    if last.0 == ptr && last.1 == sig {
        return;
    }
    let mut grid = (*nav.0).clone();
    for (a, g) in &anomalies {
        if !avoided(a.kind) {
            continue;
        }
        let c = g.translation();
        // Closed, not just dear: a path's smoothing goes by what's
        // walkable, and would cut a dear detour straight back through.
        grid.block(
            extraction_core::nav::Obstacle::Circle {
                at: [c.x, c.z],
                radius: a.kind.reach(a.radius),
            },
            AVOID_MARGIN,
        );
    }
    commands.insert_resource(Wary(std::sync::Arc::new(grid)));
    *last = (ptr, sig);
}

pub struct HazardsPlugin;

impl Plugin for HazardsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HazardStruck>()
            .add_systems(
                FixedUpdate,
                (in_anomalies, blasts, scald, wear_off)
                    .chain()
                    .before(super::perceive),
            )
            .add_systems(Update, avoid_anomalies);
    }
}

/// A machine's place, and what it is, for hazards.
#[derive(bevy::ecs::query::QueryData)]
#[query_data(mutable)]
pub struct Body {
    pub entity: Entity,
    pub robot: &'static Robot,
    pub at: &'static GlobalTransform,
    pub hound: Has<super::hound::Hound>,
    pub crawler: Has<super::indoor::Crawler>,
    pub heavy: Has<super::heavy::Heavy>,
    pub grappler: Has<super::grappler::Grappler>,
    pub children: Option<&'static Children>,
    pub velocity: Option<&'static mut LinearVelocity>,
    pub gait: Option<&'static mut super::anim::Gait>,
}

impl BodyItem<'_, '_> {
    pub fn machine(&self) -> Option<Machine> {
        if self.robot.wrecked {
            return None;
        }
        machine_of(
            self.robot,
            self.hound,
            self.crawler,
            self.heavy,
            self.grappler,
        )
    }
}

/// Inside an anomaly of `kind` with this radius at `centre` (a cylinder,
/// as they reach: their height over the ground).
pub fn inside(kind: AnomalyKind, radius: f32, centre: Vec3, p: Vec3) -> bool {
    let flat = Vec2::new(p.x - centre.x, p.z - centre.z).length();
    let up = p.y - centre.y;
    let tall = match kind {
        AnomalyKind::Lift => 4.0,
        AnomalyKind::Burner => 3.0,
        _ => 3.5,
    };
    flat < kind.reach(radius) && up > -1.0 && up < tall
}

/// Tells (and only so often): one per machine per hazard per TELL_EVERY.
#[derive(Default)]
pub struct Told(std::collections::HashMap<(Entity, Hazard), f32>);

impl Told {
    fn tell(
        &mut self,
        out: &mut MessageWriter<HazardStruck>,
        robot: Entity,
        hazard: Hazard,
        effect: Effect,
        at: Vec3,
        now: f32,
    ) {
        let key = (robot, hazard);
        if self.0.get(&key).is_some_and(|t| now - t < TELL_EVERY) {
            return;
        }
        self.0.insert(key, now);
        out.write(HazardStruck {
            robot,
            hazard,
            effect,
            at,
        });
    }
}

/// The anomalies' effects on machines in them (what the anomalies don't
/// do themselves), and every one told.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn in_anomalies(
    mut commands: Commands,
    anomalies: Query<(Entity, &Anomaly, &GlobalTransform)>,
    mut pops: MessageReader<crate::anomaly::AnomalyEvent>,
    mut bodies: Query<Body>,
    parts: Query<&RobotPart>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut out: MessageWriter<HazardStruck>,
    time: Res<Time>,
    mut told: Local<Told>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    told.0
        .retain(|(e, _), t| now - *t < 10.0 && bodies.contains(*e));
    let popped: Vec<Entity> = pops
        .read()
        .filter(|p| matches!(p.what, crate::anomaly::AnomalyAct::Pop))
        .map(|p| p.anomaly)
        .collect();
    let list: Vec<(Entity, AnomalyKind, f32, Vec3)> = anomalies
        .iter()
        .map(|(e, a, g)| (e, a.kind, a.radius, g.translation()))
        .collect();
    for mut b in &mut bodies {
        let Some(machine) = b.machine() else {
            continue;
        };
        let p = b.at.translation();
        for &(ae, kind, radius, centre) in &list {
            if !inside(kind, radius, centre, p) {
                continue;
            }
            let hazard = Hazard::of(kind);
            let fx = effect(machine, hazard);
            if fx == Effect::Unaffected {
                continue;
            }
            let e = b.entity;
            match (hazard, fx) {
                (Hazard::Lift, Effect::Thrown) => {
                    // Winding up, it drags at the legs; when it pops,
                    // it throws.
                    if machine != Machine::Drone {
                        commands.entity(e).try_insert(Hampered {
                            pace: LIFT_SLOW,
                            left: 0.3,
                        });
                    }
                    if !popped.contains(&ae) {
                        told.tell(&mut out, e, hazard, Effect::Slowed, p, now);
                        continue;
                    }
                    let away = Vec3::new(p.x - centre.x, 0.0, p.z - centre.z).normalize_or(Vec3::X);
                    if let Some(v) = b.velocity.as_mut() {
                        v.0 += away * LIFT_THROW.0 + Vec3::Y * LIFT_THROW.1;
                    }
                    if let Some(g) = b.gait.as_mut() {
                        g.daze(LIFT_STAGGER, away);
                        g.knock(Vec3::Y, away * 2.0);
                    }
                    commands.entity(e).try_insert(Dazzled(THROWN_DAZZLE));
                    told.0.remove(&(e, hazard));
                }
                (Hazard::Well, Effect::Slowed) => {
                    let heart = crate::anomaly::wells::heart(centre);
                    let to = Vec3::new(heart.x - p.x, 0.0, heart.z - p.z);
                    let d = to.length();
                    // Dragged toward its heart: a drone by its body, a
                    // walker by where it's going.
                    if let Some(v) = b.velocity.as_mut() {
                        v.0 += to.normalize_or_zero() * WELL_DRAG * dt * 4.0;
                    }
                    commands.entity(e).try_insert(Hampered {
                        pace: WELL_SLOW,
                        left: 0.3,
                    });
                    if d < radius * extraction_core::anomaly::WELL_HEART {
                        if let Some(core) = core_of(b.children, &parts) {
                            hits.write(crush(core, p, WELL_CRUSH * dt));
                        }
                        told.tell(&mut out, e, hazard, Effect::Hurt, p, now);
                        continue;
                    }
                }
                (Hazard::Frost, Effect::Slowed) => {
                    // Joints stiff, and its eyes frosting over (`Frosted`).
                    commands.entity(e).try_insert((
                        Hampered {
                            pace: FROST_PACE,
                            left: 0.3,
                        },
                        Frosted(FROST_THAW),
                    ));
                    // Rotors icing: a drone loses way, and lift.
                    if machine == Machine::Drone {
                        if let Some(v) = b.velocity.as_mut() {
                            v.0 *= 1.0 - (FROST_ICE * dt).min(1.0);
                            v.0.y -= FROST_SINK * dt;
                        }
                    }
                }
                (Hazard::Mirror, Effect::Dazzled) => {
                    commands.entity(e).try_insert(Dazzled(MIRROR_DAZZLE));
                }
                (Hazard::Hush, Effect::Deafened) => {
                    commands.entity(e).try_insert(Deafened(HUSH_AFTER));
                }
                (Hazard::Hush, Effect::Dazzled) => {
                    commands
                        .entity(e)
                        .try_insert((Dazzled(HUSH_AFTER), Deafened(HUSH_AFTER)));
                }
                // The anomaly does it itself (a burner's heat, a snare's
                // bolt, the heavy's stun): it's only told here.
                _ => {}
            }
            told.tell(&mut out, e, hazard, fx, p, now);
        }
    }
}

/// A robot's core part, if it has one.
fn core_of(children: Option<&Children>, parts: &Query<&RobotPart>) -> Option<Entity> {
    children?.iter().find(|c| {
        parts
            .get(*c)
            .is_ok_and(|p| p.role == PartRole::Core && p.alive())
    })
}

fn crush(part: Entity, at: Vec3, damage: f32) -> crate::weapons::ballistics::Hit {
    crate::weapons::ballistics::Hit {
        entity: part,
        point: at,
        normal: Vec3::Y,
        velocity: Vec3::ZERO,
        damage,
        shooter: None,
        mass_g: 0.0,
        load: Default::default(),
        blow: None,
    }
}

/// Gas flashes, mines and EMPs: what they do the hazards do themselves;
/// the machines by them are told.
#[allow(clippy::too_many_arguments)]
fn blasts(
    mut gas: MessageReader<crate::world::workings::GasIgnited>,
    mut mines: MessageReader<super::minelayer::MineEvent>,
    mut emps: MessageReader<crate::gadgets::EmpBlast>,
    mut bodies: Query<Body>,
    mut out: MessageWriter<HazardStruck>,
    time: Res<Time>,
    mut told: Local<Told>,
) {
    let now = time.elapsed_secs();
    let mut at: Vec<(Hazard, Vec3, f32)> = gas
        .read()
        .map(|g| (Hazard::Gas, g.at, BLAST_TOLD))
        .collect();
    at.extend(
        mines
            .read()
            .filter(|m| matches!(m.what, super::minelayer::MineAct2::Blast))
            .map(|m| (Hazard::Mine, m.at, extraction_core::minelayer::REACH)),
    );
    at.extend(emps.read().map(|e| (Hazard::Emp, e.at, e.radius)));
    if at.is_empty() {
        return;
    }
    for b in &mut bodies {
        let Some(machine) = b.machine() else {
            continue;
        };
        let p = b.at.translation();
        for &(hazard, centre, reach) in &at {
            let fx = effect(machine, hazard);
            if fx == Effect::Unaffected || p.distance(centre) > reach {
                continue;
            }
            told.tell(&mut out, b.entity, hazard, fx, p, now);
        }
    }
}

/// A machine in a hot gallery is burned as you are (`world::furnace`):
/// `HEAT_HARM` a second times how hot, `SCORCH_HARM` where it's
/// scorching, on its core.
#[allow(clippy::type_complexity)]
fn scald(
    heat: Option<Res<crate::world::furnace::Heat>>,
    sites: Query<(Entity, &crate::world::buildings::Building, &Transform)>,
    mut bodies: Query<Body>,
    parts: Query<&RobotPart>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut out: MessageWriter<HazardStruck>,
    time: Res<Time>,
    mut told: Local<Told>,
) {
    let Some(heat) = heat.filter(|h| !h.0.is_empty()) else {
        return;
    };
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    for b in &mut bodies {
        let Some(machine) = b.machine() else {
            continue;
        };
        let fx = effect(machine, Hazard::Heat);
        if fx == Effect::Unaffected {
            continue;
        }
        let p = b.at.translation();
        let h = heat.at(&sites, p + Vec3::Y * 0.2);
        if h <= 0.0 {
            continue;
        }
        let dps = if h >= extraction_core::underground::SCORCHING {
            crate::world::furnace::SCORCH_HARM
        } else {
            crate::world::furnace::HEAT_HARM * h
        };
        if let Some(core) = core_of(b.children, &parts) {
            hits.write(crush(core, p, dps * dt));
        }
        told.tell(&mut out, b.entity, Hazard::Heat, fx, p, now);
    }
}

/// What wears off.
fn wear_off(
    mut commands: Commands,
    mut dazzled: Query<(Entity, &mut Dazzled)>,
    mut deaf: Query<(Entity, &mut Deafened)>,
    mut slow: Query<(Entity, &mut Hampered)>,
    mut frosted: Query<(Entity, &mut Frosted)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut f) in &mut frosted {
        f.0 -= dt;
        if f.0 <= 0.0 {
            commands.entity(e).try_remove::<Frosted>();
        }
    }
    for (e, mut d) in &mut dazzled {
        d.0 -= dt;
        if d.0 <= 0.0 {
            commands.entity(e).try_remove::<Dazzled>();
        }
    }
    for (e, mut d) in &mut deaf {
        d.0 -= dt;
        if d.0 <= 0.0 {
            commands.entity(e).try_remove::<Deafened>();
        }
    }
    for (e, mut h) in &mut slow {
        h.left -= dt;
        if h.left <= 0.0 {
            commands.entity(e).try_remove::<Hampered>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every hazard touches some machine, and every machine is touched by
    /// some hazard; an EMP stops them all.
    #[test]
    fn the_table_is_whole() {
        for h in HAZARDS {
            assert!(
                MACHINES.iter().any(|m| effect(*m, h) != Effect::Unaffected),
                "{h:?} touches nothing"
            );
        }
        for m in MACHINES {
            assert!(
                HAZARDS.iter().any(|h| effect(m, *h) != Effect::Unaffected),
                "{m:?} is touched by nothing"
            );
            assert_eq!(effect(m, Hazard::Emp), Effect::Stunned);
        }
    }
}
