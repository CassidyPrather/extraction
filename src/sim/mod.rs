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

pub mod harvest;
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
            .add_systems(Update, armour_up)
            .add_systems(
                Update,
                (
                    raider::think,
                    telemetry::record,
                    telemetry::finish,
                    career_memory,
                )
                    .chain(),
            );
    }
}

/// A career of sim raids (`SIM_MEMORY_FILE`): the valley's memory of the
/// raider (`robots::memory`) is read from the file before the raid and,
/// with this raid folded in, written back once it ends. Without the file
/// each raid starts from a blank memory, as `zone_memory::MEMORY` at 0
/// would leave it.
fn career_memory(
    log: Res<telemetry::SimLog>,
    raid: Res<crate::raid::Raid>,
    zone: Res<Zone>,
    mut mind: ResMut<crate::robots::memory::ZoneMind>,
    bots: Query<&raider::Raider>,
    mut state: Local<(bool, bool)>,
) {
    let Some(path) = std::env::var_os("SIM_MEMORY_FILE").map(std::path::PathBuf::from) else {
        return;
    };
    let (loaded, written) = &mut *state;
    if !*loaded {
        *loaded = true;
        if let Some(m) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| ron::from_str::<extraction_core::zone_memory::ZoneMemory>(&t).ok())
        {
            mind.memory = m;
            mind.memory.sanitize();
        }
        return;
    }
    if !log.finished || *written {
        return;
    }
    *written = true;
    // Out (or down, or out of time) before the raid's own turn folded it.
    if !mind.trace.is_empty() {
        if matches!(raid.phase, crate::raid::Phase::Extracting { .. }) && mind.trace.rv.is_none() {
            mind.trace.rv = bots
                .iter()
                .find_map(|r| r.rv)
                .and_then(|i| zone.map.markers_of(&MarkerKind::Rendezvous).nth(i))
                .map(|m| m.at);
        }
        let trace = std::mem::take(&mut mind.trace);
        mind.memory.fold(&trace);
    }
    match ron::ser::to_string_pretty(&mind.memory, ron::ser::PrettyConfig::new()) {
        Ok(text) => {
            let _ = std::fs::write(&path, text);
        }
        Err(e) => warn!("sim: memory not written: {e}"),
    }
}

/// What a fresh game keeps in the truck's stash and the dev cache lacks:
/// the radio and two dressings (and a skilled raider's bolts,
/// `harvest::kit`), laid out by the insertion point.
fn spawn_sim_kit(
    mut commands: Commands,
    zone: Res<Zone>,
    catalog: Res<crate::items::Catalog>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    let Some(ins) = zone.marker(&MarkerKind::Insertion) else {
        return;
    };
    let mut kit = vec!["radio", "field_dressing", "field_dressing"];
    kit.extend(raider::contract_kit());
    // Bound for Rime Fell, a career has the hearth "Up the fell" gave it:
    // the way into the frost (`progression::contracts`).
    if opts.map == "rime_fell" && !kit.contains(&"art_hearth") {
        kit.push("art_hearth");
    }
    kit.extend(harvest::kit(
        opts.personality.as_deref().unwrap_or("looter"),
    ));
    for (i, id) in kit.iter().enumerate() {
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

    /// A sim replays from its seed: two raids on one seed write the same
    /// telemetry, event for event. (On many threads they parted within
    /// half a minute; see `crate::single_threaded`.)
    #[test]
    fn a_seed_plays_the_same_raid_twice() {
        let run = |n: u32| {
            let dir =
                std::env::temp_dir().join(format!("extraction-replay-{}-{n}", std::process::id()));
            let opts = LaunchOptions {
                sim_run: Some("replay".into()),
                sim_out: Some(dir.clone()),
                map: "sallow_reservoir".into(),
                seed: 1,
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            for _ in 0..90 * 60 {
                app.update();
            }
            app.world_mut().resource_mut::<telemetry::SimLog>().flush();
            let text = std::fs::read_to_string(dir.join("replay.jsonl")).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            text
        };
        let (a, b) = (run(0), run(1));
        assert!(a.lines().count() > 40, "the raid got going:\n{a}");
        if let Some((i, (x, y))) = a
            .lines()
            .zip(b.lines())
            .enumerate()
            .find(|(_, (x, y))| x != y)
        {
            panic!("the two runs part at event {i}:\n{x}\n{y}");
        }
        assert_eq!(a.lines().count(), b.lines().count());
    }

    /// Damage the telemetry can't place isn't called a fall: a wet aegis in
    /// the pack shocking its carrier is named as such, and nothing that
    /// happens on level ground reads "fall" (it once did, and a raider
    /// killed by its own artifact went down in the report as a bad landing).
    #[test]
    fn telemetry_names_an_aegis_shock_and_never_guesses_a_fall() {
        use crate::interact::body::{Backpack, InBackpack};
        use avian3d::prelude::{ColliderDisabled, RigidBodyDisabled};
        let dir = std::env::temp_dir().join(format!("extraction-aegis-{}", std::process::id()));
        let opts = LaunchOptions {
            sim_run: Some("aegis".into()),
            sim_out: Some(dir.clone()),
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .next()
            .unwrap();
        let pack = app
            .world_mut()
            .query::<(Entity, &Backpack)>()
            .iter(app.world())
            .find(|(_, b)| b.owner == player && b.carried)
            .map(|(e, _)| e)
            .unwrap();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let knot = crate::items::spawn_item(
            &mut Commands::new(&mut queue, app.world()),
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("art_aegis").unwrap(),
                transform: Transform::default(),
                condition: 1.0,
            },
        );
        queue.apply(app.world_mut());
        app.world_mut()
            .get_mut::<Backpack>(pack)
            .unwrap()
            .contents
            .push(knot);
        app.world_mut().entity_mut(knot).insert((
            InBackpack { bag: pack },
            ChildOf(pack),
            RigidBodyDisabled,
            ColliderDisabled,
        ));
        for _ in 0..90 * 40 {
            if let Some(mut w) = app
                .world_mut()
                .get_resource_mut::<crate::world::weather::Wetness>()
            {
                w.level = 1.0;
            }
            app.update();
        }
        let _ = std::fs::remove_dir_all(&dir);
        let damage = &app.world().resource::<telemetry::SimLog>().stats.damage;
        assert!(
            damage.get("aegis").is_some_and(|d| *d > 0.0),
            "the shocks are the aegis's: {damage:?}"
        );
        assert!(
            !damage.contains_key("fall") && !damage.contains_key("other"),
            "nothing else hurt the bot: {damage:?}"
        );
    }
}

/// `SIM_ARMOUR=late`: a bot on a job of the third tier or later (the late
/// game) goes out in a plate carrier with a front plate, and a helmet, as
/// a player that far up the ladder would. Seated straight into the body
/// slots at the start, like the sim's kit cache; once.
fn armour_up(world: &mut World) {
    use crate::interact::body::BodySlot;
    use crate::interact::{InSocket, Socket, SocketKind, SocketOwner};
    use extraction_core::inventory::SlotKind;
    if std::env::var("SIM_ARMOUR").as_deref() != Ok("late") {
        return;
    }
    let Some(tier) = world
        .get_resource::<crate::progression::Progression>()
        .filter(|p| p.0.job.is_some())
        .map(|p| p.0.tier)
    else {
        return;
    };
    let Some(player) = world
        .query_filtered::<Entity, (With<crate::rig::LocalPlayer>, With<raider::Raider>)>()
        .iter(world)
        .next()
    else {
        return;
    };
    if world.get::<ArmouredUp>(player).is_some() {
        return;
    }
    world.entity_mut(player).insert(ArmouredUp);
    if tier < 2 {
        return;
    }
    let catalog = world.resource::<crate::items::Catalog>().clone();
    let slot = |world: &mut World, kind: SlotKind| -> Option<Entity> {
        world
            .query::<(Entity, &Socket, &SocketOwner, &BodySlot)>()
            .iter(world)
            .find(|(_, s, o, _)| {
                o.0 == player && s.kind == SocketKind::Body(kind) && s.occupant.is_none()
            })
            .map(|(e, ..)| e)
    };
    // Spawned already worn (in the socket, its body off), so physics never
    // sees it loose.
    let wear = |world: &mut World, id: &str, socket: Entity| -> Option<Entity> {
        let kind = catalog.kind(id)?;
        let seat = world.get::<Socket>(socket)?.seat;
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let e = {
            let mut c = Commands::new(&mut queue, world);
            let e = crate::items::spawn_item(
                &mut c,
                &catalog,
                crate::items::SpawnItem {
                    kind,
                    transform: Transform::default(),
                    condition: 1.0,
                },
            );
            c.entity(e).insert((
                InSocket { socket },
                avian3d::prelude::RigidBodyDisabled,
                avian3d::prelude::ColliderDisabled,
                ChildOf(socket),
                seat,
            ));
            e
        };
        queue.apply(world);
        world.flush();
        world.get_mut::<Socket>(socket)?.occupant = Some(e);
        Some(e)
    };
    if let Some(s) = slot(world, SlotKind::Vest) {
        if let Some(vest) = wear(world, "vest", s) {
            // Its pockets come with it; the plate goes in the front one.
            let pocket = world
                .query::<(
                    Entity,
                    &Socket,
                    &SocketOwner,
                    &crate::weapons::armour::PlatePocket,
                )>()
                .iter(world)
                .find(|(_, _, o, p)| {
                    o.0 == vest && p.0 == extraction_core::firearm_terminal::ArmourArea::Front
                })
                .map(|(e, ..)| e);
            if let Some(pocket) = pocket {
                wear(world, "steel_plate", pocket);
            }
        }
    }
    if let Some(s) = slot(world, SlotKind::Head) {
        wear(world, "helmet", s);
    }
    info!("sim: armoured up for a tier-{tier} job");
}

/// [`armour_up`] has looked at this bot.
#[derive(Component)]
struct ArmouredUp;
