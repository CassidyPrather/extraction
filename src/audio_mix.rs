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

/// The score's gain (`audio_score`), set by measurement: at full it stays
/// under the wind and hides no robot (`scripts/audio_score.py`).
pub const SCORE_DB: f32 = -42.0;

/// The score steps aside for a robot speaking within `SCORE_DUCK_NEAR` m:
/// down this many dB while it speaks (the voices are the information; the
/// score only the mood). `scripts/audio_score.py` checks voices with it.
pub const SCORE_DUCK_DB: f32 = 12.0;
pub const SCORE_DUCK_NEAR: f32 = 80.0;

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
    cue!("kitchen.chop", "impacts/wood", -6.0, 65.0, 0.6, Detail),
    cue!("kitchen.grind", "footsteps/gravel", -7.5, 60.0, 0.6, Detail),
    cue!("kitchen.work", "physical/cloth", -12.0, 50.0, 0.6, Detail),
    cue!("kitchen.done", "physical/handle_metal", -14.0, 60.0, 1.0, Detail),
    cue!("use.switch", "physical/switch", -3.5, 50.0, 0.4, Detail),
    cue!("step.done", "physical/chime", -22.0, 60.0, 0.0, Detail),
    cue!("step.station", "ui/chime_station", -20.0, 65.0, 0.0, Detail),
    cue!("drop", "physical/drop_metal", -6.0, 70.0, 1.5, Detail),
    cue!("drop.heavy", "impacts/dirt", -6.0, 85.0, 1.5, Event, 25.0),
    // Guns in your hands.
    cue!("gun.shot", "guns/rifle_shot", 0.0, 140.0, 0.5, Action),
    cue!("gun.shot_suppressed", "guns/rifle_shot", -5.0, 118.0, 0.5, Action),
    cue!("gun.distant", "guns/distant_shot", -6.0, 95.0, 100.0, Event),
    cue!("gun.malfunction", "guns/bolt_close", -9.0, 75.0, 0.5, Signal),
    cue!("gun.dry_fire", "guns/dry_fire", -5.0, 70.0, 0.5, Signal),
    cue!("gun.mag_insert", "guns/mag_insert", -6.0, 75.0, 0.5, Detail),
    cue!("gun.mag_release", "guns/mag_release", -5.0, 70.0, 0.5, Detail),
    cue!("gun.safety", "guns/safety", -7.0, 60.0, 0.5, Detail),
    cue!("gun.shell_insert", "guns/shell_insert", -8.0, 65.0, 0.5, Detail),
    cue!("gun.stripper_clip", "guns/stripper_clip", -10.0, 70.0, 0.5, Detail),
    cue!("gun.bolt_close", "guns/bolt_close", -9.0, 80.0, 0.5, Detail),
    cue!("gun.carrier_home", "guns/bolt_close", -10.0, 80.0, 0.5, Detail),
    cue!("gun.open_bolt", "guns/bolt_close", -12.0, 80.0, 0.5, Detail),
    cue!("gun.bump", "physical/drop_metal", -10.0, 75.0, 1.0, Detail),
    cue!("drag.metal", "drag/metal", -18.5, 60.0, 1.5, Detail, 1.0),
    cue!("drag.wood", "drag/wood", -16.0, 60.0, 1.5, Detail, 1.0),
    cue!("drag.roll", "drag/roll", -16.0, 60.0, 1.5, Detail, 1.0),
    cue!("drag.body", "drag/body", -16.0, 60.0, 1.5, Detail, 1.0),
    cue!("gun.bolt_open", "guns/bolt_open", -9.0, 78.0, 0.5, Detail),
    cue!("gun.shotgun_close", "guns/shotgun_close", -4.0, 80.0, 0.5, Detail),
    cue!("gun.shotgun_open", "guns/shotgun_open", -4.0, 75.0, 0.5, Detail),
    cue!("gun.charging", "guns/charging_handle", -10.5, 80.0, 0.5, Detail),
    cue!("gun.slide_rack", "guns/slide_rack", -6.0, 80.0, 0.5, Detail),
    cue!("gun.casing", "guns/casing", -9.0, 60.0, 1.5, Detail),
    cue!("gun.pump_back", "guns/shotgun_open", -4.0, 75.0, 0.5, Detail),
    cue!("gun.pump_fwd", "guns/shotgun_close", -4.0, 80.0, 0.5, Detail),
    cue!("flare.pop", "guns/pistol_shot", -12.0, 85.0, 0.5, Detail),
    cue!("flare.burn", "anomaly/burner_roar", -2.0, 60.0, 10.0, Detail),
    cue!("bipod.swing", "physical/handle_metal", -14.0, 55.0, 0.6, Detail),
    cue!("bipod.lock", "machines/clank", -10.0, 70.0, 0.6, Detail),
    cue!("hand.slap_wood", "impacts/wood", -6.0, 80.0, 0.5, Detail),
    cue!("hand.slap_metal", "impacts/metal", -8.0, 80.0, 0.5, Detail),
    cue!("hand.slap_glass", "physical/glass_break", -12.0, 70.0, 0.5, Detail),
    cue!("hand.slap_stone", "impacts/concrete", -6.0, 75.0, 0.5, Detail),
    cue!("hand.slap_skin", "physical/body_hit", -11.0, 70.0, 0.5, Detail),
    cue!("hand.slap_wire", "physical/handle_metal", -10.0, 70.0, 0.5, Detail),
    cue!("gun.squib", "guns/rifle_shot", -6.0, 85.0, 0.5, Action),
    cue!("gun.bulged", "guns/rifle_shot", -1.5, 130.0, 0.5, Action),
    cue!("gun.bulged_ring", "impacts/metal", -8.0, 100.0, 0.5, Action),
    cue!("gun.out_of_battery", "guns/rifle_shot", -4.0, 120.0, 0.5, Action),
    cue!("gun.burst_case", "physical/drop_metal", -8.0, 90.0, 0.5, Action),
    cue!("gun.grind", "physical/handle_metal", -14.0, 60.0, 0.5, Detail),
    cue!("smith.clamp", "machines/clank", -8.0, 75.0, 1.0, Detail),
    cue!("smith.saw", "physical/handle_metal", -12.0, 65.0, 1.0, Detail),
    cue!("smith.cut", "impacts/metal", -8.0, 80.0, 1.0, Detail),
    cue!("smith.screw", "physical/door_creak", -16.0, 55.0, 0.5, Detail),
    cue!("smith.stock", "physical/handle_wood", -8.0, 65.0, 0.5, Detail),
    cue!("glow.snap", "physical/handle_plastic", -8.0, 65.0, 0.5, Detail),
    cue!("glow.strike", "physical/zip", -16.0, 70.0, 0.5, Detail),
    cue!("glow.burn", "anomaly/burner_roar", -3.0, 65.0, 1.0, Detail, 1.0),
    cue!("gun.rod_scrape", "physical/handle_metal", -12.0, 55.0, 0.5, Detail),
    cue!("gun.rod_tink", "guns/casing", -10.0, 55.0, 0.8, Detail),
    cue!("gun.bolt_stuck", "guns/bolt_open", -8.0, 70.0, 0.5, Detail),
    cue!("gun.bolt_freed", "impacts/metal", -10.0, 85.0, 0.5, Action),
    cue!("gun.slam_snap", "guns/safety", -6.0, 70.0, 0.5, Detail),
    cue!("gear.plate", "physical/drop_metal", -7.0, 55.0, 0.8, Detail, 1.0),
    cue!("gear.swivel", "physical/handle_metal", -14.0, 45.0, 0.8, Detail, 1.0),
    cue!("gear.pack", "physical/cloth", -11.0, 50.0, 0.8, Detail, 1.0),
    cue!("gear.slosh", "footsteps/water", 0.0, 45.0, 0.8, Detail, 1.0),
    cue!("armour.clang", "impacts/metal", -1.5, 110.0, 15.0, Action, 8.0),
    cue!("armour.thump", "physical/body_hit", -4.0, 90.0, 15.0, Action, 8.0),
    cue!("armour.ping", "impacts/ricochet", 1.0, 105.0, 15.0, Action, 8.0),
    cue!("epilogue.creak", "physical/door_creak", -13.0, 60.0, 2.0, Event),
    cue!("scope.click", "guns/safety", -8.0, 45.0, 0.5, Detail),
    cue!("break.crunch", "physical/glass_break", -12.0, 60.0, 1.7, Detail),
    cue!("break.glass", "physical/glass_break", -9.0, 95.0, 10.0, Action, 8.0),
    cue!("break.wood", "impacts/wood", 3.0, 90.0, 10.0, Action, 8.0),
    cue!("break.splinter", "impacts/wood", -2.0, 80.0, 10.0, Detail, 6.0),
    cue!("break.wood_big", "physical/door_close", -9.0, 95.0, 10.0, Action, 10.0),
    cue!("break.breach", "impacts/concrete", 0.0, 105.0, 15.0, Action, 12.0),
    cue!("break.rubble", "ambience/rockfall", -4.0, 90.0, 15.0, Action, 10.0),
    cue!("flock.wings", "physical/cloth", -6.0, 70.0, 20.0, Event, 8.0),
    cue!("flock.call", "ambience/crow", -3.5, 75.0, 25.0, Event, 15.0),
    cue!("detector.ping", "device/stone_ping", -14.0, 60.0, 0.5, Signal),
    cue!("bite.shock", "device/crackle", -8.0, 85.0, 0.4, Action),
    cue!("bite.glint", "physical/chime", -22.0, 45.0, 0.5, Detail),
    cue!("burn.sizzle", "anomaly/burner_roar", -6.0, 65.0, 0.5, Detail),
    cue!("tool.pry", "physical/door_creak", -8.0, 70.0, 1.0, Detail),
    cue!("tool.cut", "guns/safety", -6.0, 70.0, 0.8, Detail),
    cue!("tool.cut_ping", "impacts/ricochet", -9.0, 65.0, 0.8, Detail),
    cue!("hush.tone", "anomaly/well_drone", -30.0, 40.0, 0.0, Bed),
    cue!("echo.slap", "guns/rifle_shot", 0.0, 120.0, 20.0, Event, 60.0),
    cue!("animal.deer", "wildlife/deer_bark", -4.0, 85.0, 40.0, Event, 25.0),
    cue!("animal.chamois", "wildlife/chamois", -6.0, 85.0, 40.0, Event, 25.0),
    cue!("animal.dog", "wildlife/dog_bark", -4.0, 90.0, 30.0, Signal, 20.0),
    cue!("animal.yelp", "wildlife/dog_yelp", -6.0, 85.0, 20.0, Event, 15.0),
    cue!("animal.hooves", "footsteps/leaves", -6.0, 70.0, 20.0, Event, 10.0),
    cue!("animal.rats", "wildlife/rats", -14.0, 50.0, 3.0, Detail),
    cue!("animal.thump", "physical/body_hit", -6.0, 75.0, 15.0, Event, 8.0),
    cue!("flock.duck", "wildlife/duck", -8.0, 75.0, 20.0, Event, 12.0),
    cue!("flock.splash", "footsteps/water", -6.0, 70.0, 20.0, Event, 10.0),
    cue!("animal.bats", "wildlife/bats", -7.0, 40.0, 5.0, Detail),
    cue!("flock.heron", "wildlife/heron", -4.0, 80.0, 30.0, Event, 20.0),
    cue!("flock.chough", "wildlife/chough", -6.0, 80.0, 30.0, Event, 20.0),
    cue!("body.breath", "body/breath", -24.0, 50.0, 0.3, Detail, 1.0),
    cue!("water.lap", "ambience/lap", -18.0, 55.0, 8.0, Detail, 5.0),
    cue!("water.drip", "ambience/drip", -14.0, 50.0, 5.0, Detail, 4.0),
    // The underground sites (`audio_underground`): drains and basins,
    // firedamp, the canary, the cart, the roof; the cellars' heat, vents
    // and crane.
    cue!("ug.valve", "physical/door_creak", -9.5, 85.0, 1.0, Detail),
    cue!("ug.pump", "machines/clank", -7.0, 75.0, 1.0, Detail),
    cue!("ug.pump_water", "footsteps/water", -6.0, 65.0, 1.5, Detail),
    cue!("ug.gurgle", "ambience/creek", -10.0, 70.0, 6.0, Detail, 6.0),
    cue!("ug.drained", "footsteps/water", -5.0, 70.0, 6.0, Detail, 6.0),
    cue!("ug.cough", "body/cough", -13.0, 75.0, 0.3, Detail, 1.0),
    cue!("ug.wheeze", "body/breath", -22.0, 55.0, 0.3, Detail, 1.0),
    cue!("ug.canary", "wildlife/canary", -12.0, 75.0, 2.0, Signal),
    cue!("ug.whump", "explosions/grenade_heavy", 2.0, 150.0, 10.0, Event, 15.0),
    cue!("ug.timbers", "machines/groan", -13.0, 90.0, 10.0, Action, 8.0),
    cue!("fell.rope", "physical/door_creak", -14.0, 70.0, 5.0, Event, 4.0),
    cue!("fell.wheel", "machines/groan", -12.0, 80.0, 10.0, Event, 8.0),
    cue!("fell.sheet", "impacts/metal", -10.0, 95.0, 30.0, Event, 12.0),
    cue!("fell.car", "physical/door_close", -10.0, 85.0, 15.0, Event, 8.0),
    cue!("fell.wind", "ambience/wind_works", -16.0, 60.0, 4.0, Detail, 3.0),
    cue!("ug.cart", "machines/truck_idle", -7.0, 75.0, 3.0, Detail),
    cue!("ug.cart_clack", "footsteps/metal", -5.5, 80.0, 3.0, Detail),
    cue!("ug.cart_stop", "machines/clank", -3.5, 95.0, 3.0, Action),
    cue!("ug.roof", "ambience/rockfall", -7.0, 110.0, 10.0, Action, 10.0),
    cue!("ug.roof_crash", "impacts/concrete", 0.5, 115.0, 10.0, Action, 10.0),
    cue!("ug.dust", "ambience/rockfall", -15.0, 60.0, 8.0, Detail, 8.0),
    cue!("ug.sizzle", "anomaly/burner_roar", -4.0, 60.0, 0.5, Detail),
    cue!("ug.heat_breath", "body/breath", -23.0, 55.0, 0.3, Detail, 1.0),
    cue!("ug.damper", "impacts/metal", -7.0, 100.0, 3.0, Action),
    cue!("ug.flue", "physical/whoosh", -9.5, 75.0, 3.0, Detail),
    cue!("ug.heat_tick", "guns/casing", -8.0, 45.0, 4.0, Detail),
    cue!("ug.crane", "machines/hum", -11.5, 75.0, 6.0, Detail, 6.0),
    cue!("ug.chain", "physical/handle_metal", -12.5, 70.0, 6.0, Detail, 6.0),
    cue!("ug.crane_clank", "machines/clank", -3.5, 95.0, 6.0, Action, 6.0),
    cue!("ug.lever", "machines/clank", -6.0, 80.0, 0.6, Detail),
    cue!("ug.cage_bell", "physical/bell", -6.0, 85.0, 3.0, Signal, 6.0),
    cue!("ug.cage_strain", "machines/groan", -10.0, 85.0, 8.0, Action, 6.0),
    cue!("ug.cage_rope", "machines/hum", -5.0, 75.0, 8.0, Detail, 4.0),
    cue!("ug.cage_land", "machines/clank", -4.0, 95.0, 3.0, Action),
    cue!("ug.cage_gate", "physical/handle_metal", -8.0, 75.0, 1.0, Detail),
    cue!("ug.cage_crush", "impacts/metal", -7.0, 100.0, 1.0, Action),
    cue!("ug.cage_body", "physical/body_hit", -11.0, 85.0, 1.0, Action),
    cue!("ug.echo", "impacts/concrete", -8.5, 60.0, 1.0, Detail, 1.0),
    cue!("ug.klaxon", "alarm/klaxon", -10.0, 105.0, 5.0, Signal, 8.0),
    cue!("ug.seal_slam", "physical/door_close", -8.0, 100.0, 2.0, Action),
    cue!("ug.seal_bolt", "machines/clank", -4.0, 95.0, 2.0, Action),
    cue!("ug.mains", "machines/hum", -10.0, 75.0, 3.0, Detail, 3.0),
    cue!("ug.arc", "device/crackle", -10.0, 90.0, 2.0, Action, 3.0),
    cue!("ug.shock", "device/crackle", -4.0, 100.0, 0.5, Action),
    cue!("ug.jolt", "physical/body_hit", -10.0, 85.0, 0.5, Action),
    cue!("ug.isolator", "machines/clank", -4.0, 95.0, 1.0, Action),
    // Each kiosk door breathing out what's below (`stair_breath`): faint,
    // heard from a few metres, gone as you go down.
    cue!("ug.breath_hum", "machines/hum", -10.5, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_mains", "machines/hum", -12.2, 38.0, 3.0, Detail, 1.5),
    cue!("ug.breath_water", "ambience/creek", -10.3, 42.0, 3.0, Detail, 1.5),
    cue!("ug.breath_draught", "ambience/wind_works", -14.5, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_hiss", "machines/radio_static", -13.0, 30.0, 3.0, Detail, 1.5),
    cue!("ug.breath_buzz", "anomaly/snare_buzz", -0.7, 35.0, 3.0, Detail, 1.5),
    cue!("ug.breath_roar", "anomaly/burner_roar", 0.1, 45.0, 3.0, Detail, 1.5),
    cue!("ug.breath_wind", "ambience/wind_mountain", -14.4, 42.0, 3.0, Detail, 1.5),
    cue!("ug.breath_drip", "ambience/drip", -9.7, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_tick", "machines/detector_click", -9.3, 35.0, 3.0, Detail, 1.5),
    cue!("ug.breath_thud", "machines/clank", -6.4, 45.0, 3.0, Detail, 1.5),
    cue!("ug.breath_creak", "machines/groan", -22.0, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_metal", "guns/casing", -3.8, 38.0, 3.0, Detail, 1.5),
    cue!("ug.breath_rattle", "machines/clank", -6.9, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_servo", "machines/servo", -12.7, 40.0, 3.0, Detail, 1.5),
    cue!("ug.breath_rope", "physical/door_creak", -11.5, 40.0, 3.0, Detail, 1.5),
    cue!("grenade.pin", "guns/safety", -8.0, 60.0, 0.5, Detail),
    cue!("grenade.scrape", "physical/handle_metal", -15.0, 50.0, 0.5, Detail),
    cue!("grenade.spoon", "guns/casing", -8.0, 65.0, 1.0, Detail),
    cue!("grenade.bang", "explosions/grenade_sharp", 2.0, 160.0, 15.0, Event, 15.0),
    cue!("grenade.bang_heavy", "explosions/grenade_heavy", 1.5, 162.0, 15.0, Event, 15.0),
    cue!("mine.blast", "explosions/grenade_sharp", 2.0, 160.0, 15.0, Event, 15.0),
    cue!("mine.click", "physical/switch", -2.0, 75.0, 3.0, Signal, 2.0),
    cue!("mine.laid", "machines/clank", -4.0, 80.0, 10.0, Detail, 5.0),
    cue!("mine.safe", "physical/switch", -1.0, 65.0, 1.0, Detail),
    cue!("mine.cookoff", "device/crackle", -6.0, 90.0, 10.0, Action, 5.0),
    cue!("mine.flee", "machines/servo", -8.0, 75.0, 10.0, Detail, 5.0),
    cue!("mender.called", "machines/servo", -10.0, 70.0, 8.0, Detail, 4.0),
    cue!("mender.weld", "device/crackle", -5.0, 85.0, 10.0, Action, 5.0),
    cue!("mender.mended", "machines/clank", -4.0, 80.0, 8.0, Detail, 4.0),
    cue!("plough.horn", "voice/heavy/horn", -14.0, 115.0, 150.0, Signal, 80.0),
    cue!("plough.bury", "impacts/dirt", 2.0, 85.0, 10.0, Action, 5.0),
    cue!("plough.jam", "machines/groan", -8.0, 85.0, 10.0, Action, 6.0),
    cue!("spotter.fix", "machines/drone_lock", -29.0, 80.0, 25.0, Signal, 15.0),
    cue!("spotter.call", "machines/siren", -6.0, 110.0, 100.0, Signal, 40.0),
    cue!("spotter.loose", "physical/whoosh", -8.0, 75.0, 10.0, Detail, 5.0),
    cue!("spotter.down", "impacts/metal", -4.0, 95.0, 15.0, Action, 8.0),
    cue!("sorter.windup", "machines/servo", -8.0, 80.0, 10.0, Signal, 6.0),
    cue!("sorter.throw", "physical/whoosh", -6.0, 80.0, 10.0, Action, 5.0),
    cue!("hound.crouch", "machines/servo", -8.0, 80.0, 10.0, Signal, 6.0),
    cue!("hound.bite", "impacts/metal", -4.0, 95.0, 2.0, Action),
    cue!("robot.wake", "machines/servo", -8.0, 80.0, 10.0, Signal, 6.0),
    cue!("device.notice", "device/notice", -14.0, 80.0, 15.0, Signal, 8.0),
    cue!("device.hunt", "device/notice_hard", -12.0, 85.0, 15.0, Signal, 8.0),
    cue!("wreck.thud", "impacts/metal", -2.0, 100.0, 20.0, Action, 10.0),
    cue!("cell.arc", "device/crackle", -9.0, 95.0, 1.0, Action),
    cue!("you.hurt", "physical/body_hit", -8.0, 85.0, 0.0, Detail),
    cue!("you.gasp", "body/breath", -22.0, 55.0, 0.0, Detail),
    cue!("you.down", "physical/body_hit", -9.0, 95.0, 0.0, Action),
    // Bullets landing.
    cue!("hit.impact", "impacts/metal", -2.0, 95.0, 15.0, Action, 8.0),
    cue!("hit.blow", "impacts/metal", -4.0, 95.0, 1.0, Action),
    cue!("hit.blow_ring", "machines/clank", -8.0, 90.0, 1.0, Action),
    cue!("hit.spall", "physical/drop_metal", -8.0, 85.0, 5.0, Action, 4.0),
    cue!("fight.torn", "physical/door_creak", -8.0, 85.0, 2.0, Action),
    cue!("fight.lurch", "physical/whoosh", -8.0, 80.0, 2.0, Detail),
    cue!("aid.wrap", "physical/bandage", -16.0, 50.0, 0.4, Detail),
    cue!("aid.strap", "physical/velcro", -14.0, 55.0, 0.4, Detail),
    cue!("aid.cinch", "physical/velcro", -17.0, 60.0, 0.4, Detail),
    cue!("aid.cap", "physical/handle_plastic", -9.0, 45.0, 0.3, Detail),
    cue!("aid.cap_off", "physical/handle_plastic", -12.0, 50.0, 0.3, Detail),
    cue!("aid.wet", "footsteps/mud", -16.0, 50.0, 0.3, Detail),
    cue!("fight.shove", "impacts/metal", -6.0, 90.0, 1.0, Action),
    cue!("fight.stagger", "machines/servo", -9.0, 75.0, 3.0, Detail, 3.0),
    cue!("fight.strain", "machines/servo", -10.0, 75.0, 1.0, Detail),
    cue!("fight.kick", "machines/clank", -3.0, 95.0, 2.0, Action),
    cue!("crawler.step", "footsteps/metal", -2.0, 70.0, 15.0, Signal, 4.0),
    cue!("crawler.charge", "device/crackle", -6.0, 85.0, 15.0, Signal, 6.0),
    cue!("crawler.zap", "device/crackle", -4.0, 95.0, 15.0, Action, 6.0),
    cue!("hit.glass", "physical/glass_break", -1.0, 90.0, 15.0, Action, 8.0),
    cue!("hit.water", "footsteps/water", 3.0, 85.0, 15.0, Action, 8.0),
    cue!("hit.ricochet", "impacts/ricochet", -1.0, 90.0, 15.0, Action, 8.0),
    cue!("hit.glance", "impacts/ricochet", -2.5, 100.0, 1.0, Signal),
    cue!("hit.body", "physical/body_hit", -8.0, 90.0, 0.5, Signal),
    // Machines.
    cue!("robot.shot", "guns/pistol_shot", -3.0, 120.0, 30.0, Action, 20.0),
    cue!("drone.rotor", "machines/drone_rotor", -4.0, 75.0, 20.0, Signal),
    cue!("drone.lock", "machines/drone_lock", -28.0, 80.0, 20.0, Signal, 25.0),
    cue!("lock.tick", "physical/switch", -8.0, 80.0, 20.0, Signal, 25.0),
    cue!("walker.step", "footsteps/metal", -2.5, 85.0, 25.0, Signal, 16.0),
    cue!("convoy.loaded", "machines/clank", -2.0, 90.0, 40.0, Event, 15.0),
    cue!("convoy.delivered", "machines/groan", -2.0, 90.0, 60.0, Event, 15.0),
    cue!("dirigible.engine", "machines/drone_rotor", -12.0, 80.0, 60.0, Signal, 20.0),
    cue!("voice.idle", "voice/walker/idle", -10.0, 55.0, 15.0, Detail, 6.0),
    cue!("voice.noticed", "voice/walker/noticed", -8.0, 72.0, 15.0, Signal, 10.0),
    cue!("voice.investigating", "voice/walker/investigating", -10.0, 68.0, 15.0, Signal, 8.0),
    cue!("voice.searching", "voice/walker/searching", -11.0, 65.0, 15.0, Signal, 8.0),
    cue!("voice.engaging", "voice/walker/engaging", -9.0, 78.0, 15.0, Signal, 12.0),
    cue!("voice.lost", "voice/walker/lost", -11.0, 65.0, 15.0, Signal, 8.0),
    cue!("voice.calling", "voice/walker/calling", -8.5, 85.0, 30.0, Signal, 18.0),
    cue!("voice.answering", "voice/walker/answering", -8.0, 85.0, 30.0, Signal, 18.0),
    cue!("voice.hurt", "voice/walker/hurt", -8.0, 75.0, 15.0, Signal, 8.0),
    cue!("voice.flanking", "voice/walker/flanking", -10.0, 70.0, 15.0, Signal, 10.0),
    cue!("voice.retreating", "voice/walker/retreating", -10.0, 70.0, 15.0, Signal, 10.0),
    cue!("voice.dying", "voice/walker/dying", -15.0, 80.0, 15.0, Signal, 10.0),
    cue!("heavy.step", "explosions/grenade_heavy", 0.0, 110.0, 40.0, Signal, 48.0),
    cue!("heavy.foot", "footsteps/metal", -2.0, 90.0, 25.0, Signal, 16.0),
    cue!("heavy.raise", "machines/servo", -7.5, 85.0, 20.0, Signal, 15.0),
    cue!("heavy.hiss", "physical/whoosh", -6.0, 80.0, 20.0, Signal, 12.0),
    cue!("heavy.slam", "explosions/grenade_heavy", 0.0, 115.0, 20.0, Action, 25.0),
    cue!("heavy.clank", "machines/clank", -2.0, 95.0, 20.0, Action, 15.0),
    cue!("heavy.tear", "physical/door_creak", -3.5, 90.0, 20.0, Action, 12.0),
    cue!("heavy.strain", "machines/groan", -11.5, 85.0, 20.0, Signal, 15.0),
    cue!("heavy.bolt", "impacts/ricochet", -4.0, 90.0, 15.0, Action, 10.0),
    cue!("heavy.loose", "machines/clank", -3.0, 75.0, 15.0, Detail, 8.0),
    cue!("heavy.plate", "physical/drop_metal", 0.0, 95.0, 15.0, Action, 12.0),
    cue!("heavy.vent", "physical/whoosh", -4.0, 85.0, 15.0, Signal, 12.0),
    cue!("heavy.stall", "anomaly/burner_roar", -6.0, 95.0, 20.0, Signal, 20.0),
    cue!("heavy.cell", "physical/switch", 0.0, 60.0, 10.0, Detail),
    cue!("heavy.idle", "machines/hum", -10.0, 70.0, 20.0, Signal, 12.0),
    cue!("heavy.horn", "voice/heavy/horn", -10.0, 120.0, 300.0, Signal, 200.0),
    cue!("sentry.chirp", "machines/drone_lock", -30.0, 85.0, 20.0, Signal, 25.0),
    cue!("sentry.turn", "machines/servo", -10.0, 70.0, 15.0, Signal, 10.0),
    cue!("walker.aim", "machines/clank", -6.0, 85.0, 25.0, Signal, 10.0),
    cue!("walker.aim_lock", "machines/servo", -9.5, 75.0, 25.0, Signal, 15.0),
    cue!("harpoon.windup", "machines/servo", -9.5, 75.0, 15.0, Signal, 12.0),
    cue!("walker.servo", "machines/servo", -9.5, 70.0, 25.0, Detail, 10.0),
    cue!("harpoon.fire", "machines/clank", 1.0, 100.0, 15.0, Action, 7.0),
    cue!("harpoon.hit", "impacts/metal", -6.0, 100.0, 1.0, Action),
    cue!("pylon.work", "machines/servo", -9.5, 75.0, 30.0, Detail, 15.0),
    cue!("robot.part", "machines/clank", 1.0, 100.0, 15.0, Action, 10.0),
    cue!("robot.glass", "physical/glass_break", -1.0, 90.0, 15.0, Action, 8.0),
    cue!("robot.wreck", "machines/groan", -8.5, 95.0, 15.0, Action, 8.0),
    cue!("camera_drone.on", "physical/switch", -2.5, 55.0, 0.5, Detail),
    cue!("camera_drone.land", "physical/handle_plastic", -7.0, 60.0, 1.5, Detail),
    cue!("camera_drone.spot", "physical/switch", -2.5, 55.0, 0.0, Detail),
    cue!("zip.rig", "impacts/metal", -8.5, 80.0, 6.0, Detail),
    cue!("zip.pack", "physical/cloth", -12.0, 50.0, 3.0, Detail),
    cue!("camera_drone", "machines/drone_rotor", -16.0, 60.0, 5.0, Detail),
    cue!("zipline", "machines/hum", -10.0, 65.0, 0.6, Detail),
    // The radio.
    cue!("radio.squelch", "machines/radio_squelch", -8.5, 75.0, 0.6, Signal),
    cue!("radio.static", "machines/radio_static", -16.0, 65.0, 0.6, Detail),
    cue!("chatter.squelch", "machines/radio_squelch", -8.5, 72.0, 0.6, Signal),
    cue!("chatter.static", "machines/radio_static", -12.0, 65.0, 0.6, Signal),
    cue!("chatter.voice", "machines/robot_voice", -20.0, 60.0, 0.6, Signal),
    // Underground power.
    cue!("power.breaker", "machines/clank", -6.0, 90.0, 3.0, Action),
    cue!("power.pull", "physical/whoosh", -10.0, 70.0, 2.0, Detail),
    cue!("power.generator", "machines/generator", -24.0, 85.0, 8.0, Signal, 6.0),
    // The truck.
    cue!("truck.arrive", "machines/truck_arrive", -6.0, 85.0, 30.0, Signal, 15.0),
    cue!("truck.idle", "machines/truck_idle", -9.0, 75.0, 15.0, Signal, 10.0),
    cue!("truck.depart", "machines/truck_depart", -10.0, 85.0, 10.0, Event, 15.0),
    cue!("truck.bell", "physical/bell", -7.5, 85.0, 1.0, Action),
    // Doors.
    cue!("door.open", "physical/door_open", -8.0, 65.0, 3.0, Detail),
    cue!("door.close", "physical/door_close", -9.0, 75.0, 3.0, Detail),
    cue!("door.locked", "machines/clank", -10.0, 70.0, 1.0, Detail),
    cue!("door.wedged", "impacts/wood", -10.0, 70.0, 1.0, Detail),
    cue!("door.blow", "impacts/wood", 1.0, 100.0, 20.0, Signal, 30.0),
    cue!("door.broken", "physical/door_close", -8.5, 100.0, 20.0, Signal, 14.0),
    // Anomalies and the Surge.
    cue!("anomaly.lift", "anomaly/lift_hum", -6.0, 65.0, 8.0, Signal),
    cue!("anomaly.burner", "anomaly/burner_roar", 0.0, 75.0, 8.0, Signal),
    cue!("anomaly.snare", "anomaly/snare_buzz", -2.0, 60.0, 8.0, Signal),
    cue!("anomaly.well", "anomaly/well_drone", -6.0, 70.0, 8.0, Signal),
    cue!("anomaly.mirror", "anomaly/mirror_shimmer", -6.0, 60.0, 8.0, Signal),
    cue!("anomaly.frost", "anomaly/frost", -26.0, 50.0, 4.0, Signal, 2.0),
    cue!("frost.gasp", "body/breath", -22.0, 55.0, 0.0, Detail),
    cue!("frost.bite", "physical/ice_crack", -8.0, 80.0, 0.0, Action),
    cue!("anomaly.blink", "anomaly/mirror_blink", -9.5, 95.0, 8.0, Action, 7.0),
    cue!("anomaly.blink_air", "physical/whoosh", -10.0, 70.0, 8.0, Detail),
    cue!("anomaly.arc", "anomaly/surge_crack", 2.5, 105.0, 8.0, Action),
    cue!("anomaly.reveal", "anomaly/reveal", -24.0, 65.0, 8.0, Signal),
    cue!("anomaly.crush", "machines/groan", -8.5, 95.0, 8.0, Action, 7.0),
    cue!("detector.click", "machines/detector_click", -8.0, 60.0, 0.4, Signal),
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
    cue!("water.scour", "ambience/creek", -10.0, 80.0, 20.0, Bed, 25.0),
    cue!("insects", "ambience/insects_night", -27.0, 45.0, 0.0, Bed),
    cue!("rustle", "ambience/rustle_leaves", -30.0, 45.0, 0.0, Bed),
    cue!("birds", "ambience/crow", -2.0, 70.0, 45.0, Event),
    cue!("valley.distant", "ambience/rockfall", 2.0, 90.0, 110.0, Event),
    cue!("works.groan", "machines/groan", -2.0, 85.0, 80.0, Event),
    cue!("works.clank", "machines/clank", -4.0, 85.0, 80.0, Event),
    cue!("thunder", "ambience/thunder_distant", -14.0, 75.0, 0.0, Event),
    cue!("thunder.crack", "explosions/grenade_sharp", -1.0, 120.0, 120.0, Event, 120.0),
    cue!("thunder.roll", "ambience/thunder_distant", -10.5, 110.0, 120.0, Event, 120.0),
    cue!("thunder.surge", "ambience/thunder_distant", -11.0, 80.0, 0.0, Event),
    // Music and broadcasts, synthesised (`radio`): a cassette player's
    // little speaker, a radio's, a gramophone's horn, a piano in a room.
    cue!("tape.music", "music/tape", -17.0, 72.0, 0.6, Detail, 2.0),
    cue!("tape.motor", "music/motor", -16.0, 50.0, 0.5, Detail),
    cue!("radio.station", "music/station", -16.5, 70.0, 0.6, Detail, 2.0),
    cue!("radio.hiss", "music/static", -15.0, 62.0, 0.6, Detail, 2.0),
    cue!("gramophone", "music/gramophone", -10.0, 75.0, 1.5, Detail, 3.0),
    cue!("piano.note", "music/piano", -8.0, 85.0, 1.0, Detail, 4.0),
    cue!("organ", "music/organ", -8.0, 90.0, 6.0, Detail, 10.0),
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

/// A cue's name as the table holds it (for components that keep it).
pub fn static_name(name: &str) -> &'static str {
    find(name).map_or("step", |c| c.name)
}

/// The head-related node a cue plays through in the headset: the same
/// reach and muffle as `spatial`.
pub fn hrtf(name: &str) -> bevy_seedling::prelude::HrtfNode {
    use bevy_seedling::firewheel::dsp::distance_attenuation::DistanceAttenuation;
    let reach = find(name).map_or(5.0, |c| c.reach);
    bevy_seedling::prelude::HrtfNode {
        distance_attenuation: DistanceAttenuation {
            reference_distance: reach,
            max_muffle_distance: MUFFLE_FULL,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A cue's role, if it's in the mix.
pub fn role(name: &str) -> Option<Role> {
    MIX.iter().find(|c| c.name == name).map(|c| c.role)
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
        // Robots' words are synthesised (`robot_voice`), not files.
        let mut voices: Vec<String> = crate::robot_voice::Utterance::ALL
            .iter()
            .map(|u| crate::robot_voice::set_name(crate::robot_voice::VoiceKind::Walker, *u))
            .collect();
        voices.push("voice/heavy/horn".into());
        voices.push("device/stone_ping".into());
        voices.push("device/crackle".into());
        voices.extend(
            crate::synth_foley::STAND_INS
                .iter()
                .map(|s| s.set.to_string()),
        );
        for c in MIX {
            assert!(
                index.get(c.set).is_some()
                    || voices.iter().any(|v| v == c.set)
                    || crate::radio::sound::is_synth_set(c.set),
                "{}: no set {}",
                c.name,
                c.set
            );
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
            "mp133",
            "svd",
            "toz8",
            "flare_pistol",
            "crossbow",
            "sks",
            "rpk74",
            "nagant",
            "ppsh",
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
