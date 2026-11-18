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
//! Cheap enough for stereo: one face a pixel (from outside a beam only
//! its front faces draw, from inside only its back: `eye_inside`, picked
//! on the CPU), and 3 to 12 samples a pixel by how much beam the ray
//! crosses. `EXTRACTION_NO_FOG_BEAMS=1` hides them all, to price them.
//!
//! Built for the searchlights (`robots::searchlight_look`) first; the
//! drones' and sentries' night lamps, the torch, and the lamps' halos are
//! to follow onto it.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
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
    /// 0..1; z: the ground's height where it lands (world y); w: how fast
    /// the light spreads down the throw (0 for a searchlight's 0.0012;
    /// [`LAMP_SPREAD`] for a lamp's or a torch's).
    pub air: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
#[bind_group_data(FogBeamKey)]
pub struct FogBeamMaterial {
    #[uniform(0)]
    pub params: FogBeamParams,
    /// The eye is in the cone (`eye_inside`): its back faces draw, and
    /// the march starts at the eye. Otherwise its front faces.
    pub inside: bool,
}

/// The pipeline key: which face draws.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FogBeamKey {
    inside: bool,
}

impl From<&FogBeamMaterial> for FogBeamKey {
    fn from(m: &FogBeamMaterial) -> Self {
        Self { inside: m.inside }
    }
}

/// Whether the eye at `eye` is in a beam from `apex` along `dir` (unit),
/// `tan` its half-angle's tangent, `length` long: then its back faces
/// draw. Generous at the wall (each eye of a pair, and an eye just inside
/// whose front faces would be behind it and the beam lost); an eye just
/// outside drawn as inside only marches a little empty air. Behind the
/// lamp (a held torch's, at the eye) it's outside: from there the cone's
/// front faces face the eye.
pub fn eye_inside(eye: Vec3, apex: Vec3, dir: Vec3, tan: f32, length: f32) -> bool {
    let rel = eye - apex;
    let along = rel.dot(dir);
    if along <= 0.0 || along > length + 2.5 {
        return false;
    }
    let off = (rel - dir * along).length();
    off < along.max(0.0) * tan * 1.1 + 0.4
}

/// The cone (`cone`) placed for a beam from `apex` along `dir` (unit),
/// `tan` its half-angle's tangent, `length` long, in world space.
pub fn placed(apex: Vec3, dir: Vec3, tan: f32, length: f32) -> Transform {
    let up = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let r = tan * length;
    Transform::from_translation(apex)
        .looking_to(dir, up)
        .with_scale(Vec3::new(r, r, length))
}

/// `EXTRACTION_NO_FOG_BEAMS=1`: no beam draws (the A/B price of them).
pub fn hidden() -> bool {
    static HIDDEN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HIDDEN.get_or_init(|| std::env::var_os("EXTRACTION_NO_FOG_BEAMS").is_some_and(|v| v != "0"))
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
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // One face a pixel: from outside the front ones, from inside the
        // back ones (they surround the eye).
        if key.bind_group_data.inside {
            descriptor.primitive.cull_mode = Some(Face::Front);
            if let Some(f) = descriptor.fragment.as_mut() {
                f.shader_defs.push("FOG_BEAM_INSIDE".into());
            }
        } else {
            descriptor.primitive.cull_mode = Some(Face::Back);
        }
        Ok(())
    }
}

/// How fast a lamp's or a torch's light spreads down its throw (`air.w`):
/// thin a few metres out, as a wide cheap lens's is.
pub const LAMP_SPREAD: f32 = 0.08;

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
/// to build its pipeline before any lit beam is in view: seen from
/// outside, or (`inside`) from in it.
pub(crate) fn decoy(world: &mut World, at: Vec3, inside: bool) -> Entity {
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
            inside,
        });
    world
        .spawn((
            Name::new(if inside {
                "Warm-up fog beam (inside)"
            } else {
                "Warm-up fog beam"
            }),
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

    /// In the cone (with a little give at the wall), not beside it,
    /// behind the lamp or past the end.
    #[test]
    fn the_eye_is_inside_where_the_cone_is() {
        let (apex, dir, tan, len) = (Vec3::new(0.0, 30.0, 0.0), Vec3::NEG_Y, 0.13, 30.0);
        assert!(eye_inside(Vec3::new(0.0, 1.7, 0.0), apex, dir, tan, len));
        assert!(eye_inside(Vec3::new(3.9, 1.7, 0.0), apex, dir, tan, len));
        assert!(!eye_inside(Vec3::new(6.0, 1.7, 0.0), apex, dir, tan, len));
        assert!(!eye_inside(Vec3::new(0.0, 33.0, 0.0), apex, dir, tan, len));
        assert!(!eye_inside(Vec3::new(0.0, -4.0, 0.0), apex, dir, tan, len));
        // A torch held at the eye: behind its lamp.
        assert!(!eye_inside(Vec3::new(0.0, 30.3, 0.0), apex, dir, tan, len));
    }

    /// `placed` puts the cone's tip at the lamp and its end `length` down
    /// the throw, `tan * length` wide.
    #[test]
    fn a_placed_cone_lies_along_its_beam() {
        let dir = Vec3::new(0.3, -1.0, 0.2).normalize();
        let t = placed(Vec3::new(1.0, 20.0, 2.0), dir, 0.13, 25.0);
        let end = t.transform_point(Vec3::new(0.0, 0.0, -1.0));
        assert!(end.distance(Vec3::new(1.0, 20.0, 2.0) + dir * 25.0) < 1e-3);
        let rim = t.transform_point(Vec3::new(1.0, 0.0, -1.0));
        assert!((rim.distance(end) - 0.13 * 25.0).abs() < 1e-3);
    }

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
