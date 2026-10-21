//! Where the tapes lie, and the players to hear them on: each tape by
//! the note of the person who made it (`docs/LORE.md`; owners Story's),
//! on whatever surface is there. Laid when the valley loads, like the
//! notes; a tape you took home is laid again next raid, as loot is.

use bevy::prelude::*;

use crate::items::{Catalog, SpawnItem, spawn_item};

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
    by("marrow_creek", "tape_practice", [-17.30, 93.70], 208.3, 0.45),
    by("marrow_creek", "cassette_player", [-17.30, 93.70], 208.3, -0.5),
    // The south gate booth: the soldier's mix and his player.
    by("marrow_creek", "tape_duty", [47.50, 284.40], 153.4, 0.3),
    by("marrow_creek", "cassette_player", [47.50, 284.40], 153.4, -0.35),
    // The Kettle chapel, by the sexton's note.
    by("kettle_pass", "tape_hymns", [-7.70, 155.40], 356.8, 0.3),
    // The hunting lodge, by Fiala's book.
    by("kettle_pass", "tape_lodge", [205.30, -204.45], 331.4, 0.3),
    // Vlasta's, by the foreman's house note.
    by("cinder_works", "tape_birthday", [-109.20, 173.30], 205.2, 0.3),
    // The keeper's cottage, by Pech's note.
    by("sallow_reservoir", "tape_bells", [-56.30, 206.80], 295.2, 0.3),
];

/// Lays this valley's tapes once it has loaded. A player laid here comes
/// with a cell, part run down: it was someone's.
pub(super) fn lay_tapes(
    mut commands: Commands,
    zone: Option<Res<crate::world::Zone>>,
    launch: Option<Res<crate::LaunchOptions>>,
    catalog: Res<Catalog>,
    mut done: Local<Option<String>>,
) {
    let (Some(zone), Some(launch)) = (zone, launch) else {
        return;
    };
    if done.as_deref() == Some(launch.map.as_str()) {
        return;
    }
    *done = Some(launch.map.clone());
    for p in PLACES.iter().filter(|p| p.map == launch.map) {
        let Some(kind) = catalog.kind(p.item) else {
            warn!("no item {}", p.item);
            continue;
        };
        let yaw = Quat::from_rotation_y(-p.rot.to_radians());
        let off = yaw * Vec3::X * p.aside;
        let (x, z) = (p.at[0] + off.x, p.at[1] + off.z);
        let y = crate::world::notes::rest_height(&zone, x, z, 2.2);
        let half = catalog.get(kind).size_m()[1] * 0.5;
        let e = spawn_item(
            &mut commands,
            &catalog,
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
