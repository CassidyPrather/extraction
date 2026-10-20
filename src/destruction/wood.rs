//! Wood that gives way: plank doors and supply crates.
//!
//! Each has [`Timber`]: so much it can take before it breaks. Rounds and
//! blows (any [`Hit`]) wear it down: a round mostly just holes a door
//! (it counts at [`ROUND_ON_DOOR`]) where a blow or a blast takes it at
//! full weight; a crate's thin slats give way to a few rounds. A door's
//! leaf also says what it's made of ([`Surfaces`]), so rounds go through
//! a plank door as through a plank wall and stop in a steel one.
//!
//! Whatever takes a door or a crate away — broken here, or torn off its
//! hinges by a walker (`robots::doors`) — leaves splinters
//! ([`debris`](super::debris)) and stays gone until the Surge. A crate
//! broken open spills what was packed inside it: a roll of its table on
//! top of what was on its lid.

use bevy::prelude::*;
use extraction_core::fracture;
use extraction_core::noise::Rng;

use super::debris::{self, DebrisPool, Kind, Piece};
use super::{Broke, Broken, Stuff, key};
use crate::items::loot::{CrateLoot, SUPPLY_CRATE, SupplyCrate};
use crate::render::materials::Mat;
use crate::weapons::ballistics::Hit;
use crate::world::buildings::{Building, DoorDef, DoorLeaf};
use crate::world::surfaces::Surfaces;

/// How much a plank door takes (a rifle round's harm is 40-90).
pub const DOOR_HP: f32 = 260.0;
/// How much a supply crate takes.
pub const CRATE_HP: f32 = 110.0;
/// A round's share of its harm that tells on a door: it holes the
/// boards, it doesn't split them.
pub const ROUND_ON_DOOR: f32 = 0.5;

/// Breakable wood.
#[derive(Component, Clone, Debug)]
pub struct Timber {
    pub hp: f32,
    pub max: f32,
    /// Its identity in the save ([`key`]).
    pub key: [i32; 3],
    /// The way the last blow pushed (the splinters fly that way).
    pub push: Vec3,
}

/// Taken away without a sound or a splinter (restored broken from the
/// save, or swept and rebuilt by the Surge).
#[derive(Component)]
pub struct Quiet;

/// Doors broken down since the Surge: their building and how they hung,
/// to hang them again.
#[derive(Resource, Default, Debug)]
pub struct GoneDoors(pub Vec<(Entity, Entity, DoorDef)>);

pub struct WoodPlugin;

impl Plugin for WoodPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GoneDoors>()
            .add_observer(door_gone)
            .add_observer(crate_gone)
            .add_systems(
                FixedUpdate,
                (fit_doors, fit_crates, take_hits)
                    .chain()
                    .in_set(super::DestructionSystems),
            );
    }
}

/// A door's leaf, as rounds meet it: planks, or a steel door's plate.
fn leaf_mat(leaf: &DoorDef) -> Mat {
    if leaf.mat == Mat::Planks {
        Mat::Planks
    } else {
        Mat::DarkMetal
    }
}

/// Doors as they're hung: what the leaf is made of, and (planks) how
/// much it takes. One the save says was broken comes down again, quietly.
fn fit_doors(
    mut commands: Commands,
    doors: Query<(Entity, &DoorLeaf, &ChildOf, Option<&Children>), Added<DoorLeaf>>,
    buildings: Query<&Transform, With<Building>>,
    broken: Res<Broken>,
) {
    for (e, leaf, parent, children) in &doors {
        let d = leaf.0;
        let half = Vec3::new(d.width, d.height, 0.05) * 0.5;
        for c in children.into_iter().flatten() {
            commands
                .entity(*c)
                .insert(Surfaces(vec![(half, Transform::IDENTITY, leaf_mat(&d))]));
        }
        if d.mat != Mat::Planks {
            continue;
        }
        let at = buildings
            .get(parent.parent())
            .map_or(d.hinge, |t| t.transform_point(d.hinge));
        let k = key(at);
        if broken.0.contains(&k) {
            commands.entity(e).insert(Quiet).despawn();
            continue;
        }
        commands.entity(e).insert(Timber {
            hp: DOOR_HP,
            max: DOOR_HP,
            key: k,
            push: Vec3::ZERO,
        });
    }
}

/// Supply crates: planks to a round, and how much they take.
fn fit_crates(
    mut commands: Commands,
    crates: Query<(Entity, &Transform), Added<SupplyCrate>>,
    broken: Res<Broken>,
) {
    for (e, t) in &crates {
        let k = key(t.translation);
        if broken.0.contains(&k) {
            commands.entity(e).insert(Quiet).despawn();
            continue;
        }
        commands.entity(e).insert((
            Surfaces(vec![(SUPPLY_CRATE * 0.5, Transform::IDENTITY, Mat::Planks)]),
            Timber {
                hp: CRATE_HP,
                max: CRATE_HP,
                key: k,
                push: Vec3::ZERO,
            },
        ));
    }
}

/// Rounds and blows wear wood down; past its last, it breaks.
fn take_hits(
    mut commands: Commands,
    mut hits: MessageReader<Hit>,
    mut timber: Query<(&mut Timber, &GlobalTransform, Has<DoorLeaf>)>,
    parents: Query<&ChildOf>,
    mut broken: ResMut<Broken>,
    mut out: MessageWriter<Broke>,
) {
    for h in hits.read() {
        // The leaf's collider is a child of the door.
        let Some(e) = std::iter::once(h.entity)
            .chain(parents.get(h.entity).ok().map(|c| c.parent()))
            .find(|e| timber.contains(*e))
        else {
            continue;
        };
        let Ok((mut t, gt, door)) = timber.get_mut(e) else {
            continue;
        };
        if t.hp <= 0.0 {
            continue;
        }
        let share = if h.mass_g > 0.0 && door {
            ROUND_ON_DOOR
        } else {
            1.0
        };
        t.hp -= h.damage * share;
        t.push = h.velocity.normalize_or_zero() * if h.mass_g > 0.0 { 2.0 } else { 3.5 };
        if t.hp <= 0.0 {
            broken.0.insert(t.key);
            out.write(Broke {
                at: gt.translation(),
                stuff: Stuff::Wood,
                size: if door { 2.0 } else { 0.5 },
            });
            commands.entity(e).despawn();
        }
    }
}

/// A board's splinters as pieces: `half` its half length (along the
/// board, `along` in the world) and half width, laid flat in a frame
/// whose z is the board's thickness.
#[allow(clippy::too_many_arguments)]
fn board_pieces(
    rng: &mut Rng,
    seed: u64,
    centre: Vec3,
    frame: Quat,
    half: Vec2,
    thick: f32,
    mat: Mat,
    push: Vec3,
    n: usize,
) -> Vec<Piece> {
    fracture::splinter(half.to_array(), seed, n)
        .into_iter()
        .map(|s| {
            let at = centre + frame * Vec3::new(s.at[0], s.at[1], 0.0);
            let (velocity, spin) = debris::fling(rng, at, centre, push);
            Piece {
                kind: Kind::Wood(mat),
                poly: s.poly,
                thick,
                at: Transform::from_translation(at).with_rotation(frame),
                velocity,
                spin,
            }
        })
        .collect()
}

/// A door gone, however it went: splinters (unless quietly), and noted
/// to be broken until the Surge.
fn door_gone(
    remove: On<Remove, DoorLeaf>,
    doors: Query<(
        &DoorLeaf,
        &GlobalTransform,
        &ChildOf,
        Option<&Timber>,
        Has<Quiet>,
    )>,
    mut commands: Commands,
    mut pool: ResMut<DebrisPool>,
    mut broken: ResMut<Broken>,
    mut gone: ResMut<GoneDoors>,
) {
    let Ok((leaf, gt, parent, timber, quiet)) = doors.get(remove.entity) else {
        return;
    };
    let d = leaf.0;
    if d.mat != Mat::Planks {
        return;
    }
    gone.0.push((remove.entity, parent.parent(), d));
    if quiet {
        return;
    }
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let (_, rot, hinge) = gt.to_scale_rotation_translation();
    // Pushed in (a walker's shoulder) unless a blow said otherwise.
    let push = timber
        .map(|t| t.push)
        .filter(|p| *p != Vec3::ZERO)
        .unwrap_or(rot * Vec3::Z * 2.5);
    // Six boards, as the door is made, each in two or three splinters;
    // the board runs up the door, so its length is the frame's x.
    let frame = rot * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let mut rng = Rng::new(super::glass::seed_of(k));
    let bw = d.width / 6.0;
    let mut pieces = Vec::new();
    for b in 0..6 {
        let centre = hinge + rot * Vec3::new(bw * (b as f32 + 0.5), d.height * 0.5 + 0.01, 0.0);
        let seed = rng.next_u64();
        pieces.extend(board_pieces(
            &mut rng,
            seed,
            centre,
            frame,
            Vec2::new(d.height * 0.5, bw * 0.5),
            0.03,
            Mat::Planks,
            push,
            3,
        ));
    }
    debris::throw(&mut commands, &mut pool, pieces);
}

/// A crate gone: its slats fly, and what was packed in it spills out.
#[allow(clippy::too_many_arguments)]
fn crate_gone(
    remove: On<Remove, SupplyCrate>,
    crates: Query<(
        &GlobalTransform,
        Option<&Timber>,
        Option<&CrateLoot>,
        Has<Quiet>,
    )>,
    mut commands: Commands,
    mut pool: ResMut<DebrisPool>,
    mut broken: ResMut<Broken>,
    loot: Option<Res<crate::items::loot::Loot>>,
    catalog: Res<crate::items::Catalog>,
) {
    let Ok((gt, timber, table, quiet)) = crates.get(remove.entity) else {
        return;
    };
    if quiet {
        return;
    }
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let (_, rot, c) = gt.to_scale_rotation_translation();
    let push = timber.map_or(Vec3::ZERO, |t| t.push);
    let mut rng = Rng::new(super::glass::seed_of(k) ^ 0xC8A7E);
    let s = SUPPLY_CRATE * 0.5;
    // Its four sides and lid, a slat or two each, splintered.
    let faces = [
        (
            Vec3::new(0.0, 0.0, s.z),
            Quat::IDENTITY,
            Vec2::new(s.x, s.y),
        ),
        (
            Vec3::new(0.0, 0.0, -s.z),
            Quat::IDENTITY,
            Vec2::new(s.x, s.y),
        ),
        (
            Vec3::new(s.x, 0.0, 0.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec2::new(s.z, s.y),
        ),
        (
            Vec3::new(-s.x, 0.0, 0.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec2::new(s.z, s.y),
        ),
        (
            Vec3::new(0.0, s.y, 0.0),
            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            Vec2::new(s.x, s.z),
        ),
    ];
    let mut pieces = Vec::new();
    for (off, turn, half) in faces {
        // Each face is two slats.
        for slat in [-0.5, 0.5] {
            let centre = c + rot * (off + turn * Vec3::new(0.0, slat * half.y, 0.0));
            let out = (rot * off).normalize_or_zero();
            let seed = rng.next_u64();
            pieces.extend(board_pieces(
                &mut rng,
                seed,
                centre,
                rot * turn,
                Vec2::new(half.x, half.y * 0.5),
                0.02,
                Mat::Planks,
                push + out * 1.5,
                2,
            ));
        }
    }
    debris::throw(&mut commands, &mut pool, pieces);
    // What was packed inside.
    let (Some(loot), Some(table)) = (loot, table) else {
        return;
    };
    for (i, drop) in loot
        .0
        .roll(&table.0, &catalog, &mut rng)
        .into_iter()
        .enumerate()
    {
        let off = Vec3::new(
            (i as f32 * 1.9).sin() * 0.25,
            -0.2 + 0.08 * i as f32,
            (i as f32 * 1.9).cos() * 0.15,
        );
        crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind: drop.kind,
                transform: Transform::from_translation(c + rot * off),
                condition: drop.condition,
            },
        );
    }
}

/// The Surge: broken doors hung again, crates packed and nailed again.
pub fn mend_all(world: &mut World) {
    let gone = std::mem::take(&mut world.resource_mut::<GoneDoors>().0);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut swaps = Vec::new();
    {
        let mut commands = Commands::new(&mut queue, world);
        for (old, building, d) in gone {
            if commands.get_entity(building).is_err() {
                continue;
            }
            let new = crate::world::buildings::spawn_door(&mut commands, building, d);
            swaps.push((old, new));
        }
    }
    queue.apply(world);
    if let Some(mut entrances) = world.get_resource_mut::<crate::robots::doors::Entrances>() {
        for (old, new) in swaps {
            for e in entrances.doors.iter_mut().filter(|e| e.door == old) {
                e.door = new;
            }
        }
    }
    // Crates: every one rebuilt where it stood, so none is doubled.
    let crates: Vec<Entity> = world
        .query_filtered::<Entity, With<SupplyCrate>>()
        .iter(world)
        .collect();
    for e in crates {
        world.entity_mut(e).insert(Quiet);
        world.despawn(e);
    }
    if let Some(zone) = world.get_resource::<crate::world::Zone>().cloned() {
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            crate::items::loot::spawn_supply_crates(&mut commands, &zone);
        }
        queue.apply(world);
    }
}
