//! `--wayfind DIR`: the render-based wayfinding gauge. From where you get
//! your bearings (the truck's stop, every rendezvous: standing, eyes
//! 1.7 m up), the free camera turns to each landmark in range (a
//! placement at least `map_lint::LANDMARK_HEIGHT` tall, between
//! [`NEAREST`] and [`FARTHEST`]) with the headset's pixels a degree, and
//! shoots it twice: as it is, and with the landmark hidden. What differs
//! is the landmark as the eye gets it, through fog, trees and the dark:
//! `scripts/wayfind.py` measures its size and contrast and says whether
//! you could steer by it. Writes `wayfind.json` beside the pictures.
//!
//! `--afford DIR` does the same for interactables (`render::affordance`):
//! up to three of each kind on the map, from 1.5 m in front of it at eye
//! height, with and without it: does it stand out from what it's on?

use bevy::camera::Camera;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use extraction_core::map::{MarkerKind, prefab_footprint};
use extraction_core::map_lint::LANDMARK_HEIGHT;

use super::FreeCamera;
use super::legibility::eye_px_per_deg;
use crate::launch::LaunchOptions;
use crate::world::Zone;
use crate::world::buildings::Building;

/// Landmarks nearer than this are what you're standing at, not steering
/// by (m). Low enough to take an RV's own tower (Cinder's fire lookout
/// stands 14.8 m from its ranger tower).
pub const NEAREST: f32 = 5.0;

/// How far out landmarks are shot (m): past the map lint's clear sight
/// (200 m in drizzle), out to where a faint twin still confuses
/// (`map_lint`'s uniqueness range), to see how far they really carry.
pub const FARTHEST: f32 = 320.0;

pub struct WayfindShotsPlugin;

impl Plugin for WayfindShotsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shots>()
            .add_systems(Update, (plan_afford, shoot).chain());
    }
}

#[derive(serde::Serialize, Clone)]
struct View {
    /// Where from: "truck" or the rendezvous' name.
    from: String,
    landmark: String,
    distance: f32,
    eye: [f32; 3],
    look: [f32; 3],
    /// With the landmark, and without it.
    with: String,
    without: String,
    /// The landmark's box in the picture: x, y, w, h (pixels).
    rect: [f32; 4],
    #[serde(skip)]
    entity: Option<Entity>,
    /// Its footprint (w, h, d) and pose, for `rect`.
    #[serde(skip)]
    size: Vec3,
    #[serde(skip)]
    pose: Transform,
}

#[derive(Resource, Default)]
struct Shots {
    views: Option<Vec<View>>,
    next: usize,
    /// 0: with the landmark; 1: without it.
    phase: u8,
    wait: u32,
    shooting: bool,
    /// Frames spent on this shot, whatever else is going on: a thing whose
    /// visibility another system also sets doesn't hold the gauge up.
    spent: u32,
    /// The landmark hidden for the last "without" shot.
    hidden: Option<Entity>,
}

const SETTLE: u32 = 8;
const EYE: f32 = 1.7;

/// For `--afford`: up to three of each interactable kind, each from 1.5 m
/// in front of it (its +Z, level) at eye height or a little over it.
fn plan_afford(world: &mut World, mut frames: Local<u32>) {
    let Some(dir) = world.resource::<LaunchOptions>().afford.clone() else {
        return;
    };
    *frames += 1;
    if world.resource::<Shots>().views.is_some()
        || *frames < world.resource::<LaunchOptions>().frames
    {
        return;
    }
    let Some(zone) = world.get_resource::<Zone>().cloned() else {
        return;
    };
    let mut picked: Vec<(super::affordance::Kind, Entity, Transform, Vec3)> = Vec::new();
    let mut found: Vec<(super::affordance::Kind, Entity)> = super::affordance::traits(world)
        .into_iter()
        .filter_map(|(e, t)| match super::affordance::kind_of(&t) {
            Ok(Some(k)) if k != super::affordance::Kind::OwnPack => Some((k, e)),
            _ => None,
        })
        .collect();
    found.sort_by_key(|(k, e)| (*k, *e));
    for (k, e) in found {
        if picked.iter().filter(|p| p.0 == k).count() >= 3 {
            continue;
        }
        let Some(gt) = world.get::<GlobalTransform>(e) else {
            continue;
        };
        // Not what's hidden for now (the order card until there's an order).
        if world
            .get::<InheritedVisibility>(e)
            .is_some_and(|v| !v.get())
        {
            continue;
        }
        let pose = gt.compute_transform();
        let size = world
            .get::<avian3d::prelude::ColliderAabb>(e)
            .map(|a| a.max - a.min)
            .unwrap_or(Vec3::splat(0.3))
            .min(Vec3::splat(3.0));
        picked.push((k, e, pose, size));
    }
    let mut spatial = bevy::ecs::system::SystemState::<avian3d::prelude::SpatialQuery>::new(world);
    let mut v = Vec::new();
    for (k, e, pose, size) in picked {
        let centre = pose.translation;
        let eye = vantage(world, &mut spatial, &zone, e, pose, size);
        let i = v.len();
        v.push(View {
            from: format!("{k:?}"),
            landmark: world
                .get::<Name>(e)
                .map_or(format!("{e}"), |n| n.to_string()),
            distance: eye.distance(centre),
            eye: eye.to_array(),
            look: centre.to_array(),
            with: format!("{i:03}_a.png"),
            without: format!("{i:03}_b.png"),
            rect: [0.0; 4],
            entity: Some(e),
            // Centred on the thing (its box goes round its middle).
            size,
            pose: Transform::from_translation(centre - Vec3::Y * size.y * 0.5),
        });
    }
    let _ = std::fs::create_dir_all(&dir);
    world.resource_mut::<Time<Virtual>>().pause();
    info!("afford: {} views", v.len());
    world.resource_mut::<Shots>().views = Some(v);
}

/// Where to stand to see an interactable: [`super::affordance::SEEN_FROM`]
/// out from its front, back or either side, at eye height (or just above
/// it), whichever first sees it with nothing in between. Its own +Z
/// alone put the camera behind the panel a button sits on, or in a wall.
fn vantage(
    world: &World,
    spatial: &mut bevy::ecs::system::SystemState<avian3d::prelude::SpatialQuery>,
    zone: &Zone,
    e: Entity,
    pose: Transform,
    size: Vec3,
) -> Vec3 {
    let centre = pose.translation;
    let fwd = pose.rotation * Vec3::Z;
    let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(Vec3::X);
    let eye_at = |dir: Vec3| {
        let mut eye = centre + dir * super::affordance::SEEN_FROM;
        eye.y = (zone.ground(centre.x, centre.z) + EYE).max(centre.y + 0.2);
        eye
    };
    let sides = [flat, -flat, flat.cross(Vec3::Y), -flat.cross(Vec3::Y)];
    let Ok(query) = spatial.get(world) else {
        return eye_at(flat);
    };
    let related = |a: Entity, b: Entity| {
        let up = |mut x: Entity, to: Entity| loop {
            if x == to {
                return true;
            }
            match world.get::<ChildOf>(x) {
                Some(c) => x = c.parent(),
                None => return false,
            }
        };
        up(a, b) || up(b, a)
    };
    let clear = |eye: Vec3| {
        let Ok(dir) = Dir3::new(centre - eye) else {
            return false;
        };
        let d = eye.distance(centre);
        match query.cast_ray(eye, dir, d, true, &Default::default()) {
            None => true,
            Some(hit) => related(hit.entity, e) || hit.distance >= d - size.length() * 0.5,
        }
    };
    sides
        .into_iter()
        .map(eye_at)
        .find(|eye| clear(*eye))
        .unwrap_or_else(|| eye_at(flat))
}

#[allow(clippy::too_many_arguments)]
fn shoot(
    mut commands: Commands,
    opts: Res<LaunchOptions>,
    zone: Option<Res<Zone>>,
    mut state: ResMut<Shots>,
    buildings: Query<(Entity, &Building, &GlobalTransform)>,
    mut vis: Query<&mut Visibility>,
    mut cams: Query<(&mut Transform, &mut Projection, &Camera), With<FreeCamera>>,
    mut time: ResMut<Time<Virtual>>,
    windows: Query<&Window>,
    mut adapt: ResMut<super::grade::Adaptation>,
    weather: Res<crate::world::sky::Weather>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let sun_dir = weather.sun_dir;
    let Some(zone) = zone else {
        return;
    };
    let Some(dir) = opts.wayfind.as_ref().or(opts.afford.as_ref()) else {
        return;
    };
    // `plan_afford` fills the views for `--afford`.
    if opts.afford.is_some() && state.views.is_none() {
        return;
    }
    *frames += 1;
    if *frames < opts.frames || state.shooting {
        return;
    }
    let state = &mut *state;
    let views = state.views.get_or_insert_with(|| {
        let mut from: Vec<(String, Vec3)> = Vec::new();
        for m in &zone.map.markers {
            // Where the truck sets you down (its stop is inside the truck).
            let name = match m.kind {
                MarkerKind::Insertion => "truck".to_string(),
                MarkerKind::Rendezvous => format!("rv {}", m.name),
                _ => continue,
            };
            from.push((name, zone.on_ground(m.at[0], m.at[1]) + Vec3::Y * EYE));
        }
        let marks: Vec<(Entity, String, Transform, Vec3)> = buildings
            .iter()
            .filter_map(|(e, b, g)| {
                let f = prefab_footprint(&b.prefab);
                (f[1] >= LANDMARK_HEIGHT)
                    .then(|| (e, b.prefab.clone(), g.compute_transform(), Vec3::from(f)))
            })
            .collect();
        let mut v = Vec::new();
        for (name, eye) in &from {
            for (e, prefab, pose, size) in &marks {
                let look = pose.translation + Vec3::Y * size.y * 0.6;
                let d = look.xz().distance(eye.xz());
                if !(NEAREST..=FARTHEST).contains(&d) {
                    continue;
                }
                let i = v.len();
                v.push(View {
                    from: name.clone(),
                    landmark: prefab.clone(),
                    distance: d,
                    eye: eye.to_array(),
                    look: look.to_array(),
                    with: format!("{i:03}_a.png"),
                    without: format!("{i:03}_b.png"),
                    rect: [0.0; 4],
                    entity: Some(*e),
                    size: *size,
                    pose: *pose,
                });
            }
        }
        // The sky as a compass: from where the truck sets you down, the
        // sky toward the sun (or the moon, by night: the light's the
        // moon's then) and away from it, low over the horizon.
        if let Some((_, eye)) = from.iter().find(|(n, _)| n == "truck") {
            let toward = -sun_dir;
            let flat = Vec3::new(toward.x, 0.0, toward.z).normalize_or(Vec3::X);
            for (what, d) in [("sky toward the light", flat), ("sky away from it", -flat)] {
                let i = v.len();
                let look = *eye + (d + Vec3::Y * 0.35) * 100.0;
                v.push(View {
                    from: "sky".into(),
                    landmark: what.into(),
                    distance: 0.0,
                    eye: eye.to_array(),
                    look: look.to_array(),
                    with: format!("{i:03}_a.png"),
                    without: format!("{i:03}_b.png"),
                    rect: [0.0; 4],
                    entity: None,
                    size: Vec3::ZERO,
                    pose: Transform::IDENTITY,
                });
            }
        }
        let _ = std::fs::create_dir_all(dir);
        // Nothing moves between a view's two shots (rain, grass, a
        // burner's flicker): only the landmark differs.
        time.pause();
        info!("wayfind: {} views from {} places", v.len(), from.len());
        v
    });
    let Some(view) = views.get(state.next).cloned() else {
        if let Ok(json) = serde_json::to_string_pretty(&views) {
            let _ = std::fs::write(dir.join("wayfind.json"), json);
        }
        exit.write(AppExit::Success);
        return;
    };
    let Ok((mut cam, mut proj, camera)) = cams.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let want = Transform::from_translation(Vec3::from_array(view.eye))
        .looking_at(Vec3::from_array(view.look), Vec3::Y);
    let fov = (window.height() / eye_px_per_deg()).to_radians();
    if let Projection::Perspective(p) = &mut *proj {
        if (p.fov - fov).abs() > 1e-4 {
            p.fov = fov;
        }
    }
    if *cam != want {
        *cam = want;
        state.wait = 0;
    }
    // The last view's landmark back; this one's shown for the first shot,
    // hidden for the second.
    if state.phase == 0 {
        if let Some(mut v) = state.hidden.take().and_then(|e| vis.get_mut(e).ok()) {
            *v = Visibility::Inherited;
        }
    } else {
        state.hidden = view.entity;
    }
    if let Some(mut v) = view.entity.and_then(|e| vis.get_mut(e).ok()) {
        let want = if state.phase == 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    state.wait += 1;
    state.spent += 1;
    if state.wait < SETTLE - 2 {
        adapt.resettle();
    }
    if state.wait < SETTLE && state.spent < 120 {
        return;
    }
    state.spent = 0;
    // Where the landmark's box lands in the picture.
    if state.phase == 0 {
        let cam_gt = GlobalTransform::from(want);
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        let s = view.size;
        for c in 0..8 {
            let local = Vec3::new(
                if c & 1 == 0 { -0.5 } else { 0.5 } * s.x,
                // Up past its top, for the lamp on it (`render::landmarks`).
                if c & 2 == 0 { 0.0 } else { s.y + 1.5 },
                if c & 4 == 0 { -0.5 } else { 0.5 } * s.z,
            );
            if let Ok(p) = camera.world_to_viewport(&cam_gt, view.pose.transform_point(local)) {
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
        let (w, h) = (window.width(), window.height());
        let lo = lo.max(Vec2::ZERO);
        let hi = hi.min(Vec2::new(w, h));
        views[state.next].rect = [lo.x, lo.y, (hi.x - lo.x).max(0.0), (hi.y - lo.y).max(0.0)];
    }
    state.shooting = true;
    let file = if state.phase == 0 {
        view.with.clone()
    } else {
        view.without.clone()
    };
    let mut save = save_to_disk(dir.join(file));
    commands.spawn(Screenshot::primary_window()).observe(
        move |shot: On<ScreenshotCaptured>, mut state: ResMut<Shots>| {
            save(shot);
            state.shooting = false;
            state.wait = 0;
            if state.phase == 0 {
                state.phase = 1;
            } else {
                state.phase = 0;
                state.next += 1;
            }
        },
    );
}
