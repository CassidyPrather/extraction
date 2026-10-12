//! Water: dark, peaty, glossy. Ripple normals and a fresnel toward the
//! overcast sky are done in the shader; the base is a blended
//! StandardMaterial so it lights and fogs like everything else.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterExt>;

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct WaterParams {
    /// Colour the surface reflects at grazing angles (the sky).
    pub sky: Vec4,
    /// Flow direction and speed (x, z) in metres per second.
    pub flow: Vec2,
    pub ripple: f32,
    pub _pad: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterExt {
    #[uniform(100)]
    pub params: WaterParams,
}

impl MaterialExtension for WaterExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

pub struct WaterMaterialPlugin;

impl Plugin for WaterMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
    }
}

pub fn water_material() -> WaterMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.035, 0.045, 0.04, 0.9),
            perceptual_roughness: 0.06,
            reflectance: 0.6,
            alpha_mode: AlphaMode::Blend,
            ..default()
        },
        extension: WaterExt {
            params: WaterParams {
                sky: Vec4::new(0.20, 0.22, 0.24, 1.0),
                flow: Vec2::new(0.35, 0.1),
                ripple: 1.0,
                _pad: 0.0,
            },
        },
    }
}
