//! Salvage by hand: a wreck is taken apart, not looted from a list. What
//! came through is still on it, a real item seated where it was (Heavy's
//! seats are the model: `heavy::Fastened`), and each comes off its own
//! way:
//!
//! - a **gun pod** hangs on its cables: cut them (bolt cutters,
//!   Destruction's `Cut`) and it's free;
//! - a **servo pack** at a hip whose leg came through is bolted on:
//!   undo its [`BOLTS`] bolts with a spanner (Destruction's `Turned`,
//!   `interact::tools::spanner`) and it lifts out;
//! - an **armour plate** is pried off with a crowbar (the crowbar's own
//!   plate seam, `interact::tools::crowbar`) and comes away as an item
//!   (`armour::shed_plates`);
//! - a **power cell** lifts out by hand, but not while the wreck is fresh:
//!   for [`HOT_FOR`] seconds it's live, and a hand that closes on it is
//!   thrown off by an arc.
//!
//! What a part is worth comes from what it took: its condition is its
//! health when the robot died ([`condition`]); a part shot through isn't
//! that part any more, it's scrap metal, about one for every
//! [`KG_A_SCRAP`] of it ([`scrap_for`]). Where you shot it matters.

use avian3d::prelude::*;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::heavy::Fastened;
use super::walker::WalkerTurret;
use super::{PartRole, Robot, RobotKind, RobotPart};
use crate::destruction::hooks::{Cuttable, Turned, Unboltable};
use crate::interact::{Hand, Socket, SocketKind, seat_in_socket};
use crate::items::{Catalog, SpawnItem, spawn_item};

/// Seconds after a robot dies that its cell is still live.
pub const HOT_FOR: f32 = 60.0;
/// The worst condition a part that came through comes off in.
pub const SCRAP: f32 = 0.05;
/// A part shot through is this much metal a piece of scrap (kg).
pub const KG_A_SCRAP: f32 = 2.0;
/// How near a closed hand has to be to a live cell to be thrown off (m).
const ARC_REACH: f32 = 0.14;
/// A live cell crackles while you're this near it (m), so the arc that
/// throws your hand off isn't the first you know of it.
pub const CRACKLE_HEARD: f32 = 4.0;
/// An open hand this near a live cell (m) draws a little spark to it, and
/// a buzz: it's live, before you take hold.
pub const WARN_REACH: f32 = 0.35;
/// Cables holding a gun pod on.
pub const CABLES: usize = 2;
/// How thick a cable is to the cutters' jaws (m): a hand needn't find a
/// 12 mm wire to the millimetre.
const CABLE_REACH: f32 = 0.035;
/// Bolts holding a servo pack on, and how far each turns to come out
/// (radians: a turn).
pub const BOLTS: usize = 3;
pub const BOLT_TURN: f32 = std::f32::consts::TAU;
/// Bolts holding the Sorter's servo pack on (a bigger mount).
pub const SORTER_BOLTS: usize = 4;
/// The hips a walker's wreck rolls up bare (`wreck::servo_hips`).
pub const MOST_SERVOS: usize = 2;

pub struct SalvagePlugin;

impl Plugin for SalvagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Arced>()
            .add_message::<Crackled>()
            .add_message::<crate::anomaly::cook::PartCooked>()
            .add_systems(
                Update,
                (
                    fit_salvage,
                    follow_wrecks,
                    bolts_turned,
                    holds_gone,
                    cool_cells,
                    live_cells,
                    live_tells,
                    tag_pieces,
                    seat_pieces,
                    cooked_salvage,
                    dirigible_salvage,
                    char_looks,
                )
                    .chain(),
            );
    }
}

/// The condition a part comes off in: its health when the robot died.
pub fn condition(health: f32) -> f32 {
    health.clamp(SCRAP, 1.0)
}

/// Pieces of scrap metal a part this heavy (kg) is, shot through.
pub fn scrap_for(mass: f32) -> usize {
    ((mass / KG_A_SCRAP).round() as usize).max(1)
}

/// Where a wreck's servo pack is (in its space), and the health of the
/// limb it drove when it fell: the Sorter's, at its shoulder
/// (`sorter::wreck_sorters`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ServoAt {
    pub local: Vec3,
    pub health: f32,
}

/// A part's health when its robot died (a plate's, pried off a wreck
/// later: `armour::shed_plates` gives it as an item in this condition).
#[derive(Component, Clone, Copy, Debug)]
pub struct HealthAtDeath(pub f32);

/// A wreck's salvage is fitted (once).
#[derive(Component, Debug)]
pub struct Fitted;

/// What holds a seated part on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hold {
    /// Cables, cut one by one.
    Cables,
    /// Bolts, undone one by one.
    Bolts,
    /// Nothing but the charge in it (a cell).
    Live,
    /// Nothing: it lifts off.
    Hand,
}

/// A seat on a wreck with a part in it.
#[derive(Component, Debug)]
pub struct SalvageSeat {
    pub wreck: Entity,
    pub hold: Hold,
    pub item: Entity,
}

/// One of the cables holding a part on: cut it (`Cuttable`).
#[derive(Component, Debug)]
pub struct Cable {
    pub seat: Entity,
}

/// One of the bolts holding a part on: undo it (`Unboltable`).
#[derive(Component, Debug)]
pub struct Bolt {
    pub seat: Entity,
}

/// A cable or a bolt: its own collider, carried with the wreck it's on
/// ([`follow_wrecks`]). A settled wreck's child colliders aren't kept
/// where it is, and the tools' queries would look for them where they
/// were.
#[derive(Component, Debug)]
pub struct OnWreck {
    pub wreck: Entity,
    pub local: Transform,
}

/// A cell still live: it arcs at a hand until `left` runs out.
#[derive(Component, Debug)]
pub struct Live {
    pub left: f32,
    /// To the next arc it may throw.
    pub next: f32,
    /// To its next crackle (`live_tells`).
    pub tick: f32,
}

/// A live cell crackled, or drew a spark to a hand near it (`to_hand`):
/// a small spark and a tick, the warning before an [`Arced`].
#[derive(Message, Clone, Copy, Debug)]
pub struct Crackled {
    pub cell: Entity,
    pub at: Vec3,
    pub to_hand: Option<Vec3>,
}

/// A hand thrown off a live cell.
#[derive(Message, Clone, Copy, Debug)]
pub struct Arced {
    pub cell: Entity,
    pub at: Vec3,
}

/// Where a walker's cell sits in its orb (local, before its size).
pub const CELL_AT: Vec3 = Vec3::new(0.0, 0.3, 0.12);
/// Where a hound's cell rides, half sunk in its back.
pub const HOUND_CELL_AT: Vec3 = Vec3::new(0.0, 0.14, 0.08);
/// A grappler's launcher (its item's middle) from its boom's head pin.
pub const LAUNCHER_FROM_HEAD: Vec3 = Vec3::new(0.0, 0.0, -0.23);
/// The Sorter's head's middle from its part's origin (`art/robots/sorter.py`
/// prints it), and the bolts through its stalk.
pub const SORTER_HEAD_MID: Vec3 = Vec3::new(0.0, 0.085, -0.01);
pub const SORTER_HEAD_BOLTS: usize = 2;
/// The Sorter's carriage motor's middle (on the carriage's top), its bolts,
/// and its state (the carriage shot through; its motor on top, worn), and
/// the key its look is hidden by (`sorter_look`'s `carriage_motor` holder).
pub const SORTER_MOTOR_AT: Vec3 = Vec3::new(-0.33, 0.464, 0.0);
pub const SORTER_MOTOR_BOLTS: usize = 4;
pub const SORTER_MOTOR_HEALTH: f32 = 0.7;
pub const SORTER_MOTOR_LOOK: PartRole = PartRole::Rotor(0);
/// Bolts through a launcher's flange onto its boom.
pub const LAUNCHER_BOLTS: usize = 4;

/// Seats an item on a wreck (held back from hands until it's freed).
fn seat(
    commands: &mut Commands,
    catalog: &Catalog,
    wreck: Entity,
    id: &str,
    at: Transform,
    size: f32,
    health: f32,
    hold: Hold,
) -> Option<(Entity, Entity)> {
    let kind = catalog.kind(id)?;
    // Shot through: scrap, lying where it was.
    if health <= 0.0 {
        let scrap = catalog.kind("scrap_metal")?;
        commands.queue(move |world: &mut World| {
            let Some(base) = world.get::<GlobalTransform>(wreck).map(|g| *g) else {
                return;
            };
            let Some(catalog) = world.get_resource::<Catalog>().cloned() else {
                return;
            };
            let n = scrap_for(catalog.get(kind).mass);
            let at = base.transform_point(at.translation) + Vec3::Y * 0.2;
            let mut q = bevy::ecs::system::SystemState::<Commands>::new(world);
            let mut c = q.get_mut(world).unwrap();
            for k in 0..n {
                let a = k as f32 * 2.1;
                spawn_item(
                    &mut c,
                    &catalog,
                    SpawnItem {
                        kind: scrap,
                        transform: Transform::from_translation(
                            at + Vec3::new(a.cos() * 0.15, 0.08 * k as f32, a.sin() * 0.15),
                        ),
                        condition: 1.0,
                    },
                );
            }
            q.apply(world);
        });
        return None;
    }
    let item = spawn_item(
        commands,
        catalog,
        SpawnItem {
            kind,
            transform: Transform::default(),
            condition: condition(health),
        },
    );
    let seat = commands
        .spawn((
            Name::new(format!("Salvage seat {id}")),
            at,
            Visibility::default(),
            ChildOf(wreck),
        ))
        .id();
    commands
        .entity(seat)
        .insert(SalvageSeat { wreck, hold, item });
    commands.queue(move |world: &mut World| {
        let mut socket = Socket {
            // (A mount nothing else fits: nothing seats back in it.)
            kind: SocketKind::MagWell("salvage".into()),
            occupant: None,
            radius: 0.0,
            // Items are world-sized; the orb may be scaled.
            seat: Transform::from_scale(Vec3::splat(1.0 / size.max(0.01))),
        };
        let mut q = bevy::ecs::system::SystemState::<Commands>::new(world);
        let mut c = q.get_mut(world).unwrap();
        seat_in_socket(&mut c, item, seat, &mut socket);
        q.apply(world);
        if let Ok(mut e) = world.get_entity_mut(item) {
            if let Some(g) = e.take::<crate::interact::Grabbable>() {
                e.insert(Fastened(g));
            }
        }
        if let Ok(mut e) = world.get_entity_mut(seat) {
            e.insert(socket);
        }
    });
    Some((seat, item))
}

/// Gives a seated item back to the hands.
fn free(commands: &mut Commands, item: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(mut e) = world.get_entity_mut(item) {
            if let Some(f) = e.take::<Fastened>() {
                e.insert(f.0);
            }
        }
    });
}

/// What holds a part on a wreck, as a row of a machine's salvage table
/// says (where, in the wreck's space).
#[derive(Clone, Copy, Debug)]
pub enum Fix {
    /// Nothing: it lifts off (unless it's hot).
    Hand,
    /// `n` bolts in a ring of `radius` round `centre`, their heads along
    /// `face`'s +Y.
    Bolts {
        n: usize,
        centre: Vec3,
        face: Quat,
        radius: f32,
    },
    /// `n` cables from the hull up to it.
    Cables { n: usize },
}

/// One part a wreck gives up: a row of its machine's salvage table
/// (#105: every visible part comes off its own way).
#[derive(Clone, Copy, Debug)]
pub struct Row {
    /// The item it comes off as.
    pub item: &'static str,
    /// Where it sits on the wreck (its space).
    pub at: Transform,
    /// The machine's size (items are world-sized; its body may be scaled).
    pub size: f32,
    /// How whole it was when the machine died (0: shot through, scrap).
    pub health: f32,
    pub fix: Fix,
    /// Live a while after it died: it arcs at a hand ([`HOT_FOR`]).
    pub hot: bool,
    /// The machine's own piece it takes the place of (despawned).
    pub replaces: Option<Entity>,
}

/// What a wreck's parts are, as [`fit_salvage`] reads them.
pub struct WreckParts<'a> {
    pub kind: RobotKind,
    pub size: f32,
    pub shape: super::anim::Shape,
    pub legs: [f32; 4],
    pub servo: Option<ServoAt>,
    pub crawler: bool,
    pub hound: bool,
    pub heavy: bool,
    pub grappler: bool,
    pub mender: bool,
    /// Its parts still on it: (entity, part, where, a walker's turret).
    pub parts: &'a [(Entity, PartRole, f32, Transform, bool)],
}

/// A machine's salvage table: every part its wreck gives up, and how each
/// comes off.
pub fn rows(w: &WreckParts) -> Vec<Row> {
    let mut out = Vec::new();
    // The Sorter's servo pack, bolted on at its shoulder.
    if let Some(sv) = w.servo {
        out.push(Row {
            item: "sorter_servo_pack",
            at: Transform::from_translation(sv.local),
            size: 1.0,
            health: sv.health,
            fix: Fix::Bolts {
                n: SORTER_BOLTS,
                centre: sv.local + Vec3::Y * 0.2,
                face: Quat::IDENTITY,
                radius: 0.15,
            },
            hot: false,
            replaces: None,
        });
    }
    // The Sorter's scanning head, bolted under its carriage's front by its
    // stalk: it comes off in its place (its own look goes with its part).
    if w.kind == RobotKind::Sorter {
        if let Some(&(e, _, health, at, _)) = w.parts.iter().find(|p| p.1 == PartRole::Sensor) {
            let mid = at.rotation * SORTER_HEAD_MID;
            out.push(Row {
                item: "sorter_head",
                at: Transform::from_translation(at.translation + mid).with_rotation(at.rotation),
                size: 1.0,
                health,
                fix: Fix::Bolts {
                    n: SORTER_HEAD_BOLTS,
                    centre: at.translation + at.rotation * Vec3::Y * 0.2,
                    face: at.rotation,
                    radius: 0.04,
                },
                hot: false,
                replaces: (e != Entity::PLACEHOLDER).then_some(e),
            });
        }
        // Its travel drive, bolted on the carriage's top.
        out.push(Row {
            item: "sorter_carriage_motor",
            at: Transform::from_translation(SORTER_MOTOR_AT),
            size: 1.0,
            health: SORTER_MOTOR_HEALTH,
            fix: Fix::Bolts {
                n: SORTER_MOTOR_BOLTS,
                centre: SORTER_MOTOR_AT - Vec3::Y * 0.09,
                face: Quat::IDENTITY,
                radius: 0.25,
            },
            hot: false,
            replaces: None,
        });
        return out;
    }
    if w.kind == RobotKind::Drone {
        drone_rows(w, &mut out);
        return out;
    }
    // (The heavy is a walker too, but its salvage is its own seats,
    // `heavy`: a walker's cell fitted in it was sealed in its hull.)
    if w.kind != RobotKind::Walker || w.heavy {
        return out;
    }
    // The cell: the core's health is the cell's (a core shot through
    // cooked it).
    let core = w
        .parts
        .iter()
        .find(|p| p.1 == PartRole::Core)
        .map_or(0.0, |p| p.2);
    // A hound's cell rides in its back, lifted out by hand.
    if w.hound {
        out.push(Row {
            item: "robot_cell",
            at: Transform::from_translation(HOUND_CELL_AT),
            size: w.size,
            health: core,
            fix: Fix::Hand,
            hot: true,
            replaces: None,
        });
        return out;
    }
    // A crawler's capacitor bank is its spark gun, on two leads.
    if w.crawler {
        for &(e, role, health, at, _) in w.parts {
            if role == PartRole::Weapon {
                out.push(Row {
                    item: "crawler_capacitor",
                    at,
                    size: w.size,
                    health,
                    fix: Fix::Cables { n: CABLES },
                    hot: true,
                    replaces: Some(e),
                });
            }
        }
        return out;
    }
    out.push(Row {
        item: "robot_cell",
        at: Transform::from_translation(CELL_AT),
        size: w.size,
        health: core,
        fix: Fix::Hand,
        hot: true,
        replaces: None,
    });
    // (Its knees are on its legs where they fell: `piece_part`.)
    // The gun pod, where the turret was, on its cables.
    for &(e, role, health, at, turret) in w.parts {
        if turret && role == PartRole::Weapon {
            out.push(Row {
                item: "robot_gunpod",
                at,
                size: w.size,
                health,
                fix: Fix::Cables { n: CABLES },
                hot: false,
                replaces: Some(e),
            });
        }
    }
    // A grappler's launcher, bolted on its boom's head where the boom lay
    // down (`grappler_armour::folded`), and its line drum, held by the
    // line still run up the boom. Both are as sound as the eye that aimed
    // them: they've no hit volume of their own.
    if w.grappler {
        let eye = w
            .parts
            .iter()
            .find(|p| p.1 == PartRole::Sensor)
            .map_or(core, |p| p.2);
        let head = super::grappler_armour::folded(
            Transform::from_translation(super::grappler_armour::BOOM_HEAD + LAUNCHER_FROM_HEAD),
            1.0,
        );
        out.push(Row {
            item: "grappler_launcher",
            at: head,
            size: w.size,
            health: eye,
            fix: Fix::Bolts {
                n: LAUNCHER_BOLTS,
                centre: head.transform_point(-LAUNCHER_FROM_HEAD + Vec3::Y * -0.1),
                face: head.rotation * Quat::from_rotation_x(std::f32::consts::PI),
                radius: 0.06,
            },
            hot: false,
            replaces: None,
        });
        out.push(Row {
            item: "grappler_drum",
            at: Transform::from_translation(super::grappler_armour::DRUM_AT),
            size: w.size,
            health: core.max(eye),
            fix: Fix::Cables { n: 1 },
            hot: false,
            replaces: None,
        });
    }
    out
}

/// A wreck, settled a moment, gets what came through seated on it: its
/// machine's salvage table ([`rows`]), each part held its own way.
#[allow(clippy::type_complexity)]
fn fit_salvage(
    mut commands: Commands,
    wrecks: Query<
        (
            Entity,
            &Robot,
            &Children,
            Option<&super::anim::Gait>,
            Option<&super::wreck::WreckLegs>,
            Option<&ServoAt>,
            Has<super::indoor::Crawler>,
            Has<super::hound::Hound>,
            Has<super::heavy::Heavy>,
            Has<super::grappler::Grappler>,
            Has<super::mender::Mender>,
        ),
        (
            With<super::wreck::Broken>,
            Without<Fitted>,
            With<super::Salvaged>,
        ),
    >,
    parts: Query<(&RobotPart, &Transform, Has<WalkerTurret>)>,
    mut harpoon: Query<
        (&crate::render::logistics::HarpoonPart, &mut Visibility),
        Without<SeatLook>,
    >,
    mut seat_looks: Query<(&SeatLook, &mut Visibility)>,
    all_parts: Query<&RobotPart>,
    catalog: Option<Res<Catalog>>,
    // (Not in a headless app: no cable to see there.)
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    for (e, robot, children, gait, legs, servo, crawler, hound, heavy, grappler, mender) in &wrecks
    {
        commands.entity(e).insert(Fitted);
        // Its plates, for prying off (`armour::shed_plates`).
        for c in children.iter() {
            if let Ok((part, _, _)) = parts.get(c) {
                if matches!(part.role, PartRole::Armour(_)) {
                    commands
                        .entity(c)
                        .insert(HealthAtDeath(part.hp / part.max_hp.max(1.0)));
                }
            }
        }
        let health = |p: &RobotPart| p.hp / p.max_hp.max(1.0);
        let mut on: Vec<(Entity, PartRole, f32, Transform, bool)> = children
            .iter()
            .filter_map(|c| {
                parts
                    .get(c)
                    .ok()
                    .map(|(p, t, turret)| (c, p.role, health(p), *t, turret))
            })
            .collect();
        // (A part not a child of the hull, as the walker's core is not.)
        for p in all_parts.iter().filter(|p| p.robot == e) {
            if !on.iter().any(|o| o.1 == p.role) {
                on.push((
                    Entity::PLACEHOLDER,
                    p.role,
                    health(p),
                    Transform::IDENTITY,
                    false,
                ));
            }
        }
        let w = WreckParts {
            kind: robot.kind,
            size: gait.map_or(1.0, |g| g.size),
            shape: gait.map_or(super::anim::Shape::Orb, |g| g.shape),
            legs: legs.map(|l| l.0).unwrap_or_default(),
            servo: servo.copied(),
            crawler,
            hound,
            heavy,
            grappler,
            mender,
            parts: &on,
        };
        let look = Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
        for row in rows(&w) {
            // A grappler's launcher head and drum: the seated part is
            // drawn where its node was, so the node goes.
            use crate::render::logistics::HarpoonPiece;
            let piece = match row.item {
                "grappler_launcher" => Some(HarpoonPiece::LauncherHead),
                "grappler_drum" => Some(HarpoonPiece::Drum),
                _ => None,
            };
            if let Some(piece) = piece {
                for (h, mut vis) in &mut harpoon {
                    if h.grappler == e && h.what == piece {
                        *vis = Visibility::Hidden;
                    }
                }
            }
            // A part's look drawn off the part (`SeatLook`) goes with it.
            let keyed = (row.item == "sorter_carriage_motor").then_some(SORTER_MOTOR_LOOK);
            if let Some(role) = row
                .replaces
                .and_then(|r| on.iter().find(|o| o.0 == r).map(|o| o.1))
                .or(keyed)
            {
                for (l, mut vis) in &mut seat_looks {
                    if l.robot == e && l.role == role {
                        *vis = Visibility::Hidden;
                    }
                }
            }
            fit(&mut commands, &catalog, e, row, &look);
        }
    }
}

/// A downed dirigible's wreck is what was on it (#105), not a table's
/// rolls: wrecked, it's `Salvaged` at once (nothing rolled while it sinks);
/// down, a gas valve bolted on the flank of each cell, its gondola's cell
/// by hand (live), its eye taken off the gondola's front (its part, under
/// the gondola, comes to lie in the ground), and its envelope's fabric and
/// a piece of its frame lying loose.
#[derive(Component)]
struct DirigibleSeated;

#[allow(clippy::type_complexity)]
fn dirigible_salvage(
    mut commands: Commands,
    ships: Query<
        (
            Entity,
            &super::dirigible::Dirigible,
            &super::Robot,
            &Children,
            &GlobalTransform,
            Has<super::Salvaged>,
        ),
        Without<DirigibleSeated>,
    >,
    parts: Query<&RobotPart>,
    catalog: Option<Res<Catalog>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    for (e, d, robot, children, g, salvaged) in &ships {
        if !robot.wrecked {
            continue;
        }
        if !salvaged {
            commands.entity(e).insert(super::Salvaged);
        }
        // Down: it's stopped on the ground.
        if !(d.sinking && d.velocity == Vec3::ZERO) {
            continue;
        }
        commands.entity(e).insert(DirigibleSeated);
        let look = Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
        let mut gondola = None;
        let mut eye = None;
        for c in children.iter() {
            let Ok(p) = parts.get(c) else {
                continue;
            };
            match p.role {
                PartRole::GasCell(_) => {
                    fit(
                        &mut commands,
                        &catalog,
                        c,
                        dirigible_row(p.role, DIRIGIBLE_VALVE_HEALTH, None).unwrap(),
                        &look,
                    );
                }
                PartRole::Core => gondola = Some((c, p.hp / p.max_hp.max(1.0))),
                PartRole::Sensor => eye = Some((c, p.hp / p.max_hp.max(1.0))),
                _ => {}
            }
        }
        if let Some((gc, health)) = gondola {
            if let Some(row) = dirigible_row(PartRole::Core, health, None) {
                fit(&mut commands, &catalog, gc, row, &look);
            }
            if let Some((ec, health)) = eye {
                if let Some(row) = dirigible_row(PartRole::Sensor, health, Some(ec)) {
                    fit(&mut commands, &catalog, gc, row, &look);
                }
            }
        }
        for (k, id) in ["polymer", "polymer", "scrap_metal"]
            .into_iter()
            .enumerate()
        {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let a = k as f32 * 2.1;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        g.translation()
                            .with_y(g.translation().y - super::dirigible::RADIUS)
                            + Vec3::new(a.cos() * 3.5, 0.0, a.sin() * 3.5),
                    ),
                    condition: 0.6,
                },
            );
        }
    }
}

/// A dirigible's gas valves come through its cells being holed (they're
/// on the frame, not the fabric).
pub const DIRIGIBLE_VALVE_HEALTH: f32 = 0.7;
pub const DIRIGIBLE_VALVE_BOLTS: usize = 4;
/// Where things sit on a downed dirigible: a valve low on each cell's
/// flank (local to the cell: its underside is over the gondola), the cell
/// in the gondola's side, the eye on the gondola's nose (local to it).
pub const DIRIGIBLE_VALVE_AT: Vec3 = Vec3::new(1.85, -1.85, 0.0);
pub const DIRIGIBLE_CELL_AT: Vec3 = Vec3::new(0.62, 0.05, 1.0);
pub const DIRIGIBLE_EYE_AT: Vec3 = Vec3::new(0.0, -0.1, -2.08);

/// What a downed dirigible gives up, part by part, and how it's held
/// (`replaces`: the eye part it's taken in place of).
pub fn dirigible_row(role: PartRole, health: f32, replaces: Option<Entity>) -> Option<Row> {
    let row = |item, at: Transform, fix, hot| Row {
        item,
        at,
        size: 1.0,
        health,
        fix,
        hot,
        replaces,
    };
    let out = Vec3::new(1.0, -1.0, 0.0).normalize();
    Some(match role {
        PartRole::GasCell(_) => row(
            "dirigible_gas_valve",
            Transform::from_translation(DIRIGIBLE_VALVE_AT)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, out)),
            Fix::Bolts {
                n: DIRIGIBLE_VALVE_BOLTS,
                centre: DIRIGIBLE_VALVE_AT - out * 0.06,
                face: Quat::from_rotation_arc(Vec3::Y, out),
                radius: 0.13,
            },
            false,
        ),
        PartRole::Core => row(
            "robot_cell",
            Transform::from_translation(DIRIGIBLE_CELL_AT),
            Fix::Hand,
            true,
        ),
        PartRole::Sensor => row(
            "walker_eye",
            Transform::from_translation(DIRIGIBLE_EYE_AT),
            Fix::Hand,
            false,
        ),
        _ => return None,
    })
}

/// The meshes a part's holds are drawn with (none in a headless app).
pub(crate) struct Look {
    bolt: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    cable: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

impl Look {
    pub(crate) fn new(
        meshes: Option<&mut Assets<Mesh>>,
        materials: Option<&mut Assets<StandardMaterial>>,
    ) -> Self {
        let (Some(m), Some(s)) = (meshes, materials) else {
            return Self {
                bolt: None,
                cable: None,
            };
        };
        Self {
            // A bolt's head: dark, oily steel (bright metal reads as a
            // loose die in daylight, not a bolt).
            bolt: Some((
                m.add(Cylinder::new(0.018, 0.016)),
                s.add(StandardMaterial {
                    base_color: Color::srgb(0.12, 0.11, 0.1),
                    metallic: 0.6,
                    perceptual_roughness: 0.55,
                    ..default()
                }),
            )),
            cable: Some((
                m.add(Cylinder::new(0.012, 0.22)),
                s.add(StandardMaterial {
                    base_color: Color::srgb(0.08, 0.08, 0.09),
                    perceptual_roughness: 0.6,
                    ..default()
                }),
            )),
        }
    }
}

/// Seats one row's part on `wreck`, held its way.
pub(crate) fn fit(
    commands: &mut Commands,
    catalog: &Catalog,
    wreck: Entity,
    row: Row,
    look: &Look,
) {
    if let Some(e) = row.replaces {
        if e != Entity::PLACEHOLDER {
            commands.entity(e).despawn();
        }
    }
    let hold = match row.fix {
        Fix::Hand if row.hot => Hold::Live,
        Fix::Hand => Hold::Hand,
        Fix::Bolts { .. } => Hold::Bolts,
        Fix::Cables { .. } => Hold::Cables,
    };
    let Some((seat_e, item)) = seat(
        commands, catalog, wreck, row.item, row.at, row.size, row.health, hold,
    ) else {
        return;
    };
    if row.hot {
        commands.entity(item).insert(Live {
            left: HOT_FOR,
            next: 0.0,
            tick: 0.0,
        });
    }
    match row.fix {
        Fix::Hand => {}
        Fix::Bolts {
            n,
            centre,
            face,
            radius,
        } => bolt_on(commands, wreck, seat_e, centre, face, n, radius, &look.bolt),
        Fix::Cables { n } => cable_on(commands, wreck, seat_e, row.at, n, &look.cable),
    }
}

/// `n` cables from the hull up to a part's mount at `at`, either side.
fn cable_on(
    commands: &mut Commands,
    wreck: Entity,
    seat: Entity,
    at: Transform,
    n: usize,
    look: &Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
) {
    for k in 0..n {
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let mid = at.translation * 0.75 + Vec3::new(0.09 * side, 0.0, 0.05);
        let local = Transform::from_translation(mid).with_rotation(Quat::from_rotation_x(0.5));
        let cable = commands
            .spawn((
                Name::new("Salvage cable"),
                Cable { seat },
                OnWreck { wreck, local },
                Cuttable { needs: 0.0 },
                // (Thicker than it looks, so jaws put to it find it;
                // found as a padlock is, a collider that meets only
                // the world. Its own collider, not one of the
                // wreck's: a settled body's child colliders aren't
                // kept where the body is, and the cutters' query
                // would look for it where it was. `follow_wrecks`
                // carries it.)
                Collider::capsule(CABLE_REACH, 0.2),
                CollisionLayers::new(crate::world::Layer::Item, [crate::world::Layer::World]),
                local,
                Visibility::default(),
            ))
            .id();
        if let Some((mesh, mat)) = look {
            commands.entity(cable).insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                NotShadowCaster,
            ));
        }
    }
}

/// `n` bolts in a ring of `radius` round `centre` (the wreck's space),
/// their heads facing `face`'s +Y, holding the part in `seat` on.
#[allow(clippy::too_many_arguments)]
fn bolt_on(
    commands: &mut Commands,
    wreck: Entity,
    seat: Entity,
    centre: Vec3,
    face: Quat,
    n: usize,
    radius: f32,
    look: &Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
) {
    for k in 0..n {
        let a = k as f32 * std::f32::consts::TAU / n as f32;
        let local = Transform::from_translation(
            centre + face * (Vec3::new(a.cos(), 0.0, a.sin()) * radius),
        )
        .with_rotation(face);
        let bolt = commands
            .spawn((
                Name::new("Salvage bolt"),
                Bolt { seat },
                OnWreck { wreck, local },
                Unboltable {
                    needs: BOLT_TURN,
                    axis: Vec3::Y,
                    turned: 0.0,
                },
                Collider::sphere(0.02),
                CollisionLayers::new(crate::world::Layer::Item, [crate::world::Layer::World]),
                local,
                Visibility::default(),
            ))
            .id();
        if let Some((mesh, mat)) = look {
            commands.entity(bolt).insert((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat.clone()),
                NotShadowCaster,
            ));
        }
    }
}

/// Cables ride their wreck (dragged off, it takes them with it); a
/// wreck gone takes its cables.
fn follow_wrecks(
    mut commands: Commands,
    mut on: Query<(
        Entity,
        &OnWreck,
        &mut Transform,
        &mut Position,
        &mut Rotation,
    )>,
    wrecks: Query<&GlobalTransform>,
) {
    for (e, w, mut t, mut pos, mut rot) in &mut on {
        let Ok(gt) = wrecks.get(w.wreck) else {
            commands.entity(e).despawn();
            continue;
        };
        let at = gt.mul_transform(w.local).compute_transform();
        let at = Transform::from_translation(at.translation).with_rotation(at.rotation);
        if t.translation.distance_squared(at.translation) > 1e-8
            || t.rotation.angle_between(at.rotation) > 1e-4
        {
            *t = at;
            pos.0 = at.translation;
            rot.0 = at.rotation;
        }
    }
}

/// A bolt turned far enough is out.
fn bolts_turned(
    mut commands: Commands,
    mut turns: MessageReader<Turned>,
    mut bolts: Query<&mut Unboltable, With<Bolt>>,
) {
    for t in turns.read() {
        let Ok(mut u) = bolts.get_mut(t.target) else {
            continue;
        };
        u.turned += t.angle;
        if u.turned >= u.needs {
            commands.entity(t.target).try_despawn();
        }
    }
}

/// A part with nothing left holding it on is free: its cables cut
/// (Destruction's `cut` takes a cut thing away), its bolts out.
fn holds_gone(
    mut commands: Commands,
    seats: Query<(Entity, &SalvageSeat)>,
    cables: Query<&Cable>,
    bolts: Query<&Bolt>,
    fastened: Query<(), With<Fastened>>,
) {
    for (seat, s) in &seats {
        if !fastened.contains(s.item) {
            continue;
        }
        let held = match s.hold {
            Hold::Cables => cables.iter().any(|c| c.seat == seat),
            Hold::Bolts => bolts.iter().any(|b| b.seat == seat),
            Hold::Live => continue,
            Hold::Hand => false,
        };
        if !held {
            free(&mut commands, s.item);
        }
    }
}

/// A live cell cools: once it's dead it lifts out.
fn cool_cells(mut commands: Commands, mut cells: Query<(Entity, &mut Live)>, time: Res<Time>) {
    let dt = time.delta_secs();
    for (e, mut live) in &mut cells {
        live.left -= dt;
        live.next -= dt;
        if live.left <= 0.0 {
            commands.entity(e).remove::<Live>();
            free(&mut commands, e);
        }
    }
}

/// A hand closing on a live cell is thrown off by an arc.
fn live_cells(
    mut cells: Query<(Entity, &mut Live, &GlobalTransform)>,
    hands: Query<(&Hand, &GlobalTransform, &crate::rig::RigHand)>,
    mut arcs: MessageWriter<Arced>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
) {
    for (cell, mut live, gt) in &mut cells {
        if live.next > 0.0 {
            continue;
        }
        let at = gt.translation();
        for (hand, hgt, rig) in &hands {
            if !hand.grip_down || hgt.translation().distance(at) > ARC_REACH {
                continue;
            }
            arcs.write(Arced { cell, at });
            haptics.write(crate::rig::haptics::Haptic {
                side: rig.side,
                amplitude: 0.9,
                seconds: 0.12,
            });
            live.next = 0.5;
        }
    }
}

/// A drone's (or a mender's) wreck: each rotor still on its strut, twisted
/// off by hand; its eye; its cell, hot, where the top panel came off. (Its
/// gun pod or torch arm and a rotor or so fly off as it crashes: pieces,
/// [`piece_part`]. Its belly plate pries off: `armour`.)
fn drone_rows(w: &WreckParts, out: &mut Vec<Row>) {
    let core = w
        .parts
        .iter()
        .find(|p| p.1 == PartRole::Core)
        .map_or(0.0, |p| p.2);
    out.push(Row {
        item: "robot_cell",
        at: Transform::from_translation(if w.mender {
            MENDER_CELL_AT
        } else {
            DRONE_CELL_AT
        }),
        size: w.size,
        health: core,
        fix: Fix::Hand,
        hot: true,
        replaces: None,
    });
    for &(e, role, health, at, _) in w.parts {
        let item = match role {
            PartRole::Rotor(_) => "drone_rotor_motor",
            PartRole::Sensor => "drone_eye",
            _ => continue,
        };
        out.push(Row {
            item,
            at,
            size: w.size,
            health,
            fix: Fix::Hand,
            hot: false,
            replaces: Some(e),
        });
    }
}

/// Where a drone's cell sits: half out of its belly, which a crash turns
/// to the sky (`wreck`'s drone crash flips it onto its back, belly plate
/// up): seen from standing, in reach, its hot band showing.
pub const DRONE_CELL_AT: Vec3 = Vec3::new(0.0, -0.11, 0.06);
/// And a mender's, on its back behind the spine.
pub const MENDER_CELL_AT: Vec3 = Vec3::new(0.0, 0.16, -0.06);

/// A look drawn on a machine for one of its parts but not under the part
/// itself (a mender's rotors hang on its root): hidden when that part's
/// salvage is seated in its place, as a part's own look goes with it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct SeatLook {
    pub robot: Entity,
    pub role: PartRole,
}

/// What a fallen piece of a walker-family wreck is worth taking, and how
/// it's held: a leg's knee (a walker's ram, a crawler's servo, a hound's
/// hip motor), bolted on; the visor's eye, or a hound's head, by hand.
/// (The heavy's are its own seats; a piece off anything else isn't one.)
pub fn piece_part(
    kind: RobotKind,
    piece: super::wreck::Piece,
    crawler: bool,
    hound: bool,
    heavy: bool,
    mender: bool,
    health: f32,
) -> Option<super::wreck::SalvagePart> {
    use super::wreck::{Piece, SalvagePart};
    let by_hand = |item| SalvagePart {
        item,
        bolts: 0,
        cables: 0,
        pry: false,
        hot: false,
        health,
    };
    // A drone's: a rotor thrown off, its gun pod (a mender's torch arm)
    // torn off as it crashed, each lying where it fell.
    if kind == RobotKind::Drone {
        return match piece {
            Piece::Rotor => Some(by_hand("drone_rotor_motor")),
            Piece::Gun if mender => Some(by_hand("mender_torch")),
            Piece::Gun => Some(by_hand("drone_gunpod")),
            _ => None,
        };
    }
    if kind != RobotKind::Walker || heavy {
        return None;
    }
    let (item, bolts) = match piece {
        Piece::LegUpper if hound => ("hound_hip_servo", 2),
        Piece::LegUpper if crawler => ("crawler_leg_servo", 2),
        Piece::LegUpper => ("walker_leg_actuator", BOLTS as u8),
        Piece::Sensor if hound => ("hound_head", 0),
        Piece::Sensor => ("walker_eye", 0),
        _ => return None,
    };
    Some(SalvagePart {
        item,
        bolts,
        cables: 0,
        pry: false,
        hot: false,
        health,
    })
}

/// Each new piece off a wreck that's a part ([`piece_part`]) is marked so:
/// it's kept from the debris cap, and seated once it's down.
#[allow(clippy::type_complexity)]
fn tag_pieces(
    mut commands: Commands,
    new: Query<
        (
            Entity,
            &super::wreck::Debris,
            Option<&super::wreck::PieceOf>,
            Option<&super::wreck::PieceHealth>,
        ),
        (
            Added<super::wreck::Debris>,
            Without<super::wreck::SalvagePart>,
        ),
    >,
    roots: Query<(
        Has<super::indoor::Crawler>,
        Has<super::hound::Hound>,
        Has<super::heavy::Heavy>,
        Has<super::mender::Mender>,
    )>,
) {
    for (e, d, of, health) in &new {
        // (A root already gone: no telling what it was, so nothing.)
        let Some(Ok((crawler, hound, heavy, mender))) = of.map(|o| roots.get(o.0)) else {
            continue;
        };
        let health = health.map_or(1.0, |h| h.0);
        if let Some(part) = piece_part(d.kind, d.piece, crawler, hound, heavy, mender, health) {
            commands.entity(e).insert(part);
        }
    }
}

/// How far a knee sits proud of its leg piece's axis (local, m).
const KNEE_PROUD: f32 = 0.1;

/// Of a lying leg piece's sides (its local ±X, ±Z: across it, its length
/// is Y), the one most nearly up, given how it lies.
pub fn up_side(rotation: Quat) -> Vec3 {
    [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
        .into_iter()
        .max_by(|a, b| (rotation * *a).y.total_cmp(&(rotation * *b).y))
        .unwrap_or(Vec3::X)
}

/// A part piece come to rest is seated where it lies: bolted on its leg (a
/// knee), or, held by nothing but a hand (an eye, a head), simply the item
/// lying there, the piece gone.
#[allow(clippy::type_complexity)]
fn seat_pieces(
    mut commands: Commands,
    settled: Query<(Entity, &super::wreck::SalvagePart, &Transform), Added<super::wreck::Settled>>,
    catalog: Option<Res<Catalog>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    if settled.is_empty() {
        return;
    }
    let look = Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
    for (e, part, t) in &settled {
        let size = t.scale.x.max(0.01);
        let by_hand = part.bolts == 0 && part.cables == 0 && !part.pry && !part.hot;
        if by_hand && part.health > 0.0 {
            let Some(kind) = catalog.kind(part.item) else {
                continue;
            };
            spawn_item(
                &mut commands,
                &catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(t.translation + Vec3::Y * 0.05)
                        .with_rotation(t.rotation),
                    condition: condition(part.health),
                },
            );
            commands.entity(e).despawn();
            continue;
        }
        // On the side of the leg that's uppermost as it lies, along it (the
        // item's long side, Z, up the piece's Y), its bolts' heads to the
        // sky: seen from standing, a spanner put to them from above.
        let up = up_side(t.rotation);
        // (Its long side, Z, along the leg; its top, Y, up.)
        let lie = Quat::from_mat3(&Mat3::from_cols(up.cross(Vec3::Y), up, Vec3::Y));
        let at = Transform::from_translation(up * KNEE_PROUD * size).with_rotation(lie);
        let fix = if by_hand {
            Fix::Hand
        } else {
            Fix::Bolts {
                n: part.bolts as usize,
                centre: (up * (KNEE_PROUD + 0.05) + Vec3::Y * 0.18) * size,
                face: Quat::from_rotation_arc(Vec3::Y, up),
                radius: 0.05,
            }
        };
        fit(
            &mut commands,
            &catalog,
            e,
            Row {
                item: part.item,
                at,
                size,
                health: part.health,
                fix,
                hot: part.hot,
                replaces: None,
            },
            &look,
        );
    }
}

/// A part cooked through in a burner's coals (`anomaly::cook`), for good:
/// it's dark with char, in the hand as on the wreck ([`char_looks`]).
#[derive(Component, Debug, Clone, Copy)]
pub struct Charred;

/// What cooking does to salvage has been done to it, once ([`cooked_salvage`];
/// [`Charred`] alone is only the look, which anything cooked may get).
#[derive(Component, Debug, Clone, Copy)]
struct SalvageCooked;

/// A [`Charred`] item's look is charred.
#[derive(Component, Debug, Clone, Copy)]
struct CharLooked;

/// A cooked plate pries with this much of the force (its seam's burnt).
pub const COOKED_PRY: f32 = 1.0 / 3.0;
/// A cooked part comes off at this much of its condition.
pub const COOKED_CONDITION: f32 = 0.5;

/// What a burner's coals do to salvage (`anomaly::cook::PartCooked`): a
/// part's holds burn through (it lifts off by hand), it's heat-struck (half
/// its condition), a live cell is spent (no arc, no charge), a plate's seam
/// gives at a third of the force; a cooked wreck cooks all that's on it.
/// A fast, lossy way to strip a wreck.
#[allow(clippy::type_complexity)]
fn cooked_salvage(
    mut commands: Commands,
    mut cooked: MessageReader<crate::anomaly::cook::PartCooked>,
    wrecks: Query<(), (With<Robot>, With<super::wreck::Broken>)>,
    seats: Query<(Entity, &SalvageSeat)>,
    holds: Query<(Entity, Option<&Bolt>, Option<&Cable>)>,
    mut items: Query<&mut crate::items::Item>,
    mut plates: Query<(
        Entity,
        &RobotPart,
        &mut HealthAtDeath,
        Option<&mut crate::destruction::hooks::Pryable>,
    )>,
    done: Query<(), With<SalvageCooked>>,
    catalog: Option<Res<Catalog>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    for ev in cooked.read() {
        let e = ev.entity;
        if wrecks.contains(e) {
            // All that's on it.
            let on: Vec<Entity> = seats
                .iter()
                .filter(|(_, s)| s.wreck == e)
                .map(|(_, s)| s.item)
                .collect();
            for item in on {
                cook_item(&mut commands, item, &done, &mut items, &seats, &holds);
            }
            let mine: Vec<Entity> = plates
                .iter()
                .filter(|(_, part, ..)| part.robot == e)
                .map(|(p, ..)| p)
                .collect();
            for p in mine {
                cook_plate(&mut commands, p, &done, &mut plates);
            }
        } else if plates.contains(e) {
            cook_plate(&mut commands, e, &done, &mut plates);
        } else if items.get(e).is_ok_and(|it| {
            catalog.get(it.kind()).category == extraction_core::items::Category::RobotPart
        }) {
            // (A loose part; anything else a burner cooks isn't salvage.)
            cook_item(&mut commands, e, &done, &mut items, &seats, &holds);
        }
    }
}

/// A part cooked: charred, heat-struck, spent, its holds burnt through.
#[allow(clippy::type_complexity)]
fn cook_item(
    commands: &mut Commands,
    item: Entity,
    done: &Query<(), With<SalvageCooked>>,
    items: &mut Query<&mut crate::items::Item>,
    seats: &Query<(Entity, &SalvageSeat)>,
    holds: &Query<(Entity, Option<&Bolt>, Option<&Cable>)>,
) {
    if done.contains(item) {
        return;
    }
    commands.entity(item).try_insert((Charred, SalvageCooked));
    if let Ok(mut it) = items.get_mut(item) {
        it.condition *= COOKED_CONDITION;
        it.charge_wh = 0.0;
    }
    // Spent: no arc.
    commands.entity(item).remove::<Live>();
    for (seat, s) in seats {
        if s.item != item {
            continue;
        }
        for (h, bolt, cable) in holds {
            if bolt.is_some_and(|b| b.seat == seat) || cable.is_some_and(|c| c.seat == seat) {
                commands.entity(h).despawn();
            }
        }
        free(commands, item);
    }
}

/// A plate cooked: charred, heat-struck, its seam giving at a third.
#[allow(clippy::type_complexity)]
fn cook_plate(
    commands: &mut Commands,
    plate: Entity,
    done: &Query<(), With<SalvageCooked>>,
    plates: &mut Query<(
        Entity,
        &RobotPart,
        &mut HealthAtDeath,
        Option<&mut crate::destruction::hooks::Pryable>,
    )>,
) {
    if done.contains(plate) {
        return;
    }
    let Ok((p, _, mut h, seam)) = plates.get_mut(plate) else {
        return;
    };
    commands.entity(p).try_insert((Charred, SalvageCooked));
    h.0 *= COOKED_CONDITION;
    if let Some(mut seam) = seam {
        seam.needs *= COOKED_PRY;
    }
}

/// A charred thing looks it: every mesh under it darkened to char, dull,
/// its glow gone (a spent cell's band is dark). One char material per
/// material it had.
fn char_looks(
    mut commands: Commands,
    charred: Query<Entity, (With<Charred>, Without<CharLooked>)>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    mut chars: Local<
        std::collections::HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
    >,
) {
    let Some(mut materials) = materials else {
        return;
    };
    for e in &charred {
        let mut any = false;
        for d in std::iter::once(e).chain(children.iter_descendants(e)) {
            let Ok(mut m) = meshes.get_mut(d) else {
                continue;
            };
            any = true;
            let id = m.0.id();
            if chars.values().any(|h| h.id() == id) {
                continue;
            }
            let ch = match chars.get(&id) {
                Some(h) => h.clone(),
                None => {
                    let Some(src) = materials.get(id).cloned() else {
                        continue;
                    };
                    let h = materials.add(char_of(&src));
                    chars.insert(id, h.clone());
                    h
                }
            };
            m.0 = ch;
        }
        // (Its model may not be in yet: again next frame.)
        if any {
            commands.entity(e).try_insert(CharLooked);
        }
    }
}

/// A material, charred: most of its colour burnt to soot, dull, no glow.
pub fn char_of(src: &StandardMaterial) -> StandardMaterial {
    let c = src.base_color.to_linear();
    let soot = LinearRgba::new(0.018, 0.016, 0.015, c.alpha);
    let k = 0.22;
    StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::new(
            c.red * k + soot.red,
            c.green * k + soot.green,
            c.blue * k + soot.blue,
            c.alpha,
        )),
        perceptual_roughness: 0.95,
        metallic: src.metallic * 0.3,
        emissive: LinearRgba::BLACK,
        ..src.clone()
    }
}

/// A live cell tells you it's live before you touch it: it crackles now
/// and then while you're near (more often as a hand comes close), and
/// draws a spark to an open hand within [`WARN_REACH`], with a buzz.
fn live_tells(
    mut cells: Query<(Entity, &mut Live, &GlobalTransform)>,
    hands: Query<(&Hand, &GlobalTransform, &crate::rig::RigHand)>,
    heads: Query<&GlobalTransform, With<crate::player::Player>>,
    mut tells: MessageWriter<Crackled>,
    mut haptics: MessageWriter<crate::rig::haptics::Haptic>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (cell, mut live, gt) in &mut cells {
        live.tick -= dt;
        if live.tick > 0.0 {
            continue;
        }
        let at = gt.translation();
        if !heads
            .iter()
            .any(|h| h.translation().distance(at) < CRACKLE_HEARD)
        {
            continue;
        }
        let near = hands
            .iter()
            .filter(|(h, hgt, _)| !h.grip_down && hgt.translation().distance(at) < WARN_REACH)
            .min_by(|a, b| {
                a.1.translation()
                    .distance(at)
                    .total_cmp(&b.1.translation().distance(at))
            });
        match near {
            Some((_, hgt, rig)) => {
                tells.write(Crackled {
                    cell,
                    at,
                    to_hand: Some(hgt.translation()),
                });
                haptics.write(crate::rig::haptics::Haptic {
                    side: rig.side,
                    amplitude: 0.25,
                    seconds: 0.05,
                });
                live.tick = 0.3;
            }
            None => {
                tells.write(Crackled {
                    cell,
                    at,
                    to_hand: None,
                });
                // (Irregular, as a fault is: 0.8 to 1.6 s.)
                live.tick = 0.8 + 0.8 * ((live.left * 7.31).sin() * 0.5 + 0.5);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A part comes off in the condition it was in; shot through, it's
    /// scrap metal by its weight (Progression's rate: a piece per 2 kg, at
    /// least one).
    /// A walker's table: its cell (hot, by hand), knee actuators at its
    /// soundest hips (bolted) and its gun pod in the turret's place (on
    /// cables); a crawler, a hound and the heavy have their own; the Sorter
    /// its servo pack.
    #[test]
    fn each_machine_has_its_salvage_table() {
        let turret = Entity::from_raw_u32(7).unwrap();
        let parts = [
            (
                Entity::PLACEHOLDER,
                PartRole::Core,
                0.8,
                Transform::IDENTITY,
                false,
            ),
            (
                turret,
                PartRole::Weapon,
                0.5,
                Transform::from_xyz(0.0, 0.6, 0.0),
                true,
            ),
        ];
        let walker = WreckParts {
            kind: RobotKind::Walker,
            size: 1.0,
            shape: super::super::anim::Shape::Orb,
            legs: [1.0, 0.9, 0.0, 0.7],
            servo: None,
            crawler: false,
            hound: false,
            heavy: false,
            grappler: false,
            mender: false,
            parts: &parts,
        };
        let r = rows(&walker);
        let ids: Vec<_> = r.iter().map(|r| r.item).collect();
        assert_eq!(
            ids.iter().filter(|i| **i == "robot_cell").count(),
            1,
            "{ids:?}"
        );
        assert!(
            !ids.contains(&"walker_leg_actuator"),
            "its knees are on its fallen legs: {ids:?}"
        );
        let cell = r.iter().find(|r| r.item == "robot_cell").unwrap();
        assert!(cell.hot && matches!(cell.fix, Fix::Hand));
        assert_eq!(cell.health, 0.8, "the core's health is the cell's");
        let pod = r.iter().find(|r| r.item == "robot_gunpod").unwrap();
        assert!(matches!(pod.fix, Fix::Cables { n: CABLES }));
        assert_eq!(pod.replaces, Some(turret));
        // The heavy's are its own seats (`heavy`).
        assert!(
            rows(&WreckParts {
                heavy: true,
                ..walker_like(&parts)
            })
            .is_empty()
        );
        // A hound: its cell, hot, by hand.
        let r = rows(&WreckParts {
            hound: true,
            ..walker_like(&parts)
        });
        assert_eq!(r.len(), 1);
        assert!(r[0].item == "robot_cell" && r[0].hot && matches!(r[0].fix, Fix::Hand));
        // A crawler: its capacitor bank in its gun's place, hot, on leads.
        let r = rows(&WreckParts {
            crawler: true,
            ..walker_like(&parts)
        });
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].item, "crawler_capacitor");
        assert!(r[0].hot && matches!(r[0].fix, Fix::Cables { n: CABLES }));
        assert_eq!(r[0].replaces, Some(turret));
        // A grappler: a walker's, and its launcher (bolted, lying back
        // with the boom) and line drum (on its line).
        let r = rows(&WreckParts {
            grappler: true,
            ..walker_like(&parts)
        });
        let launcher = r.iter().find(|r| r.item == "grappler_launcher").unwrap();
        assert!(matches!(
            launcher.fix,
            Fix::Bolts {
                n: LAUNCHER_BOLTS,
                ..
            }
        ));
        assert!(
            launcher.at.translation.y < 1.0 && launcher.at.translation.z > 0.5,
            "it lies back along the hull: {}",
            launcher.at.translation
        );
        let drum = r.iter().find(|r| r.item == "grappler_drum").unwrap();
        assert!(matches!(drum.fix, Fix::Cables { n: 1 }));
        assert!(r.iter().any(|r| r.item == "robot_cell"));
        let sorter = WreckParts {
            kind: RobotKind::Sorter,
            servo: Some(ServoAt {
                local: Vec3::Y,
                health: 0.6,
            }),
            ..walker_like(&parts)
        };
        let r = rows(&sorter);
        // Its servo pack, and its head (the test's parts have a Sensor).
        assert_eq!(r[0].item, "sorter_servo_pack");
        let head = r.iter().find(|r| r.item == "sorter_head");
        assert_eq!(
            head.is_some(),
            parts.iter().any(|p| p.1 == PartRole::Sensor),
            "{:?}",
            r.iter().map(|r| r.item).collect::<Vec<_>>()
        );
        assert!(r.iter().any(|r| r.item == "sorter_carriage_motor"));
        if let Some(h) = head {
            assert!(matches!(
                h.fix,
                Fix::Bolts {
                    n: SORTER_HEAD_BOLTS,
                    ..
                }
            ));
        }
        assert!(matches!(
            r[0].fix,
            Fix::Bolts {
                n: SORTER_BOLTS,
                ..
            }
        ));
    }

    fn walker_like<'a>(parts: &'a [(Entity, PartRole, f32, Transform, bool)]) -> WreckParts<'a> {
        WreckParts {
            kind: RobotKind::Walker,
            size: 1.0,
            shape: super::super::anim::Shape::Orb,
            legs: [1.0; 4],
            servo: None,
            crawler: false,
            hound: false,
            heavy: false,
            grappler: false,
            mender: false,
            parts,
        }
    }

    /// A live cell crackles while you're near it, before any hand's on
    /// it, and not when no one's there to see it.
    #[test]
    fn a_live_cell_warns_before_it_burns() {
        #[derive(Resource, Default)]
        struct Heard(usize);
        fn count(mut r: MessageReader<Crackled>, mut n: ResMut<Heard>) {
            n.0 += r.read().count();
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<Crackled>()
            .add_message::<crate::rig::haptics::Haptic>()
            .init_resource::<Heard>()
            .add_systems(Update, (live_tells, count).chain());
        let live = || Live {
            left: HOT_FOR,
            next: 0.0,
            tick: 0.0,
        };
        app.world_mut()
            .spawn((live(), GlobalTransform::from_xyz(0.0, 0.0, 0.0)));
        app.world_mut()
            .spawn((live(), GlobalTransform::from_xyz(50.0, 0.0, 0.0)));
        app.world_mut().spawn((
            crate::player::Player,
            GlobalTransform::from_xyz(1.5, 1.6, 0.0),
        ));
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Heard>().0,
            1,
            "the near cell crackled once (then waits); the far one is quiet"
        );
    }

    /// A walker-family wreck's fallen legs carry its knees (each machine's
    /// own, bolted) and its visor its eye (a hound's, its head), by hand;
    /// a panel, and anything off the heavy or another machine, nothing.
    #[test]
    fn its_fallen_pieces_are_its_knees_and_eye() {
        use super::super::wreck::Piece;
        let w = RobotKind::Walker;
        let knee = |c, h| piece_part(w, Piece::LegUpper, c, h, false, false, 0.7).unwrap();
        let k = knee(false, false);
        assert_eq!((k.item, k.bolts as usize), ("walker_leg_actuator", BOLTS));
        assert_eq!(k.health, 0.7);
        assert_eq!(knee(true, false).item, "crawler_leg_servo");
        assert_eq!(knee(false, true).item, "hound_hip_servo");
        let eye = piece_part(w, Piece::Sensor, false, false, false, false, 1.0).unwrap();
        assert!(eye.item == "walker_eye" && eye.bolts == 0 && eye.cables == 0);
        assert_eq!(
            piece_part(w, Piece::Sensor, false, true, false, false, 1.0)
                .unwrap()
                .item,
            "hound_head"
        );
        assert!(piece_part(w, Piece::Panel(0), false, false, false, false, 1.0).is_none());
        assert!(piece_part(w, Piece::LegLower, false, false, false, false, 1.0).is_none());
        assert!(piece_part(w, Piece::LegUpper, false, false, true, false, 1.0).is_none());
        assert!(
            piece_part(
                RobotKind::Drone,
                Piece::Sensor,
                false,
                false,
                false,
                false,
                1.0
            )
            .is_none()
        );
        // (And every one is an item.)
        let catalog = crate::items::Catalog::load();
        for (c, h) in [(false, false), (true, false), (false, true)] {
            for p in [Piece::LegUpper, Piece::Sensor] {
                let id = piece_part(w, p, c, h, false, false, 1.0).unwrap().item;
                assert!(catalog.kind(id).is_some(), "{id}");
            }
        }
    }

    /// A cooked wreck cooks what's on it: a hot cell's spent (no arc, no
    /// charge), its holds are burnt off, it's half what it was and
    /// charred; a plate's seam gives at a third of the force.
    #[test]
    fn a_cooked_wreck_strips_lossy() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<crate::anomaly::cook::PartCooked>()
            .insert_resource(Catalog::load())
            .add_systems(Update, cooked_salvage);
        let catalog = app.world().resource::<Catalog>().clone();
        let w = app.world_mut();
        let wreck = w
            .spawn((
                Robot {
                    kind: RobotKind::Walker,
                    wrecked: true,
                },
                super::super::wreck::Broken,
            ))
            .id();
        // (Charred already, as `anomaly::cook` leaves what it cooks: the
        // look, not the cooking, which is still to do.)
        let cell = w
            .spawn((
                Charred,
                crate::items::Item(extraction_core::items::ItemState::fresh(
                    &catalog,
                    catalog.expect("robot_cell"),
                    0.8,
                )),
                Live {
                    left: HOT_FOR,
                    next: 0.0,
                    tick: 0.0,
                },
            ))
            .id();
        let seat = w
            .spawn(SalvageSeat {
                wreck,
                hold: Hold::Bolts,
                item: cell,
            })
            .id();
        let bolt = w.spawn(Bolt { seat }).id();
        let plate = w
            .spawn((
                RobotPart {
                    robot: wreck,
                    role: PartRole::Armour(0),
                    hp: 10.0,
                    max_hp: 10.0,
                },
                HealthAtDeath(1.0),
                crate::destruction::hooks::Pryable {
                    needs: 3.0,
                    seam: Vec3::ZERO,
                    along: Vec3::X,
                    held: 0.0,
                },
            ))
            .id();
        w.write_message(crate::anomaly::cook::PartCooked {
            entity: wreck,
            at: Vec3::ZERO,
        });
        app.update();
        let w = app.world();
        let it = w.get::<crate::items::Item>(cell).unwrap();
        assert!((it.condition - 0.4).abs() < 1e-5, "{}", it.condition);
        assert_eq!(it.charge_wh, 0.0, "spent");
        assert!(w.get::<Live>(cell).is_none(), "no arc");
        assert!(w.get::<Charred>(cell).is_some());
        assert!(w.get_entity(bolt).is_err(), "its bolts burnt off");
        assert!((w.get::<HealthAtDeath>(plate).unwrap().0 - 0.5).abs() < 1e-5);
        let seam = w.get::<crate::destruction::hooks::Pryable>(plate).unwrap();
        assert!((seam.needs - 1.0).abs() < 1e-5, "{}", seam.needs);
        // (And char takes the glow out.)
        let hot = StandardMaterial {
            emissive: LinearRgba::rgb(2.0, 0.4, 0.0),
            ..default()
        };
        assert_eq!(char_of(&hot).emissive, LinearRgba::BLACK);
    }

    /// A drone's wreck: its cell (hot, by hand), each rotor still on and
    /// its eye, twisted off by hand in their places; a mender's cell sits
    /// on its back. What flew off as it crashed lies where it fell: a
    /// rotor, its gun pod (a mender's, its torch arm).
    #[test]
    fn a_drones_wreck_is_its_rotors_eye_and_cell() {
        use super::super::wreck::Piece;
        let rotor = Entity::from_raw_u32(3).unwrap();
        let eye = Entity::from_raw_u32(4).unwrap();
        let parts = [
            (
                Entity::PLACEHOLDER,
                PartRole::Core,
                0.6,
                Transform::IDENTITY,
                false,
            ),
            (
                rotor,
                PartRole::Rotor(1),
                0.9,
                Transform::from_xyz(0.3, 0.05, 0.3),
                false,
            ),
            (
                eye,
                PartRole::Sensor,
                0.0,
                Transform::from_xyz(0.0, -0.06, -0.22),
                false,
            ),
        ];
        let drone = WreckParts {
            kind: RobotKind::Drone,
            ..walker_like(&parts)
        };
        let r = rows(&drone);
        let ids: Vec<_> = r.iter().map(|r| r.item).collect();
        assert_eq!(
            ids,
            ["robot_cell", "drone_rotor_motor", "drone_eye"],
            "{ids:?}"
        );
        assert!(r[0].hot && r[0].health == 0.6 && r[0].at.translation == DRONE_CELL_AT);
        assert_eq!(r[1].replaces, Some(rotor));
        assert!(r.iter().all(|r| matches!(r.fix, Fix::Hand)));
        assert_eq!(r[2].health, 0.0, "a shot-out eye is scrap");
        let mender = rows(&WreckParts {
            kind: RobotKind::Drone,
            mender: true,
            ..walker_like(&parts)
        });
        assert_eq!(mender[0].at.translation, MENDER_CELL_AT);
        let d = RobotKind::Drone;
        let piece = |p, m| piece_part(d, p, false, false, false, m, 0.5).map(|s| s.item);
        assert_eq!(piece(Piece::Rotor, false), Some("drone_rotor_motor"));
        assert_eq!(piece(Piece::Gun, false), Some("drone_gunpod"));
        assert_eq!(piece(Piece::Gun, true), Some("mender_torch"));
        assert_eq!(piece(Piece::Panel(2), false), None);
        let catalog = crate::items::Catalog::load();
        for id in [
            "drone_eye",
            "drone_gunpod",
            "mender_torch",
            "drone_rotor_motor",
        ] {
            assert!(catalog.kind(id).is_some(), "{id}");
        }
    }

    /// A knee goes on whichever side of its lying leg is up.
    #[test]
    fn a_knee_sits_on_top_of_its_leg() {
        // A leg lying along the ground, its local X turned to the sky.
        let lying = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let up = up_side(lying);
        assert!((lying * up).y > 0.99, "{up}");
        // On its back the other way: -X up.
        assert!(
            (up_side(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2)) - Vec3::NEG_X).length()
                < 1e-5
        );
    }

    #[test]
    fn where_you_shot_it_is_what_its_worth() {
        assert_eq!(condition(1.0), 1.0);
        assert_eq!(condition(0.6), 0.6);
        assert!(condition(0.6) > condition(0.2));
        assert_eq!(scrap_for(6.0), 3);
        assert_eq!(scrap_for(7.5), 4);
        assert_eq!(scrap_for(0.9), 1);
    }
}

#[cfg(test)]
mod dirigible_tests {
    use super::*;
    use crate::interact::InSocket;
    use crate::items::Item;

    /// Two cells holed, a dirigible sinks; down, what was on it is seated
    /// where a hand reaches from the ground (a valve on each cell's flank,
    /// its gondola's cell, live, its eye on the gondola's nose), its fabric
    /// and frame loose, and no table rolled over it.
    #[test]
    fn a_downed_dirigible_is_what_was_on_it() {
        let opts = crate::LaunchOptions {
            map: extraction_core::map::TEST_FIELD_ID.into(),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let ground = zone.on_ground(40.0, 230.0);
        let e = {
            let mut c = app.world_mut().commands();
            let at = ground + Vec3::Y * 9.0;
            let e = super::super::dirigible::spawn_dirigible(
                &mut c,
                at,
                super::super::Job {
                    kind: super::super::JobKind::Survey,
                    site: at,
                    duration: 1.0e6,
                    exit: at,
                },
            );
            app.world_mut().flush();
            e
        };
        app.update();
        app.world_mut()
            .get_mut::<super::super::dirigible::Dirigible>(e)
            .unwrap()
            .sinking = true;
        for _ in 0..90 * 15 {
            app.update();
        }
        assert!(app.world().get::<super::super::Robot>(e).unwrap().wrecked);
        assert!(app.world().get::<super::super::Salvaged>(e).is_some());
        let catalog = app.world().resource::<Catalog>().clone();
        let seated: Vec<(String, Vec3, bool)> = app
            .world_mut()
            .query_filtered::<(&Item, &GlobalTransform, Has<Live>), With<InSocket>>()
            .iter(app.world())
            .map(|(i, g, live)| (catalog.get(i.kind()).id.clone(), g.translation(), live))
            .filter(|(id, at, _)| {
                at.xz().distance(ground.xz()) < 12.0
                    && ["dirigible_gas_valve", "robot_cell", "walker_eye"].contains(&id.as_str())
            })
            .collect();
        let mut ids: Vec<&str> = seated.iter().map(|(id, ..)| id.as_str()).collect();
        ids.sort();
        assert_eq!(
            ids,
            [
                "dirigible_gas_valve",
                "dirigible_gas_valve",
                "dirigible_gas_valve",
                "robot_cell",
                "walker_eye"
            ],
            "{seated:?}"
        );
        for (id, at, live) in &seated {
            let over = at.y - zone.ground(at.x, at.z);
            assert!(
                (0.0..crate::sim::indoor::MAX_REACH_UP).contains(&over),
                "{id} {over:.2} m over the ground"
            );
            assert_eq!(*live, id == "robot_cell", "{id} live: {live}");
        }
        let loose: Vec<String> = app
            .world_mut()
            .query_filtered::<(&Item, &GlobalTransform), Without<InSocket>>()
            .iter(app.world())
            .filter(|(_, g)| g.translation().xz().distance(ground.xz()) < 12.0)
            .map(|(i, _)| catalog.get(i.kind()).id.clone())
            // (Its own, not the map's: the table's rolls were these and more.)
            .filter(|id| {
                [
                    "polymer",
                    "scrap_metal",
                    "electronics",
                    "walker_eye",
                    "hound_hip_servo",
                ]
                .contains(&id.as_str())
            })
            .collect();
        let mut loose_ids: Vec<&str> = loose.iter().map(|s| s.as_str()).collect();
        loose_ids.sort();
        assert_eq!(loose_ids, ["polymer", "polymer", "scrap_metal"]);
    }
}
