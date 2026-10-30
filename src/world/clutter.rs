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
//! Outside, a house has its yard (`yard`, `art/props/yards.py`): a
//! clothes line, the garden's beds, a bench, the woodpile, by valley.
//!
//! The buildings that aren't houses (cabins, the shop, sheds, barns, the
//! pump station, huts, booths, halls) hold their workers' things instead
//! (`workplace`, `art/props/workplaces.py`).
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
    /// Rime Fell's Coldharbour: snowed in four months a year.
    Rime,
}

fn valley(map: &MapDef) -> Option<Valley> {
    let n = map.name.to_lowercase();
    if n.contains("marrow") {
        Some(Valley::Marrow)
    } else if n.contains("kettle") {
        Some(Valley::Kettle)
    } else if n.contains("cinder") {
        Some(Valley::Cinder)
    } else if n.contains("rime") {
        Some(Valley::Rime)
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
    // The floorboards' top (`Builder::plinth`'s 3 cm floor): what stands
    // on the floor stood 3 cm into it (`capture::footing`).
    let floor = f + 0.03;
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
            Vec3::new(-0.95, floor, -z + 0.45),
            Vec3::new(0.95, floor, -z + 0.45),
        ],
        door_wall: [
            Vec3::new(-1.1, f + 1.7, -z + 0.09),
            Vec3::new(1.1, f + 1.7, -z + 0.09),
        ],
        room_floor: Vec3::new(x * 0.25, floor, -z * 0.45),
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
    if std::env::var("EXTRACTION_NO_CLUTTER").is_ok_and(|v| v == "1") {
        return Vec::new();
    }
    let base = extraction_core::map::prefab_base(prefab);
    // A drowned house was emptied sixty years before the water came: what
    // the silt holds is old and sparse.
    if prefab.ends_with("_drowned") {
        return plan(base).map_or_else(Vec::new, |(w, d)| silted(xf, prefab, w, d));
    }
    // Whom Story's notes name here, if anyone.
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let named = named(map, base, prefab, at);
    let Some((valley, (w, d))) = valley(map).zip(plan(base)) else {
        // Not a house (or not a valley's): Story's things, or else the
        // workers' (`workplace`).
        let things = match named {
            Some(n) => n.things,
            None => workplace(map, prefab, at),
        };
        return clear_of_notes(map, xf, things);
    };
    let ruin = if prefab.ends_with("_burnt") {
        Some(Ruin::Burnt)
    } else if prefab.ends_with("_damaged") {
        Some(Ruin::Damaged)
    } else {
        None
    };
    let s = slots(w, d);
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
                Valley::Rime => out.push(on("felt_boots", s.door_floor[0], 0.3)),
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
                    // In off the shelf's back (the wall behind it).
                    out.push(on(
                        "helmet_lamp",
                        s.shelf[1] - Vec3::X * 0.05,
                        std::f32::consts::FRAC_PI_2,
                    ));
                }
                // Snowed in: the boots and the shovel by the door, the skis
                // on the wall, the stores laid in, the wood by the stove and
                // the wet things drying over it.
                Valley::Rime => {
                    let (x, z) = (w * 0.5, d * 0.5);
                    let f = s.door_floor[0].y;
                    out.push(on("felt_boots", s.door_floor[0], 0.0));
                    out.push(facing(
                        "snow_shovel",
                        Vec3::new(1.45, f, -z + 0.09),
                        Vec3::Z,
                    ));
                    out.push(facing("skis_wall", Vec3::new(-1.45, f, -z + 0.09), Vec3::Z));
                    out.push(on(
                        "winter_stores",
                        Vec3::new(-x + 1.1, f, 1.0 + (z - 3.5)),
                        std::f32::consts::FRAC_PI_2,
                    ));
                    out.push(on("firewood_basket", Vec3::new(-0.6, f, z - 0.5), 0.0));
                    // Over the stove's pipe (`Builder::stove`, -x * 0.35).
                    out.push(on(
                        "stove_pipe_rack",
                        Vec3::new(-x * 0.35, f + 1.7, z - 0.09),
                        0.0,
                    ));
                    out.push(facing(
                        "drying_line",
                        Vec3::new(-x + 0.09, f + 1.75, -z * 0.55),
                        Vec3::X,
                    ));
                    // On the floor by the door (on the table's slot it hung
                    // in the air where the house has no table).
                    out.push(on("ice_axe", s.door_floor[1], 0.4));
                    out.push(on("hurricane_lamp", s.kitchen[1], 0.5));
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
    if let (None, None, None) = (ruin, story, &named) {
        if let Some(r) = resident(xf, prefab, valley == Valley::Cinder) {
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
    if let Some(n) = named {
        out.retain(|(m, _)| !n.drop.contains(m));
        out.extend(n.things);
    }
    clear_of_notes(map, xf, out)
}

/// What Old Sallow's silt holds in a drowned `w` x `d` house: two or three
/// of an iron bedstead, a cart wheel, a mangle and broken crockery, from
/// its seed, on the silt (`buildings::SILT` over the footing).
fn silted(xf: &Transform, prefab: &str, w: f32, d: f32) -> Vec<(&'static str, Transform)> {
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(at, prefab) ^ 0x5117);
    let top = 0.05 + super::buildings::SILT;
    let (x, z) = (w * 0.5, d * 0.5);
    let all = [
        on("bedstead_silted", Vec3::new(x * 0.45, top, z * 0.4), 0.05),
        on("cart_wheel", Vec3::new(-x * 0.45, top, -z * 0.35), 0.8),
        on("mangle", Vec3::new(x * 0.1, top, z * 0.7), 2.6),
        on("dishes_broken", Vec3::new(-x * 0.1, top, -z * 0.6), 1.9),
    ];
    let mut all = all.to_vec();
    rng.shuffle(&mut all);
    all.truncate(2 + rng.usize(..2));
    all
}

/// `things` less those within [`NOTE_CLEAR`] of a note or a terminal.
fn clear_of_notes(
    map: &MapDef,
    xf: &Transform,
    mut things: Vec<(&'static str, Transform)>,
) -> Vec<(&'static str, Transform)> {
    let near_note = |p: Vec3| {
        let wp = xf.transform_point(p);
        map.markers.iter().any(|m| {
            matches!(
                m.kind,
                MarkerKind::Note { .. } | MarkerKind::Terminal { .. }
            ) && Vec2::new(m.at[0] - wp.x, m.at[1] - wp.z).length() < NOTE_CLEAR
        })
    };
    things.retain(|(_, t)| !near_note(t.translation));
    things
}

/// A named resident's things in their house, and the valley's things they
/// take the place of.
struct Named {
    things: Vec<(&'static str, Transform)>,
    drop: &'static [&'static str],
}

/// On a wall's face at `at`, facing into the room along `into` (a wall
/// thing's model stands out to its -Z).
fn facing(model: &'static str, at: Vec3, into: Vec3) -> (&'static str, Transform) {
    on(model, at, (-into.x).atan2(-into.z))
}

/// The people Story's notes name (and a few it asked to be seen), by the
/// house they stand in: their things, building-local. Cabins, shops, the
/// pump station and the chapel too (by their recipes' layouts).
fn named(map: &MapDef, base: &str, prefab: &str, at: Vec2) -> Option<Named> {
    use std::f32::consts::PI;
    let n = map.name.to_lowercase();
    let here = |x: f32, z: f32| at.distance(Vec2::new(x, z)) < 1.0;
    let (px, nx, pz, nz) = (Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z);
    // The log cabin (`buildings::cabin`): 7 x 6, its floor 0.4 up, its
    // walls' insides 3.36 and 2.86 out; the door at x -1.5 in the front.
    let cf = 0.4;
    let (cx, cz) = (3.36, 2.86);
    let things = |v: Vec<(&'static str, Transform)>| {
        Some(Named {
            things: v,
            drop: &[],
        })
    };
    let house = |f: fn(&Slots) -> Vec<(&'static str, Transform)>, drop: &'static [&'static str]| {
        plan(base).map(|(w, d)| Named {
            things: f(&slots(w, d)),
            drop,
        })
    };
    let valley = ["marrow", "kettle", "cinder", "sallow"]
        .into_iter()
        .find(|v| n.contains(v))?;
    match (valley, prefab) {
        // The shop shut on 13 October: "pay me when you're back".
        ("marrow", "shop") if here(-18.2, 3.5) => {
            let f = 0.3;
            things(vec![
                on("till", Vec3::new(-2.2, f + 0.9, -0.45), 0.1),
                on("chit_spike", Vec3::new(-1.8, f + 0.9, -0.3), 0.0),
                // Inside the -X window's glass, read from the street.
                on("closed_card", Vec3::new(-2.5, f + 1.95, -3.5 + 0.16), 0.0),
            ])
        }
        // J., the logging foreman.
        ("marrow", "cabin_log") if here(-110.8, 113.5) => things(vec![
            facing("axe_pegs", Vec3::new(-0.4, cf + 1.5, -cz), pz),
            on("whetstone", Vec3::new(0.38, cf + 0.785, 0.38), 0.3),
            on("boots", Vec3::new(1.95, cf, 2.2), 0.3),
        ]),
        // Tomas of J.'s crew: his tally on the wall by the bed, as his
        // note has it.
        ("marrow", "cabin_log") if here(-70.8, 113.5) => things(vec![facing(
            "tally_board",
            Vec3::new(-cx, cf + 1.45, 1.0),
            px,
        )]),
        // Ondrej Brabec, the pump keeper, who knocked on wood.
        ("marrow", "pump_station") if here(-110.0, 12.1) => {
            let f = 0.3;
            things(vec![
                facing("horseshoe", Vec3::new(0.0, f + 2.3, -4.85), pz),
                facing("barometer", Vec3::new(3.56, f + 1.55, -2.2), px),
                on("kettle", Vec3::new(5.1, f + 0.785, -3.15), 0.4),
            ])
        }
        // The crossing keeper: the railwayman, his flags.
        ("kettle", "house_plank") if here(76.0, 222.0) => house(
            |s| {
                let mut v = Resident::Railwayman.things(s, 8.0, 7.0);
                v.push(on(
                    "signal_flags",
                    s.partition + Vec3::new(-0.55, 0.6, 0.0),
                    0.0,
                ));
                v
            },
            &[],
        ),
        // The weir book's keeper.
        ("kettle", "house_plank") if here(-99.0, 154.0) => house(
            |s| {
                vec![
                    wall("waders", s.door_wall[1] + Vec3::Y * 0.1),
                    on("level_staff", s.partition + Vec3::new(0.55, 0.95, 0.0), 0.0),
                ]
            },
            &[],
        ),
        // The Nováks, who hid in the deep mine: small boots, a drawing.
        ("kettle", "house_plank") if here(-42.0, 125.0) => house(
            |s| {
                let mut v = Resident::Family.things(s, 8.0, 7.0);
                v.push(on(
                    "child_drawing",
                    s.partition + Vec3::new(0.5, 0.25, 0.0),
                    0.0,
                ));
                v
            },
            &[],
        ),
        // Old Fiala's lodge.
        ("kettle", "cabin_log") if here(205.0, -205.0) => things(vec![
            facing("traps_hook", Vec3::new(-0.4, cf + 1.6, -cz), pz),
            facing("antlers", Vec3::new(1.0, cf + 1.85, cz), nz),
            facing("hide_frame", Vec3::new(cx, cf + 1.3, -1.2), nx),
        ]),
        // Mirek Hájek, the charcoal burner: "my name's on the frame so you
        // know whose it was".
        ("kettle", "cabin_log") if here(119.0, -138.0) => things(vec![
            on(
                "bed_name_mirek",
                Vec3::new(-2.8, cf + 0.32, 1.5 - 0.975 - 0.03),
                0.0,
            ),
            on("charcoal_sack", Vec3::new(-0.45, cf, -2.4), 0.5),
        ]),
        // The sexton's chapel: the hymns still up from the last Sunday.
        ("kettle", "chapel") if here(-8.0, 150.0) => things(vec![facing(
            "hymn_board",
            Vec3::new(3.0, 0.5 + 1.9, 7.0 - 0.175),
            nz,
        )]),
        // The scavenger off Lev's truck: a bedroll, a tin, lately.
        ("cinder", "house_plank_damaged") if here(0.0, 150.0) => house(
            |_| {
                vec![
                    on("bedroll", Vec3::new(-4.0 + 0.95, 0.5, -3.5 + 1.1), PI * 0.5),
                    on("tin_empty", Vec3::new(-4.0 + 1.55, 0.5, -3.5 + 0.45), 0.7),
                ]
            },
            &[],
        ),
        // T. Veselý, the woodward.
        ("cinder", "cabin_log") if here(-75.0, -159.0) => things(vec![
            facing("axe_pegs", Vec3::new(-0.4, cf + 1.5, -cz), pz),
            facing("bow_saw", Vec3::new(0.45, cf + 1.75, -cz), pz),
        ]),
        // Alois Pech, the keeper, alone and old, his legs bad.
        ("sallow", "house_brick") if here(-58.0, 206.0) => house(
            |s| {
                let edge = s.table[1] + Vec3::new(-0.1, 1.7 - 0.785, -0.36);
                vec![
                    on("cup_one", s.table[0], 0.0),
                    on("walking_stick", edge, 0.0),
                    (
                        "reading_glasses",
                        Transform::from_translation(s.sills[0].0 + Vec3::Z * 0.1),
                    ),
                    on("photo_church", s.sills[1].0 + Vec3::Z * 0.1, PI),
                ]
            },
            &["dishes"],
        ),
        // J. Rybář's shop: the key gone from its hook.
        ("sallow", "shop") if here(-180.0, -222.0) => things(vec![facing(
            "key_hook_empty",
            Vec3::new(0.9, 0.3 + 1.6, -3.5 + 0.125),
            pz,
        )]),
        // The angling club's cabin.
        ("sallow", "cabin_log") if here(-160.0, 112.0) => things(vec![
            facing("rods_rack", Vec3::new(-cx, cf + 1.6, -0.6), px),
            facing("landing_net", Vec3::new(cx, cf + 1.5, -1.0), nx),
            facing("stuffed_pike", Vec3::new(0.9, cf + 1.7, cz), nz),
        ]),
        // Mrs Hlavač, who wouldn't go twice, and went in the end on her own
        // feet: her chair at the window, her slippers, her peg empty.
        ("sallow", _) if here(-210.0, -222.0) => house(
            |s| {
                let w = s.sills[0].0;
                vec![
                    on("armchair", Vec3::new(w.x, 0.5, w.z + 0.6), 0.0),
                    on("slippers", Vec3::new(w.x + 0.15, 0.5, w.z + 1.15), 0.2),
                    wall("peg_empty", s.door_wall[0]),
                ]
            },
            &[],
        ),
        _ => None,
    }
}

/// The buildings that aren't houses which get their workers' things
/// (`workplace`).
pub const WORKPLACES: [&str; 11] = [
    "cabin_log",
    "shop",
    "shed",
    "barn",
    "pump_station",
    "mast_hut",
    "guard_booth",
    "signal_box",
    "bus_depot",
    "rail_shed",
    "factory_hall",
];

/// What Cinder's fire left of a workplace's things: iron and tin.
const FIREPROOF: [&str; 10] = [
    "shop_scales",
    "hurricane_lamp",
    "wall_clock",
    "gauge_board",
    "oil_cans",
    "toolbox",
    "pitchfork",
    "milk_churn",
    "tyre_stack",
    "watering_can",
];

/// What the people who worked in a building that isn't a house left
/// (`art/props/workplaces.py`), building-local: three to six of its set
/// (`kit`), from its seed, so no two barns are alike. Kettle's lamps are
/// carbide, and its miners left their dinner pails; in Cinder only the
/// iron and tin came through the fire, two or three things of it. Nothing
/// is thrown about: the workers left in order.
fn workplace(map: &MapDef, prefab: &str, at: Vec2) -> Vec<(&'static str, Transform)> {
    let n = map.name.to_lowercase();
    if !["marrow", "kettle", "cinder", "sallow", "rime"]
        .iter()
        .any(|v| n.contains(v))
    {
        return Vec::new();
    }
    let cinder = n.contains("cinder");
    // Rime Fell's were miners too: their carbide lamps.
    let mut all = kit(prefab, n.contains("kettle") || n.contains("rime"));
    if cinder {
        all.retain(|(m, _)| FIREPROOF.contains(m));
    }
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(at, prefab) ^ 0x3c0b);
    rng.shuffle(&mut all);
    let keep = if cinder {
        2 + rng.usize(..2)
    } else {
        3 + rng.usize(..4)
    };
    all.truncate(keep);
    all
}

/// A floor thing with its back (its +Z, `back` deep from its origin) to
/// the wall whose face is at `at`, facing into the room along `into`.
fn against(model: &'static str, at: Vec3, into: Vec3, back: f32) -> (&'static str, Transform) {
    facing(model, at + into * (back + 0.02), into)
}

/// Every thing a workplace could hold, building-local, by its recipe
/// (`buildings`): against the walls and in the corners, a metre and more
/// from every doorway, on the furniture's tops clear of the loot. Each
/// stands clear of the others, as any few of them are chosen.
fn kit(prefab: &str, kettle: bool) -> Vec<(&'static str, Transform)> {
    let (px, nx, pz, nz) = (Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z);
    let lamp = if kettle {
        "carbide_lamp"
    } else {
        "hurricane_lamp"
    };
    // How far the models reach behind their origins (their +Z).
    let (cans, tools) = (0.16, 0.112);
    let mut v = match prefab {
        // `buildings::cabin`: 7 x 6, the floor 0.4 up (its boards' top
        // 0.43), the walls' insides 3.36 and 2.86 out; the door at x -1.5
        // in the front, the stove in the back +X corner.
        "cabin_log" => {
            let (f, x, z) = (0.43, 3.36, 2.86);
            let table = |along: f32| {
                Vec3::new(0.0, 0.4 + 0.785, 0.5) + Quat::from_rotation_y(0.3) * Vec3::X * along
            };
            let mut v = vec![
                facing("woodpile", Vec3::new(1.6, f, z - 0.23), nz),
                on(lamp, table(0.42), 0.3),
                against("washstand", Vec3::new(-x, f, -0.9), px, 0.2),
                facing("snowshoes", Vec3::new(-x, 0.4 + 1.9, -2.15), px),
                on("rug", Vec3::new(-0.3, f, -1.6), 0.1),
                facing("broom", Vec3::new(0.25, f, -z), pz),
                on("jars", Vec3::new(-1.0, 0.4 + 0.315, 2.7), 0.0),
            ];
            if kettle {
                v.push(on("lunch_pail", table(-0.42), 0.3));
            }
            v
        }
        // `buildings::shop`: 9 x 7, the floor 0.3 up, the walls' insides
        // 4.375 and 3.375 out; the counter across the front room, the
        // shelves on the partition (z 1.6, its door at x 1.0), the back
        // room behind.
        "shop" => {
            let (f, x) = (0.33, 4.375);
            let mut v = vec![
                on("shop_scales", Vec3::new(0.0, 0.3 + 0.9, -0.4), 0.0),
                // Behind the counter, under the -X window.
                on("flour_sacks", Vec3::new(-3.5, f, 0.42), 0.0),
                facing("price_board", Vec3::new(x, 0.3 + 1.7, -1.2), nx),
                facing("broom", Vec3::new(x, f, -2.6), nx),
                on("jars", Vec3::new(3.8, 0.3 + 0.315, 1.2), 0.0),
                on("milk_churn", Vec3::new(1.9, f, 3.1), 0.0),
            ];
            if kettle {
                v.push(on("lunch_pail", Vec3::new(-2.1, 0.3 + 0.9, -0.4), 0.4));
            }
            v
        }
        // `buildings::shed`: 4 x 3, the floor 0.15 up, the walls' insides
        // 1.95 and 1.45 out; the door at x -1.0.
        "shed" => {
            let f = 0.18;
            vec![
                facing("garden_tools", Vec3::new(-0.35, 0.15 + 1.6, 1.45), nz),
                against("watering_can", Vec3::new(-1.95, f, -0.2), px, 0.084),
                against("toolbox", Vec3::new(1.95, f, 0.85), nx, tools),
                on("wheelbarrow", Vec3::new(1.55, f, -0.3), 0.0),
                facing("broom", Vec3::new(0.6, f, -1.45), pz),
            ]
        }
        // `buildings::barn`: 12 x 16, the walls' insides 5.925 and 7.925
        // out; doorways front and back, a door at z -4 in the +X wall,
        // the hay in the back -X corner, the workbench along +X.
        "barn" => {
            let (f, x) = (0.065, 5.925);
            vec![
                on("milk_churn", Vec3::new(-5.65, f, 0.6), 0.0),
                facing("pitchfork", Vec3::new(-x, f, -1.0), px),
                facing("harness_peg", Vec3::new(-x, 0.05 + 1.9, -3.0), px),
                against("flour_sacks", Vec3::new(-x, f, 6.6), px, 0.19),
                on(lamp, Vec3::new(5.0, 0.05 + 0.94, -0.2), 0.0),
                on("wheelbarrow", Vec3::new(-3.0, f, -6.6), 0.0),
                against("toolbox", Vec3::new(x, f, 2.2), nx, tools),
                against("oil_cans", Vec3::new(x, f, -1.2), nx, cans),
            ]
        }
        // `buildings::pump_station`: 14 x 10, the floor 0.3 up, the walls'
        // insides 6.85 and 4.85 out; the office behind the wall at x 3.5
        // (its door at z -1.5), the pumps down the middle.
        "pump_station" => {
            let (f, x, z) = (0.33, 6.85, 4.85);
            let mut v = vec![
                facing("wall_clock", Vec3::new(3.44, 0.3 + 2.4, -3.2), nx),
                facing("gauge_board", Vec3::new(0.0, 0.3 + 1.5, z), nz),
                facing("wall_telephone", Vec3::new(x, 0.3 + 1.5, -2.4), nx),
                on("ledger_open", Vec3::new(4.95, 0.3 + 0.785, -3.0), 0.2),
                against("oil_cans", Vec3::new(-3.9, f, -z), pz, cans),
                against("toolbox", Vec3::new(-x, f, 2.2), px, tools),
                on(lamp, Vec3::new(-6.2, 0.3 + 0.94, -3.5), 0.0),
                facing("jacket_hook", Vec3::new(3.56, 0.3 + 1.7, -4.0), px),
            ];
            if kettle {
                v.push(on("lunch_pail", Vec3::new(-4.8, 0.3 + 0.94, -3.5), 0.0));
            }
            v
        }
        // `buildings::mast_hut`: 5 x 4, the floor 0.2 up, the walls'
        // insides 2.375 and 1.875 out; the door at x -1.3.
        "mast_hut" => {
            let (f, x, z) = (0.23, 2.375, 1.875);
            vec![
                facing("wall_clock", Vec3::new(x, 0.2 + 2.0, -0.2), nx),
                facing("wall_telephone", Vec3::new(x, 0.2 + 1.45, -1.0), nx),
                on("ledger_open", Vec3::new(0.45, 0.2 + 0.9, 1.4), 0.1),
                against("toolbox", Vec3::new(1.2, f, -z), pz, tools),
                against("oil_cans", Vec3::new(-x, f, 0.3), px, cans),
                on(lamp, Vec3::new(2.0, 0.2 + 0.9, 1.4), 0.0),
            ]
        }
        // `buildings::guard_booth`: 2.2 square, the floor 0.6 up, the
        // walls' insides 1.06 out; the counter across the back.
        "guard_booth" => {
            let x = 1.06;
            vec![
                on("ledger_open", Vec3::new(-0.5, 0.6 + 0.9, 0.6), 0.0),
                facing("wall_telephone", Vec3::new(x, 0.6 + 1.5, 0.78), nx),
                facing("wall_clock", Vec3::new(-x, 0.6 + 1.85, 0.78), px),
                on(lamp, Vec3::new(0.55, 0.6 + 0.9, 0.6), 0.0),
            ]
        }
        // `buildings::signal_box`: 6 x 3.6, the floor 0.3 up, the walls'
        // insides 2.85 and 1.65 out; the door at x -1.5, the lever frame
        // along the back, the desk in the back -X corner.
        "signal_box" => {
            let (f, x, z) = (0.33, 2.85, 1.65);
            vec![
                facing("wall_clock", Vec3::new(x, 0.3 + 2.1, 0.3), nx),
                facing("wall_telephone", Vec3::new(x, 0.3 + 1.45, -0.6), nx),
                on(
                    "ledger_open",
                    Vec3::new(-2.3, 0.3 + 0.785, 1.0),
                    std::f32::consts::FRAC_PI_2,
                ),
                on(lamp, Vec3::new(-1.45, 0.3 + 0.785, 1.1), 0.0),
                against("oil_cans", Vec3::new(2.35, f, z), nz, cans),
                facing("jacket_hook", Vec3::new(-x, 0.3 + 1.7, 1.2), px),
            ]
        }
        "bus_depot" => hall(30.0, 18.0, kettle),
        "rail_shed" => hall(20.0, 12.0, kettle),
        "factory_hall" => hall(36.0, 20.0, kettle),
        _ => Vec::new(),
    };
    // The depot's office: its telephone, the day's ledger on its table.
    if prefab == "bus_depot" {
        v.push(facing(
            "wall_telephone",
            Vec3::new(13.7, 0.1 + 1.5, 4.06),
            pz,
        ));
        v.push(on(
            "ledger_open",
            Vec3::new(11.98, 0.1 + 0.785, 6.5),
            std::f32::consts::FRAC_PI_2,
        ));
    }
    v
}

/// A `buildings::hall`'s things, `w` x `d`: the floor 0.1 up, the walls'
/// insides 0.125 in; round the -X bay's workbench (at -0.35 w, 0.35 d)
/// and the back wall behind it, and a gauge board on the +X wall.
fn hall(w: f32, d: f32, kettle: bool) -> Vec<(&'static str, Transform)> {
    let (f, x, z) = (0.1, w * 0.5 - 0.125, d * 0.5 - 0.125);
    let (bx, bz) = (-w * 0.35, d * 0.35);
    let mut v = vec![
        on("tyre_stack", Vec3::new(-x + 1.05, f, z - 0.75), 0.4),
        against("oil_cans", Vec3::new(bx - 0.6, f, z), Vec3::NEG_Z, 0.16),
        on("toolbox", Vec3::new(bx - 0.6, f + 0.94, bz), 0.0),
        facing("jacket_hook", Vec3::new(bx + 1.6, f + 1.7, z), Vec3::NEG_Z),
        facing("wall_clock", Vec3::new(bx, f + 3.2, z), Vec3::NEG_Z),
        facing("gauge_board", Vec3::new(x, f + 1.5, -2.4), Vec3::NEG_X),
    ];
    if kettle {
        v.push(on("lunch_pail", Vec3::new(bx + 0.6, f + 0.94, bz), 0.0));
    }
    v
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

/// The resident of the house `prefab` placed at `xf`, if it has one;
/// `no_children` where a valley's children's things are gone (Cinder's
/// machines hauled them to the furnace: their absence is the point).
pub fn resident(xf: &Transform, prefab: &str, no_children: bool) -> Option<Resident> {
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(at, prefab) ^ 0x2e51);
    if rng.f32() < 0.25 {
        return None;
    }
    Some(RESIDENTS[rng.usize(..RESIDENTS.len())])
        .filter(|r| !(no_children && *r == Resident::Family))
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
        (Valley::Marrow, "house_plank", _) if here(-20.2, 63.5) => Some(Story::Hana),
        (Valley::Kettle, "house_plank", _) if here(-30.0, 72.0) => Some(Story::ShotFirer),
        _ => None,
    }
}

// --- Yards ------------------------------------------------------------------------------------

/// A yard thing (`art/props/yards.py`): its model, half its footprint
/// along its own x and z, and whether it stands against a wall, its back
/// (+Z) to it, or out in the yard.
#[derive(Clone, Copy, Debug)]
struct YardThing {
    model: &'static str,
    hx: f32,
    hz: f32,
    wall: bool,
}

const fn thing(model: &'static str, hx: f32, hz: f32, wall: bool) -> YardThing {
    YardThing {
        model,
        hx,
        hz,
        wall,
    }
}

const CLOTHES_LINE: YardThing = thing("clothes_line", 2.1, 0.35, false);
const CLOTHES_LINE_BARE: YardThing = thing("clothes_line_bare", 2.1, 0.35, false);
const BED_CABBAGE: YardThing = thing("garden_bed_cabbage", 1.22, 0.52, false);
const BED_ROOTS: YardThing = thing("garden_bed_roots", 1.22, 0.52, false);
const BENCH: YardThing = thing("bench_yard", 0.78, 0.2, true);
const BENCH_CHARRED: YardThing = thing("bench_charred", 0.78, 0.2, true);
const RAIN_BARREL: YardThing = thing("rain_barrel", 0.33, 0.37, true);
const WOODPILE: YardThing = thing("woodpile_lean", 1.13, 0.47, true);
const WASHTUB: YardThing = thing("washtub_stand", 0.4, 0.36, true);
const CHOPPING_BLOCK: YardThing = thing("chopping_block", 0.6, 0.36, false);
const BEEHIVE: YardThing = thing("beehive", 0.27, 0.33, false);
const HEN_COOP: YardThing = thing("hen_coop", 0.8, 0.74, false);
const HANDCART: YardThing = thing("handcart", 0.53, 1.04, false);
const HAND_PUMP: YardThing = thing("hand_pump", 0.42, 0.72, false);
const DOG_KENNEL: YardThing = thing("dog_kennel", 0.53, 0.57, false);
const TUB_IRON: YardThing = thing("tub_iron", 0.75, 0.36, true);
const COAL_BUNKER: YardThing = thing("coal_bunker", 0.64, 0.72, true);
const SNOW_SHOVEL: YardThing = thing("snow_shovel_out", 0.24, 0.21, true);
const SLEDGE: YardThing = thing("sledge", 0.24, 0.63, false);

/// Every yard model, for the tests.
#[cfg(test)]
const YARD_THINGS: [YardThing; 19] = [
    CLOTHES_LINE,
    CLOTHES_LINE_BARE,
    BED_CABBAGE,
    BED_ROOTS,
    BENCH,
    BENCH_CHARRED,
    RAIN_BARREL,
    WOODPILE,
    WASHTUB,
    CHOPPING_BLOCK,
    BEEHIVE,
    HEN_COOP,
    HANDCART,
    HAND_PUMP,
    DOG_KENNEL,
    TUB_IRON,
    COAL_BUNKER,
    SNOW_SHOVEL,
    SLEDGE,
];

/// Marrow's gardens: kept, the beds in rows, hives and hens, a cart left
/// half loaded.
const MARROW_YARD: [YardThing; 10] = [
    BED_CABBAGE,
    BED_ROOTS,
    CLOTHES_LINE,
    BENCH,
    RAIN_BARREL,
    WOODPILE,
    BEEHIVE,
    HEN_COOP,
    HANDCART,
    CHOPPING_BLOCK,
];
/// Kettle's miners' rows: coal, wood, the wash, a pump, a potato bed.
const KETTLE_YARD: [YardThing; 10] = [
    COAL_BUNKER,
    WOODPILE,
    CLOTHES_LINE,
    WASHTUB,
    CHOPPING_BLOCK,
    BENCH,
    RAIN_BARREL,
    BED_ROOTS,
    HAND_PUMP,
    HANDCART,
];
/// Coldharbour's, on its bench above the trees: coal and wood for the
/// winter, the shovel, a sledge; nothing grows and the water freezes.
const RIME_YARD: [YardThing; 6] = [
    COAL_BUNKER,
    WOODPILE,
    SNOW_SHOVEL,
    SLEDGE,
    CHOPPING_BLOCK,
    BENCH,
];
/// Cinder's: what the fire left, nothing alive.
const CINDER_YARD: [YardThing; 3] = [BENCH_CHARRED, TUB_IRON, CLOTHES_LINE_BARE];
/// New Sallow's and the keepers' cottages.
const SALLOW_YARD: [YardThing; 8] = [
    BED_CABBAGE,
    BED_ROOTS,
    CLOTHES_LINE,
    BENCH,
    RAIN_BARREL,
    WOODPILE,
    HAND_PUMP,
    WASHTUB,
];

/// Gap kept between yard things, and between a wall thing and its wall.
const YARD_GAP: f32 = 0.35;
/// How far out from the walls a yard reaches.
pub const YARD_REACH: f32 = 4.0;

/// A house's four sides, building-local: the front (-Z, the door's), the
/// back, and the two ends.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Side {
    Front,
    Back,
    Left,
    Right,
}

/// The things in a house's yard, building-local: `prefab` placed at `xf`
/// (its tag `tag`) on `map`, two to five from its valley's kit (fewer for
/// a ruin), along its back and ends, a bench maybe either side of the
/// door. `ground` is the ground's height at a map point (xz), `None`
/// under water. Each keeps within [`YARD_REACH`] of the walls and clear
/// of the door's way, the other placements, the roads and paths, the
/// water, the markers and Story's scenes. None for a drowned house, nor
/// for the buildings that were never homes.
pub fn yard(
    map: &MapDef,
    prefab: &str,
    tag: &str,
    xf: &Transform,
    ground: &dyn Fn(Vec2) -> Option<f32>,
) -> Vec<(&'static str, Transform)> {
    if std::env::var("EXTRACTION_NO_CLUTTER").is_ok_and(|v| v == "1") {
        return Vec::new();
    }
    let base = extraction_core::map::prefab_base(prefab);
    let Some((w, d)) = plan(base) else {
        return Vec::new();
    };
    if prefab.ends_with("_drowned") || matches!(tag, "post_office" | "mine_office") {
        return Vec::new();
    }
    let ruin = prefab != base;
    let sallow = map.name.to_lowercase().contains("sallow");
    let at = Vec2::new(xf.translation.x, xf.translation.z);
    let (kit, count): (&[YardThing], std::ops::RangeInclusive<usize>) = match valley(map) {
        // Old Sallow's ruins lay sixty years under the reservoir.
        None if sallow && !ruin => (&SALLOW_YARD, 2..=4),
        None => return Vec::new(),
        Some(Valley::Cinder) => (&CINDER_YARD, if ruin { 1..=2 } else { 1..=3 }),
        Some(_) if ruin => (&CINDER_YARD[1..2], 1..=1),
        Some(Valley::Marrow) => (&MARROW_YARD, 3..=5),
        Some(Valley::Kettle) => (&KETTLE_YARD, 3..=5),
        Some(Valley::Rime) => (&RIME_YARD, 2..=4),
    };
    let mut rng = fastrand::Rng::with_seed(super::overgrowth::seed(at, prefab) ^ 0x7a2d);
    let mut want: Vec<YardThing> = kit.to_vec();
    rng.shuffle(&mut want);
    want.truncate(rng.usize(count));
    // Hana's dog's kennel, first.
    if valley(map) == Some(Valley::Marrow)
        && special(Valley::Marrow, base, tag, at) == Some(Story::Hana)
    {
        want.insert(0, DOG_KENNEL);
    }
    // A damaged house's +X wall is breached, its rubble spilled outside.
    let sides: &[Side] = if prefab.ends_with("_damaged") {
        &[Side::Back, Side::Left]
    } else {
        &[Side::Back, Side::Left, Side::Right]
    };
    let site = YardSite::new(map, prefab, xf, w, d);
    let mut placed: Vec<(Vec2, Vec2)> = Vec::new();
    let mut out = Vec::new();
    for t in want {
        // A bench may stand either side of the door, under a window.
        let bench = t.wall && t.hz < 0.25;
        for _ in 0..16 {
            let side = if bench && rng.f32() < 0.4 {
                Side::Front
            } else {
                sides[rng.usize(..sides.len())]
            };
            let Some((c, half, yaw)) = site.spot(t, side, &mut rng) else {
                continue;
            };
            if placed.iter().any(|(o, oh)| {
                (c - *o)
                    .abs()
                    .cmplt(half + *oh + Vec2::splat(YARD_GAP))
                    .all()
            }) {
                continue;
            }
            if !site.clear(c, half) {
                continue;
            }
            let Some((y, tilt)) = site.level(c, half, ground) else {
                continue;
            };
            placed.push((c, half));
            out.push((
                t.model,
                Transform::from_xyz(c.x, y, c.y).with_rotation(tilt * Quat::from_rotation_y(yaw)),
            ));
            break;
        }
    }
    out
}

/// A house's ground: its walls, its door's way, what's round it on the map.
struct YardSite<'a> {
    map: &'a MapDef,
    xf: &'a Transform,
    /// The walls' outer faces (the plinth's), half across and half deep.
    half: Vec2,
    /// The door's way out: half its width with a metre either side, and
    /// how far out it's kept clear.
    door: (f32, f32),
    /// Trees keep this far from the house's middle (`map::prefab_clearance`).
    trees: f32,
    /// Windows along each side (their middles, along the side's own axis).
    windows: [Vec<f32>; 4],
    scenes: Vec<Vec2>,
}

impl<'a> YardSite<'a> {
    fn new(map: &'a MapDef, prefab: &str, xf: &'a Transform, w: f32, d: f32) -> Self {
        let door = extraction_core::map::prefab_entrances(prefab)
            .first()
            .map_or((1.5, 3.6), |e| (e.width * 0.5 + 1.0, e.clear));
        let id = map.name.to_lowercase().replace(' ', "_");
        YardSite {
            map,
            xf,
            half: Vec2::new(w * 0.5 + 0.1, d * 0.5 + 0.1),
            door,
            trees: extraction_core::map::prefab_clearance(prefab),
            // As `buildings::house` cuts them (see `slots`).
            windows: [
                vec![-w * 0.3, w * 0.3],
                vec![-w * 0.25, w * 0.25],
                vec![0.0],
                vec![-d * 0.2, d * 0.25],
            ],
            scenes: super::scenes::of(&id)
                .map(|(_, s)| Vec2::from(s.at))
                .collect(),
        }
    }

    /// Where `t` might go on `side`: its middle (xz), its half extents
    /// (building axes) and its yaw, its front turned away from the wall.
    fn spot(&self, t: YardThing, side: Side, rng: &mut fastrand::Rng) -> Option<(Vec2, Vec2, f32)> {
        use std::f32::consts::{FRAC_PI_2, PI};
        let (along, wall_at) = match side {
            Side::Front | Side::Back => (self.half.x, self.half.y),
            Side::Left | Side::Right => (self.half.y, self.half.x),
        };
        let out = if t.wall {
            t.hz + 0.06
        } else {
            1.3 + rng.f32() * 1.2 + t.hz
        };
        if out + t.hz > YARD_REACH {
            return None;
        }
        // Along the side: wall things within it (a barrel in its corner),
        // the others a little past its ends at most.
        let reach = if t.wall {
            along - t.hx - 0.1
        } else {
            along + 0.5 - t.hx
        };
        if reach < 0.0 {
            return None;
        }
        let s = if t.model == RAIN_BARREL.model {
            reach * if rng.bool() { 1.0 } else { -1.0 }
        } else {
            (rng.f32() * 2.0 - 1.0) * reach
        };
        let k = match side {
            Side::Front => 0,
            Side::Back => 1,
            Side::Left => 2,
            Side::Right => 3,
        };
        // Nothing tall over a window's sill.
        if t.model == WOODPILE.model && self.windows[k].iter().any(|&m| (s - m).abs() < t.hx + 0.6)
        {
            return None;
        }
        if side == Side::Front && s.abs() - t.hx < self.door.0 {
            return None;
        }
        let d = wall_at + out;
        let (c, yaw) = match side {
            Side::Front => (Vec2::new(s, -d), 0.0),
            Side::Back => (Vec2::new(s, d), PI),
            Side::Left => (Vec2::new(-d, s), FRAC_PI_2),
            Side::Right => (Vec2::new(d, s), -FRAC_PI_2),
        };
        let half = match side {
            Side::Front | Side::Back => Vec2::new(t.hx, t.hz),
            Side::Left | Side::Right => Vec2::new(t.hz, t.hx),
        };
        Some((c, half, yaw))
    }

    fn to_map(&self, p: Vec2) -> Vec2 {
        let q = self.xf.transform_point(Vec3::new(p.x, 0.0, p.y));
        Vec2::new(q.x, q.z)
    }

    /// Whether a thing at `c` (building-local), `half` its half extents,
    /// keeps clear of the house, the door's way, the trees' line and
    /// everything else on the map.
    fn clear(&self, c: Vec2, half: Vec2) -> bool {
        let lo = c - half;
        let hi = c + half;
        // Off the walls, and the door's way.
        if lo.x < self.half.x && hi.x > -self.half.x && lo.y < self.half.y && hi.y > -self.half.y {
            return false;
        }
        let (dw, dc) = self.door;
        if lo.x < dw && hi.x > -dw && lo.y < -self.half.y + 0.1 && hi.y > -self.half.y - dc {
            return false;
        }
        // Inside the ring the trees keep clear.
        let corners = [lo, hi, Vec2::new(lo.x, hi.y), Vec2::new(hi.x, lo.y)];
        if corners.iter().any(|p| p.length() > self.trees - 0.6) {
            return false;
        }
        let at = self.to_map(c);
        let r = half.length();
        let me = Vec2::new(self.xf.translation.x, self.xf.translation.z);
        // Nobody else's footprint.
        for o in &self.map.placements {
            let oat = Vec2::from(o.at);
            if oat.distance(me) < 0.5 {
                continue;
            }
            let [ow, _, od] = extraction_core::map::prefab_footprint(&o.prefab);
            let local = Quat::from_rotation_y(o.rot.to_radians())
                * Vec3::new(at.x - oat.x, 0.0, at.y - oat.y);
            let out = (Vec2::new(local.x, local.z).abs() - Vec2::new(ow, od) * 0.5).max(Vec2::ZERO);
            if out.length() < r + 0.5 {
                return false;
            }
        }
        // Off the roads and paths, out of the water.
        let p = [at.x, at.y];
        for road in &self.map.roads {
            let (dist, ..) = extraction_core::geom::polyline_closest(&road.points, p);
            if dist < (road.width * 0.5 + 1.0).max(2.0) + r {
                return false;
            }
        }
        for water in &self.map.water {
            let (dist, ..) = extraction_core::geom::polyline_closest(&water.points, p);
            if dist < water.width * 0.5 + 1.0 + r {
                return false;
            }
        }
        // Clear of the markers (a note, a sign, loot on the ground, an
        // anomaly's reach) and Story's scenes.
        for m in &self.map.markers {
            let keep = match &m.kind {
                MarkerKind::Anomaly { radius, .. } => radius + 1.5,
                _ => 1.2,
            };
            if Vec2::from(m.at).distance(at) < keep + r {
                return false;
            }
        }
        !self.scenes.iter().any(|s| s.distance(at) < 3.0 + r)
    }

    /// The thing's height (building-local) and tilt to stand on the ground
    /// under it, tilted to it (set at its lowest corner and level, a bed
    /// or a pump on a slope stood into the hill on its high side:
    /// `capture::footing`), or `None` if the spot is under water or too
    /// steep.
    fn level(
        &self,
        c: Vec2,
        half: Vec2,
        ground: &dyn Fn(Vec2) -> Option<f32>,
    ) -> Option<(f32, Quat)> {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        let mut h = [0.0f32; 5];
        for (i, k) in [
            Vec2::ZERO,
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            h[i] = ground(self.to_map(c + half * k))? - self.xf.translation.y;
            lo = lo.min(h[i]);
            hi = hi.max(h[i]);
        }
        if hi - lo >= 0.35 {
            return None;
        }
        // The ground's plane under it (building-local slopes along x, z).
        let dx = ((h[2] + h[3]) - (h[1] + h[4])) / (4.0 * half.x.max(0.01));
        let dz = ((h[3] + h[4]) - (h[1] + h[2])) / (4.0 * half.y.max(0.01));
        let mean = h.iter().sum::<f32>() / 5.0;
        let tilt = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-dx, 1.0, -dz).normalize());
        // Settled a little, and no corner standing off the ground.
        let worst = [
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ]
        .iter()
        .zip(&h[1..])
        .map(|(k, g)| mean + dx * half.x * k.x + dz * half.y * k.y - g)
        .fold(0.0f32, f32::max);
        Some((mean - worst - 0.01, tilt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every clutter model is built.
    #[test]
    fn every_clutter_model_exists() {
        let root = crate::asset_root_path();
        for name in extraction_core::map::VALLEYS.map(|v| v.0) {
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
            .filter(|p| {
                resident(
                    &Transform::from_xyz(p.at[0], 0.0, p.at[1]),
                    &p.prefab,
                    false,
                )
                .is_some()
            })
            .count();
        assert!(lived * 2 > houses.len(), "{lived} of {}", houses.len());
    }

    /// Everyone Story's notes name has their things where they lived, the
    /// things built and inside the building.
    #[test]
    fn storys_named_people_have_their_things() {
        let root = crate::asset_root_path();
        let mut map = crate::world::load_map_def("marrow_creek");
        let named_at = [
            ("marrow", "shop", -18.2, 3.5),
            ("marrow", "cabin_log", -110.8, 113.5),
            ("marrow", "cabin_log", -70.8, 113.5),
            ("marrow", "pump_station", -110.0, 12.1),
            ("kettle", "house_plank", 76.0, 222.0),
            ("kettle", "house_plank", -99.0, 154.0),
            ("kettle", "house_plank", -42.0, 125.0),
            ("kettle", "cabin_log", 205.0, -205.0),
            ("kettle", "cabin_log", 119.0, -138.0),
            ("kettle", "chapel", -8.0, 150.0),
            ("cinder", "house_plank_damaged", 0.0, 150.0),
            ("cinder", "cabin_log", -75.0, -159.0),
            ("sallow", "house_brick", -58.0, 206.0),
            ("sallow", "shop", -180.0, -222.0),
            ("sallow", "cabin_log", -160.0, 112.0),
            ("sallow", "house_plank", -210.0, -222.0),
        ];
        for (valley, prefab, x, z) in named_at {
            map.name = format!("{valley} test");
            map.markers.clear();
            let xf = Transform::from_xyz(x, 0.0, z);
            let things = dress(&map, prefab, "", &xf, false);
            let [w, _, d] = extraction_core::map::prefab_footprint(prefab);
            assert!(
                !things.is_empty(),
                "{valley} {prefab} at ({x}, {z}): nothing"
            );
            for (model, t) in &things {
                let path = root.join(crate::render::world_props::path(model));
                assert!(path.exists(), "{prefab}: no model {}", path.display());
                let p = t.translation;
                // (A sill's things stand on the wall's middle.)
                assert!(
                    p.x.abs() <= w * 0.5 + 0.01 && p.z.abs() <= d * 0.5 + 0.01,
                    "{valley} {prefab} {model} at {p}, outside"
                );
            }
        }
        // Mirek's name is on his bed; Tomas's tally is by his.
        map.name = "kettle".into();
        let mirek = dress(
            &map,
            "cabin_log",
            "",
            &Transform::from_xyz(119.0, 0.0, -138.0),
            false,
        );
        assert!(mirek.iter().any(|(m, _)| *m == "bed_name_mirek"));
        map.name = "marrow".into();
        let tomas = dress(
            &map,
            "cabin_log",
            "",
            &Transform::from_xyz(-70.8, 0.0, 113.5),
            false,
        );
        assert!(tomas.iter().any(|(m, _)| *m == "tally_board"));
        // No child's things left in Cinder.
        let cinder = crate::world::load_map_def("cinder_works");
        for p in &cinder.placements {
            let xf = Transform::from_xyz(p.at[0], 0.0, p.at[1]);
            for (m, _) in dress(&cinder, &p.prefab, &p.tag, &xf, true) {
                assert!(
                    !["cot", "rocking_horse", "small_boots", "child_drawing"].contains(&m),
                    "{} at {:?}: {m}",
                    p.prefab,
                    p.at
                );
            }
        }
    }

    /// Old Sallow's drowned houses hold two or three old things, no more.
    #[test]
    fn the_silt_holds_old_things_sparsely() {
        let map = crate::world::load_map_def("marrow_creek");
        for (k, id) in [
            "house_plank_drowned",
            "farmhouse_drowned",
            "house_brick_drowned",
        ]
        .into_iter()
        .enumerate()
        {
            let xf = Transform::from_xyz(k as f32 * 31.0, 0.0, 7.0);
            let things = dress(&map, id, "", &xf, false);
            assert!((2..=3).contains(&things.len()), "{id}: {things:?}");
            for (m, _) in &things {
                assert!(
                    ["bedstead_silted", "cart_wheel", "mangle", "dishes_broken"].contains(m),
                    "{id}: {m}"
                );
            }
        }
    }

    /// Nothing for a room comes through its walls: every thing a house
    /// is dressed with stays inside its outer walls' inner faces, give or
    /// take 3 cm (inside the wall, unseen); a sill's things may stand on
    /// the wall's middle, in the window. (A coat's sleeves once ran out
    /// through the front wall beside the door.)
    #[test]
    fn nothing_comes_through_the_walls() {
        let root = crate::asset_root_path();
        let mut cache = std::collections::HashMap::new();
        let mut bad = Vec::new();
        for name in ["marrow_creek", "kettle_pass", "cinder_works"] {
            let map = crate::world::load_map_def(name);
            for p in &map.placements {
                let base = extraction_core::map::prefab_base(&p.prefab);
                let Some((w, d)) = plan(base) else { continue };
                let inner = Vec2::new(w * 0.5 - 0.09, d * 0.5 - 0.09);
                let xf = Transform::from_xyz(p.at[0], 0.0, p.at[1]);
                for (model, t) in dress(&map, &p.prefab, &p.tag, &xf, true) {
                    let b = cache.entry(model).or_insert_with(|| {
                        let path = root.join(crate::render::world_props::path(model));
                        crate::capture::glb::read(&path)
                            .ok()
                            .and_then(|g| g.bounds_of(|_| true))
                    });
                    let Some((lo, hi)) = *b else { continue };
                    let (mut a, mut z) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                    for k in 0..8 {
                        let c = Vec3::new(
                            if k & 1 == 0 { lo.x } else { hi.x },
                            if k & 2 == 0 { lo.y } else { hi.y },
                            if k & 4 == 0 { lo.z } else { hi.z },
                        );
                        let q = t.transform_point(c);
                        a = a.min(q);
                        z = z.max(q);
                    }
                    let sill = (t.translation.y - 1.425).abs() < 0.05;
                    let slack = if sill { 0.2 } else { 0.03 };
                    if a.x < -inner.x - slack
                        || z.x > inner.x + slack
                        || a.z < -inner.y - slack
                        || z.z > inner.y + slack
                    {
                        bad.push(format!(
                            "{name} {} at {:?}: {model} from {a} to {z}",
                            p.prefab, p.at
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// A model's box at `t` (building-local), from its glb's bounds.
    fn placed_bounds(
        cache: &mut std::collections::HashMap<&'static str, Option<(Vec3, Vec3)>>,
        model: &'static str,
        t: &Transform,
    ) -> Option<(Vec3, Vec3)> {
        let root = crate::asset_root_path();
        let (lo, hi) = (*cache.entry(model).or_insert_with(|| {
            let path = root.join(crate::render::world_props::path(model));
            crate::capture::glb::read(&path)
                .ok()
                .and_then(|g| g.bounds_of(|_| true))
        }))?;
        Some(corners(lo, hi, |c| t.transform_point(c)))
    }

    /// The box round the eight corners of `lo`..`hi` taken through `f`.
    fn corners(lo: Vec3, hi: Vec3, f: impl Fn(Vec3) -> Vec3) -> (Vec3, Vec3) {
        let (mut a, mut z) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for k in 0..8 {
            let c = Vec3::new(
                if k & 1 == 0 { lo.x } else { hi.x },
                if k & 2 == 0 { lo.y } else { hi.y },
                if k & 4 == 0 { lo.z } else { hi.z },
            );
            let q = f(c);
            a = a.min(q);
            z = z.max(q);
        }
        (a, z)
    }

    fn overlap(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
        a.0.cmplt(b.1).all() && b.0.cmplt(a.1).all()
    }

    /// The walls' insides of each workplace, half width and half depth
    /// (from its recipe: its size less half its walls' thickness).
    fn inside(prefab: &str) -> Vec2 {
        let (w, d, t) = match prefab {
            "cabin_log" => (7.0, 6.0, 0.28),
            "shop" => (9.0, 7.0, 0.25),
            "shed" => (4.0, 3.0, 0.1),
            "barn" => (12.0, 16.0, 0.15),
            "pump_station" => (14.0, 10.0, 0.3),
            "mast_hut" => (5.0, 4.0, 0.25),
            "guard_booth" => (2.2, 2.2, 0.08),
            "signal_box" => (6.0, 3.6, 0.3),
            "bus_depot" => (30.0, 18.0, 0.25),
            "rail_shed" => (20.0, 12.0, 0.25),
            "factory_hall" => (36.0, 20.0, 0.25),
            _ => panic!("{prefab}: no recipe"),
        };
        Vec2::new(w - t, d - t) * 0.5
    }

    /// Every thing a workplace could hold is built and stands inside its
    /// walls (give or take 3 cm, inside the wall, unseen), off its loot,
    /// a metre and more from every door and doorway, clear of its
    /// furniture and of the others it could be chosen with.
    #[test]
    fn workplaces_hold_their_things_in_place() {
        use crate::world::buildings::Shape;
        let root = crate::asset_root_path();
        let mut cache = std::collections::HashMap::new();
        let mut bad = Vec::new();
        for prefab in WORKPLACES {
            let geo = crate::world::buildings::prefab(prefab).unwrap();
            let inner = inside(prefab);
            // The furniture, the walls and what else stands in the room:
            // every piece but the floors' thin slabs.
            let solid: Vec<_> = geo
                .pieces
                .iter()
                .filter_map(|p| {
                    let half = match p.shape {
                        Shape::Box(s) | Shape::Prism(s) => s * 0.5,
                        Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
                    };
                    let b = corners(-half, half, |c| p.at.transform_point(c));
                    (b.1.y - b.0.y > 0.06).then_some(b)
                })
                .collect();
            for kettle in [false, true] {
                let things = kit(prefab, kettle);
                assert!(things.len() >= 4, "{prefab}: {things:?}");
                let mut boxes: Vec<(&str, (Vec3, Vec3))> = Vec::new();
                for (model, t) in &things {
                    let path = root.join(crate::render::world_props::path(model));
                    assert!(path.exists(), "{prefab}: no model {}", path.display());
                    let p = t.translation;
                    let Some((a, z)) = placed_bounds(&mut cache, model, t) else {
                        bad.push(format!("{prefab} {model}: no bounds"));
                        continue;
                    };
                    let s = 0.03;
                    if a.x < -inner.x - s
                        || z.x > inner.x + s
                        || a.z < -inner.y - s
                        || z.z > inner.y + s
                    {
                        bad.push(format!("{prefab} {model} from {a} to {z}: through a wall"));
                    }
                    for l in &geo.loot {
                        if l.at.distance(p) <= 0.35 {
                            bad.push(format!("{prefab} {model} at {p}: on {} loot", l.table));
                        }
                    }
                    // Doors, inside and out: a metre from the doorway's
                    // width either side.
                    for dr in &geo.doors {
                        let along = Quat::from_rotation_y(dr.yaw) * Vec3::X;
                        let k = (p - dr.hinge).dot(along).clamp(0.0, dr.width);
                        let near = (dr.hinge + along * k).xz().distance(p.xz());
                        if near < 1.0 {
                            bad.push(format!("{prefab} {model} at {p}: {near:.2} from a door"));
                        }
                    }
                    // Doorways (a barn's, a hall's have no doors), on
                    // their inside.
                    for e in &geo.entrances {
                        let (at, out) = (Vec2::from(e.at), Vec2::from(e.out));
                        let r = p.xz() - at;
                        let inward = -r.dot(out);
                        let side = r.dot(out.perp());
                        let off = side - side.clamp(-e.width * 0.5, e.width * 0.5);
                        let near = Vec2::new(off, inward).length();
                        if inward > -0.5 && near < 1.0 {
                            bad.push(format!("{prefab} {model} at {p}: {near:.2} from a doorway"));
                        }
                    }
                    // Into the furniture or the walls (by more than 3 cm:
                    // a thing's back is against a wall, its foot on a top).
                    let shrunk = (a + Vec3::splat(s), z - Vec3::splat(s));
                    for b in &solid {
                        if overlap(shrunk, *b) {
                            bad.push(format!(
                                "{prefab} {model} from {a} to {z}: into a piece {} to {}",
                                b.0, b.1
                            ));
                        }
                    }
                    for (other, o) in &boxes {
                        if overlap(shrunk, *o) {
                            bad.push(format!("{prefab} {model} into {other}"));
                        }
                    }
                    boxes.push((model, (a, z)));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Every workplace in the valleys gets three to six of its things (two
    /// or three in Cinder, all iron and tin); one Story names keeps only
    /// Story's; and not every barn is alike.
    #[test]
    fn workplaces_are_dressed_by_valley() {
        for (name, _) in extraction_core::map::VALLEYS {
            let mut map = crate::world::load_map_def(name);
            map.markers.clear();
            let cinder = name.contains("cinder");
            for p in map
                .placements
                .iter()
                .filter(|p| WORKPLACES.contains(&p.prefab.as_str()))
            {
                if named(&map, &p.prefab, &p.prefab, Vec2::from(p.at)).is_some() {
                    continue;
                }
                let xf = Transform::from_xyz(p.at[0], 0.0, p.at[1]);
                let things = dress(&map, &p.prefab, &p.tag, &xf, false);
                let n = things.len();
                let want = if cinder { 2..=3 } else { 3..=6 };
                assert!(want.contains(&n), "{name} {} at {:?}: {n}", p.prefab, p.at);
                if cinder {
                    for (m, _) in &things {
                        assert!(FIREPROOF.contains(m), "{} in Cinder: {m}", p.prefab);
                    }
                }
            }
        }
        // Story's shop and the foreman's cabin: their things, none of the
        // set's.
        let mut map = crate::world::load_map_def("marrow_creek");
        map.markers.clear();
        for (prefab, x, z) in [("shop", -18.2, 3.5), ("cabin_log", -110.8, 113.5)] {
            let xf = Transform::from_xyz(x, 0.0, z);
            let things = dress(&map, prefab, "", &xf, false);
            let set = kit(prefab, false);
            assert!(!things.is_empty());
            for (m, _) in &things {
                assert!(!set.iter().any(|(s, _)| s == m), "{prefab}: {m}");
            }
        }
        let map = crate::world::load_map_def("kettle_pass");
        let sets: std::collections::HashSet<Vec<&str>> = (0..6)
            .map(|k| {
                let mut v: Vec<_> = workplace(&map, "barn", Vec2::new(k as f32 * 37.0, 11.0))
                    .into_iter()
                    .map(|(m, _)| m)
                    .collect();
                v.sort();
                v
            })
            .collect();
        assert!(sets.len() > 2, "{sets:?}");
    }

    /// Story's houses get their things, and Hana's table stays bare.
    #[test]
    fn storys_houses_have_their_things() {
        let map = crate::world::load_map_def("marrow_creek");
        let xf = Transform::from_xyz(-20.2, 0.0, 63.5);
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

    /// Every yard model is built.
    #[test]
    fn every_yard_model_exists() {
        let root = crate::asset_root_path();
        for t in YARD_THINGS {
            let path = root.join(crate::render::world_props::path(t.model));
            assert!(path.exists(), "no model {}", path.display());
        }
    }

    /// Every valley's houses, on their real ground: each yard thing stands
    /// outside the walls and within reach of them, out of the door's way
    /// and off every other placement; the drowned houses and the offices
    /// have none; Cinder's are nothing alive; Hana's house alone has the
    /// kennel; and most houses have a yard.
    #[test]
    fn yards_keep_to_their_ground() {
        use extraction_core::map::{VALLEYS, prefab_entrances, prefab_footprint};
        let things: std::collections::HashMap<_, _> =
            YARD_THINGS.iter().map(|t| (t.model, *t)).collect();
        for (name, _) in VALLEYS {
            let map = crate::world::load_map_def(name);
            let hm = extraction_core::terrain::Heightmap::build(&map);
            let ground = |p: Vec2| {
                let p = [p.x, p.y];
                hm.water_level(p).is_none().then(|| hm.height(p))
            };
            let (mut houses, mut yards) = (0, 0);
            for p in &map.placements {
                let base = extraction_core::map::prefab_base(&p.prefab);
                if plan(base).is_none() {
                    continue;
                }
                let y = hm.footing(&p.prefab, p.at, p.rot);
                let xf = Transform::from_xyz(p.at[0], y, p.at[1])
                    .with_rotation(Quat::from_rotation_y(-p.rot.to_radians()));
                let yard = yard(&map, &p.prefab, &p.tag, &xf, &ground);
                let here = format!("{name} {} at {:?}", p.prefab, p.at);
                // (Old Sallow's ruins lay under the reservoir.)
                if p.prefab.ends_with("_drowned")
                    || (name == "sallow_reservoir" && p.prefab != base)
                    || matches!(p.tag.as_str(), "post_office" | "mine_office")
                {
                    assert!(yard.is_empty(), "{here}: {yard:?}");
                    continue;
                }
                houses += 1;
                yards += usize::from(!yard.is_empty());
                let [w, _, d] = prefab_footprint(&p.prefab);
                let (hw, hd) = (w * 0.5, d * 0.5);
                let door = prefab_entrances(&p.prefab)[0];
                let hana = name == "marrow_creek" && Vec2::from(p.at) == Vec2::new(-20.2, 63.5);
                assert_eq!(
                    yard.iter().any(|(m, _)| *m == "dog_kennel"),
                    hana,
                    "{here}: {yard:?}"
                );
                for (m, t) in &yard {
                    let thing = things[m];
                    if name == "cinder_works" {
                        assert!(
                            ["bench_charred", "tub_iron", "clothes_line_bare"].contains(m),
                            "{here}: {m}"
                        );
                    }
                    // Its footprint, building-local.
                    let c = t.translation.xz();
                    let turned = (t.rotation * Vec3::X).x.abs() < 0.5;
                    let half = if turned {
                        Vec2::new(thing.hz, thing.hx)
                    } else {
                        Vec2::new(thing.hx, thing.hz)
                    };
                    let (lo, hi) = (c - half, c + half);
                    assert!(
                        lo.x >= hw || hi.x <= -hw || lo.y >= hd || hi.y <= -hd,
                        "{here}: {m} at {c} in the house"
                    );
                    let off = (c.abs() - Vec2::new(hw, hd)).max(Vec2::ZERO).length();
                    assert!(off <= YARD_REACH, "{here}: {m} at {c}, {off} m out");
                    let dw = door.width * 0.5 + 1.0;
                    assert!(
                        !(lo.x < dw && hi.x > -dw && hi.y > door.at[1] - door.clear && lo.y < hd),
                        "{here}: {m} at {c} in the door's way"
                    );
                    // Off every other placement.
                    let at = xf.transform_point(Vec3::new(c.x, 0.0, c.y)).xz();
                    for o in &map.placements {
                        if std::ptr::eq(o, p) {
                            continue;
                        }
                        let [ow, _, od] = prefab_footprint(&o.prefab);
                        let l = Quat::from_rotation_y(o.rot.to_radians())
                            * Vec3::new(at.x - o.at[0], 0.0, at.y - o.at[1]);
                        let out = (l.xz().abs() - Vec2::new(ow, od) * 0.5).max(Vec2::ZERO);
                        assert!(
                            out.length() > half.length(),
                            "{here}: {m} on the {} at {:?}",
                            o.prefab,
                            o.at
                        );
                    }
                    // On the ground, not hanging over it.
                    let g = ground(at).expect("in the water") - y;
                    assert!(t.translation.y <= g + 0.01, "{here}: {m} floats");
                }
            }
            assert!(
                yards * 4 >= houses * 3,
                "{name}: {yards} yards for {houses} houses"
            );
        }
    }
}
