//! The Custodian's snow plough: a tracked rotary snow-blower that keeps
//! Rime Fell's pass road open for its convoys. It carries no gun, and it
//! only runs where snow lies. (Its rules are `extraction_core::plough`.)
//!
//! - **The tell.** Its amber beacon turns, its auger roars, and its chute
//!   throws a plume of snow you see a valley off. Something in its lane
//!   ahead, it sounds its horn, seconds before it gets there: it doesn't
//!   stop.
//! - **The lane.** Behind it the road is packed and clean, and takes no
//!   prints for a while (`Lanes`): the one way through the snow the
//!   print-reading machines can't follow. In full view, and it comes back.
//! - **The bank.** Whatever lies in its way, loot and all, goes out of
//!   its chute into a bank on the downhill side, buried. Hold a hand in
//!   the bank to dig it out ([`Buried`]).
//! - **The weakness.** Anything heavy (a crowbar, a rifle, a fuel can)
//!   into its auger jams it. Jammed, its engine hatch springs open over
//!   the power cell: shoot the cell and it's done. It clears the jam in
//!   half a minute and the hatch shuts. An EMP stalls it.
//! - **The salvage.** What you saw on it: its cell, its sensor bar, its
//!   beacon, the drive sprockets at the back of its tracks and its auger's
//!   motor, each seated where it was (`plough_wreck`); its fuel and a piece
//!   of its auger lie by it.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::plough::{self as rules, Leg, Run};

use super::{PartRole, Robot, RobotKind, part_bundle};
use crate::weapons::ballistics::Hit;
use crate::world::Zone;

/// What its parts take. Its body shrugs off rounds; its cell is what
/// matters, and it's under the hatch.
pub const HULL_HP: f32 = 4000.0;
pub const CELL_HP: f32 = 90.0;
pub const EYE_HP: f32 = 80.0;
/// Where its parts sit (local; the root's on the road under its middle).
pub const HULL: Vec3 = Vec3::new(0.0, 1.1, 0.4);
pub const CELL: Vec3 = Vec3::new(0.0, 1.5, 1.85);
pub const HATCH: Vec3 = Vec3::new(0.0, 1.75, 1.9);
/// How far the cell rides up out of its well when the hatch springs.
pub const CELL_LIFT: f32 = 0.6;
pub const EYE: Vec3 = Vec3::new(0.0, 2.2, -0.75);
pub const AUGER: Vec3 = Vec3::new(0.0, 0.75, -2.0);
pub const CHUTE_TOP: Vec3 = Vec3::new(1.5, 2.9, -1.4);
/// Counters a scenario reads.
pub const JAMS: &str = "plough_jams";
pub const BURIED: &str = "plough_buried";
pub const DUG: &str = "plough_dug";
pub const HORNS: &str = "plough_horns";
/// Seconds between horn blasts while something's in its lane.
const HORN_EVERY: f32 = 1.5;
/// How far apart (m) the bank's sections are laid down.
pub const BANK_STEP: f32 = 4.0;

/// A plough.
#[derive(Component, Debug)]
pub struct Plough {
    /// The road it keeps (x/z), from its shed up.
    pub road: Vec<[f32; 2]>,
    pub run: Run,
    /// Seconds of jam left (0: running).
    pub jammed: f32,
    /// Seconds to the next horn blast.
    pub horn: f32,
    /// Whether it's ploughing now (moving, in snow): the plume and roar.
    pub ploughing: bool,
    /// The slots of road it's banked ([`bank_slot`]): each once, so a
    /// pass back down lays no second bank over the first.
    pub banked: std::collections::HashSet<i32>,
    /// Its heading, degrees (the game's yaw).
    pub yaw: f32,
    /// Whether it needs snow lying to run (a scenario's plough doesn't).
    pub needs_snow: bool,
}

/// Its engine hatch (a part): shut, it covers the cell.
#[derive(Component, Debug)]
pub struct PloughHatch;

/// Its power cell (its core).
#[derive(Component, Debug)]
pub struct PloughCell;

/// Something it threw into the bank: hidden in the snow, until a hand
/// digs it out.
#[derive(Component, Debug, Clone, Copy)]
pub struct Buried {
    /// Seconds of digging so far.
    pub dug: f32,
}

/// A section of bank by the road: its `slot` is which [`BANK_STEP`] of
/// the road it lies beside ([`bank_slot`]), one to a slot however often
/// the plough passes, so the look can sweep a road's pieces into one bank.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankPiece {
    pub slot: i32,
}

/// Which slot of bank `s` metres along the road is in.
pub fn bank_slot(s: f32) -> i32 {
    (s / BANK_STEP).floor() as i32
}

/// Stretches of road it's cleared, and when (the raid clock, last
/// cleared): straight runs of up to [`RUN_LEN`], each new bit of lane
/// joining the last run while it carries straight on. A lane takes no
/// prints while it's fresh (`rules::lane_fresh`): [`wipe_lanes`] takes
/// them up as they're laid. The terrain draws them packed
/// (`world::snow::Packed`, [`pack_lanes`]).
#[derive(Resource, Debug, Default)]
pub struct Lanes(pub Vec<(Vec2, Vec2, f32)>);

/// The longest a lane's run gets (m), and how far it may bend and still
/// be the same run (radians).
pub const RUN_LEN: f32 = 25.0;
const RUN_BEND: f32 = 0.15;
/// The most runs kept (the oldest go).
const RUNS: usize = 256;

impl Lanes {
    /// Whether `p` lies in a lane still fresh at `now` with snow falling
    /// this hard.
    pub fn clean(&self, p: Vec3, now: f32, falling: f32) -> bool {
        let q = Vec2::new(p.x, p.z);
        self.0.iter().any(|(a, b, t)| {
            rules::lane_fresh(now - t, falling) && seg_dist(q, *a, *b) < rules::BLADE * 0.5
        })
    }

    /// A bit of lane cleared from `a` to `b` at `now`: onto the last run
    /// if it carries it straight on, else a new run.
    pub fn clear(&mut self, a: Vec2, b: Vec2, now: f32) {
        if a.distance(b) < 1e-3 {
            return;
        }
        if let Some((ra, rb, t)) = self.0.last_mut() {
            let run = *rb - *ra;
            let more = b - a;
            let straight =
                run.length() < 1e-3 || run.normalize().angle_to(more.normalize()).abs() < RUN_BEND;
            if rb.distance(a) < 0.5 && straight && ra.distance(b) <= RUN_LEN {
                *rb = b;
                *t = now;
                return;
            }
        }
        self.0.push((a, b, now));
        if self.0.len() > RUNS {
            self.0.remove(0);
        }
    }
}

fn seg_dist(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// What a plough did, for sound and light.
#[derive(Message, Debug, Clone, Copy)]
pub struct PloughEvent {
    pub plough: Entity,
    pub at: Vec3,
    pub what: PloughAct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PloughAct {
    /// Something's in its lane ahead.
    Horn,
    /// Something went into the bank.
    Buried,
    /// Something heavy in the auger: jammed, the hatch open.
    Jammed,
    /// The jam cleared, the hatch shut.
    Cleared,
    /// Something caught in its auger.
    Caught,
    /// Dug out of the bank.
    Dug,
}

pub struct PloughPlugin;

impl Plugin for PloughPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PloughEvent>()
            .init_resource::<crate::bot::Counters>()
            .init_resource::<Lanes>()
            .add_systems(
                FixedUpdate,
                (
                    plough_drive,
                    plough_mouth,
                    dig_banks,
                    plough_wreck,
                    plough_plate_off,
                )
                    .chain()
                    .before(PhysicsSystems::First),
            )
            .add_systems(FixedUpdate, (place_ploughs, stage_scenario))
            .add_systems(
                Update,
                (wipe_lanes, pack_lanes).after(crate::world::snow::Treading),
            );
    }
}

/// A plough in its shed at the foot of `road` (x/z points).
pub fn spawn_plough(commands: &mut Commands, zone: &Zone, road: Vec<[f32; 2]>) -> Entity {
    let len = rules::length(&road);
    let (p, d) = rules::along(&road, 0.0);
    let yaw = extraction_core::sentry::bearing([0.0, 0.0], d);
    let robot = commands
        .spawn((
            Name::new("Plough"),
            Robot {
                kind: RobotKind::Pylon,
                wrecked: false,
            },
            super::Breed("plough"),
            super::heavy::OwnLook,
            super::heavy::NotOnPatrol,
            Plough {
                road,
                run: Run::new(len),
                jammed: 0.0,
                horn: 0.0,
                ploughing: false,
                banked: Default::default(),
                yaw,
                needs_snow: true,
            },
            Transform::from_translation(zone.on_ground(p[0], p[1]))
                .with_rotation(Quat::from_rotation_y(-yaw.to_radians())),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    // Struck, it jolts about the ground under it: heavier than a heavy,
    // a Mosin's round about 2 degrees, quick to settle (`rock`).
    commands.entity(robot).insert(super::rock::Rocks {
        pivot: Vec3::ZERO,
        full: 0.04,
        hz: 3.0,
        damping: 0.45,
    });
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Plough hull"),
            part_bundle(
                robot,
                PartRole::Armour(0),
                HULL_HP,
                // (Short of its tail: the cell sits behind it, under the
                // hatch, so a round can reach it once the hatch is open.)
                Collider::cuboid(2.2, 1.4, 3.1),
                Transform::from_translation(HULL - Vec3::Z * 0.35),
            ),
            // (Its body shrugs off a crowbar as it does rounds.)
            super::ShrugsOffPry,
        ));
        c.spawn((
            Name::new("Plough cell"),
            PloughCell,
            part_bundle(
                robot,
                PartRole::Core,
                CELL_HP,
                Collider::cuboid(0.6, 0.35, 0.4),
                Transform::from_translation(CELL),
            ),
            // Under the hatch: nothing reaches it until the hatch opens.
            ColliderDisabled,
        ));
        c.spawn((
            Name::new("Plough hatch"),
            PloughHatch,
            Transform::from_translation(HATCH),
            Visibility::default(),
        ));
        c.spawn((
            Name::new("Plough eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                EYE_HP,
                Collider::cuboid(1.4, 0.15, 0.1),
                Transform::from_translation(EYE),
            ),
        ));
        c.spawn((
            Name::new("Plough housing"),
            part_bundle(
                robot,
                PartRole::Armour(1),
                HULL_HP,
                Collider::cuboid(3.0, 1.1, 1.0),
                Transform::from_translation(AUGER + Vec3::Y * 0.1),
            ),
        ));
    });
    robot
}

/// Along its road on its beat; in its shed while no snow lies.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn plough_drive(
    mut commands: Commands,
    mut ploughs: Query<(
        Entity,
        &mut Plough,
        &mut Transform,
        &Robot,
        Has<super::Stunned>,
    )>,
    zone: Res<Zone>,
    snow: Option<Res<crate::world::snow::Snow>>,
    mut lanes: ResMut<Lanes>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let lying = snow.as_ref().map_or(0.0, |s| s.cover);
    for (e, mut p, mut t, robot, stunned) in &mut ploughs {
        if robot.wrecked {
            p.ploughing = false;
            continue;
        }
        p.jammed = (p.jammed - dt).max(0.0);
        // No snow worth clearing: it stays in (or heads back to) its shed.
        let out =
            !p.needs_snow || lying >= rules::NEEDS_COVER || !matches!(p.run.leg, Leg::Rest(_));
        let speed = if p.jammed > 0.0 || stunned || !out {
            0.0
        } else {
            rules::SPEED
        };
        let before = t.translation;
        let (s, moving) = p.run.step(speed, dt);
        let (at, dir) = rules::along(&p.road, s);
        let ground = zone.on_ground(at[0], at[1]);
        t.translation = ground;
        let facing = p.run.facing();
        let want = extraction_core::sentry::bearing([0.0, 0.0], [dir[0] * facing, dir[1] * facing]);
        let off = extraction_core::sentry::wrap_deg(want - p.yaw);
        let step = rules::TURN * dt.max(1e-3) * 4.0;
        p.yaw = extraction_core::sentry::wrap_deg(p.yaw + off.clamp(-step, step));
        t.rotation = Quat::from_rotation_y(-p.yaw.to_radians());
        let in_snow = !p.needs_snow || snow.as_ref().is_some_and(|sn| sn.lies(ground.y) > 0.05);
        p.ploughing = moving && in_snow;
        if p.ploughing {
            lanes.clear(
                Vec2::new(before.x, before.z),
                Vec2::new(ground.x, ground.z),
                now,
            );
            // The bank, a section to each slot of road ploughed (once,
            // whichever way it's going), on the downhill side.
            let slot = bank_slot(s);
            if p.banked.insert(slot) {
                if let Some(piece) = bank_piece(&zone, &p.road, slot) {
                    commands.spawn((Name::new("Plough bank"), piece, OfPlough(e)));
                }
            }
        }
    }
}

/// Prints laid in a fresh lane go as they're made (the lane takes none):
/// a scan of the fresh runs, only when the prints have changed.
fn wipe_lanes(
    lanes: Res<Lanes>,
    tracks: Option<ResMut<crate::world::snow::Tracks>>,
    snow: Option<Res<crate::world::snow::Snow>>,
    mut seen: Local<u32>,
    time: Res<Time>,
) {
    let Some(mut tracks) = tracks else { return };
    if tracks.version == *seen || lanes.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let falling = snow.map_or(0.0, |s| s.falling);
    for (a, b, t) in &lanes.0 {
        if !rules::lane_fresh(now - t, falling) {
            continue;
        }
        let along = *b - *a;
        tracks.wipe(
            Vec3::new(a.x, 0.0, a.y),
            rules::BLADE * 0.5,
            Vec3::new(along.x, 0.0, along.y),
            along.length(),
        );
    }
    *seen = tracks.version;
}

/// The fresh runs to the terrain, to draw packed: written only when a run
/// is added or its freshness crosses a step.
fn pack_lanes(
    lanes: Res<Lanes>,
    packed: Option<ResMut<crate::world::snow::Packed>>,
    snow: Option<Res<crate::world::snow::Snow>>,
    time: Res<Time>,
) {
    let Some(mut packed) = packed else { return };
    let now = time.elapsed_secs();
    let life = rules::lane_life(snow.map_or(0.0, |s| s.falling));
    let want: Vec<crate::world::snow::PackedLane> = lanes
        .0
        .iter()
        .filter_map(|(a, b, t)| {
            let fresh = 1.0 - (now - t) / life;
            (fresh > 0.0).then(|| crate::world::snow::PackedLane {
                a: *a,
                b: *b,
                half_width: rules::BLADE * 0.5,
                // In steps, so it's written now and then, not every frame.
                fresh: (fresh * 20.0).ceil() / 20.0,
            })
        })
        .collect();
    if packed.lanes != want {
        packed.lanes = want;
    }
}

/// Which piece of bank belongs to which plough.
#[derive(Component, Debug, Clone, Copy)]
pub struct OfPlough(pub Entity);

/// The section of bank for `slot` of `road`: beside the slot's start on
/// the downhill side, along the road; none at a junction or a hut's door,
/// nor in a gallery.
pub fn bank_piece(zone: &Zone, road: &[[f32; 2]], slot: i32) -> Option<impl Bundle> {
    let (at, dir) = rules::along(road, slot as f32 * BANK_STEP);
    let ground = zone.on_ground(at[0], at[1]);
    let b = ground + bank_dir(zone, ground, dir) * rules::BANK_OUT;
    rules::bank_ok(&zone.map, [b.x, b.z]).then(|| {
        (
            BankPiece { slot },
            Transform::from_translation(zone.on_ground(b.x, b.z)).with_rotation(
                Quat::from_rotation_y(
                    -extraction_core::sentry::bearing([0.0, 0.0], dir).to_radians(),
                ),
            ),
            Visibility::default(),
        )
    })
}

/// The downhill side of the road at `at` going along `dir` (unit, world).
fn bank_dir(zone: &Zone, at: Vec3, dir: [f32; 2]) -> Vec3 {
    let right = Vec3::new(-dir[1], 0.0, dir[0]);
    let probe = |v: Vec3| {
        let p = at + v * rules::BANK_OUT;
        zone.ground(p.x, p.z)
    };
    right * rules::bank_side(probe(-right), probe(right))
}

/// Its mouth: what it horns at, what it catches, what it throws in the
/// bank, what jams it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn plough_mouth(
    mut commands: Commands,
    mut ploughs: Query<(Entity, &mut Plough, &GlobalTransform, &Robot, &Children)>,
    players: Query<&GlobalTransform, With<crate::player::Player>>,
    bodies: Query<(Entity, &GlobalTransform), With<crate::player::BodyCollider>>,
    parts: Query<(Entity, &super::RobotPart, &GlobalTransform)>,
    items: Query<
        (Entity, &crate::items::Item, &GlobalTransform),
        (crate::interact::Loose, Without<Buried>),
    >,
    cells: Query<Entity, With<PloughCell>>,
    catalog: Res<crate::items::Catalog>,
    zone: Res<Zone>,
    mut hits: MessageWriter<Hit>,
    mut out: MessageWriter<PloughEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut p, gt, robot, children) in &mut ploughs {
        let cell = children.iter().find(|c| cells.contains(*c));
        if robot.wrecked {
            continue;
        }
        let here = gt.translation();
        let fwd = gt.forward().as_vec3();
        let right = gt.right().as_vec3();
        let local = |q: Vec3| {
            let d = q - here;
            (d.dot(fwd), d.dot(right), d.y)
        };
        let running = p.jammed <= 0.0 && matches!(p.run.leg, Leg::Up(_) | Leg::Down(_));
        // The horn: someone in its lane ahead.
        p.horn = (p.horn - dt).max(0.0);
        let ahead = players.iter().any(|g| {
            let (a, s, _) = local(g.translation());
            rules::in_lane_ahead(a, s)
        });
        if running && ahead && p.horn <= 0.0 {
            p.horn = HORN_EVERY;
            counters.bump(HORNS);
            out.write(PloughEvent {
                plough: e,
                at: here + fwd * 2.0,
                what: PloughAct::Horn,
            });
        }
        if !running {
            continue;
        }
        // The auger: a person or a machine in its mouth is caught.
        for (b, g) in &bodies {
            let (a, s, h) = local(g.translation());
            if rules::in_mouth(a, s) && h < 2.0 {
                hits.write(Hit {
                    entity: b,
                    point: g.translation(),
                    normal: -fwd,
                    velocity: fwd * 20.0,
                    damage: rules::AUGER_HARM * dt,
                    shooter: Some(e),
                    mass_g: 0.0,
                    load: Default::default(),
                    blow: None,
                });
                out.write(PloughEvent {
                    plough: e,
                    at: g.translation(),
                    what: PloughAct::Caught,
                });
            }
        }
        for (pe, part, g) in &parts {
            if part.robot == e {
                continue;
            }
            let (a, s, h) = local(g.translation());
            if rules::in_mouth(a, s) && h < 2.0 {
                hits.write(Hit {
                    entity: pe,
                    point: g.translation(),
                    normal: -fwd,
                    velocity: fwd * 20.0,
                    damage: rules::AUGER_HARM * 2.0 * dt,
                    shooter: Some(e),
                    mass_g: 0.0,
                    load: Default::default(),
                    blow: None,
                });
            }
        }
        // Loose things in its mouth: the heavy jam it, the rest go in the
        // bank.
        let dir2 = [fwd.x, fwd.z];
        let dir2 = {
            let l = (dir2[0] * dir2[0] + dir2[1] * dir2[1]).sqrt().max(1e-4);
            [dir2[0] / l, dir2[1] / l]
        };
        for (it, item, g) in &items {
            let (a, s, h) = local(g.translation());
            if !rules::in_mouth(a, s) || h > 2.0 {
                continue;
            }
            let mass = catalog.get(item.kind()).mass;
            if mass >= rules::JAM_KG {
                // Jammed: the item's lodged in the flights (gone), the
                // engine labours and cuts out, and the hatch springs.
                commands.entity(it).despawn();
                p.jammed = rules::JAM_TIME;
                counters.bump(JAMS);
                info!("plough: jammed on a {}", catalog.get(item.kind()).id);
                if let Some(c) = cell {
                    // The hatch springs and the cell rides up out of its
                    // well, clear of the deck: there to be shot.
                    commands
                        .entity(c)
                        .remove::<ColliderDisabled>()
                        .insert(Transform::from_translation(CELL + Vec3::Y * CELL_LIFT));
                }
                out.write(PloughEvent {
                    plough: e,
                    at: here + fwd * rules::MOUTH,
                    what: PloughAct::Jammed,
                });
                break;
            }
            // Out of the chute, into the bank.
            let side = bank_dir(&zone, here, dir2);
            let spot = g.translation() + side * rules::BANK_OUT;
            let ground = zone.on_ground(spot.x, spot.z);
            commands.entity(it).insert((
                Buried { dug: 0.0 },
                Transform::from_translation(ground - Vec3::Y * 0.3),
                Visibility::Hidden,
                ColliderDisabled,
                RigidBodyDisabled,
            ));
            counters.bump(BURIED);
            out.write(PloughEvent {
                plough: e,
                at: ground,
                what: PloughAct::Buried,
            });
        }
    }
    // Jams that have cleared: the hatch shuts over the cell.
    for (e, p, gt, robot, children) in &ploughs {
        if robot.wrecked || p.jammed > 0.0 {
            continue;
        }
        if let Some(c) = children.iter().find(|c| cells.contains(*c)) {
            commands
                .entity(c)
                .try_insert((ColliderDisabled, Transform::from_translation(CELL)));
        }
        let _ = (e, gt);
    }
}

/// A hand held in the bank over something buried digs it out.
fn dig_banks(
    mut commands: Commands,
    mut buried: Query<(Entity, &mut Buried, &Transform)>,
    hands: Query<(&crate::interact::Hand, &GlobalTransform)>,
    mut out: MessageWriter<PloughEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (e, mut b, t) in &mut buried {
        let at = t.translation + Vec3::Y * 0.3;
        let digging = hands.iter().any(|(h, g)| {
            h.grip_down && h.holding.is_none() && g.translation().distance(at) < rules::DIG_REACH
        });
        if !digging {
            b.dug = 0.0;
            continue;
        }
        b.dug += dt;
        if b.dug >= rules::DIG {
            commands
                .entity(e)
                .remove::<(Buried, ColliderDisabled, RigidBodyDisabled)>()
                .insert((
                    Visibility::Inherited,
                    Transform::from_translation(at + Vec3::Y * 0.25),
                ));
            counters.bump(DUG);
            out.write(PloughEvent {
                plough: Entity::PLACEHOLDER,
                at,
                what: PloughAct::Dug,
            });
        }
    }
}

/// Wrecked (its cell shot through): it stops where it is, and what you saw
/// on it is what comes off (`salvage`, #105). Its cell in its well under
/// the sprung hatch (scrap if it was shot through; live if not), its
/// sensor bar sagging off the cab front by its lugs, its beacon snapped
/// off the roof onto the deck (both by hand, in reach from the ground),
/// the drive sprocket at the back of each track (three bolts) and the
/// auger's motor on the housing's right end (four). Its fuel and a
/// piece of its auger lie by it.
#[allow(clippy::type_complexity)]
fn plough_wreck(
    mut commands: Commands,
    ploughs: Query<
        (Entity, &Robot, &GlobalTransform, &Children),
        // (Not `Broken`: the wreck module marks a pylon-kind machine broken
        // the moment it's wrecked, before this sees it.)
        (With<Plough>, Without<super::Salvaged>),
    >,
    mut parts: Query<(&super::RobotPart, &mut Transform, Has<PloughCell>)>,
    catalog: Res<crate::items::Catalog>,
    mut seat_looks: Query<(&super::salvage::SeatLook, &mut Visibility)>,
    (mut meshes, mut materials): (
        Option<ResMut<Assets<Mesh>>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
) {
    for (e, robot, g, children) in &ploughs {
        if !robot.wrecked {
            continue;
        }
        commands
            .entity(e)
            .insert((super::wreck::Broken, super::Salvaged));
        info!("plough: wrecked at {:.0}", g.translation());
        let look = super::salvage::Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
        let mut seated = Vec::new();
        for c in children.iter() {
            let Ok((p, mut t, cell)) = parts.get_mut(c) else {
                continue;
            };
            let health = (p.hp / p.max_hp.max(1.0)).clamp(0.0, 1.0);
            let role = match p.role {
                PartRole::Core if cell => PartRole::Core,
                PartRole::Sensor => {
                    *t = sagged_eye();
                    PartRole::Sensor
                }
                _ => continue,
            };
            if let Some(row) = wreck_row(role, health) {
                seated.push(role);
                super::salvage::fit(&mut commands, &catalog, c, row, &look);
            }
        }
        // On the root: the beacon, the sprockets, the motor (whole: its
        // body shrugs off rounds).
        for role in [
            PartRole::Lamp(0),
            PartRole::Leg(0),
            PartRole::Leg(1),
            PartRole::Rotor(0),
        ] {
            if let Some(row) = wreck_row(role, WHOLE) {
                seated.push(role);
                super::salvage::fit(&mut commands, &catalog, e, row, &look);
            }
        }
        for (l, mut vis) in &mut seat_looks {
            if l.robot == e && seated.contains(&l.role) {
                *vis = Visibility::Hidden;
            }
        }
        for (k, id) in ["fuel_can", "scrap_metal"].into_iter().enumerate() {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let a = k as f32 * 1.6;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        g.translation() + Vec3::new(a.cos() * 2.4, 0.5, a.sin() * 2.4),
                    ),
                    condition: 0.6,
                },
            );
        }
    }
}

/// The chevroned plate along its auger housing's top rail, prised off with
/// a crowbar in two halves (its seam is the housing's, `crowbar::pryable_plates`):
/// salvage, `plough_plate`, dropped where it was; its look goes
/// (`plough_look`, keyed `Armour(1)`).
fn plough_plate_off(
    mut commands: Commands,
    mut destroyed: MessageReader<super::PartDestroyed>,
    ploughs: Query<&GlobalTransform, With<Plough>>,
    catalog: Res<crate::items::Catalog>,
    mut seat_looks: Query<(&super::salvage::SeatLook, &mut Visibility)>,
) {
    for d in destroyed.read() {
        if d.role != PartRole::Armour(1) {
            continue;
        }
        let Ok(root) = ploughs.get(d.robot) else {
            continue;
        };
        info!(
            "plough: its rail plate prised off at {:.0}",
            root.translation()
        );
        let Some(kind) = catalog.kind("plough_plate") else {
            continue;
        };
        let (_, rot, _) = root.to_scale_rotation_translation();
        // (It comes off in two halves, a piece a person carries each.)
        for side in [-1.0, 1.0] {
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        root.transform_point(PLATE_DROP + Vec3::X * side * PLATE_HALF_APART),
                    )
                    .with_rotation(rot),
                    condition: PLATE_CONDITION,
                },
            );
        }
        for (l, mut vis) in &mut seat_looks {
            if l.robot == d.robot && l.role == PartRole::Armour(1) {
                *vis = Visibility::Hidden;
            }
        }
    }
}

/// The housing plate's middle (local to the root: `art/robots/plough.py`
/// prints it from the housing's origin), and its state when it comes off.
pub const PLATE_AT: Vec3 = Vec3::new(0.0, 0.85 + 0.251, -1.95 - 0.499);
/// Where it falls once it's off: just out in front of the housing, clear of
/// the housing's own collider (spawned inside it, it would be thrown).
pub const PLATE_DROP: Vec3 = Vec3::new(0.0, 0.6, -2.85);
pub const PLATE_CONDITION: f32 = 0.7;
/// Its two halves' middles, either side of the plate's.
pub const PLATE_HALF_APART: f32 = 0.8;

/// The condition of what's on its root at its wreck: its body shrugs off
/// rounds, so its beacon, sprockets and motor come through worn, not shot.
pub const WHOLE: f32 = 0.75;
/// Its eye bar, sagged off the cab front by one lug (local to the root).
pub fn sagged_eye() -> Transform {
    Transform::from_translation(EYE + Vec3::new(0.15, -0.5, -0.08))
        .with_rotation(Quat::from_rotation_z(0.3))
}
/// Where its beacon lies once it's snapped off the roof: on the engine
/// deck, by the hatch (local to the root).
pub const BEACON_DOWN: Vec3 = Vec3::new(0.55, 2.0, 1.0);
/// The drive sprockets at the back of its tracks, and the auger's motor
/// (local to the root: `art/robots/plough.py`'s origins), and each
/// item's middle from there.
pub const SPROCKET: Vec3 = Vec3::new(0.85, 0.45, 1.77);
pub const SPROCKET_MID: f32 = -0.05;
pub const MOTOR: Vec3 = Vec3::new(1.75, 0.75, -2.0);
pub const MOTOR_MID: Vec3 = Vec3::new(-0.015, 0.11, 0.0);
pub const SPROCKET_BOLTS: usize = 3;
pub const MOTOR_BOLTS: usize = 4;

/// What a plough's wreck gives up, and how each is held: `Leg(i)` its
/// sprockets (0 left, 1 right), `Rotor(0)` its auger's motor, `Lamp(0)`
/// its beacon (keys its look holders share: `plough_look`).
pub fn wreck_row(role: PartRole, health: f32) -> Option<super::salvage::Row> {
    use super::salvage::{Fix, Row};
    let row = |item, at: Transform, fix, hot| Row {
        item,
        at,
        size: 1.0,
        health,
        fix,
        hot,
        replaces: None,
    };
    Some(match role {
        PartRole::Core => row("plough_cell", Transform::IDENTITY, Fix::Hand, true),
        PartRole::Sensor => row("plough_eye", Transform::IDENTITY, Fix::Hand, false),
        PartRole::Lamp(_) => row(
            "plough_beacon",
            Transform::from_translation(BEACON_DOWN),
            Fix::Hand,
            false,
        ),
        PartRole::Leg(i) => {
            let sx = if i == 0 { -1.0 } else { 1.0 };
            let at = Vec3::new(sx * SPROCKET.x, SPROCKET.y, SPROCKET.z);
            // (The item is the right one: the left is it turned round.)
            let turn = if i == 0 {
                Quat::from_rotation_y(std::f32::consts::PI)
            } else {
                Quat::IDENTITY
            };
            row(
                "plough_track_drive",
                Transform::from_translation(at + Vec3::X * sx * SPROCKET_MID).with_rotation(turn),
                Fix::Bolts {
                    n: SPROCKET_BOLTS,
                    centre: at + Vec3::X * sx * 0.22,
                    face: Quat::from_rotation_arc(Vec3::Y, Vec3::X * sx),
                    radius: 0.1,
                },
                false,
            )
        }
        PartRole::Rotor(0) => row(
            "plough_auger_motor",
            Transform::from_translation(MOTOR + MOTOR_MID),
            Fix::Bolts {
                n: MOTOR_BOLTS,
                centre: Vec3::new(1.655, MOTOR.y, MOTOR.z),
                face: Quat::from_rotation_arc(Vec3::Y, Vec3::X),
                radius: 0.185,
            },
            false,
        ),
        _ => return None,
    })
}

/// The map's plough, in its shed, once the Zone is in (robots on): one
/// on the valley's road named [`ROAD`], in a valley where snow lies.
pub const ROAD: &str = rules::ROAD;

fn place_ploughs(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    opts: Option<Res<crate::LaunchOptions>>,
    mut done: Local<bool>,
) {
    let Some(zone) = zone else { return };
    if *done {
        return;
    }
    *done = true;
    if !opts.is_some_and(|o| o.robots) || zone.map.snow <= 0.0 {
        return;
    }
    if let Some(beat) = rules::beat(&zone.map) {
        spawn_plough(&mut commands, &zone, beat);
    }
}

/// The scenario: by a snowy road, a plough coming. A tin of stew lies in
/// its way and goes in the bank; the bot lays a crowbar in its lane, it
/// jams, its hatch opens and the bot shoots its cell.
pub const SCENARIO: &str = "plough_jam";

#[allow(clippy::too_many_arguments)]
fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
    mut bots: Query<&mut crate::bot::Bot>,
) {
    for s in setups.read() {
        if s.name != SCENARIO {
            continue;
        }
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        let side0 = Vec3::new(-f.z, 0.0, f.x);
        // A straight road across in front of the bot, 6 m out, run from
        // 40 m off to one side to 40 m off to the other.
        let open = |d: Vec3, side: Vec3| {
            (-40..=40).step_by(4).all(|k| {
                let p = s.at + d * 6.0 + side * k as f32;
                nav.0.walkable([p.x, p.z])
            })
        };
        let (dir, side) = (0..8)
            .map(|k| {
                let q = Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 8.0);
                (q * f, q * side0)
            })
            .find(|(d, sd)| open(*d, *sd))
            .unwrap_or((f, side0));
        let mid = s.at + dir * 6.0;
        let a = mid - side * 40.0;
        let b = mid + side * 40.0;
        let road = vec![[a.x, a.z], [b.x, b.z]];
        let e = spawn_plough(&mut commands, &zone, road);
        // (Whatever this valley's weather: the scenario wants it out.)
        commands.queue(move |w: &mut World| {
            if let Some(mut p) = w.get_mut::<Plough>(e) {
                p.needs_snow = false;
            }
        });
        info!("scenario: a plough coming along {a:.0} -> {b:.0}");
        // A tin in its way, 10 m before the bot's crossing.
        let tin = mid - side * 10.0;
        for (id, at) in [
            ("canned_stew", tin),
            ("crowbar", s.at + side * 0.6),
            ("ak74", s.at - side * 0.6),
            ("ak_mag", s.at - side * 1.0),
        ] {
            if let Some(kind) = catalog.kind(id) {
                let it = crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::from_translation(
                            zone.on_ground(at.x, at.z) + Vec3::Y * 0.1,
                        ),
                        condition: 1.0,
                    },
                );
                if id == "ak_mag" {
                    commands.queue(move |w: &mut World| {
                        if let Some(mut i) = w.get_mut::<crate::items::Item>(it) {
                            i.rounds = 30;
                        }
                    });
                }
            }
        }
        // Fill in the bot's placeholders: where to lay the crowbar (in the
        // lane, ahead of where the plough is now), and its target.
        let lay = mid - side * 3.0;
        for mut bot in &mut bots {
            for t in bot.tasks.iter_mut() {
                match t {
                    crate::bot::Task::WalkTo { to, .. } if *to == LAY_PLACEHOLDER => {
                        *to = Vec2::new(lay.x, lay.z);
                    }
                    crate::bot::Task::WalkTo { to, .. } if *to == BACK_PLACEHOLDER => {
                        *to = Vec2::new(s.at.x, s.at.z);
                    }
                    crate::bot::Task::Shoot { target, .. } if *target == Entity::PLACEHOLDER => {
                        *target = e;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Filled in by the setup: where the bot lays the crowbar, and where it
/// steps back to.
pub const LAY_PLACEHOLDER: Vec2 = Vec2::new(-6e6, -6e6);
pub const BACK_PLACEHOLDER: Vec2 = Vec2::new(-7e6, -7e6);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Acts(Vec<PloughAct>, f32);

    fn collect(
        mut ev: MessageReader<PloughEvent>,
        mut acts: ResMut<Acts>,
        mut hurt: MessageReader<crate::health::Hurt>,
    ) {
        acts.0.extend(ev.read().map(|e| e.what));
        acts.1 += hurt.read().map(|h| h.amount).sum::<f32>();
    }

    /// A plough on a straight road from `a` toward `b` (x/z), running
    /// whatever the snow.
    fn world(a: Vec2, b: Vec2) -> (crate::vr_harness::Vr, Zone, Entity) {
        let mut vr = crate::vr_harness::Vr::new(extraction_core::map::TEST_FIELD_ID);
        vr.app.init_resource::<Acts>().add_systems(Update, collect);
        let zone = vr.app.world().resource::<Zone>().clone();
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_plough(&mut c, &zone, vec![a.to_array(), b.to_array()]);
            vr.app.world_mut().flush();
            e
        };
        vr.app.world_mut().get_mut::<Plough>(e).unwrap().needs_snow = false;
        (vr, zone, e)
    }

    fn drop_item(vr: &mut crate::vr_harness::Vr, zone: &Zone, id: &str, at: Vec2) -> Entity {
        vr.spawn(
            id,
            Transform::from_translation(zone.on_ground(at.x, at.y) + Vec3::Y * 0.1),
        )
    }

    /// Standing in its lane: the horn sounds well before it gets there.
    /// Step aside and it passes; stay, and the auger has you.
    #[test]
    fn it_horns_then_its_auger_has_you() {
        let (mut vr, zone, _) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        vr.stand(zone.on_ground(40.0, 238.0), 0.0);
        let mut first_horn = None;
        for k in 0..(90 * 14) {
            vr.step(1);
            let acts = &vr.app.world().resource::<Acts>().0;
            if first_horn.is_none() && acts.contains(&PloughAct::Horn) {
                first_horn = Some(k);
            }
            if acts.contains(&PloughAct::Caught) {
                let horn = first_horn.expect("a horn first");
                let lead = (k - horn) as f32 / 90.0;
                assert!(lead > 4.0, "{lead:.1} s from the horn to the auger");
                vr.step(90);
                assert!(vr.app.world().resource::<Acts>().1 > 0.0, "and it hurt");
                return;
            }
        }
        panic!("it never reached you");
    }

    /// A tin in its way goes in the bank, out of sight; a hand held in the
    /// bank digs it out.
    #[test]
    fn what_it_ploughs_goes_in_the_bank_and_digs_out() {
        use crate::rig::Side;
        let (mut vr, zone, _) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        vr.stand(zone.on_ground(60.0, 250.0), 0.0);
        let tin = drop_item(&mut vr, &zone, "canned_stew", Vec2::new(40.0, 248.0));
        vr.step(90 * 8);
        let buried = *vr.app.world().get::<Transform>(tin).unwrap();
        assert!(vr.app.world().get::<Buried>(tin).is_some(), "buried");
        assert_eq!(
            vr.app.world().get::<Visibility>(tin),
            Some(&Visibility::Hidden)
        );
        let off = Vec2::new(buried.translation.x - 40.0, 0.0).length();
        assert!(off > 3.0, "beside the road, in the bank: {off:.1} m out");
        // Dig.
        let at = buried.translation + Vec3::Y * 0.3;
        vr.stand(zone.on_ground(at.x + 0.5, at.z), 0.0);
        vr.hand(Side::Left, at, Quat::IDENTITY, 1.0, 0.0);
        vr.step((90.0 * (rules::DIG + 0.5)) as usize);
        assert!(vr.app.world().get::<Buried>(tin).is_none(), "dug out");
        assert!(
            vr.app
                .world()
                .resource::<Acts>()
                .0
                .contains(&PloughAct::Dug)
        );
    }

    /// A valley with every one of these machines out (its sentries and
    /// spotter on the map, a minelayer, two menders, a plough) still
    /// sends a logistics run and a response team: none of them count
    /// against the director's active robots (`heavy::NotOnPatrol`).
    #[test]
    fn the_machines_leave_the_director_room() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
            robots: true,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let nav = app
            .world()
            .resource::<super::super::walker::Nav>()
            .0
            .clone();
        {
            let mut c = app.world_mut().commands();
            let at = zone.on_ground(40.0, 230.0);
            super::super::minelayer::spawn_minelayer(&mut c, &zone, &nav, at, vec![at], at);
            for k in 0..2 {
                let m = at + Vec3::new(k as f32 * 5.0, 9.0, 0.0);
                super::super::mender::spawn_mender(&mut c, m, m);
            }
            spawn_plough(&mut c, &zone, vec![[60.0, 240.0], [60.0, 200.0]]);
            app.world_mut().flush();
        }
        app.update();
        let standing = app
            .world_mut()
            .query_filtered::<&Robot, With<super::super::heavy::NotOnPatrol>>()
            .iter(app.world())
            .count();
        assert!(standing >= 6, "a valley's worth of them: {standing}");
        {
            let mut d = app
                .world_mut()
                .resource_mut::<super::super::director::Director>();
            d.next_logistics = 0.0;
            d.response_cooldown = 0.0;
            let at = zone.on_ground(40.0, 230.0);
            d.queue_response(0.0, at);
        }
        for _ in 0..90 {
            app.update();
        }
        let mut q = app
            .world_mut()
            .query::<(&Robot, Option<&super::super::Job>)>();
        let robots: Vec<(RobotKind, Option<super::super::JobKind>)> = q
            .iter(app.world())
            .map(|(r, j)| (r.kind, j.map(|j| j.kind)))
            .collect();
        assert!(
            robots.iter().any(|(k, _)| *k == RobotKind::Dirigible),
            "a logistics run: {robots:?}"
        );
        assert!(
            robots
                .iter()
                .any(|(_, j)| *j == Some(super::super::JobKind::Response)),
            "a response team: {robots:?}"
        );
    }

    /// Nothing carried is eaten: in a hand, a socket, a pack, a crowbar in
    /// its mouth is left alone (`interact::Loose`); a loose one jams it.
    #[test]
    fn it_eats_nothing_carried() {
        let (mut vr, zone, e) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        vr.stand(zone.on_ground(60.0, 250.0), 0.0);
        let carried: Vec<Entity> = (0..3)
            .map(|k| {
                let it = drop_item(
                    &mut vr,
                    &zone,
                    "crowbar",
                    Vec2::new(40.0, 252.0 - k as f32 * 1.5),
                );
                let mut w = vr.app.world_mut().entity_mut(it);
                match k {
                    0 => {
                        w.insert(crate::interact::Held {
                            player: it,
                            hand: it,
                            side: crate::rig::Side::Left,
                            point: 0,
                            offset: Transform::IDENTITY,
                        });
                    }
                    1 => {
                        w.insert(crate::interact::InSocket {
                            socket: Entity::PLACEHOLDER,
                        });
                    }
                    _ => {
                        w.insert(crate::interact::body::InBackpack {
                            bag: Entity::PLACEHOLDER,
                        });
                    }
                }
                it
            })
            .collect();
        vr.step(90 * 8);
        for (k, it) in carried.iter().enumerate() {
            assert!(
                vr.app.world().get_entity(*it).is_ok(),
                "carried crowbar {k} was eaten"
            );
        }
        assert!(
            vr.app.world().get::<Plough>(e).unwrap().jammed <= 0.0,
            "and nothing carried jammed it"
        );
        let loose = drop_item(&mut vr, &zone, "crowbar", Vec2::new(40.0, 225.0));
        vr.step(90 * 8);
        let jammed = vr.app.world().get::<Plough>(e).unwrap().jammed > 0.0;
        assert!(
            jammed || vr.app.world().get_entity(loose).is_err(),
            "a loose one still is"
        );
    }

    /// A crowbar in its way jams it: it stops, the cell comes up to be
    /// shot; after the jam clears it goes on.
    #[test]
    fn a_crowbar_jams_it() {
        let (mut vr, zone, e) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        vr.stand(zone.on_ground(60.0, 250.0), 0.0);
        drop_item(&mut vr, &zone, "crowbar", Vec2::new(40.0, 250.0));
        vr.step(90 * 6);
        let p = vr.app.world().get::<Plough>(e).unwrap();
        assert!(p.jammed > 0.0, "jammed");
        let at = vr.app.world().get::<Transform>(e).unwrap().translation;
        vr.step(90 * 3);
        let still = vr.app.world().get::<Transform>(e).unwrap().translation;
        assert!(at.distance(still) < 0.05, "it stopped");
        let cell = vr
            .app
            .world_mut()
            .query_filtered::<(Entity, &Transform), With<PloughCell>>()
            .iter(vr.app.world())
            .next()
            .map(|(c, t)| (c, t.translation))
            .unwrap();
        assert!(cell.1.y > CELL.y + 0.3, "the cell's up");
        assert!(vr.app.world().get::<ColliderDisabled>(cell.0).is_none());
        vr.step((90.0 * rules::JAM_TIME) as usize);
        let on = vr.app.world().get::<Transform>(e).unwrap().translation;
        assert!(on.distance(still) > 1.0, "the jam cleared and it went on");
        assert!(vr.app.world().get::<ColliderDisabled>(cell.0).is_some());
    }

    /// Up the road and back down it leaves one bank, not a second laid
    /// over the first: a section to each slot of road, whichever way.
    #[test]
    fn up_and_back_banks_the_road_once() {
        let (mut vr, _, e) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 236.0));
        let leg = |vr: &crate::vr_harness::Vr| vr.app.world().get::<Plough>(e).unwrap().run.leg;
        let mut went = 0;
        while !matches!(leg(&vr), Leg::Top(_)) {
            vr.step(9);
            went += 1;
            assert!(went < 200, "it never got up the road");
        }
        let up = slots(&mut vr);
        let banked =
            |vr: &crate::vr_harness::Vr| vr.app.world().get::<Plough>(e).unwrap().banked.len();
        // (Some slots lie where no bank goes: by a path, a door.)
        assert!(banked(&vr) >= 5, "banked {} slots going up", banked(&vr));
        assert!(!up.is_empty(), "no bank at all");
        // (Past its wait at the top.)
        vr.app.world_mut().get_mut::<Plough>(e).unwrap().run.leg = Leg::Down(24.0);
        while !matches!(leg(&vr), Leg::Rest(_)) {
            vr.step(9);
            went += 1;
            assert!(went < 400, "it never got back down");
        }
        let both = slots(&mut vr);
        let mut once = both.clone();
        once.dedup();
        assert_eq!(both, once, "a slot banked twice");
        assert!(both.len() <= up.len() + 1, "{up:?} then {both:?}");
    }

    fn slots(vr: &mut crate::vr_harness::Vr) -> Vec<i32> {
        let mut q = vr.app.world_mut().query::<&BankPiece>();
        let mut s: Vec<i32> = q.iter(vr.app.world()).map(|b| b.slot).collect();
        s.sort();
        s
    }

    /// Shot through its cell, it stops, and what you saw on it is seated
    /// where it was, each in a hand's reach from the ground (its beacon
    /// down on the deck, its eye bar sagged off the cab): its cell's scrap,
    /// the rest whole.
    #[test]
    fn its_wreck_is_what_was_on_it() {
        let (mut vr, zone, e) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        vr.stand(zone.on_ground(60.0, 230.0), 0.0);
        vr.step(30);
        let cell = vr
            .app
            .world_mut()
            .query_filtered::<(Entity, &super::super::RobotPart), With<PloughCell>>()
            .iter(vr.app.world())
            .find(|(_, p)| p.robot == e)
            .map(|(c, _)| c)
            .unwrap();
        crate::robots::heavy::tests::hit(&mut vr.app, cell, CELL_HP + 10.0);
        vr.step(90 * 3);
        assert!(vr.app.world().get::<Robot>(e).unwrap().wrecked);
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let seated: Vec<(String, Vec3)> = vr
            .app
            .world_mut()
            .query_filtered::<(&crate::items::Item, &GlobalTransform), With<crate::interact::InSocket>>()
            .iter(vr.app.world())
            .map(|(i, g)| (catalog.get(i.kind()).id.clone(), g.translation()))
            .filter(|(id, _)| id.starts_with("plough_"))
            .collect();
        let mut ids: Vec<&str> = seated.iter().map(|(id, _)| id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "plough_auger_motor",
                "plough_beacon",
                "plough_eye",
                "plough_track_drive",
                "plough_track_drive"
            ],
            "{seated:?}"
        );
        let root = *vr.app.world().get::<GlobalTransform>(e).unwrap();
        for (id, at) in &seated {
            let over = at.y - zone.ground(at.x, at.z);
            assert!(
                (0.0..crate::sim::indoor::MAX_REACH_UP).contains(&over),
                "{id} {over:.2} m over the ground"
            );
            assert!(at.distance(root.translation()) < 3.5, "{id} on it: {at}");
        }
        // Its fuel and a piece of its auger by it; nothing that isn't its.
        let loose: Vec<String> = vr
            .app
            .world_mut()
            .query_filtered::<(&crate::items::Item, &GlobalTransform), Without<crate::interact::InSocket>>()
            .iter(vr.app.world())
            .filter(|(_, g)| g.translation().distance(root.translation()) < 4.0)
            .map(|(i, _)| catalog.get(i.kind()).id.clone())
            .collect();
        assert!(!loose.iter().any(|id| id == "hound_hip_servo"), "{loose:?}");
        assert!(loose.iter().any(|id| id == "fuel_can"), "{loose:?}");
    }

    /// A crowbar finds no seam on its hull (nothing comes off it), but on
    /// its auger housing it prises off the chevroned rail plate: salvage,
    /// dropped where it was.
    #[test]
    fn a_crowbar_takes_its_rail_plate_not_its_hull() {
        use crate::destruction::hooks::{Pried, Pryable};
        let (mut vr, zone, e) = world(Vec2::new(40.0, 260.0), Vec2::new(40.0, 200.0));
        // (Jammed: running, it would bank what falls in its lane, the
        // plate with it.)
        vr.app.world_mut().get_mut::<Plough>(e).unwrap().jammed = 1.0e6;
        vr.stand(zone.on_ground(60.0, 230.0), 0.0);
        vr.step(5);
        let part = |vr: &mut crate::vr_harness::Vr, role| {
            vr.app
                .world_mut()
                .query::<(Entity, &super::super::RobotPart)>()
                .iter(vr.app.world())
                .find(|(_, p)| p.robot == e && p.role == role)
                .map(|(c, _)| c)
                .unwrap()
        };
        let hull = part(&mut vr, PartRole::Armour(0));
        let housing = part(&mut vr, PartRole::Armour(1));
        assert!(
            vr.app.world().get::<Pryable>(hull).is_none(),
            "no seam on its hull"
        );
        assert!(vr.app.world().get::<Pryable>(housing).is_some());
        for _ in 0..60 {
            vr.app.world_mut().write_message(Pried {
                target: housing,
                torque: 1.0e4,
                by: Entity::PLACEHOLDER,
            });
            vr.step(1);
        }
        let root = *vr.app.world().get::<GlobalTransform>(e).unwrap();
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let plates: Vec<Vec3> = vr
            .app
            .world_mut()
            .query::<(&crate::items::Item, &GlobalTransform)>()
            .iter(vr.app.world())
            .filter(|(i, _)| catalog.get(i.kind()).id == "plough_plate")
            .map(|(_, g)| g.translation())
            .collect();
        assert_eq!(plates.len(), 2, "in two halves: {plates:?}");
        assert!(
            plates[0].distance(root.translation()) < 5.0,
            "by it: {} vs {}",
            plates[0],
            root.translation()
        );
    }
}
