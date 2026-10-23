//! Firearm mechanisms as plain state machines.
//!
//! A [`Firearm`] knows nothing about hands or meshes. The game drives it the
//! way a VR player drives a real gun: by moving the charging handle along its
//! travel ([`Firearm::set_bolt_travel`]), lifting a bolt handle, breaking a
//! shotgun open, seating and dropping magazines, pressing and releasing the
//! trigger. Each operation returns the [`Event`]s it caused (a round ejected,
//! a shot fired, a dry click) for the game to turn into physics, sound and
//! damage.
//!
//! Because the handle position is continuous, VR hands, the flat-screen
//! macros and the simulation bots all drive the same code.

use crate::ammo::{Load, Loads};
use serde::{Deserialize, Serialize};

/// How the gun cycles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    /// Self-loading: firing ejects the case and chambers the next round.
    SemiAuto,
    /// Manually operated turn-bolt: lift, pull, push, lower.
    BoltAction,
    /// Hinged barrels, loaded by hand, one round per barrel.
    BreakAction,
    /// A cylinder of `internal_capacity` chambers behind a loading gate
    /// (a Nagant): rounds go in and out one at a time at the gate; cocking
    /// the hammer (by thumb, or the long double-action pull) turns it.
    Revolver,
    /// A pump (slide) action: shells thumbed into a tube magazine through
    /// the loading port; the forend pumped back throws out what's in the
    /// chamber and lets the next shell onto the carrier, pumped forward
    /// chambers it. A cocked hammer locks the action shut until the shot
    /// (or the action release).
    Pump,
    /// A crossbow: the string drawn back by hand catches on the latch;
    /// a bolt laid in the groove; the trigger looses it. Nothing to
    /// eject, nothing to cycle.
    Crossbow,
}

/// Fire-selector positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FireMode {
    Safe,
    Semi,
    Auto,
}

/// Static description of a gun model, authored in `items.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FirearmDef {
    pub action: ActionKind,
    pub cartridge: String,
    /// Detachable magazines of this family fit. `None` for internal feeds.
    #[serde(default)]
    pub magazine_family: Option<String>,
    /// Capacity of a fixed internal magazine (bolt actions).
    #[serde(default)]
    pub internal_capacity: u32,
    /// Number of barrels (and chambers). 1 except for doubles.
    #[serde(default = "one")]
    pub barrels: u32,
    /// Selector positions, in lever order.
    pub modes: Vec<FireMode>,
    /// Cyclic rate for automatic fire, rounds per minute.
    #[serde(default = "default_rpm")]
    pub rpm: f32,
    /// Slide or bolt locks back on an empty magazine.
    #[serde(default)]
    pub last_round_hold_open: bool,
    /// Trigger can cock and drop the hammer itself.
    #[serde(default)]
    pub double_action: bool,
    /// Fires from an open bolt (a PPSh): at rest the bolt is back on the
    /// sear; the trigger lets it run forward, and it fires as it closes.
    #[serde(default)]
    pub open_bolt: bool,
    /// A revolver's loading gate is this many chambers round from the one
    /// under the hammer, counting the way the cylinder turns (the Nagant's
    /// is 5: two back, on the right).
    #[serde(default)]
    pub gate_offset: u32,
    /// Metres per second.
    pub muzzle_velocity: f32,
    /// Base damage of one projectile (shot counts multiply this).
    pub damage: f32,
    /// Projectiles per shot (pellets for shotguns).
    #[serde(default = "one")]
    pub projectiles: u32,
    /// Spread in minutes of angle at full condition.
    pub spread_moa: f32,
    /// Recoil impulse in newton-seconds-ish; the game scales it.
    pub recoil: f32,
    /// Condition lost per shot at zero fouling.
    #[serde(default = "default_wear")]
    pub wear_per_shot: f32,
    /// Fouling added per shot.
    #[serde(default = "default_fouling")]
    pub fouling_per_shot: f32,
    /// A free-floating firing pin (an SKS): worn and dirty, it can fire the
    /// round the bolt slams home on.
    #[serde(default)]
    pub floating_pin: bool,
    /// How far robots hear a shot from it, against what its calibre would
    /// say (a .22 rifle's crack is a fraction of a pistol's).
    #[serde(default = "unit")]
    pub report: f32,
    /// Rimfire rounds misfire more: lift the bolt and drop it to strike
    /// again.
    #[serde(default)]
    pub rimfire: bool,
    /// How its kick feels beside another gun's of the same momentum: the
    /// game's handling reads it (`weapons::feel`).
    #[serde(default)]
    pub kick: Kick,
}

/// A gun's own kick, over what its momentum and shape say: the game works
/// out the climb from the bore's height over where the gun bears, and these
/// scale it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kick {
    /// The muzzle's climb, against what its shape gives (a brake or a
    /// compensator under 1; under 0 it dips, as a crossbow's limbs throw
    /// it).
    pub climb: f32,
    /// The push back along the bore, likewise (under 0, forward).
    pub push: f32,
    /// Which way the muzzle walks, as a share of the shot's yaw: 1 always
    /// left, -1 always right, 0 either way.
    pub walk: f32,
    /// How long the shot's shove lasts, seconds: 0 a rifle's snap, a
    /// shotgun's or a flare's a longer push.
    pub shove: f32,
}

impl Default for Kick {
    fn default() -> Self {
        Self {
            climb: 1.0,
            push: 1.0,
            walk: 0.0,
            shove: 0.0,
        }
    }
}

/// The malfunction chance per unit of grime, and of stiffness.
pub const GRIME_JAMS: f32 = 0.06;
pub const STIFF_JAMS: f32 = 0.04;
/// A shot's fouling grows by this share per unit of grime.
pub const GRIME_FOULS: f32 = 2.0;
/// A stiff action cycles this much slower at the worst.
pub const STIFF_SLOWS: f32 = 0.6;

const fn one() -> u32 {
    1
}
const fn unit() -> f32 {
    1.0
}
const fn default_rpm() -> f32 {
    600.0
}
const fn default_wear() -> f32 {
    0.0008
}
const fn default_fouling() -> f32 {
    0.004
}

/// A cartridge sitting in a chamber or a feed path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Round {
    Live,
    Spent,
}

/// What can go wrong when a gun is worn and dirty. Each one has its own
/// cure, as on the real thing ([`Malfunction::remedy`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Malfunction {
    /// Stovepipe: the spent case caught upright in the port, the bolt
    /// riding on it. Rack it.
    FailureToEject,
    /// The magazine slipped in the well and the bolt closed on an empty
    /// chamber: the trigger clicks. Tap the magazine home, then rack.
    FailureToFeed,
    /// The spent case stayed in the chamber and the next round rammed into
    /// it. The magazine holds both in place: drop it, rack, reload.
    DoubleFeed,
    /// A stiff bolt worked timidly came back short and went home again:
    /// the spent case still in, the action half-done. Work it fully.
    ShortStroke,
}

impl Malfunction {
    /// What clears it, in the words a range officer would use.
    pub fn remedy(self) -> &'static str {
        match self {
            Self::FailureToEject => "rack",
            Self::FailureToFeed => "tap, rack",
            Self::DoubleFeed => "drop the mag, rack",
            Self::ShortStroke => "work it fully",
        }
    }
}

/// A detachable magazine as an inventory object.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Magazine {
    pub family: String,
    pub capacity: u32,
    pub rounds: u32,
    /// What its rounds are loaded with, top last.
    #[serde(default)]
    pub loads: Loads,
}

/// Something that happened, for the game to show and play.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A shot left barrel `barrel`.
    Fired {
        barrel: u32,
    },
    /// The hammer fell on nothing (or on a dud).
    DryFire,
    /// The primer was struck and nothing happened.
    Dud,
    /// A cartridge was thrown clear of the gun.
    Ejected(Round),
    /// A round went into a chamber.
    Chambered,
    /// The hammer or striker was cocked.
    Cocked,
    /// The bolt or slide locked open.
    BoltLocked,
    /// The bolt or slide closed (went fully forward).
    BoltClosed,
    MagazineInserted,
    /// A magazine pushed in too gently: it hangs on the catch, in sight
    /// but not seated ([`Firearm::half_seat_magazine`]).
    MagazineHalfSeated,
    /// A half-seated magazine shaken out of the well by the action.
    MagazineDropped,
    MagazineRemoved,
    /// A round was pushed into an internal magazine or a breech.
    RoundLoaded,
    Malfunctioned(Malfunction),
    ModeChanged(FireMode),
    /// A squib: the primer popped, too little powder, and the bullet
    /// stopped in barrel `barrel`. The action doesn't cycle.
    Squib {
        barrel: u32,
    },
    /// A round fired into a barrel a squib had blocked: it bulges.
    Bulged {
        barrel: u32,
    },
    /// A rod pushed down the bore knocked the squib's bullet out.
    Rodded,
    /// A self-loader's carrier slamming home after the shot (the game
    /// times it from the cycle and sends it).
    CarrierHome,
    /// An open bolt slamming home on the round it fires (likewise).
    OpenBoltSlam,
    /// A dirty bolt stuck shut after the shot: slap the handle up.
    BoltStuck,
    BoltFreed,
    /// The bolt slammed home and the floating pin fired the round (a
    /// `Fired` follows).
    SlamFire,
    /// The hammer fell and the round didn't go: yet. A hang-fire: it goes
    /// off on its own a moment later (a `Fired` follows), so keep the
    /// muzzle where it was and the action shut.
    HangFire,
    /// A hang-fire went off with the action open: the case burst outside
    /// the chamber. It hurts the hand and the gun.
    OutOfBattery,
}

/// A round hanging fire in a chamber: it goes in `left` seconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hang {
    pub left: f32,
    pub barrel: u32,
    pub chamber: usize,
    pub load: Load,
}

/// What a round's primer and powder add to the misfire chance: surplus,
/// damp, and (per unit of cold) a cold-soaked round; and the same for a
/// hang-fire. A hang-fire goes between [`HANG_MIN`] and [`HANG_MAX`]
/// seconds after the hammer falls.
pub const SURPLUS_DUD: f32 = 0.03;
pub const DAMP_DUD: f32 = 0.15;
pub const COLD_DUD: f32 = 0.04;
pub const SURPLUS_HANG: f32 = 0.03;
pub const DAMP_HANG: f32 = 0.08;
pub const COLD_HANG: f32 = 0.05;
pub const HANG_MIN: f32 = 0.3;
pub const HANG_MAX: f32 = 1.0;
/// What a round going off out of battery costs the gun.
pub const OUT_OF_BATTERY_WEAR: f32 = 0.15;

/// Bolt travel past which whatever is in the chamber is thrown out.
const EJECT_AT: f32 = 0.85;
/// Bolt travel past which the hammer is cocked.
const COCK_AT: f32 = 0.5;
/// Bolt travel at which, coming forward, the bolt strips the top round.
const STRIP_AT: f32 = 0.6;
/// Bolt travel below which the bolt counts as closed.
const CLOSED_AT: f32 = 0.03;
/// Bolt-handle lift at or below which a turn-bolt is locked down.
const HANDLE_DOWN_AT: f32 = 0.05;
/// Where a stovepipe's case holds the bolt.
const STOVEPIPE_AT: f32 = 0.2;
/// Pump travel past which the tube lets the next shell onto the carrier
/// (short of [`EJECT_AT`]: a short stroke releases a shell without
/// throwing out the case, and the two jam).
const PUMP_RELEASE: f32 = 0.7;
/// What loosing a crossbow with no bolt on it (a dry loose: the prod's
/// whole spring goes into its own limbs and the string) takes off its
/// condition.
pub const DRY_LOOSE: f32 = 0.04;
/// A crossbow's string let go short of the latch slaps home: a little wear.
pub const STRING_SLAP: f32 = 0.005;
/// A paper-cased 26 mm signal cartridge's extra chance of a misfire (the
/// card soaks up the damp): break the gun, swap it.
pub const PAPER_DUD: f32 = 0.06;
/// A rimfire round's extra chance of a misfire.
pub const RIMFIRE_DUD: f32 = 0.03;
/// Where a double feed wedges the bolt: it won't come further forward.
const DOUBLE_FEED_AT: f32 = 0.4;
/// A re-packed round's chance of a squib, and of a dud on top of the gun's.
pub const SQUIB_CHANCE: f32 = 0.04;
pub const REPACK_DUD: f32 = 0.02;
/// What a bulged barrel leaves of the gun's condition.
pub const BULGE_KEEPS: f32 = 0.3;

/// Below this condition an action starts to drag ([`Firearm::action_drag`]),
/// up to `DRY_DRAG` at none; full fouling adds `FOUL_DRAG`.
pub const DRY_FROM: f32 = 0.6;
pub const DRY_DRAG: f32 = 0.5;
pub const FOUL_DRAG: f32 = 0.25;

/// One gun's live state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Firearm {
    pub def: FirearmDef,
    /// One per barrel.
    pub chambers: Vec<Option<Round>>,
    /// One per barrel.
    pub hammers_cocked: Vec<bool>,
    /// 0 = closed, 1 = fully to the rear. For break actions: 0 shut, 1 open.
    pub bolt_travel: f32,
    /// Turn-bolt handle lift, 0 = down/locked, 1 = up. Unused otherwise.
    pub handle_lift: f32,
    /// Held open by the slide stop.
    pub locked_back: bool,
    /// The bolt cleared the ejection port on this stroke and is coming
    /// back forward: the next pass of [`STRIP_AT`] picks up a round.
    stroke_cleared: bool,
    /// A round stripped from the magazine, riding the bolt face forward.
    in_transit: Option<Round>,
    pub magazine: Option<Magazine>,
    /// Rounds in a fixed internal magazine.
    pub internal_rounds: u32,
    /// What those rounds are loaded with, top last.
    #[serde(default)]
    pub internal_loads: Loads,
    /// What each live round in `chambers` is loaded with (by index).
    #[serde(default)]
    pub chamber_loads: Vec<Load>,
    /// The round riding the bolt face's load.
    #[serde(default)]
    transit_load: Load,
    /// The round last taken from the feed.
    #[serde(default)]
    fed: Load,
    /// The load of the round last fired (for the shot it makes).
    #[serde(default)]
    pub last_fired: Load,
    /// Loads of live rounds thrown clear, in the order their
    /// [`Event::Ejected`] went out (drained by the game).
    #[serde(default)]
    pub ejected: Vec<Load>,
    /// A squib's bullet stuck in this barrel.
    #[serde(default)]
    pub squib: Option<u32>,
    /// A dirty turn-bolt stuck shut: its handle won't lift until slapped.
    #[serde(default)]
    pub bolt_stuck: bool,
    /// The bolt slammed home on a round the floating pin will fire next
    /// tick.
    #[serde(default)]
    slam_pending: bool,
    /// The gun's own dice, for a bolt let go by hand (which has none
    /// passed in): kept apart so a gun that can't slam-fire never draws
    /// from anyone's rolls.
    #[serde(default)]
    pub fault_dice: u64,
    pub mode: usize,
    pub malfunction: Option<Malfunction>,
    /// The magazine has slipped down in the well (a failure to feed): the
    /// bolt can't strip from it until it's tapped home.
    #[serde(default)]
    pub mag_unseated: bool,
    /// The magazine went in too gently and hangs on its catch: it doesn't
    /// feed, and the jolt of a rack drops it out (a tap seats it).
    #[serde(default)]
    pub mag_half: bool,
    /// A revolver: the chamber under the hammer (and at the gate).
    #[serde(default)]
    pub cylinder: usize,
    /// A revolver's loading gate is open: the cylinder turns by hand and
    /// the hammer is disconnected.
    #[serde(default)]
    pub gate_open: bool,
    /// The ejector rod's stroke, 1 as it pushes a case out, easing to 0.
    #[serde(default)]
    pub rod: f32,
    /// A pump's action release is pressed: the forend comes back on a
    /// cocked hammer (until the action closes again).
    #[serde(default)]
    pub action_released: bool,
    /// 0..=1. Lower is worse: more malfunctions, more spread.
    pub condition: f32,
    /// 0..=1. Dirt from shooting; cleaning removes it.
    pub fouling: f32,
    /// 0..=1. Dirt from outside in the action: rain and damp, ash, snow
    /// melting in it. It jams the action as fouling does, and a shot
    /// fouls a grimy gun faster. A field wipe takes it off ([`Self::wipe`]).
    #[serde(default)]
    pub grime: f32,
    /// 0..=1. Its oil stiffened by cold: the action drags, cycles slower
    /// and jams more. Warmth undoes it.
    #[serde(default)]
    pub stiff: f32,
    /// 0..=1. How cold its rounds are (they're at the gun's temperature):
    /// a cold primer is a lazy one.
    #[serde(default)]
    pub ammo_cold: f32,
    /// A round hanging fire.
    #[serde(default)]
    pub hang: Option<Hang>,
    /// What it's been made into at the vice (`gunsmith`).
    #[serde(default)]
    pub build: crate::gunsmith::Build,
    trigger_down: bool,
    /// Semi-auto needs the trigger released before the next shot.
    trigger_reset: bool,
    /// Seconds until the next automatic shot may fire.
    cooldown: f32,
    /// Which barrel fires next on a double.
    next_barrel: u32,
}

impl Firearm {
    /// A gun as found: closed, empty, hammer down, on its first selector
    /// position.
    pub fn new(def: FirearmDef, condition: f32) -> Self {
        let n = def.barrels.max(1) as usize;
        let chambers = if def.action == ActionKind::Revolver {
            def.internal_capacity.max(1) as usize
        } else {
            n
        };
        Self {
            def,
            chambers: vec![None; chambers],
            hammers_cocked: vec![false; n],
            bolt_travel: 0.0,
            handle_lift: 0.0,
            locked_back: false,
            stroke_cleared: false,
            in_transit: None,
            magazine: None,
            internal_rounds: 0,
            internal_loads: Loads::default(),
            chamber_loads: vec![Load::Standard; chambers],
            transit_load: Load::Standard,
            fed: Load::Standard,
            last_fired: Load::Standard,
            ejected: Vec::new(),
            squib: None,
            bolt_stuck: false,
            slam_pending: false,
            fault_dice: 0,
            mode: 0,
            malfunction: None,
            mag_unseated: false,
            mag_half: false,
            cylinder: 0,
            gate_open: false,
            rod: 0.0,
            action_released: false,
            condition: condition.clamp(0.0, 1.0),
            fouling: 0.0,
            grime: 0.0,
            stiff: 0.0,
            ammo_cold: 0.0,
            hang: None,
            build: crate::gunsmith::Build::default(),
            trigger_down: false,
            trigger_reset: true,
            cooldown: 0.0,
            next_barrel: 0,
        }
    }

    pub fn fire_mode(&self) -> FireMode {
        self.def
            .modes
            .get(self.mode)
            .copied()
            .unwrap_or(FireMode::Semi)
    }

    /// Steps the selector to the next position (wrapping) and reports it.
    pub fn cycle_mode(&mut self) -> Vec<Event> {
        if self.def.modes.is_empty() {
            return Vec::new();
        }
        self.mode = (self.mode + 1) % self.def.modes.len();
        vec![Event::ModeChanged(self.fire_mode())]
    }

    pub fn is_closed(&self) -> bool {
        let shut = self.bolt_travel <= CLOSED_AT && !self.locked_back;
        match self.def.action {
            ActionKind::BoltAction => shut && self.handle_lift <= HANDLE_DOWN_AT,
            ActionKind::Revolver => !self.gate_open,
            _ => shut,
        }
    }

    /// Ready to fire: closed, or for an open bolt, back on the sear.
    pub fn is_ready(&self) -> bool {
        if self.def.open_bolt || self.def.action == ActionKind::Crossbow {
            self.locked_back
        } else {
            self.is_closed()
        }
    }

    /// Rounds available to feed, wherever they are.
    pub fn rounds_in_feed(&self) -> u32 {
        self.magazine
            .as_ref()
            .map_or(self.internal_rounds, |m| m.rounds)
    }

    /// What the round in chamber `c` is loaded with.
    pub fn chamber_load(&self, c: usize) -> Load {
        self.chamber_loads.get(c).copied().unwrap_or_default()
    }

    fn set_chamber_load(&mut self, c: usize, load: Load) {
        if self.chamber_loads.len() < self.chambers.len() {
            self.chamber_loads
                .resize(self.chambers.len(), Load::Standard);
        }
        if let Some(l) = self.chamber_loads.get_mut(c) {
            *l = load;
        }
    }

    /// Takes what's in chamber `c`, noting a live round's load as thrown
    /// clear.
    fn eject_chamber(&mut self, c: usize) -> Option<Round> {
        let r = self.chambers.get_mut(c)?.take();
        if r == Some(Round::Live) {
            self.note_ejected(self.chamber_load(c));
        }
        r
    }

    /// A live round of `load` thrown clear (only the latest few are kept:
    /// a game drains them as it shows the ejections).
    fn note_ejected(&mut self, load: Load) {
        if self.ejected.len() >= 8 {
            self.ejected.remove(0);
        }
        self.ejected.push(load);
    }

    /// Total live rounds in the gun, chambers included.
    pub fn live_rounds(&self) -> u32 {
        let chambered = self
            .chambers
            .iter()
            .filter(|c| **c == Some(Round::Live))
            .count() as u32;
        chambered + self.rounds_in_feed() + u32::from(self.in_transit == Some(Round::Live))
    }

    /// Advances timers. Call once per fixed tick with the trigger state;
    /// handles automatic fire. `roll` supplies uniform randoms in `0..1`.
    pub fn update(&mut self, dt: f32, trigger: bool, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        // With its bolt out there's nothing for the trigger to let go.
        let trigger = trigger && !self.build.bolt_out;
        // Up to a tick of overshoot carries into the next interval, so the
        // cyclic rate holds whatever the tick.
        self.cooldown = (self.cooldown - dt).max(-dt);
        self.rod = (self.rod - dt * 4.0).max(0.0);
        let mut events = Vec::new();
        if self.slam_pending {
            // The pin went in with the bolt: it fires whatever the trigger
            // or the safety.
            self.slam_pending = false;
            if self.is_closed() && self.chambers[0] == Some(Round::Live) {
                events.push(Event::SlamFire);
                let load = self.chamber_load(0);
                self.chambers[0] = Some(Round::Spent);
                let (ev, cycles) = self.discharge(0, load, roll);
                events.extend(ev);
                if cycles && self.def.action == ActionKind::SemiAuto {
                    events.extend(self.auto_cycle(roll));
                }
            }
        }
        if let Some(mut h) = self.hang {
            events.extend(self.hang_on(&mut h, dt, roll));
        }
        if trigger && !self.trigger_down {
            self.trigger_down = true;
            events.extend(self.pull(roll));
        } else if trigger && self.trigger_down {
            if self.fire_mode() == FireMode::Auto && self.cooldown <= 0.0 {
                events.extend(self.pull(roll));
            }
        } else if !trigger && self.trigger_down {
            self.trigger_down = false;
            self.trigger_reset = true;
        }
        events
    }

    /// A hanging round: it goes off when its time's up, or out of battery
    /// if the action's opened on it first.
    fn hang_on(&mut self, h: &mut Hang, dt: f32, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        let mut events = Vec::new();
        let c = h.chamber;
        if self.chambers.get(c) != Some(&Some(Round::Live)) {
            // Taken out of the gun some other way (a revolver's gate).
            self.hang = None;
            return events;
        }
        let open = match self.def.action {
            ActionKind::BreakAction => self.bolt_travel >= 0.5,
            ActionKind::Revolver => false,
            ActionKind::BoltAction => self.handle_lift > 0.3 || self.bolt_travel > 0.05,
            _ => self.bolt_travel > 0.05,
        };
        if open {
            self.hang = None;
            self.chambers[c] = None;
            self.condition = (self.condition - OUT_OF_BATTERY_WEAR).max(0.0);
            events.push(Event::OutOfBattery);
            return events;
        }
        h.left -= dt;
        if h.left > 0.0 {
            self.hang = Some(*h);
            return events;
        }
        self.hang = None;
        self.chambers[c] = Some(Round::Spent);
        let (ev, cycles) = self.discharge(h.barrel, h.load, roll);
        events.extend(ev);
        if cycles && self.def.action == ActionKind::SemiAuto {
            events.extend(self.auto_cycle(roll));
        }
        events
    }

    /// What a round of `load` adds to the misfire and hang-fire chances.
    pub fn primer(&self, load: Load) -> (f32, f32) {
        let (d, h) = match load {
            Load::Surplus => (SURPLUS_DUD, SURPLUS_HANG),
            Load::Damp => (DAMP_DUD, DAMP_HANG),
            _ => (0.0, 0.0),
        };
        let cold = self.ammo_cold.clamp(0.0, 1.0);
        (d + COLD_DUD * cold, h + COLD_HANG * cold)
    }

    /// A timid stroke of a stiff bolt came back short: the action's
    /// half-done until it's worked fully.
    pub fn short_stroke(&mut self) -> Vec<Event> {
        if self.malfunction.is_some() {
            return Vec::new();
        }
        self.malfunction = Some(Malfunction::ShortStroke);
        vec![Event::Malfunctioned(Malfunction::ShortStroke)]
    }

    /// One trigger stroke (or one auto-fire cycle).
    fn pull(&mut self, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        let mut events = Vec::new();
        if self.fire_mode() == FireMode::Safe
            || !self.trigger_reset
            || !self.is_ready()
            || self.hang.is_some()
        {
            return events;
        }
        if self.def.action == ActionKind::Crossbow {
            self.trigger_reset = false;
            return self.loose(roll);
        }
        if self.def.open_bolt {
            if self.malfunction.is_some() {
                return events;
            }
            if self.fire_mode() != FireMode::Auto {
                self.trigger_reset = false;
            }
            return self.open_bolt_cycle(roll);
        }
        // A failure to feed leaves the bolt closed on an empty chamber: the
        // hammer falls and clicks. The others hold the bolt open anyway.
        if self
            .malfunction
            .is_some_and(|m| m != Malfunction::FailureToFeed)
        {
            return events;
        }
        if self.fire_mode() != FireMode::Auto {
            self.trigger_reset = false;
        }
        let barrel = self.firing_barrel();
        let b = barrel as usize;
        if !self.hammers_cocked[b] {
            if self.def.double_action {
                // The long pull cocks it (and turns a cylinder) first.
                self.hammers_cocked[b] = true;
                self.turn_cylinder();
            } else {
                return events;
            }
        }
        self.hammers_cocked[b] = false;
        if self.def.barrels > 1 {
            self.next_barrel = (barrel + 1) % self.def.barrels;
        }
        // The chamber in line with the bore: a revolver's is under the
        // hammer.
        let c = if self.def.action == ActionKind::Revolver {
            self.cylinder
        } else {
            b
        };
        match self.chambers[c] {
            Some(Round::Live) => {
                let load = self.chamber_load(c);
                // Duds are rare and mostly the gun's fault (light strikes);
                // re-packed primers fail more.
                let dud_chance = 0.002
                    + 0.02 * (1.0 - self.condition)
                    + if load == Load::Repacked {
                        REPACK_DUD
                    } else {
                        0.0
                    }
                    + if self.def.rimfire { RIMFIRE_DUD } else { 0.0 }
                    + if self.def.cartridge == "26mm" {
                        PAPER_DUD
                    } else {
                        0.0
                    };
                let (dud, hang) = self.primer(load);
                if roll() < dud_chance + dud {
                    events.push(Event::Dud);
                    return events;
                }
                if hang > 0.0 && roll() < hang {
                    self.hang = Some(Hang {
                        left: HANG_MIN + (HANG_MAX - HANG_MIN) * roll(),
                        barrel,
                        chamber: c,
                        load,
                    });
                    events.push(Event::HangFire);
                    return events;
                }
                self.chambers[c] = Some(Round::Spent);
                let (ev, cycles) = self.discharge(barrel, load, roll);
                events.extend(ev);
                if cycles && self.def.action == ActionKind::SemiAuto {
                    events.extend(self.auto_cycle(roll));
                }
                if cycles
                    && self.def.action == ActionKind::BoltAction
                    && self.sticky_chance() > 0.0
                    && roll() < self.sticky_chance()
                {
                    self.bolt_stuck = true;
                    events.push(Event::BoltStuck);
                }
            }
            _ => events.push(Event::DryFire),
        }
        events
    }

    /// A crossbow's trigger lets the string go: a bolt on the rail flies;
    /// none, and the prod spends itself in a dry loose.
    fn loose(&mut self, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        self.locked_back = false;
        self.bolt_travel = 0.0;
        self.hammers_cocked[0] = false;
        match self.chambers[0].take() {
            Some(Round::Live) => {
                let load = self.chamber_load(0);
                self.discharge(0, load, roll).0
            }
            _ => {
                self.condition = (self.condition - DRY_LOOSE).max(0.0);
                vec![Event::DryFire]
            }
        }
    }

    /// An open bolt let go: it runs forward, strips a round, fires it as
    /// it closes and blows back onto the sear (or on an empty feed closes
    /// on nothing).
    fn open_bolt_cycle(&mut self, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        let mut events = Vec::new();
        self.locked_back = false;
        if !self.take_from_feed() {
            self.bolt_travel = 0.0;
            self.stroke_cleared = false;
            events.push(Event::BoltClosed);
            return events;
        }
        let dud_chance = 0.002 + 0.02 * (1.0 - self.condition);
        if roll() < dud_chance {
            // A dud stays chambered under a closed bolt: rack it out.
            self.chambers[0] = Some(Round::Live);
            self.set_chamber_load(0, self.fed);
            self.bolt_travel = 0.0;
            self.stroke_cleared = false;
            events.push(Event::Chambered);
            events.push(Event::Dud);
            return events;
        }
        let (ev, cycles) = self.discharge(0, self.fed, roll);
        events.extend(ev);
        if !cycles {
            // A squib: no gas to blow the bolt back; it sits closed on the
            // case.
            self.chambers[0] = Some(Round::Spent);
            self.bolt_travel = 0.0;
            self.stroke_cleared = false;
            return events;
        }
        if roll() < self.malfunction_chance() {
            // Blowback catches the case: a stovepipe, the bolt short of
            // the sear.
            self.chambers[0] = Some(Round::Spent);
            self.bolt_travel = STOVEPIPE_AT;
            self.malfunction = Some(Malfunction::FailureToEject);
            events.push(Event::Malfunctioned(Malfunction::FailureToEject));
            return events;
        }
        events.push(Event::Ejected(Round::Spent));
        self.bolt_travel = 1.0;
        self.locked_back = true;
        events
    }

    /// A round of `load` goes off in `barrel`: a shot, a squib (the bullet
    /// stays in the barrel), or a shot into a squib's bullet (the barrel
    /// bulges and the gun's mostly ruined). Returns the events and whether
    /// there was gas to work the action.
    fn discharge(
        &mut self,
        barrel: u32,
        load: Load,
        roll: &mut impl FnMut() -> f32,
    ) -> (Vec<Event>, bool) {
        let mut events = Vec::new();
        if self.squib == Some(barrel) {
            self.squib = None;
            self.condition *= BULGE_KEEPS;
            events.push(Event::Bulged { barrel });
        } else if load == Load::Repacked && roll() < SQUIB_CHANCE {
            self.squib = Some(barrel);
            events.push(Event::Squib { barrel });
            return (events, false);
        }
        self.last_fired = load;
        events.push(Event::Fired { barrel });
        self.wear();
        self.cooldown = self.cooldown.min(0.0)
            + 60.0 / self.def.rpm.max(1.0) * (1.0 + STIFF_SLOWS * self.stiff);
        (events, true)
    }

    /// A roll from the gun's own dice.
    fn own_roll(&mut self) -> f32 {
        let mut r = crate::noise::Rng::new(self.fault_dice);
        let v = r.f32();
        self.fault_dice = r.next_u64();
        v
    }

    /// A rod pushed down the bore from the muzzle: a squib's bullet comes
    /// out.
    pub fn rod_through(&mut self) -> Vec<Event> {
        if self.squib.take().is_some() {
            vec![Event::Rodded]
        } else {
            Vec::new()
        }
    }

    /// Chance a turn-bolt sticks shut after a shot: none in a gun cleaned
    /// lately, likely in a filthy one (fouling seizes the bolt's lugs).
    pub fn sticky_chance(&self) -> f32 {
        ((self.fouling - 0.4).max(0.0) * 1.5).min(0.9)
    }

    /// A palm slapped up hard under a stuck bolt's handle breaks it free.
    pub fn free_bolt(&mut self) -> Vec<Event> {
        if self.bolt_stuck {
            self.bolt_stuck = false;
            vec![Event::BoltFreed]
        } else {
            Vec::new()
        }
    }

    /// Chance a floating pin fires the round the bolt slams home on: never
    /// in a gun in fair order, more as it wears and fouls.
    pub fn slam_chance(&self) -> f32 {
        if !self.def.floating_pin {
            return 0.0;
        }
        let worn = (0.5 - self.condition).max(0.0) * 2.0;
        (worn * (0.15 + 0.6 * self.fouling)).min(0.6)
    }

    /// The chamber at a revolver's loading gate.
    pub fn gate_chamber(&self) -> usize {
        (self.cylinder + self.def.gate_offset as usize) % self.chambers.len().max(1)
    }

    /// A revolver's cylinder steps one chamber on.
    fn turn_cylinder(&mut self) {
        if self.def.action == ActionKind::Revolver {
            self.cylinder = (self.cylinder + 1) % self.chambers.len().max(1);
        }
    }

    /// Thumbs the hammer back (single action): a revolver's cylinder turns
    /// as it cocks. Not with a revolver's gate open.
    pub fn cock_hammer(&mut self) -> Vec<Event> {
        if self.def.open_bolt
            || matches!(
                self.def.action,
                ActionKind::BoltAction | ActionKind::Crossbow
            )
            || self.hammers_cocked.first() != Some(&false)
            || (self.def.action == ActionKind::Revolver && self.gate_open)
        {
            return Vec::new();
        }
        self.hammers_cocked[0] = true;
        self.turn_cylinder();
        vec![Event::Cocked]
    }

    /// Opens or shuts a revolver's loading gate (the hammer must be down
    /// to open it).
    pub fn set_gate(&mut self, open: bool) -> Vec<Event> {
        if self.def.action != ActionKind::Revolver || open == self.gate_open {
            return Vec::new();
        }
        if open && self.hammers_cocked[0] {
            return Vec::new();
        }
        self.gate_open = open;
        vec![if open {
            Event::BoltLocked
        } else {
            Event::BoltClosed
        }]
    }

    /// Turns a revolver's cylinder a chamber by hand (gate open).
    pub fn rotate_cylinder(&mut self) -> Vec<Event> {
        if self.def.action != ActionKind::Revolver || !self.gate_open {
            return Vec::new();
        }
        self.turn_cylinder();
        vec![Event::Cocked]
    }

    /// The ejector rod pushes out whatever is in the chamber at the gate.
    pub fn eject_at_gate(&mut self) -> Vec<Event> {
        if self.def.action != ActionKind::Revolver || !self.gate_open {
            return Vec::new();
        }
        self.rod = 1.0;
        let g = self.gate_chamber();
        self.eject_chamber(g)
            .map(Event::Ejected)
            .into_iter()
            .collect()
    }

    /// Which barrel the trigger will drop next.
    fn firing_barrel(&self) -> u32 {
        if self.def.barrels <= 1 {
            return 0;
        }
        // Doubles fire the next barrel that has a cocked hammer, falling
        // back to the nominal next one so dry fire still clicks.
        (0..self.def.barrels)
            .map(|i| (self.next_barrel + i) % self.def.barrels)
            .find(|&i| self.hammers_cocked[i as usize])
            .unwrap_or(self.next_barrel)
    }

    fn wear(&mut self) {
        self.condition =
            (self.condition - self.def.wear_per_shot * (1.0 + 2.0 * self.fouling)).max(0.0);
        self.fouling =
            (self.fouling + self.def.fouling_per_shot * (1.0 + GRIME_FOULS * self.grime)).min(1.0);
    }

    /// Chance per cycle that a self-loader jams: one in a thousand for a
    /// clean gun in good order, about one in twenty-five at half condition,
    /// one in six worn out and filthy.
    pub fn malfunction_chance(&self) -> f32 {
        let wear = (1.0 - self.condition).powi(2);
        (0.001
            + 0.15 * wear
            + 0.03 * self.fouling
            + GRIME_JAMS * self.grime
            + STIFF_JAMS * self.stiff)
            .min(0.5)
    }

    /// The gas system's stroke after a shot: eject, cock, feed (or jam, or
    /// lock open).
    fn auto_cycle(&mut self, roll: &mut impl FnMut() -> f32) -> Vec<Event> {
        let mut events = Vec::new();
        if roll() < self.malfunction_chance() {
            // Stovepipes are the common one; double feeds need two rounds
            // to jam together, failures to feed a magazine to slip.
            let r = roll();
            let feed = if self.magazine.is_some() {
                self.rounds_in_feed()
            } else {
                0
            };
            let kind = if r < 0.45 || feed == 0 {
                Malfunction::FailureToEject
            } else if r < 0.85 || feed < 2 {
                Malfunction::FailureToFeed
            } else {
                Malfunction::DoubleFeed
            };
            match kind {
                Malfunction::FailureToEject => {
                    // The case is stuck; the bolt rides on it, part open.
                    self.bolt_travel = STOVEPIPE_AT;
                }
                Malfunction::FailureToFeed => {
                    self.chambers[0] = None;
                    events.push(Event::Ejected(Round::Spent));
                    self.mag_unseated = true;
                }
                Malfunction::DoubleFeed => {
                    // The case stays put; the next round rams into it.
                    if self.take_from_feed() {
                        self.in_transit = Some(Round::Live);
                        self.transit_load = self.fed;
                    }
                    self.bolt_travel = DOUBLE_FEED_AT;
                }
                // (Only ever a hand's doing: `short_stroke`.)
                Malfunction::ShortStroke => {}
            }
            self.hammers_cocked[0] = true;
            self.malfunction = Some(kind);
            events.push(Event::Malfunctioned(kind));
            return events;
        }
        self.chambers[0] = None;
        events.push(Event::Ejected(Round::Spent));
        self.hammers_cocked[0] = true;
        if self.take_from_feed() {
            self.chambers[0] = Some(Round::Live);
            self.set_chamber_load(0, self.fed);
            events.push(Event::Chambered);
            if self.slam_chance() > 0.0 && roll() < self.slam_chance() {
                self.slam_pending = true;
            }
        } else if self.def.last_round_hold_open && (self.magazine.is_some() || self.fixed_feed()) {
            self.locked_back = true;
            self.bolt_travel = 1.0;
            events.push(Event::BoltLocked);
        }
        events
    }

    /// A fixed magazine on a self-loader (an SKS), loaded from clips.
    fn fixed_feed(&self) -> bool {
        self.def.action == ActionKind::SemiAuto
            && self.def.magazine_family.is_none()
            && self.def.internal_capacity > 0
    }

    fn take_from_feed(&mut self) -> bool {
        if self.mag_unseated {
            return false;
        }
        if let Some(mag) = &mut self.magazine {
            if mag.rounds > 0 {
                self.fed = mag.loads.pop(mag.rounds);
                mag.rounds -= 1;
                return true;
            }
            return false;
        }
        if self.def.magazine_family.is_none() && self.internal_rounds > 0 {
            self.fed = self.internal_loads.pop(self.internal_rounds);
            self.internal_rounds -= 1;
            return true;
        }
        false
    }

    /// Moves the charging handle, slide or bolt to `travel` (0 closed, 1 full
    /// rear). Call continuously while a hand holds it; let go by calling
    /// [`Self::release_bolt`]. For turn-bolts, the handle must be lifted
    /// first or the bolt will not move.
    pub fn set_bolt_travel(&mut self, travel: f32) -> Vec<Event> {
        let mut events = Vec::new();
        if matches!(
            self.def.action,
            ActionKind::BreakAction | ActionKind::Revolver
        ) {
            return events;
        }
        if self.def.action == ActionKind::BoltAction && self.handle_lift < 0.95 {
            return events;
        }
        // Worked fully, a short-stroked action's put right (and goes on
        // to throw out whatever's in the chamber).
        if self.malfunction == Some(Malfunction::ShortStroke) && travel >= 0.98 {
            self.malfunction = None;
        }
        if self.def.action == ActionKind::Pump {
            return self.pump_travel(travel.clamp(0.0, 1.0));
        }
        if self.def.action == ActionKind::Crossbow {
            // Drawn by hand; caught on the latch, it stays there.
            if !self.locked_back {
                self.bolt_travel = travel.clamp(0.0, 1.0);
            }
            return events;
        }
        let mut to = travel.clamp(0.0, 1.0);
        let from = self.bolt_travel;
        if self.malfunction == Some(Malfunction::DoubleFeed) {
            // The pair is jammed together: the bolt won't go home past it.
            to = to.max(DOUBLE_FEED_AT);
            if self.magazine.is_some() {
                // Wedged against the magazine's lips: the bolt comes back
                // but nothing comes out.
                self.bolt_travel = to;
                return Vec::new();
            }
        }
        if self.locked_back && to < 1.0 {
            // Tugging the slide back off the stop releases it, and it
            // strips a round on the way home as if the stop had. An open
            // bolt eased off its sear by hand just rides (racking a cocked
            // PPSh mustn't throw a round away).
            self.locked_back = false;
            self.stroke_cleared = !self.def.open_bolt;
        }
        if to > from {
            // A half-seated magazine doesn't ride out the rack.
            if self.mag_half && from < STRIP_AT && to >= STRIP_AT {
                self.mag_half = false;
                self.mag_unseated = false;
                self.magazine = None;
                events.push(Event::MagazineDropped);
            }
            if from < COCK_AT && to >= COCK_AT {
                for h in &mut self.hammers_cocked {
                    if !*h {
                        *h = true;
                        events.push(Event::Cocked);
                    }
                }
            }
            // The ejector throws what's on the bolt face once a stroke, as
            // the bolt passes it (a bolt held open and jiggled doesn't throw
            // out the round just thumbed in).
            if from < EJECT_AT && to >= EJECT_AT {
                if let Some(round) = self.eject_chamber(0) {
                    events.push(Event::Ejected(round));
                }
                if let Some(round) = self.in_transit.take() {
                    if round == Round::Live {
                        self.note_ejected(self.transit_load);
                    }
                    events.push(Event::Ejected(round));
                }
                // Racking throws out a stovepipe's case and a double feed's
                // pair (its magazine out). A slipped magazine stays a
                // failure to feed until it's tapped home.
                if !(self.malfunction == Some(Malfunction::FailureToFeed) && self.mag_unseated) {
                    self.malfunction = None;
                }
                self.stroke_cleared = true;
            }
        } else if to < from {
            if self.stroke_cleared && from >= STRIP_AT && to < STRIP_AT {
                self.stroke_cleared = false;
                if self.take_from_feed() {
                    self.in_transit = Some(Round::Live);
                    self.transit_load = self.fed;
                }
            }
            if to <= CLOSED_AT && from > CLOSED_AT {
                if let Some(round) = self.in_transit.take() {
                    self.chambers[0] = Some(round);
                    self.set_chamber_load(0, self.transit_load);
                    events.push(Event::Chambered);
                    if self.slam_chance() > 0.0 && self.own_roll() < self.slam_chance() {
                        self.slam_pending = true;
                    }
                }
                events.push(Event::BoltClosed);
            }
        }
        self.bolt_travel = to;
        events
    }

    /// A pump's forend moved to `to`: back, it cocks the hammer, lets the
    /// next shell out of the tube onto the carrier and (at the end of the
    /// stroke) throws out what's in the chamber; forward, it rams the
    /// carrier's shell home. A shell let out on a short stroke meets the
    /// case still in the chamber, and the two jam until the forend is
    /// pumped hard to the rear.
    fn pump_travel(&mut self, mut to: f32) -> Vec<Event> {
        let mut events = Vec::new();
        let from = self.bolt_travel;
        let jammed = self.malfunction == Some(Malfunction::DoubleFeed);
        if to > from
            && from <= CLOSED_AT
            && self.hammers_cocked[0]
            && !self.action_released
            && !jammed
        {
            // The action bar lock holds it shut on a cocked hammer.
            return events;
        }
        if to > from {
            if from < COCK_AT && to >= COCK_AT && !self.hammers_cocked[0] {
                self.hammers_cocked[0] = true;
                events.push(Event::Cocked);
            }
            if from < PUMP_RELEASE
                && to >= PUMP_RELEASE
                && self.in_transit.is_none()
                && self.take_from_feed()
            {
                self.in_transit = Some(Round::Live);
                self.transit_load = self.fed;
            }
            if from < EJECT_AT && to >= EJECT_AT {
                // A worn extractor can slip off a fired hull and leave it.
                let slips = self.chambers[0] == Some(Round::Spent)
                    && self.own_roll() < self.malfunction_chance();
                if !slips {
                    if let Some(round) = self.eject_chamber(0) {
                        events.push(Event::Ejected(round));
                    }
                    if jammed {
                        self.malfunction = None;
                    }
                }
            }
        } else if to < from {
            let both = self.chambers[0].is_some() && self.in_transit.is_some();
            if both && to < DOUBLE_FEED_AT {
                // The carrier's shell runs into the case: it stops there.
                to = DOUBLE_FEED_AT.min(from);
                if !jammed {
                    self.malfunction = Some(Malfunction::DoubleFeed);
                    events.push(Event::Malfunctioned(Malfunction::DoubleFeed));
                }
            } else if to <= CLOSED_AT && from > CLOSED_AT {
                if let Some(round) = self.in_transit.take() {
                    self.chambers[0] = Some(round);
                    self.set_chamber_load(0, self.transit_load);
                    events.push(Event::Chambered);
                }
                self.action_released = false;
                events.push(Event::BoltClosed);
            }
        }
        self.bolt_travel = to;
        events
    }

    /// A pump's action release: the forend will come back on a cocked
    /// hammer (to clear a live shell, or to look).
    pub fn release_action(&mut self) -> Vec<Event> {
        if self.def.action != ActionKind::Pump || self.action_released {
            return Vec::new();
        }
        self.action_released = true;
        vec![Event::BoltLocked]
    }

    /// The hand lets go of the bolt or slide. Spring-driven actions slam home
    /// (or onto the slide stop); turn-bolts stay where they were left.
    pub fn release_bolt(&mut self) -> Vec<Event> {
        match self.def.action {
            ActionKind::Crossbow => {
                if self.locked_back {
                    return Vec::new();
                }
                let drawn = self.bolt_travel;
                self.bolt_travel = 0.0;
                if drawn >= EJECT_AT {
                    // Over the latch: it catches.
                    self.bolt_travel = 1.0;
                    self.locked_back = true;
                    self.hammers_cocked[0] = true;
                    vec![Event::Cocked]
                } else if drawn > 0.3 {
                    // Let go short: the string slaps home.
                    self.condition = (self.condition - STRING_SLAP).max(0.0);
                    vec![Event::BoltClosed]
                } else {
                    Vec::new()
                }
            }
            ActionKind::SemiAuto if self.def.open_bolt => {
                if self.locked_back {
                    return Vec::new();
                }
                if self.bolt_travel >= EJECT_AT {
                    // Drawn back to the sear: it catches there, cocked.
                    self.bolt_travel = 1.0;
                    self.locked_back = true;
                    self.stroke_cleared = false;
                    return vec![Event::BoltLocked];
                }
                // Let go short of the sear: it rides home over the feed
                // without stripping (an open bolt doesn't slam-fire here).
                self.stroke_cleared = false;
                self.set_bolt_travel(0.0)
            }
            ActionKind::SemiAuto => {
                let empty_hold = self.def.last_round_hold_open
                    && (self.magazine.as_ref().is_some_and(|m| m.rounds == 0)
                        || (self.fixed_feed() && self.internal_rounds == 0))
                    && self.stroke_cleared
                    && self.bolt_travel >= EJECT_AT;
                if empty_hold {
                    self.locked_back = true;
                    self.bolt_travel = 1.0;
                    self.stroke_cleared = false;
                    return vec![Event::BoltLocked];
                }
                if self.locked_back {
                    return Vec::new();
                }
                self.set_bolt_travel(0.0)
            }
            _ => Vec::new(),
        }
    }

    /// A hand tugs a locked-back slide or bolt rearward, off its stop: let
    /// go, it runs home ([`Self::release_bolt`]) and strips a round.
    pub fn take_off_stop(&mut self) {
        if self.locked_back && !self.def.open_bolt {
            self.locked_back = false;
            self.stroke_cleared = true;
        }
    }

    /// Thumbs the slide stop down: a locked-back slide runs home.
    pub fn release_slide_stop(&mut self) -> Vec<Event> {
        // An open bolt's sear is the trigger's business.
        if !self.locked_back || self.def.open_bolt {
            return Vec::new();
        }
        self.locked_back = false;
        self.bolt_travel = 1.0;
        self.stroke_cleared = true;
        self.set_bolt_travel(0.0)
    }

    /// Lifts or lowers a turn-bolt handle (0 down, 1 up). Lifting cocks the
    /// striker, as on a Mosin. The bolt must be forward to turn.
    pub fn set_handle_lift(&mut self, lift: f32) -> Vec<Event> {
        let mut events = Vec::new();
        if self.def.action != ActionKind::BoltAction || self.bolt_travel > CLOSED_AT {
            return events;
        }
        if self.bolt_stuck {
            return events;
        }
        let to = lift.clamp(0.0, 1.0);
        if to >= 0.9 && self.handle_lift < 0.9 && !self.hammers_cocked[0] {
            self.hammers_cocked[0] = true;
            events.push(Event::Cocked);
        }
        self.handle_lift = to;
        events
    }

    /// Seats a magazine. Returns it back if it doesn't fit or one is in.
    pub fn insert_magazine(&mut self, mag: Magazine) -> Result<Vec<Event>, Magazine> {
        let fits = self.def.magazine_family.as_deref() == Some(mag.family.as_str());
        if !fits || self.magazine.is_some() {
            return Err(mag);
        }
        self.magazine = Some(mag);
        self.mag_unseated = false;
        self.mag_half = false;
        Ok(vec![Event::MagazineInserted])
    }

    /// A magazine pushed in short of its catch: it's in the well, it won't
    /// feed, and the action's jolt drops it; a tap seats it.
    pub fn half_seat_magazine(&mut self, mag: Magazine) -> Result<Vec<Event>, Magazine> {
        self.insert_magazine(mag)?;
        self.mag_unseated = true;
        self.mag_half = true;
        Ok(vec![Event::MagazineHalfSeated])
    }

    /// Presses the mag release.
    pub fn remove_magazine(&mut self) -> Option<Magazine> {
        self.mag_unseated = false;
        self.mag_half = false;
        self.magazine.take()
    }

    /// A palm slapped up on the magazine's floorplate: a slipped magazine
    /// seats (the "tap" of tap-rack).
    pub fn tap_magazine(&mut self) -> Vec<Event> {
        if self.magazine.is_some() && self.mag_unseated {
            self.mag_unseated = false;
            self.mag_half = false;
            vec![Event::MagazineInserted]
        } else {
            Vec::new()
        }
    }

    /// Breaks a double open (true) or snaps it shut (false). Opening cocks
    /// both hammers and kicks out spent shells; live shells stay put.
    pub fn set_break_open(&mut self, open: bool) -> Vec<Event> {
        let mut events = Vec::new();
        if self.def.action != ActionKind::BreakAction {
            return events;
        }
        let was_open = self.bolt_travel > 0.5;
        if open && !was_open {
            self.bolt_travel = 1.0;
            for (i, chamber) in self.chambers.iter_mut().enumerate() {
                if *chamber == Some(Round::Spent) {
                    *chamber = None;
                    events.push(Event::Ejected(Round::Spent));
                }
                if !self.hammers_cocked[i] {
                    self.hammers_cocked[i] = true;
                    events.push(Event::Cocked);
                }
            }
        } else if !open && was_open {
            self.bolt_travel = 0.0;
            events.push(Event::BoltClosed);
        }
        events
    }

    /// Pulls a shell out of an open break-action's barrel by hand.
    pub fn unload_chamber(&mut self, barrel: u32) -> Option<Round> {
        if self.def.action == ActionKind::BreakAction && self.bolt_travel < 0.5 {
            return None;
        }
        self.eject_chamber(barrel as usize)
    }

    /// Thumbs one round into wherever loose rounds go on this gun: an open
    /// barrel on a double, the internal magazine on a bolt action (bolt
    /// open), or directly into an open chamber otherwise. Returns false if
    /// there is no room.
    pub fn load_round(&mut self) -> bool {
        self.load_round_of(Load::Standard)
    }

    /// [`Self::load_round`] with a round of `load`.
    pub fn load_round_of(&mut self, load: Load) -> bool {
        match self.def.action {
            ActionKind::Crossbow => {
                // A bolt into the groove, the string on the latch.
                if !self.locked_back || self.chambers[0].is_some() {
                    return false;
                }
                self.chambers[0] = Some(Round::Live);
                self.set_chamber_load(0, load);
                true
            }
            ActionKind::Revolver => {
                let c = self.gate_chamber();
                if self.gate_open && self.chambers[c].is_none() {
                    self.chambers[c] = Some(Round::Live);
                    self.set_chamber_load(c, load);
                    return true;
                }
                false
            }
            ActionKind::BreakAction => {
                if self.bolt_travel < 0.5 {
                    return false;
                }
                if let Some(c) = self.chambers.iter().position(|c| c.is_none()) {
                    self.chambers[c] = Some(Round::Live);
                    self.set_chamber_load(c, load);
                    return true;
                }
                false
            }
            ActionKind::BoltAction if self.def.internal_capacity == 0 => {
                // A single-shot: the round goes into the open chamber.
                if self.bolt_travel < EJECT_AT || self.chambers[0].is_some() {
                    return false;
                }
                self.chambers[0] = Some(Round::Live);
                self.set_chamber_load(0, load);
                true
            }
            ActionKind::Pump => {
                // Through the loading port into the tube, forend forward.
                if self.bolt_travel > 0.5 || self.internal_rounds >= self.def.internal_capacity {
                    return false;
                }
                self.internal_loads.push(self.internal_rounds, load);
                self.internal_rounds += 1;
                true
            }
            ActionKind::BoltAction => {
                if self.bolt_travel < EJECT_AT {
                    return false;
                }
                if self.internal_rounds < self.def.internal_capacity {
                    self.internal_loads.push(self.internal_rounds, load);
                    self.internal_rounds += 1;
                    return true;
                }
                false
            }
            ActionKind::SemiAuto if self.fixed_feed() => {
                if self.bolt_travel < EJECT_AT || self.internal_rounds >= self.def.internal_capacity
                {
                    return false;
                }
                self.internal_loads.push(self.internal_rounds, load);
                self.internal_rounds += 1;
                true
            }
            ActionKind::SemiAuto => {
                if self.bolt_travel >= EJECT_AT && self.chambers[0].is_none() {
                    self.chambers[0] = Some(Round::Live);
                    self.set_chamber_load(0, load);
                    return true;
                }
                false
            }
        }
    }

    /// Strips a clip of `n` rounds into an internal magazine through the open
    /// action. Returns how many went in.
    pub fn strip_clip(&mut self, n: u32) -> u32 {
        self.strip_clip_of(n, &mut Loads::default())
    }

    /// [`Self::strip_clip`] from a clip of `n` rounds loaded as `loads`
    /// (which lose the rounds that went in).
    pub fn strip_clip_of(&mut self, n: u32, loads: &mut Loads) -> u32 {
        let clip_fed = self.def.action == ActionKind::BoltAction || self.fixed_feed();
        if !clip_fed || self.bolt_travel < EJECT_AT {
            return 0;
        }
        let room = self.def.internal_capacity - self.internal_rounds;
        let moved = room.min(n);
        loads.move_to(n, &mut self.internal_loads, self.internal_rounds, moved);
        self.internal_rounds += moved;
        moved
    }

    /// Cleaning: removes fouling (and the grime with it). `effort` is
    /// 0..=1 of a full cleaning.
    pub fn clean(&mut self, effort: f32) {
        let e = effort.clamp(0.0, 1.0);
        self.fouling = (self.fouling - e).max(0.0);
        self.grime = (self.grime - e).max(0.0);
    }

    /// A field wipe: a glove run over the action takes off `effort` of the
    /// grime; the fouling inside wants a proper cleaning.
    pub fn wipe(&mut self, effort: f32) {
        self.grime = (self.grime - effort.clamp(0.0, 1.0)).max(0.0);
    }

    /// Repair with spare parts: restores condition, never above `cap` (worn
    /// guns can only be brought back so far).
    pub fn repair(&mut self, amount: f32, cap: f32) {
        self.condition = (self.condition + amount)
            .min(cap.clamp(0.0, 1.0))
            .max(self.condition);
    }

    /// Spread in radians for the next shot.
    /// How much its action drags in the hand, 0 free ..= 1: cold oil
    /// (`stiff`), or a worn, dry, fouled action (worn past 0.6 it starts to
    /// grind, and fouling adds to it): the feel of a gun that wants
    /// cleaning and oil.
    pub fn action_drag(&self) -> f32 {
        let worn = ((DRY_FROM - self.condition) / DRY_FROM).clamp(0.0, 1.0);
        self.stiff
            .max(DRY_DRAG * worn + FOUL_DRAG * self.fouling.clamp(0.0, 1.0))
    }

    pub fn spread_radians(&self) -> f32 {
        let moa = self.def.spread_moa * (1.0 + 3.0 * (1.0 - self.condition) + self.fouling);
        moa / 60.0 * std::f32::consts::PI / 180.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pistol() -> FirearmDef {
        FirearmDef {
            action: ActionKind::SemiAuto,
            cartridge: "9x18".into(),
            magazine_family: Some("pm".into()),
            internal_capacity: 0,
            barrels: 1,
            modes: vec![FireMode::Safe, FireMode::Semi],
            rpm: 600.0,
            last_round_hold_open: true,
            double_action: true,
            open_bolt: false,
            gate_offset: 0,
            muzzle_velocity: 315.0,
            damage: 30.0,
            projectiles: 1,
            spread_moa: 8.0,
            recoil: 1.0,
            wear_per_shot: 0.0,
            fouling_per_shot: 0.0,
            floating_pin: false,
            report: 1.0,
            rimfire: false,
            kick: Kick::default(),
        }
    }

    fn rifle() -> FirearmDef {
        FirearmDef {
            magazine_family: Some("ak545".into()),
            modes: vec![FireMode::Safe, FireMode::Auto, FireMode::Semi],
            last_round_hold_open: false,
            double_action: false,
            ..pistol()
        }
    }

    fn mosin() -> FirearmDef {
        FirearmDef {
            action: ActionKind::BoltAction,
            magazine_family: None,
            internal_capacity: 5,
            modes: vec![FireMode::Semi],
            double_action: false,
            last_round_hold_open: false,
            ..pistol()
        }
    }

    fn double() -> FirearmDef {
        FirearmDef {
            action: ActionKind::BreakAction,
            magazine_family: None,
            barrels: 2,
            modes: vec![FireMode::Semi],
            double_action: false,
            last_round_hold_open: false,
            ..pistol()
        }
    }

    fn mag(family: &str, rounds: u32) -> Magazine {
        Magazine {
            family: family.into(),
            capacity: 8,
            rounds,
            ..Default::default()
        }
    }

    /// AP thumbed on top of ball comes out first; a live round racked out
    /// is noted with its load.
    #[test]
    fn rounds_keep_their_loads_through_the_gun() {
        let mut gun = Firearm::new(pistol(), 1.0);
        gun.cycle_mode();
        let mut m = mag("pm", 3);
        m.loads.fill(1, Load::Ap, 2);
        gun.insert_magazine(m).unwrap();
        rack(&mut gun);
        assert_eq!(gun.chamber_load(0), Load::Ap);
        let ev = shoot(&mut gun);
        assert!(fired(&ev));
        assert_eq!(gun.last_fired, Load::Ap);
        assert_eq!(gun.chamber_load(0), Load::Ap, "the second AP chambered");
        gun.ejected.clear();
        rack(&mut gun);
        assert_eq!(gun.ejected, vec![Load::Ap], "racked out live");
        shoot(&mut gun);
        assert_eq!(gun.last_fired, Load::Standard, "then the ball");
    }

    /// Rolls in order, then lucky ones.
    fn rolls(v: &[f32]) -> impl FnMut() -> f32 {
        let mut it = v.to_vec().into_iter();
        move || it.next().unwrap_or(0.99)
    }

    /// A re-packed round squibs: a pop, no shot, the bullet in the bore and
    /// the action unmoved. Fire into it and the barrel bulges; rod it out
    /// first and the next round fires clean.
    #[test]
    fn a_squib_blocks_the_bore_until_rodded() {
        for rod_first in [false, true] {
            let mut gun = Firearm::new(pistol(), 1.0);
            gun.cycle_mode();
            let mut m = mag("pm", 3);
            m.loads.fill(0, Load::Repacked, 3);
            gun.insert_magazine(m).unwrap();
            rack(&mut gun);
            // The dud roll, then the squib's.
            let mut r = rolls(&[0.5, 0.01]);
            let ev = gun.update(0.016, true, &mut r);
            gun.update(0.016, false, &mut r);
            assert!(ev.contains(&Event::Squib { barrel: 0 }), "{ev:?}");
            assert!(!fired(&ev), "no shot");
            assert_eq!(gun.squib, Some(0));
            assert_eq!(gun.chambers[0], Some(Round::Spent), "the case stays");
            assert!(gun.is_closed(), "no gas to cycle it");
            rack(&mut gun);
            if rod_first {
                assert_eq!(gun.rod_through(), vec![Event::Rodded]);
            }
            let ev = shoot(&mut gun);
            assert!(fired(&ev));
            let bulged = ev.contains(&Event::Bulged { barrel: 0 });
            assert_eq!(bulged, !rod_first, "{ev:?}");
            if bulged {
                assert!(gun.condition < 0.35, "ruined: {}", gun.condition);
            }
            assert_eq!(gun.squib, None);
        }
    }

    /// A filthy Mosin's bolt sticks shut after the shot: the handle won't
    /// lift until it's knocked free; a clean one never sticks.
    #[test]
    fn a_dirty_mosin_bolt_sticks() {
        let mut gun = Firearm::new(mosin(), 1.0);
        assert_eq!(gun.sticky_chance(), 0.0, "clean: never");
        gun.condition = 0.1;
        assert_eq!(gun.sticky_chance(), 0.0, "worn but clean: never");
        gun.condition = 1.0;
        gun.fouling = 1.0;
        gun.set_handle_lift(1.0);
        gun.set_bolt_travel(1.0);
        assert!(gun.load_round());
        gun.set_bolt_travel(0.0);
        gun.set_handle_lift(0.0);
        // The dud roll, then the stick's.
        let mut r = rolls(&[0.5, 0.0]);
        let ev = gun.update(0.016, true, &mut r);
        assert!(fired(&ev));
        assert!(ev.contains(&Event::BoltStuck), "{ev:?}");
        gun.set_handle_lift(1.0);
        assert_eq!(gun.handle_lift, 0.0, "it won't lift");
        assert_eq!(gun.free_bolt(), vec![Event::BoltFreed]);
        gun.set_handle_lift(1.0);
        assert_eq!(gun.handle_lift, 1.0);
    }

    /// A worn, dirty SKS's floating pin fires the round the bolt slams home
    /// on, trigger untouched and on safe; one in fair order never does.
    #[test]
    fn a_worn_sks_slam_fires() {
        let sks = FirearmDef {
            floating_pin: true,
            ..rifle()
        };
        let mut good = Firearm::new(sks.clone(), 0.9);
        good.fouling = 1.0;
        assert_eq!(good.slam_chance(), 0.0);
        let mut gun = Firearm::new(sks, 0.1);
        gun.fouling = 1.0;
        assert!(gun.slam_chance() > 0.3, "{}", gun.slam_chance());
        assert_eq!(gun.fire_mode(), FireMode::Safe);
        gun.insert_magazine(mag("ak545", 8)).unwrap();
        // Racked until the bolt slams one home (better than even odds each
        // time on its own dice).
        let mut ev = Vec::new();
        for _ in 0..10 {
            rack(&mut gun);
            ev = gun.update(0.016, false, &mut lucky());
            if fired(&ev) {
                break;
            }
        }
        assert_eq!(ev.first(), Some(&Event::SlamFire), "{ev:?}");
        assert!(fired(&ev), "it went off by itself");
    }

    /// Never jams, never duds.    /// Never jams, never duds.
    fn lucky() -> impl FnMut() -> f32 {
        || 0.99
    }

    fn rack(gun: &mut Firearm) -> Vec<Event> {
        let mut ev = gun.set_bolt_travel(1.0);
        ev.extend(gun.release_bolt());
        ev
    }

    fn shoot(gun: &mut Firearm) -> Vec<Event> {
        let mut r = lucky();
        let mut ev = gun.update(0.016, true, &mut r);
        ev.extend(gun.update(0.016, false, &mut r));
        ev
    }

    fn fired(ev: &[Event]) -> bool {
        ev.iter().any(|e| matches!(e, Event::Fired { .. }))
    }

    #[test]
    fn pistol_load_rack_fire_until_slide_locks() {
        let mut gun = Firearm::new(pistol(), 1.0);
        gun.cycle_mode(); // off safe
        gun.insert_magazine(mag("pm", 2)).unwrap();
        assert!(!fired(&shoot(&mut gun)), "nothing chambered yet");
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Chambered));
        assert!(fired(&shoot(&mut gun)));
        assert!(fired(&shoot(&mut gun)));
        assert!(gun.locked_back, "slide locks on the empty mag");
        assert!(!fired(&shoot(&mut gun)));
        // Fresh mag, drop the slide.
        gun.remove_magazine();
        gun.insert_magazine(mag("pm", 8)).unwrap();
        let ev = gun.release_slide_stop();
        assert!(ev.contains(&Event::Chambered));
        assert!(fired(&shoot(&mut gun)));
        assert_eq!(gun.live_rounds(), 7);
    }

    #[test]
    fn safety_blocks_the_trigger() {
        let mut gun = Firearm::new(pistol(), 1.0);
        gun.insert_magazine(mag("pm", 8)).unwrap();
        rack(&mut gun);
        assert_eq!(gun.fire_mode(), FireMode::Safe);
        assert!(!fired(&shoot(&mut gun)));
        gun.cycle_mode();
        assert!(fired(&shoot(&mut gun)));
    }

    #[test]
    fn racking_a_loaded_gun_ejects_a_live_round() {
        let mut gun = Firearm::new(pistol(), 1.0);
        gun.insert_magazine(mag("pm", 8)).unwrap();
        rack(&mut gun);
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Live)));
        assert_eq!(gun.live_rounds(), 7);
    }

    #[test]
    fn wrong_magazine_does_not_fit() {
        let mut gun = Firearm::new(pistol(), 1.0);
        assert!(gun.insert_magazine(mag("ak545", 30)).is_err());
    }

    #[test]
    fn semi_needs_trigger_reset_and_auto_does_not() {
        let mut gun = Firearm::new(rifle(), 1.0);
        gun.insert_magazine(Magazine {
            family: "ak545".into(),
            capacity: 30,
            rounds: 30,
            ..Default::default()
        })
        .unwrap();
        rack(&mut gun);
        gun.cycle_mode(); // Auto
        let mut r = lucky();
        let mut shots = 0;
        // One second of held trigger at 600 rpm is ~10 shots.
        for _ in 0..60 {
            shots += gun
                .update(1.0 / 60.0, true, &mut r)
                .iter()
                .filter(|e| matches!(e, Event::Fired { .. }))
                .count();
        }
        assert!((9..=11).contains(&shots), "{shots} shots");
        gun.update(1.0 / 60.0, false, &mut r);
        gun.cycle_mode(); // Semi
        let mut semi_shots = 0;
        for _ in 0..60 {
            semi_shots += gun
                .update(1.0 / 60.0, true, &mut r)
                .iter()
                .filter(|e| matches!(e, Event::Fired { .. }))
                .count();
        }
        assert_eq!(semi_shots, 1);
    }

    #[test]
    fn ak_does_not_lock_back() {
        let mut gun = Firearm::new(rifle(), 1.0);
        gun.insert_magazine(Magazine {
            family: "ak545".into(),
            capacity: 30,
            rounds: 1,
            ..Default::default()
        })
        .unwrap();
        rack(&mut gun);
        gun.cycle_mode();
        gun.cycle_mode(); // Semi
        assert!(fired(&shoot(&mut gun)));
        assert!(!gun.locked_back);
        assert!(gun.is_closed());
        let ev = shoot(&mut gun);
        assert!(ev.contains(&Event::DryFire) || ev.is_empty());
    }

    #[test]
    fn bolt_action_cycle() {
        let mut gun = Firearm::new(mosin(), 1.0);
        assert!(
            gun.set_bolt_travel(1.0).is_empty(),
            "handle must lift first"
        );
        gun.set_handle_lift(1.0);
        gun.set_bolt_travel(1.0);
        assert_eq!(gun.strip_clip(5), 5);
        gun.set_bolt_travel(0.0);
        assert!(!gun.is_closed(), "handle still up");
        assert!(!fired(&shoot(&mut gun)));
        gun.set_handle_lift(0.0);
        assert!(fired(&shoot(&mut gun)));
        assert!(!fired(&shoot(&mut gun)), "bolt guns don't self-load");
        gun.set_handle_lift(1.0);
        let ev = gun.set_bolt_travel(1.0);
        assert!(ev.contains(&Event::Ejected(Round::Spent)));
        gun.set_bolt_travel(0.0);
        gun.set_handle_lift(0.0);
        assert!(fired(&shoot(&mut gun)));
        assert_eq!(gun.live_rounds(), 3);
    }

    #[test]
    fn double_barrel_fires_both_then_ejects_both() {
        let mut gun = Firearm::new(double(), 1.0);
        gun.set_break_open(true);
        assert!(gun.load_round());
        assert!(gun.load_round());
        assert!(!gun.load_round());
        gun.set_break_open(false);
        assert!(fired(&shoot(&mut gun)));
        assert!(fired(&shoot(&mut gun)));
        assert!(!fired(&shoot(&mut gun)));
        let ev = gun.set_break_open(true);
        let spent = ev
            .iter()
            .filter(|e| **e == Event::Ejected(Round::Spent))
            .count();
        assert_eq!(spent, 2);
    }

    #[test]
    fn worn_dirty_guns_jam_and_racking_clears() {
        let mut gun = Firearm::new(pistol(), 0.0);
        gun.fouling = 1.0;
        assert!(gun.malfunction_chance() > 0.1);
        gun.cycle_mode();
        gun.insert_magazine(mag("pm", 8)).unwrap();
        rack(&mut gun);
        // Rolls: dud check passes, then jam roll hits, then FTE.
        let mut seq = [0.99, 0.0, 0.1].into_iter();
        let mut r = move || seq.next().unwrap_or(0.99);
        let ev = gun.update(0.016, true, &mut r);
        assert!(ev.contains(&Event::Malfunctioned(Malfunction::FailureToEject)));
        gun.update(0.016, false, &mut r);
        assert!(!fired(&shoot(&mut gun)), "jammed");
        rack(&mut gun);
        assert!(gun.malfunction.is_none());
        assert!(fired(&shoot(&mut gun)));
    }

    /// A worn gun, loaded and ready, whose next shot jams as `kind`: the
    /// rolls pass the dud check, hit the jam roll, then pick the kind.
    fn jam(kind_roll: f32) -> (Firearm, Vec<Event>) {
        let mut gun = Firearm::new(rifle(), 0.0);
        gun.fouling = 1.0;
        gun.insert_magazine(Magazine {
            family: "ak545".into(),
            capacity: 30,
            rounds: 10,
            ..Default::default()
        })
        .unwrap();
        rack(&mut gun);
        gun.cycle_mode();
        gun.cycle_mode(); // Semi
        let mut seq = [0.99, 0.0, kind_roll].into_iter();
        let mut r = move || seq.next().unwrap_or(0.99);
        let mut ev = gun.update(0.016, true, &mut r);
        ev.extend(gun.update(0.016, false, &mut r));
        (gun, ev)
    }

    #[test]
    fn a_half_seated_magazine_feeds_nothing_and_drops_on_the_rack() {
        let mag = || Magazine {
            family: "ak545".into(),
            capacity: 30,
            rounds: 10,
            ..Default::default()
        };
        let mut gun = Firearm::new(rifle(), 1.0);
        assert_eq!(
            gun.half_seat_magazine(mag()).unwrap(),
            vec![Event::MagazineHalfSeated]
        );
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::MagazineDropped), "{ev:?}");
        assert!(!ev.contains(&Event::Chambered), "it fed nothing");
        assert!(gun.magazine.is_none(), "it fell out");
        // Tapped home before the rack, it feeds.
        gun.half_seat_magazine(mag()).unwrap();
        assert_eq!(gun.tap_magazine(), vec![Event::MagazineInserted]);
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Chambered), "{ev:?}");
        assert!(gun.magazine.is_some());
    }

    #[test]
    fn a_stovepipe_holds_the_bolt_until_racked() {
        let (mut gun, ev) = jam(0.1);
        assert!(ev.contains(&Event::Malfunctioned(Malfunction::FailureToEject)));
        assert!(!gun.is_closed(), "the case holds the bolt open");
        assert!(!fired(&shoot(&mut gun)));
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Spent)), "the case flies");
        assert!(gun.malfunction.is_none());
        assert!(fired(&shoot(&mut gun)));
    }

    #[test]
    fn a_failure_to_feed_clicks_and_wants_a_tap_before_the_rack() {
        let (mut gun, ev) = jam(0.6);
        assert!(ev.contains(&Event::Malfunctioned(Malfunction::FailureToFeed)));
        assert!(gun.is_closed());
        // Rack to cock it (an AK isn't double action): click, no bang.
        rack(&mut gun);
        let ev = shoot(&mut gun);
        assert!(ev.contains(&Event::DryFire), "click: {ev:?}");
        // Racking alone doesn't feed from a slipped magazine.
        let ev = rack(&mut gun);
        assert!(!ev.contains(&Event::Chambered));
        assert_eq!(gun.malfunction, Some(Malfunction::FailureToFeed));
        // Tap, rack, bang.
        assert_eq!(gun.tap_magazine(), vec![Event::MagazineInserted]);
        assert!(gun.tap_magazine().is_empty(), "already home");
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Chambered));
        assert!(gun.malfunction.is_none());
        assert!(fired(&shoot(&mut gun)));
    }

    #[test]
    fn a_double_feed_clears_only_with_the_magazine_out() {
        let (mut gun, ev) = jam(0.95);
        assert!(ev.contains(&Event::Malfunctioned(Malfunction::DoubleFeed)));
        let before = gun.live_rounds();
        // Rack all you like with the magazine in: nothing comes out.
        for _ in 0..3 {
            assert!(rack(&mut gun).is_empty());
        }
        assert!(!gun.is_closed());
        assert_eq!(gun.malfunction, Some(Malfunction::DoubleFeed));
        let mag = gun.remove_magazine().unwrap();
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Spent)));
        assert!(ev.contains(&Event::Ejected(Round::Live)));
        assert!(gun.malfunction.is_none());
        assert!(gun.is_closed());
        gun.insert_magazine(mag).unwrap();
        assert!(rack(&mut gun).contains(&Event::Chambered));
        assert!(fired(&shoot(&mut gun)));
        assert_eq!(gun.live_rounds(), before - 2, "one thrown out, one fired");
    }

    /// A hand takes the handle at rest, so its first pull asks for zero
    /// travel: a double-fed bolt still can't go home on the pair.
    #[test]
    fn a_double_fed_bolt_never_closes_on_the_pair() {
        let (mut gun, _) = jam(0.95);
        gun.remove_magazine();
        assert!(gun.set_bolt_travel(0.0).is_empty());
        assert!(!gun.is_closed());
        let ev = rack(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Spent)), "{ev:?}");
        assert!(ev.contains(&Event::Ejected(Round::Live)), "{ev:?}");
    }

    /// A slide locked back on an empty magazine, a fresh one in, and the
    /// slide tugged and ridden home by hand: it chambers, as the stop does.
    #[test]
    fn riding_a_locked_slide_home_chambers() {
        let mut gun = Firearm::new(pistol(), 1.0);
        gun.cycle_mode();
        gun.insert_magazine(mag("pm", 1)).unwrap();
        rack(&mut gun);
        assert!(fired(&shoot(&mut gun)));
        assert!(gun.locked_back);
        gun.remove_magazine();
        gun.insert_magazine(mag("pm", 8)).unwrap();
        let mut ev = Vec::new();
        for t in [0.95, 0.6, 0.3, 0.0] {
            ev.extend(gun.set_bolt_travel(t));
        }
        assert!(ev.contains(&Event::Chambered), "{ev:?}");
        // Or the slingshot: a tug off the stop and let go.
        assert!(fired(&shoot(&mut gun)));
        gun.remove_magazine();
        gun.insert_magazine(mag("pm", 0)).unwrap();
        while !gun.locked_back {
            shoot(&mut gun);
        }
        gun.remove_magazine();
        gun.insert_magazine(mag("pm", 8)).unwrap();
        gun.take_off_stop();
        assert!(gun.release_bolt().contains(&Event::Chambered));
    }

    /// Jams per hundred rounds, `n` rounds through a well-kept rifle as
    /// stiff and grimy as given, a trigger press each (each jam cleared by
    /// a rack); and its rounds a second on automatic.
    fn weathered_run(stiff: f32, grime: f32, n: u32) -> (f32, f32) {
        let mut gun = Firearm::new(rifle(), 1.0);
        gun.stiff = stiff;
        gun.grime = grime;
        while gun.fire_mode() != FireMode::Semi {
            gun.cycle_mode();
        }
        let mut rng = crate::noise::Rng::new(7);
        let mut roll = || rng.f32();
        let (mut shots, mut jams, mut presses) = (0u32, 0u32, 0u32);
        while shots < n {
            presses += 1;
            assert!(presses < 3 * n, "stuck: {gun:?}");
            if gun.rounds_in_feed() == 0 {
                let _ = gun.remove_magazine();
                gun.insert_magazine(mag("ak545", 30)).unwrap();
            }
            // (A dud leaves the hammer down on it: rack that out too.)
            if gun.malfunction.is_some() || gun.chambers[0].is_none() || !gun.hammers_cocked[0] {
                gun.malfunction = None;
                gun.mag_unseated = false;
                rack(&mut gun);
            }
            let mut ev = gun.update(0.2, true, &mut roll);
            ev.extend(gun.update(0.2, false, &mut roll));
            for e in &ev {
                match e {
                    Event::Fired { .. } => shots += 1,
                    Event::Malfunctioned(_) => jams += 1,
                    _ => {}
                }
            }
        }
        // A second of automatic from a fresh magazine.
        let mut auto = Firearm::new(rifle(), 1.0);
        auto.stiff = stiff;
        while auto.fire_mode() != FireMode::Auto {
            auto.cycle_mode();
        }
        auto.insert_magazine(mag("ak545", 30)).unwrap();
        rack(&mut auto);
        let mut lucky = lucky();
        let mut fired = 0;
        for _ in 0..1000 {
            fired += auto
                .update(0.001, true, &mut lucky)
                .iter()
                .filter(|e| matches!(e, Event::Fired { .. }))
                .count();
        }
        (jams as f32 * 100.0 / n as f32, fired as f32)
    }

    /// The weather's gauge: a warm, dry, well-kept rifle hardly jams; cold
    /// oil jams it more and slows it; grime from rain and ash jams it
    /// more still; a field wipe takes it most of the way back.
    #[test]
    fn weather_jams_a_gun_and_a_wipe_takes_it_back() {
        let n = 6000;
        let (clean, rate) = weathered_run(0.0, 0.0, n);
        let (cold, cold_rate) = weathered_run(1.0, 0.0, n);
        let (grimy, _) = weathered_run(0.0, 1.0, n);
        let (both, _) = weathered_run(1.0, 1.0, n);
        let mut wiped = Firearm::new(rifle(), 1.0);
        wiped.grime = 1.0;
        for _ in 0..4 {
            wiped.wipe(0.2);
        }
        let (after, _) = weathered_run(0.0, wiped.grime, n);
        println!(
            "jams per 100: clean {clean:.2}, cold {cold:.2}, grimy {grimy:.2}, both {both:.2}, \
             wiped {after:.2}; rounds/s clean {rate:.1}, cold {cold_rate:.1}"
        );
        assert!(clean < 0.5, "well kept: {clean}");
        assert!(cold > 2.0 * clean.max(0.1), "cold oil: {cold} vs {clean}");
        assert!(grimy > cold, "grime worse than cold: {grimy} vs {cold}");
        assert!(both > grimy, "both worse still: {both}");
        assert!(after < grimy * 0.4, "wiped down: {after} vs {grimy}");
        assert!(
            cold_rate < rate * 0.75,
            "stiff cycles slower: {cold_rate} vs {rate}"
        );
    }

    /// Misfires and hang-fires per hundred rounds of `load`, as cold as
    /// given, a trigger press each; and the longest a hang-fire took.
    fn primer_run(load: Load, cold: f32, n: u32) -> (f32, f32, f32) {
        let mut gun = Firearm::new(rifle(), 1.0);
        gun.ammo_cold = cold;
        while gun.fire_mode() != FireMode::Semi {
            gun.cycle_mode();
        }
        let mut rng = crate::noise::Rng::new(11);
        let mut roll = || rng.f32();
        let (mut shots, mut duds, mut hangs, mut longest) = (0u32, 0u32, 0u32, 0.0f32);
        let mut hung_at: Option<f32> = None;
        let mut t = 0.0;
        let mut ticks = 0u32;
        while shots < n {
            ticks += 1;
            assert!(ticks < 200 * n, "stuck: {gun:?}");
            if gun.hang.is_none()
                && (gun.chambers[0].is_none()
                    || !gun.hammers_cocked[0]
                    || gun.malfunction.is_some())
            {
                gun.malfunction = None;
                gun.mag_unseated = false;
                if gun.rounds_in_feed() == 0 {
                    let _ = gun.remove_magazine();
                    let mut m = mag("ak545", 30);
                    m.capacity = 30;
                    m.loads.fill(0, load, 30);
                    gun.insert_magazine(m).unwrap();
                }
                rack(&mut gun);
            }
            // Press, then wait out any hang-fire.
            let trigger = gun.hang.is_none() && ticks % 2 == 0;
            for e in gun.update(0.01, trigger, &mut roll) {
                match e {
                    Event::Fired { .. } => {
                        shots += 1;
                        if let Some(h) = hung_at.take() {
                            longest = longest.max(t - h);
                        }
                    }
                    Event::Dud => duds += 1,
                    Event::HangFire => {
                        hangs += 1;
                        hung_at = Some(t);
                    }
                    _ => {}
                }
            }
            t += 0.01;
        }
        let per = 100.0 / n as f32;
        (duds as f32 * per, hangs as f32 * per, longest)
    }

    /// The ammunition's gauge: fresh ball hardly ever misfires or hangs;
    /// surplus does now and then, damp a lot, a cold-soaked round more
    /// (the bench's restore makes them fresh ball again). Every hang-fire
    /// went within its second.
    #[test]
    fn ammo_condition_misfires_and_hang_fires() {
        let n = 4000;
        let fresh = primer_run(Load::Standard, 0.0, n);
        let surplus = primer_run(Load::Surplus, 0.0, n);
        let damp = primer_run(Load::Damp, 0.0, n);
        let cold = primer_run(Load::Standard, 1.0, n);
        let repacked = primer_run(Load::Repacked, 0.0, n);
        println!(
            "misfires/hangs per 100: fresh {:.2}/{:.2}, surplus {:.2}/{:.2}, damp {:.2}/{:.2}, \
             cold {:.2}/{:.2}, re-packed {:.2}/{:.2}; longest hang {:.2} s",
            fresh.0,
            fresh.1,
            surplus.0,
            surplus.1,
            damp.0,
            damp.1,
            cold.0,
            cold.1,
            repacked.0,
            repacked.1,
            damp.2.max(surplus.2).max(cold.2)
        );
        assert!(fresh.0 < 0.5 && fresh.1 == 0.0, "fresh: {fresh:?}");
        assert!(
            surplus.0 > 4.0 * fresh.0.max(0.2) && surplus.1 > 1.0,
            "surplus: {surplus:?}"
        );
        assert!(
            damp.0 > 2.0 * surplus.0 && damp.1 > surplus.1,
            "damp: {damp:?}"
        );
        assert!(cold.0 > fresh.0 && cold.1 > 2.0, "cold: {cold:?}");
        // (Scavenged re-packs don't hang; their trouble is the squib.)
        assert!(repacked.1 == 0.0, "re-packed: {repacked:?}");
        for r in [surplus, damp, cold] {
            assert!(r.2 <= HANG_MAX + 0.02, "a hang went {} s", r.2);
        }
    }

    /// A hang-fire: the hammer falls, nothing, then the shot a moment later;
    /// opened on before it goes, it goes off out of battery and costs the
    /// gun.
    #[test]
    fn a_hang_fire_goes_late_or_out_of_battery() {
        let hung = || {
            let mut gun = Firearm::new(mosin(), 1.0);
            gun.cycle_mode();
            gun.set_handle_lift(1.0);
            gun.set_bolt_travel(1.0);
            assert!(gun.load_round_of(Load::Damp));
            gun.set_bolt_travel(0.0);
            gun.set_handle_lift(0.0);
            // A roll that misses the misfire and hits the hang.
            let mut rolls = [0.99f32, 0.0, 0.5].into_iter();
            let ev = gun.update(0.01, true, &mut || rolls.next().unwrap_or(0.5));
            assert_eq!(ev, vec![Event::HangFire], "{gun:?}");
            gun
        };
        let mut gun = hung();
        let mut t = 0.0;
        let mut fired = None;
        while t < 2.0 && fired.is_none() {
            t += 0.01;
            if gun
                .update(0.01, false, &mut || 0.5)
                .iter()
                .any(|e| matches!(e, Event::Fired { .. }))
            {
                fired = Some(t);
            }
        }
        let fired = fired.expect("it went off");
        assert!(
            (HANG_MIN..=HANG_MAX + 0.02).contains(&fired),
            "{fired} s late"
        );

        let mut gun = hung();
        gun.update(0.01, false, &mut || 0.5);
        gun.set_handle_lift(1.0);
        let ev = gun.update(0.01, false, &mut || 0.5);
        assert_eq!(ev, vec![Event::OutOfBattery]);
        assert!(gun.condition < 1.0 - OUT_OF_BATTERY_WEAR + 0.01);
        assert!(gun.hang.is_none() && gun.chambers[0].is_none());
    }

    /// A short stroke holds the trigger dead until the bolt's worked
    /// fully.
    #[test]
    fn a_short_stroke_wants_a_full_stroke() {
        let mut gun = Firearm::new(mosin(), 1.0);
        gun.cycle_mode();
        assert_eq!(
            gun.short_stroke(),
            vec![Event::Malfunctioned(Malfunction::ShortStroke)]
        );
        assert!(shoot(&mut gun).is_empty(), "dead");
        gun.set_handle_lift(1.0);
        gun.set_bolt_travel(0.6);
        assert_eq!(gun.malfunction, Some(Malfunction::ShortStroke), "not yet");
        gun.set_bolt_travel(1.0);
        assert_eq!(gun.malfunction, None, "worked fully");
    }

    /// Wet, a box's plain rounds go damp one at a time; sealed AP doesn't.
    #[test]
    fn rounds_spoil_one_at_a_time() {
        let mut box_of = Loads::default();
        assert!(box_of.spoil(10, Load::Surplus, 0));
        assert_eq!(box_of.top(10), Load::Damp);
        assert!(box_of.spoil(10, Load::Surplus, 3));
        assert_eq!(box_of.0.iter().filter(|l| **l == Load::Damp).count(), 2);
        assert_eq!(box_of.0.len(), 10);
        let mut ap = Loads::default();
        assert!(!ap.spoil(5, Load::Ap, 0), "sealed");
        let mut mag = Loads::default();
        assert!(mag.spoil(30, Load::Standard, 29));
        assert_eq!(mag.pop_or(30, Load::Standard), Load::Standard);
    }

    /// A grimy gun fouls faster with each shot; the cleaning kit takes the
    /// grime with the fouling, a wipe only the grime.
    #[test]
    fn grime_fouls_faster_and_cleaning_takes_both() {
        let def = FirearmDef {
            fouling_per_shot: 0.004,
            ..rifle()
        };
        let foul = |grime: f32| {
            let mut gun = Firearm::new(def.clone(), 1.0);
            gun.grime = grime;
            gun.cycle_mode();
            gun.insert_magazine(mag("ak545", 30)).unwrap();
            rack(&mut gun);
            for _ in 0..20 {
                shoot(&mut gun);
            }
            gun
        };
        let dry = foul(0.0).fouling;
        let mut gun = foul(1.0);
        assert!(gun.fouling > 2.5 * dry, "{} vs {dry}", gun.fouling);
        let fouling = gun.fouling;
        gun.wipe(0.5);
        assert_eq!(gun.fouling, fouling, "a wipe leaves the bore");
        assert!((gun.grime - 0.5).abs() < 1e-6);
        gun.clean(1.0);
        assert_eq!((gun.fouling, gun.grime), (0.0, 0.0));
    }

    #[test]
    fn well_kept_guns_rarely_jam() {
        let mut gun = Firearm::new(rifle(), 1.0);
        assert!(gun.malfunction_chance() < 0.002);
        gun.condition = 0.5;
        let half = gun.malfunction_chance();
        assert!((0.02..0.06).contains(&half), "{half}");
        gun.repair(0.45, 0.95);
        assert!(gun.malfunction_chance() < half / 4.0, "the bench helps");
    }

    /// An SKS-pattern rifle: semi-auto, ten-round fixed magazine charged
    /// from clips through the locked-back action.
    #[test]
    fn a_fixed_magazine_self_loader_takes_clips_and_holds_open() {
        let mut gun = Firearm::new(
            FirearmDef {
                magazine_family: None,
                internal_capacity: 10,
                double_action: false,
                ..pistol()
            },
            1.0,
        );
        gun.cycle_mode();
        gun.set_bolt_travel(1.0);
        assert!(
            gun.release_bolt().contains(&Event::BoltLocked),
            "held open, empty"
        );
        assert_eq!(gun.strip_clip(10), 10);
        assert!(gun.release_slide_stop().contains(&Event::Chambered));
        for _ in 0..10 {
            assert!(fired(&shoot(&mut gun)));
        }
        assert!(gun.locked_back, "locks back on the last round");
        assert!(gun.load_round(), "thumbs one in");
        assert_eq!(gun.internal_rounds, 1);
    }

    fn nagant() -> FirearmDef {
        FirearmDef {
            action: ActionKind::Revolver,
            magazine_family: None,
            internal_capacity: 7,
            modes: vec![FireMode::Semi],
            last_round_hold_open: false,
            double_action: true,
            gate_offset: 5,
            ..pistol()
        }
    }

    fn ppsh() -> FirearmDef {
        FirearmDef {
            magazine_family: Some("ppsh_drum".into()),
            modes: vec![FireMode::Safe, FireMode::Auto, FireMode::Semi],
            rpm: 900.0,
            last_round_hold_open: false,
            double_action: false,
            open_bolt: true,
            ..pistol()
        }
    }

    /// A Nagant: gate open, a round in at the gate and a turn, seven
    /// times; gate shut; seven shots, by thumb-cocking and by the long
    /// pull; gate open, the rod pushes the empties out one at a turn.
    #[test]
    fn a_revolver_loads_at_the_gate_and_fires_round_the_cylinder() {
        let mut gun = Firearm::new(nagant(), 1.0);
        assert_eq!(gun.chambers.len(), 7);
        assert!(!gun.load_round(), "gate shut");
        assert!(!gun.set_gate(true).is_empty());
        for _ in 0..7 {
            assert!(gun.load_round());
            assert!(!gun.load_round(), "that chamber's full");
            gun.rotate_cylinder();
        }
        assert!(!fired(&shoot(&mut gun)), "won't fire with the gate open");
        gun.set_gate(false);
        assert!(gun.rotate_cylinder().is_empty(), "locked with it shut");
        // Single action: thumb back (the cylinder turns), then a light pull.
        assert_eq!(gun.cock_hammer(), vec![Event::Cocked]);
        assert!(fired(&shoot(&mut gun)));
        // Double action for the rest.
        for _ in 0..6 {
            assert!(fired(&shoot(&mut gun)));
        }
        let ev = shoot(&mut gun);
        assert!(ev.contains(&Event::DryFire), "all seven gone: {ev:?}");
        assert!(gun.set_gate(true).is_empty() || gun.gate_open);
        gun.set_gate(true);
        let mut out = 0;
        for _ in 0..7 {
            out += gun
                .eject_at_gate()
                .iter()
                .filter(|e| **e == Event::Ejected(Round::Spent))
                .count();
            gun.rotate_cylinder();
        }
        assert_eq!(out, 7);
        assert_eq!(gun.live_rounds(), 0);
    }

    /// A PPSh: rack it back onto the sear; the trigger runs the bolt home
    /// and it fires, ending back on the sear; on auto it runs on at 900
    /// rounds a minute; empty, it closes on nothing and wants racking.
    #[test]
    fn an_open_bolt_fires_as_it_closes() {
        let mut gun = Firearm::new(ppsh(), 1.0);
        gun.insert_magazine(Magazine {
            family: "ppsh_drum".into(),
            capacity: 71,
            rounds: 71,
            ..Default::default()
        })
        .unwrap();
        gun.cycle_mode(); // Auto
        gun.cycle_mode(); // Semi
        assert!(!fired(&shoot(&mut gun)), "bolt forward: not cocked");
        assert!(gun.release_slide_stop().is_empty());
        gun.set_bolt_travel(1.0);
        assert_eq!(gun.release_bolt(), vec![Event::BoltLocked]);
        assert_eq!(gun.chambers[0], None, "nothing chambered at rest");
        let ev = shoot(&mut gun);
        assert!(fired(&ev) && ev.contains(&Event::Ejected(Round::Spent)));
        assert!(gun.locked_back, "back on the sear");
        assert_eq!(gun.rounds_in_feed(), 70);
        // Racking it again while it sits on the sear loses no round.
        let before = gun.live_rounds();
        gun.set_bolt_travel(0.4);
        gun.set_bolt_travel(1.0);
        assert_eq!(gun.release_bolt(), vec![Event::BoltLocked]);
        assert_eq!(
            gun.live_rounds(),
            before,
            "a rack on the sear strips nothing"
        );
        gun.cycle_mode();
        gun.cycle_mode(); // Auto
        let mut r = lucky();
        let mut shots = 0;
        for _ in 0..60 {
            shots += gun
                .update(1.0 / 60.0, true, &mut r)
                .iter()
                .filter(|e| matches!(e, Event::Fired { .. }))
                .count();
        }
        assert!((14..=16).contains(&shots), "{shots} in a second");
        // Run it dry.
        for _ in 0..600 {
            gun.update(1.0 / 60.0, true, &mut r);
        }
        assert_eq!(gun.rounds_in_feed(), 0);
        assert!(!gun.locked_back && gun.is_closed(), "closed on nothing");
        // A handle let go short of the sear rides home, no shot.
        gun.remove_magazine();
        gun.insert_magazine(mag("ppsh_drum", 8)).unwrap();
        gun.set_bolt_travel(0.5);
        assert!(!gun.release_bolt().iter().any(|e| *e == Event::Chambered));
        gun.set_bolt_travel(1.0);
        gun.release_bolt();
        assert!(fired(&shoot(&mut gun)));
    }

    #[test]
    fn cleaning_and_repair() {
        let mut gun = Firearm::new(pistol(), 0.5);
        gun.fouling = 0.8;
        gun.clean(0.5);
        assert!((gun.fouling - 0.3).abs() < 1e-6);
        gun.repair(0.9, 0.8);
        assert!((gun.condition - 0.8).abs() < 1e-6);
        gun.repair(0.1, 0.6);
        assert!((gun.condition - 0.8).abs() < 1e-6, "repair never lowers");
    }

    fn pump() -> FirearmDef {
        FirearmDef {
            action: ActionKind::Pump,
            cartridge: "12ga".into(),
            magazine_family: None,
            internal_capacity: 4,
            modes: vec![FireMode::Safe, FireMode::Semi],
            double_action: false,
            last_round_hold_open: false,
            ..pistol()
        }
    }

    /// One full pump: back to the stop, forward home.
    fn pump_once(gun: &mut Firearm) -> Vec<Event> {
        let mut ev = gun.set_bolt_travel(1.0);
        ev.extend(gun.set_bolt_travel(0.0));
        ev
    }

    /// Four shells thumbed into the tube, one pumped into the chamber and a
    /// fifth topped up: five shots, each one pumped, the hull thrown out
    /// on the back stroke. The forend stays where the hand leaves it.
    #[test]
    fn a_pump_feeds_from_its_tube_and_cycles_by_hand() {
        let mut gun = Firearm::new(pump(), 1.0);
        gun.cycle_mode();
        for _ in 0..4 {
            assert!(gun.load_round());
        }
        assert!(!gun.load_round(), "the tube holds four");
        let ev = pump_once(&mut gun);
        assert!(ev.contains(&Event::Chambered), "{ev:?}");
        assert!(gun.load_round(), "top the tube up");
        assert_eq!(gun.live_rounds(), 5);
        for n in (0..5).rev() {
            assert!(fired(&shoot(&mut gun)), "shot {}", 5 - n);
            assert!(!fired(&shoot(&mut gun)), "a pump doesn't cycle itself");
            let ev = pump_once(&mut gun);
            assert!(ev.contains(&Event::Ejected(Round::Spent)), "{ev:?}");
            assert_eq!(gun.live_rounds(), n);
        }
        // Pumped on an empty tube it's cocked and locked; a click frees it.
        assert!(gun.set_bolt_travel(1.0).is_empty());
        assert!(shoot(&mut gun).contains(&Event::DryFire));
        gun.set_bolt_travel(0.6);
        assert!(gun.release_bolt().is_empty());
        assert_eq!(gun.bolt_travel, 0.6, "left half open");
    }

    /// Pumped onto a live shell, the action is locked by the cocked
    /// hammer: the forend won't come back until the action release is
    /// pressed, and then it throws the live shell out.
    #[test]
    fn a_pump_is_locked_on_a_cocked_hammer_until_released() {
        let mut gun = Firearm::new(pump(), 1.0);
        gun.load_round();
        gun.load_round();
        pump_once(&mut gun);
        assert!(gun.set_bolt_travel(1.0).is_empty());
        assert_eq!(gun.bolt_travel, 0.0, "locked shut");
        gun.release_action();
        let ev = pump_once(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Live)), "{ev:?}");
        assert!(ev.contains(&Event::Chambered));
        assert!(!gun.action_released, "closing locks it again");
        assert_eq!(gun.bolt_travel, 0.0);
        assert!(gun.set_bolt_travel(1.0).is_empty());
    }

    /// A short stroke lets the next shell onto the carrier without
    /// throwing out the hull: forward, the two jam and the forend stops
    /// short. Pumped hard to the rear, the hull goes and the shell feeds.
    #[test]
    fn a_short_stroke_jams_until_pumped_hard() {
        let mut gun = Firearm::new(pump(), 1.0);
        gun.cycle_mode();
        for _ in 0..3 {
            gun.load_round();
        }
        pump_once(&mut gun);
        assert!(fired(&shoot(&mut gun)));
        gun.set_bolt_travel(0.75);
        let ev = gun.set_bolt_travel(0.0);
        assert!(
            ev.contains(&Event::Malfunctioned(Malfunction::DoubleFeed)),
            "{ev:?}"
        );
        assert!(gun.bolt_travel >= DOUBLE_FEED_AT, "stopped short");
        assert!(!gun.is_closed());
        assert!(!fired(&shoot(&mut gun)));
        let ev = pump_once(&mut gun);
        assert!(ev.contains(&Event::Ejected(Round::Spent)), "{ev:?}");
        assert!(ev.contains(&Event::Chambered), "{ev:?}");
        assert_eq!(gun.malfunction, None);
        assert!(fired(&shoot(&mut gun)));
        assert_eq!(gun.live_rounds(), 1, "nothing lost");
    }

    /// A single-shot bolt rifle (a TOZ-8): the round goes straight into
    /// the open chamber; close, fire, open throws the case out.
    #[test]
    fn a_single_shot_bolt_loads_into_its_chamber() {
        let def = FirearmDef {
            internal_capacity: 0,
            rimfire: true,
            ..mosin()
        };
        let mut gun = Firearm::new(def, 1.0);
        assert!(!gun.load_round(), "bolt shut");
        gun.set_handle_lift(1.0);
        gun.set_bolt_travel(1.0);
        assert!(gun.load_round());
        assert!(!gun.load_round(), "one at a time");
        // Jiggled at the back, the bolt doesn't throw the round out.
        gun.set_bolt_travel(0.9);
        gun.set_bolt_travel(1.0);
        assert_eq!(gun.chambers[0], Some(Round::Live));
        gun.set_bolt_travel(0.0);
        gun.set_handle_lift(0.0);
        assert!(fired(&shoot(&mut gun)));
        gun.set_handle_lift(1.0);
        let ev = gun.set_bolt_travel(1.0);
        assert!(ev.contains(&Event::Ejected(Round::Spent)), "{ev:?}");
    }

    /// Rimfire misfires: the round stays; lift the bolt and drop it, and
    /// it strikes again.
    #[test]
    fn a_rimfire_misfire_is_struck_again() {
        let def = FirearmDef {
            internal_capacity: 0,
            rimfire: true,
            ..mosin()
        };
        let mut gun = Firearm::new(def, 1.0);
        gun.set_handle_lift(1.0);
        gun.set_bolt_travel(1.0);
        gun.load_round();
        gun.set_bolt_travel(0.0);
        gun.set_handle_lift(0.0);
        // A roll inside the rimfire's extra chance.
        let ev = gun.update(0.016, true, &mut rolls(&[0.02]));
        gun.update(0.016, false, &mut lucky());
        assert_eq!(ev, vec![Event::Dud]);
        assert!(!fired(&shoot(&mut gun)), "hammer down");
        gun.set_handle_lift(1.0);
        gun.set_handle_lift(0.0);
        assert!(fired(&shoot(&mut gun)), "struck again");
    }

    fn crossbow() -> FirearmDef {
        FirearmDef {
            action: ActionKind::Crossbow,
            cartridge: "bolt".into(),
            magazine_family: None,
            modes: vec![FireMode::Safe, FireMode::Semi],
            double_action: false,
            last_round_hold_open: false,
            ..pistol()
        }
    }

    /// A crossbow: drawn short and let go, the string slaps home; drawn
    /// to the latch it catches and stays; a bolt laid on, and the trigger
    /// looses it, leaving nothing behind. Loosed with no bolt, the dry
    /// loose costs the bow.
    #[test]
    fn a_crossbow_draws_latches_and_looses() {
        let mut bow = Firearm::new(crossbow(), 1.0);
        bow.cycle_mode();
        assert!(!bow.load_round(), "not drawn");
        bow.set_bolt_travel(0.6);
        assert_eq!(bow.release_bolt(), vec![Event::BoltClosed]);
        assert!(!bow.locked_back && bow.bolt_travel == 0.0, "slapped home");
        bow.set_bolt_travel(1.0);
        assert_eq!(bow.release_bolt(), vec![Event::Cocked]);
        assert!(bow.locked_back);
        bow.set_bolt_travel(0.2);
        assert_eq!(bow.bolt_travel, 1.0, "on the latch it stays");
        assert!(bow.load_round());
        assert!(!bow.load_round(), "one bolt");
        let ev = shoot(&mut bow);
        assert!(fired(&ev));
        assert!(!ev.iter().any(|e| matches!(e, Event::Ejected(_))));
        assert_eq!(bow.chambers[0], None);
        assert!(!bow.locked_back);
        assert!(!fired(&shoot(&mut bow)), "not drawn: nothing");
        let before = bow.condition;
        bow.set_bolt_travel(1.0);
        bow.release_bolt();
        assert_eq!(shoot(&mut bow), vec![Event::DryFire]);
        assert!(
            before - bow.condition >= DRY_LOOSE * 0.99,
            "the dry loose cost it"
        );
    }

    /// A damp signal cartridge misfires and stays in; broken open, it
    /// comes out by hand and another goes in.
    #[test]
    fn a_damp_flare_misfires_and_is_swapped() {
        let def = FirearmDef {
            cartridge: "26mm".into(),
            barrels: 1,
            ..double()
        };
        let mut gun = Firearm::new(def, 1.0);
        gun.set_break_open(true);
        assert!(gun.load_round());
        gun.set_break_open(false);
        // A roll inside the paper's extra chance.
        let ev = gun.update(0.016, true, &mut rolls(&[0.05]));
        gun.update(0.016, false, &mut lucky());
        assert_eq!(ev, vec![Event::Dud]);
        assert_eq!(gun.chambers[0], Some(Round::Live), "still in");
        gun.set_break_open(true);
        assert_eq!(gun.unload_chamber(0), Some(Round::Live), "out by hand");
        assert!(gun.load_round());
        gun.set_break_open(false);
        assert!(fired(&shoot(&mut gun)));
    }

    /// A sound, clean action runs free; a worn one drags, a fouled worn
    /// one more; cold oil drags whatever the wear.
    #[test]
    fn a_worn_dry_action_drags() {
        let mut gun = Firearm::new(rifle(), 1.0);
        assert_eq!(gun.action_drag(), 0.0, "sound and clean");
        gun.condition = 0.7;
        assert_eq!(gun.action_drag(), 0.0, "a little worn still runs free");
        gun.condition = 0.2;
        let worn = gun.action_drag();
        assert!(worn > 0.3, "{worn}");
        gun.fouling = 1.0;
        assert!(gun.action_drag() > worn + 0.2);
        let mut cold = Firearm::new(rifle(), 1.0);
        cold.stiff = 0.9;
        assert_eq!(cold.action_drag(), 0.9);
    }
}
