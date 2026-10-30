//! Where the tapes lie, and the players to hear them on: each tape by
//! the note of the person who made it (`docs/LORE.md`; owners Story's),
//! on whatever surface is there. Laid when the valley loads, like the
//! notes; a tape you took home is laid again next raid, as loot is.

use bevy::prelude::*;

use crate::items::{SpawnItem, spawn_item};

/// One thing laid by a note: the map, the item, where (x, z), and how
/// far (m) to its right of the note as the note faces.
pub struct Place {
    pub map: &'static str,
    pub item: &'static str,
    pub at: [f32; 2],
    pub rot: f32,
    pub aside: f32,
}

const fn by(map: &'static str, item: &'static str, at: [f32; 2], rot: f32, aside: f32) -> Place {
    Place {
        map,
        item,
        at,
        rot,
        aside,
    }
}

pub const PLACES: &[Place] = &[
    // Hana's house by the well: by her letter to Anna, not on it (her
    // table top stays bare), and a player to hear it on.
    by(
        "marrow_creek",
        "tape_practice",
        [-19.60, 62.20],
        208.3,
        0.45,
    ),
    by(
        "marrow_creek",
        "cassette_player",
        [-19.60, 62.20],
        208.3,
        -0.5,
    ),
    // The south gate booth: the soldier's mix and his player.
    by("marrow_creek", "tape_duty", [29.50, 162.90], 153.4, 0.3),
    by(
        "marrow_creek",
        "cassette_player",
        [29.50, 162.90],
        153.4,
        -0.35,
    ),
    // The Kettle chapel, by the sexton's note.
    by("kettle_pass", "tape_hymns", [-7.70, 155.40], 356.8, 0.3),
    // The hunting lodge, by Fiala's book.
    by("kettle_pass", "tape_lodge", [205.30, -204.45], 331.4, 0.3),
    by(
        "kettle_pass",
        "record_augustin",
        [205.30, -204.45],
        331.4,
        -0.35,
    ),
    // Vlasta's, by the foreman's house note.
    by(
        "cinder_works",
        "tape_birthday",
        [-109.20, 173.30],
        205.2,
        0.3,
    ),
    // The keeper's cottage, by Pech's note.
    by(
        "sallow_reservoir",
        "tape_bells",
        [-56.30, 206.80],
        295.2,
        0.3,
    ),
];

/// A standing instrument: where (x, floor y, z) and which way its front
/// faces (the maps' `rot`, front its -Z). Spots chosen by looking: each
/// stands on a floor with its back to a wall and room in front.
pub struct Fixture {
    pub map: &'static str,
    pub what: Standing,
    pub at: [f32; 3],
    pub rot: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Standing {
    Piano,
    Harmonium,
    Gramophone,
}

pub const FIXTURES: &[Fixture] = &[
    // Hana's upright, where Anna practised: against the front room's
    // back partition, between its two doorways, facing the front door.
    Fixture {
        map: "marrow_creek",
        what: Standing::Piano,
        at: [-18.21, 19.08, 95.00],
        rot: 90.0,
    },
    // Fiala's gramophone, on the lodge floor against the windowless log
    // wall (the one table is the loot's).
    Fixture {
        map: "kettle_pass",
        what: Standing::Gramophone,
        at: [201.81, 36.63, -206.00],
        rot: 90.0,
    },
    // The sexton's harmonium, by the altar against the east wall.
    Fixture {
        map: "kettle_pass",
        what: Standing::Harmonium,
        at: [-11.00, 12.65, 156.52],
        rot: 0.0,
    },
    // Old Sallow's drowned church, open to the sky: an organ in the silt
    // by the altar, facing the doorway (Pech's tape is its bells and a few
    // bars of it).
    Fixture {
        map: "sallow_reservoir",
        what: Standing::Harmonium,
        at: [-67.49, 38.95, -28.00],
        rot: 270.0,
    },
];

/// Where a harmonium's keys sit against the piano's (`piano::Keyboard`;
/// `art/props/music.py` builds it so).
pub const HARMONIUM_SHIFT: f32 = 0.023;

pub(super) fn stand_fixtures(
    mut commands: Commands,
    launch: Option<Res<crate::LaunchOptions>>,
    zone: Option<Res<crate::world::Zone>>,
    mut done: Local<Option<String>>,
) {
    let (Some(launch), Some(_)) = (launch, zone) else {
        return;
    };
    if done.as_deref() == Some(launch.map.as_str()) {
        return;
    }
    *done = Some(launch.map.clone());
    for f in FIXTURES.iter().filter(|f| f.map == launch.map) {
        let at = Transform::from_xyz(f.at[0], f.at[1], f.at[2])
            .with_rotation(Quat::from_rotation_y(-f.rot.to_radians()));
        match f.what {
            Standing::Piano => {
                super::piano::spawn(&mut commands, at, super::piano::Keyboard::default());
            }
            Standing::Harmonium => {
                super::piano::spawn(
                    &mut commands,
                    at,
                    super::piano::Keyboard {
                        low: 36,
                        high: 96,
                        shift: HARMONIUM_SHIFT,
                        voice: super::piano::Voice::Reed,
                    },
                );
            }
            Standing::Gramophone => {
                super::gramophone::spawn(&mut commands, at);
            }
        }
    }
}

/// Lower than this over the ground is the floor (m).
pub const SURFACE: f32 = 0.3;

/// A surface by the note to lay a thing on: beside it as asked, the other
/// side, then round it further out. Never the floor (Story's rule for
/// scenes and loot alike).
pub fn surface_near(zone: &crate::world::Zone, p: &Place, yaw: Quat) -> Option<(f32, f32, f32)> {
    let mut tries = vec![Vec3::X * p.aside, Vec3::X * -p.aside];
    for r in [0.6f32, 0.9, 1.2, 1.5] {
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU;
            tries.push(Vec3::new(a.cos(), 0.0, a.sin()) * r);
        }
    }
    tries.into_iter().find_map(|o| {
        let off = yaw * o;
        let (x, z) = (p.at[0] + off.x, p.at[1] + off.z);
        let y = crate::world::notes::rest_height(zone, x, z, 2.2);
        (y - zone.ground(x, z) >= SURFACE).then_some((x, y, z))
    })
}

/// Lays this valley's tapes and players: part of the zone's loot roll
/// (`items::loot::roll_zone_loot`), so a zone restored from a save keeps
/// what it had instead of getting another set, and a new cycle lays them
/// again as it re-rolls the loot. A player laid here comes with a cell,
/// part run down: it was someone's.
pub fn lay_tapes(
    commands: &mut Commands,
    zone: &crate::world::Zone,
    catalog: &crate::items::Catalog,
    map: &str,
) {
    for p in PLACES.iter().filter(|p| p.map == map) {
        let Some(kind) = catalog.kind(p.item) else {
            warn!("no item {}", p.item);
            continue;
        };
        let yaw = Quat::from_rotation_y(-p.rot.to_radians());
        let Some((x, y, z)) = surface_near(zone, p, yaw) else {
            warn!("{}: no surface by {:?}", p.item, p.at);
            continue;
        };
        let half = catalog.get(kind).size_m()[1] * 0.5;
        let e = spawn_item(
            commands,
            catalog,
            SpawnItem {
                kind,
                transform: Transform::from_xyz(x, y + half + 0.01, z).with_rotation(yaw),
                condition: 0.8,
            },
        );
        if p.item == "cassette_player" {
            commands.entity(e).insert(super::deck::WithCell(0.5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything laid finds a table, shelf or crate by its note: none of
    /// it lies on the floor.
    #[test]
    fn everything_lies_on_a_surface() {
        let catalog =
            extraction_core::items::ItemCatalog::from_ron(extraction_core::items::BUILTIN_CATALOG)
                .unwrap();
        let mut maps: Vec<&str> = PLACES.iter().map(|p| p.map).collect();
        maps.dedup();
        for map in maps {
            let zone = crate::world::Zone::load(map);
            for p in PLACES.iter().filter(|p| p.map == map) {
                assert!(catalog.kind(p.item).is_some(), "{}", p.item);
                let yaw = Quat::from_rotation_y(-p.rot.to_radians());
                let (x, y, z) = surface_near(&zone, p, yaw)
                    .unwrap_or_else(|| panic!("{} in {map}: no surface", p.item));
                assert!(y - zone.ground(x, z) >= SURFACE, "{}", p.item);
            }
        }
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    /// The harmonium's keys where its model has them (key_36 at -0.4025,
    /// key_60 at -0.0805, key_96 at 0.4025).
    #[test]
    fn the_harmoniums_keys_match_its_model() {
        let kb = crate::radio::piano::Keyboard {
            low: 36,
            high: 96,
            shift: HARMONIUM_SHIFT,
            voice: crate::radio::piano::Voice::Reed,
        };
        for (k, x) in [(36, -0.4025), (60, -0.0805), (96, 0.4025)] {
            assert!((kb.x(k) - x).abs() < 1e-4, "key_{k} at {}", kb.x(k));
        }
        assert_eq!(kb.under(Vec3::new(kb.x(21 + 15), 0.75, -0.19)), Some(36));
        assert_eq!(
            kb.under(Vec3::new(-0.5, 0.75, -0.19)),
            None,
            "past its compass"
        );
    }

    /// Every standing instrument is on its map's ground or a floor above
    /// it (not buried, not floating far up), and away from the others.
    #[test]
    fn fixtures_stand_on_a_floor() {
        for f in FIXTURES {
            let zone = crate::world::Zone::load(f.map);
            let ground = zone.ground(f.at[0], f.at[2]);
            let over = f.at[1] - ground;
            assert!(
                (-0.3..1.5).contains(&over),
                "{:?} in {} is {over:.2} m over the ground",
                f.what,
                f.map
            );
        }
    }
}
