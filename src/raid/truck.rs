//! The truck: base of operations and ride out.
//!
//! An old six-wheel army truck with a canvas-covered bed. The bed is home:
//! the stash crate lives there (it carries everything you own between
//! raids), and a bell pull on the cab wall tells the driver to go. The truck
//! is a real object in the Zone — it is parked at the south gate between
//! raids and drives round to a rendezvous when called.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::inventory;
use extraction_core::map::MarkerKind;

use super::{DepartRequest, Phase, Raid};
use crate::health::Health;
use crate::interact::body::Backpack;
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::player::{Body, Player};
use crate::rig::LocalPlayer;
use crate::world::{Layer, Zone};

/// Bed interior, truck space: x across, y up from the ground, z along
/// (forward is -Z).
const BED_MIN: Vec3 = Vec3::new(-1.1, 0.9, -1.8);
const BED_MAX: Vec3 = Vec3::new(1.1, 3.1, 1.8);
pub const FLOOR: f32 = 1.1;

#[derive(Component, Debug)]
pub struct Truck {
    /// Where it's parked: base (the south gate) or a rendezvous index.
    pub at_base: bool,
}

impl Truck {
    /// Whether a world point is inside the bed.
    pub fn in_bed(&self, gt: &GlobalTransform, p: Vec3) -> bool {
        let l = gt.affine().inverse().transform_point3(p);
        l.cmpge(BED_MIN).all() && l.cmple(BED_MAX).all()
    }
}

/// Something you grab to *do* something rather than pick it up.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressable {
    /// Tell the driver to go.
    BellPull,
    /// Swing a door open or shut.
    Door,
    /// The bench's selector knob: next recipe.
    BenchSelect,
    /// The bench's lever: run the selected recipe.
    BenchRun,
    /// A camp kitchen station's tool or knob.
    Cook,
    /// Fold a deployed cover back up into an item.
    FoldCover,
    /// The route card: which valley the truck drives to next.
    Destination,
    /// A comfort board card (turning, vignette).
    Setting,
    /// The driver's request card: hand over what's wanted.
    Request,
    /// A terminal in the Zone: next page.
    Terminal,
    /// A card on the camp's board (`camp`).
    Camp,
    /// The logbook: turn to the other page (`raid/logbook.rs`).
    Logbook,
    /// A note on the cork board: take it down, pin it back
    /// (`raid/board.rs`).
    Note,
}

/// The stash crate in the truck bed.
#[derive(Component)]
pub struct Stash;

/// Truck geometry parts, for visuals: (size, local centre, material slot).
pub const TRUCK_PARTS: &[(Vec3, Vec3, u8)] = &[
    // Cab.
    (Vec3::new(2.3, 1.7, 2.0), Vec3::new(0.0, 1.75, -2.9), 0),
    // Bonnet.
    (Vec3::new(2.0, 0.9, 1.1), Vec3::new(0.0, 1.35, -4.4), 0),
    // Chassis rails. They stop under the bed's tail: run out past it, they'd
    // wall off the rear steps (0.9 m is no step).
    (Vec3::new(1.2, 0.3, 6.8), Vec3::new(0.0, 0.75, -1.6), 1),
    // Bed floor.
    (
        Vec3::new(2.3, 0.14, 3.6),
        Vec3::new(0.0, FLOOR - 0.07, 0.0),
        2,
    ),
    // Bed sides (wood planks).
    (
        Vec3::new(0.06, 0.55, 3.6),
        Vec3::new(-1.15, FLOOR + 0.27, 0.0),
        2,
    ),
    (
        Vec3::new(0.06, 0.55, 3.6),
        Vec3::new(1.15, FLOOR + 0.27, 0.0),
        2,
    ),
    // Canopy: canvas sides, front, roof.
    (
        Vec3::new(0.04, 1.5, 3.6),
        Vec3::new(-1.15, FLOOR + 1.3, 0.0),
        3,
    ),
    (
        Vec3::new(0.04, 1.5, 3.6),
        Vec3::new(1.15, FLOOR + 1.3, 0.0),
        3,
    ),
    (
        Vec3::new(2.3, 2.05, 0.04),
        Vec3::new(0.0, FLOOR + 1.02, -1.8),
        3,
    ),
    (
        Vec3::new(2.34, 0.05, 3.64),
        Vec3::new(0.0, FLOOR + 2.05, 0.0),
        3,
    ),
    // Rear steps.
    (Vec3::new(0.9, 0.37, 0.3), Vec3::new(0.0, 0.185, 2.25), 1),
    (Vec3::new(0.9, 0.74, 0.3), Vec3::new(0.0, 0.37, 1.95), 1),
];

/// Wheel centres (truck space).
pub const WHEELS: &[Vec3] = &[
    Vec3::new(-1.0, 0.55, -3.9),
    Vec3::new(1.0, 0.55, -3.9),
    Vec3::new(-1.0, 0.55, 0.2),
    Vec3::new(1.0, 0.55, 0.2),
    Vec3::new(-1.0, 0.55, 1.4),
    Vec3::new(1.0, 0.55, 1.4),
];

pub struct TruckPlugin;

impl Plugin for TruckPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_truck)
            .add_systems(PostStartup, start_in_truck)
            .add_systems(Update, (press_things, drive_truck, finish_raid).chain());
    }
}

/// Base pose: the south-gate truck stop, facing out of the Zone so the bed
/// opens toward it.
pub fn base_pose(zone: &Zone) -> Transform {
    let m = zone.marker(&MarkerKind::TruckStop);
    let at = m.map(|m| m.at).unwrap_or([0.0, 0.0]);
    let ins = zone
        .marker(&MarkerKind::Insertion)
        .map(|m| m.at)
        .unwrap_or([0.0, 0.0]);
    // Rear (+Z) toward the insertion point.
    let back = Vec3::new(ins[0] - at[0], 0.0, ins[1] - at[1]).normalize_or(Vec3::Z);
    let yaw = (back.x).atan2(back.z);
    parked(zone, at, yaw)
}

pub fn rv_pose(zone: &Zone, rv: usize) -> Transform {
    let rvs: Vec<[f32; 2]> = zone
        .map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| m.at)
        .collect();
    let at = rvs.get(rv).copied().unwrap_or([0.0, 0.0]);
    // Park with the bed toward the map centre, so you walk into it.
    let back = Vec3::new(-at[0], 0.0, -at[1]).normalize_or(Vec3::Z);
    let yaw = back.x.atan2(back.z);
    parked(zone, at, yaw)
}

/// The truck standing at `at` facing `yaw`, settled low enough that its
/// rear steps reach the ground where the ground falls away behind it
/// (otherwise the bed is out of reach on a slope).
fn parked(zone: &Zone, at: [f32; 2], yaw: f32) -> Transform {
    let rot = Quat::from_rotation_y(yaw);
    let mut t = Transform::from_translation(zone.on_ground(at[0], at[1])).with_rotation(rot);
    let foot = t.transform_point(Vec3::new(0.0, 0.0, 2.6));
    let drop = t.translation.y - zone.ground(foot.x, foot.z);
    t.translation.y -= drop.clamp(0.0, 0.8);
    t
}

fn spawn_truck(mut commands: Commands, zone: Res<Zone>) {
    let pose = base_pose(&zone);
    let truck = commands
        .spawn((
            Name::new("Truck"),
            Truck { at_base: true },
            pose,
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(truck).with_children(|c| {
        for (size, centre, _) in TRUCK_PARTS {
            c.spawn((
                Collider::cuboid(size.x, size.y, size.z),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_translation(*centre),
            ));
        }
        c.spawn((
            Name::new("Bell pull"),
            Pressable::BellPull,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            // The whole rope down to its wooden toggle (drawn 0.26 below
            // the middle), which is where a hand takes it.
            Collider::cuboid(0.08, 0.62, 0.08),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            Transform::from_xyz(0.7, FLOOR + 1.6, -1.7),
            Visibility::default(),
        ));
    });
    // The stash: a big crate bolted to the bed floor.
    commands.spawn((
        Name::new("Stash crate"),
        Stash,
        Backpack {
            owner: Entity::PLACEHOLDER,
            open: true,
            model: inventory::Backpack {
                capacity: extraction_core::progression::STASH_BASE,
                mass: 0.0,
            },
            contents: Vec::new(),
            carried: false,
            half: Vec3::new(0.45, 0.3, 0.6),
        },
        // Up on a stand, the kit on a shelf in its top: waist height for
        // someone standing in the bed (it sat on the floor; the first
        // playtest found most things to pick up were on the floor).
        Transform::from_xyz(-0.55, FLOOR + STASH_RAISE + 0.35, -1.0),
        Visibility::default(),
        ChildOf(truck),
    ));
}

/// How high the stash crate's stand lifts it off the bed.
pub const STASH_RAISE: f32 = 0.3;

/// The local player starts in the truck bed.
pub(crate) fn start_in_truck(
    zone: Res<Zone>,
    opts: Res<crate::launch::LaunchOptions>,
    mut players: Query<&mut Transform, With<LocalPlayer>>,
    mut stashes: Query<&mut Backpack, Or<(With<Stash>, With<super::bench::Tray>)>>,
    locals: Query<Entity, With<LocalPlayer>>,
) {
    // The stash (and the bench tray) belong to whoever is playing.
    if let Some(p) = locals.iter().next() {
        for mut s in &mut stashes {
            s.owner = p;
        }
    }
    // Scenarios and screenshots place the player themselves.
    if opts.scenario.is_some() || opts.camera.is_some() {
        return;
    }
    let pose = base_pose(&zone);
    for mut t in &mut players {
        t.translation = pose.transform_point(Vec3::new(0.3, FLOOR + 0.05, 0.6));
        t.rotation = pose.rotation;
    }
}

/// Grabbing a pressable presses it.
fn press_things(
    mut events: MessageReader<InteractEvent>,
    pressables: Query<&Pressable>,
    mut depart: MessageWriter<DepartRequest>,
    locals: Query<Entity, With<LocalPlayer>>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    for ev in events.read() {
        if let InteractEvent::Pressed { item, player } = ev {
            match pressables.get(*item) {
                // In the training yard the cord ends the practice
                // (`camp`), it doesn't send the truck anywhere.
                Ok(Pressable::BellPull) if opts.map == crate::camp::TRAINING_YARD => {}
                Ok(Pressable::BellPull) => {
                    depart.write(DepartRequest { player: *player });
                }
                _ => {}
            }
        }
    }
    let _ = locals;
}

/// The truck appears at the rendezvous when it arrives, and goes home after.
fn drive_truck(raid: Res<Raid>, zone: Res<Zone>, mut trucks: Query<(&mut Truck, &mut Transform)>) {
    for (mut truck, mut t) in &mut trucks {
        match raid.phase {
            // The short exfil: it never left.
            Phase::TruckWaiting { rv, .. } if rv == super::SHORT_EXFIL => {}
            Phase::TruckWaiting { rv, .. } if truck.at_base => {
                *t = rv_pose(&zone, rv);
                truck.at_base = false;
            }
            Phase::Out | Phase::TruckCalled { .. } if !truck.at_base => {
                *t = base_pose(&zone);
                truck.at_base = true;
            }
            _ => {}
        }
    }
}

/// End of a raid: drive home with whatever you're carrying, or wake up in
/// the truck with nothing.
fn finish_raid(
    mut raid: ResMut<Raid>,
    time: Res<Time>,
    zone: Res<Zone>,
    mut trucks: Query<(&mut Truck, &mut Transform), Without<Player>>,
    mut players: Query<
        (Entity, &mut Transform, &mut Health, &mut Body),
        (With<LocalPlayer>, With<Player>),
    >,
    mut drop: MessageWriter<crate::raid::truck::DropEverything>,
    mut save: MessageWriter<crate::persist::SaveNow>,
) {
    let dt = time.delta_secs();
    let (done, died) = match &mut raid.phase {
        Phase::Extracting { left } => {
            *left -= dt;
            (*left <= 0.0, false)
        }
        Phase::Down { left } => {
            *left -= dt;
            (*left <= 0.0, true)
        }
        _ => return,
    };
    if !done {
        return;
    }
    let base = base_pose(&zone);
    for (mut truck, mut t) in &mut trucks {
        *t = base;
        truck.at_base = true;
    }
    for (e, mut t, mut h, mut body) in &mut players {
        if died {
            drop.write(DropEverything {
                player: e,
                at: t.translation,
            });
        }
        t.translation = base.transform_point(Vec3::new(0.3, FLOOR + 0.05, 0.6));
        t.rotation = base.rotation;
        *h = Health::default();
        body.velocity = Vec3::ZERO;
    }
    raid.phase = Phase::AtBase;
    save.write(crate::persist::SaveNow);
    // Home whole, the driver has his say (`raid::lev`, it sees what you
    // carry).
    if died {
        raid.say("You wake in the back of the truck. Your kit is gone.");
    }
}

/// Everything a downed player carried is left where they fell (for the
/// memorial convoy to collect, or for you to get back first).
#[derive(Message, Clone, Copy, Debug)]
pub struct DropEverything {
    pub player: Entity,
    pub at: Vec3,
}
