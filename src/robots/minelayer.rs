//! The Custodian's minelayer: a low tracked crawler with a striped hopper
//! on its back. It crawls out from the gate to the ways people walk and
//! sows a short line of pressure mines across each one. It carries no
//! gun. (Its rules, and the mines', are `extraction_core::minelayer`.)
//!
//! - **The tell.** It grinds along slowly with its amber lamp turning,
//!   and stops to lay with a chunk and a hiss from its chute. Each laid
//!   mine blinks red every couple of seconds and sits in a ring of
//!   turned earth.
//! - **The weakness.** It's slow and blind behind, and the hopper on its
//!   back is full of mines: hole it and they cook off, taking the crawler
//!   with them. Seen, it turns tail, and drops a mine behind it as it
//!   goes: don't follow in its tracks.
//! - **The mines.** Find them, then cut the lead with bolt cutters or
//!   hold a hand on the prong until it's safe: either way it's yours, a
//!   safe `pressure_mine` to sell. Or shoot it from out of its reach.
//! - **The salvage.** Its cell and drive servo; and if the hopper came
//!   through whole, the mines still in it, safe.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::minelayer::{self as rules, Disarm, MineAct};

use super::{Awareness, Brain, Job, JobKind, PartRole, Robot, RobotKind, Senses, part_bundle};
use crate::weapons::ballistics::Hit;
use crate::world::{Layer, Zone};

pub const HULL_HP: f32 = 220.0;
pub const HOPPER_HP: f32 = 70.0;
pub const EYE_HP: f32 = 40.0;
pub const TREAD_HP: f32 = 150.0;
/// Where its parts sit (local; the root is on the ground under its
/// middle).
pub const HULL: Vec3 = Vec3::new(0.0, 0.45, 0.0);
pub const HOPPER_AT: Vec3 = Vec3::new(0.0, 0.85, 0.3);
pub const EYE: Vec3 = Vec3::new(0.0, 0.72, -0.62);
pub const CHUTE: Vec3 = Vec3::new(0.0, 0.3, 0.78);
pub const TREADS: [Vec3; 2] = [Vec3::new(-0.5, 0.25, 0.0), Vec3::new(0.5, 0.25, 0.0)];
/// Seen within this, it turns tail; for this long.
pub const FLEE_NEAR: f32 = 30.0;
pub const FLEE_FOR: f32 = 7.0;
/// What a person weighs on a mine before what they carry (kg); a walker;
/// a heavy.
pub const PERSON_KG: f32 = 75.0;
pub const WALKER_KG: f32 = 180.0;
/// Counters a scenario reads.
pub const BLOWN: &str = "mines_blown";
pub const DISARMED: &str = "mines_disarmed";
pub const LAID: &str = "mines_laid";

/// A minelayer.
#[derive(Component, Debug)]
pub struct Minelayer {
    /// Mines left in its hopper.
    pub hopper: u32,
    /// Where it lays its fields, in turn, and where it leaves by.
    pub sites: Vec<Vec3>,
    pub exit: Vec3,
    pub doing: Doing,
    /// Its heading, degrees (the game's yaw).
    pub yaw: f32,
    /// Seconds before it may turn tail again.
    pub flee_cooldown: f32,
}

/// What it's about.
#[derive(Debug, Clone, PartialEq)]
pub enum Doing {
    /// Crawling a path to its next site (or out).
    Travel { path: Vec<Vec2>, next: usize },
    /// Laying a field: its spots, which it's at, and seconds left on this
    /// one.
    Lay {
        spots: Vec<Vec2>,
        next: usize,
        wait: f32,
    },
    /// Turning tail from something it saw.
    Flee { from: Vec3, left: f32 },
    /// Done: crawling out.
    Leave { path: Vec<Vec2>, next: usize },
    /// Wrecked.
    Dead,
}

/// Its hopper (a part).
#[derive(Component, Debug)]
pub struct Hopper;

/// A laid mine.
#[derive(Component, Debug)]
pub struct LaidMine {
    pub mine: rules::Mine,
    pub disarm: Disarm,
    /// Its light's phase.
    pub phase: f32,
}

/// A mine's lead: cut it and the mine's safe. (Destruction takes it
/// away; [`lead_cut`] notices.)
#[derive(Component, Debug)]
pub struct MineLead(pub Entity);

/// What a minelayer or a mine did, for sound and light.
#[derive(Message, Debug, Clone, Copy)]
pub struct MineEvent {
    pub at: Vec3,
    pub what: MineAct2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MineAct2 {
    /// The chute: a mine laid.
    Laid,
    /// Stepped on.
    Click,
    Blast,
    /// Made safe.
    Disarmed,
    /// Its hopper cooked off.
    CookOff,
    /// It saw you and turned tail.
    Fled,
}

pub struct MinelayerPlugin;

impl Plugin for MinelayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MineEvent>()
            .init_resource::<crate::bot::Counters>()
            .init_resource::<CutLeads>()
            .add_observer(lead_cut)
            .add_systems(
                FixedUpdate,
                (
                    minelayer_mind,
                    shot_mines,
                    mines,
                    hopper_cook_off,
                    minelayer_wreck,
                )
                    .chain()
                    .after(super::perceive)
                    .before(PhysicsSystems::First),
            )
            .add_systems(FixedUpdate, (send_minelayer, stage_scenario));
    }
}

/// A minelayer at `at` (on the ground), off to lay a field at each of
/// `sites` and leave by `exit`.
pub fn spawn_minelayer(
    commands: &mut Commands,
    zone: &Zone,
    nav: &extraction_core::nav::NavGrid,
    at: Vec3,
    sites: Vec<Vec3>,
    exit: Vec3,
) -> Entity {
    let first = sites.first().copied().unwrap_or(exit);
    let path = route(nav, at, first);
    let yaw = extraction_core::sentry::bearing([at.x, at.z], [first.x, first.z]);
    let foot = zone.on_ground(at.x, at.z);
    let robot = commands
        .spawn((
            Name::new("Minelayer"),
            Robot {
                kind: RobotKind::Pylon,
                wrecked: false,
            },
            super::Breed("minelayer"),
            super::heavy::OwnLook,
            super::heavy::NotOnPatrol,
            Minelayer {
                hopper: rules::HOPPER,
                sites,
                exit,
                doing: Doing::Travel { path, next: 0 },
                yaw,
                flee_cooldown: 0.0,
            },
            Senses {
                sight: 35.0,
                fov_deg: 120.0,
                hearing: 0.6,
                eye: EYE,
            },
            Awareness::default(),
            Brain::Working {
                at: first,
                left: f32::INFINITY,
            },
            Job {
                kind: JobKind::Watch,
                site: first,
                duration: f32::INFINITY,
                exit,
            },
            Transform::from_translation(foot)
                .with_rotation(Quat::from_rotation_y(-yaw.to_radians())),
            Visibility::default(),
            RigidBody::Kinematic,
        ))
        .id();
    commands.entity(robot).with_children(|c| {
        c.spawn((
            Name::new("Minelayer hull"),
            part_bundle(
                robot,
                PartRole::Core,
                HULL_HP,
                Collider::cuboid(0.8, 0.4, 1.4),
                Transform::from_translation(HULL),
            ),
        ));
        c.spawn((
            Name::new("Minelayer hopper"),
            Hopper,
            part_bundle(
                robot,
                PartRole::Armour(20),
                HOPPER_HP,
                Collider::cuboid(0.6, 0.45, 0.56),
                Transform::from_translation(HOPPER_AT),
            ),
        ));
        c.spawn((
            Name::new("Minelayer eye"),
            part_bundle(
                robot,
                PartRole::Sensor,
                EYE_HP,
                Collider::cuboid(0.2, 0.12, 0.1),
                Transform::from_translation(EYE),
            ),
        ));
        for (k, at) in TREADS.into_iter().enumerate() {
            c.spawn((
                Name::new(if k == 0 {
                    "Minelayer tread (left)"
                } else {
                    "Minelayer tread (right)"
                }),
                part_bundle(
                    robot,
                    PartRole::Armour(60),
                    TREAD_HP,
                    Collider::cuboid(0.22, 0.5, 1.5),
                    Transform::from_translation(at),
                ),
            ));
        }
    });
    robot
}

/// A mine on the ground at `at`.
pub fn lay_mine(commands: &mut Commands, at: Vec3, phase: f32) -> Entity {
    let mine = commands
        .spawn((
            Name::new("Pressure mine"),
            LaidMine {
                mine: rules::Mine::default(),
                disarm: Disarm::default(),
                phase,
            },
            Transform::from_translation(at),
            Visibility::default(),
            RigidBody::Static,
            // Low enough to step over (and onto); a round finds it.
            Collider::cylinder(0.15, 0.08),
            CollisionLayers::new(Layer::Robot, [Layer::Item]),
        ))
        .id();
    commands.spawn((
        Name::new("Pressure mine lead"),
        MineLead(mine),
        crate::destruction::hooks::Cuttable { needs: 0.0 },
        Sensor,
        Collider::sphere(0.12),
        CollisionLayers::new(Layer::Hand, LayerMask::NONE),
        Transform::from_xyz(0.1, 0.06, 0.0),
        ChildOf(mine),
    ));
    mine
}

/// A path over the walk grid from `a` to `b` (straight if there's none).
fn route(nav: &extraction_core::nav::NavGrid, a: Vec3, b: Vec3) -> Vec<Vec2> {
    nav.path([a.x, a.z], [b.x, b.z])
        .map(|p| p.into_iter().map(Vec2::from).collect())
        .unwrap_or_else(|| vec![Vec2::new(b.x, b.z)])
}

/// Turn toward `to` and crawl on when it's ahead; true once there.
fn drive(t: &mut Transform, yaw: &mut f32, to: Vec2, speed: f32, dt: f32) -> bool {
    let at = Vec2::new(t.translation.x, t.translation.z);
    if at.distance(to) < 0.6 {
        return true;
    }
    let want = extraction_core::sentry::bearing([at.x, at.y], [to.x, to.y]);
    let off = extraction_core::sentry::wrap_deg(want - *yaw);
    let step = rules::TURN * dt;
    *yaw = extraction_core::sentry::wrap_deg(*yaw + off.clamp(-step, step));
    if off.abs() < 35.0 {
        let fwd = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::NEG_Z;
        t.translation += fwd * speed * dt;
    }
    false
}

/// Crawl, lay, turn tail, leave.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn minelayer_mind(
    mut commands: Commands,
    mut layers: Query<(
        Entity,
        &mut Minelayer,
        &mut Transform,
        &Awareness,
        &Robot,
        &Children,
        Has<super::Stunned>,
    )>,
    parts: Query<&super::RobotPart>,
    targets: Query<&GlobalTransform>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    mut out: MessageWriter<MineEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let mut dropped: Vec<Vec3> = Vec::new();
    let mut fled: Vec<Vec3> = Vec::new();
    for (e, mut m, mut t, aware, robot, children, stunned) in &mut layers {
        if robot.wrecked {
            m.doing = Doing::Dead;
            continue;
        }
        if stunned {
            continue;
        }
        m.flee_cooldown = (m.flee_cooldown - dt).max(0.0);
        // A lost tread slows it to a limp.
        let treads = children
            .iter()
            .filter_map(|c| parts.get(c).ok())
            .filter(|p| p.role == PartRole::Armour(60) && p.alive())
            .count();
        let speed = rules::SPEED * if treads == 2 { 1.0 } else { 0.35 };
        let here = t.translation;
        let mut drop_mine = |m: &mut Minelayer, commands: &mut Commands, at: Vec3| {
            drop_one(m, commands, &zone, &mut counters, &mut dropped, at)
        };
        let chute = |t: &Transform| t.transform_point(CHUTE);
        // Seen near: turn tail, a mine dropped behind.
        let seen = aware
            .target
            .filter(|_| aware.can_see_target && aware.suspicion >= 0.5)
            .and_then(|p| targets.get(p).ok())
            .map(|g| g.translation())
            .filter(|p| p.distance(here) < FLEE_NEAR);
        if let Some(p) = seen {
            if m.flee_cooldown <= 0.0 && !matches!(m.doing, Doing::Flee { .. }) {
                let c = chute(&t);
                drop_mine(&mut m, &mut commands, c);
                m.doing = Doing::Flee {
                    from: p,
                    left: FLEE_FOR,
                };
                m.flee_cooldown = FLEE_FOR * 3.0;
                fled.push(here);
            }
        }
        let mut yaw = m.yaw;
        let mut doing = std::mem::replace(&mut m.doing, Doing::Dead);
        doing = match doing {
            Doing::Travel { path, mut next } => {
                if next < path.len() {
                    if drive(&mut t, &mut yaw, path[next], speed, dt) {
                        next += 1;
                    }
                    Doing::Travel { path, next }
                } else if let Some(site) = m.sites.first().copied() {
                    m.sites.remove(0);
                    // Across the way in from the gate (as the map's lint
                    // reckons it: `map_mines`).
                    let along = Vec2::new(site.x - m.exit.x, site.z - m.exit.z);
                    let along = if along.length() > 1.0 {
                        along
                    } else {
                        Vec2::new(site.x - here.x, site.z - here.z)
                    };
                    // Never in a doorway, on a road or close by a building.
                    let spots =
                        rules::site_field(&zone.map, &zone.hm, [site.x, site.z], along.into())
                            .into_iter()
                            .map(Vec2::from)
                            .collect();
                    Doing::Lay {
                        spots,
                        next: 0,
                        wait: rules::LAY_TIME,
                    }
                } else {
                    Doing::Leave {
                        path: route(&nav.0, here, m.exit),
                        next: 0,
                    }
                }
            }
            Doing::Lay {
                spots,
                mut next,
                mut wait,
            } => {
                if next >= spots.len() || m.hopper == 0 {
                    // On to the next site, or out.
                    let to = m.sites.first().copied();
                    match to {
                        Some(s) if m.hopper > 0 => Doing::Travel {
                            path: route(&nav.0, here, s),
                            next: 0,
                        },
                        _ => Doing::Leave {
                            path: route(&nav.0, here, m.exit),
                            next: 0,
                        },
                    }
                } else {
                    // Drive so the chute comes over the spot, then stop and
                    // lay.
                    let back = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::Z * CHUTE.z;
                    let to = spots[next] - Vec2::new(back.x, back.z);
                    if drive(&mut t, &mut yaw, to, speed * 0.7, dt) {
                        wait -= dt;
                        if wait <= 0.0 {
                            let c = chute(&t);
                            drop_mine(&mut m, &mut commands, c);
                            next += 1;
                            wait = rules::LAY_TIME;
                        }
                    }
                    Doing::Lay { spots, next, wait }
                }
            }
            Doing::Flee { from, mut left } => {
                left -= dt;
                let away = Vec2::new(here.x - from.x, here.z - from.z).normalize_or(Vec2::X);
                let to = Vec2::new(here.x, here.z) + away * 10.0;
                drive(&mut t, &mut yaw, to, speed * 1.3, dt);
                if left > 0.0 {
                    Doing::Flee { from, left }
                } else {
                    match m.sites.first().copied() {
                        Some(s) if m.hopper > 0 => Doing::Travel {
                            path: route(&nav.0, here, s),
                            next: 0,
                        },
                        _ => Doing::Leave {
                            path: route(&nav.0, here, m.exit),
                            next: 0,
                        },
                    }
                }
            }
            Doing::Leave { path, mut next } => {
                if next < path.len() {
                    if drive(&mut t, &mut yaw, path[next], speed, dt) {
                        next += 1;
                    }
                    Doing::Leave { path, next }
                } else {
                    // Out by the gate: gone.
                    commands.entity(e).despawn();
                    continue;
                }
            }
            Doing::Dead => Doing::Dead,
        };
        m.doing = doing;
        m.yaw = yaw;
        let ground = zone.ground(t.translation.x, t.translation.z);
        t.translation.y = ground;
        t.rotation = Quat::from_rotation_y(-yaw.to_radians());
    }
    for at in dropped {
        out.write(MineEvent {
            at,
            what: MineAct2::Laid,
        });
    }
    for at in fled {
        out.write(MineEvent {
            at,
            what: MineAct2::Fled,
        });
    }
}

/// A mine out of the hopper onto the ground under `at`.
fn drop_one(
    m: &mut Minelayer,
    commands: &mut Commands,
    zone: &Zone,
    counters: &mut crate::bot::Counters,
    dropped: &mut Vec<Vec3>,
    at: Vec3,
) {
    if m.hopper == 0 {
        return;
    }
    m.hopper -= 1;
    let p = zone.on_ground(at.x, at.z);
    lay_mine(commands, p, (m.hopper as f32) * 0.71);
    counters.bump(LAID);
    dropped.push(p);
}

/// Leads cut since [`mines`] last looked.
#[derive(Resource, Default)]
struct CutLeads(Vec<Entity>);

fn lead_cut(gone: On<Remove, MineLead>, leads: Query<&MineLead>, mut cut: ResMut<CutLeads>) {
    if let Ok(l) = leads.get(gone.entity) {
        cut.0.push(l.0);
    }
}

/// A round into a mine (read apart from [`mines`], which writes the
/// blast's own hits).
#[derive(Component, Debug)]
pub struct MineShot;

fn shot_mines(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mines: Query<(), With<LaidMine>>,
) {
    for h in hits.read() {
        if mines.contains(h.entity) {
            commands.entity(h.entity).try_insert(MineShot);
        }
    }
}

/// Mines: weight on them, a hand on them, a round in them.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn mines(
    mut commands: Commands,
    mut laid: Query<(Entity, &mut LaidMine, &GlobalTransform, Has<MineShot>)>,
    players: Query<(&GlobalTransform, &crate::player::Body), With<crate::player::Player>>,
    robots: Query<(&Robot, &GlobalTransform, Has<super::heavy::Heavy>)>,
    hands: Query<(&crate::interact::Hand, &GlobalTransform)>,
    parts: Query<(Entity, &GlobalTransform), With<super::RobotPart>>,
    bodies: Query<(Entity, &GlobalTransform), With<crate::player::BodyCollider>>,
    mut cut: ResMut<CutLeads>,
    catalog: Res<crate::items::Catalog>,
    spatial: SpatialQuery,
    mut hits: MessageWriter<Hit>,
    mut noise: MessageWriter<super::Noise>,
    mut out: MessageWriter<MineEvent>,
    mut counters: ResMut<crate::bot::Counters>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let cut: Vec<Entity> = std::mem::take(&mut cut.0);
    let mut blasts: Vec<Vec3> = Vec::new();
    for (e, mut m, gt, shot) in &mut laid {
        let at = gt.translation();
        let on_plate = |p: Vec3| {
            Vec2::new(p.x - at.x, p.z - at.z).length() < rules::PLATE && (p.y - at.y).abs() < 0.6
        };
        // Cut, or a hand held on its prong: safe, and yours.
        let held = hands.iter().any(|(h, hg)| {
            h.grip_down && h.holding.is_none() && hg.translation().distance(at) < 0.2
        });
        let disarmed = cut.contains(&e) || m.disarm.step(held, dt);
        if disarmed && m.mine.clicked.is_none() {
            commands.entity(e).despawn();
            if let Some(kind) = catalog.kind("pressure_mine") {
                crate::items::spawn_item(
                    &mut commands,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::from_translation(at + Vec3::Y * 0.12),
                        condition: 1.0,
                    },
                );
            }
            counters.bump(DISARMED);
            out.write(MineEvent {
                at,
                what: MineAct2::Disarmed,
            });
            continue;
        }
        let mut weight: f32 = 0.0;
        for (p, body) in &players {
            if on_plate(p.translation()) {
                weight = weight.max(PERSON_KG + body.carried_kg);
            }
        }
        for (r, rg, heavy) in &robots {
            if r.kind == RobotKind::Walker || heavy {
                if on_plate(rg.translation()) {
                    weight = weight.max(WALKER_KG);
                }
            }
        }
        let act = if shot {
            MineAct::Blast
        } else {
            m.mine.step(weight, dt)
        };
        match act {
            MineAct::Click => out.write(MineEvent {
                at,
                what: MineAct2::Click,
            }),
            MineAct::Blast => {
                commands.entity(e).despawn();
                blasts.push(at);
                counters.bump(BLOWN);
                out.write(MineEvent {
                    at,
                    what: MineAct2::Blast,
                })
            }
            MineAct::Quiet => continue,
        };
    }
    for at in blasts {
        blast(at, &parts, &bodies, &spatial, &mut hits, &mut noise, None);
    }
}

/// A blast at `at`: harm to what it can see by distance, and a bang heard
/// far off.
fn blast(
    at: Vec3,
    parts: &Query<(Entity, &GlobalTransform), With<super::RobotPart>>,
    bodies: &Query<(Entity, &GlobalTransform), With<crate::player::BodyCollider>>,
    spatial: &SpatialQuery,
    hits: &mut MessageWriter<Hit>,
    noise: &mut MessageWriter<super::Noise>,
    scale: Option<f32>,
) {
    let k = scale.unwrap_or(1.0);
    let origin = at + Vec3::Y * 0.15;
    noise.write(super::Noise {
        at,
        radius: 150.0,
        source: None,
    });
    for (target, tgt) in parts.iter().chain(bodies.iter()) {
        let p = tgt.translation();
        let d = p.distance(origin) / k;
        let harm = rules::blast(d) * k;
        if harm <= 0.0 {
            continue;
        }
        let v = p - origin;
        let blocked = Dir3::new(v).ok().and_then(|dir| {
            spatial.cast_ray(
                origin,
                dir,
                v.length() - 0.05,
                true,
                &SpatialQueryFilter::from_mask(Layer::World),
            )
        });
        if blocked.is_some() {
            continue;
        }
        hits.write(Hit {
            entity: target,
            point: p,
            normal: (origin - p).normalize_or(Vec3::Y),
            velocity: v.normalize_or(Vec3::Y) * 50.0,
            damage: harm,
            shooter: None,
            mass_g: 0.0,
            load: Default::default(),
            blow: None,
        });
    }
}

/// Its hopper holed: the mines in it go off together, and it with them.
#[allow(clippy::too_many_arguments)]
fn hopper_cook_off(
    mut destroyed: MessageReader<super::PartDestroyed>,
    hoppers: Query<(), With<Hopper>>,
    mut layers: Query<&mut Minelayer>,
    parts: Query<(Entity, &GlobalTransform), With<super::RobotPart>>,
    bodies: Query<(Entity, &GlobalTransform), With<crate::player::BodyCollider>>,
    spatial: SpatialQuery,
    mut hits: MessageWriter<Hit>,
    mut noise: MessageWriter<super::Noise>,
    mut out: MessageWriter<MineEvent>,
    cores: Query<(Entity, &super::RobotPart, &GlobalTransform)>,
) {
    for d in destroyed.read() {
        if !hoppers.contains(d.part) {
            continue;
        }
        let Ok(mut m) = layers.get_mut(d.robot) else {
            continue;
        };
        let n = m.hopper;
        m.hopper = 0;
        info!("minelayer: its hopper cooked off ({n} mines)");
        out.write(MineEvent {
            at: d.at,
            what: MineAct2::CookOff,
        });
        // Bigger the fuller it was; never less than enough to finish it.
        let k = 1.0 + 0.15 * n as f32;
        blast(
            d.at,
            &parts,
            &bodies,
            &spatial,
            &mut hits,
            &mut noise,
            Some(k.max(1.6)),
        );
        // And its own heart goes with it.
        for (c, p, g) in &cores {
            if p.robot == d.robot && p.role == PartRole::Core {
                hits.write(Hit {
                    entity: c,
                    point: g.translation(),
                    normal: Vec3::Y,
                    velocity: Vec3::Y * 50.0,
                    damage: HULL_HP * 3.0,
                    shooter: None,
                    mass_g: 0.0,
                    load: Default::default(),
                    blow: None,
                });
            }
        }
    }
}

/// Wrecked: it stays where it stopped; its cell, its drive, and what's
/// left in its hopper, safe, lie by it.
fn minelayer_wreck(
    mut commands: Commands,
    layers: Query<(Entity, &Minelayer, &Robot, &GlobalTransform), Without<super::wreck::Broken>>,
    catalog: Res<crate::items::Catalog>,
) {
    for (e, m, robot, gt) in &layers {
        if !robot.wrecked {
            continue;
        }
        commands
            .entity(e)
            .insert((super::wreck::Broken, super::Salvaged));
        let at = gt.translation();
        let mut drops = vec!["robot_cell", "servo"];
        for _ in 0..m.hopper.min(3) {
            drops.push("pressure_mine");
        }
        for (k, id) in drops.into_iter().enumerate() {
            let Some(kind) = catalog.kind(id) else {
                continue;
            };
            let a = k as f32 * 1.7;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        at + Vec3::new(a.cos() * 1.3, 0.4, a.sin() * 1.3),
                    ),
                    condition: 0.6,
                },
            );
        }
    }
}

/// Seconds into a raid the minelayer sets out (once a raid). (How many
/// fields it lays: `rules::FIELDS`.)
pub const SET_OUT: f32 = 300.0;

/// Once a raid, robots on, a minelayer sets out from the gate to lay its
/// fields at points of interest along the way in.
#[allow(clippy::too_many_arguments)]
fn send_minelayer(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    nav: Option<Res<super::walker::Nav>>,
    director: Option<Res<super::director::Director>>,
    raid: Option<Res<crate::raid::Raid>>,
    mut sent: Local<Option<f32>>,
    time: Res<Time>,
) {
    let (Some(zone), Some(nav), Some(director)) = (zone, nav, director) else {
        return;
    };
    if !director.enabled || raid.as_ref().is_some_and(|r| r.recalled) {
        return;
    }
    let now = time.elapsed_secs();
    let t0 = *sent.get_or_insert(now);
    if t0 < 0.0 || now - t0 < SET_OUT {
        return;
    }
    *sent = Some(-1.0);
    let gate = super::director::gate(&zone);
    let insertion = zone
        .marker(&extraction_core::map::MarkerKind::Insertion)
        .map(|m| Vec2::from(m.at))
        .unwrap_or(Vec2::ZERO);
    // The points of interest nearest the way in from the insertion.
    let mut pois: Vec<Vec3> = zone
        .map
        .markers
        .iter()
        .filter(|m| m.kind == extraction_core::map::MarkerKind::Poi)
        .map(|m| zone.on_ground(m.at[0], m.at[1]))
        .collect();
    pois.sort_by(|a, b| {
        let da = Vec2::new(a.x, a.z).distance(insertion);
        let db = Vec2::new(b.x, b.z).distance(insertion);
        da.total_cmp(&db)
    });
    // Each laid out in front of its POI where there's room
    // (`rules::site_spot`: the map's lint checks the same).
    let sites: Vec<Vec3> = pois
        .into_iter()
        .skip(1)
        .filter_map(|p| {
            let along = [p.x - gate.x, p.z - gate.z];
            rules::site_spot(&zone.map, &zone.hm, [p.x, p.z], along)
        })
        .filter(|s| nav.0.walkable(*s))
        .map(|s| zone.on_ground(s[0], s[1]))
        .take(rules::FIELDS)
        .collect();
    if sites.is_empty() {
        return;
    }
    info!("minelayer: out from the gate to {sites:.0?}");
    spawn_minelayer(&mut commands, &zone, &nav.0, gate, sites, gate);
}

/// The scenario: a field of mines across the way ahead, and a minelayer
/// beyond it. The bot finds the first, cuts its lead, goes round the
/// rest, and comes through unhurt.
pub const SCENARIO: &str = "minefield";
/// How far ahead the field lies, and how far past it the bot walks.
pub const STAGE_AHEAD: f32 = 10.0;
pub const STAGE_PAST: f32 = 14.0;
/// Filled in by the setup: where the bot walks to (past the field), and a
/// spot by the first mine it disarms.
pub const PAST_PLACEHOLDER: Vec2 = Vec2::new(-3e6, -3e6);
pub const MINE_PLACEHOLDER: Vec2 = Vec2::new(-4e6, -4e6);
pub const ROUND_PLACEHOLDER: Vec2 = Vec2::new(-5e6, -5e6);

#[allow(clippy::too_many_arguments)]
fn stage_scenario(
    mut commands: Commands,
    mut setups: MessageReader<crate::bot::scenarios::Setup>,
    zone: Res<Zone>,
    nav: Res<super::walker::Nav>,
    catalog: Res<crate::items::Catalog>,
    mut bots: Query<&mut crate::bot::Bot>,
    anomalies: Query<(&crate::anomaly::Anomaly, &GlobalTransform)>,
) {
    let clear = |p: Vec3| {
        anomalies.iter().all(|(a, g)| {
            let c = g.translation();
            Vec2::new(p.x - c.x, p.z - c.z).length() > a.kind.reach(a.radius) + 3.0
        })
    };
    for s in setups.read() {
        if s.name != SCENARIO && s.name != CONTROL {
            continue;
        }
        let f = Vec3::new(s.facing.x, 0.0, s.facing.z).normalize_or(Vec3::NEG_Z);
        let open = |d: Vec3| {
            let side = Vec3::new(-d.z, 0.0, d.x);
            (1..=((STAGE_AHEAD + STAGE_PAST) as usize + 2)).all(|m| {
                [-6.0, -3.0, 0.0, 3.0, 6.0].iter().all(|w| {
                    let p = s.at + d * m as f32 + side * *w;
                    nav.0.walkable([p.x, p.z]) && clear(p)
                })
            })
        };
        let dir = (0..16)
            .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * f)
            .find(|d| open(*d))
            .unwrap_or(f);
        let side = Vec3::new(-dir.z, 0.0, dir.x);
        // The field: right across the way, ready (armed), one of its
        // mines square in the bot's line.
        let site = s.at + dir * STAGE_AHEAD + side * (rules::SPACING * 0.5);
        let spots = rules::field([site.x, site.z], [dir.x, dir.z], rules::PER_FIELD);
        let mut first = None;
        for (k, p) in spots.iter().enumerate() {
            let at = zone.on_ground(p[0], p[1]);
            let m = lay_mine(&mut commands, at, k as f32 * 0.5);
            commands.queue(move |w: &mut World| {
                if let Some(mut l) = w.get_mut::<LaidMine>(m) {
                    l.mine.age = rules::ARM;
                }
            });
            // The one dead ahead (in the bot's line) is the first it meets.
            let off =
                (Vec2::new(at.x, at.z) - Vec2::new(s.at.x, s.at.z)).dot(Vec2::new(side.x, side.z));
            let nearer = match first {
                Some((_, o)) => off.abs() < f32::abs(o),
                None => true,
            };
            if nearer {
                first = Some((at, off));
            }
        }
        let (mine, _) = first.unwrap_or((site, 0.0));
        // Where it stands to cut it, a step short; then round the field's
        // end, then on past.
        let short = mine - dir * 1.0;
        let round = site + side * (rules::SPACING * rules::PER_FIELD as f32 * 0.5 + 2.5);
        let past = s.at + dir * (STAGE_AHEAD + STAGE_PAST);
        info!("scenario: a minefield at {site:.0}");
        for mut bot in &mut bots {
            for t in bot.tasks.iter_mut() {
                if let crate::bot::Task::WalkTo { to, .. } = t {
                    if *to == PAST_PLACEHOLDER {
                        *to = Vec2::new(past.x, past.z);
                    } else if *to == MINE_PLACEHOLDER {
                        *to = Vec2::new(short.x, short.z);
                    } else if *to == ROUND_PLACEHOLDER {
                        *to = Vec2::new(round.x, round.z);
                    }
                }
            }
        }
        // Its layer, beyond, going about it (it's in no hurry).
        let far = site + dir * (STAGE_PAST + 20.0);
        spawn_minelayer(
            &mut commands,
            &zone,
            &nav.0,
            zone.on_ground(far.x, far.z),
            vec![zone.on_ground(far.x, far.z) + side * 30.0],
            zone.on_ground(far.x, far.z) + dir * 30.0,
        );
        if let Some(kind) = catalog.kind("bolt_cutters") {
            let p = s.at + side * 0.7 + dir * 0.5;
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        zone.on_ground(p.x, p.z) + Vec3::Y * 0.1,
                    ),
                    condition: 1.0,
                },
            );
        }
    }
}

/// The control: the same field, walked straight through.
pub const CONTROL: &str = "minefield_control";

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Acts(Vec<MineAct2>, f32);

    fn collect(
        mut ev: MessageReader<MineEvent>,
        mut acts: ResMut<Acts>,
        mut hurt: MessageReader<crate::health::Hurt>,
    ) {
        acts.0.extend(ev.read().map(|e| e.what));
        acts.1 += hurt.read().map(|h| h.amount).sum::<f32>();
    }

    fn harness() -> (crate::vr_harness::Vr, Zone) {
        let mut vr = crate::vr_harness::Vr::new(extraction_core::map::TEST_FIELD_ID);
        vr.app.init_resource::<Acts>().add_systems(Update, collect);
        let zone = vr.app.world().resource::<Zone>().clone();
        (vr, zone)
    }

    fn mine_at(vr: &mut crate::vr_harness::Vr, at: Vec3, armed: bool) -> Entity {
        let mut c = vr.app.world_mut().commands();
        let m = lay_mine(&mut c, at, 0.0);
        vr.app.world_mut().flush();
        if armed {
            vr.app.world_mut().get_mut::<LaidMine>(m).unwrap().mine.age = rules::ARM;
        }
        m
    }

    /// Step on an armed mine: a click, a blast, and it hurts. A fresh one
    /// doesn't; nor does one you stand well clear of.
    #[test]
    fn a_step_on_it_hurts() {
        let (mut vr, zone) = harness();
        let at = zone.on_ground(40.0, 230.0);
        mine_at(&mut vr, at, true);
        vr.stand(at, 0.0);
        vr.step(90 * 2);
        let acts = vr.app.world().resource::<Acts>();
        assert!(acts.0.contains(&MineAct2::Click), "{:?}", acts.0);
        assert!(acts.0.contains(&MineAct2::Blast), "{:?}", acts.0);
        assert!(acts.1 > 40.0, "hurt: {}", acts.1);

        let (mut vr, zone) = harness();
        let at = zone.on_ground(40.0, 230.0);
        mine_at(&mut vr, at, false);
        vr.stand(at, 0.0);
        vr.step(90 * 2);
        assert!(
            !vr.app
                .world()
                .resource::<Acts>()
                .0
                .contains(&MineAct2::Blast),
            "fresh: not armed yet"
        );
    }

    /// Cut its lead and it's a safe mine to carry off.
    #[test]
    fn cut_its_lead_and_its_yours() {
        let (mut vr, zone) = harness();
        let at = zone.on_ground(40.0, 230.0);
        let m = mine_at(&mut vr, at, true);
        vr.stand(zone.on_ground(40.0, 232.0), 0.0);
        vr.step(3);
        let lead = vr.app.world().get::<Children>(m).unwrap()[0];
        vr.app
            .world_mut()
            .write_message(crate::destruction::hooks::Cut {
                target: lead,
                by: vr.player,
            });
        vr.step(10);
        assert!(vr.app.world().get_entity(m).is_err(), "the mine's gone");
        let catalog = vr.app.world().resource::<crate::items::Catalog>().clone();
        let safe = vr
            .app
            .world_mut()
            .query::<(&crate::items::Item, &GlobalTransform)>()
            .iter(vr.app.world())
            .any(|(i, g)| {
                catalog.get(i.kind()).id == "pressure_mine" && g.translation().distance(at) < 1.0
            });
        assert!(safe, "a safe mine lies there");
        // And standing on where it was now does nothing.
        vr.stand(at, 0.0);
        vr.step(90);
        let acts = vr.app.world().resource::<Acts>();
        assert!(!acts.0.contains(&MineAct2::Blast));
        assert_eq!(acts.1, 0.0);
    }

    /// A round into it from out of its reach sets it off harmlessly.
    #[test]
    fn shoot_it_from_afar() {
        let (mut vr, zone) = harness();
        let at = zone.on_ground(40.0, 230.0);
        let m = mine_at(&mut vr, at, true);
        vr.stand(zone.on_ground(40.0, 238.0), 0.0);
        vr.step(3);
        crate::robots::heavy::tests::hit(&mut vr.app, m, 30.0);
        vr.step(30);
        let acts = vr.app.world().resource::<Acts>();
        assert!(acts.0.contains(&MineAct2::Blast), "{:?}", acts.0);
        assert_eq!(acts.1, 0.0, "out of its reach");
    }

    /// Sent to a site, it crawls there and lays a field across the way;
    /// then it leaves.
    #[test]
    fn it_lays_a_field_across_the_way() {
        let (mut vr, zone) = harness();
        // Out of the way: no one for it to see.
        vr.stand(zone.on_ground(140.0, 330.0), 0.0);
        let nav = vr
            .app
            .world()
            .resource::<super::super::walker::Nav>()
            .0
            .clone();
        let from = zone.on_ground(40.0, 230.0);
        let site = zone.on_ground(40.0, 214.0);
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_minelayer(&mut c, &zone, &nav, from, vec![site], from);
            vr.app.world_mut().flush();
            e
        };
        vr.step(90 * 40);
        let mines: Vec<Vec3> = vr
            .app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<LaidMine>>()
            .iter(vr.app.world())
            .map(|g| g.translation())
            .collect();
        // The field, less any spot it may not lay in (a road, a doorway).
        let want = rules::site_field(
            &zone.map,
            &zone.hm,
            [site.x, site.z],
            [site.x - from.x, site.z - from.z],
        );
        assert!(want.len() >= 2, "a site with room: {want:?}");
        assert_eq!(mines.len(), want.len(), "{mines:?} vs {want:?}");
        for p in &mines {
            assert!(
                Vec2::new(p.x - site.x, p.z - site.z).length() < 5.0,
                "near its site: {p}"
            );
        }
        let left = vr.app.world().get::<Minelayer>(e).map(|m| m.hopper);
        assert!(left.is_none() || left == Some(rules::HOPPER - want.len() as u32));
    }

    /// Hole its hopper and it goes up with its mines.
    #[test]
    fn its_hopper_cooks_off() {
        let (mut vr, zone) = harness();
        vr.stand(zone.on_ground(40.0, 260.0), 0.0);
        let nav = vr
            .app
            .world()
            .resource::<super::super::walker::Nav>()
            .0
            .clone();
        let from = zone.on_ground(40.0, 230.0);
        let far = zone.on_ground(40.0, 160.0);
        let e = {
            let mut c = vr.app.world_mut().commands();
            let e = spawn_minelayer(&mut c, &zone, &nav, from, vec![far], far);
            vr.app.world_mut().flush();
            e
        };
        vr.step(5);
        let hopper = vr
            .app
            .world_mut()
            .query_filtered::<(Entity, &super::super::RobotPart), With<Hopper>>()
            .iter(vr.app.world())
            .find(|(_, p)| p.robot == e)
            .map(|(h, _)| h)
            .unwrap();
        let at = vr
            .app
            .world()
            .get::<GlobalTransform>(hopper)
            .unwrap()
            .translation();
        vr.app.world_mut().write_message(Hit {
            blow: None,
            entity: hopper,
            point: at,
            normal: Vec3::Y,
            velocity: Vec3::NEG_Z * 500.0,
            damage: HOPPER_HP * 3.0,
            shooter: None,
            mass_g: 8.0,
            load: Default::default(),
        });
        vr.step(20);
        let acts = vr.app.world().resource::<Acts>();
        assert!(acts.0.contains(&MineAct2::CookOff), "{:?}", acts.0);
        assert!(
            vr.app.world().get::<Robot>(e).unwrap().wrecked,
            "it went with them"
        );
        assert_eq!(acts.1, 0.0, "you were 30 m off");
    }
}
