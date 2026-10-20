//! Hitches: a frame that runs long once. In a headset a single long
//! frame is a judder the player feels, however good the average is. The
//! gauge here sets off the raid's heavy one-off events in the busy moment
//! (`budget::tests::busy_scene`, the same scene the CPU budget holds) and
//! times every frame after each: a grenade's fragments, robots wrecking
//! into debris, the Surge's blooms, the Depot's residents coming out, the
//! truck's homecoming, and a valley loading. Each event's worst frame is
//! held to a ceiling in reference units (`perf::fight::reference_ms`).
//! See docs/PERFORMANCE.md, "Hitches".

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use std::time::Instant;

    use super::super::budget::tests::{SystemTimes, busy_scene, client_threads, heavy};
    use crate::perf::fight::{keep_alive, player, reference_ms};
    use std::collections::HashMap;

    /// Frames timed after each event (a second at 90 Hz).
    const AFTER: usize = 90;

    /// Per-system times, when built with Bevy's `trace` feature: the
    /// worst frame of each event is named by system.
    static PROFILE: std::sync::OnceLock<SystemTimes> = std::sync::OnceLock::new();

    /// The systems' times this frame (empty without the profile).
    fn systems() -> HashMap<String, f64> {
        PROFILE
            .get()
            .map(|p| std::mem::take(&mut *p.0.lock().unwrap()))
            .unwrap_or_default()
    }

    /// The `n` slowest systems of a frame, leaving out the schedule
    /// runners that hold them.
    fn slowest(of: &HashMap<String, f64>, n: usize) -> String {
        let mut v: Vec<_> = of
            .iter()
            .filter(|(k, _)| {
                !k.contains("run_main")
                    && !k.contains("run_fixed_main")
                    && !k.contains("run_physics_schedule")
                    && !k.contains("run_substep_schedule")
            })
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(a.1));
        v.iter()
            .take(n)
            .map(|(k, ms)| {
                let short = k.rsplit("::").next().unwrap_or(k);
                format!("{short} {ms:.1}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// One frame, wall time in ms. The free camera rides the player's
    /// head, so the view follows them (home in the truck, say).
    fn frame(app: &mut App) -> f64 {
        keep_alive(app);
        let (_, at) = player(app);
        {
            let w = app.world_mut();
            let mut q = w.query_filtered::<&mut Transform, With<crate::render::FreeCamera>>();
            if let Ok(mut t) = q.single_mut(w) {
                t.translation = at + Vec3::Y * 1.7;
            }
        }
        systems();
        let t = Instant::now();
        app.update();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        WORST.with(|w| {
            let mut w = w.borrow_mut();
            if ms > w.0 {
                *w = (ms, systems());
            }
        });
        ms
    }

    thread_local! {
        /// The slowest frame since last taken, and its systems.
        static WORST: std::cell::RefCell<(f64, HashMap<String, f64>)> =
            std::cell::RefCell::new((0.0, HashMap::new()));
    }

    fn take_worst() -> HashMap<String, f64> {
        WORST.with(|w| std::mem::take(&mut *w.borrow_mut()).1)
    }

    /// p50, p99 and the worst of `v`.
    fn spread(v: &[f64]) -> (f64, f64, f64) {
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        let at = |q: f64| s[((s.len() as f64 * q) as usize).min(s.len() - 1)];
        (at(0.5), at(0.99), s[s.len() - 1])
    }

    /// What an event did, and its frames.
    pub(crate) struct Spike {
        pub name: String,
        pub did: String,
        pub frames: Vec<f64>,
        /// The worst frame's systems (with the profile).
        pub worst: HashMap<String, f64>,
    }

    fn count<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
        let w = app.world_mut();
        w.query_filtered::<(), F>().iter(w).count()
    }

    /// Where the player looks, on the ground `d` metres on.
    fn ahead(app: &mut App, d: f32) -> Vec3 {
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let w = app.world_mut();
        let mut q = w.query_filtered::<&Transform, With<crate::render::FreeCamera>>();
        let t = q.single(w).unwrap();
        let mut f = t.forward().as_vec3();
        f.y = 0.0;
        let p = t.translation + f.normalize_or(Vec3::NEG_Z) * d;
        zone.on_ground(p.x, p.z)
    }

    /// Runs `event` in the busy moment and times the frames after it
    /// (`event` gives what it did, and any frames it ran itself).
    fn time(
        app: &mut App,
        name: &str,
        event: impl FnOnce(&mut App) -> (String, Vec<f64>),
    ) -> Spike {
        take_worst();
        let (did, mut frames) = event(app);
        while frames.len() < AFTER {
            frames.push(frame(app));
        }
        Spike {
            name: name.into(),
            did,
            frames,
            worst: take_worst(),
        }
    }

    /// The events, in turn, in one busy moment on `map`.
    pub(crate) fn spikes(map: &str) -> (Vec<f64>, Vec<Spike>, f64) {
        use crate::weapons::ballistics::Projectile;
        use extraction_core::anomaly::AnomalyKind;

        let mut app = busy_scene(map);
        // A grenade lying ready, six metres on, so its own arrival
        // (model, collider) isn't the explosion's.
        let grenade = {
            let at = ahead(&mut app, 6.0) + Vec3::Y * 0.3;
            let catalog = app.world().resource::<crate::items::Catalog>().0.clone();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let g = {
                let mut c = Commands::new(&mut queue, app.world());
                crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind("rgd5").expect("the RGD-5"),
                        transform: Transform::from_translation(at),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            app.world_mut()
                .entity_mut(g)
                .insert(avian3d::prelude::RigidBody::Static);
            g
        };
        for _ in 0..30 {
            frame(&mut app);
        }
        let reference: Vec<f64> = (0..15).map(|_| reference_ms()).collect();
        let reference = spread(&reference).0;
        let base: Vec<f64> = (0..180).map(|_| frame(&mut app)).collect();
        let mut out = Vec::new();

        out.push(time(&mut app, "grenade", |app| {
            let before = count::<With<Projectile>>(app);
            app.world_mut()
                .get_mut::<crate::weapons::grenades::Live>(grenade)
                .expect("a live grenade")
                .grenade
                .fuse = extraction_core::grenade::Fuse::Burning(1e-4);
            let first = frame(app);
            let after = count::<With<Projectile>>(app);
            (
                format!("{} fragments", after.saturating_sub(before)),
                vec![first],
            )
        }));

        for (name, walker) in [("walker wreck", true), ("drone wreck", false)] {
            out.push(time(&mut app, name, |app| {
                let (_, at) = player(app);
                let w = app.world_mut();
                let mut q = w.query::<(
                    Entity,
                    &GlobalTransform,
                    &crate::robots::Robot,
                    Has<crate::robots::walker::Walker>,
                    Has<crate::robots::drone::Drone>,
                )>();
                let pick = q
                    .iter(w)
                    .filter(|(_, _, r, wk, dr)| !r.wrecked && if walker { *wk } else { *dr })
                    .min_by(|a, b| {
                        a.1.translation()
                            .distance(at)
                            .total_cmp(&b.1.translation().distance(at))
                    })
                    .map(|(e, g, ..)| (e, g.translation().distance(at)));
                let Some((e, d)) = pick else {
                    return ("none to wreck".into(), Vec::new());
                };
                w.get_mut::<crate::robots::Robot>(e).unwrap().wrecked = true;
                (format!("{d:.0} m off"), Vec::new())
            }));
        }

        for kind in AnomalyKind::ALL {
            out.push(time(&mut app, &format!("bloom: {kind:?}"), |app| {
                let at = ahead(app, 14.0);
                super::super::warmup::bloom(app.world_mut(), kind, at);
                ("14 m ahead".into(), Vec::new())
            }));
        }

        out.push(time(&mut app, "Depot residents", |app| {
            let before = count::<With<crate::robots::Robot>>(app);
            app.world_mut()
                .resource_mut::<crate::robots::director::Director>()
                .enabled = true;
            let first = vec![frame(app), frame(app)];
            app.world_mut()
                .resource_mut::<crate::robots::director::Director>()
                .enabled = false;
            let after = count::<With<crate::robots::Robot>>(app);
            (
                format!("{} robots out", after.saturating_sub(before)),
                first,
            )
        }));

        out.push(time(&mut app, "homecoming", |app| {
            app.world_mut().resource_mut::<crate::raid::Raid>().phase =
                crate::raid::Phase::Extracting { left: 1e-4 };
            let (_, at) = player(app);
            (format!("from {at:.0}"), Vec::new())
        }));

        (base, out, reference)
    }

    /// A valley loading: every frame from the first until the forest,
    /// grass and furniture have all arrived.
    pub(crate) fn load(map: &str) -> (Vec<f64>, HashMap<String, f64>) {
        let t = Instant::now();
        let mut app = client_threads(map, "downpour", Some(1));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ));
        let mut frames = vec![t.elapsed().as_secs_f64() * 1000.0];
        let (mut last, mut still) = (0, 0);
        take_worst();
        for _ in 0..3000 {
            systems();
            let t = Instant::now();
            app.update();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            frames.push(ms);
            WORST.with(|w| {
                let mut w = w.borrow_mut();
                if ms > w.0 {
                    *w = (ms, systems());
                }
            });
            let n = app
                .world()
                .resource::<super::super::budget::Triangles>()
                .0
                .len();
            still = if n == last { still + 1 } else { 0 };
            last = n;
            if still > 60 {
                break;
            }
        }
        (frames, take_worst())
    }

    fn report(base: &[f64], spikes: &[Spike], reference: f64) {
        let (p50, p99, max) = spread(base);
        println!(
            "reference {reference:.3} ms; busy moment p50 {p50:.1} ms, p99 {p99:.1}, max {max:.1} \
             ({:.0} / {:.0} / {:.0} units)",
            p50 / reference,
            p99 / reference,
            max / reference
        );
        for s in spikes {
            let (_, p99, max) = spread(&s.frames);
            let worst = s.frames.iter().position(|f| *f == max).unwrap_or(0);
            println!(
                "  {:>18}: p99 {p99:6.1} ms, max {max:6.1} ms at frame {worst:2} ({:5.0} units, \
                 {:4.1}x the moment's p50); {}",
                s.name,
                max / reference,
                max / p50,
                s.did
            );
            if !s.worst.is_empty() {
                println!("{:>22}{}", "", slowest(&s.worst, 6));
            }
        }
    }

    /// The worst frame after an event, at most this many times the busy
    /// moment's median frame (measured in the same run, so a loaded
    /// machine moves both). The truck's homecoming was 3.6-4.2x before the
    /// collider trees were optimised in place.
    const EVENT_CEILING: f64 = 3.0;
    /// A valley loading: every frame after the first two (the startup
    /// systems, then the first frame's text and screens) at most this
    /// many times the moment's median. Dressing the Depot was 57x before
    /// its shading was baked in the background.
    const LOAD_CEILING: f64 = 10.0;

    /// The raid's one-off events and a valley loading hold their worst
    /// frame to a ceiling (`SPIKE_MAP` for another valley;
    /// `SPIKE_PROFILE=1` with `--features "bevy/trace bevy/debug"` names
    /// the systems in each worst frame).
    #[test]
    fn hitches_hold_their_ceiling() {
        let _one_at_a_time = heavy();
        let map = std::env::var("SPIKE_MAP").unwrap_or("marrow_creek".into());
        if std::env::var_os("SPIKE_PROFILE").is_some() {
            use bevy::log::tracing_subscriber::layer::SubscriberExt;
            let times = PROFILE.get_or_init(SystemTimes::default).clone();
            let subscriber = bevy::log::tracing_subscriber::registry().with(times);
            bevy::log::tracing::subscriber::set_global_default(subscriber).unwrap();
        }
        // A real hitch comes back; a frame lost to the rest of the suite
        // rarely hits the same event twice. So: what's over in one run,
        // and again in a second.
        let first = over_ceiling(&map);
        if first.is_empty() {
            return;
        }
        println!("over in the first run: {first:?}; again");
        let second = over_ceiling(&map);
        let both: Vec<String> = second
            .iter()
            .filter(|(what, _)| first.iter().any(|(w, _)| w == what))
            .map(|(_, why)| why.clone())
            .collect();
        assert!(
            both.is_empty(),
            "hitches over their ceiling twice (SPIKE_PROFILE=1 with --features \"bevy/trace \
             bevy/debug\" names the systems):\n{}",
            both.join("\n")
        );
    }

    /// One run of the load and the events: what's over its ceiling (the
    /// event, or "loading"; and why).
    fn over_ceiling(map: &str) -> Vec<(String, String)> {
        let (frames, worst) = load(map);
        let (base, spikes, reference) = spikes(map);
        report(&base, &spikes, reference);
        let p50 = spread(&base).0;
        let (lp50, lp99, lmax) = spread(&frames[1..]);
        let at = frames.iter().skip(1).position(|f| *f == lmax).unwrap_or(0) + 1;
        println!(
            "{map} load: build {:.0} ms, then {} frames: p50 {lp50:.1} ms, p99 {lp99:.1}, max \
             {lmax:.1} (frame {at})",
            frames[0],
            frames.len() - 1,
        );
        if !worst.is_empty() {
            println!("    {}", slowest(&worst, 8));
        }
        let mut long: Vec<(usize, f64)> = frames
            .iter()
            .copied()
            .enumerate()
            .skip(1)
            .filter(|(_, f)| *f > 50.0)
            .collect();
        long.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!(
            "    frames over 50 ms: {}",
            long.iter()
                .take(12)
                .map(|(i, f)| format!("#{i} {f:.0}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut over = Vec::new();
        for s in &spikes {
            let max = spread(&s.frames).2;
            if max > EVENT_CEILING * p50 {
                over.push((
                    s.name.clone(),
                    format!(
                        "{}: {max:.1} ms, {:.1}x the moment's median > {EVENT_CEILING}x",
                        s.name,
                        max / p50
                    ),
                ));
            }
        }
        if let Some((i, f)) = long.iter().find(|(i, _)| *i > 2) {
            if *f > LOAD_CEILING * p50 {
                over.push((
                    "loading".into(),
                    format!(
                        "loading, frame {i}: {f:.0} ms, {:.0}x the moment's median > \
                         {LOAD_CEILING}x",
                        f / p50
                    ),
                ));
            }
        }
        over
    }
}
