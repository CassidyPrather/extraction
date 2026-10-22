//! The headset's frame, not one eye's: every lookbook view through both
//! eyes (64 mm apart, the same 100° view, each with its own sun cascades
//! as Bevy draws them), held to a stereo budget. The headset's per-eye
//! render size sets the fragment budget: the game's own auto render scale
//! (`Comfort::AUTO_EYE_PX`, 1800 px, under what SteamVR recommends for a
//! Bigscreen Beyond 2) unless `BUDGET_EYE_PX` says otherwise.
//!
//! What this catches that the one-eye gauges can't: a third view drawing
//! the world (a mirror, a scope or drone feed left on), work done for one
//! eye that the other repeats, a second eye that sees more than the first
//! (a range or a cut measured from one eye only), and the shaded
//! fragments of the whole frame at the headset's size.

#![cfg(test)]

use bevy::camera::visibility::{RenderLayers, VisibleEntities};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

use super::budget::{BUDGET, LoadCounter, ViewLoad};

/// Between the eyes, metres.
const IPD: f32 = 0.064;

/// The frame's budget: two eyes' worth of draws and shadow draws, and
/// shaded fragments at the eye size with every pixel at the one-eye
/// overdraw cap.
struct Stereo {
    draws: usize,
    shadow_draws: usize,
    /// Millions of fragments a frame, both eyes.
    fragments_m: f32,
}

fn eye_px() -> f32 {
    std::env::var("BUDGET_EYE_PX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::rig::comfort::Comfort::AUTO_EYE_PX)
}

fn budget() -> Stereo {
    let px = eye_px();
    Stereo {
        draws: BUDGET.draws * 2,
        shadow_draws: BUDGET.shadow_draws * 2,
        fragments_m: 2.0 * px * px * BUDGET.overdraw_mean / 1e6,
    }
}

/// The second eye: the flat camera's twin, `IPD` to its right.
#[derive(Component)]
struct RightEye;

fn both_eyes(app: &mut App) -> (Entity, Entity) {
    let w = app.world_mut();
    let left = w
        .query_filtered::<Entity, With<crate::render::FreeCamera>>()
        .single(w)
        .expect("the free camera");
    let weather = w.resource::<crate::world::sky::Weather>().clone();
    let right = w
        .spawn((
            Name::new("Right eye"),
            RightEye,
            super::camera_bundle(&weather),
            Transform::from_xyz(IPD, 0.0, 0.0),
            ChildOf(left),
        ))
        .id();
    (left, right)
}

/// Both eyes' loads, and how many 3D views draw anything at all.
fn measure(app: &mut App, eyes: (Entity, Entity)) -> (ViewLoad, ViewLoad, usize) {
    app.world_mut()
        .run_system_once(
            move |cams: Query<(Entity, &Camera, &VisibleEntities), With<Camera3d>>,
                  layers: Query<&RenderLayers>,
                  counter: LoadCounter| {
                let one = |e: Entity| {
                    let (_, _, seen) = cams.get(e).expect("an eye");
                    counter.measure(e, seen)
                };
                // Views drawing the world (an impostor capture rig draws
                // its own layer, at start; with no GPU here, for ever).
                let drawing = cams
                    .iter()
                    .filter(|(e, c, v)| {
                        c.is_active
                            && layers
                                .get(*e)
                                .map_or(true, |l| l.intersects(&RenderLayers::default()))
                            && v.entities.values().any(|l| !l.is_empty())
                    })
                    .count();
                (one(eyes.0), one(eyes.1), drawing)
            },
        )
        .unwrap()
}

fn holds(map: &str) {
    let _one_at_a_time = super::budget::tests::heavy();
    let mut app = super::budget::tests::client_threads(map, "clearing", None);
    // (The flat camera comes a few frames in.)
    for _ in 0..300 {
        app.update();
    }
    let eyes = both_eyes(&mut app);
    for _ in 0..30 {
        app.update();
    }
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let file = crate::asset_root_path()
        .join("..")
        .join(format!("scripts/lookbook/{map}.views"));
    let views = crate::tour::scripted_views(&zone, &file);
    assert!(!views.is_empty());
    let b = budget();
    let px = eye_px();
    let mut over = Vec::new();
    for v in views {
        {
            let w = app.world_mut();
            let mut q =
                w.query_filtered::<(&mut Transform, &mut Projection), With<super::FreeCamera>>();
            let (mut t, mut p) = q.single_mut(w).unwrap();
            *t = Transform::from_translation(Vec3::from_array(v.at))
                .looking_at(Vec3::from_array(v.look), Vec3::Y);
            if let Projection::Perspective(p) = &mut *p {
                p.fov = super::budget::EYE_FOV_DEG.to_radians();
                p.aspect_ratio = 1.0;
            }
            let mut q =
                w.query_filtered::<&mut Projection, (With<RightEye>, Without<super::FreeCamera>)>();
            if let Ok(mut p) = q.single_mut(w) {
                if let Projection::Perspective(p) = &mut *p {
                    p.fov = super::budget::EYE_FOV_DEG.to_radians();
                    p.aspect_ratio = 1.0;
                }
            }
        }
        // Settled: both eyes' counts the same three checks running.
        let mut last = (0, 0);
        let mut still = 0;
        let mut m = measure(&mut app, eyes);
        for _ in 0..60 {
            for _ in 0..10 {
                app.update();
            }
            m = measure(&mut app, eyes);
            let now = (m.0.draws, m.1.draws);
            still = if now == last { still + 1 } else { 0 };
            last = now;
            if still >= 3 {
                break;
            }
        }
        let (l, r, views_drawing) = m;
        let od_l = super::overdraw::measure(app.world_mut(), eyes.0);
        let od_r = super::overdraw::measure(app.world_mut(), eyes.1);
        let draws = l.draws + r.draws;
        let shadow_draws = l.shadow_draws + r.shadow_draws;
        let fragments_m = (od_l.mean + od_r.mean) * px * px / 1e6;
        let name = v.file.trim_end_matches(".png");
        println!(
            "{map} {name}: draws {} + {} = {draws}, shadow draws {} + {} = {shadow_draws}, \
             {fragments_m:.1}M fragments at {px:.0}² an eye (overdraw {:.2} / {:.2}); {views_drawing} views drawing",
            l.draws, r.draws, l.shadow_draws, r.shadow_draws, od_l.mean, od_r.mean
        );
        let mut o = Vec::new();
        if draws > b.draws {
            o.push(format!("draws {draws} > {}", b.draws));
        }
        if shadow_draws > b.shadow_draws {
            o.push(format!("shadow draws {shadow_draws} > {}", b.shadow_draws));
        }
        if fragments_m > b.fragments_m {
            o.push(format!(
                "{fragments_m:.1}M fragments > {:.1}M",
                b.fragments_m
            ));
        }
        // The eyes see nearly the same: one seeing far more is a cut or a
        // range taken from one eye.
        let (lo, hi) = (l.draws.min(r.draws), l.draws.max(r.draws));
        if hi > lo + lo / 10 + 20 {
            o.push(format!(
                "the eyes differ: {} and {} draws",
                l.draws, r.draws
            ));
        }
        if views_drawing > 2 {
            o.push(format!(
                "{views_drawing} views drawing the world, not the two eyes"
            ));
        }
        if !o.is_empty() {
            over.push(format!("{name}: {}", o.join(", ")));
        }
    }
    assert!(
        over.is_empty(),
        "{map}, both eyes over the frame budget:\n{}",
        over.join("\n")
    );
}

#[test]
fn marrow_creek_holds_the_stereo_budget() {
    holds("marrow_creek");
}

/// Every valley, both eyes (slow: `cargo test --lib -- --ignored stereo`).
#[test]
#[ignore]
fn every_valley_holds_the_stereo_budget() {
    for (map, _) in extraction_core::map::VALLEYS {
        holds(map);
    }
}

/// The comfort board's SMOOTHING reaches the views of the world (both
/// eyes here) and leaves offscreen pictures alone.
#[test]
fn smoothing_reaches_the_eyes() {
    let _one_at_a_time = super::budget::tests::heavy();
    let mut app = super::budget::tests::client_threads("training_yard", "clearing", None);
    for _ in 0..120 {
        app.update();
    }
    let eyes = both_eyes(&mut app);
    let image = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let scope = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Camera {
                is_active: false,
                ..default()
            },
            bevy::camera::RenderTarget::Image(image.into()),
            Msaa::Off,
        ))
        .id();
    app.update();
    let msaa = |app: &App, e: Entity| *app.world().get::<Msaa>(e).unwrap();
    assert_eq!(msaa(&app, eyes.0), Msaa::Sample4);
    assert_eq!(msaa(&app, eyes.1), Msaa::Sample4);
    app.world_mut()
        .resource_mut::<crate::rig::comfort::Comfort>()
        .msaa = 2;
    app.update();
    assert_eq!(msaa(&app, eyes.0), Msaa::Sample2);
    assert_eq!(msaa(&app, eyes.1), Msaa::Sample2);
    assert_eq!(
        msaa(&app, scope),
        Msaa::Off,
        "an offscreen picture keeps its own"
    );
    // Off: FXAA instead, on the eyes only; back on, it goes.
    let fxaa = |app: &App, e: Entity| app.world().get::<bevy::anti_alias::fxaa::Fxaa>(e).is_some();
    assert!(!fxaa(&app, eyes.0));
    app.world_mut()
        .resource_mut::<crate::rig::comfort::Comfort>()
        .msaa = 0;
    app.update();
    assert_eq!(msaa(&app, eyes.0), Msaa::Off);
    assert!(fxaa(&app, eyes.0) && fxaa(&app, eyes.1));
    assert!(!fxaa(&app, scope));
    app.world_mut()
        .resource_mut::<crate::rig::comfort::Comfort>()
        .msaa = 4;
    app.update();
    assert!(!fxaa(&app, eyes.0) && !fxaa(&app, eyes.1));
}
