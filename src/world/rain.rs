//! Rain you can see: a field of streaks animated entirely in a vertex
//! shader (`assets/shaders/rain.wgsl`), thickened or thinned by the
//! weather's rain intensity.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::weather::Conditions;

pub struct RainPlugin;

impl Plugin for RainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<RainMaterial>::default())
            .add_systems(Startup, spawn_rain)
            .add_systems(Update, rain_intensity);
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct RainParams {
    pub params: Vec4,
    pub wind: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct RainMaterial {
    #[uniform(0)]
    pub rain: RainParams,
}

impl Material for RainMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/rain.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/rain.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

const STREAKS: usize = 2500;
const BOX: f32 = 24.0;

fn streak_mesh() -> Mesh {
    let mut rng = extraction_core::noise::Rng::new(0x4A1);
    let mut pos = Vec::with_capacity(STREAKS * 4);
    let mut uv = Vec::with_capacity(STREAKS * 4);
    let mut idx = Vec::with_capacity(STREAKS * 6);
    for i in 0..STREAKS {
        let p = [
            rng.range(-BOX * 0.5, BOX * 0.5),
            rng.range(-BOX * 0.5, BOX * 0.5),
            rng.range(-BOX * 0.5, BOX * 0.5),
        ];
        let b = (i * 4) as u32;
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pos.push(p);
            uv.push([u, v]);
        }
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

#[derive(Component)]
struct Rain(Handle<RainMaterial>);

fn spawn_rain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RainMaterial>>,
) {
    let m = materials.add(RainMaterial {
        rain: RainParams {
            params: Vec4::new(0.35, 9.0, 0.55, BOX),
            wind: Vec4::new(1.2, 0.0, 0.6, 0.0),
        },
    });
    commands.spawn((
        Name::new("Rain"),
        Rain(m.clone()),
        Mesh3d(meshes.add(streak_mesh())),
        MeshMaterial3d(m),
        Transform::IDENTITY,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn rain_intensity(
    conditions: Res<Conditions>,
    rain: Query<(&Rain, &mut Visibility)>,
    mut materials: ResMut<Assets<RainMaterial>>,
) {
    if !conditions.is_changed() {
        return;
    }
    for (r, mut vis) in rain {
        *vis = if conditions.rain > 0.01 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(mut m) = materials.get_mut(&r.0) {
            m.rain.params.x = conditions.rain;
            m.rain.wind = Vec4::new(1.2 + 2.0 * conditions.rain, 0.0, 0.6 + conditions.rain, 0.0);
        }
    }
}
