//! Torch beams: every spotlight (the torch gadget's, the screenshot
//! torch, a drone's belly lamp, a sentry's) gets its beam as light in the
//! air (`render::fog_beam`, the searchlights' look): soft to its wall,
//! thinning down its throw, slow billows drifting in it, brighter looking
//! up it at the lamp. Cut short where the light hits something so it
//! doesn't poke through walls. The air decides how much shows: a little
//! dust on a dry night, far more in fog and in rain.

use avian3d::prelude::*;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

use super::weather::{Conditions, WeatherKind};
use crate::render::fog_beam::{self, FogBeamMaterial, FogBeamParams};

pub struct BeamsPlugin;

impl Plugin for BeamsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (add_beams, throw_beams).chain());
    }
}

/// The longest beam drawn (m): past that the light's too spread to show.
pub const REACH: f32 = 14.0;

/// A light's beam (a child of the light), its own material (each beam's
/// lamp, throw and air go to the shader), and how strong its lamp is.
#[derive(Component)]
struct Beam {
    material: Handle<FogBeamMaterial>,
    strength: f32,
}

/// The colour a lamp's haze takes: cold, whatever the lamp. A warm glow in
/// the air reads as a shelter's light (the lit tells); the lamp's own
/// colour stays on what it lights.
const HAZE: Vec3 = Vec3::new(0.85, 0.92, 1.0);

/// A lamp this near the eye is in your hand (a torch): it shows by what it
/// lights, with only a faint shaft, never a haze over where you point it.
const AT_HAND: f32 = 1.5;
const HAND_SHAFT: f32 = 0.2;

/// Lamps whose beams come this near the eye share one haze between them:
/// three drones and a torch over you light the air no more than two lamps
/// would.
const NEAR_EYE: f32 = 8.0;
const SHARED_BY: f32 = 2.0;

/// How much a lamp's haze shows for the air: next to nothing on a clear
/// night, half in drizzle, all of it in fog. (A searchlight's long throw
/// shows on any night; a lamp's few metres need real wet air.)
pub fn lamp_haze(c: &Conditions) -> f32 {
    ((air(c).0 - 0.2) / 0.6).clamp(0.05, 1.0)
}

/// The nearest a beam from `lamp` along `dir` (unit), `len` long, comes to
/// `eye`.
fn reach_to(eye: Vec3, lamp: Vec3, dir: Vec3, len: f32) -> f32 {
    let t = (eye - lamp).dot(dir).clamp(0.0, len);
    eye.distance(lamp + dir * t)
}

/// Where a light's beam lands (m, from the light; `REACH` when it finds
/// nothing), settled over a few frames: on the light. A held torch on a
/// surface at arm's length lights the eye far more than one on the floor
/// across a room (`render::grade::torch_near`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Throw(pub f32);

/// How much the air shows a beam, for the weather: dust on a dry night,
/// mist, fog's droplets, rain's streaks.
pub fn air(c: &Conditions) -> (f32, f32) {
    let scatter = match c.kind {
        WeatherKind::Clearing => 0.25,
        WeatherKind::Drizzle => 0.5,
        WeatherKind::Downpour => 0.7,
        WeatherKind::PeaSouper => 1.0,
    };
    // Ground fog thickens it too (it's most of what a torch lights).
    let mist = (c.ground_fog / 0.02).clamp(0.0, 1.0) * 0.3;
    ((scatter + mist).min(1.2), c.rain)
}

fn add_beams(
    mut commands: Commands,
    lights: Query<(Entity, &SpotLight), Added<SpotLight>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogBeamMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    if std::env::var_os("EXTRACTION_NO_BEAMS").is_some() || fog_beam::hidden() {
        return;
    }
    for (e, spot) in &lights {
        let mesh = mesh
            .get_or_insert_with(|| meshes.add(fog_beam::cone()))
            .clone();
        let c = spot.color.to_linear();
        let haze = Vec3::new(c.red, c.green, c.blue).lerp(HAZE, 0.75);
        let tan = spot.outer_angle.tan();
        let strength = radiance(spot.intensity);
        // (Placed by `throw_beams` once the light is where it is.)
        let material = materials.add(FogBeamMaterial {
            params: FogBeamParams {
                light: haze.extend(strength),
                apex: Vec3::ZERO.extend(tan),
                axis: Vec3::NEG_Z.extend(REACH),
                air: Vec4::new(0.5, 0.0, -1000.0, 0.0),
            },
            inside: false,
        });
        commands.entity(e).insert(Throw(REACH)).with_child((
            Name::new("Torch beam"),
            Beam {
                material: material.clone(),
                strength,
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            beam_transform(tan, REACH),
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
}

/// The lit air's strength for a light this strong: a searchlight's is 1,
/// a torch's 250 klm half that (its beam is at the eye, over most of the
/// view), a drone's belly lamp a little more.
fn radiance(lumens: f32) -> f32 {
    (lumens / 500_000.0).clamp(0.2, 0.8)
}

fn beam_transform(tan: f32, length: f32) -> Transform {
    let r = tan * length;
    Transform::from_scale(Vec3::new(r, r, length))
}

/// Each beam stops where its light lands (one ray down it a frame), and
/// its lamp, throw, air and face go to its material.
#[allow(clippy::too_many_arguments)]
fn throw_beams(
    mut lights: Query<(&GlobalTransform, &Children, Option<&mut Throw>), With<SpotLight>>,
    mut beams: Query<(&Beam, &mut Transform)>,
    mut materials: ResMut<Assets<FogBeamMaterial>>,
    spatial: SpatialQuery,
    zone: Option<Res<super::Zone>>,
    conditions: Res<Conditions>,
    weather: Option<Res<super::sky::Weather>>,
    eyes: crate::render::puff::Eyes,
) {
    let eye = crate::render::puff::eye(&eyes);
    let density = fog_beam::air_density(&conditions);
    let wet = lamp_haze(&conditions);
    // How many lamps' beams come near the eye (last frame's throws): they
    // share one haze.
    let near = eye.map_or(0, |e| {
        lights
            .iter()
            .filter(|(gt, _, throw)| {
                let len = throw.map_or(REACH, |t| t.0);
                reach_to(e, gt.translation(), gt.forward().as_vec3(), len) < NEAR_EYE
            })
            .count()
    });
    let share = (SHARED_BY / near.max(1) as f32).min(1.0);
    let day = weather
        .as_ref()
        .map_or(0.0, |w| 1.0 - w.night.clamp(0.0, 1.0));
    for (gt, children, mut throw) in &mut lights {
        for c in children.iter() {
            let Ok((beam, mut xf)) = beams.get_mut(c) else {
                continue;
            };
            let fwd = gt.forward();
            let lamp = gt.translation();
            // Start clear of the hand that holds it.
            let from = lamp + fwd * 0.3;
            let landed = spatial
                .cast_ray(from, fwd, REACH, true, &SpatialQueryFilter::default())
                .map(|h| h.distance + 0.3)
                .unwrap_or(REACH);
            if let Some(t) = throw.as_deref_mut() {
                let to = t.0 + (landed - t.0) * 0.5;
                if (t.0 - to).abs() > 0.01 {
                    t.0 = to;
                }
            }
            let hit = landed.max(0.8);
            // Settle, don't snap: a ray grazing an edge flickers.
            let len = xf.scale.z + (hit - xf.scale.z) * 0.5;
            let Some(m) = materials.get(&beam.material) else {
                continue;
            };
            let tan = m.params.apex.w;
            let want = beam_transform(tan, len);
            if (xf.scale - want.scale).length() > 0.01 {
                *xf = want;
            }
            let dir = fwd.as_vec3();
            let end = lamp + dir * len;
            // The ground where it lands (the fade into it): the valley's,
            // or what it landed on when that's lower (a cellar, a ditch).
            let ground = zone
                .as_ref()
                .map_or(end.y, |z| z.ground(end.x, end.z).min(end.y));
            // A torch in your hand: a faint shaft. Near the eye: its share.
            let at_hand = eye.is_some_and(|e| e.distance(lamp) < AT_HAND);
            let close = eye.is_some_and(|e| reach_to(e, lamp, dir, len) < NEAR_EYE);
            let strength = beam.strength
                * wet
                * if at_hand { HAND_SHAFT } else { 1.0 }
                * if close { share } else { 1.0 };
            let p = FogBeamParams {
                light: m.params.light.truncate().extend(strength),
                apex: lamp.extend(tan),
                axis: dir.extend(len),
                air: Vec4::new(density, day, ground, fog_beam::LAMP_SPREAD),
            };
            let inside = eye.is_some_and(|e| fog_beam::eye_inside(e, lamp, dir, tan, len));
            // The material changes only when the beam does (a held torch
            // moves most frames; one standing on a ledge doesn't).
            let q = m.params;
            let changed = (q.apex - p.apex).length() > 0.01
                || (q.light.w - p.light.w).abs() > 0.01
                || (q.axis - p.axis).length() > 0.002
                || (q.air - p.air).length() > 0.02
                || m.inside != inside;
            if changed {
                if let Some(mut m) = materials.get_mut(&beam.material) {
                    m.params = p;
                    m.inside = inside;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_and_rain_show_a_beam_more_than_a_clear_night() {
        use super::super::weather::Hour;
        let at = |k| air(&Conditions::compose(k, Hour::Night, 4)).0;
        assert!(at(WeatherKind::PeaSouper) > at(WeatherKind::Drizzle));
        assert!(at(WeatherKind::Downpour) > at(WeatherKind::Clearing));
        assert!(at(WeatherKind::Clearing) > 0.0, "dust shows it a little");
    }

    /// A lamp's haze is next to nothing on a clear night, and grows with
    /// the wet in the air.
    #[test]
    fn a_lamps_haze_needs_wet_air() {
        use super::super::weather::Hour;
        let at = |k| lamp_haze(&Conditions::compose(k, Hour::Night, 4));
        // (A clear night's ground mist is in it: under a third.)
        assert!(
            at(WeatherKind::Clearing) < 0.33,
            "{}",
            at(WeatherKind::Clearing)
        );
        assert!(at(WeatherKind::Drizzle) > at(WeatherKind::Clearing));
        assert!(at(WeatherKind::PeaSouper) > at(WeatherKind::Drizzle));
    }
}
