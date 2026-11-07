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
    pub(crate) fn screen(&self) -> String {
        let n = self.pages.len();
        let body = self.pages.get(self.page).cloned().unwrap_or_default();
        if n > 1 {
            format!("{body}\n\n[{}/{}]  press for more", self.page + 1, n)
        } else {
            body
        }
    }
}

/// Paper size, metres: a sheet you can read leaning over a table (a
/// little over A4: its letters clear the must-read floor from there,
/// `docs/LEGIBILITY.md`).
pub const NOTE: Vec2 = Vec2::new(0.26, 0.36);

pub fn note_panel(text: &str) -> ScreenText {
    ScreenText {
        text: crate::render::screen::wrap_to_fit(text, NOTE, ScreenStyle::Paper),
        size: NOTE,
        style: ScreenStyle::Paper,
    }
}

/// A signpost's board, big enough to read from a few steps off (the first
/// headset session couldn't read the 0.74 m ones).
pub const SIGN: Vec2 = Vec2::new(1.0, 0.7);

/// A warning board's carrier (`art/props/warnings.py`): the prop, its text
/// area (w x h, m) about its `text_face` node, how high that is, and the
/// material the area is painted (or cast) in. Nothing of the prop stands
/// in front of the area: the words go on it, facing the prop's +Z.
pub struct WarningBoard {
    pub prop: &'static str,
    pub area: Vec2,
    pub centre_y: f32,
    pub ground: &'static str,
}

/// The warning boards, each big enough that a few words read from 10 m and
/// more (Visual design's sizes for `ScreenStyle::Sign`).
pub const WARNING_BOARDS: &[WarningBoard] = &[
    // Across a track: one line of up to ~16 letters, read from ~15 m.
    WarningBoard {
        prop: "warning_gate",
        area: Vec2::new(2.4, 0.5),
        centre_y: 1.1,
        ground: "Cream",
    },
    // On two posts: three short lines, read from ~12 m.
    WarningBoard {
        prop: "warning_hoarding",
        area: Vec2::new(2.0, 1.2),
        centre_y: 1.6,
        ground: "Cream",
    },
    // A railway's enamel plate on a post, at eye height.
    WarningBoard {
        prop: "warning_enamel",
        area: Vec2::new(1.0, 0.6),
        centre_y: 1.45,
        ground: "Enamel",
    },
    // Stencilled on a concrete fence slab.
    WarningBoard {
        prop: "warning_wall",
        area: Vec2::new(1.6, 0.8),
        centre_y: 1.6,
        ground: "Concrete",
    },
    // Stencilled on a steel booth door, on a panel of fresh flat paint
    // (the door's own is blotched with rust).
    WarningBoard {
        prop: "warning_door",
        area: Vec2::new(1.0, 0.6),
        centre_y: 1.6,
        ground: "Pebble",
    },
];

pub fn sign_panel(text: &str) -> ScreenText {
    ScreenText {
        text: crate::render::screen::wrap_to_fit(text, SIGN, ScreenStyle::Sign),
        size: SIGN,
        style: ScreenStyle::Sign,
    }
}

pub fn terminal_panel(text: String) -> ScreenText {
    ScreenText {
        text,
        size: Vec2::new(0.34, 0.26),
        style: ScreenStyle::Crt,
    }
}

/// Every panel a map's markers put up (each terminal page as it shows),
/// named for a report.
pub fn marker_panels(map: &extraction_core::map::MapDef) -> Vec<(String, ScreenText)> {
    let mut out = Vec::new();
    for m in &map.markers {
        match &m.kind {
            MarkerKind::Note { text } => out.push((format!("note {}", m.name), note_panel(text))),
            MarkerKind::Sign { text } => out.push((format!("sign {}", m.name), sign_panel(text))),
            MarkerKind::Terminal { pages } => {
                for page in 0..pages.len() {
                    let term = Terminal {
                        pages: pages.clone(),
                        page,
                    };
                    out.push((
                        format!("terminal {} page {}", m.name, page + 1),
                        terminal_panel(term.screen()),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

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
                    crate::render::legibility::TextSource::Note,
                    crate::interact::read::readable(NOTE),
                    note_panel(text),
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
                // A post and a board, until the signpost model is in
                // (`art/props/outdoor.py`).
                commands
                    .entity(sign)
                    .insert(crate::render::world_props::SwapIn::one("signpost"));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.09, 1.9, 0.09))),
                    MeshMaterial3d(mat(Mat::Timber)),
                    Transform::from_xyz(0.0, 0.8, -0.03),
                    ChildOf(sign),
                    crate::render::world_props::Blockout,
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(SIGN.x + 0.06, SIGN.y + 0.06, 0.03))),
                    MeshMaterial3d(mat(Mat::Planks)),
                    Transform::from_xyz(0.0, 1.45, 0.03),
                    ChildOf(sign),
                    crate::render::world_props::Blockout,
                ));
                // For shots: `EXTRACTION_SIGN_TEXT` puts these words on
                // every sign (a warning at 10 m, before and after).
                let shown = std::env::var("EXTRACTION_SIGN_TEXT").ok();
                commands.spawn((
                    crate::render::legibility::TextSource::Sign,
                    sign_panel(shown.as_deref().unwrap_or(text)),
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
                // The monitor: a deep case, a bezel, the glass. Boxes until
                // its model (`terminal`) is in.
                commands
                    .entity(root)
                    .insert(crate::render::world_props::SwapIn::one("terminal"));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.44, 0.38, 0.4))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.22, -0.06),
                    crate::render::world_props::Blockout,
                    ChildOf(root),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.2, 0.03, 0.2))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.015, -0.06),
                    crate::render::world_props::Blockout,
                    ChildOf(root),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.42, 0.03, 0.15))),
                    MeshMaterial3d(mat(Mat::Polymer)),
                    Transform::from_xyz(0.0, 0.015, 0.25)
                        .with_rotation(Quat::from_rotation_x(0.08)),
                    crate::render::world_props::Blockout,
                    ChildOf(root),
                ));
                commands.spawn((
                    Name::new("Terminal screen"),
                    term,
                    crate::render::legibility::TextSource::Terminal,
                    terminal_panel(text),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a ray from `o` along `d` meets the triangle, as a distance.
    fn hit(o: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
        let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-9 {
            return None;
        }
        let s = o - t[0];
        let u = s.dot(p) / det;
        let q = s.cross(e1);
        let v = d.dot(q) / det;
        if !(0.0..=1.0).contains(&u) || v < 0.0 || u + v > 1.0 {
            return None;
        }
        let k = e2.dot(q) / det;
        (k > 0.0).then_some(k)
    }

    /// Every warning board's model has its `text_face` where the table
    /// says, facing +Z, and its text area is clear: looked at square from
    /// in front, every point of it (a 2 cm grid) is the area's own painted
    /// ground on the face's plane, nothing in front of it. A wobbled plank
    /// once stood 4 mm through a hoarding's paint; a cast rib ran across a
    /// wall's stencil patch.
    #[test]
    fn warning_boards_keep_their_text_areas_clear() {
        let mut bad = Vec::new();
        for b in WARNING_BOARDS {
            let path = crate::asset_root_path().join(crate::render::world_props::path(b.prop));
            let Some(face) = crate::capture::seated::node_transform(&path, "text_face") else {
                bad.push(format!("{}: no text_face", b.prop));
                continue;
            };
            let (_, turn, at) = face.to_scale_rotation_translation();
            if turn.angle_between(Quat::IDENTITY) > 0.01 {
                bad.push(format!("{}: text_face turned off +Z", b.prop));
            }
            if (at.x.abs() > 0.005) || (at.y - b.centre_y).abs() > 0.005 {
                bad.push(format!(
                    "{}: text_face at {at:.3}, not over the origin at {} m",
                    b.prop, b.centre_y
                ));
            }
            let tris = crate::capture::glb::triangles(&path).unwrap();
            let back = face.inverse();
            let local: Vec<([Vec3; 3], &str)> = tris
                .iter()
                .map(|t| (t.p.map(|p| back.transform_point3(p)), t.material.as_str()))
                .collect();
            let (nx, ny) = ((b.area.x / 0.02) as i32, (b.area.y / 0.02) as i32);
            let mut wrong = 0;
            let mut first = None;
            for i in 0..=nx {
                for j in 0..=ny {
                    // Inset 3 mm from the area's edge (the border's own edge).
                    let x = (i as f32 / nx as f32 - 0.5) * (b.area.x - 0.006);
                    let y = (j as f32 / ny as f32 - 0.5) * (b.area.y - 0.006);
                    let o = Vec3::new(x, y, 1.0);
                    let near = local
                        .iter()
                        .filter_map(|(t, m)| hit(o, Vec3::NEG_Z, t).map(|k| (k, *m)))
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    let ok = near.is_some_and(|(k, m)| (1.0 - k).abs() <= 0.001 && m == b.ground);
                    if !ok {
                        wrong += 1;
                        first.get_or_insert((x, y, near));
                    }
                }
            }
            if wrong > 0 {
                let (x, y, near) = first.unwrap();
                bad.push(format!(
                    "{}: {wrong} points of its text area aren't clear {} ground, e.g. at \
                     ({x:.2}, {y:.2}): {near:?} (distance from 1 m in front, material)",
                    b.prop, b.ground
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "warning boards' text areas:\n{}",
            bad.join("\n")
        );
    }
}

#[cfg(test)]
mod sign_tests {
    use super::*;
    use crate::render::legibility::{MUST_READ_DEG, letter_height};
    use crate::render::screen::{em_metres, wrap_to_fit};

    /// The farthest a panel's letters still read (m), at the must-read
    /// floor.
    fn reach(panel: &ScreenText) -> f32 {
        letter_height(&panel.text, em_metres(panel)) / MUST_READ_DEG.to_radians().tan()
    }

    /// As a sign was before it had its own style: a page's letters.
    fn as_page(text: &str) -> ScreenText {
        ScreenText {
            text: wrap_to_fit(text, SIGN, ScreenStyle::Paper),
            size: SIGN,
            style: ScreenStyle::Paper,
        }
    }

    #[test]
    fn a_two_word_warning_reads_from_ten_metres() {
        for text in [
            "KEEP OUT",
            "NO ENTRY",
            "HOT ZONE",
            "DANGER RADIATION",
            "TURN BACK",
        ] {
            let (now, was) = (reach(&sign_panel(text)), reach(&as_page(text)));
            println!("{text:>18}: {was:4.1} m -> {now:4.1} m");
            assert!(now >= 10.0, "{text}: {now:.1} m\n{}", sign_panel(text).text);
            assert!(now > was * 1.5, "{text}: {was:.1} -> {now:.1} m");
        }
    }

    #[test]
    fn a_sign_never_cuts_a_word_and_a_long_one_still_wraps() {
        // One long word, whole (a page's narrowest wrap cut it to 12).
        let p = sign_panel("CONTAMINATION");
        assert_eq!(p.text, "CONTAMINATION");
        // A sentence still wraps to lines, every word whole.
        let long = "Road closed beyond the dam. Walkers patrol the spillway at night; \
                    keep to the fence line and use the north stair.";
        let p = sign_panel(long);
        assert!(p.text.lines().count() >= 3, "{}", p.text);
        let words: Vec<&str> = long.split_whitespace().collect();
        let drawn: Vec<&str> = p.text.split_whitespace().collect();
        assert_eq!(words, drawn);
        // And its letters no smaller than a page's.
        assert!(em_metres(&p) >= em_metres(&as_page(long)) - 1e-4);
    }

    #[test]
    fn notes_are_fitted_as_they_were() {
        // A note is still a page: the sign's style is the sign's alone.
        let text = "Ivan - the key is under the third sleeper from the hut. Don't \
                    take the torch, they see it. Back by Thursday. - M.";
        let p = note_panel(text);
        assert_eq!(p.style, ScreenStyle::Paper);
        let mut was = (0.0, String::new());
        for columns in 12..=60 {
            let w = crate::render::screen::wrap_lines(text, columns);
            let em = em_metres(&ScreenText {
                text: w.clone(),
                size: NOTE,
                style: ScreenStyle::Paper,
            });
            if em > was.0 + 1e-4 {
                was = (em, w);
            }
        }
        assert_eq!(p.text, was.1);
    }
}
