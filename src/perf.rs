//! CPU cost of the simulation (no rendering): `cargo test --release --lib
//! perf -- --ignored --nocapture`. A headset at 90 Hz leaves about 11 ms a
//! frame for everything; the game logic should take a small slice of it.

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::robots::{Brain, Job, JobKind};

    /// Wall time spent in stretches of the frame, per tick.
    #[derive(Resource, Default)]
    struct Spans {
        mark: Option<std::time::Instant>,
        logic_now: f64,
        physics_now: f64,
    }

    fn start(mut s: ResMut<Spans>) {
        s.mark = Some(std::time::Instant::now());
    }
    fn end_logic(mut s: ResMut<Spans>) {
        let t = s
            .mark
            .take()
            .map_or(0.0, |m| m.elapsed().as_secs_f64() * 1000.0);
        s.logic_now += t;
    }
    fn end_physics(mut s: ResMut<Spans>) {
        let t = s
            .mark
            .take()
            .map_or(0.0, |m| m.elapsed().as_secs_f64() * 1000.0);
        s.physics_now += t;
    }
    fn instrument(app: &mut App) {
        use avian3d::prelude::PhysicsSystems;
        // FixedFirst -> FixedUpdate is game logic; physics runs inside
        // FixedPostUpdate.
        app.init_resource::<Spans>()
            .add_systems(FixedFirst, start)
            .add_systems(
                FixedPostUpdate,
                (end_logic, start).chain().before(PhysicsSystems::First),
            )
            .add_systems(FixedPostUpdate, end_physics.after(PhysicsSystems::Last));
    }
    fn take(app: &mut App) -> (f64, f64) {
        let mut s = app.world_mut().resource_mut::<Spans>();
        let r = (s.logic_now, s.physics_now);
        s.logic_now = 0.0;
        s.physics_now = 0.0;
        r
    }

    fn report(name: &str, mut ms: Vec<f64>) {
        ms.sort_by(f64::total_cmp);
        let pct = |p: f64| ms[((ms.len() - 1) as f64 * p) as usize];
        let mean = ms.iter().sum::<f64>() / ms.len() as f64;
        println!(
            "{name}: {} ticks, mean {mean:.2} ms, p50 {:.2}, p95 {:.2}, p99 {:.2}, max {:.2}",
            ms.len(),
            pct(0.5),
            pct(0.95),
            pct(0.99),
            ms[ms.len() - 1]
        );
    }

    /// The headless game's tick in a busy moment (`super::fight`: two
    /// guns in hand, ten robots on the player), in units of the reference
    /// workload timed beside it: each tick over the reference timed right
    /// after it, the lower quartile of those. (The tick runs on every core
    /// and the reference on one, so neighbours on the suite's other test
    /// threads slow the tick far more than its yardstick: the median of
    /// each, taken apart, read 62.7 units once in the parallel suite and
    /// failed. The quartile is the moments they left it alone.)
    fn headless_units(armed: bool) -> (f64, f64, f64) {
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..30 {
            app.update();
        }
        if armed {
            super::fight::arm(&mut app);
        }
        super::fight::hunt(&mut app);
        for _ in 0..90 {
            super::fight::keep_alive(&mut app);
            app.update();
        }
        let (mut tick, mut reference) = (Vec::new(), Vec::new());
        for _ in 0..450 {
            super::fight::keep_alive(&mut app);
            let t = std::time::Instant::now();
            app.update();
            tick.push(t.elapsed().as_secs_f64() * 1000.0);
            reference.push(super::fight::reference_ms());
        }
        let median = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let ratios: Vec<f64> = tick.iter().zip(&reference).map(|(t, r)| t / r).collect();
        let quartile = {
            let mut v = ratios;
            v.sort_by(f64::total_cmp);
            v[v.len() / 4]
        };
        (quartile, median(tick), median(reference))
    }

    /// The whole headless game holds a floor of ticks a second in a busy
    /// moment (`super::fight`), in reference units: a slowdown anywhere in
    /// the simulation fails here, where the client gauge's shares only
    /// see one schedule outgrowing the others. The floor is about three
    /// times today's tick: wall time beside the rest of the suite runs
    /// more than double what it does alone.
    #[test]
    fn headless_ticks_hold_their_floor() {
        let (units, tick, reference) = headless_units(true);
        println!(
            "headless busy tick: {tick:.2} ms (median), reference {reference:.3} ms: {units:.1} units, \
             {:.0} ticks a second",
            1000.0 / tick
        );
        assert!(
            units < HEADLESS_TICK_UNITS,
            "the headless game's busy tick is {units:.1} reference units > {HEADLESS_TICK_UNITS} \
             ({tick:.1} ms, {:.0} ticks a second): something in the simulation got much dearer \
             (`cargo test --lib system_profile --features \"bevy/trace bevy/debug\" -- --ignored \
             --nocapture` names systems)",
            1000.0 / tick
        );
    }

    /// What a settled valley's loose items are doing: how many dynamic
    /// items, how many still awake (anywhere, and within 15 m of where the
    /// truck drops you), the awake by kind, and the worst creeps (mm a
    /// frame over 30 frames, by name).
    fn awake_items(
        map: &str,
    ) -> (
        usize,
        usize,
        usize,
        Vec<(String, usize)>,
        Vec<(f32, String)>,
    ) {
        use avian3d::prelude::*;
        let _counting = crate::render::budget::tests::counted();
        let mut app = crate::render::budget::tests::client_threads(map, "clearing", None);
        crate::render::budget::tests::settle(&mut app);
        for _ in 0..120 {
            app.update();
        }
        let drop = {
            let zone = app.world().resource::<crate::world::Zone>();
            zone.map
                .markers_of(&extraction_core::map::MarkerKind::Insertion)
                .next()
                .map(|m| Vec2::from(m.at))
        };
        // Only bodies the physics step simulates: a stowed item (a pack,
        // the crate, a socket) is disabled, never asleep. And not one that
        // lifts (a Featherstone hovers on a force every frame, by design,
        // and a force keeps a body awake).
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let positions = |w: &mut World| -> Vec<(Entity, Vec3)> {
            w.query_filtered::<(Entity, &GlobalTransform, &RigidBody, &crate::items::Item), (
                Without<RigidBodyDisabled>,
                Without<ColliderDisabled>,
                Without<crate::interact::Held>,
            )>()
            .iter(w)
            .filter(|(_, _, b, i)| b.is_dynamic() && catalog.get(i.kind()).lift_kg <= 0.0)
            .map(|(e, g, _, _)| (e, g.translation()))
            .collect()
        };
        let before = positions(app.world_mut());
        for _ in 0..30 {
            app.update();
        }
        let w = app.world_mut();
        let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
        let mut creep = Vec::new();
        let (mut awake, mut awake_at_drop) = (0, 0);
        for (e, was) in &before {
            let Some(g) = w.get::<GlobalTransform>(*e) else {
                continue;
            };
            let now = g.translation();
            let name = w.get::<Name>(*e).map_or("?".into(), |n| n.to_string());
            let mm = (now - *was).length() * 1000.0 / 30.0;
            if w.get::<Sleeping>(*e).is_none() {
                awake += 1;
                if drop.is_some_and(|d| now.xz().distance(d) < 15.0) {
                    awake_at_drop += 1;
                }
                *kinds.entry(name.clone()).or_default() += 1;
                creep.push((mm, name));
            }
        }
        let mut kinds: Vec<(String, usize)> = kinds.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1));
        creep.sort_by(|a, b| b.0.total_cmp(&a.0));
        (before.len(), awake, awake_at_drop, kinds, creep)
    }

    /// Of a settled valley's dynamic items, at most this share awake: a
    /// settled item should be asleep (`soak::census`).
    const AWAKE_SHARE: f32 = 0.02;

    /// Every valley and the yard, settled: loose items asleep, at the
    /// drop and across the valley. Awake items keep the physics step
    /// solving their contacts every frame, and nothing that builds on
    /// rest (a merge, a cheaper tick) can. First measure (2026-11-01):
    /// 4-8 awake a valley (1-2%), 3-4 of them within 15 m of the drop,
    /// piled guns, magazines and boxes rocking on their contacts; the yard
    /// none. (Stowed items, disabled, and Featherstones, which hover on a
    /// force, aren't counted.)
    #[test]
    #[ignore = "known failing (2026-11-01): stacked items never sleep; Hands un-ignores it with the fix. \
                cargo test --lib gauge_items_fall_asleep -- --ignored --nocapture"]
    fn gauge_items_fall_asleep() {
        let mut over = Vec::new();
        let maps = extraction_core::map::VALLEYS
            .iter()
            .map(|v| v.0)
            .chain([crate::camp::TRAINING_YARD]);
        for map in maps {
            let (n, awake, at_drop, kinds, creep) = awake_items(map);
            let share = awake as f32 / n.max(1) as f32;
            println!(
                "{map}: {awake} of {n} dynamic items awake ({:.0}%), {at_drop} within 15 m of the drop",
                share * 100.0
            );
            let top = |v: &[(String, usize)]| {
                v.iter()
                    .take(8)
                    .map(|(k, c)| format!("{k} {c}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("    awake by kind: {}", top(&kinds));
            println!(
                "    worst creep (mm a frame): {}",
                creep
                    .iter()
                    .take(8)
                    .map(|(mm, k)| format!("{k} {mm:.3}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if share > AWAKE_SHARE || at_drop > 0 {
                over.push(format!(
                    "{map}: {awake} of {n} awake ({:.0}%), {at_drop} at the drop",
                    share * 100.0
                ));
            }
        }
        assert!(
            over.is_empty(),
            "items still awake once settled:\n{}",
            over.join("\n")
        );
    }

    /// The ceiling for `headless_ticks_hold_their_floor`.
    const HEADLESS_TICK_UNITS: f64 = 60.0;

    #[test]
    #[ignore = "calibration: cargo test --lib headless_calibration -- --ignored --nocapture"]
    fn headless_calibration() {
        for armed in [false, true] {
            let (units, tick, reference) = headless_units(armed);
            println!(
                "armed {armed}: tick {tick:.2} ms, reference {reference:.3}: {units:.1} units"
            );
        }
    }

    /// How steady the load yardstick is: single samples, and medians of
    /// `n`, one after another for about a minute. The spread within a
    /// median is jitter (a longer sample helps); between medians, drift
    /// (only sampling across the load helps).
    #[test]
    #[ignore = "calibration: cargo test --lib reference_load_noise -- --ignored --nocapture"]
    fn reference_load_noise() {
        use super::fight::{median_ms, reference_load_ms};
        let spread = |v: &[f64]| {
            let lo = v.iter().copied().fold(f64::MAX, f64::min);
            let hi = v.iter().copied().fold(0.0, f64::max);
            (lo, hi, hi / lo - 1.0)
        };
        let singles: Vec<f64> = (0..60).map(|_| reference_load_ms()).collect();
        let (lo, hi, s) = spread(&singles);
        println!("60 single: {lo:.2}-{hi:.2} ms ({:.0}%)", s * 100.0);
        for n in [9, 25] {
            let t = std::time::Instant::now();
            let mut medians = Vec::new();
            while t.elapsed().as_secs() < 30 {
                medians.push(median_ms(n, reference_load_ms));
            }
            let (lo, hi, s) = spread(&medians);
            println!(
                "{} medians of {n}: {lo:.2}-{hi:.2} ms ({:.0}%)",
                medians.len(),
                s * 100.0
            );
        }
    }

    #[test]
    #[ignore = "timing: run in release with --nocapture"]
    fn tick_cost_in_a_firefight() {
        // `EXTRACTION_TEST_MAP=cinder_works` to time another valley.
        let mut opts = crate::LaunchOptions::default();
        if let Ok(map) = std::env::var("EXTRACTION_TEST_MAP") {
            opts.map = map;
        }
        let mut app = crate::headless_app(opts, false);
        if std::env::var("PERF_SINGLE").is_ok() {
            use bevy::ecs::schedule::SingleThreadedExecutor;
            app.edit_schedule(FixedUpdate, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            app.edit_schedule(Update, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            app.edit_schedule(PostUpdate, |s| {
                s.set_executor(SingleThreadedExecutor::new());
            });
            println!("single-threaded schedules");
        }
        if let Some(n) = std::env::var("PERF_SUBSTEPS")
            .ok()
            .and_then(|n| n.parse().ok())
        {
            app.insert_resource(avian3d::prelude::SubstepCount(n));
            println!("{n} substeps");
        }
        instrument(&mut app);
        app.finish();
        app.cleanup();
        let mut quiet = Vec::new();
        for _ in 0..90 * 10 {
            let t = std::time::Instant::now();
            app.update();
            quiet.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        report("raid start, director running", quiet.split_off(90));
        // A response team arriving: six drones and four walkers on the player.
        app.world_mut().resource_mut::<crate::raid::Raid>().phase = crate::raid::Phase::Out;
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let player = app
            .world_mut()
            .query_filtered::<(Entity, &GlobalTransform), With<crate::rig::LocalPlayer>>()
            .iter(app.world())
            .map(|(_, g)| g.translation())
            .next()
            .unwrap();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut robots = Vec::new();
        {
            let mut c = Commands::new(&mut queue, app.world());
            for i in 0..10 {
                let a = i as f32 * 0.63;
                let at = zone.on_ground(player.x + a.cos() * 40.0, player.z + a.sin() * 40.0);
                let job = Job {
                    kind: JobKind::Response,
                    site: player,
                    duration: 999.0,
                    exit: at,
                };
                robots.push(if i < 6 {
                    crate::robots::drone::spawn_drone(&mut c, at + Vec3::Y * 15.0, job)
                } else {
                    crate::robots::walker::spawn_walker(&mut c, &zone, at, job)
                });
            }
        }
        queue.apply(app.world_mut());
        app.update();
        for r in &robots {
            *app.world_mut().get_mut::<Brain>(*r).unwrap() = Brain::Investigate {
                at: player,
                left: 999.0,
            };
        }
        let mut busy = Vec::new();
        let (mut logic, mut physics) = (Vec::new(), Vec::new());
        take(&mut app);
        for _ in 0..90 * 20 {
            let t = std::time::Instant::now();
            app.update();
            busy.push(t.elapsed().as_secs_f64() * 1000.0);
            let (l, p) = take(&mut app);
            logic.push(l);
            physics.push(p);
        }
        report("ten robots on the player", busy);
        report("  of which game logic (fixed tick, before physics)", logic);
        report("  of which physics", physics);
        // What the physics step is carrying.
        use avian3d::prelude::*;
        let w = app.world_mut();
        let dynamic = w
            .query::<&RigidBody>()
            .iter(w)
            .filter(|b| b.is_dynamic())
            .count();
        let asleep = w
            .query_filtered::<&RigidBody, With<Sleeping>>()
            .iter(w)
            .filter(|b| b.is_dynamic())
            .count();
        let disabled = w
            .query_filtered::<&RigidBody, With<RigidBodyDisabled>>()
            .iter(w)
            .count();
        let kinematic = w
            .query::<&RigidBody>()
            .iter(w)
            .filter(|b| b.is_kinematic())
            .count();
        let colliders = w.query::<&Collider>().iter(w).count();
        println!(
            "bodies: {dynamic} dynamic ({asleep} asleep, {disabled} disabled), {kinematic} kinematic; {colliders} colliders"
        );
    }

    /// A collider whose scale drifts off one (a transform rebuilt from a
    /// matrix every tick, say) is re-derived by the physics engine every
    /// time it changes — as a convex hull, for anything but a box. Walker
    /// legs did exactly that and cost a quarter of the physics step.
    #[test]
    fn no_collider_scale_drifts() {
        use avian3d::prelude::Collider;
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let at = zone.on_ground(40.0, 200.0);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let robots: Vec<Entity> = {
            let mut c = Commands::new(&mut queue, app.world());
            let job = Job {
                kind: JobKind::Response,
                site: at,
                duration: 999.0,
                exit: at,
            };
            vec![
                crate::robots::walker::spawn_walker(&mut c, &zone, at, job.clone()),
                crate::robots::drone::spawn_drone(&mut c, at + Vec3::new(5.0, 12.0, 0.0), job),
            ]
        };
        queue.apply(app.world_mut());
        app.update();
        for r in &robots {
            *app.world_mut().get_mut::<Brain>(*r).unwrap() = Brain::Investigate {
                at: at + Vec3::new(30.0, 0.0, 10.0),
                left: 999.0,
            };
        }
        for _ in 0..90 * 4 {
            app.update();
        }
        let w = app.world_mut();
        let drifting: Vec<String> = w
            .query::<(&Collider, Option<&Name>)>()
            .iter(w)
            .filter(|(c, _)| c.scale() != Vec3::ONE && c.shape().as_cuboid().is_none())
            .map(|(c, n)| format!("{:?} {:?}", n.map(|n| n.as_str()), c.scale()))
            .collect();
        assert!(
            drifting.is_empty(),
            "scaled non-box colliders: {drifting:?}"
        );
    }
}

/// A raid's busy moment, staged the same way for every CPU gauge: two
/// guns in the player's hands (a rifle right, a pistol left, taken the
/// way a hand takes them: held guns run their handling every tick) and
/// ten robots (six drones, four walkers) hunting the player from 35 m.
#[cfg(test)]
pub(crate) mod fight {
    use bevy::prelude::*;

    use crate::robots::{Brain, Job, JobKind};

    /// The local player's root and where it stands.
    pub fn player(app: &mut App) -> (Entity, Vec3) {
        let w = app.world_mut();
        let (e, t) = w
            .query_filtered::<(Entity, &GlobalTransform), With<crate::rig::LocalPlayer>>()
            .single(w)
            .expect("the local player");
        (e, t.translation())
    }

    pub fn arm(app: &mut App) {
        let catalog = app.world().resource::<crate::items::Catalog>().0.clone();
        let (player, at) = player(app);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let guns: Vec<(Entity, crate::rig::Side)> = {
            let mut c = Commands::new(&mut queue, app.world());
            [
                ("ak74", crate::rig::Side::Right),
                ("pm_pistol", crate::rig::Side::Left),
            ]
            .into_iter()
            .map(|(id, side)| {
                let kind = catalog.kind(id).expect("a gun in the catalogue");
                let e = crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind,
                        transform: Transform::from_translation(at + Vec3::Y * 1.2),
                        condition: 1.0,
                    },
                );
                (e, side)
            })
            .collect()
        };
        queue.apply(app.world_mut());
        app.update();
        for (item, side) in guns {
            app.world_mut()
                .write_message(crate::interact::HandAction::Grab {
                    player,
                    side,
                    item,
                    point: None,
                });
        }
        for _ in 0..10 {
            app.update();
        }
        let w = app.world_mut();
        let held = w
            .query_filtered::<(), (With<crate::weapons::Gun>, With<crate::interact::Held>)>()
            .iter(w)
            .count();
        assert_eq!(held, 2, "the busy moment's player should hold two guns");
    }

    pub fn hunt(app: &mut App) {
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let (_, at) = player(app);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut robots = Vec::new();
        {
            let mut c = Commands::new(&mut queue, app.world());
            for i in 0..10 {
                let a = i as f32 * 0.63;
                let spot = zone.on_ground(at.x + a.cos() * 35.0, at.z + a.sin() * 35.0);
                let job = Job {
                    kind: JobKind::Response,
                    site: at,
                    duration: 999.0,
                    exit: spot,
                };
                robots.push(if i < 6 {
                    crate::robots::drone::spawn_drone(&mut c, spot + Vec3::Y * 15.0, job)
                } else {
                    crate::robots::walker::spawn_walker(&mut c, &zone, spot, job)
                });
            }
        }
        queue.apply(app.world_mut());
        app.update();
        for r in &robots {
            if let Some(mut b) = app.world_mut().get_mut::<Brain>(*r) {
                *b = Brain::Investigate { at, left: 999.0 };
            }
        }
    }

    /// Keeps everyone standing (the moment is about cost, not who wins).
    pub fn keep_alive(app: &mut App) {
        let w = app.world_mut();
        for mut h in w.query::<&mut crate::health::Health>().iter_mut(w) {
            h.hp = h.max;
            h.down = false;
        }
    }

    /// A fixed piece of work, timed (ms): the yardstick ticks are
    /// measured against, so a slower machine isn't a regression.
    pub fn reference_ms() -> f64 {
        let t = std::time::Instant::now();
        let mut x = 1u64;
        for i in 0..200_000u64 {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(i);
            x ^= x >> 13;
        }
        std::hint::black_box(x);
        t.elapsed().as_secs_f64() * 1000.0
    }

    /// The same work, `PARALLEL_CHUNKS` of it, shared by as many threads as
    /// the machine runs at once (ms): the yardstick for what the task
    /// pools spread (a valley's load), so fewer cores or a CPU quota read
    /// as a slower machine, not a regression.
    pub fn reference_parallel_ms() -> f64 {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let next = AtomicUsize::new(0);
        let t = std::time::Instant::now();
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    while next.fetch_add(1, Ordering::Relaxed) < PARALLEL_CHUNKS {
                        reference_ms();
                    }
                });
            }
        });
        t.elapsed().as_secs_f64() * 1000.0
    }

    /// The yardstick for a valley's load (ms): the geometric mean of the
    /// one-thread and every-thread timings of the same work. A load is
    /// part serial, part spread over the task pools: held to two cores of
    /// four, the every-thread timing doubled and the loads grew 1.3-1.5x
    /// (the mean, 1.43x, is the square root of two's), so clock speed
    /// counts in full and the core count by its square root.
    pub fn reference_load_ms() -> f64 {
        let one: f64 = (0..PARALLEL_CHUNKS).map(|_| reference_ms()).sum();
        (one * reference_parallel_ms()).sqrt()
    }

    /// How much work `reference_parallel_ms` shares out, in
    /// `reference_ms` pieces: enough that the threads' start is noise.
    pub const PARALLEL_CHUNKS: usize = 64;

    /// The median of `n` timings of `f` (ms): one is at the mercy of
    /// whatever else the machine was doing.
    pub fn median_ms(n: usize, f: impl Fn() -> f64) -> f64 {
        let mut v: Vec<f64> = (0..n.max(1)).map(|_| f()).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    }
}
