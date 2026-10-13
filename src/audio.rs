//! Sound: recorded, mastered CC0 sets (see `docs/AUDIO.md`) played through
//! `bevy_seedling` with basic spatialisation.
//!
//! Everything here listens to gameplay messages — footsteps, gun events,
//! hits, grabs — and turns them into sound. Nothing in the game logic
//! knows audio exists, so the headless sims don't either.

use bevy::prelude::*;
use bevy_seedling::prelude::*;
use extraction_core::firearm::{Event as FireEvent, Round};
use extraction_core::noise::Rng;
use extraction_core::terrain::Surface;
use std::collections::HashMap;

use crate::interact::InteractEvent;
use crate::player::Footstep;
use crate::rig::RigHead;
use crate::robots::drone::Drone;
use crate::robots::{Robot, RobotPart};
use crate::weapons::GunEvent;
use crate::weapons::ballistics::{Hit, Shot};
use crate::world::Zone;

/// Every sound set by name (`"guns/rifle_shot"`), as loaded handles.
#[derive(Resource, Default)]
pub struct SoundBank {
    sets: HashMap<String, Vec<Handle<AudioSample>>>,
}

impl SoundBank {
    pub fn has(&self, set: &str) -> bool {
        self.sets.contains_key(set)
    }

    fn pick(&self, set: &str, rng: &mut Rng) -> Option<Handle<AudioSample>> {
        let v = self.sets.get(set)?;
        (!v.is_empty()).then(|| v[rng.below(v.len())].clone())
    }
}

#[derive(Resource)]
struct AudioRng(Rng);

/// A positioned, looping sound that follows its parent (drone rotors).
#[derive(Component)]
struct RotorLoop;

/// The rain bed, as loud as the weather says.
#[derive(Component)]
struct RainLoop;

fn rain_volume(
    conditions: Res<crate::world::weather::Conditions>,
    loops: Query<&SampleEffects, With<RainLoop>>,
    mut volumes: Query<&mut VolumeNode>,
) {
    for effects in &loops {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            let db = if conditions.rain < 0.01 {
                -80.0
            } else {
                -30.0 + 18.0 * conditions.rain.sqrt()
            };
            v.volume = Volume::Decibels(db);
        }
    }
}

/// The Surge's roar, faded with how much Surge is in the air.
#[derive(Component)]
struct SurgeDrone;

/// A ringing in the ears that grows with attention (`anomaly::Attention`).
#[derive(Component)]
struct AttentionLoop;

/// The ambient creek bed, moved along the bank to stay nearest the listener.
#[derive(Component)]
struct CreekLoop;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SeedlingPlugins)
            .insert_resource(AudioRng(Rng::new(0xA0D10)))
            .add_systems(Startup, load_bank)
            .add_systems(PostStartup, (attach_listener, start_ambience))
            .add_systems(
                Update,
                (
                    footsteps,
                    gun_sounds,
                    shot_sounds,
                    hit_sounds,
                    hand_sounds,
                    rotor_loops,
                    gadget_loops,
                    door_sounds,
                    walker_steps,
                    pylon_sounds,
                    radio_sounds,
                    chatter_sounds,
                    lock_sounds,
                    drop_sounds,
                ),
            )
            .add_systems(
                Update,
                (
                    indoor_ambience,
                    attention_hum,
                    ambient_oneshots,
                    move_creek,
                    surge_sounds,
                    anomaly_loops,
                    anomaly_sounds,
                    rain_volume,
                ),
            );
    }
}

fn load_bank(mut commands: Commands, server: Res<AssetServer>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/audio/sounds.json");
    let mut bank = SoundBank::default();
    let Ok(text) = std::fs::read_to_string(&path) else {
        warn!("no assets/audio/sounds.json: running silent (run scripts/fetch_audio.py)");
        commands.insert_resource(bank);
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        warn!("sounds.json does not parse");
        commands.insert_resource(bank);
        return;
    };
    if let Some(obj) = json.as_object() {
        for (name, set) in obj {
            let files: Vec<Handle<AudioSample>> = set["files"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|f| f["file"].as_str())
                        .map(|f| server.load(f.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            bank.sets.insert(name.clone(), files);
        }
    }
    info!("sound bank: {} sets", bank.sets.len());
    commands.insert_resource(bank);
}

/// The local player's head hears (it follows the headset in VR, the
/// mouse-look camera when flat).
fn attach_listener(
    mut commands: Commands,
    heads: Query<(Entity, &ChildOf), With<RigHead>>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
) {
    for (e, parent) in &heads {
        if locals.contains(parent.parent()) {
            commands.entity(e).insert(SpatialListener3D);
        }
    }
}

/// Plays one variant of a set at a point (or unspatialised with `None`).
fn play(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Option<Vec3>,
    gain_db: f32,
    pitch: f64,
) {
    let Some(sample) = bank.pick(set, rng) else {
        return;
    };
    let speed = 1.0 + (rng.f32() as f64 - 0.5) * 2.0 * pitch;
    let player = SamplePlayer::new(sample).with_volume(Volume::Decibels(gain_db));
    let settings = PlaybackSettings::default().with_speed(speed);
    match at {
        Some(p) => {
            commands.spawn((
                player,
                settings,
                Transform::from_translation(p),
                sample_effects![SpatialBasicNode::default()],
            ));
        }
        None => {
            commands.spawn((player, settings));
        }
    }
}

fn surface_set(s: Surface) -> &'static str {
    match s {
        Surface::Grass => "footsteps/grass",
        Surface::Forest => "footsteps/leaves",
        Surface::Dirt => "footsteps/mud",
        Surface::Mud => "footsteps/mud",
        Surface::Rock | Surface::Asphalt => "footsteps/concrete",
        Surface::Gravel => "footsteps/gravel",
        Surface::Water => "footsteps/water",
    }
}

fn footsteps(
    mut commands: Commands,
    mut steps: MessageReader<Footstep>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    zone: Res<Zone>,
    trucks: Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
) {
    for s in steps.read() {
        let p = [s.position.x, s.position.z];
        let in_truck = trucks
            .iter()
            .any(|(gt, t)| t.in_bed(gt, s.position + Vec3::Y * 0.2));
        let set = if in_truck {
            "footsteps/wood"
        } else {
            surface_set(zone.hm.surface(p, zone.in_forest(p[0], p[1])))
        };
        let gain = -14.0 + (s.speed / 4.0).min(1.0) * 6.0;
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(s.position),
            gain,
            0.06,
        );
    }
}

fn shot_set(item: &str) -> &'static str {
    match item {
        "pm_pistol" => "guns/pistol_shot",
        "ak74" => "guns/rifle_shot",
        "mosin" => "guns/bolt_rifle_shot",
        "toz_double" => "guns/shotgun_shot",
        _ => "guns/rifle_shot",
    }
}

fn gun_sounds(
    mut commands: Commands,
    mut events: MessageReader<GunEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    guns: Query<&crate::weapons::Gun>,
) {
    let ear = listener
        .iter()
        .next()
        .map(|g| g.translation())
        .unwrap_or_default();
    for ev in events.read() {
        let action = guns.get(ev.gun).map(|g| g.firearm.def.action).ok();
        use extraction_core::firearm::ActionKind::*;
        let (set, gain) = match (&ev.event, action) {
            (FireEvent::Fired { .. }, _) => {
                let far = ev.at.distance(ear) > 80.0;
                if far {
                    ("guns/distant_shot", -6.0)
                } else {
                    (shot_set(&ev.item), 0.0)
                }
            }
            (FireEvent::DryFire | FireEvent::Dud, _) => ("guns/dry_fire", -8.0),
            (FireEvent::MagazineInserted, _) => ("guns/mag_insert", -6.0),
            (FireEvent::MagazineRemoved, _) => ("guns/mag_release", -6.0),
            (FireEvent::ModeChanged(_), _) => ("guns/safety", -10.0),
            (FireEvent::RoundLoaded, Some(BreakAction)) => ("guns/shell_insert", -8.0),
            (FireEvent::RoundLoaded, _) => ("guns/stripper_clip", -10.0),
            (FireEvent::BoltClosed, Some(BoltAction)) => ("guns/bolt_close", -6.0),
            (FireEvent::BoltClosed, Some(BreakAction)) => ("guns/shotgun_close", -4.0),
            (FireEvent::BoltClosed, Some(SemiAuto)) if ev.item == "ak74" => {
                ("guns/charging_handle", -6.0)
            }
            (FireEvent::BoltClosed, Some(SemiAuto)) => ("guns/slide_rack", -6.0),
            (FireEvent::Cocked, Some(BoltAction)) => ("guns/bolt_open", -6.0),
            (FireEvent::Cocked, Some(BreakAction)) => ("guns/shotgun_open", -4.0),
            (FireEvent::Ejected(Round::Spent), _) => ("guns/casing", -16.0),
            (FireEvent::Ejected(Round::Live), _) => ("guns/casing", -12.0),
            _ => continue,
        };
        // Shots aren't pitch-shifted much: a gun has one voice.
        let pitch = if matches!(ev.event, FireEvent::Fired { .. }) {
            0.03
        } else {
            0.07
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(ev.at),
            gain,
            pitch,
        );
    }
}

/// Robot guns have no GunEvent; their shots sound here.
fn shot_sounds(
    mut commands: Commands,
    mut shots: MessageReader<Shot>,
    robots: Query<(), With<Robot>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in shots.read() {
        if s.shooter.is_some_and(|e| robots.contains(e)) {
            // A tighter, higher crack than the player's guns.
            let speed = 1.25;
            if let Some(sample) = bank.pick("guns/pistol_shot", &mut rng.0) {
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(-3.0)),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(s.origin),
                    sample_effects![SpatialBasicNode::default()],
                ));
            }
        }
    }
}

fn hit_sounds(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    parts: Query<(), With<RobotPart>>,
    trees: Query<(), With<crate::world::trees::TreeTrunks>>,
    structures: Query<(), With<crate::world::structures::Structure>>,
    items: Query<(), With<crate::items::Item>>,
) {
    for h in hits.read() {
        let set = if parts.contains(h.entity) || items.contains(h.entity) {
            "impacts/metal"
        } else if trees.contains(h.entity) {
            "impacts/wood"
        } else if structures.contains(h.entity) {
            "impacts/concrete"
        } else {
            "impacts/dirt"
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(h.point),
            -6.0,
            0.08,
        );
        if rng.0.chance(0.12) && set != "impacts/dirt" {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "impacts/ricochet",
                Some(h.point),
                -10.0,
                0.1,
            );
        }
    }
}

fn hand_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    poses: Query<&GlobalTransform>,
) {
    for ev in events.read() {
        let (item, set, gain) = match ev {
            InteractEvent::Grabbed { item, .. } => (*item, "physical/cloth", -20.0),
            InteractEvent::Stowed { item } => (*item, "physical/zip", -12.0),
            InteractEvent::Socketed { item, .. } => (*item, "physical/velcro", -14.0),
            InteractEvent::Refused { item } => (*item, "physical/cloth", -10.0),
            _ => continue,
        };
        let at = poses.get(item).map(|g| g.translation()).ok();
        play(&mut commands, &bank, &mut rng.0, set, at, gain, 0.1);
    }
}

/// Each drone carries a rotor drone; pitch follows thrust.
fn rotor_loops(
    mut commands: Commands,
    drones: Query<(Entity, &Drone, Option<&Children>), With<Robot>>,
    loops: Query<Entity, With<RotorLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<RotorLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, d, children) in &drones {
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        let thrust: f32 = d.thrust.iter().sum::<f32>() / (4.0 * 15.0);
        match existing {
            None => {
                if let Some(sample) = bank.pick("machines/drone_rotor", &mut rng.0) {
                    commands.spawn((
                        RotorLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(-4.0)),
                        PlaybackSettings::default(),
                        Transform::default(),
                        sample_effects![SpatialBasicNode::default()],
                        ChildOf(e),
                    ));
                }
            }
            Some(l) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    s.speed = (0.85 + thrust.clamp(0.0, 1.5) as f64 * 0.3).max(0.2);
                }
            }
        }
    }
}

fn start_ambience(mut commands: Commands, bank: Res<SoundBank>, mut rng: ResMut<AudioRng>) {
    if let Some(sample) = bank.pick("ambience/rain_light", &mut rng.0) {
        commands.spawn((
            RainLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("ambience/wind_conifers", &mut rng.0) {
        commands.spawn((
            WindLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::Decibels(WIND_DB),
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("ambience/room_tone", &mut rng.0) {
        commands.spawn((
            RoomLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("anomaly/attention", &mut rng.0) {
        commands.spawn((
            AttentionLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("anomaly/surge_drone", &mut rng.0) {
        commands.spawn((
            SurgeDrone,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::SILENT,
                ..default()
            }],
        ));
    }
    if let Some(sample) = bank.pick("ambience/creek", &mut rng.0) {
        commands.spawn((
            CreekLoop,
            SamplePlayer::new(sample)
                .looping()
                .with_volume(Volume::Decibels(-4.0)),
            PlaybackSettings::default(),
            Transform::default(),
            sample_effects![SpatialBasicNode::default()],
        ));
    }
}

/// Crows, distant thunder: now and then, somewhere out in the fog.
fn ambient_oneshots(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    mut next: Local<(f32, f32)>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let dt = time.delta_secs();
    next.0 -= dt;
    next.1 -= dt;
    if next.0 <= 0.0 {
        next.0 = rng.0.range(12.0, 45.0);
        let a = rng.0.range(0.0, std::f32::consts::TAU);
        let d = rng.0.range(25.0, 70.0);
        let at = ear + Vec3::new(a.cos() * d, rng.0.range(4.0, 15.0), a.sin() * d);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "ambience/crow",
            Some(at),
            -6.0,
            0.08,
        );
    }
    if next.1 <= 0.0 {
        next.1 = rng.0.range(60.0, 180.0);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "ambience/thunder_distant",
            None,
            -14.0,
            0.05,
        );
    }
}

/// The creek sound sits at the nearest point of the water to the listener.
fn move_creek(
    mut creek: Query<&mut Transform, With<CreekLoop>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    mut lines: Local<Option<Vec<Vec<[f32; 2]>>>>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let lines = lines.get_or_insert_with(|| {
        zone.map
            .water
            .iter()
            .map(|w| extraction_core::geom::catmull_rom(&w.points, 4.0))
            .collect()
    });
    let p = [ear.x, ear.z];
    let mut best: Option<(f32, [f32; 2])> = None;
    for line in lines.iter() {
        for q in line {
            let d = extraction_core::geom::dist(*q, p);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, *q));
            }
        }
    }
    if let (Some((_, q)), Ok(mut t)) = (best, creek.single_mut()) {
        t.translation = zone.on_ground(q[0], q[1]);
    }
}

/// The drone swells with the Surge; discharges crack overhead; the sirens
/// at the Gate wail when the warning starts.
fn surge_sounds(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    raid: Res<crate::raid::Raid>,
    mut flashes: MessageReader<crate::world::sky::SurgeFlash>,
    drones: Query<&SampleEffects, With<SurgeDrone>>,
    mut volumes: Query<&mut VolumeNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    mut warned: Local<bool>,
) {
    let level = raid.surge_level();
    for effects in &drones {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            let db = if level < 0.01 {
                -80.0
            } else {
                -30.0 + 30.0 * level.sqrt()
            };
            v.volume = Volume::Decibels(db);
        }
    }
    let ear = listener.iter().next().map(|g| g.translation());
    for f in flashes.read() {
        let at = ear.map(|e| e + f.dir * 120.0);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "anomaly/surge_crack",
            at,
            -2.0 + 6.0 * f.strength,
            0.12,
        );
    }
    let warning = raid.surge_in() <= crate::raid::SURGE_WARNING && raid.surge_in() > 0.0;
    if warning && !*warned && raid.phase != crate::raid::Phase::AtBase {
        let gate = zone
            .marker(&extraction_core::map::MarkerKind::RobotGate)
            .map(|m| zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * 12.0);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/siren",
            gate,
            10.0,
            0.0,
        );
    }
    *warned = warning;
}

/// A spatial loop on a nearby anomaly.
#[derive(Component)]
struct AnomalyLoop;

/// Anomalies hum, roar and buzz when you're near enough to hear them (loops
/// are only kept on the close ones).
fn anomaly_loops(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    anomalies: Query<(
        Entity,
        &crate::anomaly::Anomaly,
        &GlobalTransform,
        Option<&Children>,
    )>,
    loops: Query<Entity, With<AnomalyLoop>>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    for (e, a, gt, children) in &anomalies {
        let d = gt.translation().distance(ear);
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match existing {
            None if d < 40.0 => {
                let (set, db) = match a.kind {
                    extraction_core::anomaly::AnomalyKind::Lift => ("anomaly/lift_hum", -2.0),
                    extraction_core::anomaly::AnomalyKind::Burner => ("anomaly/burner_roar", 0.0),
                    extraction_core::anomaly::AnomalyKind::Snare => ("anomaly/snare_buzz", -6.0),
                };
                if let Some(sample) = bank.pick(set, &mut rng.0) {
                    commands.entity(e).with_child((
                        AnomalyLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(db)),
                        PlaybackSettings::default().with_speed(0.9 + rng.0.f32() as f64 * 0.2),
                        Transform::from_xyz(0.0, 1.0, 0.0),
                        sample_effects![SpatialBasicNode::default()],
                    ));
                }
            }
            Some(l) if d > 50.0 => {
                commands.entity(l).despawn();
            }
            _ => {}
        }
    }
}

fn anomaly_sounds(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut events: MessageReader<crate::anomaly::AnomalyEvent>,
    mut glances: MessageReader<crate::health::Glanced>,
    mut emps: MessageReader<crate::gadgets::EmpBlast>,
    mut offerings: MessageReader<crate::anomaly::offering::OfferingTaken>,
    mut harpoons: MessageReader<crate::robots::grappler::Harpoon>,
    positions: Query<&GlobalTransform>,
) {
    for h in harpoons.read() {
        let at = positions.get(h.from).ok().map(|g| g.translation());
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/clank",
            at,
            4.0,
            0.1,
        );
        if h.hit {
            let at = positions.get(h.to).ok().map(|g| g.translation());
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "impacts/metal",
                at,
                0.0,
                0.1,
            );
        }
    }
    for o in offerings.read() {
        let speed = if o.boon == crate::anomaly::offering::Boon::Nothing {
            1.3
        } else {
            1.0
        };
        if let Some(sample) = bank.pick("anomaly/offering", &mut rng.0) {
            commands.spawn((
                SamplePlayer::new(sample).with_volume(Volume::Decibels(4.0)),
                PlaybackSettings::default().with_speed(speed),
                Transform::from_translation(o.at),
                sample_effects![SpatialBasicNode::default()],
            ));
        }
    }
    use crate::anomaly::AnomalyAct;
    for ev in events.read() {
        let (set, db, speed) = match ev.what {
            AnomalyAct::Discharge => ("anomaly/surge_crack", 2.0, 1.7),
            AnomalyAct::Pop => ("anomaly/lift_pop", 4.0, 1.0),
            AnomalyAct::Bloom => ("anomaly/surge_crack", -4.0, 0.6),
            AnomalyAct::Flare => continue,
        };
        let Some(sample) = bank.pick(set, &mut rng.0) else {
            continue;
        };
        commands.spawn((
            SamplePlayer::new(sample).with_volume(Volume::Decibels(db)),
            PlaybackSettings::default().with_speed(speed),
            Transform::from_translation(ev.at),
            sample_effects![SpatialBasicNode::default()],
        ));
    }
    for b in emps.read() {
        for (set, db, speed) in [
            ("anomaly/surge_crack", 6.0, 0.7),
            ("anomaly/lift_pop", 2.0, 1.4),
        ] {
            if let Some(sample) = bank.pick(set, &mut rng.0) {
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(db)),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(b.at),
                    sample_effects![SpatialBasicNode::default()],
                ));
            }
        }
    }
    for g in glances.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/ricochet",
            Some(g.at),
            2.0,
            0.1,
        );
    }
}

/// A gadget's running sound: the camera drone's rotors, a zipline trolley.
#[derive(Component)]
struct GadgetLoop;

/// The camera drone whines while its rotors turn (a robot drone's rotor,
/// pitched up and quieter); a zipline hums under your hand, rising with
/// speed.
fn gadget_loops(
    mut commands: Commands,
    drones: Query<(
        Entity,
        &crate::gadgets::camera_drone::CameraDrone,
        Option<&Children>,
    )>,
    grips: Query<(
        Entity,
        &crate::gadgets::climbing::RopeGrip,
        Option<&Children>,
    )>,
    ropes: Query<&crate::gadgets::climbing::Rope>,
    loops: Query<Entity, With<GadgetLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<GadgetLoop>>,
    all_children: Query<&Children>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::camera_drone::Flight;
    let mut want: Vec<(Entity, &str, f32, f64)> = Vec::new();
    for (e, d, _) in &drones {
        if d.flight != Flight::Idle {
            want.push((e, "machines/drone_rotor", -16.0, 1.9));
        }
    }
    for (e, g, _) in &grips {
        if ropes.get(g.rope).is_ok_and(|r| r.zip) && g.speed > 0.3 {
            let pitch = 0.6 + (g.speed / crate::gadgets::climbing::ZIP_SPEED) as f64 * 0.9;
            want.push((e, "machines/hum", -10.0, pitch));
        }
    }
    // Stop the ones that aren't wanted any more.
    for l in &loops {
        let parent_wanted = want
            .iter()
            .any(|(e, ..)| all_children.get(*e).is_ok_and(|c| c.iter().any(|c| c == l)));
        if !parent_wanted {
            commands.entity(l).despawn();
        }
    }
    for (e, set, db, speed) in want {
        let existing = all_children
            .get(e)
            .ok()
            .and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match existing {
            Some(l) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    if (s.speed - speed).abs() > 0.02 {
                        s.speed = speed;
                    }
                }
            }
            None => {
                if let Some(sample) = bank.pick(set, &mut rng.0) {
                    commands.spawn((
                        GadgetLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(db)),
                        PlaybackSettings { speed, ..default() },
                        Transform::default(),
                        sample_effects![SpatialBasicNode::default()],
                        ChildOf(e),
                    ));
                }
            }
        }
    }
}

/// Doors creak open and bang shut, whoever moves them; a walker battering
/// a wedged one is heard a long way off.
fn door_sounds(
    mut commands: Commands,
    doors: Query<(Entity, &crate::world::buildings::Door, &GlobalTransform)>,
    mut was_open: Local<std::collections::HashMap<Entity, bool>>,
    mut blows: MessageReader<crate::robots::doors::DoorBlow>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, door, gt) in &doors {
        let before = was_open.insert(e, door.open);
        if before.is_some_and(|b| b != door.open) {
            let (set, db) = if door.open {
                ("physical/door_open", -8.0)
            } else {
                ("physical/door_close", -6.0)
            };
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(gt.translation()),
                db,
                0.08,
            );
        }
    }
    for b in blows.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "impacts/wood",
            Some(b.at),
            2.0,
            0.15,
        );
        if b.broke {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/door_close",
                Some(b.at),
                4.0,
                0.2,
            );
        }
    }
}

/// The wind in the trees, and the hush of a room: which you hear depends
/// on whether you're under a roof.
#[derive(Component)]
struct WindLoop;
#[derive(Component)]
struct RoomLoop;

const WIND_DB: f32 = -16.0;

fn indoor_ambience(
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    wind: Query<&SampleEffects, With<WindLoop>>,
    room: Query<&SampleEffects, With<RoomLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
    mut inside: Local<f32>,
) {
    let indoors = entrances.is_some_and(|e| {
        cams.iter()
            .next()
            .is_some_and(|c| e.building_at(c.translation()).is_some())
    });
    // Ease over about a second as you step through a door.
    let target = if indoors { 1.0 } else { 0.0 };
    *inside += (target - *inside).clamp(-time.delta_secs(), time.delta_secs());
    let k = *inside;
    for effects in &wind {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = Volume::Decibels(WIND_DB - 14.0 * k);
        }
    }
    for effects in &room {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if k < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(-30.0 + 12.0 * k)
            };
        }
    }
}

/// Attention is felt, not shown: past the first prickle a ringing rises in
/// your ears, loudest when the whole Zone is looking at you.
fn attention_hum(
    players: Query<&crate::anomaly::Attention, With<crate::rig::LocalPlayer>>,
    hum: Query<&SampleEffects, With<AttentionLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    time: Res<Time>,
    mut level: Local<f32>,
) {
    let target = players
        .iter()
        .next()
        .map_or(0.0, |a| ((a.total() - 0.3) / 2.7).clamp(0.0, 1.0));
    // Swells over a few seconds, fades slower.
    let rate = if target > *level { 0.25 } else { 0.1 };
    *level += (target - *level).clamp(-rate * time.delta_secs(), rate * time.delta_secs());
    let k = *level;
    for effects in &hum {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if k < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(-34.0 + 20.0 * k)
            };
        }
    }
}

/// A walker's foot coming down: a heavy metal footfall, and now and then
/// the whine of its servos.
fn walker_steps(
    mut commands: Commands,
    mut steps: MessageReader<crate::robots::walker::WalkerStep>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in steps.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "footsteps/metal",
            Some(s.at),
            2.0,
            0.12,
        );
        if rng.0.f32() < 0.35 {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "machines/servo",
                Some(s.at),
                -6.0,
                0.15,
            );
        }
    }
}

/// Keying the radio: a squelch, a breath of static.
fn radio_sounds(
    mut commands: Commands,
    mut calls: MessageReader<crate::raid::RadioCall>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for c in calls.read() {
        let at = positions
            .get(c.player)
            .ok()
            .map(|g| g.translation() + Vec3::Y * 1.3);
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_squelch",
            at,
            -6.0,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            -16.0,
            0.05,
        );
    }
}

/// Robot chatter on a carried radio: a squelch, a long hiss of static
/// with servo-chirps through it — louder when it's about somewhere close.
fn chatter_sounds(
    mut commands: Commands,
    mut heard: MessageReader<crate::raid::chatter::RobotChatter>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for h in heard.read() {
        let at = positions.get(h.radio).ok().map(|g| g.translation());
        let gain = if h.close { 0.0 } else { -5.0 };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_squelch",
            at,
            -8.0 + gain,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            -12.0 + gain,
            0.1,
        );
        for _ in 0..2 {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "machines/servo",
                at,
                -20.0 + gain,
                0.4,
            );
        }
    }
}

/// A drone settling its gun on you: the chirp that means move.
fn lock_sounds(
    mut commands: Commands,
    mut locks: MessageReader<crate::robots::drone::DroneLock>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for l in locks.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/drone_lock",
            Some(l.at),
            4.0,
            0.03,
        );
    }
}

/// Things you drop land with a sound that depends on what they're made of,
/// louder the harder they hit (`items::impacts`).
fn drop_sounds(
    mut commands: Commands,
    mut impacts: MessageReader<crate::items::impacts::ItemImpact>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::items::impacts::Material;
    for i in impacts.read() {
        let set = match i.material {
            Material::Metal => "physical/drop_metal",
            Material::Wood => "physical/drop_wood",
            Material::Plastic => "physical/drop_plastic",
            Material::Soft => "physical/drop_cardboard",
        };
        let db = (-18.0 + 4.0 * i.speed + 3.0 * i.mass.max(0.05).log2()).min(0.0);
        play(&mut commands, &bank, &mut rng.0, set, Some(i.at), db, 0.1);
    }
}

/// A pylon going up or coming down: servos and clanks from its mast.
fn pylon_sounds(
    mut commands: Commands,
    pylons: Query<(&crate::robots::pylon::Pylon, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    use crate::robots::pylon::PylonState;
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.9;
    for (p, gt) in &pylons {
        if !matches!(p.state, PylonState::Building | PylonState::Packing) {
            continue;
        }
        let at = gt.translation() + Vec3::Y * (1.2 + crate::robots::pylon::MAST * p.progress);
        let set = if rng.0.f32() < 0.6 {
            "machines/servo"
        } else {
            "machines/clank"
        };
        play(&mut commands, &bank, &mut rng.0, set, Some(at), -4.0, 0.2);
    }
}
