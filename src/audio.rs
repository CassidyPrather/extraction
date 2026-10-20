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

use crate::audio_mix as mix;
use crate::interact::InteractEvent;
use crate::player::Footstep;
use crate::rig::RigHead;
use crate::robots::drone::Drone;
use crate::robots::{Robot, RobotPart};
use crate::weapons::GunEvent;
use crate::weapons::ballistics::{Hit, Shot};
use crate::world::Zone;
use crate::world::surfaces::{Surfaces, footstep_set, impact_set, underfoot};
use avian3d::prelude::*;

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

/// A roof's own sound in the rain (`acoustics::rain_under`): tin
/// drumming, a tent's patter.
#[derive(Component)]
struct RoofRain(&'static str);

/// The rain bed follows the weather, and where you are: duller and quieter
/// under a roof (a tin roof drumming over you if there's a recording for
/// it), gone underground. Eased, so a doorway is a crossfade.
fn rain_volume(
    mut commands: Commands,
    conditions: Res<crate::world::weather::Conditions>,
    listening: Res<Listening>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    loops: Query<&SampleEffects, With<RainLoop>>,
    roofs: Query<(&RoofRain, &SampleEffects)>,
    mut volumes: Query<&mut VolumeNode>,
    mut lowpass: Query<&mut FastLowpassNode>,
    mut now: Local<Option<(f32, f32)>>,
    mut drums: Local<HashMap<&'static str, f32>>,
) {
    let rain_db = if conditions.rain < 0.01 {
        None
    } else {
        Some(mix::db("rain") - 18.0 + 18.0 * conditions.rain.sqrt())
    };
    let (want_db, want_cut, drum) = crate::acoustics::rain_under(&listening.space);
    let k = (time.delta_secs() * 2.0).min(1.0);
    let (db, cut) = now.get_or_insert((want_db, want_cut));
    *db += (want_db - *db) * k;
    // Ease the cutoff in octaves.
    *cut = (cut.log2() + (want_cut.log2() - cut.log2()) * k).exp2();
    for effects in &loops {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = rain_db.map_or(Volume::SILENT, |r| Volume::Decibels(r + *db));
        }
        if let Ok(mut lp) = lowpass.get_effect_mut(effects) {
            if (lp.cutoff_hz - *cut).abs() > 1.0 {
                lp.cutoff_hz = *cut;
            }
        }
    }
    // The roof's drumming: started the first time you're under that roof
    // in the rain, faded in and out after.
    if let (Some(set), Some(_)) = (drum, rain_db) {
        if !drums.contains_key(set) && bank.has(set) {
            drums.insert(set, 0.0);
            if let Some(sample) = bank.pick(set, &mut rng.0) {
                commands.spawn((
                    RoofRain(set),
                    SamplePlayer::new(sample).looping(),
                    PlaybackSettings::default(),
                    sample_effects![VolumeNode {
                        volume: Volume::SILENT,
                        ..default()
                    }],
                ));
            }
        }
    }
    for (set, level) in drums.iter_mut() {
        let target = if drum == Some(*set) && rain_db.is_some() {
            1.0
        } else {
            0.0
        };
        *level += (target - *level) * k;
    }
    for (roof, effects) in &roofs {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            let level = drums.get(roof.0).copied().unwrap_or(0.0);
            v.volume = match rain_db {
                Some(r) if level > 0.01 => Volume::Decibels(r + 20.0 * level.log10()),
                _ => Volume::SILENT,
            };
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
            .init_resource::<Listening>()
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
                    splash_sounds,
                    lock_sounds,
                    drop_sounds,
                ),
            )
            .add_systems(
                Update,
                (
                    button_sounds,
                    press_sounds,
                    far_sounds,
                    step_sounds,
                    truck_sounds,
                    use_sounds,
                    part_sounds,
                    pack_sounds,
                    landing_sounds,
                    gadget_sounds,
                    notice_sounds,
                    convoy_sounds,
                    dirigible_loops,
                    drone_and_zip_sounds,
                    kitchen_sounds,
                    mind_sounds,
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
                    room_acoustics,
                ),
            )
            // After everything that plays, so a sound is placed the frame
            // it starts.
            .add_systems(
                PostUpdate,
                occlude.after(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn load_bank(mut commands: Commands, server: Res<AssetServer>) {
    let path = crate::asset_root_path().join("audio/sounds.json");
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

/// The reverb bus's input: every placed sound sends here
/// (`acoustics`), and the room's reverb comes back from it.
#[derive(NodeLabel, PartialEq, Eq, Debug, Hash, Clone)]
pub struct ReverbIn;

/// A placed sound's effects: its distance falloff and muffle (and
/// occlusion, set later by `occlude`), then a send to the reverb.
fn spatial_fx(cue: &str) -> impl Bundle {
    sample_effects![mix::spatial(cue), reverb_send(cue)]
}

/// The reverb bus: a level for the room, then the reverb, to the main
/// bus. `room_acoustics` sets both from where you stand.
fn spawn_reverb_bus(commands: &mut Commands) {
    commands
        .spawn((
            ReverbIn,
            VolumeNode {
                volume: Volume::SILENT,
                ..default()
            },
        ))
        .chain_node(FreeverbNode {
            room_size: 0.5,
            damping: 0.5,
            width: 0.8,
            ..default()
        });
}

/// How much of a cue goes to the reverb: all of it, but for recordings
/// with their own space baked in (the far-off shot's echo, the siren's
/// valley) and the beds.
fn reverb_send(cue: &str) -> SendNode {
    let share = match mix::role(cue) {
        Some(mix::Role::Bed) => 0.0,
        _ if matches!(cue, "gun.distant" | "surge.siren" | "surge.crack") => 0.3,
        _ => 1.0,
    };
    SendNode::new(Volume::Linear(share), ReverbIn)
}

/// Plays one variant of a set at a point (or unspatialised with `None`),
/// at the cue's gain (`audio_mix`) plus `offset_db`, falling off from the
/// cue's reach.
#[allow(clippy::too_many_arguments)]
fn play(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Option<Vec3>,
    cue: &str,
    offset_db: f32,
    pitch: f64,
) {
    play_at_speed(commands, bank, rng, set, at, cue, offset_db, 1.0, pitch);
}

/// [`play`], centred on `speed` (below 1 is lower and slower: heavier).
#[allow(clippy::too_many_arguments)]
fn play_at_speed(
    commands: &mut Commands,
    bank: &SoundBank,
    rng: &mut Rng,
    set: &str,
    at: Option<Vec3>,
    cue: &str,
    offset_db: f32,
    speed: f64,
    pitch: f64,
) {
    let Some(sample) = bank.pick(set, rng) else {
        return;
    };
    let speed = speed * (1.0 + (rng.f32() as f64 - 0.5) * 2.0 * pitch);
    let player = SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue) + offset_db));
    let settings = PlaybackSettings::default().with_speed(speed);
    match at {
        Some(p) => {
            commands.spawn((
                player,
                settings,
                Transform::from_translation(p),
                spatial_fx(cue),
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
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
) {
    for s in steps.read() {
        let set = ground_set(&zone, &trucks, &spatial, &built, s.position);
        let offset = -6.0 + (s.speed / 4.0).min(1.0) * 6.0;
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(s.position),
            "step",
            offset,
            0.06,
        );
    }
}

/// The footstep set for whatever is under `at`: the truck's bed, a built
/// floor, or the ground.
fn ground_set(
    zone: &Zone,
    trucks: &Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
    spatial: &SpatialQuery,
    built: &Query<(&GlobalTransform, &Surfaces)>,
    at: Vec3,
) -> &'static str {
    let p = [at.x, at.z];
    let in_truck = trucks
        .iter()
        .any(|(gt, t)| t.in_bed(gt, at + Vec3::Y * 0.2));
    // A floor, a deck, a stair: whatever's underfoot, if it's built.
    if in_truck {
        "footsteps/wood"
    } else if let Some(m) = underfoot(spatial, built, at) {
        footstep_set(m)
    } else {
        surface_set(zone.hm.surface(p, zone.forest_cover(p[0], p[1]) > 0.5))
    }
}

/// Coming down from a jump or a drop: both feet at once, harder the
/// faster you fell.
#[allow(clippy::too_many_arguments)]
fn landing_sounds(
    mut commands: Commands,
    bodies: Query<(Entity, &crate::player::Body, &GlobalTransform)>,
    zone: Res<Zone>,
    trucks: Query<(&GlobalTransform, &crate::raid::truck::Truck)>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
    mut was: Local<HashMap<Entity, (bool, f32, f32)>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, body, gt) in &bodies {
        let before = was.insert(e, (body.grounded, body.velocity.y, body.airborne));
        let Some((was_grounded, vy, airborne)) = before else {
            continue;
        };
        if was_grounded || !body.grounded || body.swimming || airborne < 0.3 || vy > -2.5 {
            continue;
        }
        let at = gt.translation();
        let set = ground_set(&zone, &trucks, &spatial, &built, at);
        let hard = ((-vy - 2.5) / 6.0).clamp(0.0, 1.0);
        for _ in 0..2 {
            play_at_speed(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                "land",
                -6.0 + 6.0 * hard,
                0.92,
                0.05,
            );
        }
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
        let (set, cue, offset) = match (&ev.event, action) {
            (FireEvent::Fired { .. }, _) => {
                let far = ev.at.distance(ear) > 80.0;
                if far {
                    ("guns/distant_shot", "gun.distant", 0.0)
                } else {
                    (shot_set(&ev.item), "gun.shot", 0.0)
                }
            }
            (FireEvent::DryFire | FireEvent::Dud, _) => ("guns/dry_fire", "gun.dry_fire", 0.0),
            // The action stopping short on a stuck case: a bolt closing,
            // slowed and lowered into a dull, wrong clack.
            (FireEvent::Malfunctioned(_), _) => ("guns/bolt_close", "gun.malfunction", 0.0),
            (FireEvent::MagazineInserted, _) => ("guns/mag_insert", "gun.mag_insert", 0.0),
            (FireEvent::MagazineRemoved, _) => ("guns/mag_release", "gun.mag_release", 0.0),
            (FireEvent::ModeChanged(_), _) => ("guns/safety", "gun.safety", 0.0),
            (FireEvent::RoundLoaded, Some(BreakAction)) => {
                ("guns/shell_insert", "gun.shell_insert", 0.0)
            }
            (FireEvent::RoundLoaded, _) => ("guns/stripper_clip", "gun.stripper_clip", 0.0),
            (FireEvent::BoltClosed, Some(BoltAction)) => ("guns/bolt_close", "gun.bolt_close", 0.0),
            (FireEvent::BoltClosed, Some(BreakAction)) => {
                ("guns/shotgun_close", "gun.shotgun_close", 0.0)
            }
            (FireEvent::BoltClosed, Some(SemiAuto)) if ev.item == "ak74" => {
                ("guns/charging_handle", "gun.charging", 0.0)
            }
            (FireEvent::BoltClosed, Some(SemiAuto)) => ("guns/slide_rack", "gun.slide_rack", 0.0),
            (FireEvent::Cocked, Some(BoltAction)) => ("guns/bolt_open", "gun.bolt_open", 0.0),
            (FireEvent::Cocked, Some(BreakAction)) => {
                ("guns/shotgun_open", "gun.shotgun_open", 0.0)
            }
            // A spent case is lighter than a live round.
            (FireEvent::Ejected(Round::Spent), _) => ("guns/casing", "gun.casing", -4.0),
            (FireEvent::Ejected(Round::Live), _) => ("guns/casing", "gun.casing", 0.0),
            _ => continue,
        };
        // Shots aren't pitch-shifted much: a gun has one voice.
        let pitch = if matches!(ev.event, FireEvent::Fired { .. }) {
            0.03
        } else {
            0.07
        };
        let speed = if matches!(ev.event, FireEvent::Malfunctioned(_)) {
            0.72
        } else {
            1.0
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(ev.at),
            cue,
            offset,
            speed,
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
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db("robot.shot"))),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(s.origin),
                    spatial_fx("robot.shot"),
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
    bodies: Query<(), With<crate::player::BodyCollider>>,
    built: Query<(&GlobalTransform, &Surfaces)>,
) {
    for h in hits.read() {
        // A round into you is a thump, not the ground's spray of dirt.
        if bodies.contains(h.entity) {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/body_hit",
                Some(h.point),
                "hit.body",
                0.0,
                0.08,
            );
            continue;
        }
        let made_of = built
            .get(h.entity)
            .ok()
            .and_then(|(gt, surf)| surf.at(gt, h.point));
        let set = if parts.contains(h.entity) || items.contains(h.entity) {
            "impacts/metal"
        } else if trees.contains(h.entity) {
            "impacts/wood"
        } else if let Some(m) = made_of {
            impact_set(m)
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
            "hit.impact",
            0.0,
            0.08,
        );
        if rng.0.chance(0.12) && set != "impacts/dirt" {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "impacts/ricochet",
                Some(h.point),
                "hit.ricochet",
                0.0,
                0.1,
            );
        }
    }
}

/// What taking something up sounds like: the thing in the glove. A rifle's
/// receiver knocks, a tin clinks, a crate's slat thuds; cloth and food
/// rustle.
pub fn handling_set(material: crate::items::impacts::Material) -> &'static str {
    use crate::items::impacts::Material;
    match material {
        Material::Metal => "physical/handle_metal",
        Material::Wood => "physical/handle_wood",
        Material::Plastic => "physical/handle_plastic",
        Material::Soft => "physical/cloth",
    }
}

/// Weight in the hand, as gain and pitch: +3 dB and about 4% lower per
/// doubling over a kilo (a 50 g tin is a tick, a 4 kg rifle a clack).
pub fn weight_tone(mass: f32) -> (f32, f64) {
    let doublings = mass.max(0.03).log2();
    (
        (3.0 * doublings).clamp(-9.0, 5.0),
        (1.0 - 0.04 * doublings as f64).clamp(0.85, 1.2),
    )
}

#[allow(clippy::too_many_arguments)]
fn hand_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    poses: Query<&GlobalTransform>,
    sockets: Query<&crate::interact::Socket>,
    items: Query<&crate::items::Item>,
    bags: Query<&crate::interact::body::InBackpack>,
    packs: Query<&crate::interact::body::Backpack>,
    catalog: Res<crate::items::Catalog>,
) {
    use crate::interact::SocketKind;
    // An item's handling sound, gain and pitch; cloth for what isn't one
    // (the pack, a rope).
    let feel = |item: Entity| {
        items
            .get(item)
            .ok()
            .map_or(("physical/cloth", 0.0, 1.0), |i| {
                let def = catalog.get(i.kind());
                let (db, speed) = weight_tone(def.mass);
                (
                    handling_set(crate::items::impacts::material_of(def)),
                    db,
                    speed,
                )
            })
    };
    for ev in events.read() {
        let (item, set, cue, db, speed) = match ev {
            InteractEvent::Grabbed { item, .. } => {
                let (set, db, speed) = feel(*item);
                if items.contains(*item) {
                    (*item, set, "hand.take", db, speed)
                } else {
                    (*item, set, "hand.grab", 0.0, 1.0)
                }
            }
            // Letting go: the same thing, lighter, as the fingers open.
            InteractEvent::Released { item, .. } if items.contains(*item) => {
                let (set, db, speed) = feel(*item);
                (*item, set, "hand.release", db, speed * 1.05)
            }
            // Into the pack: its zip. Into a crate, a tray or the stash:
            // the thing set down in it.
            InteractEvent::Stowed { item } => {
                let into_crate = bags
                    .get(*item)
                    .ok()
                    .and_then(|b| packs.get(b.bag).ok())
                    .is_some_and(|p| !p.carried);
                if into_crate {
                    let (set, db, speed) = feel(*item);
                    (*item, set, "hand.place", db, speed)
                } else {
                    (*item, "physical/zip", "hand.stow", 0.0, 1.0)
                }
            }
            // A pouch or holster takes it with a rustle (the long velcro rips
            // sounded like a motor in the first headset session); a gun's
            // magazine well has its own click (`gun.mag_insert`), and a rail
            // or cradle a small click.
            InteractEvent::Socketed { item, socket } => match sockets.get(*socket).map(|s| &s.kind)
            {
                Ok(SocketKind::Body(_)) => (*item, "physical/cloth", "hand.socket", 0.0, 1.0),
                Ok(SocketKind::MagWell(_)) => continue,
                _ => (*item, "guns/safety", "hand.attach", 0.0, 1.0),
            },
            InteractEvent::Refused { item } => (*item, "physical/cloth", "hand.refused", 0.0, 1.0),
            _ => continue,
        };
        let at = poses.get(item).map(|g| g.translation()).ok();
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            cue,
            db,
            speed,
            0.08,
        );
    }
}

/// Push buttons click going in and again, lighter, coming back out.
fn button_sounds(
    mut commands: Commands,
    buttons: Query<(Entity, &crate::interact::poke::PokeButton, &GlobalTransform)>,
    mut was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, b, gt) in &buttons {
        let before = was.insert(e, b.down).unwrap_or(false);
        if before == b.down {
            continue;
        }
        let (cue, speed) = if b.down {
            ("button.in", 1.0)
        } else {
            ("button.out", 1.12)
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/button",
            Some(gt.translation()),
            cue,
            0.0,
            speed,
            0.04,
        );
    }
}

/// Things pressed with the grip (or the flat key): the bell cord rings
/// the truck's bell; a card, knob or lever clicks, unless its push button
/// already did. Doors sound in `door_sounds`.
fn press_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    pressables: Query<(&crate::raid::truck::Pressable, &GlobalTransform)>,
    buttons: Query<&crate::interact::poke::PokeButton>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::raid::truck::Pressable;
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        let Ok((kind, gt)) = pressables.get(*item) else {
            continue;
        };
        let (set, cue) = match kind {
            Pressable::BellPull => ("physical/bell", "truck.bell"),
            Pressable::Door => continue,
            Pressable::FoldCover => ("physical/cloth", "hand.grab"),
            Pressable::BenchRun => ("physical/switch", "press.lever"),
            _ if buttons.iter().any(|b| b.target == *item && b.down) => continue,
            _ => ("physical/button", "button.in"),
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(gt.translation()),
            cue,
            0.0,
            0.05,
        );
    }
}

/// The far grab: a whoosh that flies with the thing, and a slap into the
/// palm when it's caught.
fn far_sounds(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    poses: Query<&GlobalTransform>,
    time: Res<Time>,
    mut flying: Local<HashMap<Entity, f32>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let now = time.elapsed_secs();
    flying.retain(|_, t| now - *t < 2.0);
    for ev in events.read() {
        match ev {
            InteractEvent::Pulled { item, .. } => {
                flying.insert(*item, now);
                if let Some(sample) = bank.pick("physical/whoosh", &mut rng.0) {
                    let speed = 1.0 + (rng.0.f32() as f64 - 0.5) * 0.12;
                    commands.spawn((
                        SamplePlayer::new(sample)
                            .with_volume(Volume::Decibels(mix::db("far.whoosh"))),
                        PlaybackSettings::default().with_speed(speed),
                        Transform::default(),
                        spatial_fx("far.whoosh"),
                        ChildOf(*item),
                    ));
                }
            }
            InteractEvent::Grabbed { item, .. } if flying.remove(item).is_some() => {
                let at = poses.get(*item).map(|g| g.translation()).ok();
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/catch",
                    at,
                    "far.catch",
                    0.0,
                    0.06,
                );
            }
            _ => {}
        }
    }
}

/// The onboarding's "done, next": a light ding, in your head.
fn step_sounds(
    mut commands: Commands,
    mut steps: MessageReader<crate::rig::haptics::StepDone>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for s in steps.read() {
        let (set, cue) = if s.station {
            ("ui/chime_station", "step.station")
        } else {
            ("physical/chime", "step.done")
        };
        play(&mut commands, &bank, &mut rng.0, set, s.at, cue, 0.0, 0.02);
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
                            .with_volume(Volume::Decibels(mix::db("drone.rotor"))),
                        PlaybackSettings::default(),
                        Transform::default(),
                        spatial_fx("drone.rotor"),
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

fn start_ambience(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    zone: Option<Res<Zone>>,
) {
    // Each valley has its own wind; fall back to the forest's.
    let wind = zone
        .as_ref()
        .map(|z| z.map.sounds.wind.as_str())
        .filter(|w| bank.has(w))
        .unwrap_or("ambience/wind_conifers");
    spawn_reverb_bus(&mut commands);
    if let Some(sample) = bank.pick("ambience/rain_light", &mut rng.0) {
        commands.spawn((
            RainLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![
                VolumeNode {
                    volume: Volume::SILENT,
                    ..default()
                },
                FastLowpassNode::<2> {
                    cutoff_hz: 20_480.0,
                    ..default()
                }
            ],
        ));
    }
    if let Some(sample) = bank.pick(wind, &mut rng.0) {
        commands.spawn((
            WindLoop,
            SamplePlayer::new(sample).looping(),
            PlaybackSettings::default(),
            sample_effects![VolumeNode {
                volume: Volume::Decibels(mix::db("wind")),
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
                .with_volume(Volume::Decibels(mix::db("creek"))),
            PlaybackSettings::default(),
            Transform::default(),
            spatial_fx("creek"),
        ));
    }
}

/// Birds, distant thunder, the valley's own far-off sounds: now and then,
/// somewhere out in the fog.
fn ambient_oneshots(
    mut commands: Commands,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    time: Res<Time>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    zone: Res<Zone>,
    mut next: Local<(f32, f32, f32, f32)>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let dt = time.delta_secs();
    next.0 -= dt;
    next.1 -= dt;
    next.2 -= dt;
    next.3 -= dt;
    // Old works settling: now and then a clank of metal from the nearest
    // big iron building, if one's within earshot.
    if next.2 <= 0.0 {
        next.2 = rng.0.range(20.0, 60.0);
        const IRON: [&str; 6] = [
            "factory_hall",
            "chimney",
            "rail_shed",
            "bus_depot",
            "pump_station",
            "boxcar",
        ];
        let here = [ear.x, ear.z];
        let near = zone
            .map
            .placements
            .iter()
            .filter(|p| IRON.contains(&p.prefab.as_str()))
            .map(|p| (extraction_core::geom::dist(p.at, here), p.at))
            .filter(|(d, _)| *d < 150.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, at)) = near {
            let at = Vec3::new(at[0], zone.ground(at[0], at[1]) + 3.0, at[1]);
            // Mostly a clank; sometimes a long groan of a beam taking the
            // weight.
            let (set, cue) = if rng.0.f32() < 0.35 {
                ("machines/groan", "works.groan")
            } else {
                ("machines/clank", "works.clank")
            };
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                cue,
                0.0,
                0.25,
            );
        }
    }
    if next.0 <= 0.0 {
        next.0 = rng.0.range(12.0, 45.0);
        let a = rng.0.range(0.0, std::f32::consts::TAU);
        let d = rng.0.range(25.0, 70.0);
        let at = ear + Vec3::new(a.cos() * d, rng.0.range(4.0, 15.0), a.sin() * d);
        let birds = &zone.map.sounds.birds;
        if !birds.is_empty() {
            let set = &birds[rng.0.below(birds.len())];
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                "birds",
                0.0,
                0.08,
            );
        }
    }
    // The valley's own far-off sounds come from high ground: scree letting
    // go on the slopes above you.
    let distant = &zone.map.sounds.distant;
    if next.3 <= 0.0 && !distant.is_empty() {
        next.3 = rng.0.range(45.0, 140.0);
        let at = (0..6)
            .map(|_| {
                let a = rng.0.range(0.0, std::f32::consts::TAU);
                let d = rng.0.range(70.0, 160.0);
                let (x, z) = (ear.x + a.cos() * d, ear.z + a.sin() * d);
                Vec3::new(x, zone.ground(x, z) + 2.0, z)
            })
            .max_by(|a, b| a.y.total_cmp(&b.y));
        let set = &distant[rng.0.below(distant.len())];
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            "valley.distant",
            0.0,
            0.1,
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
            "thunder",
            0.0,
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
                mix::db("surge.drone") - 30.0 + 30.0 * level.sqrt()
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
            "surge.crack",
            -6.0 + 6.0 * f.strength,
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
            "surge.siren",
            0.0,
            0.0,
        );
    }
    *warned = warning;
}

/// A spatial loop on a nearby anomaly.
#[derive(Component)]
struct AnomalyLoop;

/// Where an anomaly's loop is kept, and where it fades: full inside
/// `FADE_NEAR`, down to -40 dB at `KEEP` where it starts and stops, so
/// walking up to one never switches its hum on.
const ANOMALY_FADE_NEAR: f32 = 25.0;
const ANOMALY_KEEP: f32 = 40.0;

fn anomaly_fade(d: f32) -> f32 {
    let t = ((d - ANOMALY_FADE_NEAR) / (ANOMALY_KEEP - ANOMALY_FADE_NEAR)).clamp(0.0, 1.0);
    -40.0 * t * t
}

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
    loops: Query<(Entity, &SampleEffects), With<AnomalyLoop>>,
    mut volumes: Query<&mut VolumeNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    for (e, a, gt, children) in &anomalies {
        let d = gt.translation().distance(ear);
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match existing {
            None if d < ANOMALY_KEEP => {
                let (set, cue) = match a.kind {
                    extraction_core::anomaly::AnomalyKind::Lift => {
                        ("anomaly/lift_hum", "anomaly.lift")
                    }
                    extraction_core::anomaly::AnomalyKind::Burner => {
                        ("anomaly/burner_roar", "anomaly.burner")
                    }
                    extraction_core::anomaly::AnomalyKind::Snare => {
                        ("anomaly/snare_buzz", "anomaly.snare")
                    }
                    // Placeholders until they get their own (Anomalies).
                    extraction_core::anomaly::AnomalyKind::Well => {
                        ("anomaly/lift_hum", "anomaly.lift")
                    }
                    extraction_core::anomaly::AnomalyKind::Mirror => {
                        ("anomaly/snare_buzz", "anomaly.snare")
                    }
                };
                if let Some(sample) = bank.pick(set, &mut rng.0) {
                    commands.entity(e).with_child((
                        AnomalyLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db(cue))),
                        PlaybackSettings::default().with_speed(0.9 + rng.0.f32() as f64 * 0.2),
                        Transform::from_xyz(0.0, 1.0, 0.0),
                        sample_effects![
                            VolumeNode {
                                volume: Volume::Decibels(anomaly_fade(d)),
                                ..default()
                            },
                            mix::spatial(cue),
                            reverb_send(cue)
                        ],
                    ));
                }
            }
            Some(l) if d > ANOMALY_KEEP + 5.0 => {
                commands.entity(l).despawn();
            }
            Some(l) => {
                if let Ok((_, effects)) = loops.get(l)
                    && let Ok(mut v) = volumes.get_effect_mut(effects)
                {
                    v.volume = Volume::Decibels(anomaly_fade(d));
                }
            }
            None => {}
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
            "harpoon.fire",
            0.0,
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
                "harpoon.hit",
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
                SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db("offering"))),
                PlaybackSettings::default().with_speed(speed),
                Transform::from_translation(o.at),
                spatial_fx("offering"),
            ));
        }
    }
    use crate::anomaly::AnomalyAct;
    for ev in events.read() {
        let (set, cue, speed) = match ev.what {
            AnomalyAct::Discharge => ("anomaly/surge_crack", "anomaly.discharge", 1.7),
            AnomalyAct::Pop => ("anomaly/lift_pop", "anomaly.pop", 1.0),
            AnomalyAct::Bloom => ("anomaly/surge_crack", "anomaly.bloom", 0.6),
            AnomalyAct::Flare => continue,
            // Placeholders until they get their own (Anomalies).
            AnomalyAct::Blink => ("anomaly/lift_pop", "anomaly.pop", 1.8),
            AnomalyAct::Reveal | AnomalyAct::Crush => continue,
        };
        let Some(sample) = bank.pick(set, &mut rng.0) else {
            continue;
        };
        commands.spawn((
            SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
            PlaybackSettings::default().with_speed(speed),
            Transform::from_translation(ev.at),
            spatial_fx(cue),
        ));
    }
    for b in emps.read() {
        for (set, cue, speed) in [
            ("anomaly/surge_crack", "emp.crack", 0.7),
            ("anomaly/lift_pop", "emp.pop", 1.4),
        ] {
            if let Some(sample) = bank.pick(set, &mut rng.0) {
                commands.spawn((
                    SamplePlayer::new(sample).with_volume(Volume::Decibels(mix::db(cue))),
                    PlaybackSettings::default().with_speed(speed),
                    Transform::from_translation(b.at),
                    spatial_fx(cue),
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
            "hit.glance",
            0.0,
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
    let mut want: Vec<(Entity, &str, &str, f64)> = Vec::new();
    for (e, d, _) in &drones {
        if d.flight != Flight::Idle {
            want.push((e, "machines/drone_rotor", "camera_drone", 1.9));
        }
    }
    for (e, g, _) in &grips {
        if ropes.get(g.rope).is_ok_and(|r| r.zip) && g.speed > 0.3 {
            let pitch = 0.6 + (g.speed / crate::gadgets::climbing::ZIP_SPEED) as f64 * 0.9;
            want.push((e, "machines/hum", "zipline", pitch));
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
    for (e, set, cue, speed) in want {
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
                            .with_volume(Volume::Decibels(mix::db(cue))),
                        PlaybackSettings { speed, ..default() },
                        Transform::default(),
                        spatial_fx(cue),
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
            let (set, cue) = if door.open {
                ("physical/door_open", "door.open")
            } else {
                ("physical/door_close", "door.close")
            };
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(gt.translation()),
                cue,
                0.0,
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
            "door.blow",
            0.0,
            0.15,
        );
        if b.broke {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "physical/door_close",
                Some(b.at),
                "door.broken",
                0.0,
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
            v.volume = Volume::Decibels(mix::db("wind") - 14.0 * k);
        }
    }
    for effects in &room {
        if let Ok(mut v) = volumes.get_effect_mut(effects) {
            v.volume = if k < 0.01 {
                Volume::SILENT
            } else {
                Volume::Decibels(mix::db("room") - 12.0 + 12.0 * k)
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
                Volume::Decibels(mix::db("attention") - 20.0 + 20.0 * k)
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
            "walker.step",
            0.0,
            0.12,
        );
        if rng.0.f32() < 0.35 {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "machines/servo",
                Some(s.at),
                "walker.servo",
                0.0,
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
            "radio.squelch",
            0.0,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            "radio.static",
            0.0,
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
            "chatter.squelch",
            gain,
            0.05,
        );
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/radio_static",
            at,
            "chatter.static",
            gain,
            0.1,
        );
        for _ in 0..2 {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                "machines/servo",
                at,
                "chatter.chirp",
                gain,
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
            "drone.lock",
            0.0,
            0.03,
        );
    }
}

/// Going into deeper water: a splash, louder the harder you went in.
fn splash_sounds(
    mut commands: Commands,
    bodies: Query<(Entity, &crate::player::Body, &GlobalTransform)>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    mut was: Local<std::collections::HashMap<Entity, f32>>,
) {
    for (e, body, gt) in &bodies {
        let before = was.insert(e, body.wading).unwrap_or(body.wading);
        if before < 0.25 && body.wading >= 0.25 {
            let hard = (body.velocity.length() / 4.0).clamp(0.2, 1.0);
            for _ in 0..2 {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "footsteps/water",
                    Some(gt.translation()),
                    "splash",
                    -8.0 + 8.0 * hard,
                    0.15,
                );
            }
        }
    }
}

/// Things you drop land with a sound that depends on what they're made of
/// and what they land on, louder the harder they hit (`items::impacts`).
/// On a floor, a crate or a rock you hear the thing; in grass, leaves or
/// mud you hear the ground take it (a magazine in the grass clanged like a
/// pan in the first headset session), with a faint clink for metal dropped
/// hard.
fn drop_sounds(
    mut commands: Commands,
    mut impacts: MessageReader<crate::items::impacts::ItemImpact>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
) {
    use crate::items::impacts::{Material, soft};
    for i in impacts.read() {
        let offset = (-18.0 + 4.0 * i.speed + 3.0 * i.mass.max(0.05).log2()).min(0.0);
        let on_built = underfoot(&spatial, &built, i.at).is_some();
        if !on_built && soft(i.ground) {
            let ground = surface_set(i.ground.unwrap_or(Surface::Grass));
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                ground,
                Some(i.at),
                "drop",
                offset - 4.0,
                0.1,
            );
            if i.material == Material::Metal && i.speed > 4.0 {
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "physical/drop_metal",
                    Some(i.at),
                    "drop",
                    offset - 14.0,
                    0.1,
                );
            }
            continue;
        }
        let set = match i.material {
            Material::Metal => "physical/drop_metal",
            Material::Wood => "physical/drop_wood",
            Material::Plastic => "physical/drop_plastic",
            Material::Soft => "physical/drop_cardboard",
        };
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(i.at),
            "drop",
            offset,
            0.1,
        );
    }
}

/// The truck's diesel, heard where it is: coming up the road to the
/// rendezvous as it's due, idling while it waits for you, pulling away.
/// (The truck itself moves between stops off-stage.)
#[derive(Component)]
struct TruckIdle;

/// Seconds before the truck is due that its approach starts (about the
/// length of `machines/truck_arrive`).
const TRUCK_APPROACH: f32 = 6.0;

#[allow(clippy::too_many_arguments)]
fn truck_sounds(
    mut commands: Commands,
    raid: Res<crate::raid::Raid>,
    zone: Res<Zone>,
    trucks: Query<(Entity, &GlobalTransform), With<crate::raid::truck::Truck>>,
    idles: Query<Entity, With<TruckIdle>>,
    mut last: Local<Option<crate::raid::Phase>>,
    mut heard_coming: Local<bool>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::raid::Phase;
    let engine = |t: &Transform| t.transform_point(Vec3::new(0.0, 1.0, -2.0));
    let rv_engine = |rv: usize| engine(&crate::raid::truck::rv_pose(&zone, rv));
    let Some((truck, truck_at)) = trucks.iter().next() else {
        return;
    };
    let here = engine(&truck_at.compute_transform());
    let phase = raid.phase.clone();
    match &phase {
        Phase::TruckCalled { rv, eta } => {
            if *eta <= TRUCK_APPROACH && !*heard_coming && *rv != crate::raid::SHORT_EXFIL {
                *heard_coming = true;
                play(
                    &mut commands,
                    &bank,
                    &mut rng.0,
                    "machines/truck_arrive",
                    Some(rv_engine(*rv)),
                    "truck.arrive",
                    0.0,
                    0.02,
                );
            }
        }
        _ => *heard_coming = false,
    }
    let idling = matches!(phase, Phase::TruckWaiting { .. });
    match (idling, idles.iter().next()) {
        (true, None) => {
            if let Some(sample) = bank.pick("machines/truck_idle", &mut rng.0) {
                commands.spawn((
                    TruckIdle,
                    SamplePlayer::new(sample)
                        .looping()
                        .with_volume(Volume::Decibels(mix::db("truck.idle"))),
                    PlaybackSettings::default(),
                    Transform::from_xyz(0.0, 1.0, -2.0),
                    spatial_fx("truck.idle"),
                    ChildOf(truck),
                ));
            }
        }
        (false, Some(e)) => commands.entity(e).despawn(),
        _ => {}
    }
    // Leaving: with you in the back, or without you.
    let left_from = match (last.as_ref(), &phase) {
        (Some(Phase::TruckWaiting { .. }), Phase::Extracting { .. }) => Some(here),
        (Some(Phase::TruckWaiting { rv, .. }), Phase::Out) if *rv != crate::raid::SHORT_EXFIL => {
            Some(rv_engine(*rv))
        }
        _ => None,
    };
    if let Some(at) = left_from {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/truck_depart",
            Some(at),
            "truck.depart",
            0.0,
            0.02,
        );
    }
    *last = Some(phase);
}

/// What using a thing sounds like, by what it is: eating, drinking,
/// tearing a dressing open, a switch.
pub fn use_sound(def: &extraction_core::items::ItemDef) -> Option<(&'static str, &'static str)> {
    use extraction_core::items::Category;
    const DRINKS: &[&str] = &[
        "food:Water",
        "food:Milk",
        "food:Tea",
        "food:MilkTea",
        "food:HotWater",
        "food:Soup",
    ];
    match def.id.as_str() {
        "field_dressing" => Some(("physical/bandage", "use.bandage")),
        "radio" | "deployable_cover" | "climbing_kit" | "zipline_kit" | "camera_drone" => None,
        _ if def.category == Category::Food => Some(if DRINKS.iter().any(|t| def.has_tag(t)) {
            ("physical/drink", "use.drink")
        } else {
            ("physical/eat", "use.eat")
        }),
        // Lights, an injector's plunger, a cloak, arming a charge.
        _ if matches!(
            def.category,
            Category::Gadget | Category::Medical | Category::Firearm | Category::Attachment
        ) || def.id.ends_with("light") =>
        {
            Some(("physical/switch", "use.switch"))
        }
        _ => None,
    }
}

/// Using what's in your hand. The kinds of what hands held last frame are
/// kept, because a thing eaten is gone before its message is read.
fn use_sounds(
    mut commands: Commands,
    mut uses: MessageReader<crate::items::usage::UseItem>,
    held: Query<(Entity, &crate::items::Item, &GlobalTransform), With<crate::interact::Held>>,
    catalog: Res<crate::items::Catalog>,
    mut kinds: Local<HashMap<Entity, (extraction_core::items::ItemKind, Vec3)>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, item, gt) in &held {
        kinds.insert(e, (item.kind(), gt.translation()));
    }
    for u in uses.read() {
        let Some((kind, at)) = kinds.get(&u.item).copied() else {
            continue;
        };
        if let Some((set, cue)) = use_sound(catalog.get(kind)) {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(at),
                cue,
                0.0,
                0.08,
            );
        }
    }
    kinds.retain(|e, _| held.contains(*e));
}

/// A robot losing a part: the crash of it, glass for an eye, and a groan
/// of the frame giving up when it was the core.
fn part_sounds(
    mut commands: Commands,
    mut parts: MessageReader<crate::robots::PartDestroyed>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::PartRole;
    for p in parts.read() {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/clank",
            Some(p.at),
            "robot.part",
            0.0,
            0.15,
        );
        let extra = match p.role {
            PartRole::Sensor => Some(("physical/glass_break", "robot.glass")),
            PartRole::Core => Some(("machines/groan", "robot.wreck")),
            _ => None,
        };
        if let Some((set, cue)) = extra {
            play(
                &mut commands,
                &bank,
                &mut rng.0,
                set,
                Some(p.at),
                cue,
                0.0,
                0.1,
            );
        }
    }
}

/// Your pack opening and closing: its zip.
fn pack_sounds(
    mut commands: Commands,
    packs: Query<(Entity, &crate::interact::body::Backpack, &GlobalTransform)>,
    locals: Query<(), With<crate::rig::LocalPlayer>>,
    mut was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, p, gt) in &packs {
        if !p.carried || !locals.contains(p.owner) {
            continue;
        }
        let before = was.insert(e, p.open);
        if before.is_none_or(|b| b == p.open) {
            continue;
        }
        let (cue, speed) = if p.open {
            ("pack.open", 1.0)
        } else {
            ("pack.close", 1.1)
        };
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/zip",
            Some(gt.translation()),
            cue,
            0.0,
            speed,
            0.06,
        );
    }
}

/// The grapnel thrown, biting, coiled back up; a cover deployed and
/// folded; the bench finishing a job.
#[allow(clippy::too_many_arguments)]
fn gadget_sounds(
    mut commands: Commands,
    grapnels: Query<(Entity, &crate::gadgets::climbing::Grapnel, &GlobalTransform)>,
    covers: Query<(Entity, &GlobalTransform), With<crate::gadgets::CoverWall>>,
    benches: Query<(Entity, &crate::raid::bench::Bench, &GlobalTransform)>,
    mut grapnel_was: Local<HashMap<Entity, (u8, bool)>>,
    mut cover_at: Local<HashMap<Entity, Vec3>>,
    mut bench_was: Local<HashMap<Entity, bool>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::climbing::Grapnel;
    let mut out: Vec<(&str, &str, Vec3, f64)> = Vec::new();
    for (e, g, gt) in &grapnels {
        let now = match g {
            Grapnel::Coiled => (0, false),
            Grapnel::Ready { thrown, .. } => (1, *thrown),
            Grapnel::Hooked { .. } => (2, false),
        };
        let at = gt.translation();
        match (grapnel_was.insert(e, now), now) {
            (Some((1, false)), (1, true)) => {
                out.push(("physical/whoosh", "grapnel.throw", at, 0.8))
            }
            (Some((1, _)), (2, _)) => out.push(("impacts/metal", "grapnel.hook", at, 1.2)),
            (Some((a, _)), (0, _)) if a != 0 => {
                out.push(("physical/cloth", "grapnel.coil", at, 1.0))
            }
            _ => {}
        }
    }
    let mut seen = Vec::new();
    for (e, gt) in &covers {
        seen.push(e);
        if cover_at.insert(e, gt.translation()).is_none() {
            out.push(("physical/drop_metal", "cover.deploy", gt.translation(), 0.8));
        }
    }
    cover_at.retain(|e, at| {
        let kept = seen.contains(e);
        if !kept {
            out.push(("physical/drop_metal", "cover.fold", *at, 1.1));
        }
        kept
    });
    for (e, b, gt) in &benches {
        let running = b.running.is_some();
        if bench_was.insert(e, running) == Some(true) && !running {
            out.push((
                "physical/bell",
                "bench.done",
                gt.translation() + Vec3::Y,
                1.6,
            ));
        }
    }
    for (set, cue, at, speed) in out {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.06,
        );
    }
}

/// A robot noticing you: the whirr of its head snapping round. Each robot
/// at most every few seconds, however its suspicion wavers.
fn notice_sounds(
    mut commands: Commands,
    mut noticed: MessageReader<crate::robots::Noticed>,
    time: Res<Time>,
    mut last: Local<HashMap<Entity, f32>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let now = time.elapsed_secs();
    for n in noticed.read() {
        if last.get(&n.robot).is_some_and(|t| now - t < 8.0) {
            continue;
        }
        last.insert(n.robot, now);
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "machines/servo",
            Some(n.at),
            "robot.notice",
            0.0,
            1.35,
            0.05,
        );
    }
}

/// The collector's convoy: the clank of a load taken on, the groan of the
/// walker setting off through the Gate.
fn convoy_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::robots::collector::ConvoyEvent>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::robots::collector::ConvoyEvent;
    for ev in events.read() {
        let (robot, set, cue) = match ev {
            ConvoyEvent::Loaded { robot } => (*robot, "machines/clank", "convoy.loaded"),
            ConvoyEvent::Delivered { robot } => (*robot, "machines/groan", "convoy.delivered"),
        };
        let at = positions
            .get(robot)
            .ok()
            .map(|g| g.translation() + Vec3::Y * 2.0);
        play(&mut commands, &bank, &mut rng.0, set, at, cue, 0.0, 0.15);
    }
}

/// A dirigible's engines: a slow, low throb overhead, quickening as it
/// moves, gone when it's wrecked.
#[derive(Component)]
struct DirigibleLoop;

fn dirigible_loops(
    mut commands: Commands,
    ships: Query<(
        Entity,
        &crate::robots::dirigible::Dirigible,
        &Robot,
        Option<&Children>,
    )>,
    loops: Query<Entity, With<DirigibleLoop>>,
    mut settings: Query<&mut PlaybackSettings, With<DirigibleLoop>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    for (e, d, robot, children) in &ships {
        let existing = children.and_then(|c| c.iter().find(|c| loops.contains(*c)));
        match (existing, robot.wrecked) {
            (Some(l), true) => commands.entity(l).despawn(),
            (Some(l), false) => {
                if let Ok(mut s) = settings.get_mut(l) {
                    let speed = 0.4 + (d.velocity.length() as f64 / 8.0).min(1.0) * 0.15;
                    if (s.speed - speed).abs() > 0.01 {
                        s.speed = speed;
                    }
                }
            }
            (None, false) => {
                if let Some(sample) = bank.pick("machines/drone_rotor", &mut rng.0) {
                    commands.spawn((
                        DirigibleLoop,
                        SamplePlayer::new(sample)
                            .looping()
                            .with_volume(Volume::Decibels(mix::db("dirigible.engine"))),
                        PlaybackSettings::default().with_speed(0.4),
                        Transform::default(),
                        spatial_fx("dirigible.engine"),
                        ChildOf(e),
                    ));
                }
            }
            (None, true) => {}
        }
    }
}

/// The camera drone: the switch as its rotors spin up, a click in your
/// ear as it marks a robot, a knock as it settles back by you. A zipline: the clank
/// of its anchor biting, and its line coiled away.
#[allow(clippy::too_many_arguments)]
fn drone_and_zip_sounds(
    mut commands: Commands,
    drones: Query<(
        Entity,
        &crate::gadgets::camera_drone::CameraDrone,
        &GlobalTransform,
    )>,
    spotted: Query<(), Added<crate::gadgets::camera_drone::Spotted>>,
    ropes: Query<(Entity, &crate::gadgets::climbing::Rope)>,
    mut flight_was: Local<HashMap<Entity, crate::gadgets::camera_drone::Flight>>,
    mut zips: Local<HashMap<Entity, Vec3>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use crate::gadgets::camera_drone::Flight;
    let mut out: Vec<(&str, &str, Vec3)> = Vec::new();
    for (e, d, gt) in &drones {
        let before = flight_was.insert(e, d.flight);
        match (before, d.flight) {
            (Some(Flight::Idle), Flight::SpunUp) => {
                out.push(("physical/switch", "camera_drone.on", gt.translation()))
            }
            (Some(Flight::Up | Flight::Returning), Flight::Idle) => out.push((
                "physical/handle_plastic",
                "camera_drone.land",
                gt.translation(),
            )),
            _ => {}
        }
    }
    // A robot marked: a click in your ear, as from the drone's handset.
    for _ in &spotted {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            "physical/switch",
            None,
            "camera_drone.spot",
            0.0,
            1.3,
            0.03,
        );
    }
    let mut live = Vec::new();
    for (e, r) in &ropes {
        if !r.zip {
            continue;
        }
        live.push(e);
        if zips.insert(e, r.top).is_none() {
            out.push(("impacts/metal", "zip.rig", r.top));
        }
    }
    zips.retain(|e, at| {
        let kept = live.contains(e);
        if !kept {
            out.push(("physical/cloth", "zip.pack", *at));
        }
        kept
    });
    for (set, cue, at) in out {
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            0.06,
        );
    }
}

/// The kitchen: each stroke of work sounds like the work (a knife on the
/// board, stones grinding, a spoon or hands in the bowl), a dish done is a
/// tap on the pot, an abandoned one a rustle.
fn kitchen_sounds(
    mut commands: Commands,
    mut events: MessageReader<crate::raid::kitchen::Cooking>,
    positions: Query<&GlobalTransform>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    use extraction_core::cooking::{Event, Process};
    for c in events.read() {
        let (set, cue, speed) = match (c.what, c.process) {
            (Event::Stroke { .. }, Process::Chop) => ("impacts/wood", "kitchen.chop", 1.6),
            (Event::Stroke { .. }, Process::Grind) => ("footsteps/gravel", "kitchen.grind", 1.2),
            (Event::Stroke { .. }, _) => ("physical/cloth", "kitchen.work", 1.0),
            (Event::Done(_), _) => ("physical/handle_metal", "kitchen.done", 0.9),
            (Event::Aborted, _) => ("physical/cloth", "kitchen.work", 0.8),
            _ => continue,
        };
        let at = positions.get(c.station).ok().map(|g| g.translation());
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            at,
            cue,
            0.0,
            speed,
            0.08,
        );
    }
}

/// How robots think, out loud: a whirr as one goes to look, servo sweeps
/// as it searches, a falling whine as it gives up, a squelch as it calls
/// the others; and the attack tells, a walker's turret settling before
/// its first shot and a grappler winding up its throw. A robot's search
/// sounds are spaced so a long hunt doesn't chatter.
#[allow(clippy::too_many_arguments)]
fn mind_sounds(
    mut commands: Commands,
    mut investigating: MessageReader<crate::robots::search::Investigating>,
    mut searching: MessageReader<crate::robots::search::Searching>,
    mut lost: MessageReader<crate::robots::search::LostYou>,
    mut called: MessageReader<crate::robots::search::CalledOut>,
    mut aims: MessageReader<crate::robots::walker::WalkerAim>,
    mut windups: MessageReader<crate::robots::grappler::HarpoonWindup>,
    positions: Query<&GlobalTransform>,
    time: Res<Time>,
    mut last: Local<HashMap<(Entity, u8), f32>>,
    bank: Res<SoundBank>,
    mut rng: ResMut<AudioRng>,
) {
    let now = time.elapsed_secs();
    let mut due = |robot: Entity, kind: u8, every: f32| {
        let ok = last.get(&(robot, kind)).is_none_or(|t| now - t >= every);
        if ok {
            last.insert((robot, kind), now);
        }
        ok
    };
    let mut out: Vec<(&str, &str, Vec3, f64)> = Vec::new();
    for m in investigating.read() {
        if due(m.robot, 0, 6.0) {
            out.push(("machines/servo", "robot.investigate", m.at, 1.15));
        }
    }
    for m in searching.read() {
        if due(m.robot, 1, 6.0) {
            out.push(("machines/servo", "robot.search", m.at, 0.95));
        }
    }
    for m in lost.read() {
        out.push(("machines/servo", "robot.lost", m.at, 0.7));
    }
    for m in called.read() {
        if due(m.robot, 2, 10.0) {
            out.push(("machines/radio_squelch", "robot.callout", m.at, 1.0));
        }
    }
    for m in aims.read() {
        out.push(("machines/clank", "walker.aim", m.at, 1.3));
        out.push(("machines/servo", "walker.aim_lock", m.at, 1.5));
    }
    for m in windups.read() {
        if let Ok(gt) = positions.get(m.from) {
            out.push(("machines/servo", "harpoon.windup", gt.translation(), 0.8));
        }
    }
    for (set, cue, at, speed) in out {
        play_at_speed(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            cue,
            0.0,
            speed,
            0.04,
        );
    }
}

/// The space you're in, probed from your head a few times a second, sets
/// the reverb: dry in the open, a short warm tail in a cabin, a ring in a
/// tin shed, a long echo underground. Eased over half a second, so a
/// doorway is a crossfade, not a switch.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Listening {
    pub space: crate::acoustics::Space,
}

fn room_acoustics(
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    spatial: SpatialQuery,
    built: Query<(&GlobalTransform, &Surfaces)>,
    lighting: Option<Res<crate::robots::senses::Lighting>>,
    time: Res<Time>,
    mut listening: ResMut<Listening>,
    mut send: Query<&mut VolumeNode, With<ReverbIn>>,
    mut reverb: Query<&mut FreeverbNode>,
    mut next: Local<f32>,
    mut now: Local<Option<crate::acoustics::Reverb>>,
) {
    let Some(head) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    *next -= time.delta_secs();
    if *next <= 0.0 {
        *next = 0.2;
        listening.space = crate::acoustics::probe(&spatial, &built, lighting.as_deref(), head);
    }
    let want = listening.space.reverb();
    let k = (time.delta_secs() * 2.0).min(1.0);
    let cur = now.get_or_insert(want);
    cur.wet_db += (want.wet_db - cur.wet_db) * k;
    cur.rt60 += (want.rt60 - cur.rt60) * k;
    cur.damping += (want.damping - cur.damping) * k;
    for mut v in &mut send {
        v.volume = if cur.wet_db <= crate::acoustics::DRY + 1.0 {
            Volume::SILENT
        } else {
            Volume::Decibels(cur.wet_db)
        };
    }
    for mut r in &mut reverb {
        let room = crate::acoustics::freeverb_room(cur.rt60);
        if (r.room_size - room).abs() > 0.005 || (r.damping - cur.damping).abs() > 0.005 {
            r.room_size = room;
            r.damping = cur.damping;
        }
    }
}

/// A placed sound heard round or through what's between it and you
/// (`acoustics::occlusion`, the robots' rule): checked as it starts, and
/// every quarter second for loops, a few rays a frame.
#[derive(Component)]
struct Occluded {
    next: f32,
}

#[allow(clippy::too_many_arguments)]
fn occlude(
    mut commands: Commands,
    players: Query<
        (
            Entity,
            &GlobalTransform,
            &SampleEffects,
            &SamplePlayer,
            Option<&Occluded>,
        ),
        Without<SpatialListener3D>,
    >,
    mut nodes: Query<&mut SpatialBasicNode>,
    listener: Query<&GlobalTransform, With<SpatialListener3D>>,
    spatial: SpatialQuery,
    doors: Query<&crate::world::buildings::Door>,
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    time: Res<Time>,
) {
    let Some(ear) = listener.iter().next().map(|g| g.translation()) else {
        return;
    };
    let now = time.elapsed_secs();
    let openings: Vec<Vec3> = entrances
        .map(|e| {
            e.doors
                .iter()
                .filter(|d| crate::robots::doors::is_open(doors.get(d.door).ok()))
                .map(|d| d.mid)
                .collect()
        })
        .unwrap_or_default();
    let mut budget = 24;
    for (e, gt, effects, player, occluded) in &players {
        let due = match occluded {
            None => true,
            Some(o) => {
                o.next.is_finite() && player.repeat_mode != RepeatMode::PlayOnce && now >= o.next
            }
        };
        if !due || budget == 0 {
            continue;
        }
        let Ok(mut node) = nodes.get_effect_mut(effects) else {
            // Not placed (in your head): nothing to occlude, ever.
            commands.entity(e).insert(Occluded {
                next: f32::INFINITY,
            });
            continue;
        };
        budget -= 1;
        let o = crate::acoustics::occlusion(&spatial, &openings, gt.translation(), ear);
        let v = Volume::Decibels(o.db);
        if node.volume != v {
            node.volume = v;
        }
        if (node.muffle_cutoff_hz - o.cutoff).abs() > 1.0 {
            node.muffle_cutoff_hz = o.cutoff;
        }
        commands.entity(e).insert(Occluded { next: now + 0.25 });
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
        play(
            &mut commands,
            &bank,
            &mut rng.0,
            set,
            Some(at),
            "pylon.work",
            0.0,
            0.2,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Using a thing sounds like what it is, and anything you'd eat or use
    /// up makes a sound (silence read as "nothing happened" in VR).
    #[test]
    fn using_things_sounds_like_what_they_are() {
        let catalog = crate::items::Catalog::load();
        let sound = |id: &str| {
            let (_, def) = catalog.0.iter().find(|(_, d)| d.id == id).expect(id);
            use_sound(def).map(|s| s.0)
        };
        assert_eq!(sound("crackers"), Some("physical/eat"));
        assert_eq!(sound("water_bottle"), Some("physical/drink"));
        assert_eq!(sound("field_dressing"), Some("physical/bandage"));
        assert_eq!(sound("flashlight"), Some("physical/switch"));
        assert_eq!(sound("radio"), None, "the radio has its own");
        let bank: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(crate::asset_root_path().join("audio/sounds.json")).unwrap(),
        )
        .unwrap();
        for (_, def) in catalog.0.iter() {
            if def.category == extraction_core::items::Category::Food {
                assert!(use_sound(def).is_some(), "{} is eaten silently", def.id);
            }
            if let Some((set, _)) = use_sound(def) {
                assert!(bank.get(set).is_some(), "{set}");
            }
            let handle = handling_set(crate::items::impacts::material_of(def));
            assert!(bank.get(handle).is_some(), "{handle}");
        }
    }

    /// On a real audio graph (seedling's mock backend): the reverb bus
    /// feeds the reverb, and a placed sound's send reaches the bus.
    #[test]
    fn placed_sounds_reach_the_reverb() {
        use bevy_seedling::node::FirewheelNode;
        use bevy_seedling::platform::mock::MockBackendPlugin;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            bevy_seedling::SeedlingCorePlugin,
            MockBackendPlugin,
            TransformPlugin,
        ))
        .add_systems(Startup, |mut commands: Commands| {
            spawn_reverb_bus(&mut commands);
            commands.spawn((Transform::default(), SpatialListener3D));
            commands.spawn((
                SamplePlayer::new(Handle::<AudioSample>::default()).looping(),
                Transform::from_xyz(3.0, 0.0, 0.0),
                spatial_fx("step"),
            ));
        });
        app.finish();
        app.cleanup();
        for _ in 0..10 {
            app.update();
        }
        let world = app.world_mut();
        let bus = world
            .query_filtered::<&FirewheelNode, With<ReverbIn>>()
            .iter(world)
            .next()
            .map(|n| n.0)
            .expect("the bus is a node");
        let verb = world
            .query_filtered::<&FirewheelNode, With<FreeverbNode>>()
            .iter(world)
            .next()
            .map(|n| n.0)
            .expect("the reverb is a node");
        let sends: Vec<_> = world
            .query_filtered::<&FirewheelNode, With<SendNode>>()
            .iter(world)
            .map(|n| n.0)
            .collect();
        assert!(!sends.is_empty(), "a send was made");
        let edges: Vec<(_, _)> = world
            .resource_mut::<bevy_seedling::context::AudioContext>()
            .with(|cx| cx.edges().map(|e| (e.src_node, e.dst_node)).collect());
        assert!(edges.contains(&(bus, verb)), "bus -> reverb");
        assert!(
            sends.iter().any(|s| edges.contains(&(*s, bus))),
            "send -> bus: {edges:?}"
        );
    }

    /// Heavier sounds louder and lower, within bounds.
    #[test]
    fn weight_is_heard() {
        let (tin, rifle) = (weight_tone(0.05), weight_tone(3.5));
        assert!(tin.0 < rifle.0 && tin.1 > rifle.1);
        for m in [0.0, 0.01, 1.0, 50.0] {
            let (db, speed) = weight_tone(m);
            assert!((-9.0..=5.0).contains(&db) && (0.85..=1.2).contains(&speed));
        }
    }

    /// Every sound a map asks for is in the index (a missing set plays
    /// nothing, silently).
    #[test]
    fn every_valley_sound_exists() {
        let path = crate::asset_root_path().join("audio/sounds.json");
        let index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut missing = Vec::new();
        for map in crate::test_maps() {
            let sounds = crate::world::Zone::load(&map).map.sounds.clone();
            for set in std::iter::once(&sounds.wind)
                .chain(&sounds.birds)
                .chain(&sounds.distant)
            {
                if index.get(set).is_none() {
                    missing.push(format!("{map}: {set}"));
                }
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }
}
