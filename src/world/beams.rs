//! Torch beams: every spotlight (the torch gadget's, the screenshot
//! torch) gets a soft cone of lit air down its throw, with dust drifting
//! in it. Cheap on purpose, like the shafts: one additive cone mesh per
//! light, no raymarch (`assets/shaders/beam.wgsl` shades it by how much of
//! the cone a ray crosses), cut short where the light hits something so it
//! doesn't poke through walls. The air decides how much shows: a little
//! dust on a dry night, far more in fog and in rain.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::weather::{Conditions, WeatherKind};

pub struct BeamsPlugin;

impl Plugin for BeamsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<BeamMaterial>::default())
            .add_systems(Update, (add_beams, (throw_beams, beam_air)).chain());
    }
}

/// The longest beam drawn (m): past that the light's too spread to show.
pub const REACH: f32 = 14.0;

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct BeamParams {
    /// rgb: the light's colour; w: the lit air's radiance (0 off).
    pub color: Vec4,
    /// x: how much the air scatters (dust 0.25, fog 1); y: rain; z: the
    /// cone's half-angle's tangent; w: the beam's length (m).
    pub air: Vec4,
    /// xyz: the way it throws (world).
    pub axis: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BeamMaterial {
    #[uniform(0)]
    pub params: BeamParams,
}

impl Material for BeamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/beam.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// A light's beam (a child of the light), and its own material: each
/// cone's length goes to the shader.
#[derive(Component)]
struct Beam(Handle<BeamMaterial>);

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

/// A unit cone, apex at the origin, opening down -Z to radius 1 at z = -1:
/// uv x round it, y along it.
fn cone() -> Mesh {
    const ROUND: u32 = 20;
    const ALONG: u32 = 6;
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    for j in 0..=ALONG {
        let t = j as f32 / ALONG as f32;
        // Rings closer near the apex, where it's brightest.
        let z = t * t;
        for i in 0..=ROUND {
            let a = i as f32 / ROUND as f32 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            pos.push([c * z, s * z, -z]);
            nrm.push(Vec3::new(c, s, 1.0).normalize().to_array());
            uv.push([i as f32 / ROUND as f32, z]);
        }
    }
    let mut idx = Vec::new();
    for j in 0..ALONG {
        for i in 0..ROUND {
            let a = j * (ROUND + 1) + i;
            let b = a + ROUND + 1;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

fn add_beams(
    mut commands: Commands,
    lights: Query<(Entity, &SpotLight), Added<SpotLight>>,
    conditions: Res<Conditions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BeamMaterial>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
) {
    if std::env::var_os("EXTRACTION_NO_BEAMS").is_some() {
        return;
    }
    for (e, spot) in &lights {
        let mesh = mesh.get_or_insert_with(|| meshes.add(cone())).clone();
        let c = spot.color.to_linear();
        let (scatter, rain) = air(&conditions);
        let tan = spot.outer_angle.tan();
        let material = materials.add(BeamMaterial {
            params: BeamParams {
                color: Vec4::new(c.red, c.green, c.blue, radiance(spot.intensity)),
                air: Vec4::new(scatter, rain, tan, REACH),
                axis: Vec4::NEG_Z,
            },
        });
        commands.entity(e).with_child((
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

/// The lit air's radiance for a light this strong (the torch's 250 klm
/// gives 1: the shader's scale).
fn radiance(lumens: f32) -> f32 {
    (lumens / 250_000.0).clamp(0.0, 4.0)
}

fn beam_transform(tan: f32, length: f32) -> Transform {
    let r = tan * length;
    Transform::from_scale(Vec3::new(r, r, length))
}

/// Each beam stops where its light lands: one ray down it a frame.
fn throw_beams(
    lights: Query<(&GlobalTransform, &Children), With<SpotLight>>,
    mut beams: Query<(&Beam, &mut Transform)>,
    mut materials: ResMut<Assets<BeamMaterial>>,
    spatial: SpatialQuery,
) {
    for (gt, children) in &lights {
        for c in children.iter() {
            let Ok((beam, mut xf)) = beams.get_mut(c) else {
                continue;
            };
            let fwd = gt.forward();
            // Start clear of the hand that holds it.
            let from = gt.translation() + fwd * 0.3;
            let hit = spatial
                .cast_ray(from, fwd, REACH, true, &SpatialQueryFilter::default())
                .map(|h| h.distance + 0.3)
                .unwrap_or(REACH)
                .max(0.8);
            // Settle, don't snap: a ray grazing an edge flickers.
            let len = xf.scale.z + (hit - xf.scale.z) * 0.5;
            let Some(m) = materials.get(&beam.0) else {
                continue;
            };
            let want = beam_transform(m.params.air.z, len);
            let axis = fwd.as_vec3().extend(0.0);
            let resized = (xf.scale - want.scale).length() > 0.01;
            if resized {
                *xf = want;
            }
            // The material changes only when the beam does (a held torch
            // turns most frames; one standing on a ledge doesn't).
            if resized || (m.params.axis - axis).length() > 0.002 {
                if let Some(mut m) = materials.get_mut(&beam.0) {
                    m.params.air.w = len;
                    m.params.axis = axis;
                }
            }
        }
    }
}

/// When the weather changes, so does what the beams light.
fn beam_air(
    conditions: Res<Conditions>,
    beams: Query<&Beam>,
    mut materials: ResMut<Assets<BeamMaterial>>,
) {
    if !conditions.is_changed() {
        return;
    }
    let (scatter, rain) = air(&conditions);
    for b in &beams {
        if let Some(mut m) = materials.get_mut(&b.0) {
            m.params.air.x = scatter;
            m.params.air.y = rain;
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
