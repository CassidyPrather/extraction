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
            .add_systems(
                Update,
                (press_things, drive_truck, finish_raid, fit_ladder).chain(),
            );
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

/// The truck standing at `at` facing `yaw` on its wheels: pitched and
/// rolled to the ground under them, and set down until the highest
/// ground under a wheel meets its tyre. (It used to be lowered as far as
/// 0.8 m so its rear steps reached the ground behind it, which sank its
/// wheels in the soil: the 28 October playtest, 0:12:57. The steps reach
/// down now instead: [`ladder_steps`].)
fn parked(zone: &Zone, at: [f32; 2], yaw: f32) -> Transform {
    let level = Transform::from_translation(zone.on_ground(at[0], at[1]))
        .with_rotation(Quat::from_rotation_y(yaw));
    let ground = |w: &Vec3| {
        let p = level.transform_point(Vec3::new(w.x, 0.0, w.z));
        zone.ground(p.x, p.z)
    };
    let mean = |f: &dyn Fn(&Vec3) -> bool| {
        let (sum, n) = WHEELS
            .iter()
            .filter(|w| f(w))
            .fold((0.0, 0.0), |(s, n), w| (s + ground(w), n + 1.0));
        sum / n
    };
    let (front, rear) = (mean(&|w| w.z < -1.0), mean(&|w| w.z > -1.0));
    let (left, right) = (mean(&|w| w.x < 0.0), mean(&|w| w.x > 0.0));
    // Front axle to the middle of the two rear ones. (The rear ladder
    // hangs plumb whatever the pitch: [`ladder_swing`].)
    let slope = (front - rear).atan2(3.9 + 0.8);
    // Steeper than a bed can stand without its loose kit sliding, the
    // truck stands level and settled low as it used to, its rear steps on
    // the ground and its wheels in the soil: the base stops fall 1 to 1.4 m
    // from cab to tail, and want levelling (`capture::footing` lists them).
    if slope.abs() > MAX_SLOPE {
        let mut t = level;
        let foot = t.transform_point(Vec3::new(0.0, 0.0, 2.6));
        let drop = t.translation.y - zone.ground(foot.x, foot.z);
        t.translation.y -= drop.clamp(0.0, 0.8);
        return t;
    }
    // Pitched no further than a body can still climb its ladder at; the
    // wheels at the low end hang down to the ground the rest of the way
    // ([`wheel_droop`]).
    let pitch = slope.clamp(-MAX_PITCH, MAX_PITCH);
    let roll = (right - left).atan2(2.0);
    let mut t = level.with_rotation(
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll),
    );
    // Each tyre's lowest points, round its contact patch, against the
    // ground under them.
    let lift = WHEELS
        .iter()
        .flat_map(|w| (-2..=2).map(move |k| *w + Vec3::new(0.0, -w.y, k as f32 * 0.05)))
        .map(|c| {
            let bottom = t.transform_point(c);
            zone.ground(bottom.x, bottom.z) - bottom.y
        })
        .fold(f32::MIN, f32::max);
    // Tyres settle a little into the soil.
    t.translation.y += lift - WHEEL_SETTLE;
    t
}

/// The most a parked truck pitches end to end (radians, 12 degrees): past
/// it the low wheels hang down to the ground. And the steepest stop it
/// stands on its wheels at (3 degrees: steeper, kit slides in the bed).
const MAX_PITCH: f32 = 0.21;
const MAX_SLOPE: f32 = 0.05;

/// How far a parked truck's tyres settle into the soil (m).
pub const WHEEL_SETTLE: f32 = 0.025;

/// How far the lowest tread (top 0.37) may stand over the ground behind
/// it before a ramp is hung from it (m): a body steps up 0.4 at most.
const STEP_REACH: f32 = 0.3;
/// The ramp's slope: well inside what a body walks up (`player`'s 0.65
/// normal, about 49 degrees).
const RAMP_SLOPE: f32 = 0.6;

/// The truck's frame turned only about the vertical: the ramp stands
/// level in it.
pub fn level_frame(pose: &Transform) -> Transform {
    let (yaw, ..) = pose.rotation.to_euler(EulerRot::YXZ);
    Transform::from_translation(pose.translation).with_rotation(Quat::from_rotation_y(yaw))
}

/// Where the rear ladder hangs from, truck space (`art/vehicles/truck.py`'s
/// `ladder`).
pub const LADDER_HINGE: Vec3 = Vec3::new(0.0, 1.0, 1.85);

/// How the rear ladder turns about [`LADDER_HINGE`], truck space, to hang
/// plumb when the truck stands pitched or rolled. Pitched with the truck,
/// a step's back face leaned over a body pressing on it, and a body can't
/// step up under an overhang (`player::try_step`).
pub fn ladder_swing(pose: &Transform) -> Quat {
    pose.rotation.inverse() * level_frame(pose).rotation
}

/// A point on the ladder (truck space, as built) where it hangs.
pub fn on_ladder(pose: &Transform, p: Vec3) -> Vec3 {
    LADDER_HINGE + ladder_swing(pose) * (p - LADDER_HINGE)
}

/// A boarding ramp hung from the lowest rear step down to the ground
/// where the ground falls away behind the truck, so the bed is in reach
/// from it: (size, centre, tilt about X) in [`level_frame`]. A plank 0.9
/// wide and 5 cm thick, its top from the tread's back edge (z 2.4, y 0.37
/// in the truck's frame) down to the lowest ground behind.
pub fn ladder_steps(zone: &Zone, pose: &Transform) -> Vec<(Vec3, Vec3, f32)> {
    let level = level_frame(pose);
    let inv = level.compute_affine().inverse();
    // Where the lowest tread's back edge hangs, in the level frame.
    let edge =
        inv.transform_point3(pose.transform_point(on_ladder(pose, Vec3::new(0.0, 0.37, 2.4))));
    // The lowest ground across the ramp's width at `z`, level frame.
    let ground = |z: f32| {
        [-0.4, 0.0, 0.4]
            .iter()
            .map(|x| {
                let w = level.transform_point(Vec3::new(*x, 0.0, z));
                zone.ground(w.x, w.z) - level.translation.y
            })
            .fold(f32::MAX, f32::min)
    };
    let top = edge.y;
    if top - ground(edge.z + 0.15) <= STEP_REACH {
        return Vec::new();
    }
    // Out until the slope meets the ground.
    let mut run = 0.3;
    while run < 3.0 && top - RAMP_SLOPE * run > ground(edge.z + run) {
        run += 0.05;
    }
    let drop = RAMP_SLOPE * run;
    let len = run.hypot(drop);
    let tilt = drop.atan2(run);
    let thick = 0.05;
    // Its top face's middle, then down by half its thickness along its
    // normal.
    let mid = Vec3::new(edge.x, top - drop * 0.5, edge.z + run * 0.5);
    let normal = Vec3::new(0.0, run / len, drop / len);
    vec![(Vec3::new(0.9, thick, len), mid - normal * thick * 0.5, tilt)]
}

/// One of the truck's fixed rear steps: its centre in the truck's frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct RearStep(pub Vec3);

/// The boarding ramp ([`ladder_steps`]): its size (`render::props` draws
/// it).
#[derive(Component, Clone, Copy, Debug)]
pub struct LadderStep(pub Vec3);

/// Fits the truck's ladder to the ground behind it wherever it stands.
fn fit_ladder(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    trucks: Query<(Entity, &Transform, Option<&Children>), (With<Truck>, Changed<Transform>)>,
    all: Query<(), With<Truck>>,
    steps: Query<(), With<LadderStep>>,
    mut rear: Query<(&RearStep, &mut Transform), Without<Truck>>,
    mut fitted: Local<std::collections::HashMap<Entity, Transform>>,
) {
    let Some(zone) = zone else { return };
    // A truck gone (the raid over) is forgotten.
    fitted.retain(|t, _| all.contains(*t));
    for (truck, pose, children) in &trucks {
        // Only where it has moved to (physics touches it every frame).
        if fitted.get(&truck).is_some_and(|f| {
            f.translation.distance(pose.translation) < 0.01
                && f.rotation.angle_between(pose.rotation) < 0.01
        }) {
            continue;
        }
        fitted.insert(truck, *pose);
        // The fixed rear steps hang plumb from the ladder's hinge.
        let swing = ladder_swing(pose);
        for c in children.into_iter().flatten() {
            if let Ok((step, mut t)) = rear.get_mut(*c) {
                t.translation = on_ladder(pose, step.0);
                t.rotation = swing;
            }
        }
        for c in children.into_iter().flatten() {
            if steps.contains(*c) {
                commands.entity(*c).despawn();
            }
        }
        // From the level frame into the truck's.
        let to_truck = Transform::from_matrix(Mat4::from(
            pose.compute_affine().inverse() * level_frame(pose).compute_affine(),
        ));
        for (size, centre, tilt) in ladder_steps(&zone, pose) {
            commands.spawn((
                Name::new("Boarding ramp"),
                LadderStep(size),
                Collider::cuboid(size.x, size.y, size.z),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                to_truck
                    * Transform::from_translation(centre)
                        .with_rotation(Quat::from_rotation_x(tilt)),
                Visibility::default(),
                ChildOf(truck),
            ));
        }
    }
}

/// How far a wheel drops on its springs to meet the ground (m): an army
/// truck's axles articulate a long way.
pub const WHEEL_DROOP: f32 = 0.4;

/// How far wheel `i` hangs below its place in `WHEELS` to meet the ground
/// under it (the truck is rigid, the ground under six wheels isn't flat).
pub fn wheel_droop(zone: &Zone, pose: &Transform, i: usize) -> f32 {
    let w = WHEELS[i];
    let bottom = pose.transform_point(w) - pose.rotation * Vec3::Y * w.y;
    (bottom.y - zone.ground(bottom.x, bottom.z)).clamp(0.0, WHEEL_DROOP)
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
            let mut e = c.spawn((
                Collider::cuboid(size.x, size.y, size.z),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                Transform::from_translation(*centre),
            ));
            // The rear steps stay level however the truck stands
            // (`level_steps`).
            if centre.z > 1.9 {
                e.insert(RearStep(*centre));
            }
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
    commands
        .spawn((
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
        ))
        .with_children(|c| {
            // Solid, as it's drawn: the stand and the crate's lower half one
            // block from the bed up to the shelf, and four walls round the
            // shelf, open on top. It had no collider at all: things dropped
            // over it fell through, and one came to rest under it, out of
            // sight and reach.
            let (w, d) = STASH_OUTER;
            let floor = -(STASH_RAISE + 0.35);
            let solid = -floor;
            c.spawn((
                Name::new("Stash crate body"),
                Collider::cuboid(w, solid, d),
                stash_layers(),
                Transform::from_xyz(0.0, floor + solid * 0.5, 0.0),
            ));
            let (wall, high) = (0.03, STASH_WALL);
            for (size, at) in [
                (
                    Vec3::new(wall, high, d),
                    Vec3::new(-(w - wall) * 0.5, high * 0.5, 0.0),
                ),
                (
                    Vec3::new(wall, high, d),
                    Vec3::new((w - wall) * 0.5, high * 0.5, 0.0),
                ),
                (
                    Vec3::new(w, high, wall),
                    Vec3::new(0.0, high * 0.5, -(d - wall) * 0.5),
                ),
                (
                    Vec3::new(w, high, wall),
                    Vec3::new(0.0, high * 0.5, (d - wall) * 0.5),
                ),
            ] {
                c.spawn((
                    Name::new("Stash crate wall"),
                    Collider::cuboid(size.x, size.y, size.z),
                    stash_layers(),
                    Transform::from_translation(at),
                ));
            }
        });
}

/// The crate stops loose things, and nothing else: a crate the body
/// couldn't pass kept it from the cord in the narrow bed, and one a held
/// thing's leash took for a wall kept the kit in it.
fn stash_layers() -> CollisionLayers {
    CollisionLayers::new(Layer::Container, [Layer::Item])
}

/// The stash crate's outside, width and depth (as drawn).
pub const STASH_OUTER: (f32, f32) = (0.95, 1.25);
/// How high its walls stand over the shelf.
pub const STASH_WALL: f32 = 0.325;

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
    if !opts.plays_for_real() || opts.camera.is_some() {
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

/// How far (m) from where the truck stands someone must be for it to
/// pull up there: its length and a little.
pub const PARK_CLEAR: f32 = 5.0;

/// The rendezvous pose, or the nearest one beside it with nobody standing
/// in it: someone new waits right on the spot, and the truck came down on
/// top of them (inside its bed's walls, they could neither move nor
/// board).
pub fn rv_pose_clear(zone: &Zone, rv: usize, people: &[Vec3]) -> Transform {
    let base = rv_pose(zone, rv);
    let clear = |t: &Transform| {
        people
            .iter()
            .all(|p| p.xz().distance(t.translation.xz()) > PARK_CLEAR)
    };
    if clear(&base) {
        return base;
    }
    let (yaw, _, _) = base.rotation.to_euler(EulerRot::YXZ);
    // Pulled on past them, or short of them, or alongside.
    for local in [
        Vec3::new(0.0, 0.0, -9.0),
        Vec3::new(0.0, 0.0, 9.0),
        Vec3::new(6.0, 0.0, 0.0),
        Vec3::new(-6.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -15.0),
    ] {
        let at = base.transform_point(local);
        let t = parked(zone, [at.x, at.z], yaw);
        if clear(&t) {
            return t;
        }
    }
    base
}

/// Arrived, the truck pulls in out of your sight, up to this long (s)
/// after it's due: never popping in where you're looking (playtest 28
/// October: "it just spawns in"). Its engine's heard coming
/// (`audio::truck_sounds`).
pub const UNSEEN_FOR: f32 = 8.0;
/// Where you're looking: this far either side of the head's line, this
/// near.
const SIGHT_CONE: f32 = 0.9;
const SIGHT_RANGE: f32 = 150.0;

/// Whether a head sees `at`.
pub fn sees(head: &GlobalTransform, at: Vec3) -> bool {
    let to = at - head.translation();
    let d = to.length();
    d < SIGHT_RANGE && (d < 1.0 || head.forward().as_vec3().angle_between(to) < SIGHT_CONE)
}

/// The truck appears at the rendezvous when it arrives (where you're not
/// looking, [`UNSEEN_FOR`]), and goes home after.
fn drive_truck(
    raid: Res<Raid>,
    zone: Res<Zone>,
    mut trucks: Query<(&mut Truck, &mut Transform), Without<Player>>,
    people: Query<&GlobalTransform, With<Player>>,
    heads: Query<&GlobalTransform, With<crate::rig::RigHead>>,
) {
    for (mut truck, mut t) in &mut trucks {
        match raid.phase {
            // The short exfil: it never left.
            Phase::TruckWaiting { rv, .. } if rv == super::SHORT_EXFIL => {}
            Phase::TruckWaiting { rv, left } if truck.at_base => {
                let people: Vec<Vec3> = people.iter().map(|p| p.translation()).collect();
                let at = rv_pose_clear(&zone, rv, &people);
                let due_for = super::TRUCK_WAIT - left;
                let watched = heads
                    .iter()
                    .any(|h| sees(h, at.translation + Vec3::Y * 1.5));
                if watched && due_for < UNSEEN_FOR {
                    continue;
                }
                *t = at;
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

/// Woken in the truck after going down.
pub const WOKE_LINE: &str = "You went down. You wake in the back of the truck.";

/// End of a raid: drive home with whatever you're carrying, or wake up in
/// the truck with nothing.
fn finish_raid(
    mut commands: Commands,
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
        // A broken leg's set in town between raids (never by a night in a
        // safehouse: out there it's a splint and a limp).
        commands
            .entity(e)
            .remove::<crate::interact::wounds::Fracture>();
        body.velocity = Vec3::ZERO;
    }
    raid.phase = Phase::AtBase;
    save.write(crate::persist::SaveNow);
    // Home whole, the driver has his say (`raid::lev`, it sees what you
    // carry).
    if died {
        // What happened, as the eyes open (the fade: `render::upgrades`):
        // the kit's where you fell, as the driver says next.
        raid.say(WOKE_LINE);
    }
}

/// Everything a downed player carried is left where they fell (for the
/// memorial convoy to collect, or for you to get back first).
#[derive(Message, Clone, Copy, Debug)]
pub struct DropEverything {
    pub player: Entity,
    pub at: Vec3,
}

#[cfg(test)]
mod park_tests {
    use super::*;
    use crate::raid::{Phase, Raid};

    /// Someone waiting right on the rendezvous: the truck pulls up beside
    /// them, not on top of them.
    #[test]
    fn the_truck_pulls_up_clear_of_whoever_waits_on_the_spot() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>();
        let spot = zone
            .map
            .markers_of(&MarkerKind::Rendezvous)
            .next()
            .map(|m| zone.on_ground(m.at[0], m.at[1]))
            .unwrap();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation = spot + Vec3::Y;
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        // (Past the wait for you to look away: `UNSEEN_FOR`.)
        app.world_mut().resource_mut::<Raid>().phase = Phase::TruckWaiting {
            rv: 0,
            left: crate::raid::TRUCK_WAIT - UNSEEN_FOR - 2.0,
        };
        app.update();
        app.update();
        let truck = app
            .world_mut()
            .query_filtered::<&Transform, With<Truck>>()
            .iter(app.world())
            .next()
            .unwrap()
            .translation;
        let d = truck.xz().distance(spot.xz());
        assert!(d > PARK_CLEAR, "parked on them: {d:.1} m");
        assert!(d < 20.0, "and still at the rendezvous: {d:.1} m");
    }

    /// Playtest 28 October: "it just spawns in". Watched, the truck holds
    /// off pulling in; looked away from, or after a while, it's there.
    #[test]
    fn the_truck_never_pops_in_where_you_are_looking() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let spot = rv_pose(&zone, 0).translation;
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        // 40 m off, facing it.
        let from = zone.on_ground(spot.x + 40.0, spot.z) + Vec3::Y * 0.1;
        let face = |app: &mut App, toward: Vec3| {
            let mut t = app.world_mut().get_mut::<Transform>(player).unwrap();
            t.translation = from;
            t.look_at(Vec3::new(toward.x, from.y, toward.z), Vec3::Y);
        };
        face(&mut app, spot);
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.update();
        let at_rv = |app: &mut App| {
            let t = app
                .world_mut()
                .query_filtered::<&Transform, With<Truck>>()
                .iter(app.world())
                .next()
                .unwrap()
                .translation;
            t.xz().distance(spot.xz()) < 25.0
        };
        app.world_mut().resource_mut::<Raid>().phase = Phase::TruckWaiting {
            rv: 0,
            left: crate::raid::TRUCK_WAIT,
        };
        for _ in 0..5 {
            face(&mut app, spot);
            app.update();
        }
        assert!(!at_rv(&mut app), "popped in while watched");
        // Looked away: it pulls in.
        let away = from + (from - spot);
        for _ in 0..3 {
            face(&mut app, away);
            app.update();
        }
        assert!(at_rv(&mut app), "there once you look away");
        assert!(sees(
            &GlobalTransform::from(Transform::IDENTITY.looking_at(Vec3::NEG_Z, Vec3::Y)),
            Vec3::new(0.0, 0.0, -20.0)
        ));
    }
}
