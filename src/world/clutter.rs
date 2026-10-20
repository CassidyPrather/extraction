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
/// (`progression::Epilogue`), which hangs the collar on Hana's lead.
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
