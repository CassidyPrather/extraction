//! The memorial board on the front wall of the truck bed: a plank with
//! chalk tallies. White strokes in fives for every item a convoy has
//! carried out of the Zone, red notches along the top for each convoy that
//! got away, green ones for each body you got back.

use bevy::prelude::*;

use crate::persist::Memorial;
use crate::raid::truck::{FLOOR, Truck};

pub struct MemorialBoardPlugin;

impl Plugin for MemorialBoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (hang_board, chalk_tallies).chain());
    }
}

#[derive(Component)]
struct Board;

/// What the driver tells everyone, once.
const DRIVERS_NOTE: &str = "Read this.\n\
- Kit's in the crate. Take what\n  you'll carry; leave the rest.\n\
- Radio me from the far side and\n  I'll drive round to an RV. Two\n  minutes. I wait five.\n\
- A can of fuel in the crate and\n  I'll come back to the gate.\n\
- Watch on your wrist: the Surge.\n  Be out or under a roof.\n\
- Bench strips robot parts and\n  packs rounds. Lever runs it.\n\
- Go down and they send the\n  collector for you. Beat it.\n\
- Pull the cord when you're in.";

#[derive(Component)]
struct Tally;

#[derive(Component)]
struct MemorialNote;

const SIZE: Vec2 = Vec2::new(1.0, 0.7);
const MAX_ROWS: usize = 5;
const GROUPS_PER_ROW: usize = 10;

fn hang_board(
    mut commands: Commands,
    trucks: Query<Entity, Added<Truck>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<super::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for truck in &trucks {
        let wood = library.get(super::materials::Mat::Planks, &server, &mut materials);
        commands.spawn((
            Name::new("Memorial board"),
            Board,
            Mesh3d(meshes.add(super::materials::metric_box(Vec3::new(
                SIZE.x, SIZE.y, 0.03,
            )))),
            MeshMaterial3d(wood),
            Transform::from_xyz(-0.4, FLOOR + 1.35, -1.76),
            Visibility::default(),
            ChildOf(truck),
        ));
        // The driver's note, pinned on the left canvas wall by the stash.
        commands.spawn((
            Name::new("Driver's note"),
            crate::render::legibility::TextSource::DriversNote,
            crate::interact::read::readable(Vec2::new(0.36, 0.5)),
            crate::raid::bench::ScreenText {
                text: DRIVERS_NOTE.into(),
                size: Vec2::new(0.36, 0.5),
                style: crate::raid::bench::ScreenStyle::Paper,
            },
            Transform::from_xyz(-1.12, FLOOR + 1.72, -0.2).with_rotation(
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) * Quat::from_rotation_z(-0.04),
            ),
            Visibility::default(),
            ChildOf(truck),
        ));
        // A pencilled list pinned beside it: the last things taken.
        commands.spawn((
            Name::new("Memorial note"),
            MemorialNote,
            crate::render::legibility::TextSource::MemorialNote,
            crate::interact::read::readable(Vec2::new(0.24, 0.34)),
            crate::raid::bench::ScreenText {
                text: String::new(),
                size: Vec2::new(0.24, 0.34),
                style: crate::raid::bench::ScreenStyle::Paper,
            },
            Transform::from_xyz(0.28, FLOOR + 1.3, -1.765)
                .with_rotation(Quat::from_rotation_z(0.03)),
            Visibility::default(),
            ChildOf(truck),
        ));
    }
}

fn chalk_tallies(
    mut commands: Commands,
    memorial: Res<Memorial>,
    boards: Query<Entity, With<Board>>,
    added: Query<(), Added<Board>>,
    tallies: Query<Entity, With<Tally>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: Local<Option<(Handle<Mesh>, Handle<Mesh>, [Handle<StandardMaterial>; 3])>>,
    mut notes: Query<&mut crate::raid::bench::ScreenText, With<MemorialNote>>,
) {
    if !memorial.is_changed() && added.is_empty() {
        return;
    }
    let Some(board) = boards.iter().next() else {
        return;
    };
    for t in &tallies {
        commands.entity(t).despawn();
    }
    let (stroke, notch, [white, red, green]) = assets
        .get_or_insert_with(|| {
            let chalk = |c: Color| StandardMaterial {
                base_color: c,
                perceptual_roughness: 0.95,
                ..default()
            };
            (
                meshes.add(Cuboid::new(0.007, 0.075, 0.004)),
                meshes.add(Cuboid::new(0.012, 0.035, 0.004)),
                [
                    materials.add(chalk(Color::srgb(0.85, 0.84, 0.8))),
                    materials.add(chalk(Color::srgb(0.65, 0.12, 0.1))),
                    materials.add(chalk(Color::srgb(0.25, 0.55, 0.25))),
                ],
            )
        })
        .clone();
    let m = &memorial.0;
    for mut note in &mut notes {
        let mut lines = vec!["Taken:".to_string()];
        lines.extend(m.lost.iter().rev().take(9).map(|n| format!("- {n}")));
        if m.lost.is_empty() {
            lines.push("(nothing yet)".into());
        }
        note.text = lines.join("\n");
    }
    let left = -SIZE.x * 0.5 + 0.06;
    let top = SIZE.y * 0.5 - 0.05;
    let z = 0.018;
    let mut spawn = |mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, t: Transform| {
        commands.spawn((
            Tally,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            t,
            ChildOf(board),
        ));
    };
    // Header: convoys lost (red), bodies recovered (green).
    for i in 0..(m.convoys as usize).min(30) {
        spawn(
            &notch,
            &red,
            Transform::from_xyz(left + i as f32 * 0.022, top, z),
        );
    }
    for i in 0..(m.recovered as usize).min(15) {
        spawn(
            &notch,
            &green,
            Transform::from_xyz(SIZE.x * 0.5 - 0.06 - i as f32 * 0.022, top, z),
        );
    }
    // Strokes in fives, the fifth struck through.
    let n = m.lost.len().min(MAX_ROWS * GROUPS_PER_ROW * 5);
    for k in 0..n {
        let group = k / 5;
        let row = group / GROUPS_PER_ROW;
        let col = group % GROUPS_PER_ROW;
        let gx = left + col as f32 * 0.088;
        let y = top - 0.1 - row as f32 * 0.115;
        let within = k % 5;
        let t = if within < 4 {
            Transform::from_xyz(gx + within as f32 * 0.014, y, z)
                .with_rotation(Quat::from_rotation_z(0.06))
        } else {
            Transform::from_xyz(gx + 0.021, y, z + 0.001)
                .with_rotation(Quat::from_rotation_z(-1.1))
                .with_scale(Vec3::new(1.0, 1.05, 1.0))
        };
        spawn(&stroke, &white, t);
    }
}
