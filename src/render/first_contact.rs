//! First contact: the hitches a player meets only the first time (a model,
//! a material, an effect, a sound or a building's inside seen for the
//! first time, the first fight). The probe walks each valley's tour (every
//! placement from outside and in, signs, the open ground), then a fight,
//! twice over: the second pass is the steady state, and every first-pass
//! frame well over its second-pass self is a first contact, logged with
//! what came in around it (assets loaded, things spawned, and with the
//! profile the systems that took the time). Pipelines are the GPU's: the
//! warm-up shelf and `timeline::WorldReady` have those
//! (`render::first_sight`). See docs/PERFORMANCE.md, "First contact".

#![cfg(test)]

use bevy::prelude::*;
use std::time::Instant;

use super::budget::tests::{as_the_game_runs, client_threads, heavy, settle};

/// Frames at each stop (the tour's own settle).
const PER_VIEW: usize = 6;
/// A first-pass frame this much over its second-pass self is a first
/// contact (and the gauge's ceiling, worst against worst).
const CEILING: f64 = 1.2;
/// Less than this over isn't worth a name (the harness's own jitter).
const SLACK_MS: f64 = 3.0;

/// What arrived in a frame: assets finished loading (by path), and how
/// many things were spawned.
#[derive(Resource, Default)]
struct Arrivals {
    loads: Vec<String>,
    spawned: usize,
}

fn note_loads<A: Asset>(
    mut events: MessageReader<AssetEvent<A>>,
    server: Res<AssetServer>,
    mut arrivals: ResMut<Arrivals>,
) {
    for ev in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = ev {
            if let Some(p) = server.get_path(*id) {
                arrivals.loads.push(p.to_string());
            }
        }
    }
}

fn note_spawns(added: Query<(), Added<Transform>>, mut arrivals: ResMut<Arrivals>) {
    arrivals.spawned += added.iter().count();
}

pub(crate) struct Frame {
    pub ms: f64,
    pub stop: usize,
    pub loads: Vec<String>,
    pub spawned: usize,
    /// The frame's slowest systems, with the profile (`FIRST_PROFILE=1`
    /// and `--features bevy/trace`).
    pub systems: Vec<(String, f64)>,
}

/// Per-system times, when built with Bevy's `trace` feature.
static PROFILE: std::sync::OnceLock<super::budget::tests::SystemTimes> = std::sync::OnceLock::new();

fn profile() {
    if std::env::var_os("FIRST_PROFILE").is_some() && PROFILE.get().is_none() {
        use bevy::log::tracing_subscriber::layer::SubscriberExt;
        let times = PROFILE.get_or_init(Default::default).clone();
        let subscriber = bevy::log::tracing_subscriber::registry().with(times);
        let _ = bevy::log::tracing::subscriber::set_global_default(subscriber);
    }
}

/// This frame's systems, slowest first (the schedule runners left out).
fn systems() -> Vec<(String, f64)> {
    let Some(p) = PROFILE.get() else {
        return Vec::new();
    };
    let mut v: Vec<(String, f64)> = std::mem::take(&mut *p.0.lock().unwrap())
        .into_iter()
        .filter(|(k, _)| {
            !k.contains("run_main")
                && !k.contains("run_fixed_main")
                && !k.contains("run_physics_schedule")
                && !k.contains("run_substep_schedule")
        })
        .map(|(k, ms)| (k.rsplit("::").next().unwrap_or(&k).to_string(), ms))
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v.truncate(4);
    v
}

/// A valley's client, settled, with what arrives each frame noted.
fn probe_app(map: &str) -> App {
    profile();
    let mut app = client_threads(map, "clearing", None);
    as_the_game_runs(&mut app);
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f64(1.0 / 90.0),
    ))
    .init_resource::<Arrivals>()
    .add_systems(
        Last,
        (
            note_loads::<Image>,
            note_loads::<Mesh>,
            note_loads::<bevy::gltf::Gltf>,
            note_spawns,
        ),
    );
    settle(&mut app);
    // What's settling keeps arriving a while: until a second's quiet.
    let mut quiet = 0;
    for _ in 0..3000 {
        app.update();
        let a = std::mem::take(&mut *app.world_mut().resource_mut::<Arrivals>());
        quiet = if a.loads.is_empty() { quiet + 1 } else { 0 };
        if quiet >= 90 {
            break;
        }
    }
    app
}

/// Puts the player (and the free camera) at a tour stop.
fn stand(app: &mut App, view: &crate::tour::View) {
    let at = Vec3::from(view.at);
    let look = Vec3::from(view.look);
    let w = app.world_mut();
    let zone = w.resource::<crate::world::Zone>().clone();
    let feet = zone.on_ground(at.x, at.z);
    if let Ok(player) = w
        .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
        .single(w)
    {
        if let Some(mut t) = w.get_mut::<Transform>(player) {
            t.translation = feet;
        }
    }
    let mut q = w.query_filtered::<&mut Transform, With<super::FreeCamera>>();
    if let Ok(mut t) = q.single_mut(w) {
        *t = Transform::from_translation(at).looking_at(look, Vec3::Y);
    }
}

/// One pass: every stop, `PER_VIEW` frames each, then a fight.
fn pass(app: &mut App, views: &[crate::tour::View]) -> Vec<Frame> {
    let mut out = Vec::new();
    let frame = |app: &mut App, stop: usize, out: &mut Vec<Frame>| {
        systems();
        let t = Instant::now();
        app.update();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let a = std::mem::take(&mut *app.world_mut().resource_mut::<Arrivals>());
        out.push(Frame {
            ms,
            stop,
            loads: a.loads,
            spawned: a.spawned,
            systems: systems(),
        });
    };
    for (i, v) in views.iter().enumerate() {
        stand(app, v);
        for _ in 0..PER_VIEW {
            frame(app, i, &mut out);
        }
    }
    // The fight: guns in hand, machines on the player (`perf::fight`).
    let fight = views.len();
    crate::perf::fight::arm(app);
    crate::perf::fight::hunt(app);
    for _ in 0..180 {
        crate::perf::fight::keep_alive(app);
        frame(app, fight, &mut out);
    }
    out
}

/// The worst frame of each stop.
fn worst_by_stop(frames: &[Frame], stops: usize) -> Vec<f64> {
    let mut w = vec![0.0f64; stops + 1];
    for f in frames {
        w[f.stop] = w[f.stop].max(f.ms);
    }
    w
}

pub(crate) struct Contact {
    pub what: String,
    pub first: f64,
    pub steady: f64,
    pub why: String,
}

/// A valley walked twice: (first-pass worst, steady worst, the first
/// contacts, the stops).
pub(crate) fn walk(map: &str) -> (f64, f64, Vec<Contact>, usize) {
    let mut app = probe_app(map);
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let views = crate::tour::views(&zone);
    let first = pass(&mut app, &views);
    let steady = pass(&mut app, &views);
    let (wf, ws) = (
        worst_by_stop(&first, views.len()),
        worst_by_stop(&steady, views.len()),
    );
    let name = |i: usize| {
        views
            .get(i)
            .map_or("the fight".to_string(), |v| v.file.clone())
    };
    let mut contacts = Vec::new();
    for (i, (f, s)) in wf.iter().zip(&ws).enumerate() {
        if *f > s * CEILING && f - s > SLACK_MS {
            // What came in round the stop's worst frame.
            let at = first
                .iter()
                .enumerate()
                .filter(|(_, fr)| fr.stop == i)
                .max_by(|a, b| a.1.ms.total_cmp(&b.1.ms))
                .map_or(0, |(k, _)| k);
            let near = &first[at.saturating_sub(2)..(at + 1).min(first.len())];
            let loads: Vec<&str> = near
                .iter()
                .flat_map(|fr| fr.loads.iter().map(String::as_str))
                .collect();
            let spawned: usize = near.iter().map(|fr| fr.spawned).sum();
            let named = first[at]
                .systems
                .iter()
                .map(|(k, ms)| format!("{k} {ms:.1}"))
                .collect::<Vec<_>>()
                .join(", ");
            let why = if !loads.is_empty() {
                format!(
                    "asset load: {}{}",
                    loads.iter().take(4).copied().collect::<Vec<_>>().join(", "),
                    if loads.len() > 4 {
                        format!(" (+{})", loads.len() - 4)
                    } else {
                        String::new()
                    }
                )
            } else if spawned > 50 {
                format!("first-run system ({spawned} things spawned) {named}")
            } else {
                format!("first-run system {named}")
            };
            contacts.push(Contact {
                what: name(i),
                first: *f,
                steady: *s,
                why,
            });
        }
    }
    let f = wf.iter().copied().fold(0.0, f64::max);
    let s = ws.iter().copied().fold(0.0, f64::max);
    (f, s, contacts, views.len())
}

fn report(map: &str, f: f64, s: f64, contacts: &[Contact], stops: usize) {
    println!(
        "{map}: {stops} stops and a fight; first pass worst {f:.1} ms, steady worst {s:.1} ms ({:.2}x)",
        f / s
    );
    for c in contacts {
        println!(
            "    {:<34} {:>6.1} ms first, {:>6.1} steady: {}",
            c.what, c.first, c.steady, c.why
        );
    }
}

/// `FIRST_MAP=<map>` for one valley; every valley otherwise.
fn maps() -> Vec<String> {
    std::env::var("FIRST_MAP").map_or_else(
        |_| {
            extraction_core::map::VALLEYS
                .iter()
                .map(|v| v.0.to_string())
                .collect()
        },
        |m| vec![m],
    )
}

/// Every valley's first pass is within 1.2x of its steady state, worst
/// frame against worst frame, and the first contacts are named. The
/// harness's frames jitter by more than a small hitch, so each valley is
/// walked `FIRST_RUNS` (3) times, each in a new game: a stop is a first
/// contact only if it is one every time, and a valley fails only if it's
/// over every time.
#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_no_first_sight_stutter() {
    let _one_at_a_time = heavy();
    let runs: usize = std::env::var("FIRST_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let mut over = Vec::new();
    for map in maps() {
        let mut every: Option<Vec<Contact>> = None;
        let mut over_every = true;
        let mut worst = (0.0, 0.0);
        for run in 1..=runs {
            let (f, s, contacts, stops) = walk(&map);
            println!("run {run}:");
            report(&map, f, s, &contacts, stops);
            let is_over = f > s * CEILING && f - s > SLACK_MS;
            over_every &= is_over;
            if is_over {
                worst = (f, s);
            }
            every = Some(match every {
                None => contacts,
                Some(before) => contacts
                    .into_iter()
                    .filter(|c| before.iter().any(|b| b.what == c.what))
                    .collect(),
            });
            if !is_over && every.as_ref().is_some_and(|e| e.is_empty()) {
                over_every = false;
                break;
            }
        }
        let every = every.unwrap_or_default();
        if !every.is_empty() {
            println!("{map}: first contacts in every run:");
            for c in &every {
                println!(
                    "    {:<34} {:.1} ms first, {:.1} steady: {}",
                    c.what, c.first, c.steady, c.why
                );
            }
        }
        if over_every {
            over.push(format!(
                "{map}: {:.1} ms first against {:.1} steady, every run",
                worst.0, worst.1
            ));
        }
    }
    assert!(over.is_empty(), "first-sight stutter:\n{}", over.join("\n"));
}
