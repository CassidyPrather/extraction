//! The cork board in the truck: a place to piece the story together
//! (`docs/LORE.md`, "The board"). Every note you've read out there gets a
//! pinned copy, in a column for its valley; where two read notes name each
//! other (`world::story::THREAD`), a length of red string joins them. What
//! you haven't read leaves nothing: no gaps, no blank cards. Press a card
//! to take the note down and read it; press it again to pin it back. What
//! you've read is kept with the career.
//!
//! Reading is looking: your head within arm's reach of a note and turned to
//! it for a moment (`read_notes`).

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use std::collections::HashMap;

use super::bench::{ScreenStyle, ScreenText};
use super::truck::{FLOOR, Pressable, Truck};
use crate::interact::{GrabPoint, Grabbable, InteractEvent};
use crate::launch::LaunchOptions;
use crate::rig::{LocalPlayer, RigHead};
use crate::world::{Layer, Zone};

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NotesRead>().add_systems(
            Update,
            (read_notes, put_up_board, pin_cards, press_cards).chain(),
        );
    }
}

/// The notes read in this career, in the order read: "map/name".
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct NotesRead(pub Vec<String>);

/// The board, truck space: on the right canvas wall toward the tailgate,
/// facing in, above the first-aid box (`art/vehicles/truck.py`, its top at
/// FLOOR + 1.27) and clear of the bench display and the route card.
pub const BOARD_AT: Vec3 = Vec3::new(1.12, FLOOR + 1.62, 1.1);
/// Its face, metres (local x across, y up): up to the canopy's top rail.
pub const BOARD: Vec2 = Vec2::new(1.2, 0.70);
/// A pinned copy: the note's name, big enough to read from the bed.
pub const CARD: Vec2 = Vec2::new(0.15, 0.054);
/// A column a valley, as wide as its notes need: rows down, cards across.
const ROW: f32 = 0.060;
/// The gap between cards in a row.
const GAP: f32 = 0.012;
/// The top band of the board, for the valleys' names.
const HEADER: f32 = 0.07;
/// Rows of cards a column holds.
pub const ROWS: usize = ((BOARD.y - HEADER - 0.02) / ROW) as usize;

/// Each valley's column: its left edge (board space) and cards a row,
/// sized so every note there is fits.
pub fn columns() -> &'static [(f32, usize); 3] {
    static COLUMNS: std::sync::OnceLock<[(f32, usize); 3]> = std::sync::OnceLock::new();
    COLUMNS.get_or_init(|| {
        let ids: Vec<String> = every_note().into_iter().map(|(i, _)| i).collect();
        let mut counts = [0usize; 3];
        for (_, col, _) in layout(&ids) {
            counts[col] += 1;
        }
        let mut out = [(0.0, 1); 3];
        let mut x = -BOARD.x * 0.5;
        for (col, n) in counts.iter().enumerate() {
            let across = n.div_ceil(ROWS).max(1);
            out[col] = (x, across);
            x += column_width(across);
        }
        out
    })
}

fn column_width(across: usize) -> f32 {
    across as f32 * (CARD.x + GAP) + GAP
}

/// How close your head comes, and how long you look, to have read a note.
const READ_REACH: f32 = 1.4;
const READ_CONE_DEG: f32 = 35.0;
const READ_TIME: f32 = 1.0;

/// The board in the truck.
#[derive(Component)]
pub struct CorkBoard;

/// A pinned copy of a read note.
#[derive(Component, Debug, Clone)]
pub struct Pin {
    pub id: String,
}

/// A note taken down from the board to read.
#[derive(Component, Debug, Clone)]
pub struct TakenDown {
    pub id: String,
}

/// A valley's name over its column.
#[derive(Component)]
pub struct ColumnHead(pub usize);

/// The red strings on a board, board space: one length per pair of read
/// notes that name each other (drawn by `string_visuals`).
#[derive(Component, Default, Debug, Clone, PartialEq)]
pub struct Strings(pub Vec<(Vec3, Vec3)>);

/// "map/name".
pub fn note_id(map: &str, name: &str) -> String {
    format!("{map}/{name}")
}

fn split(id: &str) -> Option<(&str, &str)> {
    id.split_once('/')
}

/// Every note there is to read, in every valley: (id, text). Map notes and
/// the staged scenes' notes; terminals are screens, not paper.
pub fn every_note() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (map, def) in crate::progression::valleys() {
        for m in &def.markers {
            if let MarkerKind::Note { text } = &m.kind {
                out.push((note_id(map, &m.name), text.clone()));
            }
        }
        for (_, s) in crate::world::scenes::of(map) {
            if let Some(n) = &s.note {
                out.push((note_id(map, &n.name), n.text.clone()));
            }
        }
    }
    out
}

fn note_text(id: &str) -> Option<String> {
    every_note()
        .into_iter()
        .find(|(i, _)| i == id)
        .map(|(_, t)| t)
}

/// Where this valley's notes lie that you could read now: (id, centre).
pub fn readable_here(
    zone: &Zone,
    map: &str,
    staged: &crate::world::scenes::Staged,
) -> Vec<(String, Vec3)> {
    let mut out = Vec::new();
    for m in &zone.map.markers {
        if matches!(m.kind, MarkerKind::Note { .. }) {
            let y = crate::world::notes::rest_height(zone, m.at[0], m.at[1], 2.2 + m.y);
            out.push((note_id(map, &m.name), Vec3::new(m.at[0], y, m.at[1])));
        }
    }
    for (id, s) in crate::world::scenes::of(map) {
        if !staged.0.contains(&id) {
            continue;
        }
        if let Some(n) = &s.note {
            let t = crate::world::scenes::note_pose(crate::world::scenes::pose(zone, s), n);
            out.push((note_id(map, &n.name), t.translation));
        }
    }
    out
}

/// Whether a head (eye point, looking along `forward`) is reading a note
/// at `at`.
pub fn reading(eye: Vec3, forward: Vec3, at: Vec3) -> bool {
    let d = at - eye;
    d.length() < READ_REACH && forward.angle_between(d).to_degrees() < READ_CONE_DEG
}

/// Notes are read by looking at them: near, turned to them, for a moment.
#[allow(clippy::too_many_arguments)]
fn read_notes(
    zone: Option<Res<Zone>>,
    opts: Res<LaunchOptions>,
    staged: Option<Res<crate::world::scenes::Staged>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    time: Res<Time>,
    mut read: ResMut<NotesRead>,
    mut firsts: MessageWriter<crate::onboarding::firsts::MeetFirst>,
    mut cache: Local<(usize, Vec<(String, Vec3)>)>,
    mut dwell: Local<HashMap<String, f32>>,
) {
    let (Some(zone), Some(staged)) = (zone, staged) else {
        return;
    };
    if cache.1.is_empty() || cache.0 != staged.0.len() {
        *cache = (staged.0.len(), readable_here(&zone, &opts.map, &staged));
    }
    let Some(head) = heads
        .iter()
        .find(|(_, parent)| locals.contains(parent.parent()))
        .map(|(g, _)| g)
    else {
        return;
    };
    let eye = head.translation();
    let forward = head.forward().as_vec3();
    for (id, at) in &cache.1 {
        if read.0.contains(id) || !reading(eye, forward, *at) {
            continue;
        }
        let t = dwell.entry(id.clone()).or_default();
        *t += time.delta_secs();
        if *t >= READ_TIME {
            // The first ever: Lev says he keeps copies (once a career).
            firsts.write(crate::onboarding::firsts::MeetFirst(
                crate::onboarding::firsts::First::NotePinned,
            ));
            info!("read: {id}");
            read.0.push(id.clone());
        }
    }
}

/// The board goes up in the truck once there is one.
fn put_up_board(
    mut commands: Commands,
    trucks: Query<Entity, With<Truck>>,
    boards: Query<(), With<CorkBoard>>,
) {
    if !boards.is_empty() {
        return;
    }
    let Some(truck) = trucks.iter().next() else {
        return;
    };
    commands.spawn((
        Name::new("Cork board"),
        CorkBoard,
        Strings::default(),
        Transform::from_translation(BOARD_AT)
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        Visibility::default(),
        ChildOf(truck),
    ));
}

/// Where a valley's `n`th read note is pinned, board space (a little
/// crooked, as pinned things are).
pub fn card_pose(column: usize, n: usize, id: &str) -> Transform {
    let (left, across) = columns()[column];
    let n = n.min(ROWS * across - 1);
    let (row, side) = (n / across, n % across);
    let x = left + GAP + (CARD.x + GAP) * side as f32 + CARD.x * 0.5;
    let y = BOARD.y * 0.5 - HEADER - CARD.y * 0.5 - ROW * row as f32;
    let h = id
        .bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let tilt = ((h % 9) as f32 - 4.0) * 0.012;
    Transform::from_xyz(x, y, 0.014).with_rotation(Quat::from_rotation_z(tilt))
}

/// Where each read note is pinned: (id, column, place in column).
pub fn layout(read: &[String]) -> Vec<(String, usize, usize)> {
    let valleys = crate::world::story::VALLEYS;
    let mut counts = [0usize; 3];
    let mut out = Vec::new();
    for id in read {
        let Some(col) = split(id).and_then(|(m, _)| valleys.iter().position(|v| *v == m)) else {
            continue;
        };
        out.push((id.clone(), col, counts[col]));
        counts[col] += 1;
    }
    out
}

/// The pairs of read notes that name each other (by the story's thread).
pub fn strung(read: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for lead in crate::world::story::THREAD {
        let a = note_id(lead.map, lead.note);
        if !read.contains(&a) {
            continue;
        }
        for (map, place) in lead.mentions {
            let b = note_id(map, place);
            if b != a
                && read.contains(&b)
                && !out
                    .iter()
                    .any(|(x, y)| (x == &a && y == &b) || (x == &b && y == &a))
            {
                out.push((a.clone(), b));
            }
        }
    }
    out
}

/// Keeps the board's cards, column names and strings to what's been read.
fn pin_cards(
    mut commands: Commands,
    read: Res<NotesRead>,
    mut boards: Query<(Entity, &mut Strings), With<CorkBoard>>,
    added: Query<(), Added<CorkBoard>>,
    pins: Query<&Pin>,
    heads: Query<&ColumnHead>,
) {
    let Some((board, mut strings)) = boards.iter_mut().next() else {
        return;
    };
    if !read.is_changed() && added.is_empty() {
        return;
    }
    let placed = layout(&read.0);
    let mut poses = HashMap::new();
    for (id, col, n) in &placed {
        let t = card_pose(*col, *n, id);
        poses.insert(id.clone(), t);
        if pins.iter().any(|p| &p.id == id) {
            continue;
        }
        let name = split(id).map_or(id.as_str(), |(_, n)| n);
        commands.spawn((
            Name::new("Pinned note"),
            Pin { id: id.clone() },
            crate::render::legibility::TextSource::PinnedNote,
            ScreenText {
                text: crate::render::screen::wrap_to_fit(name, CARD, ScreenStyle::Card),
                size: CARD,
                style: ScreenStyle::Card,
            },
            Pressable::Note,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(CARD.x, CARD.y, 0.01),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            t,
            Visibility::default(),
            ChildOf(board),
        ));
    }
    for col in 0..3 {
        let used = placed.iter().any(|(_, c, _)| *c == col);
        if used && !heads.iter().any(|h| h.0 == col) {
            let name = crate::progression::valleys()[col].1.name.to_uppercase();
            commands.spawn((
                Name::new("Board column"),
                ColumnHead(col),
                crate::render::legibility::TextSource::BoardColumn,
                ScreenText {
                    text: name,
                    size: Vec2::new(column_width(columns()[col].1) - 0.03, HEADER - 0.015),
                    style: ScreenStyle::Card,
                },
                Transform::from_xyz(
                    columns()[col].0 + column_width(columns()[col].1) * 0.5,
                    BOARD.y * 0.5 - HEADER * 0.5,
                    0.012,
                ),
                Visibility::default(),
                ChildOf(board),
            ));
        }
    }
    // A pin at each card's top; the string runs pin to pin.
    let pin = |t: &Transform| t.translation + t.rotation * Vec3::new(0.0, CARD.y * 0.35, 0.004);
    let want = Strings(
        strung(&read.0)
            .iter()
            .filter_map(|(a, b)| Some((pin(poses.get(a)?), pin(poses.get(b)?))))
            .collect(),
    );
    if *strings != want {
        *strings = want;
    }
}

/// Press a card to take the note down and read it; press it (or the note)
/// again to pin it back. One down at a time.
fn press_cards(
    mut commands: Commands,
    mut events: MessageReader<InteractEvent>,
    pins: Query<(&Pin, &Transform, &ChildOf)>,
    down: Query<(Entity, &TakenDown)>,
) {
    for e in events.read() {
        let InteractEvent::Pressed { item, .. } = e else {
            continue;
        };
        if let Ok((_, d)) = down.get(*item) {
            let _ = d;
            commands.entity(*item).despawn();
            continue;
        }
        let Ok((pin, t, parent)) = pins.get(*item) else {
            continue;
        };
        let was = down.iter().find(|(_, d)| d.id == pin.id).map(|(e, _)| e);
        for (e, _) in &down {
            commands.entity(e).despawn();
        }
        if was.is_some() {
            continue;
        }
        let Some(text) = note_text(&pin.id) else {
            continue;
        };
        let note = crate::world::notes::note_panel(&text);
        let size = note.size;
        // Off the board toward you, kept on the board's face.
        let x = t
            .translation
            .x
            .clamp(-BOARD.x * 0.5 + size.x * 0.5, BOARD.x * 0.5 - size.x * 0.5);
        commands.spawn((
            Name::new("Note taken down"),
            TakenDown { id: pin.id.clone() },
            crate::render::legibility::TextSource::Note,
            note,
            Pressable::Note,
            Grabbable {
                points: vec![GrabPoint::free()],
            },
            Collider::cuboid(size.x, size.y, 0.01),
            CollisionLayers::new(Layer::Item, LayerMask::NONE),
            Transform::from_xyz(x, 0.0, 0.2),
            Visibility::default(),
            ChildOf(parent.parent()),
        ));
    }
}

/// The red string: a sagging ribbon pin to pin, all of a board's in one
/// mesh. Segments a length.
const STRING_SEGMENTS: usize = 10;
const STRING_WIDTH: f32 = 0.004;

/// Triangles in a board's strings.
pub fn string_tris(lengths: usize) -> usize {
    lengths * STRING_SEGMENTS * 2
}

/// The strings' mesh, board space (facing +z).
pub fn string_mesh(strings: &Strings) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for (a, b) in &strings.0 {
        let sag = 0.05 * a.distance(*b).min(1.0);
        let across = (*b - *a).cross(Vec3::Z).normalize_or(Vec3::Y) * STRING_WIDTH * 0.5;
        let base = pos.len() as u32;
        for i in 0..=STRING_SEGMENTS {
            let s = i as f32 / STRING_SEGMENTS as f32;
            // A hanging curve: lowest in the middle.
            let p = a.lerp(*b, s) - Vec3::Y * sag * 4.0 * s * (1.0 - s) + Vec3::Z * 0.003;
            pos.push((p - across).into());
            pos.push((p + across).into());
        }
        for i in 0..STRING_SEGMENTS as u32 {
            let k = base + i * 2;
            idx.extend([k, k + 1, k + 2, k + 1, k + 3, k + 2]);
        }
    }
    let n = pos.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
    .with_inserted_indices(Indices::U32(idx))
}

/// Client: the cork, its frame, and the red string.
pub struct BoardVisualsPlugin;

impl Plugin for BoardVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (dress_board, string_visuals));
    }
}

#[derive(Component)]
struct StringMesh;

use crate::render::world_props::{Blockout, SwapIn};

fn dress_board(
    mut commands: Commands,
    boards: Query<Entity, Added<CorkBoard>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for board in &boards {
        let cork = materials.add(StandardMaterial {
            base_color: Color::srgb(0.47, 0.34, 0.22),
            perceptual_roughness: 0.95,
            ..default()
        });
        let frame = materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.24, 0.18),
            perceptual_roughness: 0.8,
            ..default()
        });
        // Boxes until the board's model (`cork_board`) is in.
        commands.entity(board).insert(SwapIn::one("cork_board"));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(BOARD.x, BOARD.y, 0.02))),
            MeshMaterial3d(cork),
            Transform::from_xyz(0.0, 0.0, 0.0),
            Blockout,
            ChildOf(board),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(BOARD.x + 0.05, BOARD.y + 0.05, 0.012))),
            MeshMaterial3d(frame),
            Transform::from_xyz(0.0, 0.0, -0.008),
            Blockout,
            ChildOf(board),
        ));
    }
}

fn string_visuals(
    mut commands: Commands,
    boards: Query<(Entity, &Strings), Changed<Strings>>,
    old: Query<(Entity, &ChildOf), With<StringMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut red: Local<Option<Handle<StandardMaterial>>>,
) {
    for (board, strings) in &boards {
        for (e, parent) in &old {
            if parent.parent() == board {
                commands.entity(e).despawn();
            }
        }
        if strings.0.is_empty() {
            continue;
        }
        let red = red
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.62, 0.08, 0.06),
                    perceptual_roughness: 0.9,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })
            })
            .clone();
        commands.spawn((
            StringMesh,
            Mesh3d(meshes.add(string_mesh(strings))),
            MeshMaterial3d(red),
            Transform::IDENTITY,
            ChildOf(board),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::legibility::{TextSource, check_panel};

    /// Every card that could be pinned reads from the bed, and every
    /// valley's notes fit their column without overlapping.
    #[test]
    fn every_card_reads_and_fits() {
        let all = every_note();
        assert!(all.len() > 40, "{} notes", all.len());
        for (id, _) in &all {
            let name = split(id).unwrap().1;
            let card = ScreenText {
                text: crate::render::screen::wrap_to_fit(name, CARD, ScreenStyle::Card),
                size: CARD,
                style: ScreenStyle::Card,
            };
            if let Err(e) = check_panel(&card, TextSource::PinnedNote) {
                panic!("{id}: {e}");
            }
        }
        let ids: Vec<String> = all.iter().map(|(i, _)| i.clone()).collect();
        // Every valley's notes fit its column, and the columns the board.
        for col in 0..3 {
            let n = layout(&ids).iter().filter(|(_, c, _)| *c == col).count();
            let (_, across) = columns()[col];
            assert!(
                n <= ROWS * across,
                "column {col}: {n} notes > {}",
                ROWS * across
            );
        }
        let (left, across) = columns()[2];
        assert!(
            left + column_width(across) <= BOARD.x * 0.5 + 1e-4,
            "the columns run off the board: {:?}",
            columns()
        );
        // And the ids are unique (a name twice in a valley would pin one).
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(seen.insert(id), "{id} twice");
        }
    }

    /// Strings join only read notes that name each other.
    #[test]
    fn strings_join_only_read_notes_that_name_each_other() {
        let a = note_id("marrow_creek", "Rusty's note");
        let b = note_id("marrow_creek", "Village note");
        let c = note_id("cinder_works", "House note");
        assert!(strung(&[a.clone()]).is_empty(), "one note, no string");
        assert_eq!(
            strung(&[a.clone(), b.clone()]),
            vec![(a.clone(), b.clone())]
        );
        assert_eq!(
            strung(&[b.clone(), c.clone()]).len(),
            0,
            "they don't name each other"
        );
        let all: Vec<String> = every_note().into_iter().map(|(i, _)| i).collect();
        for (x, y) in strung(&all) {
            let named = |p: &str, q: &str| {
                let (pm, pn) = split(p).unwrap();
                let (qm, qn) = split(q).unwrap();
                crate::world::story::THREAD
                    .iter()
                    .any(|l| l.map == pm && l.note == pn && l.mentions.contains(&(qm, qn)))
            };
            assert!(named(&x, &y) || named(&y, &x), "{x} -- {y}");
        }
    }

    /// Everything read: the board stays a small part of the truck's frame
    /// (the whole frame is 1200 draws, 900k triangles).
    #[test]
    fn the_board_stays_within_budget() {
        let all: Vec<String> = every_note().into_iter().map(|(i, _)| i).collect();
        // Cards, three column names, the cork, its frame, one string mesh,
        // one note taken down.
        let draws = all.len() + 3 + 2 + 1 + 1;
        assert!(draws <= 80, "{draws} draws");
        let tris = string_tris(strung(&all).len()) + all.len() * 2 + 24;
        assert!(tris <= 5_000, "{tris} triangles");
        let mesh = string_mesh(&Strings(vec![(Vec3::ZERO, Vec3::X); 3]));
        assert_eq!(mesh.indices().unwrap().len() / 3, string_tris(3));
    }

    /// Looking at a note up close reads it; from across the room, or
    /// looking away, doesn't.
    #[test]
    fn reading_is_looking() {
        let note = Vec3::new(0.0, 0.8, -0.6);
        let eye = Vec3::new(0.0, 1.6, 0.0);
        assert!(reading(eye, (note - eye).normalize(), note));
        assert!(!reading(eye, Vec3::Z, note), "looking away");
        assert!(
            !reading(eye + Vec3::Z * 3.0, (note - eye).normalize(), note),
            "too far"
        );
    }

    /// What's pinned is what was read, and it comes back with the career.
    #[test]
    fn pinned_set_is_the_read_set_across_save_and_load() {
        let dir = std::env::temp_dir().join(format!("extraction-board-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("career.ron");
        let app_with = |path: &std::path::Path| {
            let opts = LaunchOptions {
                robots: false,
                save: Some(path.to_owned()),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..4 {
                app.update();
            }
            app
        };
        let pinned = |app: &mut App| {
            let mut ids: Vec<String> = app
                .world_mut()
                .query::<&Pin>()
                .iter(app.world())
                .map(|p| p.id.clone())
                .collect();
            ids.sort();
            ids
        };
        let mut a = app_with(&path);
        assert!(pinned(&mut a).is_empty(), "nothing read, nothing pinned");
        let read = vec![
            note_id("marrow_creek", "Village note"),
            note_id("marrow_creek", "Gate log"),
            note_id("kettle_pass", "Refuge board"),
        ];
        a.world_mut().resource_mut::<NotesRead>().0 = read.clone();
        for _ in 0..2 {
            a.update();
        }
        let mut want = read.clone();
        want.sort();
        assert_eq!(pinned(&mut a), want);
        let strings = a
            .world_mut()
            .query::<&Strings>()
            .single(a.world())
            .unwrap()
            .0
            .len();
        assert_eq!(strings, 1, "the gate log and her letter name each other");
        let save = crate::persist::snapshot(a.world_mut());
        let mut career = a
            .world()
            .resource::<crate::persist::CareerState>()
            .0
            .clone();
        career.store(&save);
        crate::persist::write_career(&path, &career);

        let mut b = app_with(&path);
        assert_eq!(
            b.world().resource::<NotesRead>().0,
            read,
            "kept with the career"
        );
        assert_eq!(pinned(&mut b), want, "and pinned again");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Through the headset path: stand at the gate booth's table and look
    /// down at the duty log, and it's read; looking straight ahead from
    /// there isn't enough.
    #[test]
    fn a_headset_reads_a_note_by_looking_at_it() {
        use crate::rig::RigInput;
        let mut vr = crate::vr_harness::Vr::new("marrow_creek");
        let id = note_id("marrow_creek", "Gate log");
        let at = {
            let w = vr.app.world();
            let zone = w.resource::<Zone>();
            let staged = w.resource::<crate::world::scenes::Staged>();
            readable_here(zone, "marrow_creek", staged)
                .into_iter()
                .find(|(i, _)| *i == id)
                .unwrap()
                .1
        };
        // Feet on the booth's floor, facing -z, the note half a metre ahead.
        let ground = crate::world::notes::rest_height(
            vr.app.world().resource::<Zone>(),
            at.x,
            at.z + 0.5,
            0.6,
        );
        vr.stand(Vec3::new(at.x, ground, at.z + 0.5), 0.0);
        let look = |vr: &mut crate::vr_harness::Vr, pitch: f32| {
            vr.app
                .world_mut()
                .get_mut::<RigInput>(vr.player)
                .unwrap()
                .head =
                Transform::from_xyz(0.0, 1.6, 0.0).with_rotation(Quat::from_rotation_x(pitch));
        };
        look(&mut vr, 0.3);
        vr.step(150);
        assert!(
            !vr.app.world().resource::<NotesRead>().0.contains(&id),
            "looking up, not read"
        );
        look(&mut vr, -1.0);
        vr.step(150);
        assert!(
            vr.app.world().resource::<NotesRead>().0.contains(&id),
            "looking at it, read"
        );
        let met = vr
            .app
            .world()
            .resource::<crate::onboarding::firsts::Firsts>();
        assert!(
            met.met(crate::onboarding::firsts::First::NotePinned),
            "Lev's first-pin moment is met"
        );
    }

    /// Through the headset path: a hand on a pinned card takes the note
    /// down; again, and it's back on the board.
    #[test]
    fn a_hand_takes_a_note_down_and_pins_it_back() {
        use crate::rig::Side;
        let mut vr = crate::vr_harness::Vr::new("marrow_creek");
        vr.app.world_mut().resource_mut::<NotesRead>().0 =
            vec![note_id("marrow_creek", "Gate log")];
        vr.step(3);
        let card = vr
            .app
            .world_mut()
            .query_filtered::<Entity, With<Pin>>()
            .iter(vr.app.world())
            .next()
            .expect("pinned");
        let squeeze = |vr: &mut crate::vr_harness::Vr, e: Entity| {
            let p = vr
                .app
                .world()
                .get::<GlobalTransform>(e)
                .unwrap()
                .translation();
            vr.hand(Side::Right, p, Quat::IDENTITY, 0.0, 0.0);
            vr.step(2);
            vr.grip(Side::Right, 1.0);
            vr.step(3);
            vr.grip(Side::Right, 0.0);
            vr.step(3);
        };
        let down = |vr: &mut crate::vr_harness::Vr| {
            vr.app
                .world_mut()
                .query_filtered::<Entity, With<TakenDown>>()
                .iter(vr.app.world())
                .next()
        };
        squeeze(&mut vr, card);
        let note = down(&mut vr).expect("taken down");
        let text = &vr.app.world().get::<ScreenText>(note).unwrap().text;
        assert!(text.contains("Bus 4"), "the whole note: {text}");
        squeeze(&mut vr, note);
        assert!(down(&mut vr).is_none(), "pinned back");
    }

    /// For a look at the board: `BOARD_CAREER=<path> cargo test
    /// write_a_career_for_a_shot -- --ignored --nocapture` writes a career
    /// that has read most of the notes, and prints a camera that faces the
    /// board (`--save <path> --camera ...`).
    #[test]
    #[ignore]
    fn write_a_career_for_a_shot() {
        let path = std::path::PathBuf::from(std::env::var("BOARD_CAREER").expect("BOARD_CAREER"));
        let opts = LaunchOptions {
            robots: false,
            save: Some(path.clone()),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let all: Vec<String> = every_note().into_iter().map(|(i, _)| i).collect();
        app.world_mut().resource_mut::<NotesRead>().0 = all.into_iter().take(36).collect();
        for _ in 0..3 {
            app.update();
        }
        let save = crate::persist::snapshot(app.world_mut());
        let mut career = app
            .world()
            .resource::<crate::persist::CareerState>()
            .0
            .clone();
        career.store(&save);
        crate::persist::write_career(&path, &career);
        let g = *app
            .world_mut()
            .query_filtered::<&GlobalTransform, With<CorkBoard>>()
            .single(app.world())
            .unwrap();
        let normal = g.rotation() * Vec3::Z;
        let eye = g.translation() + normal * 1.3;
        let look = -normal;
        // Camera yaw: 0 looks toward -Z, 90 toward +X.
        let yaw = look.x.atan2(-look.z).to_degrees();
        println!(
            "CAMERA {:.2},{:.2},{:.2},{:.1},-5",
            eye.x, eye.y, eye.z, yaw
        );
    }
}
