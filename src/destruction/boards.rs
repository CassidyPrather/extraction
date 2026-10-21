//! Boarded-up windows: someone nailed planks over the glass (it went
//! first) and left. World models decides which sashes
//! ([`boarded_sashes`]) and draws the boards (`window_board`); each board
//! is its own breakable thing here: a plank to a round, pried off with a
//! crowbar at its ends, splintered by blows, saved broken until the Surge.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::wood::{Drawn, Quiet, Timber};
use super::{Broken, key};
use crate::render::materials::Mat;
use crate::world::buildings::{Building, Geo};
use crate::world::surfaces::Surfaces;

/// A board's size: long across the window, a hand wide, an inch thick.
pub const BOARD: Vec3 = Vec3::new(1.2, 0.15, 0.025);
/// How much a board takes: less than a door, more than a slat.
pub const BOARD_HP: f32 = 60.0;
/// Torque to lever one off at an end (nails, no latch).
pub const BOARD_PRY: f32 = 90.0;
/// The board model (`art/props/openings.py`).
pub const MODEL: &str = "window_board";

/// One board over a window.
#[derive(Component, Debug)]
pub struct WindowBoard;

/// Which of a building's sashes are boarded (in `glass::panes_of`'s
/// order). World models' decision (`world::decay::boarded_sashes`); a
/// boarded sash is always a weathered one, so its panes draw no glass.
pub fn boarded_sashes(prefab: &str, at: &Transform) -> Vec<bool> {
    crate::world::decay::boarded_sashes(prefab, at)
}

/// The boards over a building's boarded sashes, in building space:
/// three across each, outside, a little tilted (their place and lean
/// from World models' notes on the sash's space).
pub fn boards_of(geo: &Geo, boarded: &[bool]) -> Vec<Transform> {
    // The sash's centre plane is the wall's; boards sit against the
    // casing, 3 cm proud of a 0.18 m wall's outer face.
    const OUT: f32 = -(0.09 + 0.03 + 0.0125);
    let mut out = Vec::new();
    let mut sash = 0;
    for d in &geo.props {
        if d.models.is_empty() || d.models.iter().any(|(n, _)| *n != super::glass::SASH) {
            continue;
        }
        for (_, xf) in &d.models {
            let k = sash;
            sash += 1;
            if !boarded.get(k).copied().unwrap_or(false) {
                continue;
            }
            for (i, y) in [-0.3, 0.05, 0.4].into_iter().enumerate() {
                // A steady lean per board, so a rebuild picks the same.
                let tilt = ((k * 3 + i) as f32 * 2.399).sin() * 0.08;
                let local = Vec3::new(0.0, y * xf.scale.y, OUT);
                let mut t = Transform::from_translation(xf.translation + xf.rotation * local)
                    .with_rotation(xf.rotation * Quat::from_rotation_z(tilt));
                // Longer only for a wider window.
                t.scale = Vec3::new(xf.scale.x.max(1.0), 1.0, 1.0);
                out.push(t);
            }
        }
    }
    out
}

/// Nail a building's boards up (but not those broken since the Surge).
pub fn spawn_boards(
    commands: &mut Commands,
    building: &Building,
    bxf: &Transform,
    boarded: &[bool],
    broken: &Broken,
) {
    let Some(geo) = crate::world::buildings::prefab(&building.prefab) else {
        return;
    };
    for local in boards_of(&geo, boarded) {
        let size = BOARD * local.scale;
        let world = (*bxf * local.with_scale(Vec3::ONE)).with_scale(Vec3::ONE);
        let k = key(world.translation);
        if broken.0.contains(&k) {
            continue;
        }
        commands.spawn((
            (
                Name::new("Window board"),
                WindowBoard,
                world,
                Visibility::default(),
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                CollisionLayers::new(crate::world::Layer::World, LayerMask::ALL),
                Surfaces(vec![(size * 0.5, Transform::IDENTITY, Mat::Planks)]),
            ),
            Timber {
                hp: BOARD_HP,
                max: BOARD_HP,
                key: k,
                push: Vec3::ZERO,
            },
            super::hooks::Pryable {
                needs: BOARD_PRY,
                seam: Vec3::new(size.x * 0.5 - 0.06, 0.0, 0.0),
                along: Vec3::Y,
                held: 0.0,
            },
            Drawn {
                model: MODEL,
                at: Transform::from_scale(local.scale),
                size,
                range: crate::render::lod::near_range().end_margin,
            },
        ));
    }
}

pub struct BoardsPlugin;

impl Plugin for BoardsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(board_gone)
            .add_systems(FixedUpdate, nail_up.in_set(super::DestructionSystems));
    }
}

fn nail_up(
    mut commands: Commands,
    buildings: Query<(&Building, &Transform), Added<Building>>,
    broken: Res<Broken>,
) {
    for (b, t) in &buildings {
        let boarded = boarded_sashes(&b.prefab, t);
        if boarded.iter().any(|x| *x) {
            spawn_boards(&mut commands, b, t, &boarded, &broken);
        }
    }
}

/// A board gone (broken or pried off): it splinters, and stays gone.
fn board_gone(
    remove: On<Remove, WindowBoard>,
    boards: Query<(&GlobalTransform, &Collider, Option<&Timber>, Has<Quiet>)>,
    mut commands: Commands,
    mut pool: ResMut<super::debris::DebrisPool>,
    mut broken: ResMut<Broken>,
) {
    let Ok((gt, collider, timber, quiet)) = boards.get(remove.entity) else {
        return;
    };
    if quiet {
        return;
    }
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let half = collider
        .shape_scaled()
        .as_cuboid()
        .map_or(BOARD * 0.5, |c| Vec3::from(c.half_extents));
    let (_, rot, c) = gt.to_scale_rotation_translation();
    let push = timber
        .map(|t| t.push)
        .filter(|p| *p != Vec3::ZERO)
        .unwrap_or(rot * Vec3::NEG_Z * 1.5);
    let mut rng = extraction_core::noise::Rng::new(super::glass::seed_of(k) ^ 0xB0A2D);
    let seed = rng.next_u64();
    let pieces = super::wood::board_pieces(
        &mut rng,
        seed,
        c,
        rot,
        Vec2::new(half.x, half.y),
        half.z * 2.0,
        Mat::Planks,
        push,
        3,
    );
    super::debris::throw(&mut commands, &mut pool, pieces);
}

/// The Surge: every boarded window boarded again.
pub fn mend_all(world: &mut World) {
    let boards: Vec<Entity> = world
        .query_filtered::<Entity, With<WindowBoard>>()
        .iter(world)
        .collect();
    for e in boards {
        world.entity_mut(e).insert(Quiet);
        world.despawn(e);
    }
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
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let none = Broken::default();
        for (b, t) in &buildings {
            let boarded = boarded_sashes(&b.prefab, t);
            if boarded.iter().any(|x| *x) {
                spawn_boards(&mut commands, b, t, &boarded, &none);
            }
        }
    }
    queue.apply(world);
}
