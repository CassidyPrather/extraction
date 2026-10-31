//! The raiding bot: an autonomous brain on top of the task-list [`Bot`].
//!
//! The [`Raider`] decides; the [`Bot`] does. Each time the bot runs out of
//! tasks (or something urgent happens — a robot opens fire, an anomaly
//! turns up on the route) the raider looks at the world and queues the
//! next few tasks: walk a path, open a door, grab that, stow it, shoot
//! that, use the radio. Everything still goes through `RigInput` and
//! `HandAction`, so a raid played by a raider is a raid a player could
//! have played.
//!
//! What it knows is what a player who'd studied the map would know: where
//! the buildings, loot markers, artifact fields and rendezvous are. What's
//! actually lying on the shelves, it finds out when it gets there. It sees
//! robots and anomalies only near it and in line of sight.

use avian3d::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use extraction_core::anomaly::AnomalyKind;
use extraction_core::items::{Category, ItemDef, SizeClass};
use extraction_core::map::{MapDef, MarkerKind};
use extraction_core::nav::{NavGrid, Obstacle};
use extraction_core::noise::Rng;
use serde_json::json;
use std::collections::{HashMap, HashSet};

use super::indoor::{BuildingPlan, plan_buildings};
use super::telemetry::{SimLog, round};
use crate::anomaly::Anomaly;
use crate::bot::{Bot, Task};
use crate::health::Health;
use crate::interact::body::{Backpack, InBackpack};
use crate::interact::{Hand, Held, InSocket, Socket, SocketKind, SocketOwner};
use crate::items::{Catalog, Item};
use crate::launch::LaunchOptions;
use crate::raid::truck::{Pressable, Truck};
use crate::raid::{Phase, Raid};
use crate::rig::{LocalPlayer, RigHand, Side};
use crate::robots::walker::Nav;
use crate::robots::{Awareness, Brain, Job, JobKind, Robot};
use crate::weapons::flat::MacroKind;
use crate::weapons::{Gun, Mag};
use crate::world::buildings::{Building, BuildingLoot, DoorLeaf};
use crate::world::{Layer, Zone};
use extraction_core::progression::Job as CardJob;

/// Who the bot is: what it values, what it risks.
#[derive(Clone, Debug)]
pub struct Traits {
    pub name: &'static str,
    /// Items valued below this are left behind.
    pub min_value: f32,
    /// Fraction of backpack bulk that sends it home.
    pub full_at: f32,
    /// Raid minutes it plans for.
    pub minutes: f32,
    /// Site preference multipliers.
    pub buildings: f32,
    pub spots: f32,
    pub fields: f32,
    /// Routes round anomalies it has seen.
    pub avoid_anomalies: bool,
    /// Opens fire on robots within this range that haven't noticed it.
    pub fight_range: f32,
    /// Slips away from robots that are only suspicious, rather than fight.
    pub evade: bool,
    /// Goes through wrecks it made.
    pub salvage: bool,
    /// Throws something into a snare before reaching into it.
    pub test_anomalies: bool,
    /// Carried kilograms that send it home (encumbrance bites past 20).
    pub max_kg: f32,
    /// How much artifact attention it will carry before heading home (the
    /// HUD's feeling: 1.5 "the air hums", 2.5 "the whole Zone is looking").
    pub heat: f32,
    /// How likely it is to get out of sight when a drone chirps its
    /// lock-on (`robots::drone::DroneLock`).
    pub duck: f32,
    /// Crouches (and so creeps) within this of a robot it can see that
    /// isn't fighting it (0: never). See [`sneak`].
    pub sneak_range: f32,
    /// Throws junk past a walker at its post to send it looking the other
    /// way.
    pub distract: bool,
    /// Creeps up on an unaware walker from outside its sight, to throw a
    /// distraction from close enough to be heard (see [`stalk`]).
    pub stalk: bool,
}

impl Traits {
    pub const NAMES: [&'static str; 4] = ["looter", "hunter", "cautious", "reckless"];

    pub fn named(name: &str) -> Self {
        let base = Traits {
            name: "looter",
            min_value: 0.0,
            full_at: 0.9,
            minutes: 13.0,
            buildings: 1.4,
            spots: 1.0,
            fields: 0.4,
            avoid_anomalies: true,
            fight_range: 0.0,
            evade: false,
            salvage: true,
            test_anomalies: true,
            max_kg: 26.0,
            heat: 1.6,
            duck: 0.75,
            sneak_range: 35.0,
            distract: true,
            stalk: false,
        };
        // (Everyone but the reckless fights only what comes for them.)
        match name {
            "hunter" => Traits {
                name: "hunter",
                min_value: 3.0,
                full_at: 0.8,
                minutes: 12.0,
                buildings: 0.5,
                spots: 0.5,
                fields: 4.0,
                heat: 2.2,
                ..base
            },
            "cautious" => Traits {
                name: "cautious",
                min_value: 2.0,
                full_at: 0.7,
                minutes: 10.0,
                buildings: 1.2,
                spots: 0.8,
                fields: 0.0,
                fight_range: 0.0,
                evade: true,
                salvage: false,
                max_kg: 18.0,
                heat: 1.0,
                duck: 0.9,
                sneak_range: 45.0,
                distract: true,
                // Off by default: it costs raids and the walkers spot it on
                // open ground (`SIM_STALK=1` turns it on to study it).
                stalk: std::env::var_os("SIM_STALK").is_some(),
                ..base
            },
            "reckless" => Traits {
                name: "reckless",
                min_value: 1.0,
                full_at: 1.0,
                minutes: 14.0,
                buildings: 1.0,
                spots: 1.0,
                fields: 1.5,
                avoid_anomalies: false,
                fight_range: 60.0,
                test_anomalies: false,
                max_kg: 34.0,
                heat: f32::INFINITY,
                duck: 0.3,
                sneak_range: 0.0,
                distract: false,
                ..base
            },
            _ => base,
        }
    }

    /// How much this bot wants an item.
    pub fn value(&self, def: &ItemDef) -> f32 {
        let v = match def.category {
            Category::Artifact => 10.0,
            Category::Firearm => 6.0,
            Category::Medical => 5.0,
            Category::Magazine | Category::Ammo | Category::Gadget => 4.0,
            Category::Tool | Category::RobotPart | Category::Attachment => 3.0,
            Category::Material | Category::Curio => 2.0,
            Category::Food => 1.5,
        };
        match (self.name, def.category) {
            ("hunter", Category::Artifact) => v * 3.0,
            ("cautious", Category::Artifact) => v * 0.3,
            _ => v,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Out of the truck bed, picking up the kit.
    KitUp,
    /// Looting.
    Raid,
    /// Heading for the rendezvous.
    Extract,
    /// Gone (extracted, dead or timed out).
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SiteKind {
    Building(usize),
    Spot,
    Field,
    /// A standing anomaly the valley is known for (a well, a mirror, a
    /// hush, a frost): it grows a stone some cycles, and a hunter goes to
    /// see.
    Anomaly,
    Wreck(Entity),
}

#[derive(Clone, Debug)]
struct Site {
    name: String,
    kind: SiteKind,
    at: Vec2,
    /// Loot spots it holds (for buildings), for scoring.
    spots: usize,
    visited: bool,
    tries: u8,
    /// The driver's job on the card sends it here (`progression`).
    job: bool,
}

/// Stuck this many times running near one spot, the raider gives up the
/// waypoint it was making for (blocks it in its own grid) and replans.
const STUCK_REPEATS: u32 = 3;
/// How much of its grid round that waypoint it gives up, metres.
const STUCK_BLOCK: f32 = 1.5;
/// Blocked this many waypoints round one spot, it's in a pocket (a ledge
/// it fell onto, a corner the grid can't see): it gives up wider discs and
/// steps further out of it.
const POCKET_BLOCKS: u32 = 3;
/// ...and this many, it gives up where it was going: another rendezvous,
/// or another site.
const POCKET_GIVE_UP: u32 = 6;
/// Health lost to nothing it can see before it gets out (see `calm`).
const HAZARD_HP: f32 = 8.0;

/// What the raider is in the middle of.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    Idle,
    Travel(usize),
    Search(usize),
    Take(usize, Entity),
    /// Heading for the rendezvous's approach point.
    ToRv,
    /// Climbing into the truck and pulling the bell.
    Board,
}

/// An ongoing fight, for the telemetry.
#[derive(Clone, Debug)]
pub struct Combat {
    pub target: Entity,
    pub kind: String,
    pub started: f32,
    pub at: Vec2,
    pub shots_at_start: u32,
    pub hits_at_start: u32,
    /// Seconds the target has been out of sight.
    pub unseen: f32,
    /// Shots fired when the last burst began, to notice a gun that clicks.
    pub burst_shots: u32,
    pub dry_bursts: u32,
    /// Rounds stopped by walls etc. when the last burst began.
    pub blocked_at_burst: u32,
}

impl Combat {
    pub fn close(&mut self, log: &mut SimLog, outcome: &str) {
        let shots = log.stats.shots - self.shots_at_start;
        let hits = log.stats.hits - self.hits_at_start;
        log.event(
            "engage_end",
            json!({"robot": self.kind, "outcome": outcome, "shots": shots, "hits": hits,
                   "secs": round(log.t - self.started, 1),
                   "x": round(self.at.x, 1), "z": round(self.at.y, 1)}),
        );
    }
}

#[derive(Component)]
pub struct Raider {
    pub traits: Traits,
    pub stage: Stage,
    pub combat: Option<Combat>,
    /// Deliberately walking into an anomaly (for an artifact).
    pub diving: bool,
    /// Why it last left a stone lying in an anomaly (for the log).
    pass_why: Option<String>,
    rng: Rng,
    grid: NavGrid,
    plans: Vec<BuildingPlan>,
    sites: Vec<Site>,
    act: Act,
    /// Items it gave up on or dropped.
    skip: HashSet<Entity>,
    /// Anomalies it has seen.
    known: HashSet<Entity>,
    /// Anomalies that have had it (it walked into one without meaning
    /// to): never again, whatever its nerve.
    burned: HashSet<Entity>,
    /// Doors a lockdown has bolted for good: shut in its plans.
    sealed: HashSet<Entity>,
    /// The current plan was cut short (a fight, a repath): redo, don't
    /// give up.
    interrupted: bool,
    pack_full: bool,
    pub rv: Option<usize>,
    radioed_at: f32,
    /// Stuck detection: where it was, and since when.
    anchor: (Vec2, f32),
    /// Since when (raid seconds) it's been losing health with nothing
    /// hitting it or bleeding, and its health then.
    calm: (f32, f32),
    /// Where it last got stuck, and how many times in a row near there.
    stuck_spot: (Vec2, u32),
    /// Where it has been blocking waypoints, and how many round there.
    pocket: (Vec2, u32),
    scan: f32,
    clock: f32,
    retreat_until: f32,
    /// Off a plough's road till then, waiting for it to go by.
    aside_until: f32,
    /// When it last ducked into cover (a drone's lock-on, or hurt).
    ducked_at: f32,
    /// Its gun, once it has one.
    gun: Option<Entity>,
    /// Stows refused (a full pack).
    refusals: u32,
    last_refill: f32,
    /// `SIM_DEBUG` is set: log every plan.
    debug: bool,
    /// Health and carried kilograms as of this frame.
    hp: f32,
    kg: f32,
    /// Attention from the artifacts it carries.
    heat: f32,
    /// Crouched since (raid seconds), when it is.
    crouched_since: Option<f32>,
    /// Waiting at the rendezvous for the truck (it keeps low and still).
    at_rv: bool,
    /// Where it waits for the truck at rendezvous `.0` while the drones
    /// survey a place nearby: under the shelter's roof, or in its lee
    /// (`map_lint::wait_shelter`), out of their eye till the truck stops.
    wait_at: Option<(usize, Option<Vec2>)>,
    /// Gone to vent a flue: the lever, and the way on (to the loot and
    /// taking it) once it's open.
    after_vent: Option<(Entity, Vec<Task>)>,
    /// What worth having was on it last tick (`knocked_loose`).
    kept: Vec<Entity>,
    /// When it last froze for a searching robot, or threw a distraction.
    froze_at: f32,
    distracted_at: f32,
    /// Creeping up on a walker to throw a distraction past it.
    stalking: Option<Stalk>,
    /// `SIM_DIVERT=1`: its distractions go into the wildlife (a stone onto
    /// a flock's roof, among a herd) where it can (see [`wild_mark`]).
    divert: bool,
    /// Calm walkers that could see it, being watched for whether they get
    /// onto it (see [`watch`]); and when each was last watched.
    watches: Vec<Watch>,
    watched: HashMap<Entity, f32>,
    /// The region it set out for, read off the field map like a player
    /// planning a raid: sites there look better than their distance says.
    objective: Option<Vec2>,
}

/// A calm walker within [`WATCH_NEAR`] with a clear line to it, that
/// could see it: did it get onto it in the next
/// [`WATCH_FOR`] seconds?
#[derive(Clone, Copy, Debug)]
struct Watch {
    walker: Entity,
    since: f32,
    spotted: bool,
    /// It threw into the wildlife meanwhile.
    wild: bool,
}

/// How long a watched stretch is (s)...
const WATCH_FOR: f32 = 30.0;
/// ...that starts with a calm walker this near with a clear line to it
/// (a walker's sight, m).
const WATCH_NEAR: f32 = 75.0;

/// A creep-and-throw in progress (see [`stalk`]).
#[derive(Clone, Copy, Debug)]
struct Stalk {
    walker: Entity,
    /// The junk it means to throw.
    item: Entity,
    /// Where the junk is meant to land.
    landing: Vec2,
    started: f32,
    /// When the throw went (its tasks ran out), if it has.
    thrown: Option<f32>,
}

impl Raider {
    pub fn stage_name(&self) -> &'static str {
        match self.stage {
            Stage::KitUp => "kit_up",
            Stage::Raid => "raid",
            Stage::Extract => "extract",
            Stage::Done => "done",
        }
    }

    pub fn doing(&self) -> &'static str {
        if self.combat.is_some() {
            return "fight";
        }
        match self.act {
            Act::Idle => "idle",
            Act::Travel(_) => "travel",
            Act::Search(_) => "search",
            Act::Take(..) => "take",
            Act::ToRv => "to_rv",
            Act::Board => "board",
        }
    }
}

/// Puts the local player under a raider's control (sim runs only).
pub fn attach_raider(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Res<Zone>,
    nav: Option<Res<Nav>>,
    players: Query<Entity, With<LocalPlayer>>,
    buildings: Query<(Entity, &Building, &Transform)>,
    doors: Query<(Entity, &DoorLeaf, &ChildOf)>,
    loot: Option<Res<BuildingLoot>>,
    vents: Query<&crate::world::furnace::VentLever>,
    mut log: ResMut<SimLog>,
) {
    let Some(run) = &opts.sim_run else {
        return;
    };
    let Some(nav) = nav else {
        error!("sim: no navigation grid");
        return;
    };
    let mut traits = Traits::named(opts.personality.as_deref().unwrap_or("looter"));
    if let Some(m) = opts.sim_minutes {
        traits.minutes = m;
    }
    let mut plans = plan_buildings(&buildings, &doors);
    for plan in &mut plans {
        plan.shut_live();
    }
    // Flues already vented are a way.
    vent_plans(
        &mut plans,
        vents.iter().filter(|v| v.open).map(|v| (v.site, v.vent)),
    );
    let mut sites: Vec<Site> = Vec::new();
    let mut site_of_plan = vec![None; plans.len()];
    let spots: Vec<Vec3> = loot
        .map(|l| l.0.iter().map(|s| s.at).collect())
        .unwrap_or_default();
    for (i, plan) in plans.iter().enumerate() {
        let n = spots.iter().filter(|s| plan.contains(**s, 0.5)).count();
        // A boxcar's floor is a climb up from the ground, which the indoor
        // grid doesn't walk (as `indoor`'s reach check skips it): not a
        // site, or it stands at the door reaching in.
        if n == 0 || plan.entries.is_empty() || plan.name == "boxcar" {
            continue;
        }
        site_of_plan[i] = Some(sites.len());
        sites.push(Site {
            name: plan.name.clone(),
            kind: SiteKind::Building(i),
            at: plan.xf.translation.xz(),
            spots: n,
            visited: false,
            tries: 0,
            job: false,
        });
    }
    for m in &zone.map.markers {
        match &m.kind {
            MarkerKind::Loot { .. } => {
                let p = zone.on_ground(m.at[0], m.at[1]);
                // (Up in a boxcar: no more a spot than the boxcar's a site.)
                if plans
                    .iter()
                    .any(|pl| pl.name == "boxcar" && pl.contains(p, 0.5))
                {
                    continue;
                }
                if let Some(s) = plans
                    .iter()
                    .position(|pl| pl.contains(p, 0.5))
                    .and_then(|i| site_of_plan[i])
                {
                    sites[s].spots += 1;
                    continue;
                }
                sites.push(Site {
                    name: if m.name.is_empty() {
                        "loot".into()
                    } else {
                        m.name.clone()
                    },
                    kind: SiteKind::Spot,
                    at: Vec2::from_array(m.at),
                    spots: 1,
                    visited: false,
                    tries: 0,
                    job: false,
                });
            }
            // (`SIM_NO_ANOMALY_SITES=1`: as before hunters knew them, to
            // compare runs.)
            MarkerKind::Anomaly { .. } if std::env::var_os("SIM_NO_ANOMALY_SITES").is_none() => {
                sites.push(Site {
                    name: m.name.clone(),
                    kind: SiteKind::Anomaly,
                    at: Vec2::from_array(m.at),
                    spots: 1,
                    visited: false,
                    tries: 0,
                    job: false,
                })
            }
            MarkerKind::ArtifactField => sites.push(Site {
                name: m.name.clone(),
                kind: SiteKind::Field,
                at: Vec2::from_array(m.at),
                spots: 6,
                visited: false,
                tries: 0,
                job: false,
            }),
            _ => {}
        }
    }
    if std::env::var_os("SIM_DUMP_PLANS").is_some() {
        if let Some(dir) = log.path.parent() {
            super::indoor::dump_plans(&plans, &spots, &dir.join("plans"));
        }
    }
    let seed = opts.seed;
    let (objective, career_rv) = match career_habit() {
        Some(h) => h.choose(&zone.map, &sites),
        None => (choose_objective(&zone.map, &sites, seed), None),
    };
    log.event(
        "start",
        json!({"run": run, "map": opts.map, "seed": seed, "personality": traits.name,
               "minutes": traits.minutes, "format": super::telemetry::FORMAT,
               "sites": sites.len(), "buildings": plans.len(),
               "objective": objective.map(|o| [round(o.x, 1), round(o.y, 1)])}),
    );
    info!(
        "sim: {} raider, {} sites ({} buildings planned)",
        traits.name,
        sites.len(),
        plans.len()
    );
    let mut kit = kit_tasks();
    // `SIM_CONTRACT`: the contract's kit lies with the rest (`sim::mod`).
    // On Rime Fell, the hearth the sim kit lays, sealed: the career's from
    // "Up the fell", the way into the frost.
    // (The case, with the hearth seated in it: `sim::seat_sim_hearth`.)
    let hearth = (opts.map == "rime_fell").then_some("lead_case");
    for id in contract_kit()
        .into_iter()
        .chain(super::harvest::kit(traits.name))
        .chain(hearth)
    {
        kit.push(Task::PickUp {
            id: id.to_owned(),
            side: Side::Left,
        });
        kit.push(Task::Stow(Side::Left));
    }
    for p in &players {
        commands.entity(p).insert((
            Bot::new(kit.clone()),
            Raider {
                traits: traits.clone(),
                stage: Stage::KitUp,
                combat: None,
                diving: false,
                rng: Rng::new(seed ^ 0x5133),
                grid: nav.0.as_ref().clone(),
                plans: plans.clone(),
                sites: sites.clone(),
                act: Act::Idle,
                skip: HashSet::new(),
                known: HashSet::new(),
                burned: HashSet::new(),
                sealed: HashSet::new(),
                interrupted: false,
                pack_full: false,
                rv: career_rv,
                radioed_at: -999.0,
                anchor: (Vec2::ZERO, 0.0),
                calm: (0.0, 100.0),
                stuck_spot: (Vec2::ZERO, 0),
                pocket: (Vec2::ZERO, 0),
                scan: 0.0,
                clock: 0.0,
                retreat_until: 0.0,
                aside_until: 0.0,
                ducked_at: -99.0,
                gun: None,
                refusals: 0,
                last_refill: -999.0,
                debug: std::env::var_os("SIM_DEBUG").is_some(),
                hp: 100.0,
                kg: 0.0,
                heat: 0.0,
                crouched_since: None,
                at_rv: false,
                wait_at: None,
                after_vent: None,
                kept: Vec::new(),
                pass_why: None,
                froze_at: -99.0,
                distracted_at: -99.0,
                stalking: None,
                divert: std::env::var("SIM_DIVERT").is_ok_and(|v| v == "1"),
                watches: Vec::new(),
                watched: HashMap::new(),
                objective,
            },
        ));
    }
}

/// Out of the truck bed and kitted up from what's on the ground by the
/// insertion point (the dev cache and the sim kit): an AK with two
/// magazines, a box of rounds, the radio, two dressings.
/// `SIM_CONTRACT=<id>[:step]`: the contract's kit (a relay to plant; the
/// camera, though the raider can't take pictures).
pub fn contract_kit() -> Vec<&'static str> {
    let Ok(v) = std::env::var("SIM_CONTRACT") else {
        return Vec::new();
    };
    let id = v.split(':').next().unwrap_or_default();
    extraction_core::contracts::contract(id)
        .map(|c| {
            c.kit
                .iter()
                .flat_map(|(id, n)| std::iter::repeat_n(*id, *n as usize))
                .collect()
        })
        .unwrap_or_default()
}

fn kit_tasks() -> Vec<Task> {
    let s = |x: &str| x.to_string();
    let mut t = vec![Task::Log(s("kitting up"))];
    t.push(Task::PickUp {
        id: s("ak74"),
        side: Side::Right,
    });
    for id in [
        "ak_mag",
        "ak_mag",
        "radio",
        "field_dressing",
        "field_dressing",
        "ammo_545",
    ] {
        t.push(Task::PickUp {
            id: s(id),
            side: Side::Left,
        });
        t.push(Task::Stow(Side::Left));
    }
    t.push(Task::Gun(MacroKind::Reload));
    // Safe -> auto -> semi: aimed single shots.
    t.push(Task::Gun(MacroKind::Mode));
    t.push(Task::Gun(MacroKind::Mode));
    t
}

/// Steps out of the truck bed first (the rear is +Z, truck space).
pub fn leave_bed(truck: &GlobalTransform) -> Vec<Task> {
    let p = |z: f32| {
        let w = truck.transform_point(Vec3::new(0.0, 0.0, z));
        Vec2::new(w.x, w.z)
    };
    vec![
        Task::WalkTo {
            to: p(2.6),
            within: 0.4,
        },
        Task::WalkTo {
            to: p(5.5),
            within: 0.8,
        },
    ]
}

#[derive(SystemParam)]
pub struct Scene<'w, 's> {
    time: Res<'w, Time>,
    raid: Res<'w, Raid>,
    zone: Res<'w, Zone>,
    catalog: Res<'w, Catalog>,
    hands: Query<'w, 's, (&'static RigHand, &'static Hand)>,
    items: Query<
        'w,
        's,
        (
            Entity,
            &'static Item,
            &'static GlobalTransform,
            Option<&'static Held>,
            Option<&'static InSocket>,
            Option<&'static InBackpack>,
        ),
    >,
    sockets: Query<'w, 's, (&'static Socket, &'static SocketOwner)>,
    bags: Query<'w, 's, &'static Backpack>,
    guns: Query<'w, 's, &'static Gun>,
    mags: Query<'w, 's, (&'static Item, &'static Mag)>,
    robots: Query<
        'w,
        's,
        (
            Entity,
            &'static Robot,
            &'static GlobalTransform,
            &'static Awareness,
            &'static Brain,
            &'static Job,
        ),
    >,
    anomalies: Query<'w, 's, (Entity, &'static Anomaly, &'static GlobalTransform)>,
    trucks: Query<'w, 's, (&'static Truck, &'static GlobalTransform)>,
    pressables: Query<'w, 's, (Entity, &'static Pressable)>,
    /// Underground sites' power: doors held shut, and the switches.
    power_locked: Query<'w, 's, &'static crate::world::underground::PowerLocked>,
    site_power: Query<'w, 's, &'static crate::world::underground::SitePower>,
    power_switches: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::world::underground::PowerSwitch,
            &'static GlobalTransform,
        ),
    >,
    ploughs: Query<
        'w,
        's,
        (
            &'static crate::robots::plough::Plough,
            &'static Robot,
            &'static GlobalTransform,
        ),
    >,
    /// What a stuck raider is up against, by name (telemetry).
    names: Query<'w, 's, (&'static Name, Option<&'static ChildOf>)>,
    vent_levers: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::world::furnace::VentLever,
            &'static GlobalTransform,
        ),
    >,
    attention: Query<'w, 's, &'static crate::anomaly::Attention>,
    breeds: Query<'w, 's, &'static crate::robots::Breed>,
    spatial: SpatialQuery<'w, 's>,
    progression: Option<Res<'w, crate::progression::Progression>>,
    opts: Res<'w, LaunchOptions>,
    house: Option<Res<'w, crate::progression::safehouse::Safehouse>>,
    /// The bots' sockets by entity (`Task::Draw` counts them in entity
    /// order).
    owned_sockets: Query<'w, 's, (Entity, &'static SocketOwner), With<Socket>>,
    /// The wildlife, for throwing into ([`wild_mark`]).
    flocks: Query<'w, 's, &'static crate::wildlife::flock::Flock>,
    herds: Query<'w, 's, &'static crate::wildlife::herd::Herd>,
    perches: Option<Res<'w, crate::wildlife::perches::Perches>>,
}

impl Scene<'_, '_> {
    /// The raid's clock less the time slept in it: a morning in the
    /// safehouse is a fresh start, not a raid gone late.
    fn awake(&self) -> f32 {
        self.raid.clock - self.house.as_ref().map_or(0.0, |h| h.slept)
    }

    fn hand(&self, children: &Children, side: Side) -> Option<Entity> {
        children.iter().find_map(|c| {
            self.hands
                .get(c)
                .ok()
                .filter(|(rh, _)| rh.side == side)
                .and_then(|(_, h)| h.holding)
        })
    }

    /// Whether this item is on me: a body slot, or my worn backpack.
    fn mine(&self, me: Entity, item: Entity) -> bool {
        let Ok((_, _, _, _, sock, bag)) = self.items.get(item) else {
            return false;
        };
        sock.is_some_and(|s| self.sockets.get(s.socket).is_ok_and(|(_, o)| o.0 == me))
            || bag.is_some_and(|b| {
                self.bags
                    .get(b.bag)
                    .is_ok_and(|b| b.owner == me && b.carried)
            })
    }

    /// My things with this id (on the body or in the pack).
    fn carried(&self, me: Entity, id: &str) -> Vec<Entity> {
        self.items
            .iter()
            .filter(|(e, it, ..)| self.catalog.get(it.kind()).id == id && self.mine(me, *e))
            .map(|(e, ..)| e)
            .collect()
    }

    fn pack(&self, me: Entity) -> Option<&Backpack> {
        self.bags.iter().find(|b| b.owner == me && b.carried)
    }

    /// Backpack bulk used and capacity.
    fn pack_use(&self, me: Entity) -> (f32, f32) {
        let Some(b) = self.pack(me) else {
            return (0.0, 1.0);
        };
        let used = b
            .contents
            .iter()
            .filter_map(|e| self.items.get(*e).ok())
            .map(|(_, it, ..)| self.catalog.get(it.kind()).bulk)
            .sum();
        (used, b.model.capacity)
    }

    fn free_slings(&self, me: Entity) -> usize {
        self.sockets
            .iter()
            .filter(|(s, o)| {
                o.0 == me
                    && s.occupant.is_none()
                    && s.kind == SocketKind::Body(extraction_core::inventory::SlotKind::Sling)
            })
            .count()
    }

    /// Rounds in the gun, and in my spare magazines for it.
    fn ammo(&self, me: Entity, gun: Option<Entity>) -> (u32, u32) {
        let Some(g) = gun.and_then(|g| self.guns.get(g).ok()) else {
            return (0, 0);
        };
        let family = g.firearm.def.magazine_family.clone().unwrap_or_default();
        let spare = self
            .items
            .iter()
            .filter(|(e, ..)| self.mine(me, *e))
            .filter_map(|(e, ..)| self.mags.get(e).ok())
            .filter(|(_, m)| m.family == family)
            .map(|(it, _)| it.rounds)
            .sum();
        (g.firearm.live_rounds(), spare)
    }

    /// A clear line from the eye to a point, and from a hand's breadth
    /// lower (a window sill lets you see what you can't shoot).
    fn visible(&self, from: Vec3, to: Vec3) -> bool {
        self.ray_clear(from, to) && self.ray_clear(from - Vec3::Y * 0.25, to)
    }

    /// What's in the way walking from `from` toward `to`, within 1.5 m,
    /// at the knee or the chest: its name and its parent's, for the
    /// stuck event. Empty if a ray meets nothing.
    fn in_the_way(&self, from: Vec3, to: Vec2) -> String {
        let flat = Vec3::new(to.x - from.x, 0.0, to.y - from.z);
        let Ok(dir) = Dir3::new(flat) else {
            return String::new();
        };
        let name = |e: Entity| {
            self.names.get(e).ok().map(|(n, p)| {
                let up = p
                    .and_then(|p| self.names.get(p.parent()).ok())
                    .map(|(n, _)| n.as_str().to_string())
                    .unwrap_or_default();
                format!("{n}<{up}")
            })
        };
        // Down its middle and at either shoulder.
        let side = Vec3::new(-dir.z, 0.0, dir.x) * 0.25;
        [0.1, 0.25, 0.4, 0.8, 1.2]
            .iter()
            .flat_map(|h| [(*h, Vec3::ZERO), (*h, side), (*h, -side)])
            .filter_map(|(h, off)| {
                self.spatial
                    .cast_ray(
                        from + off + Vec3::Y * h,
                        dir,
                        1.5,
                        true,
                        &SpatialQueryFilter::from_mask([Layer::World, Layer::Robot, Layer::Item]),
                    )
                    .map(|hit| (h, off.length() > 0.0, hit))
            })
            .map(|(h, shoulder, hit)| {
                let at = if shoulder { "s" } else { "" };
                format!(
                    "{} @{:.2} h{h}{at}",
                    name(hit.entity).unwrap_or_else(|| format!("{:?}", hit.entity)),
                    hit.distance
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Nothing solid between the two points, all the way.
    fn path_clear(&self, from: Vec3, to: Vec3) -> bool {
        let d = to - from;
        let Ok(dir) = Dir3::new(d) else {
            return true;
        };
        self.spatial
            .cast_ray(
                from,
                dir,
                d.length() + 0.3,
                true,
                &SpatialQueryFilter::from_mask(Layer::World),
            )
            .is_none()
    }

    fn ray_clear(&self, from: Vec3, to: Vec3) -> bool {
        let d = to - from;
        let len = d.length();
        let Ok(dir) = Dir3::new(d) else {
            return true;
        };
        self.spatial
            .cast_ray(
                from,
                dir,
                (len - 0.8).max(0.0),
                true,
                &SpatialQueryFilter::from_mask(Layer::World),
            )
            .is_none()
    }
}

/// The raider's think step: react, then plan when the bot runs dry.
pub fn think(
    mut bots: Query<
        (
            Entity,
            &mut Bot,
            &mut Raider,
            &Transform,
            &Health,
            &crate::player::Body,
            &Children,
            &mut crate::rig::RigInput,
        ),
        With<LocalPlayer>,
    >,
    scene: Scene,
    mut log: ResMut<SimLog>,
    mut locks: MessageReader<crate::robots::drone::DroneLock>,
    mut vented: MessageReader<crate::world::furnace::Vented>,
) {
    let dt = scene.time.delta_secs();
    let locks: Vec<Entity> = locks.read().map(|l| l.drone).collect();
    let vented: Vec<(Entity, usize)> = vented.read().map(|v| (v.site, v.vent)).collect();
    for (me, mut bot, mut r, tf, health, body, children, mut input) in &mut bots {
        let r = &mut *r;
        // A flue vented: its heat's cells are a way again.
        vent_plans(&mut r.plans, vented.iter().copied());
        if r.stage == Stage::Done || health.down {
            continue;
        }
        r.clock += dt;
        r.hp = health.hp;
        r.kg = body.carried_kg;
        r.heat = scene.attention.get(me).map_or(0.0, |a| a.carried);
        let pos = tf.translation;
        let here = pos.xz();
        let eye = pos + Vec3::Y * 1.6;
        // What's in the right hand, and its own gun wherever it is (slung
        // between fights: a long gun carried in one hand snags on door
        // frames).
        let gun = scene.hand(children, Side::Right);
        let own_gun = r.gun.or(gun);

        // Leaving the truck bed comes before everything.
        if r.stage == Stage::KitUp && r.clock < dt * 1.5 {
            if let Some((_, t)) = scene.trucks.iter().next() {
                let mut tasks = leave_bed(t);
                tasks.extend(bot.tasks.drain(..));
                bot.replace(tasks);
            }
        }

        // --- React: a drone's lock-on chirp. ---------------------------
        for drone in &locks {
            if let Some(tasks) = duck(me, r, &scene, &mut log, pos, *drone) {
                r.interrupted = true;
                bot.replace(tasks);
                bot.sprint = true;
            }
        }

        // A plough coming up its road with it in the lane: off the road.
        if r.clock > r.aside_until
            && let Some((off, wait)) = scene.ploughs.iter().find_map(|(p, robot, gt)| {
                let running = !robot.wrecked
                    && p.jammed <= 0.0
                    && matches!(
                        p.run.leg,
                        extraction_core::plough::Leg::Up(_) | extraction_core::plough::Leg::Down(_)
                    );
                running
                    .then(|| {
                        plough_aside(
                            &r.grid,
                            here,
                            gt.translation().xz(),
                            gt.forward().as_vec3().xz(),
                        )
                    })
                    .flatten()
            })
        {
            r.aside_until = r.clock + wait;
            log.event(
                "plough_aside",
                json!({"x": round(here.x, 1), "z": round(here.y, 1), "to_x": round(off.x, 1), "to_z": round(off.y, 1)}),
            );
            r.interrupted = true;
            bot.sprint = true;
            bot.replace([
                Task::WalkTo {
                    to: off,
                    within: 0.6,
                },
                Task::Wait(wait),
            ]);
        }

        // --- React: robots. -------------------------------------------
        r.scan -= dt;
        if r.scan <= 0.0 && r.stage != Stage::KitUp {
            r.scan = 0.25;
            react_to_robots(
                me,
                &mut bot,
                r,
                &scene,
                &mut log,
                eye,
                pos,
                gun,
                health.since_hit < 4.0,
            );
            if r.stage == Stage::Done {
                continue;
            }
            spot_anomalies(&mut bot, r, &scene, here);
            spot_sealed(&mut bot, r, &scene, &mut log);
            spot_held(&mut bot, r, &scene);
            sneak(
                me,
                &mut bot,
                r,
                &scene,
                &mut log,
                eye,
                body.swimming,
                &mut input,
            );
        }

        // Stuck: walking somewhere and getting nowhere.
        if bot.walk.is_some() && r.combat.is_none() {
            if here.distance(r.anchor.0) > 1.0 {
                r.anchor = (here, r.clock);
            } else if r.clock - r.anchor.1 > 6.0 {
                r.anchor = (here, r.clock);
                log.event(
                    "stuck",
                    json!({"x": round(here.x, 1), "y": round(pos.y, 1), "z": round(here.y, 1),
                    "ground": round(scene.zone.ground(here.x, here.y), 1),
                    "doing": r.doing(), "task": format!("{:?}", bot.tasks.front()),
                           "then": format!("{:?}", bot.tasks.get(1)),
                    // (Toward the walk it's on, past a step aside.)
                    "by": match (bot.tasks.front(), bot.tasks.get(1)) {
                        (Some(Task::WalkTo { within, .. }), Some(Task::WalkTo { to, .. }))
                            if *within == crate::bot::SIDESTEP => scene.in_the_way(pos, *to),
                        (Some(Task::WalkTo { to, .. }), _) => scene.in_the_way(pos, *to),
                        _ => String::new(),
                    }}),
                );
                // Stuck again and again in one place: the waypoint it's
                // making for can't be reached from here, whatever the grid
                // says. Block it in its own grid so the next plan goes round.
                r.stuck_spot = if here.distance(r.stuck_spot.0) < 4.0 {
                    (r.stuck_spot.0, r.stuck_spot.1 + 1)
                } else {
                    (here, 1)
                };
                let mut step = 3.0;
                if r.stuck_spot.1 >= STUCK_REPEATS
                    && let Some(Task::WalkTo { to, .. }) = bot.tasks.front()
                {
                    r.pocket = if here.distance(r.pocket.0) < 6.0 {
                        (r.pocket.0, r.pocket.1 + 1)
                    } else {
                        (here, 1)
                    };
                    let radius = if r.pocket.1 >= POCKET_BLOCKS {
                        step = 8.0;
                        STUCK_BLOCK * 2.5
                    } else {
                        STUCK_BLOCK
                    };
                    r.grid.block(
                        extraction_core::nav::Obstacle::Circle {
                            at: to.to_array(),
                            radius,
                        },
                        0.0,
                    );
                    log.event(
                        "stuck_block",
                        json!({"x": round(to.x, 1), "z": round(to.y, 1), "pocket": r.pocket.1}),
                    );
                    r.stuck_spot.1 = 0;
                    if r.pocket.1 >= POCKET_GIVE_UP {
                        give_up_goal(r, &scene, here, &mut log);
                        r.pocket.1 = 0;
                    }
                }
                open_heat_here(r, pos, &mut log);
                let a = r.rng.range(0.0, std::f32::consts::TAU);
                let side = here + Vec2::new(a.cos(), a.sin()) * step;
                // Give up on what it was reaching for; sidestep; replan.
                if let Act::Take(_, item) = r.act {
                    r.skip.insert(item);
                }
                if let Act::Travel(s) = r.act {
                    r.sites[s].tries += 1;
                }
                r.interrupted = true;
                bot.replace([Task::WalkTo {
                    to: side,
                    within: 0.8,
                }]);
            }
        } else {
            r.anchor = (here, r.clock);
        }

        // Hurting with nothing hitting it and no wound bleeding: heat, gas,
        // whatever its plan didn't know of. Out of the building, and that
        // site crossed off.
        if health.since_hit < 3.0 || health.bleeding > 0.05 || r.hp > r.calm.1 {
            r.calm = (r.clock, r.hp);
        } else if r.calm.1 - r.hp > HAZARD_HP
            && r.combat.is_none()
            && {
                open_heat_here(r, pos, &mut log);
                true
            }
            && let Some(tasks) = leave_building(r, &scene, pos)
        {
            log.event(
                "hazard",
                json!({"x": round(here.x, 1), "y": round(pos.y, 1), "z": round(here.y, 1),
                       "lost": round(r.calm.1 - r.hp, 0), "secs": round(r.clock - r.calm.0, 1)}),
            );
            r.calm = (r.clock, r.hp);
            if let Act::Travel(s) | Act::Search(s) | Act::Take(s, _) = r.act {
                r.sites[s].tries = r.sites[s].tries.max(3);
            }
            r.act = Act::Idle;
            r.interrupted = true;
            bot.replace(tasks);
        }

        if let Some(tasks) = knocked_loose(me, r, &scene, &mut log, pos) {
            r.interrupted = true;
            bot.replace(tasks);
            continue;
        }

        if !bot.done() {
            continue;
        }

        // --- Plan. -----------------------------------------------------
        // Back from a vent lever: on through, if it did open.
        if let Some((lever, rest)) = r.after_vent.take()
            && r.combat.is_none()
            && scene.vent_levers.get(lever).is_ok_and(|(_, v, _)| v.open)
        {
            bot.replace(rest);
            continue;
        }
        if let Some(target) = r.combat.as_ref().map(|c| c.target) {
            // Between bursts: a dressing if it's bad, else keep at it.
            if let Some(tasks) = heal(me, r, &scene, health, children, &mut log, true) {
                bot.replace(tasks);
                continue;
            }
            let hit = health.since_hit < 2.0;
            fight_on(me, &mut bot, r, &scene, &mut log, eye, gun, target, hit);
            continue;
        }
        r.diving = false;
        bot.sprint = false;
        if let Some(tasks) = heal(me, r, &scene, health, children, &mut log, false) {
            bot.replace(tasks);
            continue;
        }
        if r.stage != Stage::KitUp {
            if let Some(tasks) = maintain(me, r, &scene, &mut log, pos, gun, children) {
                r.interrupted = true;
                bot.replace(tasks);
                continue;
            }
        }
        match r.stage {
            Stage::KitUp => {
                let loaded = gun
                    .and_then(|g| scene.guns.get(g).ok())
                    .map(|g| g.firearm.live_rounds())
                    .unwrap_or(0);
                log.event(
                    "kit",
                    json!({"gun": gun.is_some(), "rounds": loaded,
                           "radio": !scene.carried(me, "radio").is_empty()}),
                );
                r.gun = gun;
                // Sling it until it's wanted.
                bot.replace([Task::Stow(Side::Right), Task::Wait(0.2)]);
                r.stage = Stage::Raid;
                r.act = Act::Idle;
            }
            Stage::Raid => {
                // What it just took counts before it decides to go: a stone
                // grabbed as the heat tips it home is still loot.
                if settle_take(me, r, &scene, &mut log, pos) {
                    if let Act::Take(s, _) = r.act {
                        r.act = Act::Search(s);
                    }
                }
                if let Some(why) = time_to_go(me, r, &scene, health, own_gun) {
                    log.stats.extract_reason = Some(why.to_string());
                    log.event(
                        "decide_extract",
                        json!({"why": why, "clock": round(scene.raid.clock, 1)}),
                    );
                    r.stage = Stage::Extract;
                    r.act = Act::Idle;
                    continue;
                }
                let tasks = raid_step(me, r, &scene, &mut log, here, pos, children);
                bot.sprint = r.diving;
                if r.debug {
                    log.event(
                        "plan",
                        json!({"tasks": format!("{tasks:?}"), "x": here.x, "z": here.y}),
                    );
                }
                bot.replace(tasks);
            }
            Stage::Extract => {
                let tasks = extract_step(me, r, &scene, &mut log, here, pos, children);
                // Hurt, it hurries.
                bot.sprint = r.hp < 50.0;
                if r.debug {
                    log.event(
                        "plan",
                        json!({"tasks": format!("{tasks:?}"), "x": here.x, "z": here.y}),
                    );
                }
                bot.replace(tasks);
            }
            Stage::Done => {}
        }
        r.interrupted = false;
    }
}

/// In heat its plan shuts (a fight took it in, or it's stuck by it):
/// the heat opened in that plan, so the way out can be planned.
fn open_heat_here(r: &mut Raider, pos: Vec3, log: &mut SimLog) {
    if let Some(plan) = r
        .plans
        .iter_mut()
        .find(|p| p.in_grid(pos) && (p.indoors(p.to_local(pos).xz()) || p.contains(pos, -0.1)))
    {
        let local = plan.to_local(pos).xz();
        if plan.open_heat_at(local) {
            log.event(
                "heat_open",
                json!({"site": plan.name, "x": round(pos.x, 1), "z": round(pos.z, 1)}),
            );
        }
    }
}

/// Caught in a pocket it can't plan its way out of: give up the goal that
/// keeps drawing it back in. Heading out, it picks the next rendezvous
/// instead; travelling to a site, it crosses the site off.
fn give_up_goal(r: &mut Raider, scene: &Scene, here: Vec2, log: &mut SimLog) {
    match r.act {
        Act::ToRv | Act::Board => {
            let mut rvs: Vec<(usize, f32)> = scene
                .zone
                .map
                .markers_of(&MarkerKind::Rendezvous)
                .enumerate()
                .filter(|(i, _)| Some(*i) != r.rv)
                .map(|(i, m)| (i, Vec2::from_array(m.at).distance(here)))
                .collect();
            rvs.sort_by(|a, b| a.1.total_cmp(&b.1));
            if let Some(&(next, _)) = rvs.first() {
                log.event("pocket_rv", json!({"from": r.rv, "to": next}));
                r.rv = Some(next);
            }
        }
        Act::Travel(s) | Act::Search(s) => {
            r.sites[s].tries = r.sites[s].tries.max(3);
            log.event("pocket_site", json!({"site": s}));
        }
        _ => {}
    }
}

/// Why it's time to head out, if it is.
fn time_to_go(
    me: Entity,
    r: &Raider,
    scene: &Scene,
    health: &Health,
    gun: Option<Entity>,
) -> Option<&'static str> {
    if scene.awake() > r.traits.minutes * 60.0 {
        return Some("time");
    }
    let (used, cap) = scene.pack_use(me);
    if r.pack_full || used >= cap * r.traits.full_at || r.kg > r.traits.max_kg {
        return Some("full");
    }
    // Carrying artifacts, the robots keep coming: hurt is hurt sooner.
    let heat = r.heat;
    if health.hp < 30.0 || (heat > 0.0 && health.hp < 50.0) {
        return Some("hurt");
    }
    if heat >= r.traits.heat {
        return Some("heat");
    }
    let meds = !scene.carried(me, "field_dressing").is_empty()
        || !scene.carried(me, "regen_injector").is_empty();
    if health.hp < 35.0 && !meds {
        return Some("hurt");
    }
    let (loaded, spare) = scene.ammo(me, gun);
    if loaded + spare == 0 {
        return Some("no_ammo");
    }
    if !r.sites.iter().any(|s| !s.visited && s.tries < 3) {
        return Some("done");
    }
    None
}

/// Dress wounds when bleeding or low (and not fighting).
fn heal(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    health: &Health,
    children: &Children,
    log: &mut SimLog,
    urgent_only: bool,
) -> Option<Vec<Task>> {
    let (bleed, low) = if urgent_only {
        (1.0, 35.0)
    } else {
        (0.4, 45.0)
    };
    if health.bleeding < bleed && health.hp > low {
        return None;
    }
    if scene.hand(children, Side::Left).is_some() {
        return None;
    }
    let item = ["regen_injector", "field_dressing"]
        .iter()
        .find_map(|id| scene.carried(me, id).first().copied())?;
    let id = scene
        .items
        .get(item)
        .map(|(_, it, ..)| scene.catalog.get(it.kind()).id.clone())
        .unwrap_or_default();
    log.stats.heals += 1;
    log.event(
        "heal",
        json!({"item": id, "hp": round(health.hp, 0), "bleeding": round(health.bleeding, 2)}),
    );
    let _ = r;
    Some(vec![
        Task::Grab {
            item,
            side: Side::Left,
        },
        Task::Use(Side::Left),
        Task::Wait(0.3),
    ])
}

/// Robots: start, continue or avoid a fight.
fn react_to_robots(
    me: Entity,
    bot: &mut Bot,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    eye: Vec3,
    pos: Vec3,
    gun: Option<Entity>,
    hurt: bool,
) {
    // Climbing into the truck, or the truck's waiting and it's close: just
    // go (running).
    let truck_close = matches!(scene.raid.phase, Phase::TruckWaiting { .. })
        && scene
            .trucks
            .iter()
            .any(|(_, t)| t.translation().xz().distance(eye.xz()) < 70.0);
    if (r.act == Act::Board || truck_close) && r.combat.is_none() {
        if truck_close {
            bot.sprint = true;
        }
        return;
    }
    if let Some(c) = &mut r.combat {
        // Keep tabs on the target between bursts.
        let gone = scene
            .robots
            .get(c.target)
            .map(|(_, rb, ..)| rb.wrecked)
            .unwrap_or(true);
        if gone {
            return;
        }
        let seen = scene
            .robots
            .get(c.target)
            .map(|(_, _, gt, ..)| scene.visible(eye, gt.translation()))
            .unwrap_or(false);
        c.unseen = if seen { 0.0 } else { c.unseen + 0.25 };
        return;
    }
    // Just ducked into cover: stay there a moment rather than step straight
    // back out to fight.
    if r.clock - r.ducked_at < 4.0 {
        return;
    }
    let (loaded, spare) = scene.ammo(me, r.gun.or(gun));
    let armed = loaded + spare > 0;
    let mut threat: Option<(Entity, f32, bool, String, Vec3)> = None;
    for (e, robot, gt, aware, brain, job) in &scene.robots {
        if robot.wrecked || job.kind == JobKind::Collect {
            continue;
        }
        let at = gt.translation();
        let d = at.distance(eye);
        if d > 60.0 {
            continue;
        }
        // Robots onto it get fought once they're close enough to hit
        // (a drone on the attack closes to circle at about 18 m).
        // Being shot at, it shoots back at anything in range; walkers hold
        // at about 35 m to fire.
        let close = d < 32.0
            || (hurt && d < 60.0)
            || (robot.kind == crate::robots::RobotKind::Walker && d < 50.0);
        let onto_me = aware.target == Some(me)
            && (matches!(brain, Brain::Engage) || aware.suspicion > 0.6)
            && close;
        if !onto_me && d > r.traits.fight_range.max(r.traits.evade as u8 as f32 * 45.0) {
            continue;
        }
        if !scene.visible(eye, at) {
            continue;
        }
        if threat.as_ref().is_none_or(|t| d < t.1) {
            let name = crate::robots::robot_name(robot.kind, scene.breeds.get(e).ok());
            threat = Some((e, d, onto_me, name, at));
        }
    }
    let Some((target, d, onto_me, kind, at)) = threat else {
        return;
    };
    // Creeping up on it to throw a distraction: neither back off nor fight.
    if r.stalking.is_some() && !onto_me {
        return;
    }
    if r.traits.evade && !onto_me {
        // Back off the way it came, out of sight.
        if r.clock > r.retreat_until {
            r.retreat_until = r.clock + 12.0;
            log.event("evade", json!({"robot": kind, "dist": round(d, 0)}));
            let shut = |d| scene.power_locked.contains(d);
            if let Some(tasks) = run_from(&r.plans, &r.grid, pos, at, 25.0, &shut) {
                r.interrupted = true;
                bot.replace(tasks);
                bot.sprint = true;
            }
        }
        return;
    }
    if !armed {
        // Nothing to fight with: run. Heading out already, run that way.
        if r.stage == Stage::Extract {
            bot.sprint = true;
            return;
        }
        if r.clock > r.retreat_until {
            r.retreat_until = r.clock + 15.0;
            log.event("flee", json!({"robot": kind, "dist": round(d, 0)}));
            let shut = |d| scene.power_locked.contains(d);
            if let Some(tasks) = run_from(&r.plans, &r.grid, pos, at, 40.0, &shut) {
                r.interrupted = true;
                bot.replace(tasks);
                bot.sprint = true;
            }
        }
        return;
    }
    log.stats.engagements += 1;
    log.event(
        "engage",
        json!({"robot": kind, "dist": round(d, 0), "noticed": onto_me,
               "x": round(eye.x, 1), "z": round(eye.z, 1), "rx": round(at.x, 1), "rz": round(at.z, 1)}),
    );
    r.combat = Some(Combat {
        target,
        kind,
        started: log.t,
        at: eye.xz(),
        shots_at_start: log.stats.shots,
        hits_at_start: log.stats.hits,
        unseen: 0.0,
        burst_shots: log.stats.shots,
        dry_bursts: 0,
        blocked_at_burst: log.stats.blocked,
    });
    r.interrupted = true;
    // Whatever's in the left hand goes down; both hands on the gun.
    let mut tasks = vec![Task::Release(Side::Left)];
    if let Some(g) = r.gun.filter(|g| gun != Some(*g)) {
        tasks.push(Task::Release(Side::Right));
        tasks.push(Task::Grab {
            item: g,
            side: Side::Right,
        });
    }
    tasks.extend([Task::Support, Task::Shoot { target, shots: 4 }]);
    bot.replace(tasks);
}

/// A walker this close that isn't onto it draws a thrown distraction (m).
const DISTRACT_RANGE: f32 = 45.0;

/// Moving like a careful player near robots (`robots::senses`: crouched
/// is harder to see, crouched footsteps carry a third as far, and crouching
/// slows the body to a creep):
/// - **crouch** within `sneak_range` of a robot it can see that isn't
///   fighting it, or near where a robot is looking for someone;
/// - **freeze** a few seconds, crouched and still, when a robot that's
///   onto something is investigating or searching close by;
/// - **distract** (all but the reckless): when a walker within
///   [`DISTRACT_RANGE`] is about but not onto it (at work, or looking for
///   someone), throw junk well off to one side, so the walker turns and
///   goes to look there instead.
///
/// Never in a fight, never with the truck waiting, never hurrying home hurt.
#[allow(clippy::too_many_arguments)]
fn sneak(
    me: Entity,
    bot: &mut Bot,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    eye: Vec3,
    swimming: bool,
    input: &mut crate::rig::RigInput,
) {
    watch(me, r, scene, log, eye);
    let hurrying = bot.sprint
        || r.combat.is_some()
        || r.act == Act::Board
        || matches!(scene.raid.phase, Phase::TruckWaiting { .. })
        || (r.stage == Stage::Extract && r.hp < 50.0);
    let mut wary = false;
    let mut freeze = false;
    let mut calm_walker: Option<Vec3> = None;
    if r.traits.stalk && !hurrying {
        if stalk(me, bot, r, scene, log, eye) {
            // Crouched all the way: creeping.
            if !input.crouch {
                input.crouch = true;
                r.crouched_since = Some(r.clock);
            }
            return;
        }
    } else if let Some(st) = r.stalking.take() {
        log.event(
            "distract_result",
            json!({"outcome": "broke_off", "why": "hurrying", "thrown": st.thrown.is_some()}),
        );
    }
    if r.traits.sneak_range > 0.0 && !hurrying {
        for (_, robot, gt, aware, brain, job) in &scene.robots {
            if robot.wrecked || job.kind == JobKind::Collect {
                continue;
            }
            let at = gt.translation();
            let d = at.distance(eye);
            let hunting = match brain {
                Brain::Investigate { at: look, .. } | Brain::Search { at: look, .. } => Some(*look),
                _ => None,
            };
            let near_look = hunting.is_some_and(|l| l.xz().distance(eye.xz()) < 25.0);
            if matches!(brain, Brain::Engage) && aware.target == Some(me) {
                continue;
            }
            if d > r.traits.sneak_range && !near_look {
                continue;
            }
            let seen = scene.visible(eye, at);
            wary |= seen || near_look;
            if seen && d < 30.0 && hunting.is_some() && aware.suspicion < 0.55 {
                freeze = true;
            }
            if seen
                && d < DISTRACT_RANGE
                && robot.kind == crate::robots::RobotKind::Walker
                && !matches!(brain, Brain::Engage | Brain::Leave { .. } | Brain::Down)
                && aware.suspicion < 0.55
            {
                calm_walker = Some(at);
            }
        }
    }
    // Afloat it ducks too, as a player does: a flooded doorway's head is
    // a hand over the water.
    // Waiting for the truck: low and still, as a player who's learnt
    // about drones does.
    let low = wary || swimming || (r.at_rv && !hurrying);
    if low != input.crouch {
        input.crouch = low;
        if low {
            r.crouched_since = Some(r.clock);
        } else if let Some(t) = r.crouched_since.take() {
            log.event("crouch", json!({"secs": round(r.clock - t, 1)}));
        }
    }
    // Freezing comes first; a distraction is for when it's had its look.
    if freeze && r.clock - r.froze_at > 10.0 {
        r.froze_at = r.clock;
        let secs = r.rng.range(2.0, 4.0);
        log.event(
            "freeze",
            json!({"secs": round(secs, 1), "x": round(eye.x, 1), "z": round(eye.z, 1)}),
        );
        r.interrupted = true;
        bot.replace([Task::Wait(secs)]);
        return;
    }
    if let Some(w) = calm_walker.filter(|_| r.traits.distract && r.clock - r.distracted_at > 60.0) {
        let Some(item) = junk(r, scene) else {
            if r.debug {
                log.event("distract_skip", json!({"why": "no junk"}));
            }
            return;
        };
        // A thrown bolt carries 15 m or so: well off to one side of the
        // line to it, so it turns and walks off that way, whichever side
        // there's ground to land on. Diverting, onto birds or among deer
        // there, if there are any.
        let toward = (w.xz() - eye.xz()).normalize_or(Vec2::X);
        let wild = if r.divert {
            wild_mark(scene, eye, w)
        } else {
            None
        };
        let side = [1.1f32, -1.1]
            .into_iter()
            .map(|a| eye.xz() + Vec2::from_angle(a).rotate(toward) * 15.0)
            .find(|p| r.grid.walkable([p.x, p.y]))
            .map(|p| scene.zone.on_ground(p.x, p.y));
        let Some(to) = wild.map(|(p, _)| p).or(side) else {
            if r.debug {
                log.event("distract_skip", json!({"why": "no ground"}));
            }
            return;
        };
        r.distracted_at = r.clock;
        if wild.is_some() {
            for wt in &mut r.watches {
                wt.wild = true;
            }
        }
        log.event(
            "distract",
            json!({"x": round(to.x, 1), "z": round(to.z, 1), "walker_dist": round(w.distance(eye), 0),
                   "wild": wild.map(|(_, k)| k)}),
        );
        r.interrupted = true;
        bot.replace([
            Task::Grab {
                item,
                side: Side::Left,
            },
            Task::Throw {
                side: Side::Left,
                at: to,
            },
            Task::Wait(2.0),
        ]);
    }
}

/// Somewhere in the wildlife to throw at, a throw off and well to one
/// side of the walker at `walker`: a sitting flock's roof, or among a
/// herd. What it is, for the log.
fn wild_mark(scene: &Scene, eye: Vec3, walker: Vec3) -> Option<(Vec3, &'static str)> {
    let toward = (walker - eye).xz().normalize_or(Vec2::X);
    let ok = |p: Vec3| {
        let d = (p - eye).xz();
        let l = d.length();
        (8.0..20.0).contains(&l) && toward.angle_to(d / l).abs() > 0.8
    };
    if let Some(perches) = &scene.perches {
        for f in &scene.flocks {
            if f.phase != crate::wildlife::flock::Phase::Settled {
                continue;
            }
            let s = &perches.sites[f.site];
            let p = s.centre.with_y(s.top());
            if ok(p) {
                return Some((p, "flock"));
            }
        }
    }
    scene
        .herds
        .iter()
        .filter_map(|h| h.leader().map(|b| b.pos))
        .find(|p| ok(*p))
        .map(|p| (p, "herd"))
}

/// Each watched walker, [`WATCH_FOR`] seconds on: did it get onto the bot
/// (`watch`: `unseen` or `spotted`, whether it was diverting, whether it
/// threw into the wildlife meanwhile).
fn watch(me: Entity, r: &mut Raider, scene: &Scene, log: &mut SimLog, eye: Vec3) {
    let clock = r.clock;
    for (e, robot, gt, aware, brain, _) in &scene.robots {
        let calm = robot.kind == crate::robots::RobotKind::Walker
            && !robot.wrecked
            && !matches!(brain, Brain::Engage | Brain::Leave { .. } | Brain::Down)
            && !(aware.target == Some(me) && aware.suspicion > 0.5);
        let at = gt.translation();
        if calm
            && at.distance(eye) < WATCH_NEAR
            && r.watched.get(&e).is_none_or(|t| clock - t > 60.0)
            && scene.visible(eye, at)
        {
            r.watched.insert(e, clock);
            r.watches.push(Watch {
                walker: e,
                since: clock,
                spotted: false,
                wild: false,
            });
        }
    }
    let divert = r.divert;
    r.watches.retain_mut(|wt| {
        let Ok((_, robot, _, aware, brain, _)) = scene.robots.get(wt.walker) else {
            return false;
        };
        if robot.wrecked {
            return false;
        }
        wt.spotted |=
            aware.target == Some(me) && (matches!(brain, Brain::Engage) || aware.suspicion > 0.5);
        if clock - wt.since < WATCH_FOR {
            return true;
        }
        log.event(
            "watch",
            json!({"outcome": if wt.spotted { "spotted" } else { "unseen" },
                   "divert": divert, "wild": wt.wild}),
        );
        false
    });
}

/// Unaware walkers this far off are worth creeping up on (m)...
const STALK_FROM: f32 = 70.0;
/// ...to this far from them, behind them...
const STALK_TO: f32 = 10.0;
/// ...outside their sight: more than this off their facing (radians; a
/// walker sees 200 degrees round)...
const STALK_BEHIND: f32 = 2.0;
/// ...by way of a point this far out behind them...
const STALK_WIDE: f32 = 25.0;
/// ...then throw it this far past them, the far side from the bot (m).
const STALK_PAST: f32 = 5.0;

/// The classic distraction, played properly (the stalking personality
/// only): see a walker that hasn't noticed you, creep round behind it
/// (crouched, from outside its sight cone) to within a throw, and lob junk
/// past it so it walks off to look there. Telemetry: `stalk` when it sets
/// off, `distract` at the throw, `distract_result` 8 s after (`drawn`: the
/// walker went to look; `spotted`: it turned on the bot instead;
/// `ignored`; or `broke_off`). True while it's at it.
fn stalk(
    me: Entity,
    bot: &mut Bot,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    eye: Vec3,
) -> bool {
    if let Some(mut st) = r.stalking {
        let Ok((_, robot, gt, aware, brain, _)) = scene.robots.get(st.walker) else {
            r.stalking = None;
            return false;
        };
        let onto_me =
            aware.target == Some(me) && (matches!(brain, Brain::Engage) || aware.suspicion > 0.5);
        if robot.wrecked || onto_me || r.combat.is_some() {
            let outcome = if onto_me { "spotted" } else { "broke_off" };
            log.event(
                "distract_result",
                json!({"outcome": outcome, "why": if onto_me { "seen" } else { "fight" },
                       "thrown": st.thrown.is_some()}),
            );
            r.stalking = None;
            return false;
        }
        // Thrown once it's left the hand (not held, not in the pack).
        let gone = scene
            .items
            .get(st.item)
            .map_or(true, |(_, _, _, held, _, bag)| {
                held.is_none() && bag.is_none()
            });
        if st.thrown.is_none() && gone {
            st.thrown = Some(r.clock);
            log.event(
                "distract",
                json!({"x": round(st.landing.x, 1), "z": round(st.landing.y, 1),
                       "walker_dist": round(gt.translation().distance(eye), 0)}),
            );
        }
        if let Some(t) = st.thrown {
            if r.clock - t > 8.0 {
                let drawn = match brain {
                    Brain::Investigate { at, .. } | Brain::Search { at, .. } => {
                        at.xz().distance(st.landing) < 6.0
                    }
                    _ => false,
                };
                log.event(
                    "distract_result",
                    json!({"outcome": if drawn { "drawn" } else { "ignored" }, "thrown": true}),
                );
                r.stalking = None;
                r.interrupted = true;
                return false;
            }
            r.stalking = Some(st);
            // Stay down and still while it goes to look.
            if bot.done() {
                bot.replace([Task::Wait(1.0)]);
            }
            return true;
        }
        // Its tasks ran out without a throw (a duck from a drone, a step
        // round something it was stuck on): give it up, and try again soon.
        let late = r.clock - st.started > 90.0;
        if late || (st.thrown.is_none() && bot.done()) {
            log.event(
                "distract_result",
                json!({"outcome": "broke_off", "why": if late { "too_long" } else { "interrupted" },
                       "thrown": false}),
            );
            r.stalking = None;
            r.interrupted = true;
            if !late {
                r.distracted_at = r.clock - 60.0;
            }
            return false;
        }
        r.stalking = Some(st);
        return true;
    }
    if r.stage != Stage::Raid || r.clock - r.distracted_at < 90.0 {
        return false;
    }
    let skip = |r: &mut Raider, log: &mut SimLog, why: &str| {
        if r.debug {
            log.event("stalk_skip", json!({"why": why}));
        }
        false
    };
    // Any walker in sight close enough to be worth it?
    let walker_near = scene.robots.iter().any(|(_, robot, gt, ..)| {
        !robot.wrecked
            && robot.kind == crate::robots::RobotKind::Walker
            && gt.translation().distance(eye) < STALK_FROM
            && scene.visible(eye, gt.translation())
    });
    if !walker_near {
        return false;
    }
    let Some(item) = junk(r, scene) else {
        return skip(r, log, "no junk");
    };
    // An unaware walker in sight.
    let pick = scene
        .robots
        .iter()
        .find(|(_, robot, gt, aware, brain, job)| {
            !robot.wrecked
            && robot.kind == crate::robots::RobotKind::Walker
            && job.kind != JobKind::Collect
            && !matches!(brain, Brain::Engage | Brain::Leave { .. } | Brain::Down)
            // Not onto it: not looking its way, at most wondering.
            && aware.suspicion < 0.35
            && !(aware.target == Some(me) && aware.can_see_target)
            && gt.translation().distance(eye) < STALK_FROM
            && gt.translation().distance(eye) > STALK_TO + 2.0
            && scene.visible(eye, gt.translation())
        });
    let Some((walker, _, gt, ..)) = pick else {
        return skip(r, log, "aware");
    };
    let w = gt.translation().xz();
    let facing = {
        let f = gt.forward().as_vec3().xz();
        f.normalize_or(Vec2::NEG_Y)
    };
    // Behind it, walkable, and the way there: nearest first.
    let mut best: Option<(f32, Vec2, Vec<Task>)> = None;
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let dir = Vec2::from_angle(a);
        if dir.angle_to(facing).abs() < STALK_BEHIND {
            continue;
        }
        let spot = w + dir * STALK_TO;
        if !r.grid.walkable([spot.x, spot.y]) {
            continue;
        }
        let d = spot.distance(eye.xz());
        if best.as_ref().is_some_and(|b| b.0 <= d) {
            continue;
        }
        // Round the back at a distance first (out of its sight), then in.
        let wide = w + dir * STALK_WIDE;
        let around = if r.grid.walkable([wide.x, wide.y]) {
            travel(&r.grid, eye.xz(), wide, 2.0).and_then(|mut t| {
                t.extend(travel(&r.grid, wide, spot, 1.0)?);
                Some(t)
            })
        } else {
            travel(&r.grid, eye.xz(), spot, 1.0)
        };
        if let Some(t) = around {
            best = Some((d, spot, t));
        }
    }
    let Some((d, spot, mut tasks)) = best else {
        return skip(r, log, "no way round");
    };
    let landing = w + (w - spot).normalize_or(Vec2::X) * STALK_PAST;
    if !r.grid.walkable([landing.x, landing.y]) {
        return skip(r, log, "no landing");
    }
    r.distracted_at = r.clock;
    log.event(
        "stalk",
        json!({"dist": round(d, 0), "x": round(eye.x, 1), "z": round(eye.z, 1)}),
    );
    tasks.extend([
        Task::Grab {
            item,
            side: Side::Left,
        },
        Task::Throw {
            side: Side::Left,
            at: scene.zone.on_ground(landing.x, landing.y),
        },
    ]);
    r.interrupted = true;
    bot.replace(tasks);
    r.stalking = Some(Stalk {
        walker,
        item,
        landing,
        started: r.clock,
        thrown: None,
    });
    true
}

/// A drone chirped its lock on this bot: get somewhere it can't see, if
/// there's somewhere close (a player hears it and steps behind a tree).
fn duck(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    pos: Vec3,
    drone: Entity,
) -> Option<Vec<Task>> {
    let (_, _, gt, aware, ..) = scene.robots.get(drone).ok()?;
    if aware.target != Some(me) || r.clock - r.ducked_at < 4.0 {
        return None;
    }
    if r.rng.range(0.0, 1.0) > r.traits.duck {
        return None;
    }
    let (d, to) = cover_from(scene, pos, gt.translation(), 7.0)?;
    r.ducked_at = r.clock;
    log.event(
        "duck",
        json!({"dist": round(d, 1), "x": round(pos.x, 1), "z": round(pos.z, 1)}),
    );
    Some(vec![Task::WalkTo { to, within: 0.6 }, Task::Wait(1.2)])
}

/// The nearest spot within `reach` that `threat` can't see.
/// The nearest spot just outside the anomaly `at` stands in, if it stands
/// in one.
fn out_of_anomaly(scene: &Scene, at: Vec2) -> Option<Vec2> {
    scene
        .anomalies
        .iter()
        .map(|(_, a, gt)| (gt.translation().xz(), a.radius))
        .find(|(c, rad)| c.distance(at) < rad + 0.5)
        .map(|(c, rad)| c + (at - c).normalize_or(Vec2::X) * (rad + 1.5))
}

fn cover_from(scene: &Scene, pos: Vec3, threat: Vec3, reach: f32) -> Option<(f32, Vec2)> {
    let mut best: Option<(f32, Vec2)> = None;
    for k in 0..12 {
        let a = k as f32 * std::f32::consts::TAU / 12.0;
        for d in [2.5, 4.5, 7.0, 10.0] {
            if d > reach {
                break;
            }
            let p = pos.xz() + Vec2::new(a.cos(), a.sin()) * d;
            let eye = Vec3::new(p.x, scene.zone.ground(p.x, p.y) + 1.5, p.y);
            // Not into water, and not through a wall to get there.
            if scene.zone.hm.water_level([p.x, p.y]).is_some()
                || !scene.path_clear(pos + Vec3::Y * 0.9, eye - Vec3::Y * 0.6)
            {
                break;
            }
            // Cover in an anomaly is no cover.
            if out_of_anomaly(scene, p).is_some() {
                continue;
            }
            if !scene.visible(threat, eye) && best.is_none_or(|b| d < b.0) {
                best = Some((d, p));
            }
        }
    }
    best
}

/// Between bursts: keep shooting, reload, or call it.
fn fight_on(
    me: Entity,
    bot: &mut Bot,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    eye: Vec3,
    gun: Option<Entity>,
    target: Entity,
    hit: bool,
) {
    let threat = scene
        .robots
        .get(target)
        .ok()
        .map(|(_, _, gt, ..)| gt.translation());
    let outcome = match scene.robots.get(target) {
        Err(_) => Some("gone"),
        Ok((_, rb, ..)) if rb.wrecked => Some("wrecked"),
        Ok((_, _, gt, ..)) if gt.translation().distance(eye) > 130.0 => Some("out_of_range"),
        _ => None,
    };
    let lost = r.combat.as_ref().is_some_and(|c| c.unseen > 5.0);
    let long = r.combat.as_ref().is_some_and(|c| log.t - c.started > 75.0);
    // Two bursts in a row with nothing fired: something's wrong with the gun.
    let mut clicked = false;
    if let Some(c) = &mut r.combat {
        if c.burst_shots == log.stats.shots {
            c.dry_bursts += 1;
        } else {
            c.dry_bursts = 0;
        }
        clicked = c.dry_bursts >= 2;
        c.burst_shots = log.stats.shots;
    }
    // Gun not in hand (a grab that didn't take): get it.
    if let Some(g) = r.gun.filter(|g| gun != Some(*g)) {
        bot.replace([
            Task::Release(Side::Right),
            Task::Grab {
                item: g,
                side: Side::Right,
            },
        ]);
        return;
    }
    // A fight that finds it standing in an anomaly (one it was working, or
    // stepped into dodging): out of it first, then on with the fight.
    if outcome.is_none() {
        if let Some(out) = out_of_anomaly(scene, eye.xz()) {
            log.event("reposition", json!({"why": "anomaly"}));
            bot.replace([
                Task::Release(Side::Left),
                Task::WalkTo {
                    to: out,
                    within: 0.6,
                },
                Task::Support,
                Task::Shoot { target, shots: 4 },
            ]);
            return;
        }
    }
    let (loaded, spare) = scene.ammo(me, gun);
    // Hurt and still being hit: get out of its sight and see to it (the
    // dressing comes once out of the fight; it'll be back if the robot
    // follows).
    let cover = threat
        .filter(|t| hit && r.hp < 45.0 && scene.visible(*t, eye))
        .filter(|_| r.rng.range(0.0, 1.0) < r.traits.duck)
        .and_then(|t| cover_from(scene, eye - Vec3::Y * 1.6, t, 10.0));
    // A drone that's hitting it: standing there is death, whatever its
    // health. Out of its sight if anywhere's near; else run across its
    // line (it can't hit that well: `drone::CROSSING_SPREAD`).
    let drone = scene
        .robots
        .get(target)
        .is_ok_and(|(_, rb, ..)| rb.kind == crate::robots::RobotKind::Drone);
    let cover = cover.or_else(|| {
        threat
            .filter(|t| drone && hit && scene.visible(*t, eye))
            .and_then(|t| {
                let feet = eye - Vec3::Y * 1.6;
                cover_from(scene, feet, t, 14.0).or_else(|| {
                    let across = Vec2::new(t.x - feet.x, t.z - feet.z)
                        .perp()
                        .normalize_or(Vec2::X);
                    let side = if r.rng.range(0.0, 1.0) < 0.5 {
                        1.0
                    } else {
                        -1.0
                    };
                    let to = feet.xz() + across * side * 10.0;
                    scene
                        .path_clear(eye, Vec3::new(to.x, eye.y, to.y))
                        .then_some((10.0, to))
                })
            })
    });
    let outcome = outcome
        .or(cover.is_some().then_some("took_cover"))
        .or(lost.then_some("lost"))
        .or(long.then_some("broke_off"))
        .or((loaded + spare == 0).then_some("out_of_ammo"));
    if let Some(outcome) = outcome {
        let mut c = r.combat.take().unwrap();
        c.close(log, outcome);
        if outcome == "wrecked" {
            *log.stats.kills.entry(c.kind.clone()).or_default() += 1;
            if r.traits.salvage {
                if let Ok((_, _, gt, ..)) = scene.robots.get(target)
                    // Come down inside a building (a drone in the Bus
                    // depot's hall) or down a site: no walk from outside
                    // reaches it, and its loot is the building's, for
                    // searching the building.
                    && !wreck_inside(&r.plans, &scene.zone, gt.translation())
                {
                    r.sites.push(Site {
                        name: format!("{} wreck", c.kind),
                        kind: SiteKind::Wreck(target),
                        at: gt.translation().xz(),
                        spots: 2,
                        visited: false,
                        tries: 0,
                        job: false,
                    });
                }
            }
        }
        r.interrupted = true;
        if let Some((d, to)) = cover {
            r.ducked_at = r.clock;
            log.event(
                "cover",
                json!({"why": "hurt", "dist": round(d, 1), "hp": round(r.hp, 0)}),
            );
            bot.sprint = true;
            bot.replace([
                Task::Release(Side::Left),
                Task::WalkTo { to, within: 0.6 },
                Task::Stow(Side::Right),
                Task::Wait(2.0),
            ]);
            return;
        }
        bot.replace([
            Task::Release(Side::Left),
            Task::Stow(Side::Right),
            Task::Wait(0.3),
        ]);
        return;
    }
    let jammed = gun
        .and_then(|g| scene.guns.get(g).ok())
        .is_some_and(|g| g.firearm.malfunction.is_some());
    if loaded > 0 && (jammed || clicked) {
        log.event("rack", json!({"jammed": jammed}));
        bot.replace([
            Task::Gun(MacroKind::Rack),
            Task::Support,
            Task::Shoot { target, shots: 4 },
        ]);
        return;
    }
    if loaded == 0 {
        // Reload where it can't see you, then step back out.
        let cover = threat
            .filter(|t| scene.visible(*t, eye))
            .filter(|_| r.rng.range(0.0, 1.0) < r.traits.duck)
            .and_then(|t| cover_from(scene, eye - Vec3::Y * 1.6, t, 7.0));
        log.event("reload", json!({"spare": spare, "cover": cover.is_some()}));
        match cover {
            Some((_, to)) => bot.replace([
                Task::Release(Side::Left),
                Task::WalkTo { to, within: 0.6 },
                Task::Gun(MacroKind::Reload),
                Task::WalkTo {
                    to: eye.xz(),
                    within: 0.8,
                },
                Task::Support,
                Task::Shoot { target, shots: 4 },
            ]),
            None => bot.replace([
                Task::Gun(MacroKind::Reload),
                Task::Support,
                Task::Shoot { target, shots: 4 },
            ]),
        }
        return;
    }
    // No line of fire: hold, rather than empty a magazine into a wall.
    if r.combat.as_ref().is_some_and(|c| c.unseen > 0.0) {
        bot.replace([Task::Wait(0.4)]);
        return;
    }
    // The last burst went into something in the way: shift a few steps.
    let walled = r.combat.as_mut().is_some_and(|c| {
        let more = log.stats.blocked - c.blocked_at_burst;
        c.blocked_at_burst = log.stats.blocked;
        more >= 3
    });
    if walled {
        let here = eye.xz();
        let to = scene
            .robots
            .get(target)
            .map(|(_, _, gt, ..)| gt.translation().xz())
            .unwrap_or(here);
        log.event("reposition", json!({}));
        // Indoors, a few steps along the plan toward it (out of a doorway
        // whose jamb the muzzle was in): a step aside there is a wall.
        if let Some(mut tasks) = indoor_step(r, eye, to, 3.0) {
            tasks.insert(0, Task::Release(Side::Left));
            tasks.extend([Task::Support, Task::Shoot { target, shots: 4 }]);
            bot.replace(tasks);
            return;
        }
        let toward = (to - here).normalize_or(Vec2::X);
        let side = Vec2::new(-toward.y, toward.x) * if r.rng.chance(0.5) { 1.0 } else { -1.0 };
        let step = here + (toward + side).normalize() * 3.0;
        bot.replace([
            Task::Release(Side::Left),
            Task::WalkTo {
                to: step,
                within: 0.6,
            },
            Task::Support,
            Task::Shoot { target, shots: 4 },
        ]);
        return;
    }
    bot.replace([Task::Support, Task::Shoot { target, shots: 4 }]);
}

/// Anomalies it can see get routed round (unless it's reckless).
/// An anomaly that has caught a bot it gives this much berth past its
/// danger radius from then on (m): a path round it cuts its corners by
/// up to a waypoint's reach (1.2 m), and still keeps out.
const BURNED_BERTH: f32 = 2.0;

fn spot_anomalies(bot: &mut Bot, r: &mut Raider, scene: &Scene, here: Vec2) {
    // In one it didn't mean to walk into: out the way it came, and never
    // that one again, reckless or not (a dive ends when it leaves).
    if !r.diving {
        let caught = scene.anomalies.iter().find(|(e, a, gt)| {
            !r.burned.contains(e) && gt.translation().xz().distance(here) < a.radius + 0.3
        });
        if let Some((e, a, gt)) = caught {
            let at = gt.translation().xz();
            let danger = a.kind.danger_radius(a.radius);
            r.burned.insert(e);
            r.grid.block(
                Obstacle::Circle {
                    at: at.to_array(),
                    radius: danger + BURNED_BERTH,
                },
                0.0,
            );
            let out = at + (here - at).normalize_or(Vec2::X) * (danger + BURNED_BERTH + 0.5);
            r.interrupted = true;
            bot.replace([Task::WalkTo {
                to: out,
                within: 0.8,
            }]);
            bot.sprint = true;
            return;
        }
    }
    // Whatever it was going to (often the thing it was caught on the way
    // to), it goes no nearer one that's caught it than the ring outside.
    for (e, a, gt) in &scene.anomalies {
        if !r.burned.contains(&e) {
            continue;
        }
        let at = gt.translation().xz();
        let danger = a.kind.danger_radius(a.radius);
        let ring = at + (here - at).normalize_or(Vec2::X) * (danger + BURNED_BERTH + 0.5);
        for t in bot.tasks.iter_mut() {
            if let Task::WalkTo { to, .. } = t {
                if to.distance(at) < danger + BURNED_BERTH {
                    *to = ring;
                }
            }
        }
    }
    let mut new = Vec::new();
    for (e, a, gt) in &scene.anomalies {
        if r.known.contains(&e) {
            continue;
        }
        let at = gt.translation().xz();
        if at.distance(here) < 30.0 {
            r.known.insert(e);
            // The berth a careful walker gives it: a well's pull and a
            // mirror's far side reach past its edge.
            new.push((at, a.kind.danger_radius(a.radius)));
        }
    }
    if new.is_empty() || !r.traits.avoid_anomalies {
        return;
    }
    for (at, radius) in &new {
        r.grid.block(
            Obstacle::Circle {
                at: at.to_array(),
                radius: radius + 1.0,
            },
            0.0,
        );
    }
    // Replan if the walk ahead goes through one (not when diving on purpose).
    if r.diving || r.combat.is_some() {
        return;
    }
    let crosses = bot.tasks.iter().any(|t| match t {
        Task::WalkTo { to, .. } => new
            .iter()
            .any(|(at, rad)| seg_dist(here, *to, *at) < rad + 0.8),
        _ => false,
    });
    if crosses {
        r.interrupted = true;
        bot.replace([Task::Wait(0.1)]);
    }
}

/// A lockdown has bolted a door for good (the shelter's programme): its
/// doorway is a wall in the plan from now on, and a walk planned through
/// it is planned again, the escape tunnel's way.
/// Which doors are held shut on power right now, in every plan; and a
/// route queued through one with no switch thrown before it (a way out
/// planned through a door the breaker still holds) is dropped, to be
/// planned again round it.
fn spot_held(bot: &mut Bot, r: &mut Raider, scene: &Scene) {
    for plan in r.plans.iter_mut() {
        if plan.doors.is_empty() {
            continue;
        }
        plan.held.clear();
        plan.held.extend(
            plan.doors
                .iter()
                .map(|d| d.entity)
                .filter(|d| scene.power_locked.contains(*d)),
        );
    }
    let mut pressed = false;
    for t in &bot.tasks {
        match t {
            Task::Press(_) => pressed = true,
            Task::OpenDoor(d) if !pressed && r.plans.iter().any(|p| p.held.contains(d)) => {
                r.interrupted = true;
                bot.replace([Task::Wait(0.1)]);
                return;
            }
            _ => {}
        }
    }
}

fn spot_sealed(bot: &mut Bot, r: &mut Raider, scene: &Scene, log: &mut SimLog) {
    let mut new = Vec::new();
    for plan in r.plans.iter_mut() {
        for d in &plan.doors {
            if r.sealed.contains(&d.entity)
                || !scene
                    .power_locked
                    .get(d.entity)
                    .is_ok_and(|l| l.stage == crate::world::underground::SEALED)
            {
                continue;
            }
            plan.grid.block(d.leaf(), 0.0);
            new.push((d.entity, plan.name.clone()));
        }
    }
    if new.is_empty() {
        return;
    }
    for (door, site) in &new {
        r.sealed.insert(*door);
        log.event("sealed", json!({"site": site}));
    }
    let through = bot
        .tasks
        .iter()
        .any(|t| matches!(t, Task::OpenDoor(d) if r.sealed.contains(d)));
    if through {
        r.interrupted = true;
        bot.replace([Task::Wait(0.1)]);
    }
}

fn seg_dist(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// A walk along the Zone grid, as WalkTo tasks of at most 20 m each.
fn travel(grid: &NavGrid, from: Vec2, to: Vec2, within: f32) -> Option<Vec<Task>> {
    let path = grid.path(from.to_array(), to.to_array())?;
    let mut tasks = Vec::new();
    let mut last = from;
    let n = path.len();
    // Standing in a building's or a tree's margin, the path starts from
    // the nearest open cell instead: step out to it first, rather than
    // cutting the corner to the next point.
    let start = Vec2::from_array(path[0]);
    if start.distance(from) > 0.5 {
        tasks.push(Task::WalkTo {
            to: start,
            within: 0.6,
        });
        last = start;
    }
    for (i, p) in path.into_iter().enumerate().skip(1) {
        let p = Vec2::from_array(p);
        let steps = (last.distance(p) / 20.0).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let q = last.lerp(p, k as f32 / steps as f32);
            let end = i == n - 1 && k == steps;
            tasks.push(Task::WalkTo {
                to: q,
                within: if end { within } else { 1.2 },
            });
        }
        last = p;
    }
    // The grid snaps the goal to walkable ground; finish the last bit
    // straight if it's short.
    if last.distance(to) > within && last.distance(to) < 6.0 {
        tasks.push(Task::WalkTo { to, within });
    }
    Some(tasks)
}

/// Indoors: from where the bot is (in the building's grid) to a local
/// point, opening doors on the way.
pub fn indoor_route(plan: &BuildingPlan, from: Vec3, to: Vec2, within: f32) -> Option<Vec<Task>> {
    indoor(plan, from, to, within)
}

/// To something to take: stand where it can be reached.
fn indoor(plan: &BuildingPlan, from: Vec3, to: Vec2, within: f32) -> Option<Vec<Task>> {
    let goal = reach_cell(plan, plan.to_local(from).xz(), to)?;
    indoor_to(plan, from, goal, within)
}

/// How near the cell it's making for a bot reaching for something at
/// `to` need get: anywhere it's still within arm's reach of it does, as a
/// person leans over a chest or a bench (Cinder sims, 31 October: raiders
/// stood at the works garage's benches and the charging queue's tool
/// chest for 40 s each, 0.3 m short of a cell the furniture wouldn't let
/// them onto).
pub(crate) fn stand_within(goal: Vec2, to: Vec2, within: f32) -> f32 {
    within.max(crate::bot::REACH - REACH_SPARE - goal.distance(to))
}

/// Short of arm's reach a stop leaves, for the hand's last lean.
const REACH_SPARE: f32 = 0.15;

/// As [`indoor`], through doors held shut on power: for a route that
/// throws their switches first.
fn indoor_through_held(
    plan: &BuildingPlan,
    from: Vec3,
    to: Vec2,
    within: f32,
) -> Option<Vec<Task>> {
    let goal = reach_cell(plan, plan.to_local(from).xz(), to)?;
    indoor_path(plan, from, goal, stand_within(goal, to, within), true)
}

/// Out of the building: to the floor nearest the way out.
fn indoor_out(plan: &BuildingPlan, from: Vec3, to: Vec2, within: f32) -> Option<Vec<Task>> {
    let goal = stand_near_with(plan, plan.to_local(from).xz(), to, false)?;
    indoor_to(plan, from, goal, within)
}

/// The longest single walk on an indoor route (m).
const LEG_MAX: f32 = 10.0;

fn indoor_to(plan: &BuildingPlan, from: Vec3, goal: Vec2, within: f32) -> Option<Vec<Task>> {
    indoor_path(plan, from, goal, within, false)
}

fn indoor_path(
    plan: &BuildingPlan,
    from: Vec3,
    goal: Vec2,
    within: f32,
    through_held: bool,
) -> Option<Vec<Task>> {
    let a = plan.to_local(from).xz();
    let path = if through_held {
        plan.path_through_held(a, goal)?
    } else {
        plan.path(a, goal)?
    };
    let mut tasks = Vec::new();
    // A long straight run (an underground site's long stair) in legs, each
    // well inside a walk's time.
    let walk = |tasks: &mut Vec<Task>, from: Vec2, to: Vec2, within: f32| {
        let legs = (from.distance(to) / LEG_MAX).ceil().max(1.0) as usize;
        for k in 1..legs {
            tasks.push(Task::WalkTo {
                to: plan.world2(from.lerp(to, k as f32 / legs as f32)),
                within: 0.6,
            });
        }
        tasks.push(Task::WalkTo {
            to: plan.world2(to),
            within,
        });
    };
    let mut last = a;
    let n = path.len();
    for (i, p) in path.into_iter().enumerate().skip(1) {
        let mut start = last;
        for (door, cross) in plan.doors_on(last, p) {
            let dir = (p - last).normalize_or_zero();
            // Square to the doorway: its normal, toward the side it's
            // coming from.
            let normal = plan
                .doors
                .iter()
                .find(|d| d.entity == door)
                .map(|d| {
                    let t = (d.b - d.a).normalize_or(Vec2::X);
                    let n = Vec2::new(-t.y, t.x);
                    if (start - cross).dot(n) < 0.0 { -n } else { n }
                })
                .unwrap_or(-dir);
            let walkable = |q: Vec2| plan.grid.walkable(q.to_array());
            // Square on where there's floor for it (a tower's landing may
            // have none); along the path otherwise.
            // Only when it starts at the door, inside the leaf's swing (the
            // trap); from further off the old way in works.
            let square_on = start.distance(cross) < 1.1 && walkable(cross + normal * 1.1);
            let before = if square_on {
                cross + normal * 1.1
            } else {
                cross - dir * 1.1
            };
            // Stand clear of the leaf's swing to open it: from further off,
            // walk up to there; from closer in (starting at the door),
            // step back out to there first, or the leaf swings into you and
            // pins you in the doorway.
            let back = square_on || (before - start).dot(dir) > 0.0;
            if back {
                walk(&mut tasks, start, before, 0.3);
            }
            tasks.push(Task::OpenDoor(door));
            // A door swinging out can shove you aside: square up to the
            // doorway again rather than go at the frame on a slant.
            if back {
                tasks.push(Task::WalkTo {
                    to: plan.world2(before),
                    within: 0.3,
                });
            }
            // Straight through, clear of the open leaf, before turning.
            let through = cross - normal * 0.7;
            if square_on && walkable(through) {
                tasks.push(Task::WalkTo {
                    to: plan.world2(through),
                    within: 0.3,
                });
                start = through;
            } else {
                start = cross;
            }
        }
        walk(&mut tasks, start, p, if i == n - 1 { within } else { 0.35 });
        last = p;
    }
    Some(tasks)
}

/// Wedged in a corner, the nearest walkable cell can be through the wall:
/// the nearest one on the same side of the footprint's edge instead.
fn near_walkable_same_side(plan: &BuildingPlan, from: Vec2) -> Option<Vec2> {
    let g = &plan.grid;
    let within = |p: Vec2, pad: f32| p.x.abs() < plan.half.x + pad && p.y.abs() < plan.half.y + pad;
    let inside = within(from, 0.0);
    let (ci, cj) = g.cell_of(from.to_array())?;
    let r = (1.5 / g.cell).ceil() as i64;
    let mut best: Option<(f32, Vec2)> = None;
    for dj in -r..=r {
        for di in -r..=r {
            let (i, j) = (ci as i64 + di, cj as i64 + dj);
            if i < 0 || j < 0 || i >= g.n as i64 || j >= g.n as i64 {
                continue;
            }
            let (i, j) = (i as usize, j as usize);
            if g.cost[j * g.n + i] == extraction_core::nav::BLOCKED {
                continue;
            }
            let c = Vec2::from_array(g.centre(i, j));
            let same = if inside {
                within(c, -0.2)
            } else {
                !within(c, 0.2)
            };
            let d = c.distance(from);
            if same && best.is_none_or(|b| d < b.0) {
                best = Some((d, c));
            }
        }
    }
    best.map(|b| b.1)
}

/// The walkable cell nearest a goal among those reachable from `from`
/// (the nearest walkable cell might be through a wall).
/// The floor cell nearest `goal` that a bot can walk to from `from`, on
/// the same side of the walls as `goal` when it's indoors: somewhere to
/// stand, not something to reach (that's [`reach_cell`]).
pub(crate) fn stand_near(plan: &BuildingPlan, from: Vec2, goal: Vec2) -> Option<Vec2> {
    stand_near_with(plan, from, goal, true)
}

/// As [`stand_near`]; `keep_in`: a goal within the walls is reached from
/// inside them, not from the ground outside. Not for a way out, which is
/// outside: an underground site's kiosk lies within its walls' bounds, and
/// keeping in left a raider no stand by it (Rime's adit, out of the
/// magazine).
fn stand_near_with(plan: &BuildingPlan, from: Vec2, goal: Vec2, keep_in: bool) -> Option<Vec2> {
    let g = &plan.grid;
    let start = if g.walkable(from.to_array()) {
        from
    } else {
        near_walkable_same_side(plan, from)
            .or_else(|| g.nearest_walkable(from.to_array()).map(Vec2::from_array))?
    };
    let (si, sj) = g.cell_of(start.to_array())?;
    let n = g.n;
    let mut seen = vec![false; n * n];
    let mut stack = vec![(si, sj)];
    seen[sj * n + si] = true;
    let mut best: Option<(f32, Vec2)> = None;
    let indoors = keep_in && plan.indoors(goal);
    while let Some((i, j)) = stack.pop() {
        let c = Vec2::from_array(g.centre(i, j));
        let d = c.distance(goal);
        if best.is_none_or(|b| d < b.0) && !(indoors && plan.outside[j * n + i]) {
            best = Some((d, c));
        }
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let k = nj as usize * n + ni as usize;
            if !seen[k] && g.cost[k] != extraction_core::nav::BLOCKED {
                seen[k] = true;
                stack.push((ni as usize, nj as usize));
            }
        }
    }
    best.filter(|b| b.0 < 2.0).map(|b| b.1)
}

pub(crate) fn reach_cell(plan: &BuildingPlan, from: Vec2, goal: Vec2) -> Option<Vec2> {
    let g = &plan.grid;
    let start = if g.walkable(from.to_array()) {
        from
    } else {
        near_walkable_same_side(plan, from)
            .or_else(|| g.nearest_walkable(from.to_array()).map(Vec2::from_array))?
    };
    let (si, sj) = g.cell_of(start.to_array())?;
    let n = g.n;
    let mut seen = vec![false; n * n];
    let mut stack = vec![(si, sj)];
    seen[sj * n + si] = true;
    let mut best: Option<(f32, Vec2)> = None;
    let mut in_door: Option<(f32, Vec2)> = None;
    let solid = has_no_floor(plan);
    while let Some((i, j)) = stack.pop() {
        let c = Vec2::from_array(g.centre(i, j));
        let d = c.distance(goal);
        // Not in a doorway if anywhere else will do: the door leaf swings
        // through it, and a bot standing there to reach in is shut out by
        // its own door.
        if reaches_with(plan, solid, c, goal) {
            let doorway = plan.doors.iter().any(|d| seg_dist(d.a, d.b, c) < DOOR_KEEP);
            let slot = if doorway { &mut in_door } else { &mut best };
            if slot.is_none_or(|b| d < b.0) {
                *slot = Some((d, c));
            }
        }
        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
            let (ni, nj) = (i as i64 + di, j as i64 + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let k = nj as usize * n + ni as usize;
            if !seen[k] && g.cost[k] != extraction_core::nav::BLOCKED {
                seen[k] = true;
                stack.push((ni as usize, nj as usize));
            }
        }
    }
    best.filter(|b| b.0 < 2.0)
        .or(in_door.filter(|b| b.0 < 2.0))
        .map(|b| b.1)
}

/// Whether a bot standing at `from` (building-local) can reach `goal`:
/// along a clear line, not through a wall or a window, nor round one (a
/// barn's open gaps join its floor to the outside, so "which side" isn't
/// the test, the line is). Something with no floor of its own on the grid
/// (a wagon, its bed at chest height) is reached into from beside it.
#[cfg(test)]
pub(crate) fn reaches(plan: &BuildingPlan, from: Vec2, goal: Vec2) -> bool {
    reaches_with(plan, has_no_floor(plan), from, goal)
}

fn reaches_with(plan: &BuildingPlan, solid: bool, from: Vec2, goal: Vec2) -> bool {
    if solid {
        from.distance(goal) <= SOLID_REACH
    } else {
        clear_reach(&plan.grid, from, goal)
    }
}

fn has_no_floor(plan: &BuildingPlan) -> bool {
    let g = &plan.grid;
    !(0..g.n * g.n).any(|k| {
        let c = Vec2::from_array(g.centre(k % g.n, k / g.n));
        g.cost[k] != extraction_core::nav::BLOCKED
            && c.x.abs() < plan.half.x
            && c.y.abs() < plan.half.y
    })
}

/// Whether a hand can go from standing at `from` to `goal` in a straight
/// line over open floor: everything between walkable but the last
/// [`SHELF`] metres, where the thing rests on its shelf or table.
fn clear_reach(g: &extraction_core::nav::NavGrid, from: Vec2, goal: Vec2) -> bool {
    let d = from.distance(goal);
    let n = ((d - SHELF).max(0.0) / 0.1).ceil() as usize;
    (0..=n).all(|k| g.walkable(from.lerp(goal, (k as f32 * 0.1 / d).min(1.0)).to_array()))
}

/// How far from a doorway a bot stands to reach for something, metres.
const DOOR_KEEP: f32 = 0.9;

/// How deep a shelf, table or crate may be between a bot and what's on it.
const SHELF: f32 = 1.1;
/// How far into something with no floor of its own (a wagon) a bot reaches.
const SOLID_REACH: f32 = 1.4;

/// The next thing to do while raiding.
fn raid_step(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    here: Vec2,
    pos: Vec3,
    children: &Children,
) -> Vec<Task> {
    // Anything still in the left hand (a refused stow, a fight) goes down
    // or away first.
    if let Some(held) = scene.hand(children, Side::Left) {
        // A stow that didn't take: the pack is full (or it's a long gun
        // and the slings are taken). Put it down and remember.
        r.refusals += 1;
        r.skip.insert(held);
        let bulky = scene
            .items
            .get(held)
            .is_ok_and(|(_, it, ..)| scene.catalog.get(it.kind()).size == SizeClass::Bulky);
        if !bulky && r.refusals >= 2 {
            r.pack_full = true;
        }
        log.event("drop", json!({"why": "no_room", "bulky": bulky}));
        return vec![Task::Release(Side::Left), Task::Wait(0.2)];
    }
    for _ in 0..8 {
        match r.act {
            Act::Idle => {
                add_job_site(r, scene);
                add_safehouse_site(r, scene);
                let Some(s) = choose_site(r, scene, here) else {
                    r.pack_full = true; // nothing left worth the walk
                    return vec![Task::Wait(0.2)];
                };
                let site = &r.sites[s];
                log.event(
                    "goal",
                    json!({"site": site.name, "kind": kind_name(site.kind),
                           "x": round(site.at.x, 1), "z": round(site.at.y, 1),
                           "dist": round(site.at.distance(here), 0)}),
                );
                r.act = Act::Travel(s);
                if let Some(t) = go_to_site(r, scene, s, pos) {
                    return t;
                }
                r.sites[s].visited = true;
                r.act = Act::Idle;
            }
            Act::Travel(s) => {
                if arrived(r, scene, s, pos) {
                    // What's lying there (all of it, and what it wants).
                    let site = &r.sites[s];
                    let (mut all, mut artifacts) = (0, 0);
                    for (_, it, gt, h, so, b) in &scene.items {
                        if h.is_none()
                            && so.is_none()
                            && b.is_none()
                            && at_site(r, site, gt.translation())
                        {
                            all += 1;
                            if scene.catalog.get(it.kind()).category == Category::Artifact {
                                artifacts += 1;
                            }
                        }
                    }
                    log.event(
                        "arrive",
                        json!({"site": site.name, "items": all, "artifacts": artifacts}),
                    );
                    r.act = Act::Search(s);
                    if let Some((relay, t)) = plant_tasks(me, scene, &r.sites[s]) {
                        // Set down for good: not loot to pick up again.
                        r.skip.insert(relay);
                        log.event("plant", json!({"site": r.sites[s].name}));
                        return t;
                    }
                    continue;
                }
                if !r.interrupted {
                    r.sites[s].tries += 1;
                }
                if r.sites[s].tries >= 3 {
                    log.event("give_up", json!({"site": r.sites[s].name}));
                    r.sites[s].visited = true;
                    r.act = Act::Idle;
                    continue;
                }
                if let Some(t) = go_to_site(r, scene, s, pos) {
                    return t;
                }
                r.sites[s].visited = true;
                r.act = Act::Idle;
            }
            Act::Search(s) => {
                match pick_item(me, r, scene, s, pos) {
                    Some(item) => {
                        r.act = Act::Take(s, item);
                        if let Some(t) = take_item(r, scene, s, item, pos) {
                            let power = t.iter().any(|t| {
                                matches!(t, Task::Press(e) if scene.power_switches.contains(*e))
                            });
                            if power {
                                log.event(
                                    "power",
                                    json!({"site": r.sites[s].name, "x": round(pos.x, 1), "z": round(pos.z, 1)}),
                                );
                            }
                            return t;
                        }
                        if let Some(why) = r.pass_why.take() {
                            let id = scene.items.get(item).map_or("?".into(), |(_, it, ..)| {
                                scene.catalog.get(it.kind()).id.clone()
                            });
                            log.event("pass", json!({"item": id, "why": why}));
                        }
                        r.skip.insert(item);
                        r.act = Act::Search(s);
                    }
                    None => {
                        // A stone lying in an anomaly it wouldn't take:
                        // say which and why not (skipped already, or not
                        // worth it).
                        if r.sites[s].kind == SiteKind::Anomaly {
                            let site = &r.sites[s];
                            for (e, it, gt, h, so, b) in &scene.items {
                                let def = scene.catalog.get(it.kind());
                                if h.is_none()
                                    && so.is_none()
                                    && b.is_none()
                                    && def.category == Category::Artifact
                                    && at_site(r, site, gt.translation())
                                {
                                    let why = if r.skip.contains(&e) {
                                        "passed on it"
                                    } else if r.traits.value(def) < r.traits.min_value {
                                        "not worth it"
                                    } else {
                                        "no room"
                                    };
                                    log.event("pass", json!({"item": def.id, "why": why}));
                                }
                            }
                        }
                        r.sites[s].visited = true;
                        r.act = Act::Idle;
                        // Out the door it came in by.
                        if let SiteKind::Building(b) = r.sites[s].kind {
                            let plan = &r.plans[b];
                            if plan.in_grid(pos) && !plan.entries.is_empty() {
                                let local = plan.to_local(pos).xz();
                                let exit = plan
                                    .entries
                                    .iter()
                                    .min_by(|a, b| a.distance(local).total_cmp(&b.distance(local)))
                                    .copied()
                                    .unwrap();
                                if let Some(t) = indoor_out(plan, pos, exit, 0.6) {
                                    return t;
                                }
                            }
                        }
                    }
                }
            }
            Act::Take(s, item) => {
                if settle_take(me, r, scene, log, pos) {
                    // Taken (and counted).
                } else if !r.interrupted || r.skip.contains(&item) {
                    r.skip.insert(item);
                } else if let Some(t) = take_item(r, scene, s, item, pos) {
                    return t;
                } else {
                    r.skip.insert(item);
                }
                r.act = Act::Search(s);
            }
            Act::ToRv | Act::Board => {
                r.act = Act::Idle;
            }
        }
    }
    vec![Task::Wait(0.2)]
}

/// A take that's done (the thing is on it): logged as loot, once. True if
/// it was.
fn settle_take(me: Entity, r: &mut Raider, scene: &Scene, log: &mut SimLog, pos: Vec3) -> bool {
    let Act::Take(s, item) = r.act else {
        return false;
    };
    if r.skip.contains(&item) || !scene.mine(me, item) {
        return false;
    }
    let (id, at) = scene
        .items
        .get(item)
        .map(|(_, it, gt, ..)| (scene.catalog.get(it.kind()).id.clone(), gt.translation()))
        .unwrap_or_default();
    log.stats.loot.push(id.clone());
    log.event(
        "loot",
        json!({"item": id, "site": r.sites[s].name, "x": round(pos.x, 1), "z": round(pos.z, 1),
               "ix": round(at.x, 1), "iz": round(at.z, 1)}),
    );
    r.skip.insert(item);
    true
}

fn kind_name(k: SiteKind) -> &'static str {
    match k {
        SiteKind::Building(_) => "building",
        SiteKind::Spot => "spot",
        SiteKind::Field => "field",
        SiteKind::Anomaly => "anomaly",
        SiteKind::Wreck(_) => "wreck",
    }
}

/// How much a long walk through dangerous country puts a site off, per
/// [`route_exposure`] unit.
const ROUTE_CAUTION: f32 = 0.15;

/// The danger a walk from `from` to `to` crosses, as a player reads it off
/// the field map: the regions' danger (0 calm .. 3 deadly) along the
/// straight line, summed per hundred metres. A rich site past a deadly
/// road is worth less than the same site next door.
fn route_exposure(map: &extraction_core::map::MapDef, from: Vec2, to: Vec2) -> f32 {
    let d = from.distance(to);
    let steps = (d / 40.0).ceil().max(1.0) as usize;
    let danger: f32 = (0..=steps)
        .map(|k| {
            let p = from.lerp(to, k as f32 / steps as f32);
            map.region_at(p.to_array()).map_or(0.0, |r| r.danger as f32)
        })
        .sum::<f32>()
        / (steps + 1) as f32;
    danger * d / 100.0
}

/// Picks the next site: worth, distance, the danger of the walk there, and
/// the pull of the far side as the clock runs down.
fn choose_site(r: &mut Raider, scene: &Scene, here: Vec2) -> Option<usize> {
    // `SIM_SITE=<prefab>`: that site first, while it's there to be done
    // (a smoke test of one underground site, say).
    if let Ok(want) = std::env::var("SIM_SITE") {
        if let Some(i) = r
            .sites
            .iter()
            .position(|s| s.name == want && !s.visited && s.tries < 3)
        {
            return Some(i);
        }
    }
    let late = (scene.awake() / (r.traits.minutes * 60.0)).clamp(0.0, 1.0);
    let rv_side = rv_centroid(&scene.zone);
    let heat = r.heat;
    let mut best: Option<(f32, usize)> = None;
    for (i, s) in r.sites.iter().enumerate() {
        if s.visited || s.tries >= 3 {
            continue;
        }
        let pull = match s.kind {
            // The driver's scout job: a player goes for it, as long as
            // they're fit to.
            // (`SIM_OVERNIGHT`'s safehouse is the point of the run.)
            _ if s.job && r.hp >= 50.0 && s.name == "safehouse" => 40.0,
            // (`SIM_CONTRACT`'s step is the point of the run, too.)
            _ if s.job && r.hp >= 50.0 && std::env::var_os("SIM_CONTRACT").is_some() => 40.0,
            _ if s.job && r.hp >= 50.0 => 12.0,
            _ if s.job => 0.0,
            // A big place looks worth more, up to a point: from the field
            // map a player sees a big building, not its shelf count (an
            // underground complex's forty spots had every fresh raider
            // walk 566 m up Marrow's Gate road to the Depot, into the
            // robots coming out).
            SiteKind::Building(_) => {
                r.traits.buildings * (1.0 + s.spots.min(BIG_SITE) as f32 * 0.35)
            }
            SiteKind::Spot => r.traits.spots,
            SiteKind::Field => r.traits.fields * 3.0,
            // One stone at most, and only some cycles; but a hunter knows
            // where they are.
            // A frost, for one who has a hearth for it: as good as a
            // field, it's what the hearth's for.
            SiteKind::Anomaly
                if hearth_kit(scene) != super::harvest::Hearth::None
                    && scene.anomalies.iter().any(|(_, a, g)| {
                        a.kind == AnomalyKind::Frost && g.translation().xz().distance(s.at) < 3.0
                    }) =>
            {
                r.traits.fields * 3.0
            }
            SiteKind::Anomaly => r.traits.fields * 1.5,
            // Picking over a wreck is for when nothing else is coming.
            SiteKind::Wreck(_) if r.hp < 60.0 || heat > 0.0 => 0.0,
            SiteKind::Wreck(_) => 4.0,
        };
        if pull <= 0.0 {
            continue;
        }
        let d = s.at.distance(here);
        // Later in the raid, sites toward the rendezvous side look better.
        let toward = (here.distance(rv_side) - s.at.distance(rv_side)) / 200.0;
        // What a player reads off the field map: richer (riskier) regions
        // are worth the walk, picked-over ones less so. Loot richness
        // already rises with danger (docs/MAP_DESIGN.md, "what the bots
        // value").
        // (A job is worth the walk whatever's on the shelves there.)
        // An underground site is worth what's down there, not the
        // surface it lies under (its kiosk's stencil says).
        let below = matches!(s.kind, SiteKind::Building(_))
            .then(|| extraction_core::underground::layout(&s.name))
            .flatten();
        let rich = if s.job {
            1.0
        } else {
            below
                .and_then(|l| l.rich)
                .unwrap_or_else(|| scene.zone.map.richness_at(s.at.to_array()))
        };
        // Its way down under cover: the danger above is skirted, not
        // crossed, for the last of the walk.
        let caution = if below.is_some_and(|l| l.covered_entry) {
            ROUTE_CAUTION * 0.5
        } else {
            ROUTE_CAUTION
        };
        let aim = r.objective.map_or(1.0, |o| {
            1.0 + OBJECTIVE_PULL * (1.0 - s.at.distance(o) / OBJECTIVE_REACH).clamp(0.0, 1.0)
        });
        let score = pull * rich * aim
            / (1.0 + d / 80.0)
            / (1.0 + caution * route_exposure(&scene.zone.map, here, s.at))
            * (1.0 + late * toward.clamp(-0.8, 1.0))
            * r.rng.range(0.75, 1.25);
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, i));
        }
    }
    best.map(|b| b.1)
}

/// Loot spots past this don't make a site look any bigger to a raider.
const BIG_SITE: usize = 8;

/// The job on the card, as a site to walk to (once): a scout's place, a
/// contract's thing to bring (where it lies, once it's laid) or place to
/// plant its relay.
fn add_job_site(r: &mut Raider, scene: &Scene) {
    // `SIM_NO_JOB=1`: play as before the bots took jobs (to compare runs).
    if r.sites.iter().any(|s| s.job && s.name.starts_with("job:"))
        || std::env::var_os("SIM_NO_JOB").is_some()
    {
        return;
    }
    let Some(job) = scene.progression.as_ref().and_then(|p| p.0.job.clone()) else {
        return;
    };
    let (map, place, at, kind) = match job {
        CardJob::Scout { map, place, at, .. } | CardJob::Plant { map, place, at, .. } => {
            (map, place, Vec2::from_array(at), SiteKind::Spot)
        }
        CardJob::Bring {
            map, place, item, ..
        } => {
            // Where it lies (not yet laid: next time).
            let Some(at) = scene
                .items
                .iter()
                .find(|(_, it, ..)| scene.catalog.get(it.kind()).id == item)
                .map(|(_, _, gt, ..)| gt.translation())
            else {
                return;
            };
            let kind = r
                .plans
                .iter()
                .position(|p| p.contains(at, 0.3))
                .map_or(SiteKind::Spot, SiteKind::Building);
            (map, place, at.xz(), kind)
        }
        _ => return,
    };
    if map != scene.opts.map {
        return;
    }
    r.sites.push(Site {
        name: format!("job: {place}"),
        kind,
        at,
        spots: 0,
        visited: false,
        tries: 0,
        job: true,
    });
}

/// A Plant job's place, reached: the relay out of the pack and set down
/// on the ground by it.
fn plant_tasks(me: Entity, scene: &Scene, site: &Site) -> Option<(Entity, Vec<Task>)> {
    let Some(CardJob::Plant {
        item,
        at,
        planted: false,
        ..
    }) = scene.progression.as_ref().and_then(|p| p.0.job.as_ref())
    else {
        return None;
    };
    if !site.name.starts_with("job:") {
        return None;
    }
    let relay = *scene.carried(me, item).first()?;
    let spot = Vec2::from_array(*at);
    // As the contract scenarios plant (`progression::contracts::scenario`):
    // close, the gun slung so the right hand's free, the relay into it from
    // its body slot (drawn) or the pack, a look at the spot, let go.
    let mut t = vec![
        Task::Log(format!("planting the {item}")),
        Task::WalkTo {
            to: spot,
            within: 1.0,
        },
        Task::Stow(Side::Right),
    ];
    let (socket, packed) = scene
        .items
        .get(relay)
        .map(|(.., so, bag)| (so.map(|s| s.socket), bag.is_some()))
        .unwrap_or_default();
    let mut mine: Vec<Entity> = scene
        .owned_sockets
        .iter()
        .filter(|(_, o)| o.0 == me)
        .map(|(e, _)| e)
        .collect();
    mine.sort();
    match socket.and_then(|s| mine.iter().position(|m| *m == s)) {
        Some(slot) => t.extend([
            Task::Draw {
                slot,
                side: Side::Right,
            },
            Task::Wait(0.5),
        ]),
        None if packed => t.extend([
            Task::ToggleBackpack,
            Task::Wait(0.6),
            Task::GrabFromHere {
                item: relay,
                side: Side::Right,
            },
            Task::ToggleBackpack,
            Task::Wait(0.5),
        ]),
        None => t.push(Task::Grab {
            item: relay,
            side: Side::Right,
        }),
    }
    let y = scene.zone.ground(spot.x, spot.y);
    t.extend([
        Task::Wait(0.5),
        Task::LookAt(Vec3::new(spot.x, y, spot.y)),
        Task::Release(Side::Right),
        Task::Wait(1.5),
    ]);
    Some((relay, t))
}

/// `SIM_OVERNIGHT=1`: the valley's safehouse as a job of its own, to spend
/// the night in (`progression::safehouse::sim_overnight` does the claiming
/// and the sleeping once it's inside).
fn add_safehouse_site(r: &mut Raider, scene: &Scene) {
    if std::env::var_os("SIM_OVERNIGHT").is_none() || r.sites.iter().any(|s| s.name == "safehouse")
    {
        return;
    }
    let Some(at) = extraction_core::safehouse::safehouse(&scene.opts.map) else {
        return;
    };
    let at = Vec2::from_array(at);
    let Some(b) = r
        .plans
        .iter()
        .position(|p| p.xf.translation.xz().distance(at) < 2.0)
    else {
        return;
    };
    r.sites.push(Site {
        name: "safehouse".into(),
        kind: SiteKind::Building(b),
        at,
        spots: 0,
        visited: false,
        tries: 0,
        job: true,
    });
}

/// A career's raider, over raid after raid (`SIM_HABIT`, with
/// `SIM_RAID` the raid's number in the career and `SIM_CAREER_SEED` the
/// career's): a creature of habit goes the same way to the same region
/// and out the same rendezvous every raid; a varier takes the regions and
/// the rendezvous in turn. For measuring what the Zone's memory of you
/// does (`robots::memory`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CareerHabit {
    pub varies: bool,
    pub raid: u32,
    pub seed: u64,
}

/// The career habit knob, from the environment, if this is a career run.
pub fn career_habit() -> Option<CareerHabit> {
    let varies = match std::env::var("SIM_HABIT").ok()?.as_str() {
        "habit" => false,
        "varier" => true,
        _ => return None,
    };
    let num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u64>().ok());
    Some(CareerHabit {
        varies,
        raid: num("SIM_RAID").unwrap_or(0) as u32,
        seed: num("SIM_CAREER_SEED").unwrap_or(1),
    })
}

impl CareerHabit {
    /// This raid's objective region and the rendezvous it means to leave by.
    fn choose(&self, map: &MapDef, sites: &[Site]) -> (Option<Vec2>, Option<usize>) {
        let rvs = map.markers_of(&MarkerKind::Rendezvous).count().max(1);
        if !self.varies {
            return (
                choose_objective(map, sites, self.seed),
                Some(self.seed as usize % rvs),
            );
        }
        let regions: Vec<_> = map
            .regions
            .iter()
            .filter(|g| g.danger > 0)
            .filter(|g| {
                sites
                    .iter()
                    .any(|s| extraction_core::geom::point_in_polygon(&g.polygon, s.at.to_array()))
            })
            .collect();
        let objective = (!regions.is_empty()).then(|| {
            let g = regions[(self.raid as usize + self.seed as usize) % regions.len()];
            let n = g.polygon.len().max(1) as f32;
            g.polygon.iter().map(|p| Vec2::from_array(*p)).sum::<Vec2>() / n
        });
        (
            objective,
            Some((self.raid as usize + self.seed as usize) % rvs),
        )
    }
}

/// How much better a site at the raid's objective looks, and how far
/// from the region's middle that fades out, metres.
const OBJECTIVE_PULL: f32 = 3.0;
const OBJECTIVE_REACH: f32 = 120.0;

/// The region a raider sets out for: any region with something to loot
/// in it and some danger (the calm insertion is where it starts), the
/// richer the likelier; never the machines' gate, and the deadliest
/// country less often.
fn choose_objective(map: &MapDef, sites: &[Site], seed: u64) -> Option<Vec2> {
    let regions: Vec<_> = map
        .regions
        .iter()
        .filter(|g| g.danger > 0)
        // Not the machines' own gate: nobody plans a raid around walking
        // into it.
        .filter(|g| {
            !map.markers_of(&MarkerKind::RobotGate)
                .any(|m| extraction_core::geom::point_in_polygon(&g.polygon, m.at))
        })
        .filter(|g| {
            sites
                .iter()
                .any(|s| extraction_core::geom::point_in_polygon(&g.polygon, s.at.to_array()))
        })
        .collect();
    // The deadliest country is a raid's aim now and then, not by default.
    let weights: Vec<f32> = regions
        .iter()
        .map(|g| g.richness() * if g.danger >= 3 { 0.3 } else { 1.0 })
        .collect();
    let g = regions[Rng::new(seed ^ 0x0B1E).weighted(&weights)?];
    let n = g.polygon.len().max(1) as f32;
    Some(g.polygon.iter().map(|p| Vec2::from_array(*p)).sum::<Vec2>() / n)
}

fn rv_centroid(zone: &Zone) -> Vec2 {
    let rvs: Vec<Vec2> = zone
        .map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| Vec2::from_array(m.at))
        .collect();
    if rvs.is_empty() {
        return Vec2::ZERO;
    }
    rvs.iter().copied().sum::<Vec2>() / rvs.len() as f32
}

/// Where to walk to visit a site.
fn site_target(r: &Raider, scene: &Scene, s: usize, pos: Vec3) -> Vec2 {
    let site = &r.sites[s];
    match site.kind {
        SiteKind::Building(b) => {
            let plan = &r.plans[b];
            let local = plan.to_local(pos).xz();
            // Not by a door held shut (an underground site's exit, on its
            // power): the nearest one that opens.
            let held = |e: &Vec2| {
                plan.doors.iter().any(|d| {
                    scene.power_locked.contains(d.entity)
                        && d.outside.is_some_and(|o| o.distance(*e) < 0.01)
                })
            };
            plan.entries
                .iter()
                .filter(|e| !held(e))
                .min_by(|a, b| a.distance(local).total_cmp(&b.distance(local)))
                .map(|e| plan.world2(*e))
                .unwrap_or(site.at)
        }
        // Its edge on this side: its middle is no place to walk to (the
        // grid keeps out of it).
        SiteKind::Anomaly => {
            let rad = anomaly_radius(scene, site.at);
            site.at + (pos.xz() - site.at).normalize_or(Vec2::X) * (rad + 2.5)
        }
        _ => site.at,
    }
}

/// The radius of the anomaly standing at `at` (a standing one's marker),
/// or a typical one's if none is there.
fn anomaly_radius(scene: &Scene, at: Vec2) -> f32 {
    scene
        .anomalies
        .iter()
        .find(|(_, _, gt)| gt.translation().xz().distance(at) < 3.0)
        .map_or(3.0, |(_, a, _)| a.radius)
}

/// Vents opened (site, vent): their zones' cells walked again in those
/// sites' plans.
fn vent_plans(plans: &mut [BuildingPlan], opened: impl Iterator<Item = (Entity, usize)>) {
    for (site, vent) in opened {
        for plan in plans.iter_mut().filter(|p| p.entity == site) {
            let zones = extraction_core::underground::layout(&plan.name)
                .and_then(|l| l.vents.get(vent))
                .map(|v| v.zones.clone())
                .unwrap_or_default();
            plan.vent(&zones);
        }
    }
}

/// How long to give a vented flue to cool before going through (s): it's
/// past scorching in five, and gentle by twenty.
const VENT_WAIT: f32 = 20.0;

/// To a local point a site's heat shuts off: by one of its vent levers
/// (closed still): the lever, the way to it, pulled and a wait while the
/// flue cools; and the way on through from it.
fn vent_first(
    scene: &Scene,
    plan: &BuildingPlan,
    pos: Vec3,
    goal: Vec2,
) -> Option<(Entity, Vec<Task>, Vec<Task>)> {
    if plan.hot.is_empty() {
        return None;
    }
    let l = extraction_core::underground::layout(&plan.name)?;
    for (lever, v, gt) in scene
        .vent_levers
        .iter()
        .filter(|(_, v, _)| v.site == plan.entity && !v.open)
    {
        let Some(def) = l.vents.get(v.vent) else {
            continue;
        };
        let mut cool = plan.clone();
        cool.vent(&def.zones);
        let at = gt.translation();
        let Some(mut tasks) = indoor(plan, pos, plan.to_local(at).xz(), 0.5) else {
            continue;
        };
        let Some(on) = indoor(&cool, at, goal, 0.3) else {
            continue;
        };
        tasks.push(Task::Press(lever));
        tasks.push(Task::Wait(VENT_WAIT));
        return Some((lever, tasks, on));
    }
    None
}

/// Indoors: `metres` along the building's plan from `pos` toward `to`
/// (world x, z), doors opened on the way. None outdoors, or with no way.
fn indoor_step(r: &Raider, pos: Vec3, to: Vec2, metres: f32) -> Option<Vec<Task>> {
    let plan = r
        .plans
        .iter()
        .find(|p| p.in_grid(pos) && (p.indoors(p.to_local(pos).xz()) || p.contains(pos, -0.1)))?;
    let a = plan.to_local(pos).xz();
    let goal = plan.to_local(Vec3::new(to.x, pos.y, to.y)).xz();
    let path = plan.path(a, goal)?;
    // The point `metres` along it.
    let (mut at, mut left) = (a, metres);
    for p in path {
        let d = at.distance(p);
        if d >= left {
            at = at.lerp(p, left / d.max(1e-4));
            break;
        }
        left -= d;
        at = p;
    }
    indoor_to(plan, pos, at, 0.6)
}

/// Indoors, out the nearest door first: the Zone grid knows nothing of
/// walls, and a straight line from a back room goes through one.
/// Whether a wreck at `at` lies in one of `plans`: inside a building's
/// walls, or anywhere over a site's floors below the ground, however high
/// up its walls it was when it was killed (a crawler, wrecked on the deep
/// kettle's wall, was made an outdoor site, and the walk to its loot went
/// straight through the rock: Kettle sims, 30 October).
fn wreck_inside(plans: &[BuildingPlan], zone: &crate::world::Zone, at: Vec3) -> bool {
    let under = zone.ground(at.x, at.z) - at.y > 0.5;
    plans.iter().any(|p| {
        if p.levels.is_some() {
            let l = p.to_local(at).xz();
            under && p.grid.cell_of(l.to_array()).is_some() && p.indoors(l) || p.contains(at, 0.0)
        } else {
            p.contains(at, 0.0)
        }
    })
}

/// Off a plough's road, if `here` is in its lane ahead (where it sounds
/// its horn: it doesn't stop, and its auger takes what's there; Rime sims,
/// 30 October, a raider walking the pass road to the RV went under one):
/// where to stand clear of its blade, to the side it's on (away from the
/// chute's side if it's in the middle), on open ground; and how long to
/// wait there for it to go by. None if it's not in the way.
fn plough_aside(grid: &NavGrid, here: Vec2, plough: Vec2, forward: Vec2) -> Option<(Vec2, f32)> {
    use extraction_core::plough as rules;
    let fwd = forward.normalize_or_zero();
    // (Its chute throws to its right.)
    let right = Vec2::new(-fwd.y, fwd.x);
    let d = here - plough;
    let (ahead, side) = (d.dot(fwd), d.dot(right));
    if !rules::in_lane_ahead(ahead, side) {
        return None;
    }
    let first = if side > 0.3 { 1.0 } else { -1.0 };
    let wait = (ahead + rules::MOUTH + 4.0) / rules::SPEED;
    [first, -first]
        .into_iter()
        .flat_map(|s| [3.0, 5.0].map(move |m| (s, m)))
        .map(|(s, m)| here - right * side + right * s * (rules::BLADE * 0.5 + m))
        .find(|p| grid.walkable(p.to_array()))
        .map(|p| (p, wait))
}

/// Away from a robot at `at` in a hurry, about `metres` off: inside a
/// building or down a site, out by its nearest way out first (the open
/// ground's grid has none of their walls, and a run on it went into them:
/// Kettle sims, 30 October, raiders fleeing a crawler in the mine); on
/// open ground, straight away.
fn run_from(
    plans: &[BuildingPlan],
    grid: &NavGrid,
    pos: Vec3,
    at: Vec3,
    metres: f32,
    shut: &dyn Fn(Entity) -> bool,
) -> Option<Vec<Task>> {
    if let Some(out) = leave_plans(plans, pos, shut) {
        return Some(out);
    }
    let away = (pos.xz() - at.xz()).normalize_or(Vec2::X);
    travel(grid, pos.xz(), pos.xz() + away * metres, 2.0)
}

fn leave_building(r: &Raider, scene: &Scene, pos: Vec3) -> Option<Vec<Task>> {
    leave_plans(&r.plans, pos, &|d| scene.power_locked.contains(d))
}

/// Out of whichever of `plans` `pos` is in, by its nearest way out (a
/// door `shut` doesn't count). None if it's in none, or there's no way.
fn leave_plans(
    plans: &[BuildingPlan],
    pos: Vec3,
    shut: &dyn Fn(Entity) -> bool,
) -> Option<Vec<Task>> {
    let plan = plans
        .iter()
        .find(|p| p.in_grid(pos) && (p.indoors(p.to_local(pos).xz()) || p.contains(pos, -0.1)));
    let why = |what: &dyn Fn() -> String| {
        if std::env::var_os("SIM_DEBUG").is_some() {
            eprintln!("leave_building at {pos:?}: {}", what());
        }
    };
    let Some(plan) = plan else {
        if plans.iter().any(|p| p.levels.is_some() && p.in_grid(pos)) {
            why(&|| "in an underground site's grid but not in it".into());
        }
        return None;
    };
    let Some(exit) = way_out(plan, plan.to_local(pos).xz(), shut) else {
        why(&|| format!("{}: no way out", plan.name));
        return None;
    };
    let out = indoor_out(plan, pos, exit, 0.6);
    if out.is_none() {
        why(&|| {
            let l = plan.to_local(pos).xz();
            format!(
                "{}: no route to {exit:?} from {l:?} (walkable {}, held {}, a path {})",
                plan.name,
                plan.grid.walkable(l.to_array()),
                plan.held.len(),
                plan.path_through_held(l, exit).is_some()
            )
        });
    }
    out
}

/// Where to walk to be out of a building from a local point: the nearest
/// door stand or point round the footprint by the walk.
pub(crate) fn way_out(
    plan: &BuildingPlan,
    local: Vec2,
    shut: &dyn Fn(Entity) -> bool,
) -> Option<Vec2> {
    // All the way out of the footprint, the shortest way round: a hall
    // with open bays counts as "outside" for its doors, and leaving only
    // the office in its corner left a bot inside, walking at the wall.
    let ring = {
        let h = plan.half + Vec2::splat(1.2);
        let steps = |len: f32| ((len / 3.0).ceil() as usize).max(1);
        let mut v = Vec::new();
        for (a, b) in [
            (Vec2::new(-h.x, -h.y), Vec2::new(h.x, -h.y)),
            (Vec2::new(h.x, -h.y), Vec2::new(h.x, h.y)),
            (Vec2::new(h.x, h.y), Vec2::new(-h.x, h.y)),
            (Vec2::new(-h.x, h.y), Vec2::new(-h.x, -h.y)),
        ] {
            let n = steps(a.distance(b));
            v.extend((0..n).map(|k| a.lerp(b, k as f32 / n as f32)));
        }
        v
    };
    // Each is a path search: the doors, and the nearest few of the ring.
    // (Not round an underground site: a ring point there can land on the
    // stair under the kiosk. Its kiosks' doors are the ways out.)
    let mut ring: Vec<Vec2> = ring
        .into_iter()
        .filter(|p| plan.levels.is_none() && plan.grid.walkable(p.to_array()))
        .collect();
    ring.sort_by(|a, b| a.distance(local).total_cmp(&b.distance(local)));
    ring.truncate(8);
    // A door stand inside the footprint (the office's, in an open hall)
    // isn't out.
    let out = |e: &Vec2| e.x.abs() > plan.half.x || e.y.abs() > plan.half.y;
    // Nor a door held shut (an exit on an underground site's power).
    let open = |e: &Vec2| {
        !plan
            .doors
            .iter()
            .any(|d| shut(d.entity) && d.outside.is_some_and(|o| o.distance(*e) < 0.01))
    };
    plan.entries
        .iter()
        .copied()
        .filter(out)
        .filter(open)
        .chain(ring)
        .filter_map(|e| {
            let path = plan.path(local, e)?;
            let len: f32 = path.windows(2).map(|w| w[0].distance(w[1])).sum();
            // A path that ends short of the exit didn't get there.
            (path.last()?.distance(e) < 0.8).then_some((len, e))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|b| b.1)
}

fn go_to_site(r: &Raider, scene: &Scene, s: usize, pos: Vec3) -> Option<Vec<Task>> {
    let to = site_target(r, scene, s, pos);
    let same = matches!(r.sites[s].kind, SiteKind::Building(b) if r.plans[b].in_grid(pos));
    if !same {
        if let Some(out) = leave_building(r, scene, pos) {
            return Some(out);
        }
    }
    let within = match r.sites[s].kind {
        SiteKind::Building(_) => 0.6,
        SiteKind::Field => 8.0,
        SiteKind::Anomaly => 1.5,
        _ => 2.0,
    };
    travel(&r.grid, pos.xz(), to, within)
}

fn arrived(r: &Raider, scene: &Scene, s: usize, pos: Vec3) -> bool {
    let site = &r.sites[s];
    match site.kind {
        SiteKind::Building(b) => {
            r.plans[b].in_grid(pos) && site_target(r, scene, s, pos).distance(pos.xz()) < 3.0
        }
        SiteKind::Field => site.at.distance(pos.xz()) < 15.0,
        // At its edge, from where the stone is seen (or isn't).
        SiteKind::Anomaly => site.at.distance(pos.xz()) < anomaly_radius(scene, site.at) + 4.0,
        _ => site.at.distance(pos.xz()) < 6.0,
    }
}

/// The best loose item at a site that's worth taking and fits.
fn pick_item(me: Entity, r: &Raider, scene: &Scene, s: usize, pos: Vec3) -> Option<Entity> {
    let site = &r.sites[s];
    let (used, cap) = scene.pack_use(me);
    let slings = scene.free_slings(me);
    let mut best: Option<(f32, Entity)> = None;
    for (e, it, gt, held, sock, bag) in &scene.items {
        if held.is_some() || sock.is_some() || bag.is_some() || r.skip.contains(&e) {
            continue;
        }
        let at = gt.translation();
        if !at_site(r, site, at) {
            continue;
        }
        let def = scene.catalog.get(it.kind());
        if matches!(site.kind, SiteKind::Field | SiteKind::Anomaly)
            && def.category != Category::Artifact
        {
            continue;
        }
        // A contract's thing to bring is worth more than anything.
        let wanted = matches!(
            scene.progression.as_ref().and_then(|p| p.0.job.as_ref()),
            Some(CardJob::Bring { item, .. }) if *item == def.id
        );
        let v = if wanted { 1000.0 } else { r.traits.value(def) };
        if v < r.traits.min_value {
            continue;
        }
        let fits = match def.size {
            SizeClass::Bulky => slings > 0,
            _ => used + def.bulk <= cap + 1e-3 || def.size <= SizeClass::Small,
        };
        if !fits {
            continue;
        }
        let d = at.distance(pos);
        let score = v / (1.0 + d / 10.0);
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, e));
        }
    }
    best.map(|b| b.1)
}

/// Tasks to fetch one item into the pack.
fn take_item(
    r: &mut Raider,
    scene: &Scene,
    s: usize,
    item: Entity,
    pos: Vec3,
) -> Option<Vec<Task>> {
    let at = scene.items.get(item).ok()?.2.translation();
    let grab = [
        Task::Grab {
            item,
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::Wait(0.2),
    ];
    let mut tasks = Vec::new();
    match r.sites[s].kind {
        SiteKind::Building(b) => {
            let plan = &r.plans[b];
            if !plan.in_grid(pos) {
                return None;
            }
            let goal = plan.to_local(at).xz();
            // A contract's thing to bring, somewhere the route can't stand
            // (behind a counter): as near as there's floor, and reach.
            let wanted = matches!(
                scene.progression.as_ref().and_then(|p| p.0.job.as_ref()),
                Some(CardJob::Bring { item: id, .. })
                    if scene.items.get(item).is_ok_and(|(_, it, ..)| scene.catalog.get(it.kind()).id == *id)
            );
            let route = match indoor_through_held(plan, pos, goal, 0.3) {
                Some(route) => route,
                None if wanted => {
                    let near = stand_near(plan, plan.to_local(pos).xz(), goal)?;
                    indoor(plan, pos, near, 0.3)?
                }
                // Behind heat nothing crosses: vent it first (it clangs),
                // unless careful.
                None if r.traits.name != "cautious" => {
                    // To the lever and pull it; the way on waits till it's
                    // open (`think`), not walked into the heat regardless.
                    let (lever, pre, on) = vent_first(scene, plan, pos, goal)?;
                    let mut rest = on;
                    rest.extend(grab);
                    r.after_vent = Some((lever, rest));
                    return Some(pre);
                }
                None => return None,
            };
            // Behind a door on an underground site's power: throw the
            // switches first, stage by stage up to the one that holds it
            // (it's loud: the crawlers come), unless careful.
            let held = route
                .iter()
                .filter_map(|t| match t {
                    Task::OpenDoor(d) => scene.power_locked.get(*d).ok().map(|l| l.stage),
                    _ => None,
                })
                .max();
            if let Some(stage) = held {
                if r.traits.name == "cautious" {
                    return None;
                }
                let first = scene.site_power.get(plan.entity).ok()?.next()?;
                let mut from = pos;
                for k in first..=stage {
                    let (switch, _, gt) = scene
                        .power_switches
                        .iter()
                        .find(|(_, sw, _)| sw.site == plan.entity && sw.stage == k)?;
                    let at_switch = gt.translation();
                    tasks.extend(indoor_through_held(
                        plan,
                        from,
                        plan.to_local(at_switch).xz(),
                        0.5,
                    )?);
                    tasks.push(Task::Press(switch));
                    tasks.push(Task::Wait(1.0));
                    from = at_switch;
                }
                tasks.extend(indoor_through_held(
                    plan,
                    from,
                    plan.to_local(at).xz(),
                    0.3,
                )?);
            } else if route
                .iter()
                .any(|t| matches!(t, Task::OpenDoor(d) if plan.held.contains(d)))
            {
                return None;
            } else {
                tasks.extend(route);
            }
        }
        _ => {
            // Artifacts sit in anomalies: test the snare, then dive.
            let hazard = scene
                .anomalies
                .iter()
                .map(|(_, a, gt)| (a.kind, gt.translation(), a.radius))
                .find(|(_, c, rad)| c.xz().distance(at.xz()) < rad + 0.6);
            if let Some((kind, centre, rad)) = hazard {
                let skilled = super::harvest::skilled(r.traits.name);
                // A dive that costs nothing done right (a frost, a hearth
                // on it) is no risk however hurt it is.
                let free = skilled
                    && kind == AnomalyKind::Frost
                    && hearth_kit(scene) != super::harvest::Hearth::None;
                // Not hurt, it doesn't; and nobody careful reaches into a
                // well's heart (unless it knows how: in and straight out).
                if r.traits.avoid_anomalies
                    && (r.hp < 60.0 && !free || kind == AnomalyKind::Well && !skilled)
                {
                    r.pass_why = Some(format!(
                        "{} ({kind:?}, skilled {skilled}, hearth {:?})",
                        if r.hp < 60.0 { "hurt" } else { "well" },
                        hearth_kit(scene),
                    ));
                    return None;
                }
                let out = (pos.xz() - centre.xz()).normalize_or(Vec2::X);
                let edge = centre.xz() + out * (rad + 1.8);
                tasks.extend(travel(&r.grid, pos.xz(), edge, 0.8).unwrap_or_default());
                if skilled {
                    let first = junk(r, scene);
                    let kit = super::harvest::Kit {
                        junk: first,
                        spare: first.and_then(|j| junk_but(r, scene, j)),
                        gun: r.gun,
                        hearth: hearth_kit(scene),
                    };
                    if let Some(way) = super::harvest::plan(kind, centre, rad, edge, item, at, &kit)
                    {
                        r.diving = true;
                        tasks.extend(way);
                        return Some(tasks);
                    }
                    // Into a frost without a hearth, nobody who knows does.
                    if kind == AnomalyKind::Frost {
                        r.pass_why = Some("frost without a hearth".into());
                        return None;
                    }
                }
                if matches!(kind, AnomalyKind::Snare | AnomalyKind::Mirror)
                    && r.traits.test_anomalies
                {
                    if let Some(junk) = junk(r, scene) {
                        tasks.push(Task::Grab {
                            item: junk,
                            side: Side::Left,
                        });
                        tasks.push(Task::Throw {
                            side: Side::Left,
                            at: centre,
                        });
                        tasks.push(Task::Wait(0.6));
                    }
                }
                r.diving = true;
                tasks.extend(grab);
                tasks.push(Task::WalkTo {
                    to: edge,
                    within: 0.8,
                });
                return Some(tasks);
            }
            if at.xz().distance(pos.xz()) > 12.0 {
                tasks.extend(travel(&r.grid, pos.xz(), at.xz(), 1.0).unwrap_or_default());
            }
        }
    }
    tasks.extend(grab);
    Some(tasks)
}

/// Something else worthless from the pack, not `first`.
fn junk_but(r: &Raider, scene: &Scene, first: Entity) -> Option<Entity> {
    let me = scene.bags.iter().find(|b| b.carried)?;
    me.contents
        .iter()
        .filter(|e| **e != first)
        .filter_map(|e| scene.items.get(*e).ok())
        .map(|(e, it, ..)| (e, r.traits.value(scene.catalog.get(it.kind()))))
        .filter(|(_, v)| *v <= 3.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// Something worthless from the pack, to throw.
fn junk(r: &Raider, scene: &Scene) -> Option<Entity> {
    let me = scene.bags.iter().find(|b| b.carried)?;
    me.contents
        .iter()
        .filter_map(|e| scene.items.get(*e).ok())
        .map(|(e, it, ..)| (e, r.traits.value(scene.catalog.get(it.kind()))))
        .filter(|(_, v)| *v <= 3.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// The hearth on it: loose (live), or sealed in a lead case, in its pack
/// or a body slot (a stowed case goes to a free belt slot before the pack).
fn hearth_kit(scene: &Scene) -> super::harvest::Hearth {
    use super::harvest::Hearth;
    let Some(me) = scene.bags.iter().find(|b| b.carried) else {
        return Hearth::None;
    };
    let is = |e: Entity, id: &str| {
        scene
            .items
            .get(e)
            .is_ok_and(|(_, it, ..)| scene.catalog.get(it.kind()).id == id)
    };
    let in_case = |case: Entity| {
        scene
            .sockets
            .iter()
            .filter(|(_, owner)| owner.0 == case)
            .find_map(|(sock, _)| sock.occupant.filter(|h| is(*h, "art_hearth")))
    };
    // In the pack, then on the body.
    let packed = me.contents.iter().map(|e| (*e, true));
    let worn = scene
        .sockets
        .iter()
        .filter(|(_, owner)| owner.0 == me.owner)
        .filter_map(|(sock, _)| sock.occupant)
        .map(|e| (e, false));
    let on: Vec<(Entity, bool)> = packed.chain(worn).collect();
    if on.iter().any(|(e, _)| is(*e, "art_hearth")) {
        return Hearth::Live;
    }
    on.into_iter()
        .filter(|(e, _)| is(*e, "lead_case"))
        .find_map(|(case, packed)| {
            in_case(case).map(|stone| Hearth::Sealed {
                case,
                stone,
                packed,
            })
        })
        .unwrap_or(Hearth::None)
}

/// Getting out: radio, walk to the RV, board, pull the bell.
fn extract_step(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    here: Vec2,
    pos: Vec3,
    children: &Children,
) -> Vec<Task> {
    let zone = &scene.zone;
    if let Some(held) = scene.hand(children, Side::Left) {
        let _ = held;
        return vec![
            Task::Stow(Side::Left),
            Task::Wait(0.1),
            Task::Release(Side::Left),
        ];
    }
    let rvs: Vec<(String, Vec2)> = zone
        .map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.clone(), Vec2::from_array(m.at)))
        .collect();
    if rvs.is_empty() {
        return vec![Task::Wait(5.0)];
    }
    // The truck goes to the RV nearest the caller, so that's the one.
    let nearest = rvs
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.1.distance(here).total_cmp(&b.1.1.distance(here)))
        .map(|(i, _)| i)
        .unwrap();
    let rv = *r.rv.get_or_insert(nearest);
    let pose = crate::raid::truck::rv_pose(zone, rv);
    let p = |z: f32| pose.transform_point(Vec3::new(0.0, 0.0, z)).xz();
    let approach = p(8.0);
    let phase = scene.raid.phase.clone();
    // Call the truck once the walk is short enough to make its window
    // (two minutes to come, five to wait), and again if it gave up.
    let pace =
        crate::player::WALK_SPEED * extraction_core::inventory::Encumbrance::from_mass(r.kg).speed;
    let walk_secs = here.distance(approach) * 1.35 / pace.max(0.5);
    let can_call = phase == Phase::Out && r.clock - r.radioed_at > 20.0;
    if can_call && walk_secs < crate::raid::TRUCK_TRANSIT + 60.0 && nearest == rv {
        if let Some(radio) = scene.carried(me, "radio").first().copied() {
            r.radioed_at = r.clock;
            log.event(
                "radio",
                json!({"rv": rvs[rv].0, "x": round(here.x, 1), "z": round(here.y, 1),
                       "walk_secs": round(walk_secs, 0)}),
            );
            return vec![
                Task::Grab {
                    item: radio,
                    side: Side::Left,
                },
                Task::Use(Side::Left),
                Task::Wait(0.3),
                Task::Stow(Side::Left),
            ];
        }
    }
    // Till the truck's stopped, wait in a shelter's lee if drones watch
    // the place; then walk out to the tailgate.
    let waiting = matches!(phase, Phase::TruckWaiting { .. });
    if r.wait_at.is_none_or(|(at, _)| at != rv) {
        let at = rvs[rv].1.to_array();
        let lee = extraction_core::map_lint::watched_by(&zone.map, at).and_then(|(poi, _)| {
            let from = zone
                .map
                .markers_of(&MarkerKind::Poi)
                .find(|m| m.name == poi)?
                .at;
            extraction_core::map_lint::wait_shelter(&zone.map, at, from)
                .map(|(_, _, lee)| Vec2::from_array(lee))
        });
        r.wait_at = Some((rv, lee));
    }
    let goal = match r.wait_at {
        Some((_, Some(lee))) if !waiting => lee,
        _ => approach,
    };
    // Under a roof it's in at the door or it's not under it: in all the
    // way, by the building's plan, and not walked back out of it.
    let goal3 = Vec3::new(goal.x, pos.y, goal.y);
    let roof = r
        .plans
        .iter()
        .position(|p| goal != approach && p.indoors(p.to_local(goal3).xz()));
    let near = if roof.is_some() { 0.8 } else { 4.0 };
    r.at_rv = here.distance(goal) <= near;
    if here.distance(goal) > near {
        r.act = Act::ToRv;
        if let Some(plan) = roof.map(|b| &r.plans[b]).filter(|p| p.in_grid(pos)) {
            if let Some(route) = indoor(plan, pos, plan.to_local(goal3).xz(), 0.4) {
                return route;
            }
        } else if let Some(out) = leave_building(r, scene, pos) {
            return out;
        } else if let Some(plan) = r
            .plans
            .iter()
            .find(|p| p.levels.is_some() && p.contains(pos, 0.0))
        {
            // Down a site with no way out found: never the Zone's straight
            // line, which walks into the rock. Its nearest way up, or wait
            // for one to open.
            return plan
                .entries
                .iter()
                .filter_map(|e| indoor_to(plan, pos, *e, 0.6))
                .min_by_key(|t| t.len())
                .unwrap_or_else(|| vec![Task::Wait(1.0)]);
        }
        return travel(&r.grid, here, goal, 2.0).unwrap_or_else(|| {
            vec![Task::WalkTo {
                to: goal,
                within: 2.0,
            }]
        });
    }
    match phase {
        Phase::TruckWaiting { .. } => {
            let bell = scene
                .pressables
                .iter()
                .find(|(_, p)| matches!(p, Pressable::BellPull))
                .map(|(e, _)| e);
            r.act = Act::Board;
            board_tasks(p, bell)
        }
        _ => vec![Task::Wait(2.0)],
    }
}

/// Its own things knocked loose (a robot's blow, a fall): anything worth
/// more than junk that was on it last tick and now lies within 15 m, out
/// of any anomaly, it goes back for at once. Not what it set down on
/// purpose (`skip`), nor junk it threw, nor while it's fighting.
fn knocked_loose(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    pos: Vec3,
) -> Option<Vec<Task>> {
    let worth = |e: Entity| {
        scene
            .items
            .get(e)
            .is_ok_and(|(_, it, ..)| r.traits.value(scene.catalog.get(it.kind())) > 3.0)
    };
    let before = std::mem::take(&mut r.kept);
    r.kept = scene
        .items
        .iter()
        .map(|(e, ..)| e)
        .filter(|e| scene.mine(me, *e) && worth(*e))
        .collect();
    if r.combat.is_some() {
        return None;
    }
    let lost = before.into_iter().find(|e| {
        !r.skip.contains(e)
            && scene.items.get(*e).is_ok_and(|(_, _, gt, h, so, b)| {
                let at = gt.translation();
                h.is_none()
                    && so.is_none()
                    && b.is_none()
                    && at.distance(pos) < 15.0
                    && out_of_anomaly(scene, at.xz()).is_none()
            })
    })?;
    let id = scene.items.get(lost).map_or(String::new(), |(_, it, ..)| {
        scene.catalog.get(it.kind()).id.clone()
    });
    log.event("fetch_kit", json!({"item": id}));
    Some(vec![
        Task::Grab {
            item: lost,
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::Wait(0.2),
    ])
}

/// Kit upkeep between jobs: pick up the magazines it dropped reloading,
/// and top them up from the ammunition it carries.
fn maintain(
    me: Entity,
    r: &mut Raider,
    scene: &Scene,
    log: &mut SimLog,
    pos: Vec3,
    _gun: Option<Entity>,
    children: &Children,
) -> Option<Vec<Task>> {
    let g = r.gun?;
    // Its gun wandered off (snagged, dropped): go and get it, sling it.
    if !scene.mine(me, g) {
        let far = scene
            .items
            .get(g)
            .map(|(_, _, gt, ..)| gt.translation().distance(pos))
            .unwrap_or(0.0);
        if far > 1.5 {
            log.event("fetch_gun", json!({"dist": round(far, 1)}));
            let mut t = Vec::new();
            if scene.hand(children, Side::Right).is_some() {
                t.push(Task::Release(Side::Right));
            }
            t.extend([
                Task::Grab {
                    item: g,
                    side: Side::Right,
                },
                Task::Stow(Side::Right),
            ]);
            return Some(t);
        }
    }
    let family = scene
        .guns
        .get(g)
        .ok()?
        .firearm
        .def
        .magazine_family
        .clone()?;
    // Its own empties, lying where they fell.
    let dropped = scene.items.iter().find(|(e, it, gt, h, s, b)| {
        h.is_none()
            && s.is_none()
            && b.is_none()
            && !r.skip.contains(e)
            && gt.translation().distance(pos) < 10.0
            && scene.mags.get(*e).is_ok_and(|(_, m)| m.family == family)
            && it.rounds < 5
    });
    if let Some((e, ..)) = dropped {
        r.skip.insert(e);
        return Some(vec![
            Task::Grab {
                item: e,
                side: Side::Left,
            },
            Task::Stow(Side::Left),
            Task::Wait(0.2),
        ]);
    }
    if r.clock - r.last_refill < 20.0 {
        return None;
    }
    // A magazine with room and a box that fits it.
    let mine: Vec<Entity> = scene
        .items
        .iter()
        .filter(|(e, ..)| scene.mine(me, *e))
        .map(|(e, ..)| e)
        .collect();
    let mag = mine.iter().copied().find(|e| {
        scene
            .mags
            .get(*e)
            .is_ok_and(|(it, m)| m.family == family && it.rounds + 5 <= m.capacity)
    })?;
    let cart = scene
        .items
        .get(mag)
        .ok()
        .and_then(|(_, it, ..)| scene.catalog.get(it.kind()).magazine.clone())?
        .cartridge;
    let ammo = mine.iter().copied().find(|e| {
        scene.items.get(*e).is_ok_and(|(_, it, ..)| {
            let d = scene.catalog.get(it.kind());
            d.category == Category::Ammo
                && d.cartridge.as_deref() == Some(cart.as_str())
                && it.rounds > 0
        })
    })?;
    r.last_refill = r.clock;
    log.event("refill", json!({}));
    // Magazine and box in hand, together, away.
    Some(vec![
        Task::Stow(Side::Right),
        Task::Grab {
            item: mag,
            side: Side::Right,
        },
        Task::Grab {
            item: ammo,
            side: Side::Left,
        },
        Task::Combine,
        Task::Wait(0.3),
        Task::Stow(Side::Left),
        Task::Stow(Side::Right),
        Task::Wait(0.2),
    ])
}

/// Into the truck from behind (`p` maps a distance along the truck's
/// rear axis to the ground), then the bell.
pub fn board_tasks(p: impl Fn(f32) -> Vec2, bell: Option<Entity>) -> Vec<Task> {
    let mut t = vec![
        Task::WalkTo {
            to: p(4.2),
            within: 0.4,
        },
        Task::WalkTo {
            to: p(1.6),
            within: 0.35,
        },
        Task::WalkTo {
            to: p(0.0),
            within: 0.35,
        },
    ];
    if let Some(bell) = bell {
        t.push(Task::Press(bell));
    }
    t.push(Task::Wait(1.0));
    t
}

/// Whether a point is part of a site (inside the building, by the marker,
/// in the field).
fn at_site(r: &Raider, site: &Site, at: Vec3) -> bool {
    match site.kind {
        SiteKind::Building(b) => {
            let plan = &r.plans[b];
            plan.contains(at, 0.3) && plan.reachable_height(at)
        }
        SiteKind::Spot => at.xz().distance(site.at) < 5.0,
        SiteKind::Field => at.xz().distance(site.at) < 40.0,
        SiteKind::Anomaly => at.xz().distance(site.at) < 4.5,
        SiteKind::Wreck(_) => at.xz().distance(site.at) < 5.0,
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A machine wrecked down a site is the site's to search, wherever on
    /// its walls it died (a crawler 3 m up the deep kettle's wall, made an
    /// outdoor site, sent raiders straight through the rock: Kettle sims,
    /// 30 October); one wrecked on the open ground beside it is its own.
    #[test]
    fn a_wreck_down_a_site_is_the_sites() {
        let opts = crate::launch::LaunchOptions {
            map: "kettle_pass".into(),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let plans: Vec<BuildingPlan> = app
            .world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap();
        let mut tried = 0;
        for plan in plans.iter().filter(|p| p.levels.is_some()) {
            let g = &plan.grid;
            let floors: Vec<Vec2> = (0..g.n * g.n)
                .map(|k| Vec2::from(g.centre(k % g.n, k / g.n)))
                .filter(|c| g.walkable(c.to_array()) && plan.indoors(*c))
                .filter(|c| {
                    let w = plan
                        .xf
                        .transform_point(Vec3::new(c.x, plan.floor_at(*c), c.y));
                    zone.ground(w.x, w.z) - w.y > 4.0
                })
                .collect();
            for c in floors.iter().step_by(37).take(6) {
                let floor = plan
                    .xf
                    .transform_point(Vec3::new(c.x, plan.floor_at(*c), c.y));
                for up in [0.3, 3.0] {
                    tried += 1;
                    assert!(
                        wreck_inside(&plans, &zone, floor + Vec3::Y * up),
                        "{}: a wreck {up} m over its floor at {c} made an outdoor site",
                        plan.name
                    );
                }
            }
        }
        assert!(tried >= 12, "tried {tried}");
        // Out on the open ground, a valley's width from anything.
        let open = zone.on_ground(0.0, 0.0) + Vec3::Y * 0.5;
        if !plans.iter().any(|p| p.contains(open, 2.0)) {
            assert!(
                !wreck_inside(&plans, &zone, open),
                "one in the open is the site's"
            );
        }
    }

    /// Buildings with loot nobody can stand in arm's reach of (furniture or
    /// walls crowd every floor cell near it). Empty: the blast furnace and
    /// the mill race were on it (31 October), their plans' floors at the
    /// ground under a cast house's and a hut's raised floor, a body never
    /// fitting on the slab; their recipes give their floors now.
    const OUT_OF_REACH: &[&str] = &[];

    /// Everything to take in a building can be taken from where a raider
    /// stops for it: somewhere within its route's last stop of the cell it
    /// makes for, a body has room to stand and the thing is in arm's reach
    /// (Cinder sims, 31 October: raiders stood 0.3 m short of a cell beside
    /// a tool chest and a garage's benches, the furniture in the way, and
    /// never reached over). Every loot spot and loose thing in a building,
    /// on every map.
    #[test]
    fn every_thing_in_a_building_is_taken_from_where_it_stops() {
        let mut out = vec![];
        let mut tried = 0;
        for map in crate::test_maps() {
            let opts = crate::launch::LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            let plans: Vec<BuildingPlan> = app
                .world_mut()
                .run_system_once(
                    |b: Query<(Entity, &Building, &Transform)>,
                     d: Query<(Entity, &DoorLeaf, &ChildOf)>| {
                        plan_buildings(&b, &d)
                    },
                )
                .unwrap();
            let mut things: Vec<Vec3> = app
                .world()
                .resource::<BuildingLoot>()
                .0
                .iter()
                .map(|s| s.at)
                .collect();
            things.extend(
                app.world_mut()
                    .query_filtered::<&GlobalTransform, With<crate::items::Item>>()
                    .iter(app.world())
                    .map(|g| g.translation()),
            );
            let zone = app.world().resource::<crate::world::Zone>().clone();
            let found: Vec<(String, Vec3)> = app
                .world_mut()
                .run_system_once(
                    move |spatial: SpatialQuery,
                          terrain: Query<Entity, With<crate::world::TerrainCollider>>| {
                        let terrain = terrain.iter().next();
                        let mut bad = vec![];
                        let mut n = 0;
                        for at in &things {
                            // (Not up in a boxcar: no raider goes in. Nor the
                            // places whose loot lies where no body stands
                            // in reach of it: Map design's, `OUT_OF_REACH`.)
                            let Some(plan) = plans.iter().find(|p| {
                                p.levels.is_none()
                                    && p.name != "boxcar"
                                    && !OUT_OF_REACH.contains(&p.name.as_str())
                                    && p.contains(*at, 0.0)
                                    && p.reachable_height(*at)
                                    && !p.entries.is_empty()
                            }) else {
                                continue;
                            };
                            let to = plan.to_local(*at).xz();
                            let Some(goal) = reach_cell(plan, plan.entries[0], to) else {
                                continue;
                            };
                            n += 1;
                            let within = stand_within(goal, to, 0.3);
                            // On the floor, or the ground where it's higher (a
                            // scene staged on a slope stands on the slope).
                            let fits = |p: Vec2| {
                                let on = plan.xf.transform_point(Vec3::new(p.x, plan.floor_at(p), p.y));
                                let feet = on.y.max(zone.ground(on.x, on.z));
                                [0.35, 0.95, 1.5].iter().all(|h| {
                                    let w = Vec3::new(on.x, feet + h, on.z);
                                    !crate::world::sphere_in_world(&spatial, &zone, terrain, w, 0.26)
                                })
                            };
                            let stands = (0..=8).any(|r| {
                                let r = within * r as f32 / 8.0;
                                (0..16).any(|k| {
                                    let p = goal + Vec2::from_angle(k as f32 * std::f32::consts::TAU / 16.0) * r;
                                    p.distance(to) <= crate::bot::REACH && fits(p)
                                })
                            });
                            if !stands {
                                bad.push((plan.name.clone(), *at));
                            }
                        }
                        (bad, n)
                    },
                )
                .map(|(bad, n)| {
                    tried += n;
                    bad.into_iter().map(|(name, at)| (name, at)).collect()
                })
                .unwrap();
            out.extend(found.into_iter().map(|(n, at)| (format!("{map} {n}"), at)));
        }
        assert!(tried >= 50, "tried {tried}");
        assert!(
            out.is_empty(),
            "{} can't be taken from where the raider stops: {out:?}",
            out.len()
        );
    }

    /// A raider in a plough's lane ahead gets off its road, clear of its
    /// blade, and waits for it to go by; to its side, or behind it, it
    /// carries on (Rime sims, 30 October: a raider walking the pass road
    /// to the RV went under the plough).
    #[test]
    fn a_raider_gets_off_a_ploughs_road() {
        use extraction_core::plough as rules;
        let open = NavGrid {
            n: 100,
            cell: 1.0,
            origin: -50.0,
            cost: vec![1; 100 * 100],
            lanes: Vec::new(),
        };
        // The plough at the origin heading -z (the game's forward).
        let (at, fwd) = (Vec2::ZERO, Vec2::NEG_Y);
        for (here, why) in [
            (Vec2::new(0.0, -10.0), "dead ahead"),
            (Vec2::new(1.2, -15.0), "ahead, by its chute"),
            (Vec2::new(-1.5, -4.0), "close ahead"),
        ] {
            let (off, wait) =
                plough_aside(&open, here, at, fwd).unwrap_or_else(|| panic!("{why}: stayed"));
            let side = (off - at).dot(Vec2::X).abs();
            assert!(
                side > rules::BLADE * 0.5 + 2.0,
                "{why}: only {side:.1} m off its line"
            );
            // Long enough there for it to have gone by.
            let ahead = (here - at).dot(fwd);
            assert!(
                wait * rules::SPEED > ahead + rules::MOUTH,
                "{why}: back too soon"
            );
        }
        for (here, why) in [
            (Vec2::new(5.0, -10.0), "beside its lane"),
            (Vec2::new(0.0, 6.0), "behind it"),
            (Vec2::new(0.0, -40.0), "far up the road"),
        ] {
            assert!(
                plough_aside(&open, here, at, fwd).is_none(),
                "{why}: stepped off"
            );
        }
        // A drop one side (the pass road's edge): the other side.
        let mut ledge = open.clone();
        for j in 0..100 {
            for i in 50..100 {
                ledge.cost[j * 100 + i] = extraction_core::nav::BLOCKED;
            }
        }
        let (off, _) = plough_aside(&ledge, Vec2::new(0.5, -10.0), at, fwd).expect("off");
        assert!(off.x < 0.0, "over the edge: {off}");
    }

    /// Running from a robot down a site, or in a building, goes out by
    /// the site's own ways, never the open ground's straight line through
    /// its walls (Kettle sims, 30 October: raiders fleeing a crawler in
    /// the mine ran into its rock). From deep in Kettle's mine and its
    /// deep kettle, every leg of the run is clear on the site's own plan.
    #[test]
    fn a_run_from_a_robot_underground_keeps_to_the_site() {
        let opts = crate::launch::LaunchOptions {
            map: "kettle_pass".into(),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let plans: Vec<BuildingPlan> = app
            .world_mut()
            .run_system_once(
                |b: Query<(Entity, &Building, &Transform)>,
                 d: Query<(Entity, &DoorLeaf, &ChildOf)>| plan_buildings(&b, &d),
            )
            .unwrap();
        // Open ground with nothing on it: what the raider's own grid knows
        // of a site's walls underground.
        let open = NavGrid {
            n: 400,
            cell: 2.0,
            origin: -400.0,
            cost: vec![1; 400 * 400],
            lanes: Vec::new(),
        };
        let mut through = vec![];
        let mut tried = 0;
        for plan in plans.iter().filter(|p| p.levels.is_some()) {
            let g = &plan.grid;
            // The few floor cells furthest from its ways in.
            let mut deep: Vec<Vec2> = (0..g.n * g.n)
                .map(|k| Vec2::from(g.centre(k % g.n, k / g.n)))
                .filter(|c| g.walkable(c.to_array()) && plan.contains(plan.to_world(*c), 0.0))
                .collect();
            let far = |c: &Vec2| {
                plan.entries
                    .iter()
                    .map(|e| e.distance(*c))
                    .fold(f32::INFINITY, f32::min)
            };
            deep.sort_by(|a, b| far(b).total_cmp(&far(a)));
            for c in deep.iter().step_by(7).take(4) {
                let feet = plan
                    .xf
                    .transform_point(Vec3::new(c.x, plan.floor_at(*c), c.y));
                let robot = feet + Vec3::X * 3.0;
                let Some(route) = run_from(&plans, &open, feet, robot, 40.0, &|_| false) else {
                    continue;
                };
                tried += 1;
                // Each leg sampled every quarter metre: inside the site,
                // on its floor (or in a doorway).
                let door = |q: Vec2| {
                    plan.doors.iter().any(|d| {
                        let t = (q - d.a).dot(d.b - d.a) / (d.b - d.a).length_squared();
                        (q - d.a.lerp(d.b, t.clamp(0.0, 1.0))).length() < 1.0
                    })
                };
                let mut at = feet.xz();
                'legs: for t in &route {
                    let Task::WalkTo { to, .. } = t else {
                        continue;
                    };
                    let steps = (at.distance(*to) / 0.25).ceil().max(1.0) as usize;
                    for k in 0..=steps {
                        let w = at.lerp(*to, k as f32 / steps as f32);
                        let q = plan.to_local(Vec3::new(w.x, feet.y, w.y)).xz();
                        let inside = plan.contains(Vec3::new(w.x, feet.y, w.y), 0.0);
                        // (Its ways out are where a run out of it ends.)
                        let exit = plan.entries.iter().any(|e| e.distance(q) < 0.5);
                        // (A path's smoothing may graze a corner: floor
                        // within a cell of it is on its way.)
                        let floor = [(0.0, 0.0), (0.3, 0.0), (-0.3, 0.0), (0.0, 0.3), (0.0, -0.3)]
                            .iter()
                            .any(|(x, z)| g.walkable([q.x + x, q.y + z]));
                        if inside && !floor && !door(q) && !exit {
                            through.push(format!("{}: from {at} to {to}, at {q}", plan.name));
                            break 'legs;
                        }
                    }
                    at = *to;
                }
            }
        }
        assert!(tried >= 4, "tried {tried}");
        assert!(through.is_empty(), "ran through the rock: {through:#?}");
    }
}
