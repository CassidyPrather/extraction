//! A snare by water charges it: its reach runs out through the water, so
//! wading within [`SNARE_WATER`] of its edge sets it off and the arc comes
//! to you through the water. Crackling on the water's surface shows how far
//! (`visuals`, `anomaly_particles.wgsl` mode 9), brighter as it charges.

use bevy::prelude::*;
use extraction_core::anomaly::AnomalyKind;

use super::Anomaly;

/// How far past its edge a snare's charge runs through water, m.
pub const SNARE_WATER: f32 = 3.0;
/// Wading deeper than this (m) and you're in its water.
pub const WADING: f32 = 0.05;
/// How often each snare looks again for water by it (they creep), s.
const LOOK_EVERY: f32 = 2.0;

/// A snare whose edge touches water: the water's level, and how far from
/// its centre its charge runs.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ChargesWater {
    pub level: f32,
    pub reach: f32,
}

/// Whether water lies at a snare's edge (centre `c`, radius `r`): its
/// level, if so.
pub fn water_at_edge(hm: &extraction_core::terrain::Heightmap, c: Vec3, r: f32) -> Option<f32> {
    (0..24).find_map(|k| {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        [r * 0.6, r, r + 0.5].into_iter().find_map(|d| {
            let p = [c.x + a.cos() * d, c.z + a.sin() * d];
            hm.water_level(p).filter(|w| *w > hm.height(p))
        })
    })
}

/// Marks the snares that touch water (and unmarks those that crept off).
pub fn find_water(
    mut commands: Commands,
    snares: Query<(Entity, &Anomaly, &GlobalTransform, Option<&ChargesWater>)>,
    zone: Res<crate::world::Zone>,
    time: Res<Time>,
    mut since: Local<Option<f32>>,
) {
    let t = since.get_or_insert(LOOK_EVERY);
    *t += time.delta_secs();
    if *t < LOOK_EVERY {
        return;
    }
    *t = 0.0;
    for (e, a, gt, had) in &snares {
        if a.kind != AnomalyKind::Snare {
            continue;
        }
        let now = water_at_edge(&zone.hm, gt.translation(), a.radius).map(|level| ChargesWater {
            level,
            reach: a.radius + SNARE_WATER,
        });
        match (now, had) {
            (Some(w), h) if h != Some(&w) => {
                commands.entity(e).try_insert(w);
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<ChargesWater>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Body;

    /// A snare on the canal's bank: wading in the canal within its water's
    /// reach sets it off into you; standing as far off on dry ground
    /// doesn't.
    #[test]
    fn a_snare_by_water_reaches_a_wader() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: "cinder_works".into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
        // The canal across x = -70: the first wet and first dry points
        // going north, the bank between.
        let wet = (0..120)
            .map(|z| [-70.0, z as f32 * 0.5])
            .find(|p| {
                zone.hm
                    .water_level(*p)
                    .is_some_and(|w| w > zone.hm.height(*p))
            })
            .expect("the canal");
        let bank = (0..60)
            .map(|k| [-70.0, wet[1] - k as f32 * 0.5])
            .find(|p| zone.hm.water_level(*p).is_none())
            .expect("its bank");
        let r = 2.0;
        let snare_at = zone.on_ground(bank[0], bank[1] - r + 0.2);
        let snare =
            super::super::spawn_anomaly(app.world_mut(), AnomalyKind::Snare, snare_at, r, None);
        for _ in 0..(90.0 * (LOOK_EVERY + 0.5)) as usize {
            app.update();
        }
        let w = *app
            .world()
            .get::<ChargesWater>(snare)
            .expect("it charges the canal");
        // Wading, 1.5 m past its edge, in the water.
        let wade = (0..40)
            .map(|k| [-70.0, wet[1] + k as f32 * 0.25])
            .find(|p| {
                let d = (p[1] - snare_at.z).abs();
                d > r + 0.5
                    && d < w.reach - 0.3
                    && zone
                        .hm
                        .water_level(*p)
                        .is_some_and(|l| l - zone.hm.height(*p) > 0.2)
            })
            .expect("water within its reach");
        app.world_mut()
            .get_mut::<Transform>(me)
            .unwrap()
            .translation = Vec3::new(wade[0], zone.hm.height(wade) + 0.05, wade[1]);
        let hp = app.world().get::<crate::health::Health>(me).unwrap().hp;
        let mut fired = false;
        for _ in 0..90 {
            app.update();
            if app.world().get::<Anomaly>(snare).unwrap().charge < 0.5 {
                fired = true;
                break;
            }
        }
        let wading = app.world().get::<Body>(me).unwrap().wading;
        assert!(wading > WADING, "wading ({wading:.2})");
        assert!(fired, "it fired through the water");
        assert!(
            app.world().get::<crate::health::Health>(me).unwrap().hp < hp,
            "into the wader"
        );
    }
}
