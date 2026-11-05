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
pub(super) struct Stereo {
    pub draws: usize,
    pub shadow_draws: usize,
    /// Millions of fragments a frame, both eyes.
    pub fragments_m: f32,
}

pub(super) fn eye_px() -> f32 {
    std::env::var("BUDGET_EYE_PX")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::rig::comfort::Comfort::AUTO_EYE_PX)
}

pub(super) fn budget() -> Stereo {
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

pub(super) fn both_eyes(app: &mut App) -> (Entity, Entity) {
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
/// Both eyes as the headset's: a square view of the eye's field.
pub(super) fn eye_projections(app: &mut App) {
    let w = app.world_mut();
    let mut q =
        w.query_filtered::<&mut Projection, Or<(With<super::FreeCamera>, With<RightEye>)>>();
    for mut p in q.iter_mut(w) {
        if let Projection::Perspective(p) = &mut *p {
            p.fov = super::budget::EYE_FOV_DEG.to_radians();
            p.aspect_ratio = 1.0;
        }
    }
}

pub(super) fn measure(app: &mut App, eyes: (Entity, Entity)) -> (ViewLoad, ViewLoad, usize) {
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
    let file = crate::asset_root_path()
        .join("..")
        .join(format!("scripts/lookbook/{map}.views"));
    let over = holds_views(map, "clearing", |zone| {
        crate::tour::scripted_views(zone, &file)
    });
    assert!(
        over.is_empty(),
        "{map}, both eyes over the frame budget:\n{}",
        over.join("\n")
    );
}

/// Each view through both eyes in `weather`, settled; what's over the
/// stereo budget, a line a view.
fn holds_views(
    map: &str,
    weather: &str,
    views: impl FnOnce(&crate::world::Zone) -> Vec<crate::tour::View>,
) -> Vec<String> {
    // Counts: draws, shadow draws, fragments.
    let _counting = super::budget::tests::counted();
    let mut app = super::budget::tests::client_threads(map, weather, None);
    // (The flat camera comes a few frames in.)
    for _ in 0..300 {
        app.update();
    }
    let eyes = both_eyes(&mut app);
    for _ in 0..30 {
        app.update();
    }
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let views = views(&zone);
    assert!(!views.is_empty(), "{map}: no views");
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
        // Near its budget: what it's spent on (one eye; the other's the
        // same), to know what to cut.
        let near = |n: usize, of: usize| n as f32 > of as f32 * 0.75;
        if near(draws, b.draws) || near(shadow_draws, b.shadow_draws) {
            let top = |by: &[(String, usize, usize)]| {
                let mut most = by.to_vec();
                most.sort_by(|a, b| b.1.cmp(&a.1));
                most.iter()
                    .take(
                        std::env::var("WALK_TOP")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(6),
                    )
                    .map(|(w, c, _)| format!("{w} {c}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            println!("    drawn: {}", top(&l.by));
            println!("    shadow: {}", top(&l.shadow_by));
        }
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
    over
}

/// Where Map design's walk reviews live: a valley's `walk.stills`, its
/// stops ahead and to the side (`docs/craft/WALK_REVIEW.md`).
/// `WALKS_DIR` reads them from elsewhere.
fn walks_dir() -> std::path::PathBuf {
    std::env::var_os("WALKS_DIR").map_or_else(
        || crate::asset_root_path().join("../docs/craft/walks"),
        std::path::PathBuf::from,
    )
}

/// The walks' weather: the reference look (drizzle), or `WALK_WEATHER`.
fn walk_weather() -> String {
    std::env::var("WALK_WEATHER").unwrap_or_else(|_| "drizzle".into())
}

/// The walks the designer takes, stop by stop, ahead and to the side,
/// in the reference weather (drizzle), through both eyes: every stop
/// within the stereo budget. A valley without a walk review is skipped.
#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_every_walk_holds_the_stereo_budget() {
    let mut over = Vec::new();
    let mut walked = 0;
    for (map, _) in extraction_core::map::VALLEYS {
        let file = walks_dir().join(map).join("walk.stills");
        if !file.exists() {
            println!("{map}: no walk review");
            continue;
        }
        walked += 1;
        over.extend(
            holds_views(map, &walk_weather(), |zone| {
                crate::tour::scripted_views(zone, &file)
            })
            .into_iter()
            .map(|o| format!("{map} {o}")),
        );
    }
    println!("{walked} walks");
    assert!(
        over.is_empty(),
        "stops over the frame budget:\n{}",
        over.join("\n")
    );
}

/// The yard's stations, where a newcomer's first minutes go: from in
/// front of each post, looking at it and turned a quarter to the right;
/// the truck's (the kit, the settings, calling it) from where it drops
/// you, the same two ways.
fn station_views(zone: &crate::world::Zone) -> Vec<crate::tour::View> {
    use extraction_core::map::MarkerKind;
    let eye = |x: f32, z: f32| Vec3::new(x, zone.ground(x, z) + 1.7, z);
    let mut out = Vec::new();
    let mut two = |name: String, at: Vec3, yaw: f32| {
        for (side, a) in [("ahead", yaw), ("side", yaw - 90.0)] {
            let r = a.to_radians();
            // Marker convention: 0 faces +Z, 90 faces -X.
            let dir = Vec3::new(-r.sin(), 0.0, r.cos());
            out.push(crate::tour::View {
                file: format!("{name}_{side}.png"),
                what: name.clone(),
                at: at.to_array(),
                look: (at + dir * 10.0).to_array(),
            });
        }
    };
    if let Some(m) = zone.map.markers_of(&MarkerKind::Insertion).next() {
        two("truck".into(), eye(m.at[0], m.at[1]), m.rot + 180.0);
    }
    for (i, st) in crate::onboarding::STATIONS.iter().enumerate() {
        if st.at == [0.0, 0.0] {
            continue;
        }
        let r = st.rot.to_radians();
        let facing = Vec3::new(-r.sin(), 0.0, r.cos());
        let stand = Vec3::new(st.at[0], 0.0, st.at[1]) + facing * 2.0;
        // Looking back at the board, the way its face looks out reversed.
        two(format!("st{i:02}"), eye(stand.x, stand.z), st.rot + 180.0);
    }
    out
}

/// The yard's stations through both eyes, in its weather: within the
/// stereo budget where a newcomer spends their first minutes.
#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_yard_stations_hold_the_stereo_budget() {
    let over = holds_views(crate::camp::TRAINING_YARD, "drizzle", station_views);
    assert!(
        over.is_empty(),
        "yard stations over the frame budget:\n{}",
        over.join("\n")
    );
}

#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_marrow_creek_holds_the_stereo_budget() {
    holds("marrow_creek");
}

/// Every valley, both eyes (the gauge tier).
#[test]
#[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
fn gauge_every_valley_holds_the_stereo_budget() {
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

/// Puffs where they stack: placed as each emitter keeps them in steady
/// state (`robots::fx`'s rates, lives and growth), through both eyes in
/// the reference weather. A puff is a 1.6 m quad, blended, so each one
/// across the view is a whole layer of fragments.
mod puffs {
    use super::*;
    use crate::render::puff::PuffAssets;

    /// (name, the eye, where it looks, the puffs: centre and scale)
    pub(super) type Scene = (&'static str, Vec3, Vec3, Vec<(Vec3, f32)>);

    /// A puff's scale at `age` of `life` seconds, grown from `size` as
    /// `robots::fx::advance_puffs` grows it.
    fn grown(size: f32, age: f32, life: f32) -> f32 {
        let left = life - age;
        let fade = (left / (life * 0.3)).min(1.0);
        size * (0.4 + 2.2 * age / life) * fade.sqrt()
    }

    /// A tiny LCG, for scatter that's the same each run.
    struct Dice(u32);
    impl Dice {
        fn f(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (self.0 >> 8) as f32 / (1u32 << 24) as f32
        }
    }

    /// The scenes, from the eye at `eye` looking along `fwd` (level).
    pub(super) fn scenes(eye: Vec3, fwd: Vec3) -> Vec<Scene> {
        let fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::NEG_Z);
        let side = fwd.cross(Vec3::Y);
        let ground = eye.y - 1.7;
        let look = eye + fwd * 10.0;
        let mut d = Dice(7);
        let mut out = Vec::new();
        // A fresh wreck smouldering 3 m ahead: a puff every 0.12-0.24 s,
        // each 2.5-4 s, rising 0.5-0.8 m/s, size 0.6.
        let wreck = eye + fwd * 3.0 - Vec3::Y * 1.5;
        let mut column = Vec::new();
        let mut age = 0.0;
        while age < 3.25 {
            let rise = 0.65 * age;
            let jitter = (Vec3::new(d.f(), 0.0, d.f()) - 0.5) * 0.36 * age;
            column.push((
                wreck + Vec3::Y * (0.25 + rise) + jitter,
                grown(0.6, age, 3.25),
            ));
            age += 0.18;
        }
        out.push(("a wreck smoking 3 m ahead", eye, look, column));
        // Inside a hazard's cloud: a struck robot 1.5 m ahead, its cloud
        // at the cap (64 puffs, 1-1.8 s each, size 0.7, gas's slow rise
        // and spread).
        let robot = eye + fwd * 1.5 - Vec3::Y * 0.7;
        let cloud: Vec<(Vec3, f32)> = (0..crate::robots::fx::MAX_PUFFS)
            .map(|i| {
                let life = 1.0 + d.f() * 0.8;
                let age = life * (i as f32 + 0.5) / crate::robots::fx::MAX_PUFFS as f32;
                let dir = Vec3::new(d.f() - 0.5, 0.0, d.f() - 0.5).normalize_or(Vec3::X);
                let at = robot + dir * 0.3 + (Vec3::Y * 0.15 + dir * 0.4) * age;
                (at, grown(0.7, age, life))
            })
            .collect();
        out.push(("standing in a hazard's cloud", eye, look, cloud));
        // A firefight: the cap's 64 puffs off wounds on robots 6-15 m
        // ahead, spread across the view (size 0.6, drawn bigger with
        // range as `at_range` does past 10 m).
        let fight: Vec<(Vec3, f32)> = (0..crate::robots::fx::MAX_PUFFS)
            .map(|i| {
                let r = 6.0 + d.f() * 9.0;
                let at = eye
                    + fwd * r
                    + side * (d.f() - 0.5) * r * 1.2
                    + Vec3::Y * (ground + 0.8 + d.f() * 1.2 - eye.y);
                let age = 3.25 * (i as f32 + 0.5) / crate::robots::fx::MAX_PUFFS as f32;
                (at, grown(0.6 * (r / 10.0).clamp(1.0, 4.0), age, 3.25))
            })
            .collect();
        out.push(("a firefight's wounds 6-15 m ahead", eye, look, fight));
        // A chimney smoking (`world::cues`) 6 m ahead and 3 m up, a storm
        // blowing its plume back over the eye: every puff of its loop.
        let top = eye + fwd * 6.0 + Vec3::Y * 1.3;
        let gale = -fwd * 2.5;
        let plume: Vec<(Vec3, f32)> = (0..crate::world::cues::PUFFS)
            .map(|i| {
                let age =
                    crate::world::cues::LIFE * (i as f32 + 0.5) / crate::world::cues::PUFFS as f32;
                let (off, scale, _) = crate::world::cues::puff_at(age, i as f32, gale);
                (top + off, scale)
            })
            .collect();
        out.push(("a chimney's plume blown over the eye", eye, top, plume));
        out
    }

    /// Spawns a scene's puffs (the game's quad and a blended smoke).
    pub(super) fn spawn(app: &mut App, puffs: &[(Vec3, f32)]) -> Vec<Entity> {
        let w = app.world_mut();
        let a = w.resource::<PuffAssets>().clone();
        let mat = w
            .resource_mut::<Assets<StandardMaterial>>()
            .add(a.material(Color::srgba(0.4, 0.4, 0.4, 0.5), true));
        puffs
            .iter()
            .map(|(at, s)| {
                w.spawn((
                    crate::render::puff::bundle(&a, mat.clone()),
                    Transform::from_translation(*at).with_scale(Vec3::splat(*s)),
                ))
                .id()
            })
            .collect()
    }
}

/// Where puffs stack (a wreck smoking at 3 m, standing in a hazard's
/// cloud, a firefight's wounds), both eyes: the frame's shaded fragments
/// within the stereo budget, from an open view in the reference weather.
///
/// First measure (2026-11-01, Marrow's meadow, 20.6M bare): the wreck
/// 21.8M, the firefight 20.8M, the hazard's cloud 82.2M of 64.8M (p95 14
/// to 39 layers: each near quad covers most of the view). `PUFF_RULE`
/// tries a rule on the scenes' puffs before they're drawn: `inside`
/// hides one the eye is in (82.2M: none is), `angle:<deg>` caps a puff's
/// apparent half-width (20: 60.3M; 12: 38.0M), measured before
/// `render::puff` faced the right eye. Now `face_puffs` caps each puff at
/// `MAX_HALF_ANGLE` from the eye. At 12 degrees: the wreck 27.5M, the
/// firefight 24.5M, the cloud 60.9M. Those were all taken with the sky
/// dome centred on another camera (an image camera taken for the eye,
/// until `render::mark_offscreen`): with its layer counted the bare view is
/// 27.0M, and the cloud at 12 degrees 67.2M. The budget stands; the cloud
/// keeps at least 5% under it, so the cap is 10 degrees (11 gave 61.7M):
/// the wreck 32.2M, the firefight 30.8M, the cloud 56.4M
/// (`docs/PERFORMANCE.md`).
#[test]
fn gauge_puffs_hold_the_stereo_budget() {
    let _counting = super::budget::tests::counted();
    let mut app = super::budget::tests::client_threads("marrow_creek", "drizzle", None);
    crate::render::puff::add_to(&mut app);
    for _ in 0..300 {
        app.update();
    }
    let eyes = both_eyes(&mut app);
    // The meadow, looking up the valley (m03's spot, level).
    let zone = app.world().resource::<crate::world::Zone>().clone();
    let (x, z) = (92.9, 131.9);
    let eye = Vec3::new(x, zone.ground(x, z) + 1.7, z);
    let fwd = Quat::from_rotation_y(-315f32.to_radians()) * Vec3::NEG_Z;
    let b = budget();
    let px = eye_px();
    let fragments = |app: &mut App| {
        let od_l = super::overdraw::measure(app.world_mut(), eyes.0);
        let od_r = super::overdraw::measure(app.world_mut(), eyes.1);
        (
            (od_l.mean + od_r.mean) * px * px / 1e6,
            od_l.p95.max(od_r.p95),
        )
    };
    let place = |app: &mut App, eye: Vec3, look: Vec3| {
        let w = app.world_mut();
        let mut q =
            w.query_filtered::<(&mut Transform, &mut Projection), With<super::FreeCamera>>();
        let (mut t, mut p) = q.single_mut(w).unwrap();
        *t = Transform::from_translation(eye).looking_at(look, Vec3::Y);
        if let Projection::Perspective(p) = &mut *p {
            p.fov = super::budget::EYE_FOV_DEG.to_radians();
            p.aspect_ratio = 1.0;
        }
    };
    let mut over = Vec::new();
    for (name, eye, look, scene) in puffs::scenes(eye, fwd) {
        place(&mut app, eye, look);
        for _ in 0..60 {
            app.update();
        }
        let (bare, bare_p95) = fragments(&mut app);
        // Trial rules (PUFF_RULE): "inside" hides a puff the eye is in;
        // "angle:<deg>" caps a puff's apparent half-width at that angle.
        let rule = std::env::var("PUFF_RULE").unwrap_or_default();
        let half = crate::render::puff::QUAD * 0.5;
        let scene: Vec<(Vec3, f32)> = scene
            .into_iter()
            .filter_map(|(at, sc)| {
                let d = at.distance(eye);
                let mut sc = sc;
                for r in rule.split(',') {
                    if r == "inside" && d < half * sc {
                        return None;
                    }
                    if let Some(deg) = r.strip_prefix("angle:").and_then(|v| v.parse::<f32>().ok())
                    {
                        sc = sc.min(d * deg.to_radians().tan() / half);
                    }
                }
                Some((at, sc))
            })
            .collect();
        let spawned = puffs::spawn(&mut app, &scene);
        for _ in 0..3 {
            app.update();
        }
        let (with, p95) = fragments(&mut app);
        println!(
            "{name}: {} puffs, {bare:.1}M fragments > {with:.1}M of {:.1}M (p95 {bare_p95} > {p95} layers)",
            scene.len(),
            b.fragments_m
        );
        if with > b.fragments_m {
            over.push(format!(
                "{name}: {with:.1}M fragments > {:.1}M",
                b.fragments_m
            ));
        }
        for e in spawned {
            app.world_mut().despawn(e);
        }
    }
    assert!(
        over.is_empty(),
        "puffs over the frame budget:\n{}",
        over.join("\n")
    );
}
