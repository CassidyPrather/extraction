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
    /// workload timed beside it: tick ms per reference ms, the median.
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
        let (t, r) = (median(tick), median(reference));
        (t / r, t, r)
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
}
