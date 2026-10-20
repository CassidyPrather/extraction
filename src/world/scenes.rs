//! Staged scenes: small arrangements of furniture, things and sometimes a
//! body, left at the valleys' points of interest, each a moment someone
//! didn't come back from (`docs/LORE.md`, "Staged scenes"). They're data,
//! in `assets/data/scenes.ron`.
//!
//! A scene is built like a building (`buildings::Builder`'s furniture,
//! dressed with the prop models), under the id `scene:<map>:<n>`, so its
//! colliders, surfaces and models come the buildings' way. It rests on
//! whatever floor is under it, or the ground. Its things are laid out with
//! the loot each Surge cycle (`items::loot`), and its note, if it has one,
//! lies on it like any other.

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::Deserialize;
use std::sync::OnceLock;

use super::buildings::{Builder, Building, Geo, Shape};
use super::{Layer, Zone};
use crate::items::{Catalog, SpawnItem, spawn_item};
use crate::render::materials::Mat;

pub const PREFIX: &str = "scene:";

pub struct ScenesPlugin;

impl Plugin for ScenesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            spawn_scenes.after(super::buildings::spawn_buildings),
        );
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Scene {
    pub map: String,
    pub name: String,
    /// Where it stands (x, z), and which way it faces (degrees, as a
    /// marker's `rot`).
    pub at: [f32; 2],
    #[serde(default)]
    pub rot: f32,
    /// Underground: the site's floor height (world y, as
    /// `underground::where_sites` prints it), not whatever is above.
    #[serde(default)]
    pub floor: Option<f32>,
    #[serde(default)]
    pub pieces: Vec<Piece>,
    #[serde(default)]
    pub items: Vec<Thing>,
    #[serde(default)]
    pub note: Option<Note>,
}

/// A piece of furniture, in the scene's frame (x, z; y up from where it
/// rests), turned `rot` degrees.
#[derive(Deserialize, Debug, Clone)]
pub enum Piece {
    Table {
        at: [f32; 2],
        rot: f32,
    },
    Chair {
        at: [f32; 2],
        rot: f32,
    },
    Barrel {
        at: [f32; 2],
    },
    Crate {
        at: [f32; 2],
        size: f32,
    },
    Pallet {
        at: [f32; 2],
        rot: f32,
    },
    Bed {
        at: [f32; 2],
        rot: f32,
    },
    Stove {
        at: [f32; 2],
        rot: f32,
    },
    Locker {
        at: [f32; 2],
        rot: f32,
    },
    /// A lantern standing on something (`at` is its foot).
    Lantern {
        at: [f32; 3],
    },
    /// A sleeping bag rolled out on the ground.
    Bedroll {
        at: [f32; 2],
        rot: f32,
    },
    /// A cold fire: a ring of stones, charred logs.
    Campfire {
        at: [f32; 2],
    },
    /// Someone who stayed: in a chair (`seated`, facing as a chair does)
    /// or on the ground.
    Remains {
        at: [f32; 2],
        rot: f32,
        seated: bool,
    },
}

/// An item left in the scene, in its frame; `y` up from where it rests.
#[derive(Deserialize, Debug, Clone)]
pub struct Thing {
    pub id: String,
    pub at: [f32; 3],
    #[serde(default)]
    pub rot: f32,
    #[serde(default = "worn")]
    pub condition: f32,
}

fn worn() -> f32 {
    0.4
}

/// A sheet of paper on the scene, in its frame (`at.y` is the surface's
/// height); read from its `rot` facing, as a map's note is.
#[derive(Deserialize, Debug, Clone)]
pub struct Note {
    pub name: String,
    pub text: String,
    pub at: [f32; 3],
    #[serde(default)]
    pub rot: f32,
}

/// Every scene in every valley.
pub fn all() -> &'static [Scene] {
    static SCENES: OnceLock<Vec<Scene>> = OnceLock::new();
    SCENES.get_or_init(|| {
        let path = crate::asset_root_path().join("data/scenes.ron");
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|_| include_str!("../../assets/data/scenes.ron").to_owned());
        ron::from_str(&text).unwrap_or_else(|e| {
            error!("scenes.ron: {e}");
            Vec::new()
        })
    })
}

/// This map's scenes, with their ids.
pub fn of(map: &str) -> impl Iterator<Item = (String, &'static Scene)> {
    all()
        .iter()
        .filter(move |s| s.map == map)
        .enumerate()
        .map(move |(i, s)| (format!("{PREFIX}{map}:{i}"), s))
}

fn by_id(id: &str) -> Option<&'static Scene> {
    let rest = id.strip_prefix(PREFIX)?;
    let (map, n) = rest.rsplit_once(':')?;
    let n: usize = n.parse().ok()?;
    all().iter().filter(|s| s.map == map).nth(n)
}

fn v(at: [f32; 2]) -> Vec3 {
    Vec3::new(at[0], 0.0, at[1])
}

fn yaw(rot: f32) -> f32 {
    -rot.to_radians()
}

/// A scene's geometry in its own frame, for `buildings::prefab`.
pub fn geo(id: &str) -> Option<Geo> {
    let scene = by_id(id)?;
    let mut b = Builder::default();
    for p in &scene.pieces {
        match *p {
            Piece::Table { at, rot } => {
                b.table(v(at), yaw(rot), None);
            }
            Piece::Chair { at, rot } => {
                b.chair(v(at), yaw(rot));
            }
            Piece::Barrel { at } => {
                b.barrel(v(at));
            }
            Piece::Crate { at, size } => {
                b.crate_box(v(at), size, None);
            }
            Piece::Pallet { at, rot } => {
                b.pallet(v(at), yaw(rot));
            }
            Piece::Bed { at, rot } => {
                b.bed(v(at), yaw(rot));
            }
            Piece::Stove { at, rot } => {
                b.stove(v(at), yaw(rot));
            }
            Piece::Locker { at, rot } => {
                b.locker(v(at), yaw(rot));
            }
            Piece::Lantern { at } => {
                let from = b.geo.pieces.len();
                b.piece(
                    Shape::Cylinder(0.07, 0.26),
                    Transform::from_xyz(at[0], at[1] + 0.13, at[2]),
                    Mat::DarkMetal,
                    false,
                );
                b.dress(
                    from,
                    vec![("lantern", Transform::from_xyz(at[0], at[1], at[2]))],
                );
            }
            Piece::Bedroll { at, rot } => {
                let from = b.geo.pieces.len();
                let t = Transform::from_translation(v(at))
                    .with_rotation(Quat::from_rotation_y(yaw(rot)));
                b.piece(
                    Shape::Box(Vec3::new(0.7, 0.08, 1.9)),
                    Transform::from_translation(v(at) + Vec3::Y * 0.04).with_rotation(t.rotation),
                    Mat::Canvas,
                    false,
                );
                b.dress(from, vec![("bedroll", t)]);
            }
            Piece::Campfire { at } => {
                let from = b.geo.pieces.len();
                b.piece(
                    Shape::Cylinder(0.4, 0.12),
                    Transform::from_translation(v(at) + Vec3::Y * 0.06),
                    Mat::Boulder,
                    false,
                );
                b.dress(from, vec![("campfire", Transform::from_translation(v(at)))]);
            }
            Piece::Remains { at, rot, seated } => remains(&mut b, v(at), yaw(rot), seated),
        }
    }
    Some(b.finish())
}

/// A body in a quilted coat: boxes until the `remains_*` models are in.
/// Nothing to collide with (you step over it; it doesn't stop a bullet).
fn remains(b: &mut Builder, at: Vec3, yaw: f32, seated: bool) {
    let from = b.geo.pieces.len();
    let rot = Quat::from_rotation_y(yaw);
    let parts: &[(Vec3, Vec3, f32)] = if seated {
        // Slumped in a chair facing -Z: thighs on the seat, shins down,
        // the head fallen forward.
        &[
            (Vec3::new(0.40, 0.52, 0.26), Vec3::new(0.0, 0.76, 0.1), 0.25),
            (Vec3::new(0.19, 0.2, 0.2), Vec3::new(0.0, 1.06, -0.04), 0.5),
            (
                Vec3::new(0.36, 0.15, 0.42),
                Vec3::new(0.0, 0.53, -0.12),
                0.0,
            ),
            (
                Vec3::new(0.30, 0.46, 0.14),
                Vec3::new(0.0, 0.24, -0.34),
                0.0,
            ),
        ]
    } else {
        // Face down along Z, head toward -Z, one knee drawn up.
        &[
            (Vec3::new(0.46, 0.24, 0.70), Vec3::new(0.0, 0.12, -0.2), 0.0),
            (Vec3::new(0.2, 0.19, 0.22), Vec3::new(0.05, 0.1, -0.7), 0.0),
            (
                Vec3::new(0.18, 0.16, 0.85),
                Vec3::new(-0.11, 0.08, 0.55),
                0.0,
            ),
            (Vec3::new(0.18, 0.16, 0.6), Vec3::new(0.16, 0.08, 0.4), -0.5),
        ]
    };
    for (size, centre, tilt) in parts {
        b.piece(
            Shape::Box(*size),
            Transform::from_translation(at + rot * *centre).with_rotation(
                rot * Quat::from_rotation_y(*tilt * 0.3) * Quat::from_rotation_x(-*tilt),
            ),
            Mat::Canvas,
            false,
        );
    }
    let model = if seated {
        "remains_seated"
    } else {
        "remains_lying"
    };
    b.dress(
        from,
        vec![(model, Transform::from_translation(at).with_rotation(rot))],
    );
}

/// Where a scene stands in the world: on whatever floor is under it, or
/// the ground.
pub fn pose(zone: &Zone, scene: &Scene) -> Transform {
    let y = scene
        .floor
        .unwrap_or_else(|| super::notes::rest_height(zone, scene.at[0], scene.at[1], 0.6));
    Transform::from_xyz(scene.at[0], y, scene.at[1])
        .with_rotation(Quat::from_rotation_y(yaw(scene.rot)))
}

fn spawn_scenes(mut commands: Commands, zone: Res<Zone>, opts: Res<crate::launch::LaunchOptions>) {
    let mut n = 0;
    for (id, scene) in of(&opts.map) {
        let Some(geo) = geo(&id) else {
            continue;
        };
        let t = pose(&zone, scene);
        let mut colliders = Vec::new();
        let mut surfaces = Vec::new();
        for piece in &geo.pieces {
            let half = match piece.shape {
                Shape::Box(s) => s * 0.5,
                Shape::Prism(s) => Vec3::new(s.x, s.y * 0.5, s.z) * 0.5,
                Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
            };
            surfaces.push((half, piece.at, piece.mat));
            if !piece.collide {
                continue;
            }
            let c = match piece.shape {
                Shape::Box(s) => Collider::cuboid(s.x, s.y, s.z),
                Shape::Prism(s) => Collider::cuboid(s.x, s.y * 0.5, s.z),
                Shape::Cylinder(r, h) => Collider::cylinder(r, h),
            };
            colliders.push((piece.at.translation, piece.at.rotation, c));
        }
        let root = commands
            .spawn((
                Name::new(format!("Scene: {}", scene.name)),
                Building { prefab: id },
                t,
                Visibility::default(),
            ))
            .id();
        if !colliders.is_empty() {
            commands.entity(root).insert((
                RigidBody::Static,
                Collider::compound(colliders),
                CollisionLayers::new(Layer::World, LayerMask::ALL),
                super::surfaces::Surfaces(surfaces),
            ));
        }
        if let Some(note) = &scene.note {
            commands.spawn((
                Name::new("Note"),
                super::notes::note_panel(&note.text),
                note_pose(t, note),
                Visibility::default(),
            ));
        }
        n += 1;
    }
    info!("{n} staged scenes");
}

/// A scene's note, lying face up on its surface.
pub fn note_pose(scene: Transform, note: &Note) -> Transform {
    let at = scene.transform_point(Vec3::new(note.at[0], note.at[1] + 0.004, note.at[2]));
    Transform::from_translation(at).with_rotation(
        scene.rotation
            * Quat::from_rotation_y(yaw(note.rot))
            * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    )
}

/// Lays out this map's scenes' things, with the loot: each Surge cycle
/// puts them back.
pub fn spawn_things(commands: &mut Commands, map: &str, zone: &Zone, catalog: &Catalog) {
    for (_, scene) in of(map) {
        let t = pose(zone, scene);
        for thing in &scene.items {
            let Some(kind) = catalog.kind(&thing.id) else {
                warn!("scene {}: no item {}", scene.name, thing.id);
                continue;
            };
            spawn_item(
                commands,
                catalog,
                SpawnItem {
                    kind,
                    transform: Transform::from_translation(
                        t.transform_point(Vec3::from(thing.at) + Vec3::Y * 0.03),
                    )
                    .with_rotation(t.rotation * Quat::from_rotation_y(yaw(thing.rot))),
                    condition: thing.condition,
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extraction_core::items::{BUILTIN_CATALOG, ItemCatalog};

    /// Every scene is in a valley, names real things, and has something
    /// in it.
    #[test]
    fn scenes_are_made_of_real_things() {
        let catalog = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        assert!(all().len() >= 6, "{} scenes", all().len());
        for s in all() {
            assert!(
                crate::world::story::VALLEYS.contains(&s.map.as_str()),
                "{}: no valley {}",
                s.name,
                s.map
            );
            assert!(!s.pieces.is_empty(), "{}: empty", s.name);
            for t in &s.items {
                assert!(
                    catalog.kind(&t.id).is_some(),
                    "{}: no item {}",
                    s.name,
                    t.id
                );
            }
        }
        for map in crate::world::story::VALLEYS {
            for (id, s) in of(map) {
                let geo = geo(&id).unwrap_or_else(|| panic!("{}: no geometry", s.name));
                assert!(!geo.pieces.is_empty());
                let models: Vec<&str> = geo
                    .props
                    .iter()
                    .flat_map(|p| p.models.iter().map(|(n, _)| *n))
                    .collect();
                assert!(!models.is_empty(), "{}: nothing dressed", s.name);
            }
        }
        assert!(crate::world::buildings::prefab("scene:marrow_creek:0").is_some());
        assert!(crate::world::buildings::prefab("scene:marrow_creek:99").is_none());
    }

    /// A scene stands where it can: in the open on level ground, clear of
    /// buildings and trees, or wholly inside one building on its floor.
    #[test]
    fn scenes_stand_clear() {
        let mut problems = Vec::new();
        for map in crate::world::story::VALLEYS {
            let zone = Zone::load(map);
            let trees = crate::world::trees::scatter_trees(&zone);
            // Which placements (by index) cover a point.
            let under = |p: Vec2| -> Vec<usize> {
                zone.map
                    .placements
                    .iter()
                    .enumerate()
                    .filter(|(_, pl)| {
                        let [w, _, d] = extraction_core::map::prefab_footprint(&pl.prefab);
                        let local = Quat::from_rotation_y(pl.rot.to_radians())
                            * Vec3::new(p.x - pl.at[0], 0.0, p.y - pl.at[1]);
                        local.x.abs() < w * 0.5 + 0.3 && local.z.abs() < d * 0.5 + 0.3
                    })
                    .map(|(i, _)| i)
                    .collect()
            };
            for (id, s) in of(map) {
                let t = pose(&zone, s);
                if let Some(floor) = s.floor {
                    // Underground: the site's own layout check keeps its
                    // rooms clear; here, only that it's well below ground.
                    if floor > zone.ground(s.at[0], s.at[1]) - 3.0 {
                        problems.push(format!("{}: floor {floor} not underground", s.name));
                    }
                    continue;
                }
                let inside = under(Vec2::from(s.at));
                let ground = zone.ground(s.at[0], s.at[1]);
                if inside.is_empty() && (t.translation.y - ground).abs() > 0.01 {
                    problems.push(format!("{}: off the ground in the open", s.name));
                }
                let geo = geo(&id).unwrap();
                for piece in &geo.pieces {
                    let w = t.transform_point(piece.at.translation);
                    let here = under(w.xz());
                    if here != inside {
                        problems.push(format!(
                            "{}: a piece at {:.1},{:.1} straddles a building",
                            s.name, w.x, w.z
                        ));
                    }
                    if inside.is_empty() {
                        let g = zone.ground(w.x, w.z);
                        if (g - t.translation.y).abs() > 0.3 {
                            problems.push(format!(
                                "{}: ground {:.2} m off level at {:.1},{:.1}",
                                s.name,
                                g - t.translation.y,
                                w.x,
                                w.z
                            ));
                        }
                    }
                    let p = [w.x, w.z];
                    for r in &zone.map.roads {
                        if extraction_core::geom::polyline_closest(&r.points, p).0
                            < r.width * 0.5 + 0.8
                        {
                            problems.push(format!("{}: a piece on {}", s.name, r.name));
                        }
                    }
                    for wt in &zone.map.water {
                        if extraction_core::geom::polyline_closest(&wt.points, p).0
                            < wt.width * 0.5 + 1.0
                        {
                            problems.push(format!("{}: a piece in {}", s.name, wt.name));
                        }
                    }
                    if let Some(tree) = trees.iter().find(|tr| tr.pos.xz().distance(w.xz()) < 1.2) {
                        problems.push(format!(
                            "{}: a tree at {:.1},{:.1}",
                            s.name, tree.pos.x, tree.pos.z
                        ));
                    }
                }
            }
        }
        problems.dedup();
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
