//! Map simulation: bots play raids headless and we write down what
//! happened.
//!
//! `extraction --sim N` (see `main.rs`) runs N raids, each in its own
//! process with `--sim-run NAME`: that process puts the local player under
//! a [`raider::Raider`] brain, plays one raid at full speed, writes
//! telemetry to `target/sim/<map>/<NAME>.jsonl` ([`telemetry`]) and exits
//! when the bot extracts, dies or runs out of time.
//! `scripts/sim_report.py` turns a directory of runs into heatmaps and a
//! summary. See `docs/SIMULATION.md`.
//!
//! Nothing here changes the game's rules: the raider drives the same
//! `RigInput` and `HandAction`s as a player, through [`crate::bot`].

pub mod indoor;
pub mod playtest;
pub mod raider;
pub mod telemetry;

use bevy::prelude::*;
use extraction_core::map::MarkerKind;

use crate::launch::LaunchOptions;
use crate::world::Zone;

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        let Some(opts) = app.world().get_resource::<LaunchOptions>().cloned() else {
            return;
        };
        let Some(run) = opts.sim_run.clone() else {
            return;
        };
        let dir = opts
            .sim_out
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("target/sim").join(&opts.map));
        app.insert_resource(telemetry::SimLog::open(dir.join(format!("{run}.jsonl"))))
            .add_systems(Startup, spawn_sim_kit)
            .add_systems(PostStartup, raider::attach_raider)
            .add_systems(
                Update,
                (raider::think, telemetry::record, telemetry::finish).chain(),
            );
    }
}

/// What a fresh game keeps in the truck's stash and the dev cache lacks:
/// the radio and two dressings, laid out by the insertion point.
fn spawn_sim_kit(mut commands: Commands, zone: Res<Zone>, catalog: Res<crate::items::Catalog>) {
    let Some(ins) = zone.marker(&MarkerKind::Insertion) else {
        return;
    };
    for (i, id) in ["radio", "field_dressing", "field_dressing"]
        .iter()
        .enumerate()
    {
        let Some(kind) = catalog.kind(id) else {
            continue;
        };
        let x = ins.at[0] + 1.0;
        let z = ins.at[1] - 3.0 - i as f32 * 0.8;
        crate::items::spawn_item(
            &mut commands,
            &catalog,
            crate::items::SpawnItem {
                kind,
                transform: Transform::from_xyz(x, zone.ground(x, z) + 0.25, z),
                condition: 0.85,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sim raid's first minute: the bot kits up, picks somewhere to go and
    /// sets off, and the telemetry says so.
    #[test]
    fn a_raider_kits_up_and_sets_off() {
        let dir = std::env::temp_dir().join(format!("extraction-sim-{}", std::process::id()));
        let opts = LaunchOptions {
            sim_run: Some("smoke".into()),
            sim_out: Some(dir.clone()),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        // A minute of game time.
        for _ in 0..90 * 60 {
            app.update();
        }
        app.world_mut().resource_mut::<telemetry::SimLog>().flush();
        let text = std::fs::read_to_string(dir.join("smoke.jsonl")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let has = |ev: &str| events.iter().any(|e| e["ev"] == ev);
        assert!(has("start") && has("kit") && has("goal"), "{text}");
        let kit = events.iter().find(|e| e["ev"] == "kit").unwrap();
        assert_eq!(kit["gun"], true, "picked up the AK");
        assert!(kit["rounds"].as_u64().unwrap() > 0, "and loaded it");
        let stats = &app.world().resource::<telemetry::SimLog>().stats;
        assert!(
            stats.distance > 60.0,
            "walked off toward the goal ({:.0} m)",
            stats.distance
        );
    }

    /// A snare that catches a bot (even a reckless one, who walks through
    /// anomalies it hasn't been hurt by) is never walked into again: it
    /// steps out the way it came and goes round.
    #[test]
    fn a_bot_caught_by_a_snare_never_walks_into_it_again() {
        use crate::bot::{Bot, Task};
        use bevy::math::Vec3Swizzles;
        let dir = std::env::temp_dir().join(format!("extraction-snare-{}", std::process::id()));
        let opts = LaunchOptions {
            sim_run: Some("snare".into()),
            sim_out: Some(dir.clone()),
            personality: Some("reckless".into()),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        // Kitted up and walking somewhere.
        let mut target = None;
        for _ in 0..90 * 90 {
            app.update();
            let walk = app
                .world_mut()
                .query::<&Bot>()
                .iter(app.world())
                .find_map(|b| {
                    b.tasks.iter().find_map(|t| match t {
                        Task::WalkTo { to, .. } => Some(*to),
                        _ => None,
                    })
                });
            let me = app
                .world_mut()
                .query_filtered::<&Transform, With<crate::rig::LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap()
                .translation;
            let stage_raid = app
                .world_mut()
                .query::<&raider::Raider>()
                .iter(app.world())
                .any(|r| r.stage == raider::Stage::Raid);
            if let Some(to) = walk.filter(|to| stage_raid && to.distance(me.xz()) > 6.0) {
                target = Some(to);
                break;
            }
        }
        let to = target.expect("it set off somewhere");
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(to.x, to.y);
        crate::anomaly::spawn_anomaly(
            app.world_mut(),
            extraction_core::anomaly::AnomalyKind::Snare,
            at,
            2.5,
            None,
        );
        for _ in 0..90 * 90 {
            app.update();
        }
        app.world_mut().resource_mut::<telemetry::SimLog>().flush();
        let text = std::fs::read_to_string(dir.join("snare.jsonl")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let entries = text
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter(|e| e["ev"] == "anomaly_enter" && e["kind"] == "Snare")
            .filter(|e| {
                let p = Vec2::new(
                    e["x"].as_f64().unwrap() as f32,
                    e["z"].as_f64().unwrap() as f32,
                );
                p.distance(to) < 4.0
            })
            .count();
        assert_eq!(entries, 1, "caught once, then kept clear: {entries}");
    }

    /// The way into the truck from behind, at base and at every
    /// rendezvous: up the steps, into the bed.
    #[test]
    fn a_bot_climbs_into_the_truck() {
        use crate::bot::Bot;
        use crate::raid::truck::{Truck, base_pose, rv_pose};
        use crate::rig::LocalPlayer;
        use bevy::math::Vec3Swizzles;
        // Every valley's stops (`EXTRACTION_TEST_MAP` for just one).
        for map in crate::test_maps() {
            let opts = LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let zone = app.world().resource::<Zone>().clone();
            let rvs = zone.map.markers_of(&MarkerKind::Rendezvous).count();
            let poses: Vec<(String, Transform)> =
                std::iter::once(("base".to_string(), base_pose(&zone)))
                    .chain((0..rvs).map(|i| (format!("rendezvous {i}"), rv_pose(&zone, i))))
                    .collect();
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            let truck = app
                .world_mut()
                .query_filtered::<Entity, With<Truck>>()
                .iter(app.world())
                .next()
                .unwrap();
            // Keep the raid logic from parking the truck back at base.
            app.world_mut().get_mut::<Truck>(truck).unwrap().at_base = true;
            for (name, pose) in poses {
                *app.world_mut().get_mut::<Transform>(truck).unwrap() = pose;
                let behind = pose.transform_point(Vec3::new(0.0, 0.0, 7.0));
                let ground = zone.ground(behind.x, behind.z);
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation = Vec3::new(behind.x, ground + 0.05, behind.z);
                app.update();
                let p = |z: f32| pose.transform_point(Vec3::new(0.0, 0.0, z)).xz();
                app.world_mut()
                    .entity_mut(player)
                    .insert(Bot::new(super::raider::board_tasks(p, None)));
                for _ in 0..90 * 20 {
                    app.update();
                    if app.world().get::<Bot>(player).unwrap().done() {
                        break;
                    }
                }
                let at = app.world().get::<Transform>(player).unwrap().translation;
                let gt = *app.world().get::<GlobalTransform>(truck).unwrap();
                let t = app.world().get::<Truck>(truck).unwrap();
                assert!(
                    t.in_bed(&gt, at + Vec3::Y * 0.1),
                    "{map} {name}: the bot is in the bed (truck-local {:.2?})",
                    gt.affine().inverse().transform_point3(at)
                );
            }
        }
    }
}
