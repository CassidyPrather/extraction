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

/// A light's beam (a child of the light), and its own material: each
/// beam's lamp, throw and air go to the shader.
#[derive(Component)]
struct Beam(Handle<FogBeamMaterial>);

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
        let tan = spot.outer_angle.tan();
        // (Placed by `throw_beams` once the light is where it is.)
        let material = materials.add(FogBeamMaterial {
            params: FogBeamParams {
                light: Vec4::new(c.red, c.green, c.blue, radiance(spot.intensity)),
                apex: Vec3::ZERO.extend(tan),
                axis: Vec3::NEG_Z.extend(REACH),
                air: Vec4::new(0.5, 0.0, -1000.0, 0.0),
            },
            inside: false,
        });
        commands.entity(e).insert(Throw(REACH)).with_child((
            Name::new("Torch beam"),
            Beam(material.clone()),
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
            let Some(m) = materials.get(&beam.0) else {
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
            let p = FogBeamParams {
                light: m.params.light,
                apex: lamp.extend(tan),
                axis: dir.extend(len),
                air: Vec4::new(density, day, ground, fog_beam::LAMP_SPREAD),
            };
            let inside = eye.is_some_and(|e| fog_beam::eye_inside(e, lamp, dir, tan, len));
            // The material changes only when the beam does (a held torch
            // moves most frames; one standing on a ledge doesn't).
            let q = m.params;
            let changed = (q.apex - p.apex).length() > 0.01
                || (q.axis - p.axis).length() > 0.002
                || (q.air - p.air).length() > 0.02
                || m.inside != inside;
            if changed {
                if let Some(mut m) = materials.get_mut(&beam.0) {
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
}
