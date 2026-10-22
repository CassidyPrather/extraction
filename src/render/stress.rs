//! The worst minute of a raid, not the average: in a headset one dropped
//! frame in a firefight is what's remembered. Each valley's stress scene
//! is a dusk downpour by a building, three machines on the player (two
//! walkers and a drone), the player firing in bursts (muzzle flash, a hot
//! barrel's smoke), a grenade's blast and smoke every few seconds, a door
//! breached in the building's wall, and a dirigible overhead.
//!
//! Measured each frame: the CPU (wall time, and against a reference
//! workload timed beside it, so a slower machine isn't a regression), and
//! every few frames both eyes' load at the headset's size (draws, shadow
//! draws, shaded fragments: `vr_budget`'s stereo budget, which is what the
//! GPU's 11.1 ms comes to at the default render scale). The worst frames'
//! systems are named with `STRESS_PROFILE=1` and
//! `--features "bevy/trace bevy/debug"`. See docs/PERFORMANCE.md, "The
//! worst minute".

#![cfg(test)]

use bevy::prelude::*;
use std::time::Instant;

use super::budget::tests::{SystemTimes, as_the_game_runs, client_threads, heavy, settle};
use crate::perf::fight::{arm, keep_alive, player, reference_ms};

/// Frames measured: 10 s at 90 Hz (`STRESS_FRAMES`; a minute is 5400).
fn frames() -> usize {
    std::env::var("STRESS_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
}

/// The CPU's p99 frame, in reference units (the busy moment's own budget
/// for its mean, `budget::CPU_FRAME_UNITS`): the worst minute's slow frames
/// within what an average busy frame may take.
const P99_UNITS: f64 = super::budget::CPU_FRAME_UNITS;

/// Shots fired in the scene (the player's and the machines').
#[derive(Resource, Default)]
struct Shots(usize);

fn count_shots(mut shots: MessageReader<crate::weapons::ballistics::Shot>, mut n: ResMut<Shots>) {
    n.0 += shots.read().count();
}

static PROFILE: std::sync::OnceLock<SystemTimes> = std::sync::OnceLock::new();

fn profile() {
    if std::env::var_os("STRESS_PROFILE").is_some() && PROFILE.get().is_none() {
        use bevy::log::tracing_subscriber::layer::SubscriberExt;
        let times = PROFILE.get_or_init(Default::default).clone();
        let subscriber = bevy::log::tracing_subscriber::registry().with(times);
        let _ = bevy::log::tracing::subscriber::set_global_default(subscriber);
    }
}

fn systems() -> std::collections::HashMap<String, f64> {
    PROFILE
        .get()
        .map(|p| std::mem::take(&mut *p.0.lock().unwrap()))
        .unwrap_or_default()
}

/// The building the scene is staged at: the one with an inside nearest the
/// map's middle. Its front wall's middle (a metre up) and the way out.
fn building(app: &App) -> (Vec3, Vec3) {
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let p = zone
        .map
        .placements
        .iter()
        .filter(|p| crate::tour::has_inside(&p.prefab))
        .min_by(|a, b| {
            let d = |p: &extraction_core::map::Placement| p.at[0].hypot(p.at[1]);
            d(a).total_cmp(&d(b))
        })
        .expect("a building to go into");
    let f = crate::world::structures::footprint(&p.prefab);
    let rot = Quat::from_rotation_y(-p.rot.to_radians());
    let out = rot * Vec3::NEG_Z;
    let ground = zone.ground(p.at[0], p.at[1]);
    let wall = Vec3::new(p.at[0], ground + 1.0, p.at[1]) + out * (f.z * 0.5);
    (wall, out)
}

/// A valley's stress scene, staged: the player 10 m out from a building's
/// front wall, facing it, guns in hand, three machines closing, a
/// dirigible over the building.
fn stress_scene(map: &str) -> App {
    profile();
    let mut app = client_threads(map, "downpour", None);
    app.world_mut()
        .resource_mut::<crate::launch::LaunchOptions>()
        .hour = Some("dusk".into());
    as_the_game_runs(&mut app);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(1.0 / 90.0),
    ))
    .init_resource::<Shots>()
    .init_resource::<Trigger>()
    .add_systems(Last, count_shots)
    // In the fixed step, before the guns read it: the flat driver's mouse
    // buttons write it every frame too.
    .add_systems(FixedPreUpdate, pull);
    settle(&mut app);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let (wall, out) = building(&app);
    let stand = zone.on_ground(wall.x + out.x * 10.0, wall.z + out.z * 10.0);
    {
        let w = app.world_mut();
        let p = w
            .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
            .single(w)
            .unwrap();
        w.get_mut::<Transform>(p).unwrap().translation = stand;
        let mut q = w.query_filtered::<&mut Transform, With<super::FreeCamera>>();
        *q.single_mut(w).unwrap() =
            Transform::from_translation(stand + Vec3::Y * 1.7).looking_at(wall, Vec3::Y);
    }
    for _ in 0..30 {
        app.update();
    }
    arm(&mut app);
    // Three machines: two walkers and a drone, 25-35 m off, after the
    // player.
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut machines = Vec::new();
    {
        let mut c = Commands::new(&mut queue, app.world());
        for (i, a) in [0.6f32, 2.4, 4.2].into_iter().enumerate() {
            let spot = zone.on_ground(stand.x + a.cos() * 30.0, stand.z + a.sin() * 30.0);
            let job = crate::robots::Job {
                kind: crate::robots::JobKind::Response,
                site: stand,
                duration: 999.0,
                exit: spot,
            };
            machines.push(if i == 2 {
                crate::robots::drone::spawn_drone(&mut c, spot + Vec3::Y * 15.0, job)
            } else {
                crate::robots::walker::spawn_walker(&mut c, &zone, spot, job)
            });
        }
        crate::robots::dirigible::spawn_dirigible(
            &mut c,
            wall + Vec3::Y * 40.0,
            crate::robots::Job {
                kind: crate::robots::JobKind::Watch,
                site: wall,
                duration: 999.0,
                exit: wall + Vec3::X * 300.0,
            },
        );
    }
    queue.apply(app.world_mut());
    app.update();
    for m in &machines {
        if let Some(mut b) = app.world_mut().get_mut::<crate::robots::Brain>(*m) {
            *b = crate::robots::Brain::Investigate {
                at: stand,
                left: 999.0,
            };
        }
    }
    // A second of the fight before it's measured.
    for _ in 0..90 {
        keep_alive(&mut app);
        app.update();
    }
    app
}

/// The player's trigger this frame (set at the fixed step's start, after
/// the flat driver's mouse has written it).
#[derive(Resource, Default)]
struct Trigger(bool);

fn pull(
    trigger: Res<Trigger>,
    mut input: Query<&mut crate::rig::RigInput, With<crate::rig::LocalPlayer>>,
) {
    if let Ok(mut i) = input.single_mut() {
        i.hand_mut(crate::rig::Side::Right).trigger = if trigger.0 { 1.0 } else { 0.0 };
    }
}

/// The player's guns loaded: a full magazine, a round in, on auto (and
/// topped up each burst, so the whole minute is fired).
fn load(app: &mut App) {
    use extraction_core::firearm::{FireMode, Magazine};
    let w = app.world_mut();
    let mut q = w.query_filtered::<&mut crate::weapons::Gun, With<crate::interact::Held>>();
    for mut g in q.iter_mut(w) {
        let f = &mut g.firearm;
        let Some(family) = f.def.magazine_family.clone() else {
            continue;
        };
        if f.rounds_in_feed() < 5 {
            let _ = f.remove_magazine();
            let _ = f.insert_magazine(Magazine {
                family,
                capacity: 60,
                rounds: 60,
                ..Default::default()
            });
            f.set_bolt_travel(1.0);
            f.release_bolt();
        }
        let auto = f.def.modes.contains(&FireMode::Auto);
        let want = if auto { FireMode::Auto } else { FireMode::Semi };
        for _ in 0..3 {
            if f.fire_mode() == want {
                break;
            }
            f.cycle_mode();
        }
        f.condition = 1.0;
    }
}

/// Bursts: 0.4 s of every second.
fn trigger(app: &mut App, frame: usize) {
    if frame % 90 == 0 {
        load(app);
    }
    app.world_mut().resource_mut::<Trigger>().0 = frame % 90 < 36;
}

/// A grenade at the player's feet-ish, 6 m toward the building, going off
/// now: its blast, its fragments and its smoke.
fn blast(app: &mut App, wall: Vec3) {
    let (_, at) = player(app);
    let to = (wall - at).with_y(0.0).normalize_or_zero();
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let spot = zone.on_ground(at.x + to.x * 6.0, at.z + to.z * 6.0) + Vec3::Y * 0.3;
    let catalog = app.world().resource::<crate::items::Catalog>().0.clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let g = {
        let mut c = Commands::new(&mut queue, app.world());
        crate::items::spawn_item(
            &mut c,
            &catalog,
            crate::items::SpawnItem {
                kind: catalog.kind("rgd5").expect("the RGD-5"),
                transform: Transform::from_translation(spot),
                condition: 1.0,
            },
        )
    };
    queue.apply(app.world_mut());
    app.update();
    if let Some(mut live) = app.world_mut().get_mut::<crate::weapons::grenades::Live>(g) {
        live.grenade.fuse = extraction_core::grenade::Fuse::Burning(1e-4);
    }
}

/// The middle of the building's `n`th wall (front, right, back, left), a
/// metre up, and the way out through it.
fn side(app: &App, n: usize) -> (Vec3, Vec3) {
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let p = zone
        .map
        .placements
        .iter()
        .filter(|p| crate::tour::has_inside(&p.prefab))
        .min_by(|a, b| a.at[0].hypot(a.at[1]).total_cmp(&b.at[0].hypot(b.at[1])))
        .expect("a building");
    let f = crate::world::structures::footprint(&p.prefab);
    let rot = Quat::from_rotation_y(-p.rot.to_radians());
    let (dir, half) = match n % 4 {
        0 => (Vec3::NEG_Z, f.z * 0.5),
        1 => (Vec3::X, f.x * 0.5),
        2 => (Vec3::Z, f.z * 0.5),
        _ => (Vec3::NEG_X, f.x * 0.5),
    };
    let out = rot * dir;
    let ground = zone.ground(p.at[0], p.at[1]);
    (Vec3::new(p.at[0], ground + 1.0, p.at[1]) + out * half, out)
}

pub(crate) struct Stress {
    /// CPU ms and reference units, each frame.
    pub ms: Vec<f64>,
    pub units: Vec<f64>,
    /// Both eyes' loads, sampled: (draws, shadow draws, fragments M).
    pub eyes: Vec<(usize, usize, f32)>,
    /// The worst frames' slowest systems (with the profile).
    pub worst: Vec<(f64, Vec<(String, f64)>)>,
    pub holes: usize,
}

/// A valley's worst minute (or `STRESS_FRAMES`).
pub(crate) fn stress(map: &str) -> Stress {
    let mut app = stress_scene(map);
    let (wall, _) = building(&app);
    let eyes = super::vr_budget::both_eyes(&mut app);
    super::vr_budget::eye_projections(&mut app);
    let px = super::vr_budget::eye_px();
    let n = frames();
    let (mut ms, mut units, mut loads) = (Vec::new(), Vec::new(), Vec::new());
    let mut worst: Vec<(f64, Vec<(String, f64)>)> = Vec::new();
    let mut fragments = 0.0f32;
    for f in 0..n {
        keep_alive(&mut app);
        trigger(&mut app, f);
        // A blast every 4 s; the wall breached at 5 s.
        if f % 360 == 120 {
            blast(&mut app, wall);
        }
        // The breach: at the front wall's middle, then (if no hole came)
        // each other side's, a few frames apart.
        if (450..450 + 4 * 10).contains(&f) && f % 10 == 0 {
            let holes = app
                .world()
                .resource::<crate::destruction::breach::Breaches>()
                .0
                .len();
            if holes == 0 {
                let (at, way) = side(&app, (f - 450) / 10);
                app.world_mut()
                    .write_message(crate::destruction::hooks::Breach { at, out: way });
            }
        }
        systems();
        let t = Instant::now();
        app.update();
        let frame = t.elapsed().as_secs_f64() * 1000.0;
        let reference = reference_ms();
        ms.push(frame);
        units.push(frame / reference);
        let named = systems();
        if !named.is_empty() {
            let mut v: Vec<(String, f64)> = named
                .into_iter()
                .filter(|(k, _)| {
                    !k.contains("run_main")
                        && !k.contains("run_fixed_main")
                        && !k.contains("run_physics_schedule")
                        && !k.contains("run_substep_schedule")
                })
                .map(|(k, v)| (k.rsplit("::").next().unwrap_or(&k).to_string(), v))
                .collect();
            v.sort_by(|a, b| b.1.total_cmp(&a.1));
            v.truncate(3);
            worst.push((frame, v));
            worst.sort_by(|a, b| b.0.total_cmp(&a.0));
            worst.truncate(5);
        }
        // Both eyes' load every third of a second; their fragments (the
        // costlier count) every second, held between.
        if f % 30 == 0 {
            let (l, r, _) = super::vr_budget::measure(&mut app, eyes);
            if f % 90 == 0 || fragments == 0.0 {
                fragments = [eyes.0, eyes.1]
                    .into_iter()
                    .map(|e| super::overdraw::measure(app.world_mut(), e).mean * px * px / 1e6)
                    .sum();
            }
            loads.push((
                l.draws + r.draws,
                l.shadow_draws + r.shadow_draws,
                fragments,
            ));
        }
    }
    let holes = app
        .world()
        .resource::<crate::destruction::breach::Breaches>()
        .0
        .len();
    {
        let w = app.world_mut();
        let robots = w
            .query_filtered::<(), With<crate::robots::Robot>>()
            .iter(w)
            .count();
        let lights = w.query::<&PointLight>().iter(w).count();
        let shadowed = w
            .query::<&PointLight>()
            .iter(w)
            .filter(|l| l.shadow_maps_enabled)
            .count();
        let sun = w
            .query::<&DirectionalLight>()
            .iter(w)
            .map(|l| (l.illuminance, l.shadow_maps_enabled))
            .collect::<Vec<_>>();
        let shots = w.resource::<Shots>().0;
        for g in w
            .query_filtered::<&crate::weapons::Gun, With<crate::interact::Held>>()
            .iter(w)
        {
            println!(
                "    held {:?}: trigger {:.1}, {:?}, {} in the feed, chamber {:?}",
                g.firearm.def.magazine_family,
                g.trigger,
                g.firearm.fire_mode(),
                g.firearm.rounds_in_feed(),
                g.firearm.chamber_load(0)
            );
        }
        println!(
            "    scene: {robots} machines, {shots} shots, {lights} point lights ({shadowed} shadowed), sun {sun:?}"
        );
    }
    Stress {
        ms,
        units,
        eyes: loads,
        worst,
        holes,
    }
}

fn pct(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() as f64 * q) as usize).min(s.len() - 1)]
}

/// `STRESS_MAP=<map>` for one valley; every valley otherwise.
fn maps() -> Vec<String> {
    std::env::var("STRESS_MAP").map_or_else(
        |_| {
            extraction_core::map::VALLEYS
                .iter()
                .map(|v| v.0.to_string())
                .collect()
        },
        |m| vec![m],
    )
}

/// Every valley's worst minute holds: the CPU's p99 within the busy
/// frame's budget (reference units), and both eyes' p99 load within the
/// stereo budget at the default render scale.
#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_every_valley_holds_its_worst_minute() {
    let _one_at_a_time = heavy();
    let budget = super::vr_budget::budget();
    let mut wrong = Vec::new();
    for map in maps() {
        let t = Instant::now();
        let s = stress(&map);
        let (p99, worst) = (pct(&s.ms, 0.99), pct(&s.ms, 1.0));
        let (u99, uworst) = (pct(&s.units, 0.99), pct(&s.units, 1.0));
        let d99 = pct(&s.eyes.iter().map(|e| e.0 as f64).collect::<Vec<_>>(), 0.99);
        let sd99 = pct(&s.eyes.iter().map(|e| e.1 as f64).collect::<Vec<_>>(), 0.99);
        let f99 = pct(
            &s.eyes.iter().map(|e| f64::from(e.2)).collect::<Vec<_>>(),
            0.99,
        );
        println!(
            "{map}: {} frames; CPU p99 {p99:.1} ms ({u99:.0} units of {P99_UNITS:.0}), worst {worst:.1} ms ({uworst:.0}); \
             both eyes p99: {d99:.0} draws of {}, {sd99:.0} shadow draws of {}, {f99:.1} M fragments of {:.1}; \
             {} holes breached ({:.0} s)",
            s.ms.len(),
            budget.draws,
            budget.shadow_draws,
            budget.fragments_m,
            s.holes,
            t.elapsed().as_secs_f64()
        );
        for (ms, top) in &s.worst {
            println!(
                "    worst {ms:.1} ms: {}",
                top.iter()
                    .map(|(k, v)| format!("{k} {v:.1}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if u99 > P99_UNITS {
            wrong.push(format!("{map}: CPU p99 {u99:.0} units > {P99_UNITS:.0}"));
        }
        if d99 > budget.draws as f64 {
            wrong.push(format!(
                "{map}: {d99:.0} draws a frame (p99) > {}",
                budget.draws
            ));
        }
        if sd99 > budget.shadow_draws as f64 {
            wrong.push(format!(
                "{map}: {sd99:.0} shadow draws (p99) > {}",
                budget.shadow_draws
            ));
        }
        if f99 > f64::from(budget.fragments_m) {
            wrong.push(format!(
                "{map}: {f99:.1} M fragments (p99) > {:.1}",
                budget.fragments_m
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "worst minutes over budget:\n{}",
        wrong.join("\n")
    );
}
