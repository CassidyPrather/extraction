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
