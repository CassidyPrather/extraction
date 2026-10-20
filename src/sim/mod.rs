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

    /// What a raider does at the end of a raid, at every rendezvous of
    /// every valley: from where it lines up behind the waiting truck
    /// (`raider::extract_step`), up into the bed and pull the bell, and the
    /// raid ends.
    #[test]
    fn a_bot_leaves_from_every_rendezvous() {
        use crate::bot::Bot;
        use crate::raid::truck::{Pressable, rv_pose};
        use crate::raid::{Phase, Raid};
        use crate::rig::LocalPlayer;
        use bevy::math::Vec3Swizzles;
        // In the training yard the bell ends the practice instead.
        for map in crate::test_maps()
            .into_iter()
            .filter(|m| m != crate::camp::TRAINING_YARD)
        {
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
            let names: Vec<String> = zone
                .map
                .markers_of(&MarkerKind::Rendezvous)
                .map(|m| m.name.clone())
                .collect();
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            let bell = app
                .world_mut()
                .query::<(Entity, &Pressable)>()
                .iter(app.world())
                .find(|(_, p)| **p == Pressable::BellPull)
                .map(|(e, _)| e);
            for (rv, name) in names.iter().enumerate() {
                app.world_mut().resource_mut::<Raid>().phase =
                    Phase::TruckWaiting { rv, left: 1.0e6 };
                app.update();
                let pose = rv_pose(&zone, rv);
                let p = |z: f32| pose.transform_point(Vec3::new(0.0, 0.0, z)).xz();
                let behind = p(8.0);
                app.world_mut()
                    .get_mut::<Transform>(player)
                    .unwrap()
                    .translation =
                    Vec3::new(behind.x, zone.ground(behind.x, behind.y) + 0.05, behind.y);
                app.update();
                app.world_mut()
                    .entity_mut(player)
                    .insert(Bot::new(super::raider::board_tasks(p, bell)));
                let mut left = false;
                for _ in 0..90 * 20 {
                    app.update();
                    if matches!(
                        app.world().resource::<Raid>().phase,
                        Phase::Extracting { .. }
                    ) {
                        left = true;
                        break;
                    }
                    if app.world().get::<Bot>(player).unwrap().done() {
                        break;
                    }
                }
                let at = app.world().get::<Transform>(player).unwrap().translation;
                assert!(
                    left,
                    "{map} {name}: the bot got in and rang the bell (stood at {:.1?}, truck-local {:.2?})",
                    at,
                    pose.compute_affine().inverse().transform_point3(at)
                );
                // Back out for the next stop.
                app.world_mut().resource_mut::<Raid>().phase = Phase::Out;
                app.update();
            }
        }
    }
}
