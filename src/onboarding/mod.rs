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
use crate::rig::haptics::Haptic;
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
    PutAway,
    Pack,
    FarGrab,
    FillMag,
    SeatMag,
    Rack,
    Safety,
    HitPlate,
    OpenDoor,
    Search,
    Dressing,
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
            Goal::PutAway => "Holster or pocket it",
            Goal::Pack => "Something in the pack",
            Goal::FarGrab => "Fetch the box up top",
            Goal::FillMag => "Rounds into the mag",
            Goal::SeatMag => "Mag into the pistol",
            Goal::Rack => "Rack the slide",
            Goal::Safety => "Safety off",
            Goal::HitPlate => "Hit a plate",
            Goal::OpenDoor => "Open the door",
            Goal::Search => "Take what's inside",
            Goal::Dressing => "Use a dressing",
            Goal::Artifact => "Take the stone",
            Goal::Button => "Push the button",
            Goal::CallTruck => "Call the truck",
            Goal::PullCord => "Pull the cord",
        }
    }

    /// How, with the control to use: two short lines at most.
    pub fn how(self, flat: bool) -> &'static str {
        match (self, flat) {
            (Goal::TakePistol | Goal::TakeRadio, false) => "Reach into the crate.\nSqueeze GRIP.",
            (Goal::TakePistol | Goal::TakeRadio, true) => "Look at it in the crate.\nPress E.",
            (Goal::PutAway, false) => "Hold it at your hip or\nchest. Open your hand.",
            (Goal::PutAway, true) => "Hold it. Press G.",
            (Goal::Pack, false) => "Hold it over your shoulder.\nOpen your hand.",
            (Goal::Pack, true) => "Hold something big (the\ncleaning kit). Press G.",
            (Goal::FarGrab, false) => "Point at it. Squeeze GRIP.\nFlick your hand back.",
            (Goal::FarGrab, true) => "Look up at it. Press E.",
            (Goal::FillMag, false) => "Mag in one hand, box in\nthe other. Touch them.",
            (Goal::FillMag, true) => "Mag in one hand, box in\nthe other. Press C.",
            (Goal::SeatMag, false) => "Pistol in one hand. Push\nthe mag up into the grip.",
            (Goal::SeatMag, true) => "Pistol in hand. Press R.",
            (Goal::Rack, false) => "Other hand on the slide:\nTRIGGER, pull back, let go.",
            (Goal::Rack, true) => "Pistol in hand. Press V.",
            (Goal::Safety, false) => "Click the stick on the\npistol's hand: safe, fire.",
            (Goal::Safety, true) => "Pistol in hand. Press X.",
            (Goal::HitPlate, false) => "Line up the sights.\nSqueeze TRIGGER.",
            (Goal::HitPlate, true) => "Right mouse aims.\nLeft mouse fires.",
            (Goal::OpenDoor, false) => "GRIP the door by its edge\nand pull it round.",
            (Goal::OpenDoor, true) => "Look at the door. Press E.",
            (Goal::Search, false) => "On the crate inside.\nSqueeze GRIP.",
            (Goal::Search, true) => "On the crate inside.\nLook at it. Press E.",
            (Goal::Dressing, false) => "Hold it. Press A or X.",
            (Goal::Dressing, true) => "Hold it. Press U.",
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
}

/// The stations, in order, from the truck north up the track.
pub const STATIONS: &[Station] = &[
    Station {
        title: "YOUR KIT",
        at: [0.0, 0.0],
        rot: 0.0,
        goals: &[Goal::TakePistol, Goal::TakeRadio],
        where_: "the crate in the truck, by your left knee",
    },
    Station {
        title: "PUT IT AWAY",
        at: [3.5, 87.0],
        rot: 0.0,
        goals: &[Goal::PutAway, Goal::Pack],
        where_: "the first lamp, off the back of the truck",
    },
    Station {
        title: "OUT OF REACH",
        at: [-3.0, 83.5],
        rot: 0.0,
        goals: &[Goal::FarGrab],
        where_: "the lamp left of the track, by the tall crates",
    },
    Station {
        title: "THE RANGE",
        at: [-14.0, 46.0],
        rot: 0.0,
        goals: &[
            Goal::FillMag,
            Goal::SeatMag,
            Goal::Rack,
            Goal::Safety,
            Goal::HitPlate,
        ],
        where_: "the range: the table left of the track, past the gate",
    },
    Station {
        title: "THE HOUSE",
        at: [7.0, 27.0],
        rot: 90.0,
        goals: &[Goal::OpenDoor, Goal::Search],
        where_: "the house, right of the track",
    },
    Station {
        title: "FIRST AID",
        at: [4.5, 8.0],
        rot: 90.0,
        goals: &[Goal::Dressing],
        where_: "the lamp right of the track, by the dressing station",
    },
    Station {
        title: "THE SHIMMER",
        at: [4.5, -30.0],
        rot: 90.0,
        goals: &[Goal::Artifact],
        where_: "the lamp right of the track: mind the shimmer past it",
    },
    Station {
        title: "BUTTONS",
        at: [-3.5, -50.0],
        rot: 270.0,
        goals: &[Goal::Button],
        where_: "the lamp left of the track, further north",
    },
    Station {
        title: "GOING HOME",
        at: [4.0, -64.0],
        rot: 0.0,
        goals: &[Goal::CallTruck, Goal::PullCord],
        where_: "the last lamp, at the north post",
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
/// The tall stand the far-grab box sits on, and how tall.
pub const FAR_STAND_AT: [f32; 2] = [-6.5, 79.5];
pub const FAR_STAND_H: f32 = 2.45;
/// Two wide crates, then a narrow one on top: its edge is close enough
/// under the box that a hand pointing up from below sees the box.
pub const FAR_STAND_LOW: f32 = 1.6;
pub const FAR_STAND_TOP: f32 = 0.36;
/// The range's table: centre, size; the shooter stands on its south side.
pub const RANGE_TABLE_AT: [f32; 2] = [-21.0, 42.0];
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
    /// The stack of crates the far-grab box sits on.
    FarStand,
    /// The range's table.
    RangeTable,
    /// A crate to put a thing on.
    Crate(Vec3),
    /// The board by the tailgate, on its post.
    BoardPost,
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

/// The training burner.
#[derive(Component)]
pub struct Shimmer;

/// Board sizes: a station's (read from a few steps off), the truck's (from
/// where you stand in the bed), the list by the tailgate.
pub const SIGN: Vec2 = crate::world::notes::SIGN;
pub const TRUCK_SIGN: Vec2 = Vec2::new(0.8, 0.5);
pub const CHECKLIST: Vec2 = Vec2::new(0.8, 1.0);
/// A post's board centre, and its lamp's height.
pub const BOARD_Y: f32 = 1.55;
pub const LAMP_Y: f32 = 2.32;
pub const SKIP_CARD: Vec2 = Vec2::new(0.22, 0.09);
pub const PUSH_CARD: Vec2 = Vec2::new(0.3, 0.12);

pub struct OnboardingPlugin;

impl Plugin for OnboardingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Onboarding>()
            .add_message::<GoalDone>()
            .add_systems(PostStartup, (setup, bot_in_the_bed).run_if(in_yard))
            .add_systems(
                Update,
                (
                    (
                        watch_hands,
                        watch_guns,
                        watch_uses.before(crate::items::usage::UsageSystems),
                        watch_world,
                        press_cards,
                    ),
                    announce,
                    (lamps, signs, checklist),
                    warn_near_anomalies,
                    hurry_the_truck,
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
                    Transform::from_xyz(-0.5, FLOOR + 1.62, -1.05)
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
            paper(String::new(), size),
            board_at,
            Visibility::default(),
            ChildOf(post),
        ));
        if STATIONS[i].goals.contains(&Goal::Button) {
            commands.spawn((
                card(OnboardCard::Push, "PUSH ME", PUSH_CARD),
                Transform::from_xyz(-0.3, BOARD_Y - SIGN.y * 0.5 - 0.16, 0.03),
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
    commands.spawn((skip, Transform::from_xyz(0.2, 0.62, 0.03), ChildOf(post)));

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

    // Out of reach: crates stacked taller than an arm, a box of rounds on
    // top (the range wants it).
    let stand = ground(&zone, FAR_STAND_AT);
    commands.spawn((
        Name::new("Far stand"),
        Prop::FarStand,
        Transform::from_translation(stand),
        Visibility::default(),
        RigidBody::Static,
        Collider::compound(vec![
            (
                Vec3::Y * FAR_STAND_LOW * 0.5,
                Quat::IDENTITY,
                Collider::cuboid(0.7, FAR_STAND_LOW, 0.7),
            ),
            (
                Vec3::Y * (FAR_STAND_LOW + FAR_STAND_H) * 0.5,
                Quat::IDENTITY,
                Collider::cuboid(FAR_STAND_TOP, FAR_STAND_H - FAR_STAND_LOW, FAR_STAND_TOP),
            ),
        ]),
        CollisionLayers::new(Layer::World, LayerMask::ALL),
    ));
    let far = spawn(
        &mut commands,
        "ammo_9x18",
        Transform::from_translation(stand + Vec3::Y * (FAR_STAND_H + 0.06)),
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
    if let Some(e) = spawn(
        &mut commands,
        "art_hearth",
        Transform::from_translation(heart + Vec3::Y * 0.4),
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
                raid.say("Driver: \"From the top, then. The crate in the truck.\"");
            }
            _ => {}
        }
    }
}

/// The driver on the radio (your wrist): the opening line, and after each
/// station, where the next one is. Both hands buzz at each tick.
fn announce(
    mut done: MessageReader<GoalDone>,
    mut ob: ResMut<Onboarding>,
    mut raid: ResMut<Raid>,
    mut haptics: MessageWriter<Haptic>,
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
        raid.say(if flat {
            "Driver: \"Your kit's in the crate, front left. Look at it, press E.\""
        } else {
            "Driver: \"Your kit's in the crate, by your left knee. Reach in and squeeze.\""
        });
    }
    for d in done.read() {
        for side in Side::BOTH {
            haptics.write(Haptic {
                side,
                amplitude: if d.station_done { 0.7 } else { 0.45 },
                seconds: if d.station_done { 0.15 } else { 0.06 },
            });
        }
        if !d.station_done {
            continue;
        }
        let line = match ob.current() {
            Some(next) if next > station_of(d.goal) => {
                format!("Driver: \"Good. Next: {}.\"", STATIONS[next].where_)
            }
            Some(_) => continue,
            None => "Driver: \"That's the lot. Off to the Zone.\"".to_string(),
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
pub fn sign_text(ob: &Onboarding, i: usize, flat: bool) -> String {
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
    t += "\n";
    match next {
        Some(g) => t += g.how(flat),
        None => {
            t += "DONE.";
            if i + 1 < STATIONS.len() {
                t += " Follow the\nnext lamp.";
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
    mut signs: Query<(&StationSign, &mut ScreenText)>,
) {
    let flat = *driver == RigDriver::Flat;
    for (s, mut screen) in &mut signs {
        let t = sign_text(&ob, s.0, flat);
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
        raid.say("Driver: \"Too close! That shimmer burns. Step back out.\"");
        for side in Side::BOTH {
            haptics.write(Haptic {
                side,
                amplitude: 0.9,
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

/// Seconds the yard's truck takes to come round.
pub const HURRY: f32 = 10.0;

pub mod bot;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::screen::{MIN_EM_DEG, em_metres, read_from};

    fn readable(screen: &ScreenText) -> Result<(), String> {
        let em = em_metres(screen);
        let at = read_from(screen);
        let deg = (em / at).atan().to_degrees();
        if deg < MIN_EM_DEG {
            return Err(format!(
                "{:.1} cm letters, {deg:.2} deg at {at} m: {:?}",
                em * 100.0,
                screen.text
            ));
        }
        Ok(())
    }

    /// Every board's words, at every stage, in both modes, read from where
    /// you'd stand (the readability test sees only what's up at the start).
    #[test]
    fn every_board_reads_at_every_stage() {
        let mut bad = Vec::new();
        let goals: Vec<Goal> = all_goals().collect();
        for k in 0..=goals.len() {
            for skipped in [false, true] {
                let mut ob = Onboarding::default();
                ob.done = goals[..k].to_vec();
                if skipped {
                    ob.skipped = ob.done.clone();
                }
                for flat in [false, true] {
                    for i in 0..STATIONS.len() {
                        let size = if i == 0 { TRUCK_SIGN } else { SIGN };
                        let s = paper(sign_text(&ob, i, flat), size);
                        if let Err(e) = readable(&s) {
                            bad.push(e);
                        }
                    }
                }
                if let Err(e) = readable(&paper(checklist_text(&ob), CHECKLIST)) {
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
            if let Err(e) = readable(&s) {
                bad.push(e);
            }
        }
        bad.dedup();
        assert!(bad.is_empty(), "too small:\n{}", bad.join("\n"));
    }

    /// Every station asks for something, and every instruction is short:
    /// a couple of lines, with the control named.
    #[test]
    fn instructions_are_short_and_name_the_control() {
        for g in all_goals() {
            for flat in [false, true] {
                let how = g.how(flat);
                assert!(how.lines().count() <= 2, "{g:?}: {how}");
                assert!(how.lines().all(|l| l.len() <= 28), "{g:?}: {how}");
                let control = if flat {
                    ["Press", "mouse"].iter().any(|k| how.contains(k))
                } else {
                    ["GRIP", "TRIGGER", "Press A", "hand", "finger"]
                        .iter()
                        .any(|k| how.contains(k))
                };
                assert!(control, "{g:?} names no control: {how}");
            }
            assert!(g.label().len() <= 22, "{g:?}");
        }
    }
}
