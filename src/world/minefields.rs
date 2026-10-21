//! `--gallery minefields`: the minelayer's fields laid at once, where the
//! game's minelayer would lay them this raid (`extraction_core::map_mines`,
//! the same pick the map lint checks), for screenshots. The minelayer
//! itself only sets out minutes into a raid.
//!
//! ```text
//! scripts/shot.sh docs/shots/map-design/minefield.png 16,0,82,330,-14 140 \
//!     --map marrow_creek --gallery minefields
//! ```

use bevy::prelude::*;

use crate::world::Zone;

pub fn lay_for_gallery(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut done: Local<bool>,
) {
    let (Some(zone), false) = (zone, *done) else {
        return;
    };
    if opts.gallery.as_deref() != Some("minefields") {
        return;
    }
    *done = true;
    let walk = extraction_core::map_nav::zone_grid(
        &zone.map,
        &zone.hm,
        extraction_core::map_nav::MAX_SLOPE,
    );
    for f in extraction_core::map_mines::fields(&zone.map, &zone.hm, &walk) {
        for (k, m) in f.mines.iter().take(f.laid).enumerate() {
            let at = zone.on_ground(m[0], m[1]);
            crate::robots::minelayer::lay_mine(&mut commands, at, k as f32 * 0.37);
        }
        info!("minefields: {} at {:.0?}", f.name, f.site);
    }
}
