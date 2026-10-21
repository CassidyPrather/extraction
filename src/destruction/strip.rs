//! Parts you can prise off a wreck for scrap and cover: a bus's folding
//! door, a side panel, a seat. Whoever places a wreck (World models)
//! spawns its parts with [`spawn_part`]; each is a static piece of the
//! wreck until a crowbar levers it off (`hooks::Pried`), or, for a wooden
//! part, until it's broken. Then it comes away: what it yields (scrap, a
//! panel you can carry) drops where it was, and it stays gone until the
//! Surge puts the wreck back together.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::wood::{Drawn, Quiet, Timber};
use super::{Broken, key};
use crate::render::materials::Mat;
use crate::world::surfaces::Surfaces;

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
}

/// Every part placed this session, to put back after the Surge.
#[derive(Resource, Default, Debug)]
pub struct Parts(pub Vec<Part>);

pub struct StripPlugin;

impl Plugin for StripPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Parts>().add_observer(part_gone);
    }
}

/// Put a part on a wreck (unless it came off since the Surge). Metal
/// comes off only to a crowbar (rounds hole sheet, they don't tear a door
/// off its hinges); wood breaks too.
pub fn spawn_part(commands: &mut Commands, parts: &mut Parts, broken: &Broken, part: Part) {
    parts.0.push(part.clone());
    place(commands, broken, &part);
}

fn place(commands: &mut Commands, broken: &Broken, part: &Part) {
    let k = key(part.at.translation);
    if broken.0.contains(&k) {
        return;
    }
    let wooden =
        crate::weapons::impacts::kind_of(part.mat) == crate::weapons::impacts::ImpactKind::Wood;
    let hp = if wooden { 80.0 } else { f32::MAX };
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
    if let Some(model) = part.model {
        e.insert(Drawn {
            model,
            at: Transform::IDENTITY,
            size: s,
            range: crate::render::lod::near_range().end_margin,
        });
    }
}

/// A part come off: what it yields drops where it was.
fn part_gone(
    remove: On<Remove, Strippable>,
    parts: Query<(&Strippable, &GlobalTransform, Option<&Timber>, Has<Quiet>)>,
    mut commands: Commands,
    mut broken: ResMut<Broken>,
    catalog: Res<crate::items::Catalog>,
) {
    let Ok((s, gt, timber, quiet)) = parts.get(remove.entity) else {
        return;
    };
    if quiet {
        return;
    }
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let (_, rot, c) = gt.to_scale_rotation_translation();
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

/// The Surge: every wreck whole again.
pub fn mend_all(world: &mut World) {
    let on: Vec<Entity> = world
        .query_filtered::<Entity, With<Strippable>>()
        .iter(world)
        .collect();
    for e in on {
        world.entity_mut(e).insert(Quiet);
        world.despawn(e);
    }
    let parts = world.resource::<Parts>().0.clone();
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
