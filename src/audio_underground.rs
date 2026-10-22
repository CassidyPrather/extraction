//! The underground signature sites, heard (`docs/AUDIO.md`, "Underground's
//! sites"): the drowned works' drains and basins, the deep workings'
//! firedamp, canary, cart and roof, the furnace cellars' heat, vents and
//! crane, the winze's cage.
//!
//! Like `audio.rs`, it only listens: the sites' logic
//! (`world::underground`, `world::workings`, `world::furnace`) knows
//! nothing of it, and it runs only on the client (`audio::AudioPlugin`).
//! Every cue is an `ug.*` line in `audio_mix::MIX`, and the set it plays
//! is the one that line names, so the audit judges what plays.

use bevy::prelude::*;
use bevy_seedling::prelude::*;
use extraction_core::noise::Rng;
use extraction_core::underground as ug;
use std::collections::HashMap;

use crate::audio::{AudioRng, SoundBank, play_at_speed, play_filtered, reverb_send};
use crate::audio_mix as mix;
use crate::interact::InteractEvent;
use crate::world::buildings::Building;
use crate::world::cage::{Cage, CageArrived, CageCrushed, CageRung};
use crate::world::furnace::{Crane, CraneLever, Heat, InHeat, VentLever, Vented};
use crate::world::underground::{BasinDrained, BasinSurface, DrainControl, Levels};
use crate::world::workings::{CART_PUSH, Canary, GasFlash, GasIgnited, InGas, MineCart, RoofFell};

pub struct UndergroundSoundsPlugin;

impl Plugin for UndergroundSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                drain_sounds,
                basin_gurgles,
                gas_sounds,
                canary_sounds,
                cart_sounds,
                roof_sounds,
                heat_sounds,
                vent_sounds,
                heat_ticks,
                crane_sounds,
                lever_sounds,
                cage_sounds,
                ease_loops,
            ),
        );
    }
}

/// The set a cue plays, as its line in the mix names it.
fn set(cue: &str) -> &'static str {
    mix::MIX
        .iter()
        .find(|c| c.name == cue)
        .map_or("", |c| c.set)
}

/// One variant of a cue's set at a point, centred on `speed`.
#[allow(clippy::too_many_arguments)]
fn sound(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    cue: &str,
    at: Vec3,
    offset_db: f32,
    speed: f64,
    pitch: f64,
) {
    play_at_speed(
        commands,
        bank,
        rng,
        set(cue),
        Some(at),
        cue,
        offset_db,
        speed,
        pitch,
    );
}

/// A looping sound kept on for as long as what makes it goes on (a basin
/// draining, a cart rolling, the crane running, a body in the heat), on
/// its `owner`, eased in and out (`ease_loops`) rather than switched.
#[derive(Component)]
struct SiteLoop {
    owner: Entity,
    cue: &'static str,
    /// Its level now and wanted, as amplitude under the cue's gain (0..1).
    now: f32,
    want: f32,
    /// How fast it eases, per second.
    rate: f32,
}

/// Starts a cue's loop on `owner` (at `at`, owner space), silent: it eases
/// up to `want`.
#[allow(clippy::too_many_arguments)]
fn start_loop(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    cue: &'static str,
    owner: Entity,
    at: Vec3,
    speed: f64,
    lowpass: Option<f32>,
    rate: f32,
    want: f32,
) {
    let Some(sample) = bank.pick(set(cue), rng) else {
        return;
    };
    let silent = VolumeNode {
        volume: Volume::SILENT,
        ..default()
    };
    let mut e = commands.spawn((
        SiteLoop {
            owner,
            cue,
            now: 0.0,
            want,
            rate,
        },
        SamplePlayer::new(sample)
            .looping()
            .with_volume(Volume::Decibels(mix::db(cue))),
        PlaybackSettings::default().with_speed(speed),
        Transform::from_translation(at),
        ChildOf(owner),
    ));
    match lowpass {
        Some(hz) => e.insert(sample_effects![
            silent,
            FastLowpassNode::<2> {
                cutoff_hz: hz,
                ..default()
            },
            mix::spatial(cue),
            reverb_send(cue)
        ]),
        None => e.insert(sample_effects![silent, mix::spatial(cue), reverb_send(cue)]),
    };
}

/// Every site loop eased toward what's wanted of it; one wanted off is
/// gone once it's faded under hearing (and with its owner, as its child).
fn ease_loops(
    mut commands: Commands,
    mut loops: Query<(Entity, &mut SiteLoop, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut l, effects) in &mut loops {
        let k = (dt * l.rate).min(1.0);
        l.now += (l.want - l.now) * k;
        if l.want <= 0.0 && l.now < 0.003 {
            commands.entity(e).despawn();
            continue;
        }
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if l.now < 0.003 {
                Volume::SILENT
            } else {
                Volume::Decibels(20.0 * l.now.log10())
            };
        }
    }
}

/// The site's drains, worked:
///
/// - a valve wheel turned the first time shrieks round (`ug.valve`: the
///   door's creak, slowed, the rusted stem giving);
/// - each stroke of a hand pump: its iron handle knocking (`ug.pump`) and
///   a gout of water out of the spout (`ug.pump_water`);
/// - a basin drained to its last (`BasinDrained`): a long glug where its
///   water was, as the pipe sucks air (`ug.drained`).
#[allow(clippy::too_many_arguments)]
fn drain_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    mut drained: MessageReader<BasinDrained>,
    controls: Query<(Ref<DrainControl>, &GlobalTransform)>,
    buildings: Query<&Building>,
    surfaces: Query<(&BasinSurface, &ChildOf, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let kind = |c: &DrainControl| {
        buildings
            .get(c.site)
            .ok()
            .and_then(|b| ug::layout(&b.prefab))
            .and_then(|l| l.drains.get(c.drain))
            .map(|d| d.kind)
    };
    // A valve is opened once: its control changes then, and only then.
    for (c, at) in &controls {
        if c.is_changed() && !c.is_added() && c.open {
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.valve",
                at.translation(),
                0.0,
                0.75,
                0.05,
            );
        }
    }
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((c, at)) = controls.get(*item) else {
            continue;
        };
        if kind(&c) != Some(ug::DrainKind::Pump) {
            continue;
        }
        let p = at.translation();
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.pump",
            p,
            0.0,
            1.2,
            0.08,
        );
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.pump_water",
            p - Vec3::Y * 0.6,
            0.0,
            0.75,
            0.1,
        );
    }
    for d in drained.read() {
        let Some(at) = surfaces
            .iter()
            .find(|(s, parent, _)| s.0 == d.basin && parent.parent() == d.site)
            .map(|(.., g)| g.translation())
        else {
            continue;
        };
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.drained",
            at,
            0.0,
            0.5,
            0.05,
        );
    }
}

/// How long a basin keeps gurgling after its level last fell (s): a pump
/// stroke's water runs on a moment.
pub const GURGLE_HOLD: f32 = 1.2;

/// A basin draining (its level in `Levels` falling): a deep gurgle from
/// its water (`ug.gurgle`: the creek slowed to 0.6 and through 900 Hz,
/// water going down a pipe), easing out when it stops.
#[allow(clippy::too_many_arguments)]
fn basin_gurgles(
    mut commands: Commands,
    levels: Option<Res<Levels>>,
    surfaces: Query<(Entity, &BasinSurface, &ChildOf)>,
    buildings: Query<&Building>,
    mut loops: Query<&mut SiteLoop>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut seen: Local<HashMap<(String, usize), (f32, f32)>>,
) {
    let Some(levels) = levels else {
        return;
    };
    let now = time.elapsed_secs();
    // (level last frame, gurgling until), by site and basin.
    for (site, basins) in &levels.0 {
        for (k, level) in basins.iter().enumerate() {
            let e = seen.entry((site.clone(), k)).or_insert((*level, 0.0));
            if *level < e.0 - 1e-5 {
                e.1 = now + GURGLE_HOLD;
            }
            e.0 = *level;
        }
    }
    seen.retain(|(site, k), _| levels.level(site, *k).is_some());
    // One voice a basin: its first surface.
    let mut voice: HashMap<(Entity, usize), Entity> = HashMap::new();
    for (e, s, parent) in &surfaces {
        voice
            .entry((parent.parent(), s.0))
            .and_modify(|v| *v = (*v).min(e))
            .or_insert(e);
    }
    let mut draining = Vec::new();
    for ((site, k), surface) in voice {
        let Ok(b) = buildings.get(site) else {
            continue;
        };
        if seen.get(&(b.prefab.clone(), k)).is_some_and(|v| now < v.1) {
            draining.push(surface);
        }
    }
    for mut l in &mut loops {
        if l.cue == "ug.gurgle" {
            l.want = if draining.contains(&l.owner) {
                1.0
            } else {
                0.0
            };
        }
    }
    for surface in draining {
        if !loops
            .iter()
            .any(|l| l.cue == "ug.gurgle" && l.owner == surface)
        {
            start_loop(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.gurgle",
                surface,
                Vec3::ZERO,
                0.6,
                Some(900.0),
                1.5,
                1.0,
            );
        }
    }
}

/// Firedamp:
///
/// - a body breathing it (`InGas`): a cough every couple of seconds
///   (`ug.cough`, a fit of one to three), a thin wheeze between
///   (`ug.wheeze`, the breath sped up), at the head;
/// - a pocket going up (`GasFlash`): a huge muffled whump where it went
///   (`ug.whump`: the heavy grenade slowed to 0.7, through 300 Hz: the
///   blast heard through rock and its own smoke, not a bang), and
///   (`GasIgnited`) the timbers groaning as the shock goes through them
///   (`ug.timbers`).
#[allow(clippy::too_many_arguments)]
fn gas_sounds(
    mut commands: Commands,
    breathing: Query<(Entity, &GlobalTransform), With<InGas>>,
    mut flashes: MessageReader<GasFlash>,
    mut ignited: MessageReader<GasIgnited>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    next.retain(|e, _| breathing.contains(*e));
    for (e, gt) in &breathing {
        let due = next.entry(e).or_insert(now + 0.4);
        if now < *due {
            continue;
        }
        let head = gt.translation() + Vec3::Y * 1.6;
        if rng.0.f32() < 0.65 {
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cough",
                head,
                0.0,
                1.0,
                0.08,
            );
        } else {
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.wheeze",
                head,
                0.0,
                1.3,
                0.05,
            );
        }
        *due = now + rng.0.range(1.6, 3.2);
    }
    for f in flashes.read() {
        play_filtered(
            &mut commands,
            &bank,
            &mut rng.0,
            set("ug.whump"),
            f.at,
            "ug.whump",
            0.7,
            WHUMP_HZ,
        );
    }
    for g in ignited.read() {
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.timbers",
            g.at + Vec3::Y * 1.5,
            0.0,
            0.7,
            0.08,
        );
    }
}

/// The firedamp's whump: the low-pass it's heard through, Hz.
pub const WHUMP_HZ: f32 = 300.0;

/// A canary heard this near (m); past it, it's left to sing unheard.
pub const CANARY_HEARD: f32 = 30.0;

/// A canary's cage (`Canary`): a calm chirp now and then while there's
/// no gas near it; with gas near, agitated runs (`ug.canary`, a stand-in
/// trill) coming quicker, louder and higher as its `alarm` rises, four a
/// second in the gas itself. Dead (`life` 0), it's silent, and the silence
/// is the warning.
fn canary_sounds(
    mut commands: Commands,
    canaries: Query<(Entity, &Canary, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut next: Local<HashMap<Entity, f32>>,
) {
    let now = time.elapsed_secs();
    next.retain(|e, _| canaries.get(*e).is_ok_and(|(_, c, _)| c.life > 0.0));
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    for (e, c, gt) in &canaries {
        if c.life <= 0.0 {
            continue;
        }
        let at = gt.translation();
        let first = now + rng.0.range(0.5, 3.0);
        let due = next.entry(e).or_insert(first);
        if now < *due {
            continue;
        }
        let a = c.alarm.clamp(0.0, 1.0);
        *due = if a < 0.05 {
            now + rng.0.range(4.0, 9.0)
        } else {
            now + (2.0 - 1.75 * a) * rng.0.range(0.8, 1.2)
        };
        if at.distance(ear) > CANARY_HEARD {
            continue;
        }
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.canary",
            at + Vec3::Y * 0.15,
            -10.0 + 10.0 * a,
            1.0 + 0.12 * a as f64,
            0.04,
        );
    }
}

/// Rail joints, every this many metres along a cart's run.
pub const RAIL_JOINT: f32 = 2.5;

/// A mine cart (`MineCart`) rolling: iron wheels rumbling on the rail
/// (`ug.cart`: the truck's idle slowed and through 600 Hz, louder and
/// quicker with its speed), a clack at each rail joint (`ug.cart_clack`),
/// and a clank when it fetches up hard against the rail's end
/// (`ug.cart_stop`).
#[allow(clippy::too_many_arguments)]
fn cart_sounds(
    mut commands: Commands,
    carts: Query<(Entity, &MineCart, &GlobalTransform)>,
    mut loops: Query<(&mut SiteLoop, &mut PlaybackSettings)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut rolled: Local<HashMap<Entity, (f32, f32, f32)>>,
) {
    rolled.retain(|e, _| carts.contains(*e));
    for (e, cart, gt) in &carts {
        let speed = cart.v.abs() / CART_PUSH;
        let p = gt.translation();
        // (where it was along its rail, its speed, metres since a joint)
        let (s, v, joint) = rolled.entry(e).or_insert((cart.s, cart.v, 0.0));
        *joint += (cart.s - *s).abs();
        if *joint >= RAIL_JOINT {
            *joint = 0.0;
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cart_clack",
                p,
                20.0 * speed.max(0.25).log10(),
                1.0,
                0.08,
            );
        }
        if v.abs() > 0.6 && cart.v == 0.0 {
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cart_stop",
                p,
                20.0 * (v.abs() / CART_PUSH).clamp(0.3, 1.0).log10(),
                0.85,
                0.05,
            );
        }
        *s = cart.s;
        *v = cart.v;
        let want = speed.clamp(0.0, 1.0);
        let mut found = false;
        for (mut l, mut settings) in &mut loops {
            if l.cue == "ug.cart" && l.owner == e {
                found = true;
                l.want = want;
                let rate = 0.5 + 0.3 * want as f64;
                if (settings.speed - rate).abs() > 0.02 {
                    settings.speed = rate;
                }
            }
        }
        if !found && want > 0.02 {
            start_loop(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cart",
                e,
                Vec3::Y * 0.3,
                0.5 + 0.3 * want as f64,
                Some(CART_HZ),
                6.0,
                want,
            );
        }
    }
}

/// The cart's rumble: the low-pass it's heard through, Hz.
pub const CART_HZ: f32 = 600.0;

/// A roof coming down where its shoring was shot out (`RoofFell`): the
/// rock crashing in (`ug.roof`), a heavy thud of it landing
/// (`ug.roof_crash`), then, a second or two on, the dust and grit still
/// trickling (`ug.dust`, the rockfall sped up and quiet).
fn roof_sounds(
    mut commands: Commands,
    mut fell: MessageReader<RoofFell>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut pending: Local<Vec<(f32, Vec3)>>,
) {
    let now = time.elapsed_secs();
    for f in fell.read() {
        let at = f.at + Vec3::Y * 1.5;
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.roof",
            at,
            0.0,
            0.85,
            0.05,
        );
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.roof_crash",
            f.at,
            0.0,
            0.7,
            0.05,
        );
        pending.push((now + rng.0.range(1.2, 2.0), at));
    }
    let due: Vec<_> = pending.iter().filter(|p| p.0 <= now).copied().collect();
    pending.retain(|p| p.0 > now);
    for (_, at) in due {
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.dust",
            at,
            0.0,
            1.5,
            0.1,
        );
    }
}

/// Every how long the heat's breath comes (s).
pub const HEAT_BREATH_EVERY: f32 = 2.2;

/// A body in the furnace cellars' heat (`InHeat`): a sizzle about it
/// (`ug.sizzle`, the burner's roar sped up), louder the hotter, eased out
/// as it leaves; and your own breath coming hard and dry in it
/// (`ug.heat_breath`) when it's past a quarter.
fn heat_sounds(
    mut commands: Commands,
    bodies: Query<(
        Entity,
        &InHeat,
        &GlobalTransform,
        Has<crate::rig::LocalPlayer>,
    )>,
    mut loops: Query<&mut SiteLoop>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut breath: Local<f32>,
) {
    for mut l in &mut loops {
        if l.cue == "ug.sizzle" {
            l.want = bodies.get(l.owner).map_or(0.0, |b| b.1.0.clamp(0.0, 1.0));
        }
    }
    *breath -= time.delta_secs();
    for (e, heat, gt, local) in &bodies {
        let h = heat.0.clamp(0.0, 1.0);
        if !loops.iter().any(|l| l.cue == "ug.sizzle" && l.owner == e) {
            start_loop(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.sizzle",
                e,
                Vec3::Y * 1.2,
                1.8,
                None,
                3.0,
                h,
            );
        }
        if local && h > 0.25 && *breath <= 0.0 {
            *breath = HEAT_BREATH_EVERY;
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.heat_breath",
                gt.translation() + Vec3::Y * 1.6,
                20.0 * h.log10(),
                0.9,
                0.05,
            );
        }
    }
}

/// A vent's damper opened (`Vented`): it clangs open in the flue over its
/// lever (`ug.damper`, a metal impact slowed), and a long breath of hot
/// air goes up it (`ug.flue`, the whoosh slowed).
fn vent_sounds(
    mut commands: Commands,
    mut vented: MessageReader<Vented>,
    levers: Query<(&VentLever, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for v in vented.read() {
        let Some(at) = levers
            .iter()
            .find(|(l, _)| l.site == v.site && l.vent == v.vent)
            .map(|(_, g)| g.translation() + Vec3::Y * 1.5)
        else {
            continue;
        };
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.damper",
            at,
            0.0,
            0.55,
            0.05,
        );
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.flue",
            at,
            0.0,
            0.45,
            0.05,
        );
    }
}

/// Ticking heard this near (m).
pub const TICKS_HEARD: f32 = 35.0;

/// A heat zone cooling (its share in `Heat` falling, a vent open): the
/// metal of the room ticking as it contracts (`ug.heat_tick`, a casing's
/// tink slowed), here and there about the room, quick at first and
/// slowing as it cools, stopping when it's done.
#[allow(clippy::too_many_arguments)]
fn heat_ticks(
    mut commands: Commands,
    heat: Option<Res<Heat>>,
    sites: Query<(&Building, &GlobalTransform)>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut cooling: Local<HashMap<(Entity, usize), (f32, f32)>>,
) {
    let Some(heat) = heat else {
        return;
    };
    cooling.retain(|(site, _), _| sites.contains(*site));
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let now = time.elapsed_secs();
    for (site, shares) in &heat.0 {
        let Ok((b, gt)) = sites.get(*site) else {
            continue;
        };
        for (z, share) in shares.iter().enumerate() {
            // (its share last frame, the next tick)
            let (last, next) = cooling.entry((*site, z)).or_insert((*share, 0.0));
            let falling = *share < *last - 1e-6;
            *last = *share;
            if !falling || now < *next {
                continue;
            }
            *next = now + rng.0.range(0.2, 0.9) * (1.0 + 2.0 * (1.0 - share));
            let Some((l, room)) = ug::layout(&b.prefab).and_then(|l| {
                l.heat
                    .get(z)
                    .and_then(|h| h.room.as_ref())
                    .and_then(|id| l.rooms.iter().find(|r| &r.id == id))
                    .map(|r| (l, r))
            }) else {
                continue;
            };
            let local = Vec3::new(
                room.at[0] + room.size[0] * rng.0.range(-0.4, 0.4),
                l.floor_of(room) + rng.0.range(1.0, 2.5),
                room.at[1] + room.size[1] * rng.0.range(-0.4, 0.4),
            );
            let at = gt.transform_point(local);
            if at.distance(ear) > TICKS_HEARD {
                continue;
            }
            let quiet = -6.0 * rng.0.f32();
            let speed = rng.0.range(0.55, 0.9) as f64;
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.heat_tick",
                at,
                quiet,
                speed,
                0.0,
            );
        }
    }
}

/// The charging crane (`Crane`) running: its trolley's motor whining
/// (`ug.crane`, the hum sped up) while it goes, its chain clinking at the
/// hook (`ug.chain`), and a clank as its brake lets go and again as it
/// fetches up at the end of its run (`ug.crane_clank`).
#[allow(clippy::too_many_arguments)]
fn crane_sounds(
    mut commands: Commands,
    cranes: Query<(Entity, &Crane, &GlobalTransform)>,
    mut loops: Query<&mut SiteLoop>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut was: Local<HashMap<Entity, (f32, f32)>>,
) {
    was.retain(|e, _| cranes.contains(*e));
    let now = time.elapsed_secs();
    for (e, crane, gt) in &cranes {
        let top = gt.translation();
        let running = crane.dir != 0.0;
        // (its direction last frame, the next chain clink)
        let (dir, clink) = was.entry(e).or_insert((crane.dir, 0.0));
        if running != (*dir != 0.0) {
            let speed = if running { 1.0 } else { 0.8 };
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.crane_clank",
                top,
                0.0,
                speed,
                0.05,
            );
        }
        *dir = crane.dir;
        if running && now >= *clink {
            *clink = now + rng.0.range(0.25, 0.5);
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.chain",
                top - Vec3::Y,
                0.0,
                0.9,
                0.1,
            );
        }
        let want = if running { 1.0 } else { 0.0 };
        let mut found = false;
        for mut l in &mut loops {
            if l.cue == "ug.crane" && l.owner == e {
                found = true;
                l.want = want;
            }
        }
        if !found && running {
            start_loop(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.crane",
                e,
                Vec3::ZERO,
                1.6,
                None,
                4.0,
                1.0,
            );
        }
    }
}

/// Strokes of the cage's bell to a ring, and the time between them (s).
const BELL_STROKES: usize = 3;
const BELL_GAP: f32 = 0.45;

/// The winze's cage (`world::cage`), whose warning has to be unmissable:
///
/// - a pull rung (`CageRung`): the mine bell, three strokes, carrying
///   down the shaft (`ug.cage_bell`, heard to the bell's 35 m);
/// - its wait out, the winding engine at the shaft's head taking the
///   strain (`ug.cage_strain`), then the rope singing over the sheave as
///   long as it rides (`ug.cage_rope`, from the head, not the cage);
/// - at a stop (`CageArrived`): the cage landing on its keps
///   (`ug.cage_land`) and the gate's latch (`ug.cage_gate`);
/// - come down on someone (`CageCrushed`): the iron floor's heavy thud
///   and the body under it (`ug.cage_crush`, `ug.cage_body`).
#[allow(clippy::too_many_arguments)]
fn cage_sounds(
    mut commands: Commands,
    mut rung: MessageReader<CageRung>,
    mut arrived: MessageReader<CageArrived>,
    mut crushed: MessageReader<CageCrushed>,
    cages: Query<(Entity, &Cage, &GlobalTransform)>,
    places: Query<&GlobalTransform>,
    mut loops: Query<(&mut SiteLoop, &mut Transform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut strokes: Local<Vec<(f32, Vec3)>>,
    mut was: Local<HashMap<Entity, bool>>,
) {
    let now = time.elapsed_secs();
    for r in rung.read() {
        for i in 0..BELL_STROKES {
            strokes.push((now + i as f32 * BELL_GAP, r.at));
        }
    }
    strokes.retain(|&(t, at)| {
        if now < t {
            return true;
        }
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.cage_bell",
            at,
            0.0,
            0.9,
            0.02,
        );
        false
    });
    was.retain(|e, _| cages.contains(*e));
    for (e, cage, gt) in &cages {
        let riding = cage.to.is_some() && cage.wait <= 0.0;
        // The head, in the cage's own space: the rope's loop sits there.
        let head = Vec3::Y * (cage.head - cage.y);
        let before = was.insert(e, riding).unwrap_or(false);
        if riding && !before {
            sound(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cage_strain",
                gt.translation() + head,
                0.0,
                0.7,
                0.05,
            );
        }
        let want = if riding { 1.0 } else { 0.0 };
        let mut found = false;
        for (mut l, mut t) in &mut loops {
            if l.cue == "ug.cage_rope" && l.owner == e {
                found = true;
                l.want = want;
                t.translation = head;
            }
        }
        if !found && riding {
            start_loop(
                &mut commands,
                &bank,
                &mut rng.0,
                "ug.cage_rope",
                e,
                head,
                1.3,
                None,
                3.0,
                1.0,
            );
        }
    }
    for a in arrived.read() {
        let Some((_, _, gt)) = cages.iter().find(|(_, c, _)| c.site == a.site) else {
            continue;
        };
        let at = gt.translation();
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.cage_land",
            at,
            0.0,
            0.8,
            0.05,
        );
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.cage_gate",
            at + Vec3::Y,
            0.0,
            0.9,
            0.08,
        );
    }
    for c in crushed.read() {
        let Ok(gt) = places.get(c.player) else {
            continue;
        };
        let at = gt.translation();
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.cage_crush",
            at,
            0.0,
            0.5,
            0.05,
        );
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.cage_body",
            at,
            0.0,
            0.85,
            0.05,
        );
    }
}

/// The cellars' iron levers (a vent's, the crane's): a heavy thunk under
/// the press's click (`ug.lever`).
fn lever_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    levers: Query<&GlobalTransform, Or<(With<VentLever>, With<CraneLever>)>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok(at) = levers.get(*item) else {
            continue;
        };
        sound(
            &mut commands,
            &bank,
            &mut rng.0,
            "ug.lever",
            at.translation(),
            0.0,
            1.3,
            0.05,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cue this file plays is a line in the mix, so its set is one
    /// the audit measures.
    #[test]
    fn every_cue_is_in_the_mix() {
        let code = include_str!("audio_underground.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let mut n = 0;
        for lit in code.split('"').skip(1).step_by(2) {
            if lit.starts_with("ug.") && !lit.contains(' ') {
                assert!(!set(lit).is_empty(), "`{lit}` is not in the mix");
                n += 1;
            }
        }
        assert!(n > 20, "{n} cues found");
    }
}
