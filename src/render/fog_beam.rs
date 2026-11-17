//! Light in fog: one shared look for a lamp's beam, as light scattered in
//! hazy air (`shaders/fog_beam.wgsl`), not a solid cone. The user (#110):
//! "Have you ever seen godrays in a dusty room?"
//!
//! The cone mesh only bounds the volume; each pixel it covers marches a
//! short stretch of the view ray through it and adds what the air there
//! scatters toward the eye. Soft to the wall, thinning down the throw, a
//! soft fade where it meets the ground, dust drifting in it, brighter
//! looking up toward the lamp, as dense as the weather makes the air.
//! Premultiplied: thin haze lightens what's behind it, never greys it.
//! Cheap enough for stereo: 12 samples a pixel, only where a beam is.
//!
//! Built for the searchlights (`robots::searchlight_look`) first; the
//! drones' and sentries' night lamps, the torch, and the lamps' halos are
//! to follow onto it.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

pub struct FogBeamPlugin;

impl Plugin for FogBeamPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<FogBeamMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct FogBeamParams {
    /// rgb: the light; w: its strength (0 off).
    pub light: Vec4,
    /// xyz: the lamp (world); w: tan of the half-angle.
    pub apex: Vec4,
    /// xyz: the way it throws (world, unit); w: how far it goes (m).
    pub axis: Vec4,
    /// x: how dense the air is (0.2 clear .. 1.2 thick fog); y: daylight
    /// 0..1; z: the ground's height where it lands (world y).
    pub air: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FogBeamMaterial {
    #[uniform(0)]
    pub params: FogBeamParams,
}

impl Material for FogBeamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/fog_beam.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // (The shader adds a little light on top of what it blends.)
        AlphaMode::Premultiplied
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Both faces: from outside the front ones draw it, from inside the
        // back ones (the shader picks).
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// How dense the air is for a beam, from the weather: a clear day thin, a
/// drizzle more, fog and ground mist thick.
pub fn air_density(c: &crate::world::weather::Conditions) -> f32 {
    0.25 + 0.75 * crate::world::beams::air(c).0.min(1.2)
}

/// A unit cone, apex at the origin, opening down -Z to radius 1 at z = -1,
/// capped at its wide end (so a ray through its end still meets a face):
/// scale it (r, r, length).
pub fn cone() -> Mesh {
    const ROUND: u32 = 24;
    const ALONG: u32 = 6;
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    for j in 0..=ALONG {
        let z = j as f32 / ALONG as f32;
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
    // The end cap: a fan, facing out along -Z.
    let centre = pos.len() as u32;
    pos.push([0.0, 0.0, -1.0]);
    nrm.push([0.0, 0.0, -1.0]);
    uv.push([0.5, 1.0]);
    let rim = ALONG * (ROUND + 1);
    let first = pos.len() as u32;
    for i in 0..=ROUND {
        let p = pos[(rim + i) as usize];
        pos.push(p);
        nrm.push([0.0, 0.0, -1.0]);
        uv.push([i as f32 / ROUND as f32, 1.0]);
    }
    for i in 0..ROUND {
        idx.extend_from_slice(&[centre, first + i + 1, first + i]);
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

/// A beam with no lamp, at `at`, for the warm-up shelf (`render::warmup`)
/// to build its pipeline before any lit beam is in view.
pub(crate) fn decoy(world: &mut World, at: Vec3) -> Entity {
    let mesh = world.resource_mut::<Assets<Mesh>>().add(cone());
    let material = world
        .resource_mut::<Assets<FogBeamMaterial>>()
        .add(FogBeamMaterial {
            params: FogBeamParams {
                light: Vec4::new(1.0, 1.0, 1.0, 1.0),
                apex: at.extend(0.13),
                axis: Vec3::NEG_Y.extend(10.0),
                air: Vec4::new(0.5, 1.0, at.y - 10.0, 0.0),
            },
        });
    world
        .spawn((
            Name::new("Warm-up fog beam"),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(at).with_scale(Vec3::new(1.3, 1.3, 10.0)),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cone's end is closed (a ray down its throw from inside meets a
    /// face), and every face's normal points out of it.
    #[test]
    fn the_cone_is_closed_and_faces_out() {
        let m = cone();
        let pos = m
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let nrm = match m.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap() {
            bevy::mesh::VertexAttributeValues::Float32x3(v) => v.clone(),
            _ => panic!(),
        };
        // Some vertices on the end cap, facing down the throw.
        assert!(nrm.iter().any(|n| n[2] < -0.99));
        for (p, n) in pos.iter().zip(&nrm) {
            let p = Vec3::from(*p);
            let n = Vec3::from(*n);
            // Out of the axis, or out of the end.
            let radial = Vec3::new(p.x, p.y, 0.0);
            assert!(n.dot(radial) >= -1e-4 || n.z < -0.99, "{p} {n}");
        }
    }
}
