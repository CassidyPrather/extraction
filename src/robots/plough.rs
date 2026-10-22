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
//! - **The salvage.** Its cell, its fuel, its auger's scrap.

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
    /// The road's last ploughed metre, for laying the bank.
    pub banked: f32,
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

/// A section of bank by the road (for its look).
#[derive(Component, Debug, Clone, Copy)]
pub struct BankPiece;

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
                (plough_drive, plough_mouth, dig_banks, plough_wreck)
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
                banked: 0.0,
                yaw,
                needs_snow: true,
            },
            Transform::from_translation(zone.on_ground(p[0], p[1]))
                .with_rotation(Quat::from_rotation_y(-yaw.to_radians())),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
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
            // The bank, every few metres of new road ploughed, on the
            // downhill side.
            if (s - p.banked).abs() >= BANK_STEP {
                p.banked = s;
                let side = bank_dir(&zone, ground, dir);
                let b = ground + side * rules::BANK_OUT;
                // (Not at a junction or a hut's door, nor in a gallery.)
                let bank_here = rules::bank_ok(&zone.map, [b.x, b.z]);
                if bank_here {
                    commands.spawn((
                        Name::new("Plough bank"),
                        BankPiece,
                        OfPlough(e),
                        Transform::from_translation(zone.on_ground(b.x, b.z)).with_rotation(
                            Quat::from_rotation_y(
                                -extraction_core::sentry::bearing([0.0, 0.0], dir).to_radians(),
                            ),
                        ),
                        Visibility::default(),
                    ));
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
        (
            Without<crate::interact::Held>,
            Without<crate::interact::InSocket>,
            Without<Buried>,
        ),
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

/// Wrecked (its cell shot through): it stops where it is; its cell is
/// done, but its fuel and its auger's scrap lie by it.
fn plough_wreck(
    mut commands: Commands,
    ploughs: Query<
        (Entity, &Robot, &GlobalTransform),
        (With<Plough>, Without<super::wreck::Broken>),
    >,
    catalog: Res<crate::items::Catalog>,
) {
    for (e, robot, g) in &ploughs {
        if !robot.wrecked {
            continue;
        }
        commands
            .entity(e)
            .insert((super::wreck::Broken, super::Salvaged));
        info!("plough: wrecked at {:.0}", g.translation());
        for (k, id) in ["fuel_can", "scrap_metal", "scrap_metal", "servo"]
            .into_iter()
            .enumerate()
        {
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
        let mut vr = crate::vr_harness::Vr::new(&crate::LaunchOptions::default().map);
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
}
