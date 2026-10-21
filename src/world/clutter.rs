//! Lived-in clutter: what people left in the common houses, by valley and
//! by what's happened to the house (`art/props/clutter.py`). Visual only:
//! no colliders, no loot, nothing the logic sees; `buildings::dress_buildings`
//! adds it to a house's near meshes.
//!
//! Marrow's homes were evacuated and are half-packed; Kettle's are miners'
//! rows (boots by the door, a work jacket on the peg, helmets and lamps);
//! Cinder's are sooted, the contents fire-damaged. A damaged house's things
//! are on the floor, a burnt one's are charred. A few houses Story names
//! have their own (see `special`).
//!
//! Each thing goes in a slot on `buildings::house`'s plan: the table top,
//! the shelves' boards that carry no loot, the bed, the floor by the door,
//! the wall beside it, the sills. Slots within [`NOTE_CLEAR`] of a note or
//! terminal marker are left empty, so a note never lies on a cup.

use bevy::prelude::*;
use extraction_core::map::{MapDef, MarkerKind};

use super::buildings::Ruin;

/// How near (xz) a note or terminal marker keeps a slot empty, metres.
pub const NOTE_CLEAR: f32 = 0.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Valley {
    Marrow,
    Kettle,
    Cinder,
}

fn valley(map: &MapDef) -> Option<Valley> {
    let n = map.name.to_lowercase();
    if n.contains("marrow") {
        Some(Valley::Marrow)
    } else if n.contains("kettle") {
        Some(Valley::Kettle)
    } else if n.contains("cinder") {
        Some(Valley::Cinder)
    } else {
        None
    }
}

/// The houses built by `buildings::house` (and their ruins): width, depth.
fn plan(base: &str) -> Option<(f32, f32)> {
    match base {
        "house_plank" => Some((8.0, 7.0)),
        "farmhouse" | "house_brick" => Some((10.0, 8.0)),
        _ => None,
    }
}

/// A house's slots, building-local, as `buildings::house` lays it out
/// (floor 0.5 up, the door in the middle of the front, -Z, wall).
struct Slots {
    /// The table's top, its two ends (the middle is the loot's).
    table: [Vec3; 2],
    /// The front room's shelf: its bottom and top boards (the middle two
    /// carry loot).
    shelf: [Vec3; 2],
    /// The kitchen shelf's bottom board, the counter's far end.
    kitchen: [Vec3; 2],
    bed: Vec3,
    /// The floor inside the door, either side.
    door_floor: [Vec3; 2],
    /// The front wall's inside face beside the door, at hook height.
    door_wall: [Vec3; 2],
    /// The front room's floor, its middle.
    room_floor: Vec3,
    /// The partition's face towards the front room, between its doorways.
    partition: Vec3,
    /// The windows' sills: (middle, length along the wall, wall's yaw).
    sills: Vec<(Vec3, f32, f32)>,
}

fn slots(w: f32, d: f32) -> Slots {
    let f = 0.5;
    let (x, z) = (w * 0.5, d * 0.5);
    let pz = z - d * 0.42;
    let table = Vec3::new(-x * 0.4, f + 0.785, -z * 0.2);
    let shelf = Vec3::new(x - 0.3, f, -z * 0.4);
    let mut sills = Vec::new();
    // Inside the walls' middles (as `shell` builds them: front, +X, back,
    // -X; the sill's top 0.925 up its wall).
    let sill_y = f + 0.9 + 0.025;
    for s in [w * 0.2, w * 0.8] {
        sills.push((Vec3::new(-x + s, sill_y, -z), 1.0, 0.0));
    }
    for s in [d * 0.3, d * 0.75] {
        sills.push((
            Vec3::new(x, sill_y, -z + s),
            1.0,
            std::f32::consts::FRAC_PI_2,
        ));
    }
    Slots {
        table: [table - Vec3::X * 0.4, table + Vec3::X * 0.42],
        shelf: [shelf + Vec3::Y * 0.315, shelf + Vec3::Y * 1.665],
        kitchen: [
            Vec3::new(-x * 0.6, f + 0.315, z - 0.3),
            Vec3::new(-x + 0.4, f + 0.9, z - 1.4 - 0.6),
        ],
        bed: Vec3::new(x - 0.6, f + 0.56, z - 1.4),
        door_floor: [
            Vec3::new(-0.95, f, -z + 0.45),
            Vec3::new(0.95, f, -z + 0.45),
        ],
        door_wall: [
            Vec3::new(-1.1, f + 1.7, -z + 0.09),
            Vec3::new(1.1, f + 1.7, -z + 0.09),
        ],
        room_floor: Vec3::new(x * 0.25, f, -z * 0.45),
        partition: Vec3::new(0.0, f + 1.05, pz - 0.05),
        sills,
    }
}

/// A thing in a slot: on a surface, turned `yaw`; or for the walls, its
/// back to the wall (models face -Z, so a wall item on the front wall's
/// inside turns round).
fn on(model: &'static str, at: Vec3, yaw: f32) -> (&'static str, Transform) {
    (
        model,
        Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
    )
}

fn wall(model: &'static str, at: Vec3) -> (&'static str, Transform) {
    on(model, at, std::f32::consts::PI)
}

/// What a house holds, building-local: `prefab` placed at `xf` (its tag
/// `tag`) on `map`; `epilogue` once Lev has been home
/// (`raid::epilogue::Epilogue`), which hangs the collar on Hana's lead.
pub fn dress(
    map: &MapDef,
    prefab: &str,
    tag: &str,
    xf: &Transform,
    epilogue: bool,
) -> Vec<(&'static str, Transform)> {
    // `EXTRACTION_NO_CLUTTER=1`: the bare rooms, for comparisons.
    let Some(valley) =
        valley(map).filter(|_| std::env::var("EXTRACTION_NO_CLUTTER").map_or(true, |v| v != "1"))
    else {
        return Vec::new();
    };
    let base = extraction_core::map::prefab_base(prefab);
    let Some((w, d)) = plan(base) else {
        return Vec::new();
    };
    // A drowned house's things went with the water.
    if prefab.ends_with("_drowned") {
        return Vec::new();
    }
    let ruin = if prefab.ends_with("_burnt") {
        Some(Ruin::Burnt)
    } else if prefab.ends_with("_damaged") {
        Some(Ruin::Damaged)
    } else {
        None
    };
    let s = slots(w, d);
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let story = special(valley, base, tag, at);
    let mut out = Vec::new();
    match ruin {
        Some(Ruin::Burnt) => {
            out.push(on(
                "books_charred",
                s.room_floor + Vec3::new(-0.6, 0.0, 0.3),
                0.4,
            ));
            out.push(on("frame_charred", s.door_floor[1] + Vec3::Z * 0.4, 1.1));
            out.push(on(
                "dishes_broken",
                s.kitchen[0] - Vec3::Y * 0.315 + Vec3::Z * -0.5,
                0.0,
            ));
            for &(c, len, yaw) in &s.sills {
                out.push(sill_ash(c, len, yaw));
            }
        }
        Some(Ruin::Drowned) => {}
        Some(Ruin::Damaged) => {
            out.push(on(
                "dishes_broken",
                s.table[0] - Vec3::Y * 0.785 + Vec3::Z * 0.6,
                0.7,
            ));
            out.push(on(
                "picture_frame",
                s.room_floor + Vec3::new(0.7, 0.0, 0.5),
                2.2,
            ));
            out.push(on("books", s.room_floor + Vec3::new(-0.5, 0.0, -0.3), 1.3));
            match valley {
                Valley::Marrow => out.push(on("suitcase_shut", s.door_floor[0], 0.2)),
                Valley::Kettle => out.push(on("boots", s.door_floor[0], 0.3)),
                // The front's sills (a damaged house's +X wall is breached).
                Valley::Cinder => {
                    for &(c, len, yaw) in &s.sills[..2] {
                        out.push(sill_ash(c, len, yaw));
                    }
                }
            }
        }
        None => {
            match valley {
                Valley::Marrow if story != Some(Story::Hana) => {
                    out.push(on("suitcase_open", s.bed, 0.1));
                    out.push(on("suitcase_shut", s.door_floor[0], 0.15));
                    out.push(on(
                        "bundle",
                        s.door_floor[0] + Vec3::new(0.45, 0.0, 0.1),
                        0.0,
                    ));
                    out.push(on("belongings_box", s.room_floor + Vec3::X * 0.6, 0.3));
                    out.push(wall("coat_hook", s.door_wall[0]));
                    out.push(on("picture_frame", s.table[1], 0.3));
                }
                Valley::Marrow => {}
                Valley::Kettle => {
                    out.push(on("boots", s.door_floor[0], 0.0));
                    out.push(wall("jacket_hook", s.door_wall[0]));
                    out.push(on("lunch_pail", s.table[1], 0.0));
                    out.push(on("carbide_lamp", s.kitchen[1], 0.5));
                    out.push(on("helmet_lamp", s.shelf[1], std::f32::consts::FRAC_PI_2));
                }
                Valley::Cinder => {
                    out.push(on("books_charred", s.shelf[1], std::f32::consts::FRAC_PI_2));
                    out.push(on("dishes_broken", s.room_floor + Vec3::Z * 0.4, 0.5));
                    out.push(on("frame_charred", s.door_floor[1], 0.6));
                    for &(c, len, yaw) in &s.sills {
                        out.push(sill_ash(c, len, yaw));
                    }
                }
            }
            if valley != Valley::Cinder {
                out.push(on("rug", s.room_floor, 0.05));
                out.push(on("jars", s.kitchen[0], 0.0));
                out.push(on("books", s.shelf[0], std::f32::consts::FRAC_PI_2));
                if story != Some(Story::Hana) {
                    out.push(on("dishes", s.table[0], 0.2));
                }
            }
        }
    }
    // Who lived here (`Resident`), in a whole house Story hasn't written
    // about: a few of their things where the valley's leave room.
    if let (None, None) = (ruin, story) {
        if let Some(r) = resident(xf, prefab) {
            if r == Resident::OldCouple {
                // Two cups on the table, not the family's dishes.
                out.retain(|(n, _)| *n != "dishes");
            }
            out.extend(r.things(&s, w, d));
        }
    }
    // Story's houses.
    match story {
        Some(Story::Hana) => {
            out.push(on("dog_bowl", s.door_floor[1], 0.0));
            out.push(wall("lead_hook", s.door_wall[1]));
            if epilogue {
                out.push(wall("dog_collar", s.door_wall[1]));
            }
            out.push(wall("coat_hook", s.door_wall[0]));
        }
        Some(Story::ShotFirer) => {
            out.push(on("helmet_lamp", s.table[0] + Vec3::Z * 0.2, -0.4));
            out.push(wall("tally_board", s.door_wall[1] + Vec3::Y * 0.1));
        }
        Some(Story::Foreman) => {
            out.push(wall("broom", s.door_floor[1] + Vec3::new(0.3, 0.0, -0.36)));
        }
        Some(Story::PostOffice) => {
            out.push(on("pigeonholes", s.partition, 0.0));
        }
        Some(Story::MineOffice) => {
            out.push(on("ledger_open", s.kitchen[1] + Vec3::Z * 0.2, 0.3));
        }
        None => {}
    }
    // Clear of notes.
    let near_note = |p: Vec3| {
        let wp = xf.transform_point(p);
        map.markers.iter().any(|m| {
            matches!(
                m.kind,
                MarkerKind::Note { .. } | MarkerKind::Terminal { .. }
            ) && Vec2::new(m.at[0] - wp.x, m.at[1] - wp.z).length() < NOTE_CLEAR
        })
    };
    out.retain(|(_, t)| !near_note(t.translation));
    out
}

/// Ash on a sill: the unit-metre film stretched along it.
fn sill_ash(c: Vec3, len: f32, yaw: f32) -> (&'static str, Transform) {
    (
        "ash_film",
        Transform::from_translation(c)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::new(len, 1.0, 1.0)),
    )
}

/// Who lived in a house, from its seed: each leaves three or so telling
/// things (`art/props/residents.py`). A house in four has nobody to tell
/// of (or nothing that does).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Resident {
    /// The village's teacher: a globe and exercise books on the sills, a
    /// slate on the partition.
    Teacher,
    /// A family with small children: a cot in the corner, a rocking
    /// horse, small boots by the door.
    Family,
    /// A hunter: traps on the peg by the door, a pelt on its frame.
    Hunter,
    /// An old couple: two cups on the table, spectacles on a book on the
    /// sill, a walking stick by the door.
    OldCouple,
    /// A seamstress: the treadle machine under the window, a dress on its
    /// dummy in the corner, pinned cloth on the sill.
    Seamstress,
    /// A radio ham: the set on the sill, its lead out of the window, the
    /// log beside it.
    RadioHam,
    /// A railwayman: the timetable on the partition, his cap and lamp on
    /// the pegs by the door.
    Railwayman,
}

const RESIDENTS: [Resident; 7] = [
    Resident::Teacher,
    Resident::Family,
    Resident::Hunter,
    Resident::OldCouple,
    Resident::Seamstress,
    Resident::RadioHam,
    Resident::Railwayman,
];

/// The resident of the house `prefab` placed at `xf`, if it has one.
pub fn resident(xf: &Transform, prefab: &str) -> Option<Resident> {
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(at, prefab) ^ 0x2e51);
    if rng.f32() < 0.25 {
        return None;
    }
    Some(RESIDENTS[rng.usize(..RESIDENTS.len())])
}

impl Resident {
    /// Their things, building-local, in the slots of a `w` x `d` house.
    fn things(self, s: &Slots, w: f32, d: f32) -> Vec<(&'static str, Transform)> {
        let f = 0.5;
        let (x, z) = (w * 0.5, d * 0.5);
        // On a front sill, a little in from the sash, facing the room.
        let sill = |k: usize, model: &'static str, along: f32| {
            let (c, _, _) = s.sills[k];
            (
                model,
                Transform::from_translation(c + Vec3::new(along, 0.0, 0.1)),
            )
        };
        // The partition's face, a picture's height up.
        let partition = |model: &'static str, along: f32| {
            on(model, s.partition + Vec3::new(along, 0.35, 0.0), 0.0)
        };
        // The front -X corner of the front room, its floor clear.
        let corner = Vec3::new(-x + 0.75, f, -z + 0.5);
        match self {
            Resident::Teacher => vec![
                sill(0, "globe", -0.15),
                sill(1, "exercise_books", 0.1),
                partition("slate_board", -0.5),
            ],
            Resident::Family => vec![
                on("cot", corner, 0.0),
                on(
                    "rocking_horse",
                    s.room_floor + Vec3::new(-0.45, 0.0, 0.85),
                    0.6,
                ),
                on(
                    "small_boots",
                    s.door_floor[1] + Vec3::new(0.3, 0.0, 0.05),
                    0.2,
                ),
            ],
            Resident::Hunter => vec![
                wall("traps_hook", s.door_wall[1]),
                partition("hide_frame", 0.6),
            ],
            Resident::OldCouple => vec![
                on("two_cups", s.table[0], 0.1),
                sill(0, "reading_glasses", 0.2),
                wall("walking_stick", s.door_wall[1]),
            ],
            Resident::Seamstress => vec![
                // Under the front's +X window, facing in.
                on(
                    "sewing_machine",
                    Vec3::new(s.sills[1].0.x, f, -z + 0.09 + 0.3),
                    std::f32::consts::PI,
                ),
                on("dress_form", corner, 0.4),
                sill(0, "cloth_pinned", 0.0),
            ],
            Resident::RadioHam => vec![
                {
                    let (c, _, _) = s.sills[1];
                    on(
                        "radio_set",
                        c + Vec3::new(0.0, 0.0, 0.12),
                        std::f32::consts::PI,
                    )
                },
                sill(0, "logbook", 0.0),
            ],
            Resident::Railwayman => vec![
                partition("timetable", 0.55),
                wall("cap_hook", s.door_wall[1]),
            ],
        }
    }
}

/// The houses Story writes about, found by where they stand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Story {
    /// Hana's, in Marrow: she stayed, so nothing packed; the dog's bowl
    /// and lead by the door; her table bare for her letter.
    Hana,
    /// The shot-firer's in Miners' Row: his helmet on the table, the tally
    /// board on its nail.
    ShotFirer,
    /// Vlasta's, the foreman's in Ash Row: the broom she swept the step
    /// with.
    Foreman,
    /// The post office's sorting rack.
    PostOffice,
    /// The mine office's ledger, open on the counter.
    MineOffice,
}

fn special(valley: Valley, base: &str, tag: &str, at: Vec2) -> Option<Story> {
    let here = |x: f32, z: f32| at.distance(Vec2::new(x, z)) < 1.0;
    match (valley, base, tag) {
        (_, _, "post_office") => Some(Story::PostOffice),
        (_, _, "mine_office") => Some(Story::MineOffice),
        (_, _, "foreman") => Some(Story::Foreman),
        (Valley::Marrow, "house_plank", _) if here(-18.0, 95.0) => Some(Story::Hana),
        (Valley::Kettle, "house_plank", _) if here(-30.0, 72.0) => Some(Story::ShotFirer),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every clutter model is built.
    #[test]
    fn every_clutter_model_exists() {
        let root = crate::asset_root_path();
        for name in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let map = crate::world::load_map_def(name);
            for p in &map.placements {
                let xf = Transform::from_xyz(p.at[0], 0.0, p.at[1]);
                for (model, _) in dress(&map, &p.prefab, &p.tag, &xf, true) {
                    let path = root.join(crate::render::world_props::path(model));
                    assert!(path.exists(), "{}: no model {}", p.prefab, path.display());
                }
            }
        }
    }

    /// Every resident's things are built and stand inside the house,
    /// clear of its loot (as far from a spot as the dishes are, or more).
    #[test]
    fn every_resident_has_their_things() {
        let root = crate::asset_root_path();
        for base in ["house_plank", "farmhouse", "house_brick"] {
            let (w, d) = plan(base).unwrap();
            let s = slots(w, d);
            let geo = crate::world::buildings::prefab(base).unwrap();
            for r in RESIDENTS {
                let things = r.things(&s, w, d);
                assert!(things.len() >= 2, "{r:?}");
                for (model, t) in things {
                    let path = root.join(crate::render::world_props::path(model));
                    assert!(path.exists(), "{r:?}: no model {}", path.display());
                    let p = t.translation;
                    assert!(
                        p.x.abs() < w * 0.5 && p.z.abs() < d * 0.5,
                        "{r:?} {model} at {p}"
                    );
                    for l in &geo.loot {
                        assert!(l.at.distance(p) > 0.35, "{r:?} {model} on {} loot", l.table);
                    }
                }
            }
        }
        // Most houses have someone.
        let map = crate::world::load_map_def("kettle_pass");
        let houses: Vec<_> = map
            .placements
            .iter()
            .filter(|p| plan(&p.prefab).is_some())
            .collect();
        let lived = houses
            .iter()
            .filter(|p| resident(&Transform::from_xyz(p.at[0], 0.0, p.at[1]), &p.prefab).is_some())
            .count();
        assert!(lived * 2 > houses.len(), "{lived} of {}", houses.len());
    }

    /// Story's houses get their things, and Hana's table stays bare.
    #[test]
    fn storys_houses_have_their_things() {
        let map = crate::world::load_map_def("marrow_creek");
        let xf = Transform::from_xyz(-18.0, 0.0, 95.0);
        let hana = dress(&map, "house_plank", "", &xf, false);
        let names: Vec<_> = hana.iter().map(|(n, _)| *n).collect();
        assert!(
            names.contains(&"dog_bowl") && names.contains(&"lead_hook"),
            "{names:?}"
        );
        // The lead alone before the ending, the collar on it after.
        assert!(!names.contains(&"dog_collar"), "{names:?}");
        let after = dress(&map, "house_plank", "", &xf, true);
        assert!(after.iter().any(|(n, _)| *n == "dog_collar"));
        assert!(
            !names.iter().any(|n| n.starts_with("suitcase")),
            "{names:?}"
        );
        let s = slots(8.0, 7.0);
        let table = s.table[0].lerp(s.table[1], 0.5);
        assert!(
            hana.iter()
                .all(|(_, t)| t.translation.xz().distance(table.xz()) > 0.8
                    || t.translation.y < table.y - 0.3),
            "something on Hana's table"
        );
    }
}
