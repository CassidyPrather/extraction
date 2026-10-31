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
        app.init_resource::<Staged>()
            .add_systems(
                Startup,
                // After the career is loaded (it is before the loot): a
                // scene behind a gate waits for the rank that opens it.
                spawn_scenes.after(crate::items::loot::LootSpawnSystems),
            )
            .add_systems(
                Update,
                (unlock_scenes, recall_screens, super::story::tell_beats),
            );
    }
}

/// What opens a scene (and a beat of the story, `world::story::THREADS`):
/// a rank on the driver's job ladder (`Progress::tier` reached), the
/// machines recalled, or this many raids set off in the scene's valley.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Tier(u32),
    Recalled,
    Visits(u32),
    /// The ending played: Lev has been down to the Depot with you.
    Ended,
}

/// Where a career stands, as far as gates go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Standing {
    pub tier: u32,
    pub recalled: bool,
    /// Raids set off in this valley.
    pub visits: u32,
    /// The ending has been played (`raid::epilogue`).
    pub ended: bool,
}

impl Gate {
    pub fn open(self, at: Standing) -> bool {
        match self {
            Gate::Tier(t) => at.tier >= t,
            Gate::Recalled => at.recalled,
            Gate::Visits(n) => at.visits >= n,
            Gate::Ended => at.ended,
        }
    }
}

/// The scenes standing in this Zone, by id.
#[derive(Resource, Default, Debug)]
pub struct Staged(pub std::collections::HashSet<String>);

/// Where the career stands in `map` (rank 0, nothing done, where there's
/// no career).
pub fn standing(
    progression: Option<&crate::progression::Progression>,
    raid: Option<&crate::raid::Raid>,
    epilogue: Option<&crate::raid::epilogue::Epilogue>,
    map: &str,
) -> Standing {
    Standing {
        tier: progression.map_or(0, |p| p.0.tier),
        recalled: raid.is_some_and(|r| r.recalled),
        visits: progression.map_or(0, |p| p.0.raids_in(map)),
        ended: epilogue.is_some_and(|e| e.done),
    }
}

/// [`standing`], from the world.
pub fn gates(world: &World, map: &str) -> Standing {
    standing(
        world.get_resource::<crate::progression::Progression>(),
        world.get_resource::<crate::raid::Raid>(),
        world.get_resource::<crate::raid::epilogue::Epilogue>(),
        map,
    )
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
    /// Only once this opens (a beat of the story): until then it isn't
    /// there.
    #[serde(default)]
    pub after: Option<Gate>,
    #[serde(default)]
    pub pieces: Vec<Piece>,
    #[serde(default)]
    pub items: Vec<Thing>,
    #[serde(default)]
    pub note: Option<Note>,
    #[serde(default)]
    pub terminal: Option<SceneTerminal>,
}

/// A terminal on the scene (a desk's), as a map's: in its frame, screen
/// facing `rot`; press for the next page. After the recall it shows
/// `recalled` instead, if there is one.
#[derive(Deserialize, Debug, Clone)]
pub struct SceneTerminal {
    pub at: [f32; 3],
    #[serde(default)]
    pub rot: f32,
    pub pages: Vec<String>,
    #[serde(default)]
    pub recalled: Vec<String>,
}

/// A scene terminal's screen, and what it says once the machines are
/// recalled.
#[derive(Component, Debug)]
pub struct SceneScreen {
    /// The scene it stands in.
    pub scene: String,
    pub recalled: Vec<String>,
}

/// A scene terminal's case (dressed by the client).
#[derive(Component)]
pub struct TerminalCase;

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
    /// Anything with a model in `props/` (a bus, a suitcase, a wreck): a
    /// box of `size` for its collider (and its stand-in till the model
    /// loads), standing at `at` (`at.y` up from the ground: on another
    /// piece), turned `rot`. Small things (`collide: false`) are stepped
    /// over.
    Prop {
        model: String,
        at: [f32; 3],
        #[serde(default)]
        rot: f32,
        size: [f32; 3],
        #[serde(default = "solid")]
        collide: bool,
    },
}

fn solid() -> bool {
    true
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

impl Scene {
    pub fn open(&self, at: Standing) -> bool {
        self.after.is_none_or(|g| g.open(at))
    }
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
            Piece::Prop {
                ref model,
                at,
                rot,
                size,
                collide,
            } => {
                // Scenes are loaded once and kept: the name lives as long.
                let model: &'static str = model.as_str();
                if collide {
                    b.furnish(model, Vec3::from(size), Vec3::from(at), yaw(rot), None);
                } else {
                    b.strew(model, Vec3::from(size), Vec3::from(at), yaw(rot));
                }
            }
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

/// The solid pieces of `map`'s scenes in the open (a row of buses, a
/// pallet line), as obstacles for the walking grid (`robots::walker`):
/// the map's placements don't know they're there.
pub fn nav_obstacles(zone: &Zone, map: &str) -> Vec<extraction_core::nav::Obstacle> {
    let mut out = Vec::new();
    for (id, s) in of(map) {
        if s.floor.is_some() {
            continue;
        }
        let Some(geo) = geo(&id) else {
            continue;
        };
        let t = pose(zone, s);
        for piece in geo.pieces.iter().filter(|p| p.collide) {
            let Shape::Box(size) = piece.shape else {
                continue;
            };
            // Small things (a case, a chair) are walked round as they come.
            if size.x.max(size.z) < 0.6 {
                continue;
            }
            let at = t.transform_point(piece.at.translation);
            let (turn, _, _) = (t.rotation * piece.at.rotation).to_euler(EulerRot::YXZ);
            out.push(extraction_core::nav::Obstacle::Rect {
                at: [at.x, at.z],
                half: [size.x * 0.5, size.z * 0.5],
                yaw: -turn,
            });
        }
    }
    out
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

/// Each of an outdoor scene's things set down on the ground under it and
/// tilted to it, its boxes with it. A scene stands at one height
/// ([`pose`]), so on a slope its chairs and crates floated (the 28 October
/// playtest, 0:33:38; `capture::footing`). `t` is where the scene stands.
pub fn settle(geo: &mut Geo, zone: &Zone, id: &str, t: &Transform) {
    if by_id(id).is_none_or(|s| s.floor.is_some()) {
        return;
    }
    let ground = |p: Vec3| {
        let w = t.transform_point(p);
        zone.ground(w.x, w.z) - t.translation.y
    };
    // What stands on the ground: its footprint (the scene's frame, before
    // settling) and how it was moved. A thing stacked on another (a case on
    // a pallet, jars on a walker's back) moves with what's under it: set
    // down on the ground itself, it went into it.
    let mut grounded: Vec<(Vec2, Vec2, Transform)> = Vec::new();
    let mut stacked: Vec<(usize, Vec2, f32)> = Vec::new();
    for d in 0..geo.props.len() {
        let range = geo.props[d].pieces.clone();
        // Its feet, in the scene's frame: its boxes' bottom corners, or
        // round each model's place.
        let mut feet: Vec<Vec3> = Vec::new();
        for i in range.clone() {
            if let Shape::Box(s) = geo.pieces[i].shape {
                let h = s * 0.5;
                for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    feet.push(
                        geo.pieces[i]
                            .at
                            .transform_point(Vec3::new(x * h.x, -h.y, z * h.z)),
                    );
                }
            }
        }
        // A model's floor is its origin: its boxes (colliders, tilted to
        // follow a body) can dip below what's drawn, and the drawn remains
        // stood 10 cm over the ground on them. Their footprint, at its floor.
        if let Some(y) = geo.props[d]
            .models
            .iter()
            .map(|(_, at)| at.translation.y)
            .reduce(f32::min)
        {
            for f in &mut feet {
                f.y = y;
            }
        }
        if feet.is_empty() {
            for (_, at) in &geo.props[d].models {
                for (x, z) in [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)] {
                    feet.push(at.transform_point(Vec3::new(x, 0.0, z)));
                }
            }
        }
        let Some(base) = feet.iter().map(|f| f.y).reduce(f32::min) else {
            continue;
        };
        feet.retain(|f| f.y < base + 0.03);
        if base > 0.05 {
            let mid = feet.iter().fold(Vec2::ZERO, |a, f| a + f.xz()) / feet.len() as f32;
            stacked.push((d, mid, base));
            continue;
        }
        let pts: Vec<Vec3> = feet
            .iter()
            .map(|f| Vec3::new(f.x, ground(*f), f.z))
            .collect();
        // The ground's plane under them (least squares, y = a x + b z + c).
        let n = pts.len() as f32;
        let m = pts.iter().fold(Vec3::ZERO, |a, p| a + *p) / n;
        let (mut sxx, mut sxz, mut szz, mut sxy, mut szy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for p in &pts {
            let q = *p - m;
            sxx += q.x * q.x;
            sxz += q.x * q.z;
            szz += q.z * q.z;
            sxy += q.x * q.y;
            szy += q.z * q.y;
        }
        let det = sxx * szz - sxz * sxz;
        let (a, b) = if det.abs() > 1e-6 {
            ((sxy * szz - szy * sxz) / det, (szy * sxx - sxy * sxz) / det)
        } else {
            (0.0, 0.0)
        };
        let tilt = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-a, 1.0, -b).normalize());
        let pivot = Vec3::new(m.x, base, m.z);
        let mut moved = Transform::from_translation(Vec3::new(m.x, m.y, m.z))
            * Transform::from_rotation(tilt)
            * Transform::from_translation(-pivot);
        // Down until its highest-floating foot is on the ground, and no
        // further than that.
        let worst = feet
            .iter()
            .zip(&pts)
            .map(|(f, g)| moved.transform_point(*f).y - g.y)
            .fold(f32::MIN, f32::max);
        moved.translation.y -= worst;
        for i in range.clone() {
            geo.pieces[i].at = moved * geo.pieces[i].at;
        }
        for (_, at) in &mut geo.props[d].models {
            *at = moved * *at;
        }
        let (lo, hi) = feet.iter().fold(
            (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN)),
            |(lo, hi), f| (lo.min(f.xz()), hi.max(f.xz())),
        );
        grounded.push((lo, hi, moved));
    }
    for (d, mid, base) in stacked {
        // On what's under it, or kept as high over the ground as it stood.
        let moved = grounded
            .iter()
            .find(|(lo, hi, _)| mid.cmpge(*lo).all() && mid.cmple(*hi).all())
            .map(|(_, _, m)| *m)
            .unwrap_or_else(|| {
                Transform::from_xyz(0.0, ground(Vec3::new(mid.x, base, mid.y)), 0.0)
            });
        for i in geo.props[d].pieces.clone() {
            geo.pieces[i].at = moved * geo.pieces[i].at;
        }
        for (_, at) in &mut geo.props[d].models {
            *at = moved * *at;
        }
    }
}

/// Whether a scene has something to stand on: the ground, or for one
/// below (`floor`), a site's floor under it. A site that isn't built in
/// this map yet leaves its scenes out, things and all.
pub fn stands(zone: &Zone, scene: &Scene) -> bool {
    scene.floor.is_none_or(|floor| {
        let p = Vec3::new(scene.at[0], floor + 0.2, scene.at[1]);
        super::underground::site_under(zone, p).is_some()
    })
}

fn spawn_scenes(
    mut commands: Commands,
    zone: Res<Zone>,
    opts: Res<crate::launch::LaunchOptions>,
    progression: Option<Res<crate::progression::Progression>>,
    raid: Res<crate::raid::Raid>,
    epilogue: Option<Res<crate::raid::epilogue::Epilogue>>,
    mut staged: ResMut<Staged>,
) {
    let at = standing(
        progression.as_deref(),
        Some(&raid),
        epilogue.as_deref(),
        &opts.map,
    );
    for (id, scene) in of(&opts.map) {
        if scene.open(at) && stands(&zone, scene) {
            stage(&mut commands, &zone, &id, scene, raid.recalled);
            staged.0.insert(id);
        }
    }
    info!("{} staged scenes", staged.0.len());
}

/// A scene whose gate opens while you play (a job handed in at base, the
/// machines recalled) is put up then, its things with it.
fn unlock_scenes(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    opts: Res<crate::launch::LaunchOptions>,
    progression: Option<Res<crate::progression::Progression>>,
    raid: Res<crate::raid::Raid>,
    epilogue: Option<Res<crate::raid::epilogue::Epilogue>>,
    catalog: Res<Catalog>,
    mut staged: ResMut<Staged>,
) {
    let Some(zone) = zone else {
        return;
    };
    let at = standing(
        progression.as_deref(),
        Some(&raid),
        epilogue.as_deref(),
        &opts.map,
    );
    for (id, scene) in of(&opts.map) {
        if staged.0.contains(&id) || !scene.open(at) || !stands(&zone, scene) {
            continue;
        }
        info!("scene opens: {}", scene.name);
        stage(&mut commands, &zone, &id, scene, raid.recalled);
        lay_things(&mut commands, &zone, scene, &catalog);
        staged.0.insert(id);
    }
}

/// Puts up one scene: its furniture (as a building) and its note.
fn stage(commands: &mut Commands, zone: &Zone, id: &str, scene: &Scene, recalled: bool) {
    let Some(mut geo) = geo(id) else {
        return;
    };
    let t = pose(zone, scene);
    settle(&mut geo, zone, id, &t);
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
            Building {
                prefab: id.to_owned(),
            },
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
            crate::render::legibility::TextSource::Note,
            crate::interact::read::readable(super::notes::NOTE),
            super::notes::note_panel(&note.text),
            note_pose(t, note),
            Visibility::default(),
        ));
    }
    if let Some(term) = &scene.terminal {
        let pages = if recalled && !term.recalled.is_empty() {
            term.recalled.clone()
        } else {
            term.pages.clone()
        };
        let screen = super::notes::Terminal { pages, page: 0 };
        let text = screen.screen();
        let root = commands
            .spawn((
                Name::new("Terminal"),
                TerminalCase,
                Transform::from_translation(t.transform_point(Vec3::from(term.at)))
                    .with_rotation(t.rotation * Quat::from_rotation_y(yaw(term.rot))),
                Visibility::default(),
            ))
            .id();
        commands.spawn((
            Name::new("Terminal screen"),
            screen,
            SceneScreen {
                scene: scene.name.clone(),
                recalled: term.recalled.clone(),
            },
            crate::render::legibility::TextSource::Terminal,
            super::notes::terminal_panel(text),
            crate::raid::truck::Pressable::Terminal,
            crate::interact::Grabbable {
                points: vec![crate::interact::GrabPoint::free()],
            },
            Collider::cuboid(0.44, 0.38, 0.1),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            Transform::from_xyz(0.0, 0.23, 0.145),
            Visibility::default(),
            ChildOf(root),
        ));
    }
}

/// The recall changes what a scene terminal says (the Depot's: "Release
/// procedure: NOT FOUND").
fn recall_screens(
    raid: Res<crate::raid::Raid>,
    mut screens: Query<(
        &mut super::notes::Terminal,
        &mut crate::raid::bench::ScreenText,
        &mut SceneScreen,
    )>,
) {
    if !raid.recalled {
        return;
    }
    for (mut term, mut text, mut s) in &mut screens {
        if s.recalled.is_empty() {
            continue;
        }
        term.pages = std::mem::take(&mut s.recalled);
        term.page = 0;
        text.text = term.screen();
    }
}

/// Client: a scene terminal's monitor, as a map terminal's.
pub struct ScenesVisualsPlugin;

impl Plugin for ScenesVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, dress_terminals);
    }
}

fn dress_terminals(
    mut commands: Commands,
    cases: Query<Entity, Added<TerminalCase>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for root in &cases {
        let polymer = library.get(Mat::Polymer, &server, &mut materials);
        for (size, at) in [
            (Vec3::new(0.44, 0.38, 0.4), Vec3::new(0.0, 0.22, -0.06)),
            (Vec3::new(0.2, 0.03, 0.2), Vec3::new(0.0, 0.015, -0.06)),
            (Vec3::new(0.42, 0.03, 0.15), Vec3::new(0.0, 0.015, 0.25)),
        ] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(polymer.clone()),
                Transform::from_translation(at),
                crate::render::world_props::Blockout,
                ChildOf(root),
            ));
        }
        // Boxes until the terminal's model is in.
        commands
            .entity(root)
            .insert(crate::render::world_props::SwapIn::one("terminal"));
    }
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

/// Lays out this map's open scenes' things, with the loot: each Surge
/// cycle puts them back.
pub fn spawn_things(
    commands: &mut Commands,
    map: &str,
    zone: &Zone,
    catalog: &Catalog,
    at: Standing,
) {
    for (_, scene) in of(map) {
        if scene.open(at) && stands(zone, scene) {
            lay_things(commands, zone, scene, catalog);
        }
    }
}

fn lay_things(commands: &mut Commands, zone: &Zone, scene: &Scene, catalog: &Catalog) {
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
                extraction_core::map::VALLEYS.iter().any(|v| v.0 == s.map),
                "{}: no valley {}",
                s.name,
                s.map
            );
            assert!(
                !s.pieces.is_empty() || s.note.is_some() || s.terminal.is_some(),
                "{}: empty",
                s.name
            );
            for t in &s.items {
                assert!(
                    catalog.kind(&t.id).is_some(),
                    "{}: no item {}",
                    s.name,
                    t.id
                );
            }
        }
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            for (id, s) in of(map) {
                let geo = geo(&id).unwrap_or_else(|| panic!("{}: no geometry", s.name));
                let models: Vec<&str> = geo
                    .props
                    .iter()
                    .flat_map(|p| p.models.iter().map(|(n, _)| *n))
                    .collect();
                assert!(
                    s.pieces.is_empty() || !models.is_empty(),
                    "{}: nothing dressed",
                    s.name
                );
            }
        }
        assert!(crate::world::buildings::prefab("scene:marrow_creek:0").is_some());
        assert!(crate::world::buildings::prefab("scene:marrow_creek:99").is_none());
    }

    /// The top of whatever piece of a scene is under `p` (scene frame),
    /// within `tol` of `p.y`.
    fn rests_on(geo: &Geo, p: Vec3, tol: f32) -> bool {
        geo.pieces.iter().any(|piece| {
            let (half, r) = match piece.shape {
                Shape::Box(s) => (s * 0.5, 0.0),
                Shape::Cylinder(r, h) => (Vec3::new(r, h * 0.5, r), r),
                Shape::Prism(_) => return false,
            };
            let q = piece.at.rotation.inverse() * (p - piece.at.translation);
            let top = half.y;
            let inside = if r > 0.0 {
                q.xz().length() <= r + 0.03
            } else {
                q.x.abs() <= half.x + 0.03 && q.z.abs() <= half.z + 0.03
            };
            inside && (q.y - top).abs() <= tol
        })
    }

    /// Every scene in the open can be walked to from where the raid sets
    /// down, on ground no steeper than Map design's comfort slope.
    #[test]
    fn scenes_are_on_a_route() {
        use extraction_core::map::MarkerKind;
        let mut problems = Vec::new();
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let zone = Zone::load(map);
            let mut grid = extraction_core::map_nav::zone_grid(
                &zone.map,
                &zone.hm,
                extraction_core::map_lint::COMFORT_SLOPE,
            );
            for ob in nav_obstacles(&zone, map) {
                grid.block(ob, 0.6);
            }
            let ins = zone
                .map
                .markers
                .iter()
                .find(|m| matches!(m.kind, MarkerKind::Insertion))
                .unwrap()
                .at;
            for (_, s) in of(map) {
                if s.floor.is_some() {
                    continue;
                }
                let to = grid
                    .nearest_walkable(s.at)
                    .filter(|p| Vec2::from(*p).distance(Vec2::from(s.at)) < 6.0);
                if to.and_then(|to| grid.path(ins, to)).is_none() {
                    problems.push(format!("{map}: {} can't be walked to", s.name));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// The walking grid knows a scene's solid pieces: Bay four's buses
    /// block it, and so does the narrow slot between two of them (a sim
    /// bot once walked in and couldn't get out).
    #[test]
    fn scene_pieces_block_the_walking_grid() {
        let map = "sallow_reservoir";
        let zone = Zone::load(map);
        let mut grid = extraction_core::map_nav::zone_grid(
            &zone.map,
            &zone.hm,
            extraction_core::map_nav::MAX_SLOPE,
        );
        let s = of(map).find(|(_, s)| s.name == "Bay four").unwrap().1;
        let at = |x: f32| [s.at[0] + x, s.at[1]];
        // Bus three's middle, and the slot between buses one and two.
        for ob in nav_obstacles(&zone, map) {
            grid.block(ob, 0.6);
        }
        assert!(!grid.walkable(at(0.0)), "a bus is solid");
        assert!(!grid.walkable(at(-4.8)), "the slot is closed");
        // The empty bay stays open: it's the point.
        assert!(grid.walkable(at(3.2)), "bay four is open");
    }

    /// The set pieces' rule, held for every scene: what's left to take
    /// lies on something (a crate, a case, a chair), never on the floor;
    /// and anything stood on something else has it under it.
    #[test]
    fn scene_things_rest_on_their_pieces() {
        let mut problems = Vec::new();
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            for (id, s) in of(map) {
                let geo = geo(&id).unwrap();
                for t in &s.items {
                    let p = Vec3::from(t.at);
                    if p.y < 0.1 || !rests_on(&geo, p, 0.04) {
                        problems.push(format!("{}: {} at {:?} on nothing", s.name, t.id, t.at));
                    }
                }
                for piece in &s.pieces {
                    if let Piece::Prop { model, at, .. } = piece
                        && at[1] > 0.01
                        && !rests_on(&geo, Vec3::from(*at), 0.03)
                    {
                        problems.push(format!("{}: {model} at {at:?} hangs in the air", s.name));
                    }
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// A scene below stands on its site's floor, or, while its site isn't
    /// built in the map, isn't put up at all (nothing on bare ground).
    #[test]
    fn scenes_below_wait_for_their_site() {
        let mut standing = 0;
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
            let zone = Zone::load(map);
            for (_, s) in of(map) {
                let Some(floor) = s.floor else {
                    assert!(stands(&zone, s), "{}", s.name);
                    continue;
                };
                let p = Vec3::new(s.at[0], floor + 0.2, s.at[1]);
                match crate::world::underground::site_under(&zone, p) {
                    Some((site, f)) => {
                        assert!(stands(&zone, s));
                        assert!(
                            (f - floor).abs() < 0.05,
                            "{}: floor {floor}, {site}'s is {f}",
                            s.name
                        );
                        standing += 1;
                    }
                    None => assert!(!stands(&zone, s), "{}", s.name),
                }
            }
        }
        // The Depot's and the mines' are built.
        assert!(standing >= 5, "{standing} scenes below stand");
    }

    /// A scene stands where it can: in the open on level ground, clear of
    /// buildings and trees, or wholly inside one building on its floor.
    #[test]
    fn scenes_stand_clear() {
        let mut problems = Vec::new();
        for map in extraction_core::map::VALLEYS.map(|v| v.0) {
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

    /// A scene behind a rank isn't there until the rank is reached, and is
    /// put up, things and all, when a job handed in reaches it.
    #[test]
    fn a_rank_opens_its_scene() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "marrow_creek".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let id = of("marrow_creek")
            .find(|(_, s)| s.name == "Rusty's corner")
            .unwrap()
            .0;
        let standing = |app: &mut App| {
            app.world_mut()
                .query::<&Building>()
                .iter(app.world())
                .any(|b| b.prefab == id)
        };
        assert!(!standing(&mut app), "not before the rank");
        let items = |app: &mut App| {
            app.world_mut()
                .query::<&crate::items::Item>()
                .iter(app.world())
                .count()
        };
        let before = items(&mut app);
        app.world_mut()
            .resource_mut::<crate::progression::Progression>()
            .0
            .tier = 1;
        for _ in 0..2 {
            app.update();
        }
        assert!(standing(&mut app), "up once the rank is reached");
        assert!(items(&mut app) > before, "with its things");
    }
}
