// Probe: how the player body climbs a box flight against a ramp of the
// same pitch. Paste into `mod tests` in src/player.rs (it uses that
// module's `use super::*`). Run with
//   cargo test --profile fast -j 4 --lib player::tests::probe_stairs_hop -- --nocapture
// It prints, per case, the max per-tick rise and the count of ticks
// rising > 3 cm, then (z, y) every third tick: 130 ticks up, 130 down.
// It asserts nothing yet: the fix's test should require max |dy| <= 0.03
// on both legs of the box flight.


    #[test]
    fn probe_stairs_hop() {
        for ramp in [false, true] {
            let opts = crate::LaunchOptions { robots: false, ..Default::default() };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            let zone = app.world().resource::<Zone>().clone();
            let g = zone.on_ground(0.0, 200.0);
            let base = g.y + 1.0;
            let slab = |w: &mut World, size: Vec3, t: Transform| {
                w.spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), t,
                    CollisionLayers::new(Layer::World, LayerMask::ALL)));
            };
            let w = app.world_mut();
            slab(w, Vec3::new(20.0, 1.0, 30.0), Transform::from_xyz(g.x, base - 0.5, g.z));
            let (rise, tread, n) = (0.17f32, 0.3f32, 6);
            if !ramp {
                for k in 1..=n {
                    let front = -1.0 - tread * (k - 1) as f32;
                    let back = -10.0;
                    let h = rise * k as f32;
                    slab(w, Vec3::new(2.0, h, front - back),
                        Transform::from_xyz(g.x, base + h * 0.5, g.z + (front + back) * 0.5));
                }
            } else {
                // The flight's nosings as one slope, then the landing.
                let run = tread * (n - 1) as f32;
                let top = rise * n as f32;
                let a = Vec2::new(-1.0, rise); // (z, y) first nosing
                let b = Vec2::new(-1.0 - run, top);
                let along = b - a;
                let len = along.length() + 0.6;
                let mid = (a + b) * 0.5;
                let pitch = (along.y / -along.x).atan();
                let th = 0.3;
                let nrm = Vec2::new(-along.y, -along.x).normalize(); // up-ish normal in (z,y)
                let c = mid - Vec2::new(nrm.x, nrm.y) * th * 0.5;
                slab(w, Vec3::new(2.0, th, len),
                    Transform::from_xyz(g.x, base + c.y, g.z + c.x)
                        .with_rotation(Quat::from_rotation_x(pitch)));
                slab(w, Vec3::new(2.0, top, 10.0 + (-1.0 - run)),
                    Transform::from_xyz(g.x, base + top * 0.5, g.z + (-10.0 + (-1.0 - run)) * 0.5));
            }
            let player = app.world_mut().query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world()).next().unwrap();
            *app.world_mut().get_mut::<Transform>(player).unwrap() =
                Transform::from_xyz(g.x, base + 0.05, g.z + 0.5);
            for _ in 0..30 { app.update(); }
            let mut ys = Vec::new();
            for i in 0..260 {
                app.world_mut().get_mut::<Transform>(player).unwrap().rotation = Quat::IDENTITY;
                let mut inp = app.world_mut().get_mut::<RigInput>(player).unwrap();
                inp.head.translation = Vec3::new(0.0, 1.6, 0.0);
                inp.locomotion = Vec2::new(0.0, if i < 130 { 1.0 } else { -1.0 });
                app.update();
                let t = app.world().get::<Transform>(player).unwrap().translation;
                ys.push((t.z - g.z, t.y - base));
            }
            let dys: Vec<f32> = ys.windows(2).map(|w| w[1].1 - w[0].1).collect();
            let big = dys.iter().filter(|d| d.abs() > 0.03).count();
            let maxd = dys.iter().fold(0.0f32, |m, d| m.max(d.abs()));
            eprintln!("RAMP={ramp} final {:?} max per-tick dy {maxd:.3} ticks>3cm {big}", ys.last());
            let line: Vec<String> = ys.iter().step_by(3).map(|(z, y)| format!("{z:.2}:{y:.3}")).collect();
            eprintln!("  {}", line.join(" "));
        }
    }
