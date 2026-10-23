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
            .add_observer(building_crate_gone)
            .add_systems(
                FixedUpdate,
                (
                    fit_doors,
                    fit_crates,
                    fit_building_crates,
                    blasts,
                    take_hits,
                )
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
        commands.entity(e).insert((
            Timber {
                hp: DOOR_HP,
                max: DOOR_HP,
                key: k,
                push: Vec3::ZERO,
            },
            super::hooks::door_seam(d.width),
        ));
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
            super::hooks::lid_seam(SUPPLY_CRATE * 0.5),
        ));
    }
}

/// Each building's crates, as it appears.
fn fit_building_crates(
    mut commands: Commands,
    buildings: Query<(&Building, &Transform), Added<Building>>,
    broken: Res<Broken>,
) {
    for (b, t) in &buildings {
        spawn_building_crates(&mut commands, b, t, &broken);
    }
}

/// A blast's harm to wood, against its harm to flesh: boards and nails
/// give to a shock wave a body rides out (a door at a grenade's foot goes
/// in; one five metres off rattles).
pub const BLAST_ON_WOOD: f32 = 3.5;

/// A grenade's blast is a blow to the wood it can see: written as a `Hit`
/// on it, so it wears down like any other.
#[allow(clippy::too_many_arguments)]
fn blasts(
    mut events: MessageReader<crate::weapons::grenades::GrenadeEvent>,
    live: Query<&crate::weapons::grenades::Live>,
    timber: Query<(Entity, &GlobalTransform, Option<&DoorLeaf>), With<Timber>>,
    parents: Query<&ChildOf>,
    spatial: avian3d::prelude::SpatialQuery,
    mut hits: MessageWriter<Hit>,
) {
    use avian3d::prelude::SpatialQueryFilter;
    for ev in events.read() {
        if ev.event != extraction_core::grenade::Event::Exploded {
            continue;
        }
        let (damage, radius) = live.get(ev.grenade).map_or((110.0, 5.0), |l| {
            (l.grenade.def.blast_damage, l.grenade.def.blast_radius)
        });
        for (e, gt, leaf) in &timber {
            // A door's middle, not its hinge.
            let c = leaf.map_or(gt.translation(), |l| {
                gt.transform_point(Vec3::new(l.0.width * 0.5, 1.0, 0.0))
            });
            let d = c.distance(ev.at);
            if d > radius + 1.0 {
                continue;
            }
            let k = (1.0 - d / radius).max(0.0);
            if k <= 0.0 {
                continue;
            }
            // Seen: nothing of the world between but the thing itself.
            let mine = |h: Entity| h == e || parents.get(h).is_ok_and(|p| p.parent() == e);
            let clear = Dir3::new(c - ev.at).ok().is_none_or(|dir| {
                spatial
                    .cast_ray(
                        ev.at,
                        dir,
                        d,
                        true,
                        &SpatialQueryFilter::from_mask(crate::world::Layer::World),
                    )
                    .is_none_or(|h| mine(h.entity) || h.distance > d - 0.6)
            });
            if !clear {
                continue;
            }
            hits.write(Hit {
                entity: e,
                point: c,
                normal: (ev.at - c).normalize_or(Vec3::Y),
                velocity: (c - ev.at).normalize_or(Vec3::Y) * 6.0,
                damage: damage * k * k * BLAST_ON_WOOD,
                shooter: None,
                mass_g: 0.0,
                load: Default::default(),
                blow: None,
            });
        }
    }
}

/// How much of a hit's harm tells on wood: a blunt blow (a butt, a
/// crowbar, a blast) all of it; an edge half (a blade bites, it doesn't
/// split); a point a quarter (it goes in and sticks); a round on a door
/// [`ROUND_ON_DOOR`] (it holes the boards).
pub fn blow_share(h: &Hit, door: bool) -> f32 {
    use extraction_core::melee::Blow;
    match h.blow {
        Some(Blow::Point) => 0.25,
        Some(Blow::Edge) => 0.5,
        Some(Blow::Blunt) => 1.0,
        None if h.mass_g > 0.0 && door => ROUND_ON_DOOR,
        None => 1.0,
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
        let share = blow_share(h, door);
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
pub(super) fn board_pieces(
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

/// A supply crate gone: its slats fly, and what was packed in it spills.
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
    burst(
        &mut commands,
        &mut pool,
        &mut broken,
        gt,
        timber,
        SUPPLY_CRATE * 0.5,
        table.zip(loot.as_deref()).map(|(t, l)| (t, l, &*catalog)),
    );
}

/// A building's crate gone, the same way.
#[allow(clippy::too_many_arguments)]
fn building_crate_gone(
    remove: On<Remove, BuildingCrate>,
    crates: Query<(
        &BuildingCrate,
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
    let Ok((c, gt, timber, table, quiet)) = crates.get(remove.entity) else {
        return;
    };
    if quiet {
        return;
    }
    burst(
        &mut commands,
        &mut pool,
        &mut broken,
        gt,
        timber,
        Vec3::splat(c.size * 0.5),
        table.zip(loot.as_deref()).map(|(t, l)| (t, l, &*catalog)),
    );
}

/// A crate of half extents `s` bursts: its four sides and lid fly as
/// splintered slats, and a roll of what it was packed with spills out.
fn burst(
    commands: &mut Commands,
    pool: &mut DebrisPool,
    broken: &mut Broken,
    gt: &GlobalTransform,
    timber: Option<&Timber>,
    s: Vec3,
    packed: Option<(
        &CrateLoot,
        &crate::items::loot::Loot,
        &crate::items::Catalog,
    )>,
) {
    let k = timber.map_or(key(gt.translation()), |t| t.key);
    broken.0.insert(k);
    let (_, rot, c) = gt.to_scale_rotation_translation();
    let push = timber.map_or(Vec3::ZERO, |t| t.push);
    let mut rng = Rng::new(super::glass::seed_of(k) ^ 0xC8A7E);
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
    debris::throw(commands, pool, pieces);
    // What was packed inside.
    let Some((table, loot, catalog)) = packed else {
        return;
    };
    for (i, drop) in loot
        .0
        .roll(&table.0, catalog, &mut rng)
        .into_iter()
        .enumerate()
    {
        let off = Vec3::new(
            (i as f32 * 1.9).sin() * s.x * 0.5,
            -s.y * 0.5 + 0.08 * i as f32,
            (i as f32 * 1.9).cos() * s.z * 0.5,
        );
        crate::items::spawn_item(
            commands,
            catalog,
            crate::items::SpawnItem {
                kind: drop.kind,
                transform: Transform::from_translation(c + rot * off),
                condition: drop.condition,
            },
        );
    }
}

/// How a breakable thing of its own is drawn (`visuals::dress_crates`):
/// `model` posed `at` in its own space, a box of `size` in `mat` until it
/// loads (or for good, with no model), gone past `range` metres.
#[derive(Component, Clone, Debug)]
pub struct Drawn {
    pub model: Option<&'static str>,
    pub mat: Mat,
    pub at: Transform,
    pub size: Vec3,
    pub range: std::ops::Range<f32>,
    /// Casts its own sun shadow. Not for a part flush with what it's
    /// fixed to (a car's door, a shutter): that casts it already, and a
    /// wreck's forty parts each casting their own blew a view's shadow
    /// budget.
    pub shadow: bool,
}

/// A drawn model's name, and the one node of it to draw if it names one
/// (`wreck_lada#door_fl`).
pub fn model_node(model: &str) -> (&str, Option<&str>) {
    match model.split_once('#') {
        Some((m, n)) => (m, Some(n)),
        None => (model, None),
    }
}

/// The crate models (`art/props/crates.py`): unit cubes on the floor.
pub const CRATE_MODELS: [&str; 2] = ["crate_slat", "crate_army"];

/// A crate standing in a building (`Builder::crate_box`), taken out of the
/// building's collider and mesh to be its own breakable thing: `size` its
/// edge, `model` and `model_at` how it's drawn (in its own space).
#[derive(Component, Clone, Debug)]
pub struct BuildingCrate {
    pub size: f32,
    pub model: &'static str,
    pub model_at: Transform,
}

/// A building's crates: (piece index, prop index) of each `crate_box`
/// above ground (an underground site's are left as they are).
pub fn crates_of(geo: &crate::world::buildings::Geo) -> Vec<(usize, usize)> {
    use crate::world::buildings::Shape;
    geo.props
        .iter()
        .enumerate()
        .filter_map(|(k, d)| {
            let [(name, _)] = d.models.as_slice() else {
                return None;
            };
            if !CRATE_MODELS.contains(name) || d.pieces.len() != 1 {
                return None;
            }
            let i = d.pieces.start;
            let p = &geo.pieces[i];
            let Shape::Box(s) = p.shape else {
                return None;
            };
            (p.mat == Mat::Planks
                && p.collide
                && (s.x - s.y).abs() < 1e-3
                && p.at.translation.y > crate::world::underground::BELOW_Y)
                .then_some((i, k))
        })
        .collect()
}

/// Which pieces are crates of their own (`buildings::spawn_buildings`
/// leaves them out of the building's collider, `dress_buildings` out of
/// its mesh).
pub fn crate_pieces(geo: &crate::world::buildings::Geo) -> Vec<bool> {
    let mut out = vec![false; geo.pieces.len()];
    for (i, _) in crates_of(geo) {
        out[i] = true;
    }
    out
}

/// Which props are those crates' models.
pub fn crate_props(geo: &crate::world::buildings::Geo) -> Vec<bool> {
    let mut out = vec![false; geo.props.len()];
    for (_, k) in crates_of(geo) {
        out[k] = true;
    }
    out
}

/// A building's crates, as their own breakable entities (all of them, or
/// on a restored Zone those not broken).
pub fn spawn_building_crates(
    commands: &mut Commands,
    building: &crate::world::buildings::Building,
    bxf: &Transform,
    broken: &Broken,
) {
    let Some(geo) = crate::world::buildings::prefab(&building.prefab) else {
        return;
    };
    for (i, k) in crates_of(&geo) {
        let p = &geo.pieces[i];
        let crate::world::buildings::Shape::Box(s) = p.shape else {
            continue;
        };
        let size = s.x;
        let world = *bxf * p.at;
        let id = key(world.translation);
        if broken.0.contains(&id) {
            continue;
        }
        let (model, at) = geo.props[k].models[0];
        // The model's pose in the crate's own space.
        let inv = p.at.rotation.inverse();
        let model_at = Transform {
            translation: inv * (at.translation - p.at.translation),
            rotation: inv * at.rotation,
            scale: at.scale,
        };
        // Its loot spot is on its lid.
        let top = p.at.translation + Vec3::Y * (size * 0.5 + 0.05);
        let table = geo
            .loot
            .iter()
            .find(|l| l.at.distance(top) < 0.05)
            .map(|l| CrateLoot(l.table.to_string()));
        let mut e = commands.spawn((
            Name::new(format!("Crate {model}")),
            BuildingCrate {
                size,
                model,
                model_at,
            },
            // Furniture's range (`buildings::dress_buildings`): gone past
            // 25-35 m, where the room's dark anyway.
            Drawn {
                model: Some(model),
                mat: Mat::Planks,
                at: model_at,
                size: Vec3::splat(size),
                range: 25.0..35.0,
                shadow: true,
            },
            world,
            Visibility::default(),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(size, size, size),
            avian3d::prelude::CollisionLayers::new(
                crate::world::Layer::World,
                avian3d::prelude::LayerMask::ALL,
            ),
            Surfaces(vec![(
                Vec3::splat(size * 0.5),
                Transform::IDENTITY,
                Mat::Planks,
            )]),
            Timber {
                hp: CRATE_HP * (size / 0.8).clamp(0.6, 1.2),
                max: CRATE_HP,
                key: id,
                push: Vec3::ZERO,
            },
            super::hooks::lid_seam(Vec3::splat(size * 0.5)),
        ));
        if let Some(t) = table {
            e.insert(t);
        }
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
    let crates: Vec<Entity> = world
        .query_filtered::<Entity, With<BuildingCrate>>()
        .iter(world)
        .collect();
    for e in crates {
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
    let zone = world.get_resource::<crate::world::Zone>().cloned();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        if let Some(zone) = &zone {
            crate::items::loot::spawn_supply_crates(&mut commands, zone);
        }
        let none = Broken::default();
        for (b, t) in &buildings {
            spawn_building_crates(&mut commands, b, t, &none);
        }
    }
    queue.apply(world);
}
