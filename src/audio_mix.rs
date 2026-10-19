//! The mix: every sound the game plays, its playback gain, and what it is
//! (`docs/AUDIO.md`, "Checking the mix").
//!
//! `audio.rs` takes its gains from here, and `scripts/audio_audit.py` reads
//! this file too, so the numbers the audit judges are the ones the game
//! plays. Keep one `cue!` per line: the audit parses them.
//!
//! Each cue says:
//! - `db`: playback gain on top of the asset's own level (assets are
//!   peak-normalised one-shots or loudness-normalised loops, see the
//!   mastering chain). For cues whose gain moves (footsteps with speed,
//!   rain with the weather), the loudest it gets.
//! - `spl`: how loud the real thing is where you'd hear it, in dB(A)
//!   (rough, from published measurements: a rifle at the shooter's ear,
//!   a footstep at the walker's, rain on the ground around you). The
//!   audit maps these into the game's range with one compression slope,
//!   so the mix keeps the world's order without its 100 dB span.
//! - `dist`: how far away it usually plays, metres.
//! - `reach`: where its distance falloff starts (full volume nearer than
//!   this, then -6 dB per doubling): 5 m for most, more for what must
//!   carry (the Surge siren across the valley, a robot's gun).
//! - `role`: what it's for. Beds are the room you're in; details are
//!   handling and foley; actions are the loud things that happen; signals
//!   carry information you must not miss (a drone locking on, a hazard),
//!   and are held above the beds whatever their realism says; events are
//!   rare set pieces (the siren, thunder).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Bed,
    Detail,
    Action,
    Signal,
    Event,
}

#[derive(Clone, Copy, Debug)]
pub struct Cue {
    pub name: &'static str,
    pub set: &'static str,
    pub db: f32,
    pub spl: f32,
    pub dist: f32,
    pub reach: f32,
    pub role: Role,
}

macro_rules! cue {
    ($name:literal, $set:literal, $db:expr, $spl:expr, $dist:expr, $role:ident) => {
        cue!($name, $set, $db, $spl, $dist, $role, 5.0)
    };
    ($name:literal, $set:literal, $db:expr, $spl:expr, $dist:expr, $role:ident, $reach:expr) => {
        Cue {
            name: $name,
            set: $set,
            db: $db,
            spl: $spl,
            dist: $dist,
            reach: $reach,
            role: Role::$role,
        }
    };
}

/// High frequencies fade with distance (air takes them): fully muffled at
/// this distance. Firewheel's default, 200 m, silenced everything past it,
/// the Surge siren included; air itself is far gentler.
pub const MUFFLE_FULL: f32 = 900.0;

#[rustfmt::skip]
pub const MIX: &[Cue] = &[
    // Your own body and hands.
    cue!("step", "footsteps/grass", -8.0, 55.0, 1.7, Detail),
    cue!("land", "footsteps/grass", -7.5, 65.0, 1.7, Detail),
    cue!("splash", "footsteps/water", -3.0, 70.0, 1.7, Detail),
    cue!("hand.grab", "physical/cloth", -17.0, 40.0, 0.6, Detail),
    cue!("hand.refused", "physical/cloth", -10.0, 45.0, 0.6, Signal),
    cue!("hand.stow", "physical/zip", -15.0, 55.0, 0.6, Detail),
    cue!("hand.socket", "physical/cloth", -15.0, 50.0, 0.6, Detail),
    cue!("hand.attach", "guns/safety", -9.0, 55.0, 0.6, Detail),
    cue!("hand.take", "physical/handle_metal", -14.5, 55.0, 0.5, Detail),
    cue!("hand.release", "physical/handle_metal", -16.5, 45.0, 0.5, Detail),
    cue!("hand.place", "physical/handle_metal", -14.5, 55.0, 0.6, Detail),
    cue!("pack.open", "physical/zip", -18.5, 55.0, 0.6, Detail),
    cue!("pack.close", "physical/zip", -18.5, 55.0, 0.6, Detail),
    cue!("far.whoosh", "physical/whoosh", -11.5, 60.0, 2.0, Detail),
    cue!("far.catch", "physical/catch", -0.5, 65.0, 0.5, Detail),
    cue!("button.in", "physical/button", -4.0, 60.0, 0.5, Detail),
    cue!("button.out", "physical/button", -5.0, 52.0, 0.5, Detail),
    cue!("press.lever", "physical/switch", -1.0, 65.0, 0.6, Detail),
    cue!("use.eat", "physical/eat", -10.0, 50.0, 0.3, Detail),
    cue!("use.drink", "physical/drink", -13.5, 50.0, 0.3, Detail),
    cue!("use.bandage", "physical/bandage", -12.0, 60.0, 0.4, Detail),
    cue!("grapnel.throw", "physical/whoosh", -12.0, 60.0, 2.0, Detail),
    cue!("grapnel.hook", "impacts/metal", -8.5, 80.0, 6.0, Detail),
    cue!("grapnel.coil", "physical/cloth", -12.0, 50.0, 0.6, Detail),
    cue!("cover.deploy", "physical/drop_metal", -7.5, 75.0, 1.5, Detail),
    cue!("cover.fold", "physical/drop_metal", -8.5, 70.0, 1.5, Detail),
    cue!("bench.done", "physical/bell", -10.0, 70.0, 2.0, Detail),
    cue!("use.switch", "physical/switch", -3.5, 50.0, 0.4, Detail),
    cue!("step.done", "physical/chime", -22.0, 60.0, 0.0, Detail),
    cue!("drop", "physical/drop_metal", -6.0, 70.0, 1.5, Detail),
    // Guns in your hands.
    cue!("gun.shot", "guns/rifle_shot", 0.0, 140.0, 0.5, Action),
    cue!("gun.distant", "guns/distant_shot", -6.0, 95.0, 100.0, Event),
    cue!("gun.dry_fire", "guns/dry_fire", -5.0, 70.0, 0.5, Signal),
    cue!("gun.mag_insert", "guns/mag_insert", -6.0, 75.0, 0.5, Detail),
    cue!("gun.mag_release", "guns/mag_release", -5.0, 70.0, 0.5, Detail),
    cue!("gun.safety", "guns/safety", -7.0, 60.0, 0.5, Detail),
    cue!("gun.shell_insert", "guns/shell_insert", -8.0, 65.0, 0.5, Detail),
    cue!("gun.stripper_clip", "guns/stripper_clip", -10.0, 70.0, 0.5, Detail),
    cue!("gun.bolt_close", "guns/bolt_close", -9.0, 80.0, 0.5, Detail),
    cue!("gun.bolt_open", "guns/bolt_open", -9.0, 78.0, 0.5, Detail),
    cue!("gun.shotgun_close", "guns/shotgun_close", -4.0, 80.0, 0.5, Detail),
    cue!("gun.shotgun_open", "guns/shotgun_open", -4.0, 75.0, 0.5, Detail),
    cue!("gun.charging", "guns/charging_handle", -10.5, 80.0, 0.5, Detail),
    cue!("gun.slide_rack", "guns/slide_rack", -6.0, 80.0, 0.5, Detail),
    cue!("gun.casing", "guns/casing", -9.0, 60.0, 1.5, Detail),
    // Bullets landing.
    cue!("hit.impact", "impacts/metal", -2.0, 95.0, 15.0, Action, 8.0),
    cue!("hit.ricochet", "impacts/ricochet", -1.0, 90.0, 15.0, Action, 8.0),
    cue!("hit.glance", "impacts/ricochet", -2.5, 100.0, 1.0, Signal),
    cue!("hit.body", "physical/body_hit", -8.0, 90.0, 0.5, Signal),
    // Machines.
    cue!("robot.shot", "guns/pistol_shot", -3.0, 120.0, 30.0, Action, 20.0),
    cue!("drone.rotor", "machines/drone_rotor", -4.0, 75.0, 20.0, Signal),
    cue!("drone.lock", "machines/drone_lock", -28.0, 80.0, 20.0, Signal, 25.0),
    cue!("walker.step", "footsteps/metal", -2.5, 85.0, 25.0, Signal, 16.0),
    cue!("walker.servo", "machines/servo", -9.5, 70.0, 25.0, Detail, 10.0),
    cue!("harpoon.fire", "machines/clank", 1.0, 100.0, 15.0, Action, 7.0),
    cue!("harpoon.hit", "impacts/metal", -6.0, 100.0, 1.0, Action),
    cue!("pylon.work", "machines/servo", -9.5, 75.0, 30.0, Detail, 15.0),
    cue!("robot.part", "machines/clank", 1.0, 100.0, 15.0, Action, 10.0),
    cue!("robot.glass", "physical/glass_break", -1.0, 90.0, 15.0, Action, 8.0),
    cue!("robot.wreck", "machines/groan", -8.5, 95.0, 15.0, Action, 8.0),
    cue!("camera_drone", "machines/drone_rotor", -16.0, 60.0, 5.0, Detail),
    cue!("zipline", "machines/hum", -10.0, 65.0, 0.6, Detail),
    // The radio.
    cue!("radio.squelch", "machines/radio_squelch", -8.5, 75.0, 0.6, Signal),
    cue!("radio.static", "machines/radio_static", -16.0, 65.0, 0.6, Detail),
    cue!("chatter.squelch", "machines/radio_squelch", -8.5, 72.0, 0.6, Signal),
    cue!("chatter.static", "machines/radio_static", -12.0, 65.0, 0.6, Signal),
    cue!("chatter.chirp", "machines/servo", -20.0, 55.0, 0.6, Detail),
    // The truck.
    cue!("truck.arrive", "machines/truck_arrive", -6.0, 85.0, 30.0, Signal, 15.0),
    cue!("truck.idle", "machines/truck_idle", -9.0, 75.0, 15.0, Signal, 10.0),
    cue!("truck.depart", "machines/truck_depart", -10.0, 85.0, 10.0, Event, 15.0),
    cue!("truck.bell", "physical/bell", -7.5, 85.0, 1.0, Action),
    // Doors.
    cue!("door.open", "physical/door_open", -8.0, 65.0, 3.0, Detail),
    cue!("door.close", "physical/door_close", -9.0, 75.0, 3.0, Detail),
    cue!("door.blow", "impacts/wood", 1.0, 100.0, 20.0, Signal, 30.0),
    cue!("door.broken", "physical/door_close", -8.5, 100.0, 20.0, Signal, 14.0),
    // Anomalies and the Surge.
    cue!("anomaly.lift", "anomaly/lift_hum", -6.0, 65.0, 8.0, Signal),
    cue!("anomaly.burner", "anomaly/burner_roar", 0.0, 75.0, 8.0, Signal),
    cue!("anomaly.snare", "anomaly/snare_buzz", -2.0, 60.0, 8.0, Signal),
    cue!("anomaly.discharge", "anomaly/surge_crack", 2.0, 110.0, 8.0, Action),
    cue!("anomaly.pop", "anomaly/lift_pop", -10.0, 100.0, 8.0, Action, 6.0),
    cue!("anomaly.bloom", "anomaly/surge_crack", -1.0, 95.0, 8.0, Action),
    cue!("offering", "anomaly/offering", -19.0, 70.0, 2.0, Signal),
    cue!("emp.crack", "anomaly/surge_crack", 2.0, 120.0, 10.0, Action, 8.0),
    cue!("emp.pop", "anomaly/lift_pop", -10.0, 110.0, 10.0, Action, 10.0),
    cue!("attention", "anomaly/attention", -9.0, 60.0, 0.0, Signal),
    cue!("surge.drone", "anomaly/surge_drone", 0.0, 85.0, 0.0, Event),
    cue!("surge.crack", "anomaly/surge_crack", 4.0, 115.0, 120.0, Event, 60.0),
    cue!("surge.siren", "machines/siren", -2.0, 125.0, 150.0, Event, 200.0),
    // The world around you.
    cue!("wind", "ambience/wind_conifers", -19.0, 48.0, 0.0, Bed),
    cue!("rain", "ambience/rain_light", -15.0, 60.0, 0.0, Bed),
    cue!("room", "ambience/room_tone", -18.0, 35.0, 0.0, Bed),
    cue!("creek", "ambience/creek", -4.0, 60.0, 15.0, Bed),
    cue!("birds", "ambience/crow", -2.0, 70.0, 45.0, Event),
    cue!("valley.distant", "ambience/rockfall", 2.0, 90.0, 110.0, Event),
    cue!("works.groan", "machines/groan", -2.0, 85.0, 80.0, Event),
    cue!("works.clank", "machines/clank", -4.0, 85.0, 80.0, Event),
    cue!("thunder", "ambience/thunder_distant", -14.0, 75.0, 0.0, Event),
];

/// Narrow dynamics (the comfort board's sound card), dB per role: the
/// forest and your hands come up, gunfire and set pieces come down, for a
/// headset's open speakers in a room that isn't quiet. One line per role:
/// the audit reads these.
#[rustfmt::skip]
pub const NARROW: &[(Role, f32)] = &[
    (Role::Bed, 6.0),
    (Role::Detail, 5.0),
    (Role::Signal, 5.0),
    (Role::Action, -3.0),
    (Role::Event, -3.0),
];

static NARROW_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Play with narrow dynamics from now on (sounds already playing keep
/// their level, beds aside, whose level is set every frame).
pub fn set_narrow(on: bool) {
    NARROW_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// A cue's gain, dB. An unknown name logs an error and plays quietly
/// rather than taking the game down (a test checks every name the audio
/// code uses).
pub fn db(name: &str) -> f32 {
    find(name).map_or(-20.0, |c| {
        let lift = if NARROW_ON.load(std::sync::atomic::Ordering::Relaxed) {
            NARROW
                .iter()
                .find(|(r, _)| *r == c.role)
                .map_or(0.0, |l| l.1)
        } else {
            0.0
        };
        c.db + lift
    })
}

fn find(name: &str) -> Option<&'static Cue> {
    let c = MIX.iter().find(|c| c.name == name);
    if c.is_none() {
        bevy::log::error!("no cue `{name}` in the mix");
    }
    c
}

/// The spatial node a cue plays through: its reach, and the gentler muffle.
pub fn spatial(name: &str) -> bevy_seedling::prelude::SpatialBasicNode {
    use bevy_seedling::firewheel::dsp::distance_attenuation::DistanceAttenuation;
    let reach = find(name).map_or(5.0, |c| c.reach);
    bevy_seedling::prelude::SpatialBasicNode {
        distance_attenuation: DistanceAttenuation {
            reference_distance: reach,
            max_muffle_distance: MUFFLE_FULL,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Narrow dynamics moves every role, and closes the gap between the
    /// quiet ones and the loud ones rather than widening it.
    #[test]
    fn narrow_dynamics_lifts_the_quiet_over_the_loud() {
        let lift = |r: Role| NARROW.iter().find(|(x, _)| *x == r).map(|l| l.1);
        for r in [
            Role::Bed,
            Role::Detail,
            Role::Action,
            Role::Signal,
            Role::Event,
        ] {
            assert!(lift(r).is_some(), "{r:?} has a lift");
        }
        assert!(lift(Role::Bed) > lift(Role::Action));
        assert!(lift(Role::Detail) > lift(Role::Action));
        assert!(
            lift(Role::Signal) >= lift(Role::Detail),
            "warnings keep their reach"
        );
    }

    /// Every cue's set exists, every name is unique, and every name the
    /// audio code asks for is in the table.
    #[test]
    fn the_mix_table_is_whole() {
        let path = crate::asset_root_path().join("audio/sounds.json");
        let index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut seen = std::collections::HashSet::new();
        for c in MIX {
            assert!(index.get(c.set).is_some(), "{}: no set {}", c.name, c.set);
            assert!(seen.insert(c.name), "{} twice", c.name);
        }
        // Every lower-case literal in audio.rs without a slash is a cue
        // name, bar a few it matches on (gun ids, prefabs, JSON keys).
        // (Its tests name items, not cues.)
        let code = include_str!("audio.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        const NOT_CUES: &[&str] = &[
            "pm_pistol",
            "ak74",
            "mosin",
            "toz_double",
            "factory_hall",
            "chimney",
            "rail_shed",
            "bus_depot",
            "pump_station",
            "boxcar",
            "file",
            "files",
            "field_dressing",
            "radio",
            "deployable_cover",
            "climbing_kit",
            "zipline_kit",
            "camera_drone",
            "light",
        ];
        for lit in code.split('"').skip(1).step_by(2) {
            let cue_like = !lit.is_empty()
                && lit
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.');
            if cue_like && !NOT_CUES.contains(&lit) {
                assert!(seen.contains(lit), "audio.rs uses `{lit}`, not in the mix");
            }
        }
    }
}
