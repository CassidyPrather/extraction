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
/// How far out of the hip the bolts' heads stand (m): proud of the hull,
/// so a spanner gets to them.
const BOLT_PROUD: f32 = 0.1;
/// Bolts holding the Sorter's servo pack on (a bigger mount).
pub const SORTER_BOLTS: usize = 4;
/// Servo packs a walker wreck gives up at most (its soundest hips).
pub const MOST_SERVOS: usize = 2;

pub struct SalvagePlugin;

impl Plugin for SalvagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Arced>()
            .add_message::<Crackled>()
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
/// Whether a grappler's wreck seats its launcher and drum yet.
const GRAPPLER_SEATS: bool = false;
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
    // The servo packs at the soundest hips, bolted on. (The side its wreck
    // rolled up: `wreck::servo_hips`.)
    for i in super::wreck::servo_hips(w.legs) {
        let hip = w.shape.hip(i) * w.size;
        let out_dir = Vec3::new(hip.x, 0.0, hip.z).normalize_or(Vec3::X);
        // Facing out of the hip, the bolts round it.
        let face = Quat::from_rotation_arc(Vec3::Y, out_dir);
        out.push(Row {
            item: "walker_leg_actuator",
            at: Transform::from_translation(hip).with_rotation(face),
            size: w.size,
            health: w.legs[i],
            fix: Fix::Bolts {
                n: BOLTS,
                centre: hip + out_dir * BOLT_PROUD,
                face,
                radius: 0.06,
            },
            hot: false,
            replaces: None,
        });
    }
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
        ),
        (
            With<super::wreck::Broken>,
            Without<Fitted>,
            With<super::Salvaged>,
        ),
    >,
    parts: Query<(&RobotPart, &Transform, Has<WalkerTurret>)>,
    all_parts: Query<&RobotPart>,
    catalog: Option<Res<Catalog>>,
    // (Not in a headless app: no cable to see there.)
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(catalog) = catalog else {
        return;
    };
    for (e, robot, children, gait, legs, servo, crawler, hound, heavy, grappler) in &wrecks {
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
            // (Not yet: its launcher's head and drum are drawn in the
            // boom's and pedestal's nodes, and would stay drawn once taken.
            // Robot bodies is splitting them out.)
            grappler: grappler && GRAPPLER_SEATS,
            parts: &on,
        };
        let look = Look::new(meshes.as_deref_mut(), materials.as_deref_mut());
        for row in rows(&w) {
            fit(&mut commands, &catalog, e, row, &look);
        }
    }
}

/// The meshes a part's holds are drawn with (none in a headless app).
struct Look {
    bolt: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    cable: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

impl Look {
    fn new(
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
fn fit(commands: &mut Commands, catalog: &Catalog, wreck: Entity, row: Row, look: &Look) {
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
            parts: &parts,
        };
        let r = rows(&walker);
        let ids: Vec<_> = r.iter().map(|r| r.item).collect();
        assert_eq!(
            ids.iter().filter(|i| **i == "robot_cell").count(),
            1,
            "{ids:?}"
        );
        assert_eq!(
            ids.iter().filter(|i| **i == "walker_leg_actuator").count(),
            MOST_SERVOS,
            "{ids:?}"
        );
        let cell = r.iter().find(|r| r.item == "robot_cell").unwrap();
        assert!(cell.hot && matches!(cell.fix, Fix::Hand));
        assert_eq!(cell.health, 0.8, "the core's health is the cell's");
        let pod = r.iter().find(|r| r.item == "robot_gunpod").unwrap();
        assert!(matches!(pod.fix, Fix::Cables { n: CABLES }));
        assert_eq!(pod.replaces, Some(turret));
        assert!(
            r.iter()
                .filter(|r| r.item == "walker_leg_actuator")
                .all(|r| matches!(r.fix, Fix::Bolts { n: BOLTS, .. }))
        );
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
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].item, "sorter_servo_pack");
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
