//! The ladder of waves, and their coming: what each wave brings and
//! through which gate, the warning, and the machines themselves, spawned
//! as the director would send them and left to their own minds.

use bevy::prelude::*;

use super::{Arcade, ArcadeSystems, RunPhase, WARNING};
use crate::raid::{Phase, Raid};
use crate::rig::LocalPlayer;
use crate::robots::{Job, JobKind, Robot};
use crate::world::Zone;

/// A machine a wave can bring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Machine {
    Walker,
    Drone,
    Hound,
    Heavy,
}

impl Machine {
    pub fn name(self) -> &'static str {
        match self {
            Machine::Walker => "walker",
            Machine::Drone => "drone",
            Machine::Hound => "hound",
            Machine::Heavy => "heavy",
        }
    }

    /// Points for putting one down.
    pub fn points(self) -> u32 {
        match self {
            Machine::Walker => 100,
            Machine::Drone => 75,
            Machine::Hound => 150,
            Machine::Heavy => 400,
        }
    }
}

/// The first six waves; after them the ladder goes round again, a machine
/// more each time round ([`EXTRA`] in turn).
pub const LADDER: [&[Machine]; 6] = [
    &[Machine::Walker],
    &[Machine::Walker, Machine::Walker],
    &[Machine::Walker, Machine::Drone],
    &[Machine::Hound],
    &[Machine::Walker, Machine::Walker, Machine::Drone],
    &[Machine::Heavy],
];

/// What each time round the ladder adds, in turn.
pub const EXTRA: [Machine; 3] = [Machine::Walker, Machine::Drone, Machine::Hound];

/// What wave `n` (from 1) brings.
pub fn wave(n: u32) -> Vec<Machine> {
    let i = (n.max(1) - 1) as usize;
    let mut out = LADDER[i % LADDER.len()].to_vec();
    let round = i / LADDER.len();
    for k in 0..round {
        out.push(EXTRA[k % EXTRA.len()]);
    }
    out.sort();
    out
}

/// A wave's machines in words: "2 walkers + drone".
pub fn describe(machines: &[Machine]) -> String {
    let mut parts = Vec::new();
    let mut ms = machines.to_vec();
    ms.sort();
    ms.dedup();
    for m in ms {
        let n = machines.iter().filter(|x| **x == m).count();
        parts.push(if n == 1 {
            m.name().to_string()
        } else {
            format!("{n} {}s", m.name())
        });
    }
    parts.join(" + ")
}

/// The fence's gates: (name, where the machines come in, the lamp).
pub const GATES: [(&str, [f32; 2], [f32; 2]); 3] = [
    ("NORTH", [0.0, -46.0], [6.0, -50.0]),
    ("EAST", [46.0, -4.0], [50.0, 2.0]),
    ("WEST", [-46.0, -4.0], [-50.0, 2.0]),
];

/// The gate wave `n` comes through: round the three in turn, so it's
/// known from the board before the call.
pub fn gate(n: u32) -> usize {
    (n.max(1) - 1) as usize % GATES.len()
}

/// A wave was called (the board's card, or the smoke scenario).
#[derive(Message, Clone, Copy, Debug)]
pub struct WaveCalled;

/// One of this run's machines.
#[derive(Component, Clone, Copy, Debug)]
pub struct WaveMachine {
    pub wave: u32,
    pub kind: Machine,
}

/// A gate's lamp: lit from the call until its wave is cleared (where
/// they're coming from, then where they came from).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct GateLamp {
    pub gate: usize,
    pub lit: bool,
}

/// The siren at a gate, once, as a wave is called (`visuals` plays it).
#[derive(Message, Clone, Copy, Debug)]
pub struct Siren {
    pub at: Vec3,
}

/// A machine of the run that burnt itself out: blind too long, or gone
/// outside the fence (`spend_machines`). Said to the record.
#[derive(Message, Clone, Copy, Debug)]
pub struct MachineSpent {
    pub machine: Entity,
    pub blind: bool,
}

/// A machine blind this long burns itself out (the playtest's walker,
/// its eye shot out, stood about for a minute and wandered off the map:
/// a fight with no end).
pub const BLIND_FOR: f32 = 30.0;
/// Further than this outside the fence, a machine burns itself out.
pub const OUT_BY: f32 = 2.0;

/// The run's end, said by the waves to the record.
#[derive(Message, Clone, Debug)]
pub struct RunOver {
    pub wave: u32,
}

pub struct WavesPlugin;

impl Plugin for WavesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Siren>()
            .add_message::<RunOver>()
            .add_message::<MachineSpent>()
            .add_systems(
                Update,
                (
                    hold_the_zone,
                    call_wave,
                    count_down,
                    spend_machines,
                    clear_wave,
                    light_lamps,
                )
                    .chain()
                    .in_set(ArcadeSystems::Play),
            )
            // Right after the raid hears of it: the arcade keeps you.
            .add_systems(
                Update,
                downed
                    .after(crate::raid::downed)
                    .before(ArcadeSystems::Setup),
            );
    }
}

/// No Surge, no truck runs: the raid's clock stays at the start and the
/// raid stays out.
fn hold_the_zone(mut raid: ResMut<Raid>) {
    raid.clock = 0.0;
    if matches!(raid.phase, Phase::AtBase) {
        raid.phase = Phase::Out;
    }
}

fn call_wave(
    mut calls: MessageReader<WaveCalled>,
    mut arcade: ResMut<Arcade>,
    mut raid: ResMut<Raid>,
    zone: Res<Zone>,
    mut siren: MessageWriter<Siren>,
) {
    for _ in calls.read() {
        if !arcade.can_call() {
            continue;
        }
        if matches!(arcade.phase, RunPhase::Over { .. }) {
            // A call after the end starts a new run: RESTART first.
            arcade.reset = super::Reset::Dropped;
            continue;
        }
        let n = arcade.next_wave();
        if matches!(arcade.phase, RunPhase::Ready) {
            arcade.run_t = 0.0;
            arcade.score = 0;
            arcade.runs += 1;
        }
        arcade.phase = RunPhase::Warning {
            wave: n,
            left: WARNING,
        };
        let (name, _, lamp) = GATES[gate(n)];
        raid.say(format!(
            "Wave {n}: {}, through the {} gate in {WARNING:.0} seconds.",
            describe(&wave(n)),
            name.to_lowercase()
        ));
        siren.write(Siren {
            at: Vec3::new(lamp[0], zone.ground(lamp[0], lamp[1]) + 3.0, lamp[1]),
        });
    }
}

fn count_down(
    mut commands: Commands,
    mut arcade: ResMut<Arcade>,
    time: Res<Time>,
    zone: Res<Zone>,
    catalog: Res<crate::items::Catalog>,
    players: Query<&Transform, With<LocalPlayer>>,
) {
    let dt = time.delta_secs();
    if !matches!(arcade.phase, RunPhase::Ready) {
        arcade.run_t += dt;
    }
    let RunPhase::Warning { wave: n, left } = arcade.phase else {
        return;
    };
    let left = left - dt;
    if left > 0.0 {
        arcade.phase = RunPhase::Warning { wave: n, left };
        return;
    }
    let (_, at, _) = GATES[gate(n)];
    let gate_at = Vec3::new(at[0], zone.ground(at[0], at[1]), at[1]);
    // They come for where you are (a response, as the director sends).
    let you = players
        .iter()
        .next()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);
    let job = |site: Vec3| Job {
        kind: JobKind::Response,
        site,
        duration: 3600.0,
        exit: gate_at,
    };
    // Side by side across the gate's mouth, two metres apart.
    let across = Vec3::new(-at[1], 0.0, at[0]).normalize_or_zero();
    let machines = wave(n);
    let count = machines.len();
    for (i, m) in machines.into_iter().enumerate() {
        let off = across * ((i as f32 - (count as f32 - 1.0) * 0.5) * 2.5);
        let p = gate_at + off;
        let site = zone.on_ground(you.x, you.z);
        let e = match m {
            Machine::Walker => {
                crate::robots::walker::spawn_walker(&mut commands, &zone, p, job(site))
            }
            Machine::Drone => crate::robots::drone::spawn_drone(
                &mut commands,
                p + Vec3::Y * 12.0,
                job(site + Vec3::Y * 8.0),
            ),
            Machine::Hound => crate::robots::hound::spawn_hound(&mut commands, &zone, p, job(site)),
            Machine::Heavy => {
                crate::robots::heavy::spawn_heavy(&mut commands, &zone, &catalog, p, job(site))
            }
        };
        commands.entity(e).insert(WaveMachine { wave: n, kind: m });
    }
    info!("arcade: wave {n} in through the {} gate", GATES[gate(n)].0);
    arcade.phase = RunPhase::Fighting { wave: n };
}

/// A machine of the run left blind for [`BLIND_FOR`] (its eye shot out)
/// or wandered [`OUT_BY`] past the fence burns itself out: its core takes
/// what it holds, through the robots' own damage, so it smokes and wrecks
/// as any machine does, and it's said. A wave doesn't hang on a machine
/// that can't find you or has left.
#[allow(clippy::too_many_arguments)]
fn spend_machines(
    machines: Query<(Entity, &Robot, &GlobalTransform), With<WaveMachine>>,
    parts: Query<(Entity, &crate::robots::RobotPart, &GlobalTransform)>,
    zone: Res<Zone>,
    time: Res<Time>,
    mut blind: Local<std::collections::HashMap<Entity, f32>>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
    mut burnt: MessageWriter<MachineSpent>,
    mut raid: ResMut<Raid>,
) {
    use crate::robots::PartRole;
    let dt = time.delta_secs();
    let inner = zone.map.size * 0.5 - zone.map.fence_inset;
    blind.retain(|e, _| machines.contains(*e));
    for (e, robot, gt) in &machines {
        if robot.wrecked {
            continue;
        }
        let eyeless = parts
            .iter()
            .any(|(_, p, _)| p.robot == e && p.role == PartRole::Sensor && !p.alive());
        let t = blind.entry(e).or_insert(0.0);
        *t = if eyeless { *t + dt } else { 0.0 };
        let Some(blind_out) = spent(*t, gt.translation(), inner) else {
            continue;
        };
        let out = !blind_out;
        let Some((core, part, cgt)) = parts
            .iter()
            .find(|(_, p, _)| p.robot == e && p.role == PartRole::Core && p.alive())
        else {
            continue;
        };
        hits.write(crate::weapons::ballistics::Hit {
            entity: core,
            point: cgt.translation(),
            normal: Vec3::Y,
            velocity: Vec3::ZERO,
            damage: part.hp + part.max_hp,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
            blow: None,
        });
        burnt.write(MachineSpent {
            machine: e,
            blind: !out,
        });
        raid.say(if out {
            "A machine past the fence burns itself out."
        } else {
            "The blind machine stops, and burns itself out."
        });
        blind.remove(&e);
    }
}

/// Whether a machine blind `blind_for` seconds and standing `at` (inside
/// a fence `inner` from the middle each way) burns itself out: Some(true)
/// blind too long, Some(false) out past the fence, None neither.
pub fn spent(blind_for: f32, at: Vec3, inner: f32) -> Option<bool> {
    if at.x.abs() > inner + OUT_BY || at.z.abs() > inner + OUT_BY {
        Some(false)
    } else if blind_for >= BLIND_FOR {
        Some(true)
    } else {
        None
    }
}

/// A wave is cleared when none of its machines stands inside: wrecked,
/// or gone back out.
fn clear_wave(
    mut arcade: ResMut<Arcade>,
    machines: Query<(&WaveMachine, &Robot)>,
    mut raid: ResMut<Raid>,
) {
    let RunPhase::Fighting { wave: n } = arcade.phase else {
        return;
    };
    if machines.iter().any(|(w, r)| w.wave == n && !r.wrecked) {
        return;
    }
    let bonus = 50 * n;
    arcade.score += bonus;
    arcade.phase = RunPhase::Cleared { wave: n };
    if n > arcade.best.0 {
        arcade.best.0 = n;
    }
    raid.say(format!(
        "Wave {n} cleared. Call the next at the bench: {}.",
        describe(&wave(n + 1))
    ));
}

fn light_lamps(arcade: Res<Arcade>, mut lamps: Query<&mut GateLamp>) {
    let lit = match arcade.phase {
        RunPhase::Warning { wave, .. } | RunPhase::Fighting { wave } => Some(gate(wave)),
        _ => None,
    };
    for mut l in &mut lamps {
        let on = lit == Some(l.gate);
        if l.lit != on {
            l.lit = on;
        }
    }
}

/// Down in the arcade: the run is over, not the career. The raid's own
/// answer (the convoy, the remains) is turned back the same frame.
#[allow(clippy::too_many_arguments)]
fn downed(
    mut commands: Commands,
    mut events: MessageReader<crate::health::Downed>,
    mut locals: Query<&mut crate::health::Health, With<LocalPlayer>>,
    machines: Query<Entity, With<WaveMachine>>,
    mut arcade: ResMut<Arcade>,
    mut raid: ResMut<Raid>,
    mut over: MessageWriter<RunOver>,
) {
    for ev in events.read() {
        let Ok(mut health) = locals.get_mut(ev.player) else {
            continue;
        };
        raid.phase = Phase::Out;
        // Up again at once, the ground quiet: the summary's on the board,
        // what you carried still on you until RESTART.
        *health = crate::health::Health::default();
        for m in &machines {
            commands.entity(m).despawn();
        }
        let wave = arcade.wave();
        arcade.phase = RunPhase::Over { wave };
        if arcade.score > arcade.best.1 {
            arcade.best.1 = arcade.score;
        }
        raid.say(format!(
            "Down on wave {wave}. Score {}. RESTART at the bench.",
            arcade.score
        ));
        over.write(RunOver { wave });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine burns itself out blind past BLIND_FOR, or past the fence;
    /// a moment blind, or at the fence, it doesn't.
    #[test]
    fn a_blind_or_strayed_machine_is_spent() {
        let inner = 20.0;
        assert_eq!(spent(5.0, Vec3::new(3.0, 0.0, 4.0), inner), None);
        assert_eq!(spent(BLIND_FOR, Vec3::ZERO, inner), Some(true));
        assert_eq!(
            spent(0.0, Vec3::new(inner + 1.0, 0.0, 0.0), inner),
            None,
            "at the fence"
        );
        assert_eq!(
            spent(0.0, Vec3::new(0.0, 0.0, -inner - OUT_BY - 0.5), inner),
            Some(false)
        );
    }

    #[test]
    fn the_ladder_climbs_and_goes_round() {
        assert_eq!(wave(1), vec![Machine::Walker]);
        assert_eq!(wave(6), vec![Machine::Heavy]);
        assert_eq!(wave(7), vec![Machine::Walker, Machine::Walker]);
        assert_eq!(
            wave(13),
            vec![Machine::Walker, Machine::Walker, Machine::Drone]
        );
        for n in 1..40 {
            assert!(
                wave(n + LADDER.len() as u32).len() > wave(n).len(),
                "each time round brings more"
            );
        }
        assert_eq!(describe(&wave(5)), "2 walkers + drone");
        assert_eq!(describe(&wave(1)), "walker");
    }

    #[test]
    fn gates_go_round_and_stand_inside_the_fence() {
        assert_eq!(gate(1), 0);
        assert_eq!(gate(2), 1);
        assert_eq!(gate(4), 0);
        let map = extraction_core::map::MapDef::from_ron(extraction_core::map::ARCADE).unwrap();
        let inner = map.size * 0.5 - map.fence_inset;
        for (_, at, lamp) in GATES {
            for p in [at, lamp] {
                assert!(p[0].abs() < inner && p[1].abs() < inner, "{p:?}");
            }
        }
    }
}
