//! Parts that come off a building or a wreck: a window's shutters, a
//! garden fence's panels, a bus's folding door and side panels. World
//! models lays them out (`Geo::breakables`) and [`fit_breakables`] puts
//! them up; anything else can place one with [`spawn_part`]. Each is a
//! static piece of what it's on until a crowbar levers it off
//! (`hooks::Pried`), or, for a wooden part, until it's broken. Then it
//! comes away: wood splinters, and what it yields (scrap, a panel you can
//! carry) drops where it was. It stays gone until the Surge puts it back.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::wood::{Drawn, Quiet, Timber};
use super::{Broken, key};
use crate::render::materials::Mat;
use crate::world::buildings::{BreakKind, Breakable, Building};
use crate::world::surfaces::Surfaces;

/// How much a shutter takes, and the torque to lever it off its hinges.
pub const SHUTTER_HP: f32 = 60.0;
pub const SHUTTER_PRY: f32 = 90.0;
/// A fence panel: thicker planks, nailed to two posts.
pub const FENCE_HP: f32 = 100.0;
pub const FENCE_PRY: f32 = 150.0;
/// A bus's door and panels: riveted sheet, a crowbar's work.
pub const BUS_PRY: f32 = 200.0;
/// A car's door off its rusted hinges; a bonnet or boot lid off theirs; a
/// wheel off its seized hub.
pub const CAR_DOOR_PRY: f32 = 180.0;
pub const CAR_LID_PRY: f32 = 140.0;
pub const CAR_WHEEL_PRY: f32 = 160.0;
/// A wooden part's hit points when nothing says otherwise.
pub const PART_HP: f32 = 80.0;

/// A part of a wreck that comes off, and what it yields: item ids and
/// how many of each.
#[derive(Component, Clone, Debug)]
pub struct Strippable {
    pub yields: Vec<(String, u32)>,
}

/// One part as it was placed (kept to put it back after the Surge).
#[derive(Clone, Debug)]
pub struct Part {
    pub at: Transform,
    pub size: Vec3,
    pub mat: Mat,
    pub yields: Vec<(String, u32)>,
    /// Torque to lever it off, N·m.
    pub pry: f32,
    /// Its model, drawn at the building's range (a box of its size until
    /// it loads, or for good if `None`).
    pub model: Option<&'static str>,
    /// Its model's pose in its own space (its scale; or, for a car's
    /// part, the way back to the car's space its model is authored in).
    pub model_at: Transform,
    /// What it takes to break; `None` for metal, which only comes off to
    /// a crowbar (rounds hole sheet, they don't tear a door off).
    pub hp: Option<f32>,
}

impl Part {
    /// A building's breakable, placed by the building's transform.
    pub fn of(b: &Breakable, bxf: &Transform) -> Self {
        let (hp, pry) = match b.kind {
            BreakKind::Shutter => (Some(SHUTTER_HP), SHUTTER_PRY),
            BreakKind::FencePanel => (Some(FENCE_HP), FENCE_PRY),
            BreakKind::BusDoor | BreakKind::BusPanel => (None, BUS_PRY),
            BreakKind::CarDoor => (None, CAR_DOOR_PRY),
            BreakKind::CarBonnet | BreakKind::CarBoot => (None, CAR_LID_PRY),
            BreakKind::CarWheel => (None, CAR_WHEEL_PRY),
        };
        // A car's part is modelled in the car's space: back from its box
        // there. (Not a part we know: drawn as its box would be.)
        let model_at = match b.model.filter(|_| b.kind.in_car_space()) {
            Some(drawn) => car_model_at(drawn).unwrap_or_else(|| {
                warn!("{drawn}: not a car part we know");
                Transform::IDENTITY
            }),
            None => Transform::from_scale(b.at.scale),
        };
        Part {
            at: (*bxf * b.at.with_scale(Vec3::ONE)).with_scale(Vec3::ONE),
            size: b.size,
            mat: b.mat,
            yields: b.yields.map(|y| (y.to_string(), 1)).into_iter().collect(),
            pry,
            model: b.model,
            model_at,
            hp,
        }
    }
}

/// Every part placed this session, to put back after the Surge.
#[derive(Resource, Default, Debug)]
pub struct Parts(pub Vec<Part>);

pub struct StripPlugin;

impl Plugin for StripPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Parts>()
            .add_observer(part_gone)
            .add_systems(
                FixedUpdate,
                fit_breakables.in_set(super::DestructionSystems),
            );
    }
}

/// One separable part of a car wreck's model: `model#node`, and its box in
/// the car's space (from the model's own bounds: centre, size, metres).
struct CarNode(&'static str, [f32; 3], [f32; 3]);

/// Kit's car wrecks (`art/vehicles/wrecks.py`): each a model whose
/// separable parts are its top-level nodes, in the car's space (floor
/// under the wheelbase's middle, front to -Z, +X its right side). The
/// boxes are the nodes' own bounds, read from the models.
const CARS: &[(&str, &[CarNode])] = &[
    (
        "wreck_lada",
        &[
            CarNode(
                "wreck_lada#bonnet",
                [0.0, 0.843, -1.493],
                [1.52, 0.035, 1.015],
            ),
            CarNode("wreck_lada#boot", [0.0, 0.852, 1.63], [1.52, 0.035, 0.72]),
            CarNode(
                "wreck_lada#door_fl",
                [-0.766, 0.83, -0.467],
                [0.092, 0.99, 0.995],
            ),
            CarNode(
                "wreck_lada#door_fr",
                [0.766, 0.83, -0.467],
                [0.092, 0.99, 0.995],
            ),
            CarNode(
                "wreck_lada#door_rl",
                [-0.766, 0.83, 0.527],
                [0.092, 0.99, 0.855],
            ),
            CarNode(
                "wreck_lada#door_rr",
                [0.766, 0.83, 0.527],
                [0.092, 0.99, 0.855],
            ),
            CarNode(
                "wreck_lada#wheel_fl",
                [-0.68, 0.27, -1.21],
                [0.17, 0.58, 0.58],
            ),
            CarNode(
                "wreck_lada#wheel_fr",
                [0.68, 0.27, -1.21],
                [0.17, 0.58, 0.58],
            ),
            CarNode(
                "wreck_lada#wheel_rl",
                [-0.68, 0.27, 1.21],
                [0.17, 0.58, 0.58],
            ),
            CarNode(
                "wreck_lada#wheel_rr",
                [0.68, 0.27, 1.21],
                [0.17, 0.58, 0.58],
            ),
        ],
    ),
    (
        "wreck_uaz",
        &[
            CarNode("wreck_uaz#bonnet", [0.0, 1.1, -1.41], [1.35, 0.06, 1.06]),
            CarNode(
                "wreck_uaz#door_fl",
                [-0.841, 1.134, -0.39],
                [0.102, 0.958, 0.9],
            ),
            CarNode(
                "wreck_uaz#door_fr",
                [0.841, 1.134, -0.39],
                [0.102, 0.958, 0.9],
            ),
            CarNode(
                "wreck_uaz#door_rl",
                [-0.841, 1.134, 0.455],
                [0.102, 0.958, 0.69],
            ),
            CarNode(
                "wreck_uaz#door_rr",
                [0.841, 1.134, 0.455],
                [0.102, 0.958, 0.69],
            ),
            CarNode("wreck_uaz#tailgate", [0.0, 0.94, 1.885], [1.54, 0.78, 0.29]),
            CarNode(
                "wreck_uaz#wheel_fl",
                [-0.72, 0.37, -1.19],
                [0.24, 0.78, 0.78],
            ),
            CarNode(
                "wreck_uaz#wheel_fr",
                [0.72, 0.37, -1.19],
                [0.24, 0.78, 0.78],
            ),
            CarNode(
                "wreck_uaz#wheel_rl",
                [-0.72, 0.37, 1.19],
                [0.24, 0.78, 0.78],
            ),
            CarNode("wreck_uaz#wheel_rr", [0.72, 0.37, 1.19], [0.24, 0.78, 0.78]),
        ],
    ),
    (
        "wreck_lorry",
        &[
            CarNode(
                "wreck_lorry#bonnet",
                [0.0, 1.235, -1.955],
                [1.124, 0.47, 1.4],
            ),
            CarNode(
                "wreck_lorry#door_fl",
                [-0.966, 1.462, -0.505],
                [0.092, 0.964, 0.95],
            ),
            CarNode(
                "wreck_lorry#door_fr",
                [0.966, 1.462, -0.505],
                [0.092, 0.964, 0.95],
            ),
            CarNode(
                "wreck_lorry#tailboard",
                [0.0, 1.44, 3.536],
                [2.31, 0.53, 0.092],
            ),
            CarNode(
                "wreck_lorry#wheel_fl",
                [-0.8, 0.43, -1.65],
                [0.24, 0.9, 0.9],
            ),
            CarNode("wreck_lorry#wheel_fr", [0.8, 0.43, -1.65], [0.24, 0.9, 0.9]),
            CarNode(
                "wreck_lorry#wheel_rl",
                [-0.86, 0.43, 1.65],
                [0.53, 0.9, 0.9],
            ),
            CarNode("wreck_lorry#wheel_rr", [0.86, 0.43, 1.65], [0.53, 0.9, 0.9]),
        ],
    ),
];

/// A car wreck's body as boxes (min, max corners, car space), read off
/// its model: a chassis between the wheels; a hull above them inside the
/// doors' inner faces, up to the bonnet's and boot's undersides; the cab
/// between the bonnet's hinge and the boot (or to the cab's back); and a
/// lorry's bed. Nothing of it stands over a part, so a round or a hand
/// meets the door, not the body round it, and there's no wall in the air
/// over a bonnet.
const BODIES: &[(&str, &[([f32; 3], [f32; 3])])] = &[
    (
        "wreck_lada",
        &[
            ([-0.595, 0.205, -2.15], [0.595, 0.56, 2.15]),
            ([-0.72, 0.56, -2.15], [0.72, 0.825, 2.15]),
            ([-0.72, 0.825, -0.985], [0.72, 1.39, 1.27]),
        ],
    ),
    (
        "wreck_uaz",
        &[
            ([-0.6, 0.24, -2.05], [0.6, 0.76, 1.74]),
            ([-0.79, 0.76, -2.05], [0.79, 1.07, 1.74]),
            ([-0.79, 1.07, -0.88], [0.79, 1.61, 1.74]),
        ],
    ),
    (
        "wreck_lorry",
        &[
            ([-0.595, 0.26, -2.81], [0.595, 0.88, 3.49]),
            ([-0.92, 0.88, -2.81], [0.92, 1.0, 3.49]),
            ([-0.92, 1.0, -1.255], [0.92, 2.17, 0.685]),
            ([-1.15, 0.88, 0.685], [1.15, 1.12, 3.49]),
        ],
    ),
];

/// A car wreck's body boxes in the car's space (centre, size): for the
/// prefab's collider.
pub fn car_body(model: &str) -> Vec<(Vec3, Vec3)> {
    BODIES
        .iter()
        .find(|(m, _)| *m == model)
        .map_or(Vec::new(), |(_, boxes)| {
            boxes
                .iter()
                .map(|(lo, hi)| {
                    let (lo, hi) = (Vec3::from(*lo), Vec3::from(*hi));
                    ((lo + hi) * 0.5, hi - lo)
                })
                .collect()
        })
}

/// A car wreck's parts as a building's breakables, the car standing at
/// `at` in the building's space: a door or a wheel faces out of its side,
/// a bonnet or a boot lid up. For the prefab that places the car (it
/// draws only the model's `body` node, and collides only the body).
pub fn car_parts(model: &'static str, at: Transform) -> Vec<Breakable> {
    let Some((_, nodes)) = CARS.iter().find(|(m, _)| *m == model) else {
        return Vec::new();
    };
    nodes
        .iter()
        .map(|n| {
            let (local, size) = car_node_pose(n);
            let node = super::wood::model_node(n.0).1.unwrap_or_default();
            let (kind, mat, yields) = match node.split('_').next().unwrap_or_default() {
                "door" => (BreakKind::CarDoor, Mat::PaintedMetal, "car_door"),
                "wheel" => (BreakKind::CarWheel, Mat::RustyMetal, "car_wheel"),
                "bonnet" => (BreakKind::CarBonnet, Mat::PaintedMetal, "car_bonnet"),
                // A boot lid, a tailgate, a tailboard.
                _ => (BreakKind::CarBoot, Mat::PaintedMetal, "car_bonnet"),
            };
            Breakable {
                kind,
                at: at * local,
                size,
                model: Some(n.0),
                mat,
                yields: Some(yields),
            }
        })
        .collect()
}

/// A car part's box in the car's space, and its size: its outer face to
/// local -Z. It faces out along its thinnest side, the way it sits from
/// the car's middle: a door or a wheel out of its side, a bonnet or a boot
/// lid up, a tailgate back.
fn car_node_pose(CarNode(_, c, s): &CarNode) -> (Transform, Vec3) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (c, s) = (Vec3::from(*c), Vec3::from(*s));
    let (rot, size) = if s.y <= s.x.min(s.z) {
        (Quat::from_rotation_x(FRAC_PI_2), Vec3::new(s.x, s.z, s.y))
    } else if s.x <= s.z {
        let out = if c.x < 0.0 { FRAC_PI_2 } else { -FRAC_PI_2 };
        (Quat::from_rotation_y(out), Vec3::new(s.z, s.y, s.x))
    } else if c.z > 0.0 {
        (Quat::from_rotation_y(PI), s)
    } else {
        (Quat::IDENTITY, s)
    };
    (Transform::from_translation(c).with_rotation(rot), size)
}

/// Where a car part's model goes on the part: back from its box to the
/// car's space its node is modelled in. `None` for a part not in
/// [`CARS`].
fn car_model_at(drawn: &str) -> Option<Transform> {
    let n = CARS
        .iter()
        .flat_map(|(_, nodes)| nodes.iter())
        .find(|n| n.0 == drawn)?;
    let (local, _) = car_node_pose(n);
    let back = local.rotation.inverse();
    Some(Transform::from_translation(-(back * local.translation)).with_rotation(back))
}

/// Put up a new building's shutters, fence panels and wreck parts (but
/// not those gone since the Surge).
pub fn fit_breakables(
    mut commands: Commands,
    buildings: Query<(&Building, &Transform), Added<Building>>,
    broken: Res<Broken>,
) {
    for (b, t) in &buildings {
        for p in parts_of(b, t) {
            place(&mut commands, &broken, &p);
        }
    }
}

/// A building's breakable parts, in the world.
pub fn parts_of(building: &Building, bxf: &Transform) -> Vec<Part> {
    crate::world::buildings::prefab(&building.prefab).map_or(Vec::new(), |g| {
        g.breakables.iter().map(|b| Part::of(b, bxf)).collect()
    })
}

/// Put a part on something (unless it came off since the Surge); kept to
/// put back after the Surge.
pub fn spawn_part(commands: &mut Commands, parts: &mut Parts, broken: &Broken, part: Part) {
    parts.0.push(part.clone());
    place(commands, broken, &part);
}

fn place(commands: &mut Commands, broken: &Broken, part: &Part) {
    let k = key(part.at.translation);
    if broken.0.contains(&k) {
        return;
    }
    let hp = part.hp.unwrap_or(f32::MAX);
    let s = part.size;
    let mut e = commands.spawn((
        (
            Name::new("Wreck part"),
            Strippable {
                yields: part.yields.clone(),
            },
            part.at,
            Visibility::default(),
            RigidBody::Static,
            Collider::cuboid(s.x, s.y, s.z),
            CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
            Surfaces(vec![(s * 0.5, Transform::IDENTITY, part.mat)]),
        ),
        Timber {
            hp,
            max: hp,
            key: k,
            push: Vec3::ZERO,
        },
        super::hooks::Pryable {
            needs: part.pry,
            seam: Vec3::new(s.x * 0.5 - 0.03, 0.0, 0.0),
            along: Vec3::Y,
            held: 0.0,
        },
    ));
    e.insert(Drawn {
        model: part.model,
        mat: part.mat,
        at: part.model_at,
        size: s,
        range: crate::render::lod::near_range().end_margin,
    });
    if part.hp.is_some() {
        e.insert(Splinters(part.mat));
    }
}

/// A wooden part: it splinters when it goes.
#[derive(Component, Clone, Copy, Debug)]
pub struct Splinters(pub Mat);

/// A part come off: wood splinters, and what it yields drops where it
/// was.
fn part_gone(
    remove: On<Remove, Strippable>,
    parts: Query<(
        &Strippable,
        &GlobalTransform,
        &Collider,
        Option<&Timber>,
        Option<&Splinters>,
        Has<Quiet>,
    )>,
    mut commands: Commands,
    mut broken: ResMut<Broken>,
    mut pool: ResMut<super::debris::DebrisPool>,
    catalog: Res<crate::items::Catalog>,
) {
    let Ok((s, gt, collider, timber, splinters, quiet)) = parts.get(remove.entity) else {
        return;
    };
    if quiet {
        return;
    }
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let (_, rot, c) = gt.to_scale_rotation_translation();
    if let (Some(Splinters(mat)), Some(cuboid)) = (splinters, collider.shape_scaled().as_cuboid()) {
        let half = Vec3::from(cuboid.half_extents);
        let push = timber
            .map(|t| t.push)
            .filter(|p| *p != Vec3::ZERO)
            .unwrap_or(rot * Vec3::NEG_Z * 1.5);
        let mut rng = extraction_core::noise::Rng::new(super::glass::seed_of(k) ^ 0x5717);
        let seed = rng.next_u64();
        // Its long side across, its short side along the grain.
        let (across, along) = (half.x.max(half.y), half.x.min(half.y));
        let lie = if half.y > half.x {
            rot * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)
        } else {
            rot
        };
        let n = ((across * 2.0 / 0.4).round() as usize).clamp(2, 6);
        let pieces = super::wood::board_pieces(
            &mut rng,
            seed,
            c,
            lie,
            Vec2::new(across, along),
            half.z * 2.0,
            *mat,
            push,
            n,
        );
        super::debris::throw(&mut commands, &mut pool, pieces);
    }
    let mut i = 0;
    for (id, n) in &s.yields {
        let Some(kind) = catalog.kind(id) else {
            warn!("wreck part yields unknown item {id}");
            continue;
        };
        for _ in 0..*n {
            // Out past its face (-Z), clear of what it was fixed to.
            let out = half_z(collider) + 0.2;
            let off = Vec3::new((i as f32 * 1.7).sin() * 0.2, 0.1 + 0.05 * i as f32, -out);
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(c + rot * off)
                        .with_rotation(lying_flat(rot, catalog.get(kind).dims)),
                    condition: 0.6,
                },
            );
            i += 1;
        }
    }
}

/// A dropped thing lying on its largest face (a door, a panel, a wheel
/// that came off flat, never left on its edge), turned the way the part
/// it came from was.
pub fn lying_flat(part: Quat, dims: Option<[f32; 3]>) -> Quat {
    use std::f32::consts::FRAC_PI_2;
    let fwd = part * Vec3::NEG_Z;
    let heading = Quat::from_rotation_y(fwd.x.atan2(fwd.z) + std::f32::consts::PI);
    let Some(d) = dims else {
        return heading;
    };
    // Its thinnest side up.
    let flat = if d[0] <= d[1] && d[0] <= d[2] {
        Quat::from_rotation_z(FRAC_PI_2)
    } else if d[2] <= d[1] {
        Quat::from_rotation_x(FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    heading * flat
}

fn half_z(collider: &Collider) -> f32 {
    collider
        .shape_scaled()
        .as_cuboid()
        .map_or(0.05, |c| c.half_extents.z)
}

/// The Surge: every shutter, panel and wreck whole again.
pub fn mend_all(world: &mut World) {
    let on: Vec<Entity> = world
        .query_filtered::<Entity, With<Strippable>>()
        .iter(world)
        .collect();
    for e in on {
        world.entity_mut(e).insert(Quiet);
        world.despawn(e);
    }
    let mut parts = world.resource::<Parts>().0.clone();
    let buildings: Vec<(Building, Transform)> = world
        .query::<(&Building, &Transform)>()
        .iter(world)
        .map(|(b, t)| {
            (
                Building {
                    prefab: b.prefab.clone(),
                },
                *t,
            )
        })
        .collect();
    for (b, t) in &buildings {
        parts.extend(parts_of(b, t));
    }
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let none = Broken::default();
        for p in &parts {
            place(&mut commands, &none, p);
        }
    }
    queue.apply(world);
}
