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
    /// Its scale, for the model (the box is already `size`).
    pub scale: Vec3,
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
        };
        Part {
            at: (*bxf * b.at.with_scale(Vec3::ONE)).with_scale(Vec3::ONE),
            size: b.size,
            mat: b.mat,
            yields: b.yields.map(|y| (y.to_string(), 1)).into_iter().collect(),
            pry,
            model: b.model,
            scale: b.at.scale,
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
        at: Transform::from_scale(part.scale),
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
            let off = Vec3::new((i as f32 * 1.7).sin() * 0.2, 0.1 + 0.05 * i as f32, 0.0);
            crate::items::spawn_item(
                &mut commands,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(c + rot * off),
                    condition: 0.6,
                },
            );
            i += 1;
        }
    }
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
