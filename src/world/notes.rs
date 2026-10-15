//! Words left in the Zone: notes on tables and floors, signs on posts,
//! terminals on desks (`MarkerKind::Note`, `Sign`, `Terminal` in a map).
//! A note or terminal rests on whatever is built where its marker stands,
//! or on the ground (a marker's `y` lets it rest that much higher: a
//! tower's cabin); press a terminal for its next page.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use super::Zone;
use super::buildings::{Shape, placement_transform, prefab};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::raid::bench::{ScreenStyle, ScreenText};
use crate::raid::truck::Pressable;
use crate::render::materials::{Mat, MaterialLibrary};

pub struct NotesPlugin;

impl Plugin for NotesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_notes, press_terminals));
    }
}

/// A terminal's pages, and which one it shows.
#[derive(Component, Debug)]
pub struct Terminal {
    pub pages: Vec<String>,
    pub page: usize,
}

impl Terminal {
    fn screen(&self) -> String {
        let n = self.pages.len();
        let body = self.pages.get(self.page).cloned().unwrap_or_default();
        if n > 1 {
            format!("{body}\n\n[{}/{}]  press for more", self.page + 1, n)
        } else {
            body
        }
    }
}

/// Paper size, metres: a sheet you can read leaning over a table.
pub const NOTE: Vec2 = Vec2::new(0.24, 0.32);

/// The top of whatever is built under (x, z) that something could rest on
/// (a floor, a table, a shelf: no higher than `reach` above the building's
/// footing, so not its roof), or the ground.
pub fn rest_height(zone: &Zone, x: f32, z: f32, reach: f32) -> f32 {
    let ground = zone.ground(x, z);
    let mut best = ground;
    for p in &zone.map.placements {
        let [w, _, d] = extraction_core::map::prefab_footprint(&p.prefab);
        if Vec2::new(x - p.at[0], z - p.at[1]).length() > w.max(d) {
            continue;
        }
        let Some(geo) = prefab(&p.prefab) else {
            continue;
        };
        let t = placement_transform(zone, p);
        let local = t
            .compute_affine()
            .inverse()
            .transform_point3(Vec3::new(x, t.translation.y, z));
        for piece in &geo.pieces {
            let Shape::Box(size) = piece.shape else {
                continue;
            };
            let q = piece.at.rotation.inverse() * (local - piece.at.translation);
            let half = size * 0.5;
            if q.x.abs() > half.x || q.z.abs() > half.z {
                continue;
            }
            let top = piece.at.translation.y + half.y;
            if top > reach {
                continue;
            }
            best = best.max(t.translation.y + top);
        }
    }
    best
}

fn spawn_notes(
    mut commands: Commands,
    zone: Option<Res<Zone>>,
    mut done: Local<bool>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    let Some(zone) = zone else {
        return;
    };
    if *done {
        return;
    }
    *done = true;
    let mut mat = |m: Mat| library.get(m, &server, &mut materials);
    for m in &zone.map.markers {
        let yaw = Quat::from_rotation_y(-m.rot.to_radians());
        match &m.kind {
            MarkerKind::Note { text } => {
                let y = rest_height(&zone, m.at[0], m.at[1], 2.2 + m.y);
                commands.spawn((
                    Name::new("Note"),
                    ScreenText {
                        text: crate::render::screen::wrap_lines(text, 24),
                        size: NOTE,
                        style: ScreenStyle::Paper,
                    },
                    // Lying face up, its top edge away from whoever reads
                    // it from the marker's facing.
                    Transform::from_xyz(m.at[0], y + 0.004, m.at[1])
                        .with_rotation(yaw * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    Visibility::default(),
                ));
            }
            MarkerKind::Sign { text } => {
                let ground = zone.ground(m.at[0], m.at[1]);
                let sign = commands
                    .spawn((
                        Name::new("Sign"),
                        Transform::from_xyz(m.at[0], ground, m.at[1]).with_rotation(yaw),
                        Visibility::default(),
                        RigidBody::Static,
                        Collider::compound(vec![(
                            Vec3::new(0.0, 0.8, 0.0),
                            Quat::IDENTITY,
                            Collider::cuboid(0.09, 1.6, 0.09),
                        )]),
                        CollisionLayers::new(super::Layer::World, LayerMask::ALL),
                    ))
                    .id();
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.09, 1.9, 0.09))),
                    MeshMaterial3d(mat(Mat::Timber)),
                    Transform::from_xyz(0.0, 0.8, -0.03),
                    ChildOf(sign),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.8, 0.56, 0.03))),
                    MeshMaterial3d(mat(Mat::Planks)),
                    Transform::from_xyz(0.0, 1.45, 0.03),
                    ChildOf(sign),
                ));
                commands.spawn((
                    ScreenText {
                        text: crate::render::screen::wrap_lines(text, 34),
                        size: Vec2::new(0.74, 0.5),
                        style: ScreenStyle::Paper,
                    },
                    Transform::from_xyz(0.0, 1.45, 0.05),
                    Visibility::default(),
                    ChildOf(sign),
                ));
            }
            MarkerKind::Terminal { pages } => {
                let y = rest_height(&zone, m.at[0], m.at[1], 2.2 + m.y);
                let term = Terminal {
                    pages: pages.clone(),
                    page: 0,
                };
                let text = term.screen();
                let root = commands
                    .spawn((
                        Name::new("Terminal"),
                        Transform::from_xyz(m.at[0], y, m.at[1]).with_rotation(yaw),
                        Visibility::default(),
                    ))
                    .id();
                // The monitor: a deep case, a bezel, the glass.
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.44, 0.38, 0.4))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.22, -0.06),
                    ChildOf(root),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.2, 0.03, 0.2))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.015, -0.06),
                    ChildOf(root),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.42, 0.03, 0.15))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.015, 0.25)
                        .with_rotation(Quat::from_rotation_x(0.08)),
                    ChildOf(root),
                ));
                commands.spawn((
                    Name::new("Terminal screen"),
                    term,
                    ScreenText {
                        text,
                        size: Vec2::new(0.34, 0.26),
                        style: ScreenStyle::Crt,
                    },
                    Pressable::Terminal,
                    Grabbable {
                        points: vec![GrabPoint::free()],
                    },
                    Collider::cuboid(0.44, 0.38, 0.1),
                    CollisionLayers::new(super::Layer::Item, LayerMask::NONE),
                    Transform::from_xyz(0.0, 0.23, 0.145),
                    Visibility::default(),
                    ChildOf(root),
                ));
            }
            _ => {}
        }
    }
}

fn press_terminals(
    mut events: MessageReader<InteractEvent>,
    mut terms: Query<(&mut Terminal, &mut ScreenText)>,
) {
    for e in events.read() {
        let InteractEvent::Pressed { item, .. } = e else {
            continue;
        };
        if let Ok((mut t, mut screen)) = terms.get_mut(*item) {
            t.page = (t.page + 1) % t.pages.len().max(1);
            screen.text = t.screen();
        }
    }
}
