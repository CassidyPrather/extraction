//! The training yard's first run: learning by doing.
//!
//! The yard used to be a gravel track of signs to read, and the first
//! headset session couldn't get started from it ("can't really figure out
//! how to start out here"). Now it is a line of short stations, each asking
//! for one or two things and noticing when you've done them, through the
//! same events the game itself runs on (a grab, a holster, a shot that
//! hits a plate). Each station is a post with a lamp and a board:
//!
//! - the **lamp** is lit on the station you're on, green on the ones done,
//!   dark on the ones ahead (a hooded lantern on a post: it must not look
//!   like an anomaly, which the first player avoided as a "tutorial
//!   marker");
//! - the **board** lists the station's steps, ticks them off, and shows how
//!   to do the next one in a few big words, with the control ("Squeeze
//!   GRIP", or in flat mode the key);
//! - finishing a step ticks it, chimes, buzzes both hands, and the driver
//!   says on the radio (your wrist) where to go next.
//!
//! The first station hangs in the truck, over the crate your kit is in:
//! that's the first thing you see. A board by the tailgate lists every
//! station and has START OVER and ON TO THE ZONE; every post has a SKIP
//! card for a station that's got you stuck. The truck's cord ends the yard
//! once you've done it (pulled early, it asks you to pull again).
//!
//! Logic here runs headless; the posts, lamps and props are dressed by
//! [`visuals`].

pub mod firsts;
pub mod hints;
pub mod second_hour;
pub mod visuals;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::items::Category;

use crate::anomaly::Anomaly;
use crate::camp::TRAINING_YARD;
use crate::interact::body::Backpack;
use crate::interact::{GrabPoint, Grabbable, Held, InteractEvent, Socket, SocketKind};
use crate::items::{Catalog, Item, SpawnItem, spawn_item};
use crate::launch::LaunchOptions;
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::truck::{FLOOR, Pressable, Stash, Truck, base_pose};
use crate::raid::{Phase, RadioCall, Raid};
use crate::rig::haptics::{Haptic, StepDone};
use crate::rig::{LocalPlayer, RigDriver, Side};
use crate::weapons::ballistics::Hit;
use crate::weapons::{Gun, GunEvent};
use crate::world::buildings::{Building, Door};
use crate::world::{Layer, Zone};

/// One thing a station asks you to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Goal {
    TakePistol,
    TakeRadio,
    Turn,
    Crouch,
    Run,
    PutAway,
    Pack,
    FarGrab,
    FillMag,
    SeatMag,
    Rack,
    Safety,
    HitPlate,
    DropMag,
    OpenDoor,
    Search,
    Wrist,
    Dressing,
    Probe,
    Artifact,
    Button,
    CallTruck,
    PullCord,
}

impl Goal {
    /// What to do, short enough for a tick list.
    pub fn label(self) -> &'static str {
        match self {
            Goal::TakePistol => "Take the pistol",
            Goal::TakeRadio => "Take the radio",
            Goal::Turn => "Turn around",
            Goal::Crouch => "Crouch",
            Goal::Run => "Run",
            Goal::PutAway => "Holster or pocket it",
            Goal::Pack => "Something in the pack",
            Goal::FarGrab => "Get the box in the pen",
            Goal::FillMag => "Rounds into the mag",
            Goal::SeatMag => "Mag into the pistol",
            Goal::Rack => "Rack the slide",
            Goal::Safety => "Safety off",
            Goal::HitPlate => "Hit a plate",
            Goal::DropMag => "Drop the mag",
            Goal::OpenDoor => "Open the door",
            Goal::Search => "Take what's inside",
            Goal::Wrist => "Read your wrist",
            Goal::Dressing => "Use a dressing",
            Goal::Probe => "Throw a part in first",
            Goal::Artifact => "Take the stone",
            Goal::Button => "Push the button",
            Goal::CallTruck => "Call the truck",
            Goal::PullCord => "Pull the cord",
        }
    }

    /// How, for how you play (`Comfort`): the sticks swap sides with GUN
    /// HAND left, and with one hand what needed two is done another way.
    pub fn how_for(self, flat: bool, comfort: &crate::rig::comfort::Comfort) -> String {
        if !flat && comfort.one_handed {
            let one = match self {
                Goal::FillMag => Some("Mag in hand. Touch it to\nthe box on the table."),
                Goal::SeatMag => Some("Mag in a pouch. Bring the\npistol in hand down on it."),
                Goal::Rack => Some("Press B or Y on the gun\nhand: it racks."),
                Goal::Dressing => Some("Hold it. Press A or X\non that hand."),
                _ => None,
            };
            if let Some(t) = one {
                return t.into();
            }
        }
        let t = self.how(flat);
        if !flat && comfort.left_handed {
            t.replace("right stick", "\u{0}")
                .replace("left stick", "right stick")
                .replace('\u{0}', "left stick")
        } else {
            t.into()
        }
    }

    /// How, with the control to use: two short lines at most.
    pub fn how(self, flat: bool) -> &'static str {
        match (self, flat) {
            (Goal::TakePistol | Goal::TakeRadio, false) => "Reach into the crate.\nSqueeze GRIP.",
            (Goal::TakePistol | Goal::TakeRadio, true) => "Look at it in the crate.\nPress E.",
            (Goal::Turn, false) => "Push the right stick left\nor right.",
            (Goal::Turn, true) => "Move the mouse round.",
            (Goal::Crouch, false) => "Squat down for real (the\nstick only slows you).",
            (Goal::Crouch, true) => "Press C (again to stand).",
            (Goal::Run, false) => "Click the left stick and\npush it forward.",
            (Goal::Run, true) => "Hold Shift and W.",
            (Goal::PutAway, false) => "Hold it at your hip or\nchest. Open your hand.",
            (Goal::PutAway, true) => "Hold it. Press G.",
            (Goal::Pack, false) => "Hold it over your shoulder.\nOpen your hand.",
            (Goal::Pack, true) => "Hold something big (the\ncleaning kit). Press G.",
            (Goal::FarGrab, false) => "Point at it. Squeeze GRIP.\nFlick your hand back.",
            (Goal::FarGrab, true) => "From the fence: look at\nit. Press E.",
            (Goal::FillMag, false) => "Mag in one hand, box in\nthe other. Touch them.",
            (Goal::FillMag, true) => "Mag in one hand, box in\nthe other. Press T.",
            (Goal::SeatMag, false) => "Pistol in one hand. Push\nthe mag up into the grip.",
            (Goal::SeatMag, true) => "Pistol in hand. Press R.",
            (Goal::Rack, false) => "Other hand on the slide:\nTRIGGER, pull back, let go.",
            (Goal::Rack, true) => "Pistol in hand. Press V.",
            (Goal::Safety, false) => "Click the stick on the\npistol's hand: safe, fire.",
            (Goal::Safety, true) => "Pistol in hand. Press X.",
            (Goal::HitPlate, false) => "Line up the sights.\nSqueeze TRIGGER.",
            (Goal::HitPlate, true) => "Right mouse aims.\nLeft mouse fires.",
            (Goal::DropMag, false) => "Press A on the gun hand.",
            (Goal::DropMag, true) => "Pistol in hand. Press B.",
            (Goal::OpenDoor, false) => "GRIP the door by its edge\nand pull it round.",
            (Goal::OpenDoor, true) => "Look at the door. Press E.",
            (Goal::Search, false) => "On the crate inside.\nSqueeze GRIP.",
            (Goal::Search, true) => "On the crate inside.\nLook at it. Press E.",
            (Goal::Wrist, false) => "Raise your watch hand and\nlook at the back of it.",
            (Goal::Wrist, true) => "Hold H to check yourself.",
            (Goal::Dressing, false) => "In one hand, wind it\nround the other forearm.",
            (Goal::Dressing, true) => "Hold it. Press U.",
            (Goal::Probe, false) => "GRIP a part, swing, open\nyour hand at the shimmer.",
            (Goal::Probe, true) => "Hold a part. Hold Q, let\ngo: it's thrown.",
            (Goal::Artifact, false) => "Don't step in. Point at\nit, GRIP, flick back.",
            (Goal::Artifact, true) => "From the edge: look at\nit. Press E.",
            (Goal::Button, false) => "Point a finger, TRIGGER\noff. Push the amber cap.",
            (Goal::Button, true) => "Look at the card. Press E.",
            (Goal::CallTruck, false) => "Radio in hand.\nPress A or X.",
            (Goal::CallTruck, true) => "Radio in hand. Press U.",
            (Goal::PullCord, false) => "Climb in the back.\nGRIP the cord by the cab.",
            (Goal::PullCord, true) => "Climb in. Look at the\ncord by the cab. Press E.",
        }
    }
}

/// A station: a post by the track (the first hangs in the truck).
pub struct Station {
    pub title: &'static str,
    /// Where its post stands, map metres (unused for the truck's).
    pub at: [f32; 2],
    /// Which way its board faces, degrees, as map markers (0 faces +Z,
    /// 90 faces -X, 270 faces +X).
    pub rot: f32,
    pub goals: &'static [Goal],
    /// Where it is, for the driver sending you on to it.
    pub where_: &'static str,
    /// What's worth knowing, shown once it's done (you have time to read
    /// then): what the old yard's signs said.
    pub tip: &'static str,
}

/// The stations, in order, from the truck north up the track.
pub const STATIONS: &[Station] = &[
    Station {
        title: "YOUR KIT",
        at: [0.0, 0.0],
        rot: 0.0,
        goals: &[Goal::TakePistol, Goal::TakeRadio],
        where_: "the crate in the truck",
        tip: "Everything you own rides\nin that crate between raids.",
    },
    Station {
        title: "GETTING ABOUT",
        at: [-3.5, 89.0],
        rot: 0.0,
        goals: &[Goal::Turn, Goal::Crouch, Goal::Run],
        where_: "the lamp off the back of the truck",
        tip: "Running tires you; weight\nslows you. Crouched, the\nmachines see you later.",
    },
    Station {
        title: "PUT IT AWAY",
        at: [3.5, 87.0],
        rot: 0.0,
        goals: &[Goal::PutAway, Goal::Pack],
        where_: "the next lamp, right of the track",
        tip: "Weight slows you.\nCarry what you'll use.",
    },
    Station {
        title: "OUT OF REACH",
        at: [-3.0, 83.5],
        rot: 0.0,
        goals: &[Goal::FarGrab],
        where_: "the lamp by the pen",
        tip: "Works on anything loose\nup to five metres off.",
    },
    Station {
        title: "THE RANGE",
        at: [-6.0, 47.5],
        rot: 315.0,
        goals: &[
            Goal::FillMag,
            Goal::SeatMag,
            Goal::Rack,
            Goal::Safety,
            Goal::HitPlate,
            Goal::DropMag,
        ],
        where_: "the range, left past the gate",
        tip: "Empty, the slide stays\nback: a new mag, then B\n(flat: V) lets it go.",
    },
    Station {
        title: "THE HOUSE",
        at: [7.0, 27.0],
        rot: 90.0,
        goals: &[Goal::OpenDoor, Goal::Search],
        where_: "the house on the right",
        tip: "Scrap and parts go to the\nbench in the truck.",
    },
    Station {
        title: "FIRST AID",
        at: [4.5, 8.0],
        rot: 90.0,
        goals: &[Goal::Wrist, Goal::Dressing],
        where_: "the lamp on the right",
        tip: "Eat when you're slow. Go\ndown and the collector\ncomes for you and your kit.",
    },
    Station {
        title: "THE SHIMMER",
        at: [4.5, -30.0],
        rot: 90.0,
        goals: &[Goal::Probe, Goal::Artifact],
        where_: "the lamp on the right. Mind the shimmer",
        tip: "Burners show; snares and\nlifts hide. Many stones\nand the machines know.",
    },
    Station {
        title: "BUTTONS",
        at: [-3.5, -50.0],
        rot: 270.0,
        goals: &[Goal::Button],
        where_: "the lamp on the left",
        tip: "Any card with a button\ndoes something: settings,\nthe route, terminals.",
    },
    Station {
        title: "GOING HOME",
        at: [4.0, -64.0],
        rot: 0.0,
        goals: &[Goal::CallTruck, Goal::PullCord],
        where_: "the north post, end of the track",
        tip: "Truck: 2 min to come, waits\n5. No more. My card in the\ntruck has your first job.",
    },
];

/// Every goal, in order.
pub fn all_goals() -> impl Iterator<Item = Goal> {
    STATIONS.iter().flat_map(|s| s.goals.iter().copied())
}

/// The station a goal belongs to.
pub fn station_of(goal: Goal) -> usize {
    STATIONS
        .iter()
        .position(|s| s.goals.contains(&goal))
        .expect("every goal has a station")
}

/// Where the training burner sits (its heart: the stone), and its size.
pub const SHIMMER_AT: [f32; 2] = [12.0, -36.0];
pub const SHIMMER_R: f32 = 1.6;
pub const STUMP_H: f32 = 0.38;
/// A beam at head height between two stacks of crates on the track, on
/// the way to the gate: you can only get under it ducking (the body's
/// height follows the head's), and you aren't told to.
pub const BEAM_AT: [f32; 2] = [0.0, 79.0];
/// The beam's underside, and the gap between the stacks.
pub const BEAM_CLEAR: f32 = 1.35;
pub const BEAM_GAP: f32 = 2.2;
const BEAM_STACK: Vec3 = Vec3::new(0.8, BEAM_CLEAR, 0.8);
/// A snare on the way in, nothing in it: you can't see where it ends until
/// something sets it off. Throw a thing in first.
pub const SNARE_AT: [f32; 2] = [9.0, -31.5];
pub const SNARE_R: f32 = 1.8;
/// The crate of spare parts to throw, by the shimmer's post.
pub const PARTS_CRATE_AT: [f32; 2] = [4.6, -28.4];
/// The pen the far-grab box sits in.
pub const FAR_STAND_AT: [f32; 2] = [-6.5, 79.5];
/// A crate at chest height in the middle of a fenced pen: plain to see,
/// no way to walk up to it. (On top of a tall stack it was out of reach
/// all right, and out of sight from below too.)
pub const PEN_HALF: f32 = 1.8;
pub const PEN_FENCE: f32 = 1.1;
pub const FAR_CRATE: Vec3 = Vec3::new(0.55, 0.95, 0.55);
/// The range's table: centre, size; the shooter stands on its south side.
pub const RANGE_TABLE_AT: [f32; 2] = [-13.5, 43.5];
pub const TABLE: Vec3 = Vec3::new(1.4, 0.9, 0.6);
/// A crate at the last station with a spare radio on it.
pub const HOME_CRATE_AT: [f32; 2] = [5.5, -63.0];

/// Where you are in it.
#[derive(Resource, Debug, Clone, Default)]
pub struct Onboarding {
    /// Goals done (skipped ones too).
    pub done: Vec<Goal>,
    /// Goals skipped with a station's SKIP card.
    pub skipped: Vec<Goal>,
    /// Seconds the yard has been open (for the opening line).
    pub age: f32,
    /// The opening line has been said.
    pub greeted: bool,
    /// Ducked under the low beam on the track (noticed, not asked for).
    pub ducked: bool,
}

impl Onboarding {
    pub fn is_done(&self, g: Goal) -> bool {
        self.done.contains(&g)
    }

    /// The first goal not done.
    pub fn next_goal(&self) -> Option<Goal> {
        all_goals().find(|g| !self.is_done(*g))
    }

    /// The station you're on: the first with something left to do.
    pub fn current(&self) -> Option<usize> {
        self.next_goal().map(station_of)
    }

    pub fn station_done(&self, i: usize) -> bool {
        STATIONS[i].goals.iter().all(|g| self.is_done(*g))
    }

    /// Everything but the cord: pulling it now ends the yard.
    pub fn ready_to_go(&self) -> bool {
        all_goals().all(|g| g == Goal::PullCord || self.is_done(g))
    }

    pub fn finished(&self) -> bool {
        self.next_goal().is_none()
    }
}

/// A goal just got done.
#[derive(Message, Clone, Copy, Debug)]
pub struct GoalDone {
    pub goal: Goal,
    /// Its station is now complete.
    pub station_done: bool,
}

/// A station's post (its lamp shows `lamp`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationPost {
    pub index: usize,
    pub lamp: Lamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lamp {
    /// Not there yet.
    Dark,
    /// This one next.
    Lit,
    Done,
}

/// A station's board (its words).
#[derive(Component, Debug, Clone, Copy)]
pub struct StationSign(pub usize);

/// The list of stations by the tailgate.
#[derive(Component, Debug, Clone, Copy)]
pub struct Checklist;

/// Cards the onboarding answers.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardCard {
    /// Skip station `n`.
    Skip(usize),
    /// The BUTTONS station's card.
    Push,
    /// Back to the first station.
    StartOver,
}

/// Props the client dresses (`visuals`).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum Prop {
    /// The fenced pen the far-grab box sits in.
    FarStand,
    /// The range's table.
    RangeTable,
    /// A crate to put a thing on.
    Crate(Vec3),
    /// The board by the tailgate, on its post.
    BoardPost,
    /// A burnt stump the burner's stone sits on, out of the grass.
    Stump,
    /// Two stacks of crates across the track with a beam over the gap.
    LowBeam,
}

/// Things put out for a goal.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placed {
    FarBox,
    HouseFind,
    Stone,
}

/// The practice house's doorway (its middle, on the floor), once found.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Doorway(pub Vec3);

/// The training anomalies.
#[derive(Component)]
pub struct Shimmer;

/// The one you find with a thrown part.
#[derive(Component)]
pub struct Probed;

/// Board sizes: a station's (read from a few steps off), the truck's (from
/// where you stand in the bed), the list by the tailgate.
pub const SIGN: Vec2 = crate::world::notes::SIGN;
pub const TRUCK_SIGN: Vec2 = Vec2::new(0.9, 0.62);
pub const CHECKLIST: Vec2 = Vec2::new(0.8, 1.0);
/// A post's board centre, and its lamp's height.
pub const BOARD_Y: f32 = 1.55;
pub const LAMP_Y: f32 = 2.32;
pub const SKIP_CARD: Vec2 = Vec2::new(0.22, 0.09);
pub const PUSH_CARD: Vec2 = Vec2::new(0.36, 0.15);

pub struct OnboardingPlugin;

impl Plugin for OnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Onboarding>()
            .add_message::<GoalDone>()
            .add_plugins((hints::HintsPlugin, firsts::FirstsPlugin))
            .add_systems(Update, second_hour::direct)
            .add_systems(
                PostStartup,
                (setup, bot_in_the_bed, dev_start_at).run_if(in_yard),
            )
            .add_systems(
                Update,
                (
                    (
                        watch_hands,
                        watch_guns,
                        watch_uses.before(crate::items::usage::UsageSystems),
                        watch_world,
                        watch_moves,
                        watch_wrist,
                        watch_probe,
                        watch_duck,
                        press_cards,
                    ),
                    announce.after(crate::items::usage::UsageSystems),
                    (lamps, signs, checklist),
                    warn_near_anomalies,
                    hurry_the_truck,
                    out_of_the_truck.after(crate::raid::tick_raid),
                    bot::direct,
                )
                    .chain()
                    .run_if(in_yard),
            );
    }
}

pub fn in_yard(opts: Res<LaunchOptions>) -> bool {
    opts.map == TRAINING_YARD
}

fn ground(zone: &Zone, at: [f32; 2]) -> Vec3 {
    Vec3::new(at[0], zone.ground(at[0], at[1]), at[1])
}

fn facing(rot: f32) -> Quat {
    Quat::from_rotation_y(-rot.to_radians())
}

/// A pressable card with words.
fn card(c: OnboardCard, text: &str, size: Vec2) -> impl Bundle {
    (
        Name::new("Training card"),
        crate::render::legibility::TextSource::Card,
        ScreenText {
            text: text.into(),
            size,
            style: ScreenStyle::Card,
        },
        c,
        Pressable::Camp,
        Grabbable {
            points: vec![GrabPoint::free()],
        },
        Collider::cuboid(size.x, size.y, 0.03),
        CollisionLayers::new(Layer::Item, LayerMask::NONE),
        Visibility::default(),
    )
}

fn paper(text: String, size: Vec2) -> ScreenText {
    ScreenText {
        text,
        size,
        style: ScreenStyle::Paper,
    }
}

/// `EXTRACTION_YARD_AT=N` (a dev aid, for screenshots of station N lit):
/// the stations before N start done.
fn dev_start_at(mut ob: ResMut<Onboarding>) {
    let Some(n) = std::env::var("EXTRACTION_YARD_AT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    else {
        return;
    };
    ob.done = STATIONS
        .iter()
        .take(n.saturating_sub(1))
        .flat_map(|s| s.goals.iter().copied())
        .collect();
    ob.greeted = true;
}

/// Posts, boards, the range table, the far stand, the house's find, the
/// training burner and its stone.
fn setup(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<Catalog>,
    trucks: Query<Entity, With<Truck>>,
    anomalies: Query<(Entity, &GlobalTransform, &Transform), With<Anomaly>>,
    items: Query<(Entity, &Item, &Transform)>,
    doors: Query<(&crate::world::buildings::DoorLeaf, &GlobalTransform)>,
    buildings: Query<(&Building, &Transform)>,
) {
    let truck = trucks.iter().next();
    let base = base_pose(&zone);
    for (i, st) in STATIONS.iter().enumerate() {
        let (post, board_at) = if i == 0 {
            // Hung from the canopy over the crate, tipped toward where you
            // stand in the bed.
            let Some(truck) = truck else { continue };
            let post = commands
                .spawn((
                    Name::new("Station 1"),
                    StationPost {
                        index: i,
                        lamp: Lamp::Dark,
                    },
                    Transform::from_xyz(-0.36, FLOOR + 1.5, -1.05)
                        .with_rotation(Quat::from_rotation_x(-0.32)),
                    Visibility::default(),
                    ChildOf(truck),
                ))
                .id();
            (post, Transform::from_xyz(0.0, 0.0, 0.02))
        } else {
            let post = commands
                .spawn((
                    Name::new(format!("Station {}", i + 1)),
                    StationPost {
                        index: i,
                        lamp: Lamp::Dark,
                    },
                    Transform::from_translation(ground(&zone, st.at)).with_rotation(facing(st.rot)),
                    Visibility::default(),
                    RigidBody::Static,
                    Collider::compound(vec![(
                        Vec3::new(0.0, 1.2, -0.05),
                        Quat::IDENTITY,
                        Collider::cuboid(0.1, 2.4, 0.1),
                    )]),
                    CollisionLayers::new(Layer::World, LayerMask::ALL),
                ))
                .id();
            // SKIP, low on the post's right, under the board.
            commands.spawn((
                card(OnboardCard::Skip(i), "SKIP", SKIP_CARD),
                Transform::from_xyz(0.27, BOARD_Y - SIGN.y * 0.5 - 0.14, 0.03),
                ChildOf(post),
            ));
            (post, Transform::from_xyz(0.0, BOARD_Y, 0.04))
        };
        let size = if i == 0 { TRUCK_SIGN } else { SIGN };
        commands.spawn((
            Name::new("Station board"),
            StationSign(i),
            if i == 0 {
                crate::render::legibility::TextSource::TruckBoard
            } else {
                crate::render::legibility::TextSource::StationBoard
            },
            paper(String::new(), size),
            board_at,
            Visibility::default(),
            ChildOf(post),
        ));
        if STATIONS[i].goals.contains(&Goal::Button) {
            commands.spawn((
                card(OnboardCard::Push, "PUSH ME", PUSH_CARD),
                Transform::from_xyz(-0.27, BOARD_Y - SIGN.y * 0.5 - 0.17, 0.03),
                ChildOf(post),
            ));
        }
    }

    // The list by the tailgate, facing the steps you come down; START OVER
    // and ON TO THE ZONE under it.
    let at = base.transform_point(Vec3::new(1.9, 0.0, 3.4));
    let post = commands
        .spawn((
            Name::new("Training board"),
            Prop::BoardPost,
            Transform::from_translation(Vec3::new(at.x, zone.ground(at.x, at.z), at.z))
                .with_rotation(base.rotation * Quat::from_rotation_y(std::f32::consts::PI)),
            Visibility::default(),
            RigidBody::Static,
            Collider::compound(vec![(
                Vec3::new(0.0, 1.0, -0.05),
                Quat::IDENTITY,
                Collider::cuboid(0.1, 2.0, 0.1),
            )]),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ))
        .id();
    commands.spawn((
        Name::new("Training list"),
        Checklist,
        crate::render::legibility::TextSource::TrainingList,
        paper(String::new(), CHECKLIST),
        Transform::from_xyz(0.0, 1.45, 0.04),
        Visibility::default(),
        ChildOf(post),
    ));
    commands.spawn((
        card(OnboardCard::StartOver, "START OVER", Vec2::new(0.3, 0.1)),
        Transform::from_xyz(-0.2, 0.8, 0.03),
        ChildOf(post),
    ));
    let mut skip = crate::camp::skip_card();
    skip.0.size = Vec2::new(0.3, 0.14);
    commands.spawn((
        skip,
        crate::render::legibility::TextSource::Card,
        Transform::from_xyz(0.2, 0.62, 0.03),
        ChildOf(post),
    ));

    let spawn = |commands: &mut Commands, id: &str, at: Transform| -> Option<Entity> {
        let kind = catalog.kind(id)?;
        Some(spawn_item(
            commands,
            &catalog,
            SpawnItem {
                kind,
                transform: at,
                condition: 0.9,
            },
        ))
    };
    let set_rounds = |commands: &mut Commands, e: Option<Entity>, n: u32| {
        if let Some(e) = e {
            commands.entity(e).queue(move |mut w: EntityWorldMut| {
                if let Some(mut it) = w.get_mut::<Item>() {
                    it.rounds = n;
                }
            });
        }
    };

    // Out of reach: a box of rounds (the range wants it) on a crate in a
    // fenced pen.
    let stand = ground(&zone, FAR_STAND_AT);
    let h = PEN_HALF;
    let rail =
        |at: Vec3, size: Vec3| (at, Quat::IDENTITY, Collider::cuboid(size.x, size.y, size.z));
    commands.spawn((
        Name::new("Far pen"),
        Prop::FarStand,
        Transform::from_translation(stand),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![
            rail(
                Vec3::new(0.0, PEN_FENCE * 0.5, h),
                Vec3::new(2.0 * h, PEN_FENCE, 0.08),
            ),
            rail(
                Vec3::new(0.0, PEN_FENCE * 0.5, -h),
                Vec3::new(2.0 * h, PEN_FENCE, 0.08),
            ),
            rail(
                Vec3::new(h, PEN_FENCE * 0.5, 0.0),
                Vec3::new(0.08, PEN_FENCE, 2.0 * h),
            ),
            rail(
                Vec3::new(-h, PEN_FENCE * 0.5, 0.0),
                Vec3::new(0.08, PEN_FENCE, 2.0 * h),
            ),
            rail(Vec3::Y * FAR_CRATE.y * 0.5, FAR_CRATE),
        ]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    let far = spawn(
        &mut commands,
        "ammo_9x18",
        Transform::from_translation(stand + Vec3::Y * (FAR_CRATE.y + 0.04)),
    );
    if let Some(e) = far {
        commands.entity(e).insert(Placed::FarBox);
    }
    set_rounds(&mut commands, far, 16);

    // The range: a pistol, an empty magazine and a box of rounds on a table.
    let table = ground(&zone, RANGE_TABLE_AT);
    commands.spawn((
        Name::new("Range table"),
        Prop::RangeTable,
        Transform::from_translation(table),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![(
            Vec3::Y * (TABLE.y - 0.02),
            Quat::IDENTITY,
            Collider::cuboid(TABLE.x, 0.04, TABLE.z),
        )]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    let top = table + Vec3::Y * (TABLE.y + 0.05);
    let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    spawn(
        &mut commands,
        "pm_pistol",
        Transform::from_translation(top + Vec3::new(-0.35, 0.0, 0.0)).with_rotation(rot),
    );
    let mag = spawn(
        &mut commands,
        "pm_mag",
        Transform::from_translation(top + Vec3::new(0.05, 0.0, 0.05)),
    );
    set_rounds(&mut commands, mag, 0);
    let ammo = spawn(
        &mut commands,
        "ammo_9x18",
        Transform::from_translation(top + Vec3::new(0.4, 0.0, 0.0)),
    );
    set_rounds(&mut commands, ammo, 16);

    // The house: a crate just inside its door, a dressing on it.
    let house = buildings
        .iter()
        .filter(|(b, _)| b.prefab.starts_with("house"))
        .map(|(_, t)| t.translation)
        .next();
    if let Some(house) = house {
        let door = doors
            .iter()
            .min_by(|a, b| {
                a.1.translation()
                    .distance(house)
                    .total_cmp(&b.1.translation().distance(house))
            })
            .map(|(leaf, gt)| (leaf.0.width, *gt));
        if let Some((width, gt)) = door {
            let mid = gt.transform_point(Vec3::new(width * 0.5, 0.0, 0.0));
            let inward = house - mid;
            let inward = Vec3::new(inward.x, 0.0, inward.z).normalize_or(Vec3::X);
            // Beside the doorway on the latch side, clear of the leaf as it
            // swings in.
            let latch = gt.rotation() * Vec3::X;
            let latch = Vec3::new(latch.x, 0.0, latch.z).normalize_or(Vec3::Z);
            let at = mid + inward * 0.9 + latch * (width * 0.5 + 0.35);
            commands.insert_resource(Doorway(mid));
            let size = Vec3::new(0.5, 0.55, 0.45);
            let floor = gt.translation().y;
            commands.spawn((
                Name::new("House crate"),
                Prop::Crate(size),
                Transform::from_xyz(at.x, floor, at.z),
                Visibility::default(),
                RigidBody::Static,
                Collider::compound(vec![(
                    Vec3::Y * size.y * 0.5,
                    Quat::IDENTITY,
                    Collider::cuboid(size.x, size.y, size.z),
                )]),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
            ));
            if let Some(e) = spawn(
                &mut commands,
                "field_dressing",
                Transform::from_xyz(at.x, floor + size.y + 0.05, at.z),
            ) {
                commands.entity(e).insert(Placed::HouseFind);
            }
        }
    }

    // The low beam on the track.
    let at = ground(&zone, BEAM_AT);
    let off = (BEAM_GAP + BEAM_STACK.x) * 0.5;
    commands.spawn((
        Name::new("Low beam"),
        Prop::LowBeam,
        Transform::from_translation(at),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![
            (
                Vec3::new(-off, BEAM_STACK.y * 0.5, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(BEAM_STACK.x, BEAM_STACK.y, BEAM_STACK.z),
            ),
            (
                Vec3::new(off, BEAM_STACK.y * 0.5, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(BEAM_STACK.x, BEAM_STACK.y, BEAM_STACK.z),
            ),
            (
                Vec3::Y * (BEAM_CLEAR + 0.08),
                Quat::IDENTITY,
                Collider::cuboid(BEAM_GAP + 1.5, 0.16, 0.16),
            ),
        ]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));

    // Going home: a spare radio on a crate, for whoever left theirs.
    let size = Vec3::new(0.6, 0.7, 0.5);
    let at = ground(&zone, HOME_CRATE_AT);
    commands.spawn((
        Name::new("Radio crate"),
        Prop::Crate(size),
        Transform::from_translation(at),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![(
            Vec3::Y * size.y * 0.5,
            Quat::IDENTITY,
            Collider::cuboid(size.x, size.y, size.z),
        )]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    spawn(
        &mut commands,
        "radio",
        Transform::from_translation(at + Vec3::Y * (size.y + 0.05)),
    );

    // The shimmer: one burner where you can see it from the track, its
    // stone at its heart. The Surge's own anomalies stay off the course
    // (one turned up on the track and looked like part of it).
    let heart = ground(&zone, SHIMMER_AT);
    let mut keep_clear: Vec<Vec2> = STATIONS.iter().skip(1).map(|s| Vec2::from(s.at)).collect();
    keep_clear.extend([
        Vec2::from(SHIMMER_AT),
        Vec2::from(SNARE_AT),
        Vec2::from(FAR_STAND_AT),
        Vec2::from(RANGE_TABLE_AT),
    ]);
    for (e, gt, _) in &anomalies {
        let p = gt.translation().xz();
        // The track runs north along x = 0.
        if p.x.abs() < 14.0 || keep_clear.iter().any(|c| c.distance(p) < 14.0) {
            commands.entity(e).despawn();
            for (ie, it, t) in &items {
                if catalog.get(it.kind()).category == Category::Artifact
                    && t.translation.xz().distance(p) < 1.0
                {
                    commands.entity(ie).despawn();
                }
            }
        }
    }
    commands.spawn((
        Name::new("Training snare"),
        Shimmer,
        Probed,
        Anomaly {
            kind: extraction_core::anomaly::AnomalyKind::Snare,
            radius: SNARE_R,
            charge: 1.0,
            bloom: None,
        },
        Transform::from_translation(ground(&zone, SNARE_AT)),
        Visibility::default(),
    ));
    let size = Vec3::new(0.5, 0.75, 0.45);
    let at = ground(&zone, PARTS_CRATE_AT);
    commands.spawn((
        Name::new("Parts crate"),
        Prop::Crate(size),
        Transform::from_translation(at),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![(
            Vec3::Y * size.y * 0.5,
            Quat::IDENTITY,
            Collider::cuboid(size.x, size.y, size.z),
        )]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    for k in 0..3 {
        spawn(
            &mut commands,
            "spare_parts",
            Transform::from_translation(
                at + Vec3::new(-0.12 + k as f32 * 0.12, size.y + 0.04, 0.0),
            ),
        );
    }
    commands.spawn((
        Name::new("Training burner"),
        Shimmer,
        Anomaly {
            kind: extraction_core::anomaly::AnomalyKind::Burner,
            radius: SHIMMER_R,
            charge: 0.0,
            bloom: None,
        },
        Transform::from_translation(heart),
        Visibility::default(),
    ));
    // On a burnt stump: on the ground the grass hid it.
    commands.spawn((
        Name::new("Burnt stump"),
        Prop::Stump,
        Transform::from_translation(heart),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![(
            Vec3::Y * STUMP_H * 0.5,
            Quat::IDENTITY,
            Collider::cylinder(0.17, STUMP_H),
        )]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    if let Some(e) = spawn(
        &mut commands,
        "art_hearth",
        Transform::from_translation(heart + Vec3::Y * (STUMP_H + 0.06)),
    ) {
        commands.entity(e).insert(Placed::Stone);
    }
}

/// A bot running the course starts where a person does: in the bed.
fn bot_in_the_bed(
    zone: Res<Zone>,
    opts: Res<LaunchOptions>,
    mut players: Query<&mut Transform, With<LocalPlayer>>,
) {
    if opts.scenario.as_deref() != Some(bot::SCENARIO) {
        return;
    }
    let base = base_pose(&zone);
    for mut t in &mut players {
        t.translation = base.transform_point(Vec3::new(0.3, FLOOR + 0.05, 0.6));
        t.rotation = base.rotation;
    }
}

/// Marks a goal done (once), and says so.
fn complete(ob: &mut Onboarding, goal: Goal, out: &mut MessageWriter<GoalDone>) {
    if ob.is_done(goal) {
        return;
    }
    ob.done.push(goal);
    let station_done = ob.station_done(station_of(goal));
    info!("onboarding: {goal:?} done");
    out.write(GoalDone { goal, station_done });
}

/// Grabs, holsters, the pack, the far grab, the house, the stone.
fn watch_hands(
    mut events: MessageReader<InteractEvent>,
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    sockets: Query<(&Socket, Option<&crate::interact::SocketOwner>)>,
    bags: Query<(Entity, &Backpack, Has<Stash>)>,
    items: Query<(&Item, Option<&Placed>)>,
    held: Query<(Entity, &Held, Option<&Placed>)>,
    locals: Query<(), With<LocalPlayer>>,
    catalog: Res<Catalog>,
    mut in_crate: Local<Vec<Entity>>,
) {
    let id = |e: Entity| {
        items
            .get(e)
            .ok()
            .map(|(it, _)| catalog.get(it.kind()).id.as_str())
    };
    for ev in events.read() {
        match ev {
            InteractEvent::Grabbed { item, .. } => {
                if in_crate.contains(item) {
                    match id(*item) {
                        Some("pm_pistol") => complete(&mut ob, Goal::TakePistol, &mut out),
                        Some("radio") => complete(&mut ob, Goal::TakeRadio, &mut out),
                        _ => {}
                    }
                }
            }
            InteractEvent::Socketed { socket, .. } => {
                if let Ok((s, owner)) = sockets.get(*socket) {
                    if matches!(s.kind, SocketKind::Body(_))
                        && owner.is_some_and(|o| locals.contains(o.0))
                    {
                        complete(&mut ob, Goal::PutAway, &mut out);
                    }
                }
            }
            InteractEvent::Stowed { item } => {
                let packed = bags.iter().any(|(_, b, stash)| {
                    !stash && b.carried && locals.contains(b.owner) && b.contents.contains(item)
                });
                if packed {
                    complete(&mut ob, Goal::Pack, &mut out);
                }
            }
            _ => {}
        }
    }
    // What's in the crate now, for the next frame's grabs (a grab takes it
    // out of the crate before the event is read).
    in_crate.clear();
    for (_, b, stash) in &bags {
        if stash {
            in_crate.extend(b.contents.iter().copied());
        }
    }
    // In a local hand: the far box, the house's find, the stone.
    for (e, h, placed) in &held {
        if !locals.contains(h.player) {
            continue;
        }
        match placed {
            Some(Placed::FarBox) => complete(&mut ob, Goal::FarGrab, &mut out),
            Some(Placed::HouseFind) => complete(&mut ob, Goal::Search, &mut out),
            _ => {}
        }
        if items
            .get(e)
            .is_ok_and(|(it, _)| catalog.get(it.kind()).category == Category::Artifact)
        {
            complete(&mut ob, Goal::Artifact, &mut out);
        }
    }
}

/// Rounds into a magazine, the magazine in, the slide racked, a plate hit.
fn watch_guns(
    mut gun_events: MessageReader<GunEvent>,
    mut hits: MessageReader<Hit>,
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    held: Query<&Held>,
    mags: Query<(Entity, &Item, &Held), With<crate::weapons::Mag>>,
    guns: Query<(), With<Gun>>,
    buildings: Query<&Building>,
    locals: Query<(), With<LocalPlayer>>,
    mut rounds: Local<std::collections::HashMap<Entity, u32>>,
) {
    let local_gun =
        |g: Entity| guns.contains(g) && held.get(g).is_ok_and(|h| locals.contains(h.player));
    for ev in gun_events.read() {
        if !local_gun(ev.gun) {
            continue;
        }
        match ev.event {
            extraction_core::firearm::Event::MagazineInserted => {
                complete(&mut ob, Goal::SeatMag, &mut out)
            }
            extraction_core::firearm::Event::Chambered => complete(&mut ob, Goal::Rack, &mut out),
            extraction_core::firearm::Event::MagazineRemoved => {
                complete(&mut ob, Goal::DropMag, &mut out)
            }
            extraction_core::firearm::Event::ModeChanged(m)
                if m != extraction_core::firearm::FireMode::Safe =>
            {
                complete(&mut ob, Goal::Safety, &mut out)
            }
            _ => {}
        }
    }
    for hit in hits.read() {
        let mine = hit.shooter.is_some_and(|s| locals.contains(s));
        if mine
            && buildings
                .get(hit.entity)
                .is_ok_and(|b| b.prefab == "target")
        {
            complete(&mut ob, Goal::HitPlate, &mut out);
        }
    }
    // A magazine in a local hand that gained rounds.
    let mut now = std::collections::HashMap::new();
    for (e, it, h) in &mags {
        if !locals.contains(h.player) {
            continue;
        }
        if rounds.get(&e).is_some_and(|r| it.rounds > *r) {
            complete(&mut ob, Goal::FillMag, &mut out);
        }
        now.insert(e, it.rounds);
    }
    *rounds = now;
}

/// A field dressing used, the radio used to call the truck.
fn watch_uses(
    mut uses: MessageReader<crate::items::usage::UseItem>,
    mut calls: MessageReader<RadioCall>,
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    held: Query<(Entity, &Item), With<Held>>,
    catalog: Res<Catalog>,
    locals: Query<(), With<LocalPlayer>>,
    raid: Res<Raid>,
    mut dressings: Local<Vec<Entity>>,
) {
    // Held dressings, remembered: a used one is gone by the time a message
    // written after this ran comes round.
    for (e, it) in &held {
        if catalog.get(it.kind()).id == "field_dressing" && !dressings.contains(&e) {
            dressings.push(e);
        }
    }
    for u in uses.read() {
        if dressings.contains(&u.item) && locals.contains(u.player) {
            complete(&mut ob, Goal::Dressing, &mut out);
        }
    }
    for c in calls.read() {
        // A call only counts once you're out where the truck can come.
        if locals.contains(c.player) && raid.phase != Phase::AtBase {
            complete(&mut ob, Goal::CallTruck, &mut out);
        }
    }
}

/// Turning (the turning stick sideways in a headset: the right, or the
/// left with GUN HAND left; the view swung round in flat mode) and
/// crouching (the toggle, or squatting for real).
fn watch_moves(
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    players: Query<(&crate::rig::RigInput, &Transform), With<LocalPlayer>>,
    driver: Res<RigDriver>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut swung: Local<(Option<f32>, f32)>,
) {
    let Some((input, t)) = players.iter().next() else {
        return;
    };
    let yaw = t.rotation.to_euler(EulerRot::YXZ).0;
    if let Some(last) = swung.0 {
        let d = (yaw - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        swung.1 += d.abs();
    }
    swung.0 = Some(yaw);
    let stick = input
        .hand(comfort.as_ref().map_or(Side::Right, |c| c.gun_side()))
        .stick;
    let turned = if *driver == RigDriver::Flat {
        swung.1 > 1.5
    } else {
        stick.x.abs() > 0.7
    };
    if ob.current().is_some_and(|i| i >= station_of(Goal::Turn)) {
        if turned {
            complete(&mut ob, Goal::Turn, &mut out);
        }
        let stick_down = *driver != RigDriver::Flat && stick.y < -0.7;
        if input.crouch || stick_down || input.head.translation.y < 1.15 {
            complete(&mut ob, Goal::Crouch, &mut out);
        }
        if input.sprint && input.locomotion.y > 0.3 {
            complete(&mut ob, Goal::Run, &mut out);
        }
    } else {
        swung.1 = 0.0;
    }
}

/// Where the wrist's screen rides on the left hand (`render::comfort`);
/// on the right, mirrored.
pub const WRIST_PANEL: Vec3 = Vec3::new(-0.03, 0.015, 0.07);

/// Looking at your wrist (the head's gaze on the left hand's screen, close,
/// for a moment), or in flat mode holding H.
fn watch_wrist(
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    heads: Query<(&GlobalTransform, &ChildOf), With<crate::rig::RigHead>>,
    hands: Query<(&GlobalTransform, &crate::rig::RigHand, &ChildOf)>,
    locals: Query<(), With<LocalPlayer>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    time: Res<Time>,
    mut held: Local<f32>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
) {
    if ob.is_done(Goal::Wrist) {
        return;
    }
    // The watch is on the hand that isn't the gun hand.
    let watch = comfort
        .as_ref()
        .map_or(Side::Left, |c| c.gun_side().other());
    let panel_at = WRIST_PANEL * Vec3::new(-watch.sign(), 1.0, 1.0);
    let mut looking = keys.is_some_and(|k| k.pressed(KeyCode::KeyH));
    for (head, owner) in &heads {
        if !locals.contains(owner.parent()) {
            continue;
        }
        let hand = hands
            .iter()
            .find(|(_, rh, p)| rh.side == watch && p.parent() == owner.parent());
        let Some((hand, ..)) = hand else { continue };
        let panel = hand.transform_point(panel_at);
        let to = panel - head.translation();
        let forward = head.rotation() * Vec3::NEG_Z;
        if to.length() < 0.65 && forward.angle_between(to) < 20f32.to_radians() {
            looking = true;
        }
    }
    *held = if looking {
        *held + time.delta_secs()
    } else {
        0.0
    };
    if *held > 0.4 {
        complete(&mut ob, Goal::Wrist, &mut out);
    }
}

/// The snare on the way in went off with you outside it: something
/// thrown found it.
fn watch_probe(
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    mut events: MessageReader<crate::anomaly::AnomalyEvent>,
    snares: Query<(&Anomaly, &GlobalTransform), With<Probed>>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
) {
    for ev in events.read() {
        let Ok((a, gt)) = snares.get(ev.anomaly) else {
            continue;
        };
        let clear = players
            .iter()
            .all(|p| p.translation().xz().distance(gt.translation().xz()) > a.radius + 0.3);
        if ev.what == crate::anomaly::AnomalyAct::Discharge && clear {
            complete(&mut ob, Goal::Probe, &mut out);
        }
    }
}

/// Under the beam with your head below it: noticed, with a click and a
/// word from the driver. (Not a step: it's for whoever does it anyway.)
fn watch_duck(
    mut ob: ResMut<Onboarding>,
    players: Query<(&crate::rig::RigInput, &Transform), With<LocalPlayer>>,
    zone: Res<Zone>,
    mut raid: ResMut<Raid>,
    mut steps: MessageWriter<StepDone>,
) {
    if ob.ducked {
        return;
    }
    let Some((input, t)) = players.iter().next() else {
        return;
    };
    let head = t.transform_point(input.head.translation);
    let rel = head.xz() - Vec2::from(BEAM_AT);
    let under = rel.x.abs() < BEAM_GAP * 0.5 && rel.y.abs() < 0.5;
    let low = head.y - zone.ground(head.x, head.z) < BEAM_CLEAR - 0.05;
    if under && low {
        ob.ducked = true;
        raid.say(DUCK_LINE);
        steps.write(StepDone {
            at: None,
            station: false,
        });
    }
}

pub const DUCK_LINE: &str = "Driver: \"Mind your head. That's it.\"";

/// The house's door open.
fn watch_world(
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    doors: Query<(&Door, &GlobalTransform)>,
    buildings: Query<(&Building, &GlobalTransform)>,
) {
    if ob.is_done(Goal::OpenDoor) {
        return;
    }
    let Some(house) = buildings
        .iter()
        .find(|(b, _)| b.prefab.starts_with("house"))
        .map(|(_, gt)| gt.translation())
    else {
        return;
    };
    if doors
        .iter()
        .any(|(d, gt)| d.open && gt.translation().distance(house) < 8.0)
    {
        complete(&mut ob, Goal::OpenDoor, &mut out);
    }
}

/// SKIP, PUSH ME, START OVER, and the cord.
fn press_cards(
    mut events: MessageReader<InteractEvent>,
    cards: Query<&OnboardCard>,
    pressables: Query<&Pressable>,
    mut ob: ResMut<Onboarding>,
    mut out: MessageWriter<GoalDone>,
    mut raid: ResMut<Raid>,
) {
    for ev in events.read() {
        let InteractEvent::Pressed { item, .. } = ev else {
            continue;
        };
        if pressables.get(*item) == Ok(&Pressable::BellPull) && ob.ready_to_go() {
            complete(&mut ob, Goal::PullCord, &mut out);
        }
        match cards.get(*item) {
            Ok(OnboardCard::Push) => complete(&mut ob, Goal::Button, &mut out),
            Ok(OnboardCard::Skip(i)) => {
                for g in STATIONS[*i].goals {
                    if !ob.is_done(*g) {
                        ob.skipped.push(*g);
                        complete(&mut ob, *g, &mut out);
                    }
                }
            }
            Ok(OnboardCard::StartOver) => {
                ob.done.clear();
                ob.skipped.clear();
                raid.say(START_OVER_LINE);
            }
            _ => {}
        }
    }
}

/// What the driver says, each short enough for the wrist (three lines of
/// 26: it shows only the newest message, and cuts a long one off).
pub const GREET_VR: &str = "Driver: \"Kit's in the crate by your left knee. Reach in, squeeze.\"";
pub const GREET_FLAT: &str = "Driver: \"Kit's in the crate, front left. Look at it, press E.\"";
pub const ALL_DONE_LINE: &str = "Driver: \"That's the lot. The real one's colder.\"";
pub const TOO_CLOSE_LINE: &str = "Driver: \"Back out. That shimmer burns. Ask me how I know.\"";
pub const START_OVER_LINE: &str = "Driver: \"From the top. The crate in the truck.\"";

pub fn next_line(station: usize) -> String {
    format!("Driver: \"Good. Next: {}.\"", STATIONS[station].where_)
}

/// The driver on the radio (your wrist): the opening line, and after each
/// station, where the next one is. Each tick chimes and clicks both hands
/// ([`StepDone`]).
fn announce(
    mut done: MessageReader<GoalDone>,
    mut ob: ResMut<Onboarding>,
    mut raid: ResMut<Raid>,
    mut steps: MessageWriter<StepDone>,
    time: Res<Time>,
    driver: Res<RigDriver>,
    players: Query<(), With<LocalPlayer>>,
) {
    let flat = *driver == RigDriver::Flat;
    if players.is_empty() {
        return;
    }
    ob.age += time.delta_secs();
    if !ob.greeted && ob.age > 1.5 {
        ob.greeted = true;
        raid.say(if flat { GREET_FLAT } else { GREET_VR });
    }
    for d in done.read() {
        steps.write(StepDone {
            at: None,
            station: d.station_done,
        });
        if !d.station_done {
            continue;
        }
        let line = match ob.current() {
            Some(next) if next > station_of(d.goal) => next_line(next),
            Some(_) => continue,
            None => ALL_DONE_LINE.to_string(),
        };
        raid.say(line);
    }
}

fn lamps(ob: Res<Onboarding>, mut posts: Query<&mut StationPost>) {
    let current = ob.current();
    for mut p in &mut posts {
        let lamp = if ob.station_done(p.index) {
            Lamp::Done
        } else if current == Some(p.index) {
            Lamp::Lit
        } else {
            Lamp::Dark
        };
        if p.lamp != lamp {
            p.lamp = lamp;
        }
    }
}

/// A station board's words, for this progress and mode.
pub fn sign_text(
    ob: &Onboarding,
    i: usize,
    flat: bool,
    comfort: &crate::rig::comfort::Comfort,
) -> String {
    let st = &STATIONS[i];
    let mut t = format!("{}  {}\n\n", i + 1, st.title);
    let next = st.goals.iter().find(|g| !ob.is_done(**g));
    for g in st.goals {
        let mark = if ob.skipped.contains(g) {
            " --  "
        } else if ob.is_done(*g) {
            " OK  "
        } else if Some(g) == next {
            " >>  "
        } else {
            "     "
        };
        t += &format!("{mark}{}\n", g.label());
    }
    match next {
        Some(g) => {
            t += "\n";
            t += &g.how_for(flat, comfort);
        }
        None => {
            // Done: the steps are ticked off; what's worth knowing instead.
            t = format!("{}  {}\n\nDONE.\n\n{}", i + 1, st.title, st.tip);
            if i + 1 < STATIONS.len() {
                t += "\n\nNext: the lit lamp.";
            }
        }
    }
    t
}

/// The list by the tailgate.
pub fn checklist_text(ob: &Onboarding) -> String {
    let mut t = String::from("TRAINING YARD\nNothing here is kept.\n\n");
    let current = ob.current();
    for (i, st) in STATIONS.iter().enumerate() {
        let mark = if ob.station_done(i) {
            " OK "
        } else if current == Some(i) {
            " >> "
        } else {
            "    "
        };
        t += &format!("{mark}{} {}\n", i + 1, st.title);
    }
    t += "\nFollow the lit lamps.";
    t
}

fn signs(
    ob: Res<Onboarding>,
    driver: Res<RigDriver>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut signs: Query<(&StationSign, &mut ScreenText)>,
) {
    let flat = *driver == RigDriver::Flat;
    let comfort = comfort.map(|c| c.clone()).unwrap_or_default();
    for (s, mut screen) in &mut signs {
        let t = sign_text(&ob, s.0, flat, &comfort);
        if screen.text != t {
            screen.text = t;
        }
    }
}

fn checklist(ob: Res<Onboarding>, mut boards: Query<&mut ScreenText, With<Checklist>>) {
    let t = checklist_text(&ob);
    for mut b in &mut boards {
        if b.text != t {
            b.text = t.clone();
        }
    }
}

/// Close to the shimmer (or any anomaly in the yard): the driver warns
/// you, and your hands buzz.
fn warn_near_anomalies(
    anomalies: Query<(&Anomaly, &GlobalTransform)>,
    players: Query<&GlobalTransform, With<LocalPlayer>>,
    mut raid: ResMut<Raid>,
    mut haptics: MessageWriter<Haptic>,
    time: Res<Time>,
    mut quiet: Local<f32>,
) {
    *quiet -= time.delta_secs();
    let Some(p) = players.iter().next() else {
        return;
    };
    let p = p.translation();
    let near = anomalies.iter().any(|(a, gt)| {
        let d = gt.translation() - p;
        Vec2::new(d.x, d.z).length() < a.radius + 0.6 && d.y.abs() < 3.0
    });
    if near && *quiet <= 0.0 {
        *quiet = 5.0;
        raid.say(TOO_CLOSE_LINE);
        // A knock, not a kick: kicks are for shots and hits
        // (`rig::haptics`).
        for side in Side::BOTH {
            haptics.write(Haptic {
                side,
                amplitude: 0.55,
                seconds: 0.25,
            });
        }
    }
}

/// In the yard the truck comes in seconds, not minutes, and waits for you.
fn hurry_the_truck(mut raid: ResMut<Raid>) {
    match &mut raid.phase {
        Phase::TruckCalled { eta, .. } if *eta > HURRY => *eta = HURRY,
        Phase::TruckWaiting { left, .. } if *left < 60.0 => *left = crate::raid::TRUCK_WAIT,
        _ => {}
    }
}

/// Stepping out of the truck says when the Surge is due: nothing to do
/// with the yard, and it pushes the driver's directions off the wrist. Here
/// the driver says where to go instead.
fn out_of_the_truck(mut raid: ResMut<Raid>, ob: Res<Onboarding>) {
    let Some(i) = raid
        .log
        .iter()
        .position(|(t, _)| t.starts_with("Out of the truck. The Surge"))
    else {
        return;
    };
    raid.log.remove(i);
    if let Some(next) = ob.current() {
        raid.say(format!(
            "Driver: \"Off you go. Next: {}.\"",
            STATIONS[next].where_
        ));
    }
}

/// Seconds the yard's truck takes to come round.
pub const HURRY: f32 = 10.0;

pub mod bot;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::legibility::{TextSource, check_panel, panel_deg};

    // The first playtest's first complaint was signs it couldn't read, and
    // these are what someone new reads first: they hold to the must-read
    // floor (`render::legibility`).
    fn readable(screen: &ScreenText, source: TextSource) -> Result<(), String> {
        check_panel(screen, source).map(|_| ())
    }

    /// Standing two-handed right-handed, left-handed, one-handed.
    fn access_modes() -> [crate::rig::comfort::Comfort; 3] {
        use crate::rig::comfort::Comfort;
        [
            Comfort::default(),
            Comfort {
                left_handed: true,
                ..Default::default()
            },
            Comfort {
                one_handed: true,
                ..Default::default()
            },
        ]
    }

    /// The sticks named are the ones that do it: turning is on the left
    /// with GUN HAND left; one-handed, a magazine is filled and racked
    /// without a second hand.
    #[test]
    fn the_yard_says_how_for_how_you_play() {
        let [_, left, one] = access_modes();
        assert!(Goal::Turn.how_for(false, &left).contains("left stick"));
        assert!(Goal::Run.how_for(false, &left).contains("right stick"));
        assert!(!Goal::FillMag.how_for(false, &one).contains("other"));
        assert!(Goal::Rack.how_for(false, &one).contains("Press B"));
        // Flat play has no hands to count.
        assert_eq!(Goal::Rack.how_for(true, &one), Goal::Rack.how(true));
    }

    /// Every board's words, at every stage, in both modes, read from where
    /// you'd stand (the readability test sees only what's up at the start).
    #[test]
    fn every_board_reads_at_every_stage() {
        let mut bad = Vec::new();
        let mut smallest = f32::MAX;
        let goals: Vec<Goal> = all_goals().collect();
        for k in 0..=goals.len() {
            for skipped in [false, true] {
                let mut ob = Onboarding::default();
                ob.done = goals[..k].to_vec();
                if skipped {
                    ob.skipped = ob.done.clone();
                }
                for (flat, comfort) in [false, true]
                    .into_iter()
                    .flat_map(|f| access_modes().into_iter().map(move |c| (f, c)))
                {
                    for i in 0..STATIONS.len() {
                        let size = if i == 0 { TRUCK_SIGN } else { SIGN };
                        let s = paper(sign_text(&ob, i, flat, &comfort), size);
                        let source = if i == 0 {
                            TextSource::TruckBoard
                        } else {
                            TextSource::StationBoard
                        };
                        smallest = smallest.min(panel_deg(&s, source));
                        if let Err(e) = readable(&s, source) {
                            bad.push(e);
                        }
                    }
                }
                if let Err(e) = readable(
                    &paper(checklist_text(&ob), CHECKLIST),
                    TextSource::TrainingList,
                ) {
                    bad.push(e);
                }
            }
        }
        for (text, size) in [
            ("SKIP", SKIP_CARD),
            ("PUSH ME", PUSH_CARD),
            ("START OVER", Vec2::new(0.3, 0.1)),
        ] {
            let s = ScreenText {
                text: text.into(),
                size,
                style: ScreenStyle::Card,
            };
            if let Err(e) = readable(&s, TextSource::Card) {
                bad.push(e);
            }
        }
        bad.dedup();
        assert!(bad.is_empty(), "too small:\n{}", bad.join("\n"));
        eprintln!("smallest letters: {smallest:.2} deg");
    }

    /// Everything the driver says fits the wrist whole.
    #[test]
    fn the_driver_fits_on_the_wrist() {
        let mut lines: Vec<String> = [
            GREET_VR,
            GREET_FLAT,
            ALL_DONE_LINE,
            TOO_CLOSE_LINE,
            START_OVER_LINE,
            DUCK_LINE,
            crate::camp::YARD_CORD_LINE,
        ]
        .map(String::from)
        .to_vec();
        lines.extend((1..STATIONS.len()).map(next_line));
        lines.extend(
            STATIONS
                .iter()
                .map(|s| format!("Driver: \"Off you go. Next: {}.\"", s.where_)),
        );
        for l in lines {
            let wrapped = crate::render::screen::wrap(&l, 26, usize::MAX);
            assert!(wrapped.len() <= 3, "{} lines: {l}", wrapped.len());
        }
    }

    /// Every station asks for something, and every instruction is short:
    /// a couple of lines, with the control named.
    #[test]
    fn instructions_are_short_and_name_the_control() {
        for st in STATIONS {
            assert!(st.tip.lines().count() <= 3, "{}", st.tip);
            assert!(st.tip.lines().all(|l| l.len() <= 28), "{}", st.tip);
        }
        for g in all_goals() {
            for (flat, comfort) in [false, true]
                .into_iter()
                .flat_map(|f| access_modes().into_iter().map(move |c| (f, c)))
            {
                let how = g.how_for(flat, &comfort);
                assert!(how.lines().count() <= 2, "{g:?}: {how}");
                assert!(how.lines().all(|l| l.len() <= 28), "{g:?}: {how}");
                let control = if flat {
                    ["Press", "mouse", "Hold"].iter().any(|k| how.contains(k))
                } else {
                    ["GRIP", "TRIGGER", "Press A", "hand", "finger", "stick"]
                        .iter()
                        .any(|k| how.contains(k))
                };
                assert!(control, "{g:?} names no control: {how}");
            }
            assert!(g.label().len() <= 22, "{g:?}");
        }
    }
}
