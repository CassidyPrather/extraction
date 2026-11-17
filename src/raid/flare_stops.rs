//! The flare stops (#94): a dead-end truck stand the map gives each valley
//! (`map_extract::flare_stops`), off the pads, that no set calls. A red
//! star fired within [`FLARE_REACH`] of one, while the truck's not already
//! coming, calls it there: the alternate way out, for the price of a
//! signal flare and every machine in the valley seeing the star
//! (`robots::distress`).
//!
//! The truck's pad index for a stop follows the rendezvous'
//! (`truck::pads`), so the rest of the raid (the drive, the wait, the
//! lines) treats it as any pad.

use bevy::prelude::*;
use extraction_core::map::MarkerKind;
use extraction_core::map_extract::{FLARE_REACH, flare_stops};

use super::{Phase, Raid, TRUCK_TRANSIT, lines};
use crate::robots::distress::SignalLit;
use crate::world::Zone;

pub struct FlareStopsPlugin;

impl Plugin for FlareStopsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SignalLit>()
            .add_systems(Update, star_calls);
    }
}

/// The flare stop a star lit at `at` calls: the nearest within
/// [`FLARE_REACH`] (on the ground's plan), as its pad index
/// (`truck::pads`).
pub fn stop_for(zone: &Zone, at: Vec3) -> Option<usize> {
    let first = zone.map.markers_of(&MarkerKind::Rendezvous).count();
    flare_stops(&zone.map)
        .iter()
        .enumerate()
        .map(|(k, (_, stop, _))| (k, Vec2::from(*stop).distance(at.xz())))
        .filter(|(_, d)| *d <= FLARE_REACH)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| first + k)
}

/// A red star lit near a flare stop, out in the valley with no truck yet
/// coming: the truck's called there.
fn star_calls(
    mut lit: MessageReader<SignalLit>,
    raid: Option<ResMut<Raid>>,
    zone: Option<Res<Zone>>,
) {
    let (Some(mut raid), Some(zone)) = (raid, zone) else {
        lit.clear();
        return;
    };
    for star in lit.read() {
        if raid.phase != Phase::Out {
            continue;
        }
        let Some(rv) = stop_for(&zone, star.at) else {
            continue;
        };
        let pads = super::truck::pads(&zone);
        let (name, stop) = &pads[rv];
        let b = lines::way([star.at.x, star.at.z], *stop);
        let n = raid.clock as u32;
        raid.pending_call = Some((
            lines::fill(lines::pick(lines::STAR_SEEN, n), name, 0.0, &b),
            false,
        ));
        raid.phase = Phase::TruckCalled {
            rv,
            eta: TRUCK_TRANSIT,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALLEYS: [&str; 5] = [
        "marrow_creek",
        "kettle_pass",
        "cinder_works",
        "sallow_reservoir",
        "rime_fell",
    ];

    /// In every valley, a star fired from each flare stop's vantage calls
    /// that stop, and the truck pulls in on it.
    #[test]
    fn a_star_from_each_vantage_calls_its_stop() {
        for valley in VALLEYS {
            let zone = Zone::load(valley);
            let stops = flare_stops(&zone.map);
            assert!(!stops.is_empty(), "{valley}: no flare stops");
            let first = zone.map.markers_of(&MarkerKind::Rendezvous).count();
            for (k, (name, stop, vantage)) in stops.iter().enumerate() {
                let star = zone.on_ground(vantage[0], vantage[1]) + Vec3::Y * 60.0;
                assert_eq!(stop_for(&zone, star), Some(first + k), "{valley}: {name}");
                let pose = super::super::truck::rv_pose(&zone, first + k);
                let off = pose.translation.xz().distance(Vec2::from(*stop));
                assert!(off < 0.5, "{valley}: the truck stands {off} m off {name}");
            }
        }
    }

    fn app() -> App {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    /// Out in the valley, a star near a stop calls the truck there, and
    /// the driver says where; a star far from every stop calls nothing,
    /// nor does one once the truck's already coming.
    #[test]
    fn a_star_near_a_stop_calls_the_truck_and_others_do_not() {
        let mut app = app();
        let zone = app.world().resource::<Zone>().clone();
        let first = zone.map.markers_of(&MarkerKind::Rendezvous).count();
        let (name, _, vantage) = flare_stops(&zone.map)[0];
        let (name, vantage) = (name.to_string(), vantage);
        let star = zone.on_ground(vantage[0], vantage[1]) + Vec3::Y * 60.0;
        let lit = |app: &mut App, at: Vec3| {
            app.world_mut().write_message(SignalLit { at, by: None });
            app.update();
        };
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        // Far from every stop: the valley's middle isn't one.
        let far = (0..400)
            .map(|i| zone.on_ground(-300.0 + i as f32 * 1.5, 0.0))
            .find(|p| stop_for(&zone, *p).is_none())
            .expect("somewhere off every stop");
        lit(&mut app, far);
        assert_eq!(app.world().resource::<Raid>().phase, Phase::Out);
        lit(&mut app, star);
        let raid = app.world().resource::<Raid>();
        assert!(
            matches!(raid.phase, Phase::TruckCalled { rv, .. } if rv == first),
            "{:?}",
            raid.phase
        );
        let said: Vec<String> = raid.log.iter().map(|l| l.0.clone()).collect();
        let pending = raid.pending_call.as_ref().map(|p| p.0.clone());
        assert!(
            said.iter().chain(pending.iter()).any(|l| l.contains(&name)),
            "the driver didn't say {name}: {said:?} {pending:?}"
        );
        // Called already, to a pad: a star doesn't move it.
        app.world_mut().resource_mut::<Raid>().phase = Phase::TruckCalled { rv: 0, eta: 50.0 };
        lit(&mut app, star);
        assert!(matches!(
            app.world().resource::<Raid>().phase,
            Phase::TruckCalled { rv: 0, .. }
        ));
    }

    /// End to end: a red star fired from a stop's vantage, as a signal
    /// flare fires it, lights (`robots::distress::signals`) and the truck's
    /// called to that stop.
    #[test]
    fn a_signal_flare_fired_at_a_vantage_brings_the_truck() {
        use crate::weapons::flares::{Flare, Star};
        let mut app = app();
        let zone = app.world().resource::<Zone>().clone();
        let first = zone.map.markers_of(&MarkerKind::Rendezvous).count();
        let vantage = flare_stops(&zone.map)[0].2;
        let me = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
        app.world_mut().spawn((
            Name::new("Flare"),
            Flare::lit(
                Star::Red,
                Vec3::Y * crate::items::usage::SIGNAL_FLARE_SPEED,
                Some(me),
            ),
            Transform::from_translation(zone.on_ground(vantage[0], vantage[1]) + Vec3::Y * 1.6),
            Visibility::default(),
        ));
        for _ in 0..120 {
            app.update();
            if app.world().resource::<Raid>().phase != Phase::Out {
                break;
            }
        }
        let phase = app.world().resource::<Raid>().phase.clone();
        assert!(
            matches!(phase, Phase::TruckCalled { rv, .. } if rv == first),
            "{phase:?}"
        );
    }
}
