//! Underground floors that are there for the raster but not for the
//! physics. A site's raster (`extraction_core::underground::Raster`) says
//! where its floor is; the game stands players, judges items "down a site"
//! and lights rooms from it. The colliders are built from the same layout,
//! separately. Where the two part, an item let go of falls through a floor
//! the player stood on: a wristwatch in Marrow's bunker went through at
//! 6 m/s (`docs/SOAK.md`, "Chaos monkey").
//!
//! The test casts down at every open cell of every site in every valley
//! (half a metre apart, walls' feet included) and wants a static collider at
//! the raster's floor there.

/// Sites whose floors are known to part from the raster, with whom: (map,
/// site, owner, what). Each comes off when fixed; anywhere else fails.
pub const OPEN: &[(&str, &str, &str, &str)] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use extraction_core::underground as ug;

    use crate::world::{Layer, Zone};

    /// How far the collider's top may be from the raster's floor, m.
    const NEAR: f32 = 0.25;

    #[test]
    fn every_underground_floor_is_there_for_things_dropped_on_it() {
        let mut bad = Vec::new();
        for map in crate::test_maps() {
            let opts = crate::LaunchOptions {
                robots: false,
                map: map.clone(),
                map_explicit: true,
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..3 {
                app.update();
            }
            let zone = app.world().resource::<Zone>().clone();
            for site in ug::placed(&zone.map) {
                if OPEN.iter().any(|(m, s, ..)| *m == map && *s == site.id) {
                    continue;
                }
                let Some(r) = ug::raster(&site.id) else {
                    continue;
                };
                let footing = zone.hm.footing(&site.id, site.at, site.rot);
                // Open cells away from walls, half a metre apart.
                let mut probes = Vec::new();
                let open = |i: usize, j: usize| {
                    let p = [
                        r.origin[0] + (i as f32 + 0.5) * ug::CELL,
                        r.origin[1] + (j as f32 + 0.5) * ug::CELL,
                    ];
                    r.floor_at(p).map(|f| (p, f))
                };
                for j in (1..r.nz.saturating_sub(1)).step_by(2) {
                    for i in (1..r.nx.saturating_sub(1)).step_by(2) {
                        let Some((p, f)) = open(i, j) else {
                            continue;
                        };
                        // Beside a wall is in (a gap where the floor
                        // meets the wall is a hole too); beside another
                        // level (a stair's edge) isn't judged.
                        let level = [(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)]
                            .iter()
                            .all(|&(a, b)| open(a, b).is_none_or(|(_, g)| (g - f).abs() < 0.01));
                        if !level {
                            continue;
                        }
                        let w = site.to_world(p);
                        probes.push(Vec3::new(w[0], footing + f, w[1]));
                    }
                }
                let total = probes.len();
                let misses: Vec<Vec3> = app
                    .world_mut()
                    .run_system_once(
                        move |q: SpatialQuery,
                              of: Query<&ColliderOf>,
                              bodies: Query<&RigidBody>| {
                            let filter = SpatialQueryFilter::from_mask(Layer::World);
                            let statics = |e: Entity| {
                                of.get(e)
                                    .ok()
                                    .and_then(|c| bodies.get(c.body).ok())
                                    .or_else(|| bodies.get(e).ok())
                                    .is_some_and(|b| *b == RigidBody::Static)
                            };
                            probes
                                .iter()
                                .copied()
                                .filter(|p| {
                                    let from = *p + Vec3::Y * 1.0;
                                    let hit = q.cast_ray_predicate(
                                        from,
                                        Dir3::NEG_Y,
                                        1.0 + NEAR + 0.5,
                                        true,
                                        &filter,
                                        &|e| statics(e),
                                    );
                                    // Something there by the floor, or above it
                                    // (a table, a bench: it catches what's
                                    // dropped too). Nothing, or only below:
                                    // a hole.
                                    !hit.is_some_and(|h| from.y - h.distance > p.y - NEAR)
                                })
                                .collect::<Vec<Vec3>>()
                        },
                    )
                    .unwrap();
                if !misses.is_empty() {
                    let some: Vec<String> = misses
                        .iter()
                        .take(4)
                        .map(|p| format!("({:.1}, {:.1}, {:.1})", p.x, p.y, p.z))
                        .collect();
                    bad.push(format!(
                        "{map}: {} has {} of {total} spots with no floor where the raster has one, e.g. {}",
                        site.id,
                        misses.len(),
                        some.join(", ")
                    ));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "underground floors the raster has and the colliders don't (fix, or list in \
             soak::floors::OPEN with the owner):\n{}",
            bad.join("\n")
        );
    }
}
