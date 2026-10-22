//! Light as the resource underground (`docs/UNDERGROUND.md`, "The dark").
//! A power cell found down a site is worn: left there years, it holds
//! between [`WORN`] of its charge, so a spare is worth having and none is
//! full. (Cells anywhere else are as they were.)

use bevy::prelude::*;

use super::Zone;

/// The share of its charge a cell found underground holds (low, high).
pub const WORN: (f32, f32) = (0.15, 0.6);

pub struct DarkPlugin;

impl Plugin for DarkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, wear_cells);
    }
}

/// How worn a cell found at `p` is: a share of its charge, the same for
/// the same spot every time.
pub fn worn_share(p: Vec3) -> f32 {
    let h = (p.x * 12.9898 + p.z * 78.233 + p.y * 37.719).sin() * 43_758.547;
    WORN.0 + (WORN.1 - WORN.0) * h.fract().abs()
}

/// A full cell turning up underground (loot, left about): worn. One that
/// isn't full (carried down, used, restored from a save) is left be.
fn wear_cells(
    zone: Option<Res<Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut cells: Query<(&mut crate::items::Item, &Transform), Added<crate::items::Item>>,
) {
    let Some(zone) = zone else {
        return;
    };
    let Some(cell) = catalog.kind("power_cell") else {
        return;
    };
    let full = catalog.get(cell).battery_wh;
    for (mut item, t) in &mut cells {
        if item.kind() != cell || item.charge_wh < full - 1e-3 {
            continue;
        }
        if super::underground::site_under(&zone, t.translation).is_none() {
            continue;
        }
        item.charge_wh = full * worn_share(t.translation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell spawned down a site comes worn, 15-60%; one up top, full; a
    /// part-used one taken down, as it was.
    #[test]
    fn cells_found_underground_are_worn() {
        let mut app = crate::headless_app(
            crate::LaunchOptions {
                robots: false,
                map: "marrow_creek".into(),
                ..Default::default()
            },
            false,
        );
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let kind = catalog.expect("power_cell");
        let full = catalog.get(kind).battery_wh;
        let site = extraction_core::underground::placed(&zone.map)
            .into_iter()
            .find(|s| s.id == "bunker_marrow")
            .unwrap();
        let l = extraction_core::underground::layout("bunker_marrow").unwrap();
        let hall = l.rooms.iter().find(|r| r.id == "hall").unwrap();
        let footing = zone.hm.footing(&site.id, site.at, site.rot);
        let down = {
            let p = site.to_world([hall.at[0], hall.at[1]]);
            Vec3::new(p[0], footing + l.floor_of(hall) + 0.3, p[1])
        };
        assert!(super::super::underground::site_under(&zone, down).is_some());
        let up = zone.on_ground(40.0, 270.0) + Vec3::Y * 0.3;
        let spawn = |app: &mut App, at: Vec3| {
            let e = crate::items::spawn_item(
                &mut app.world_mut().commands(),
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::from_translation(at),
                    condition: 1.0,
                },
            );
            app.world_mut().flush();
            e
        };
        let charge =
            |app: &App, e: Entity| app.world().get::<crate::items::Item>(e).unwrap().charge_wh;
        let below = spawn(&mut app, down);
        let above = spawn(&mut app, up);
        let used = spawn(&mut app, down + Vec3::X * 0.5);
        app.world_mut()
            .get_mut::<crate::items::Item>(used)
            .unwrap()
            .charge_wh = full * 0.9;
        app.update();
        let c = charge(&app, below);
        assert!(
            c >= full * WORN.0 - 1e-3 && c <= full * WORN.1 + 1e-3,
            "worn below: {c} of {full}"
        );
        assert_eq!(charge(&app, above), full, "full up top");
        assert!(
            (charge(&app, used) - full * 0.9).abs() < 1e-3,
            "a used one as it was"
        );
    }
}
