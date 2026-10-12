//! Overcast sky, sun, ambient light and fog.
//!
//! The look is low cloud and a sun that never quite breaks through: dim,
//! cool directional light for soft shadows, a strong grey-blue ambient, and
//! fog in layers. Distance fog here; ground-hugging fog banks live in
//! `render::fog`.

use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .insert_resource(Weather::default())
            .insert_resource(ClearColor(Weather::default().horizon))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.62, 0.68, 0.76),
                brightness: 700.0,
                ..default()
            })
            .add_systems(Startup, spawn_sky)
            .add_systems(PostUpdate, follow_camera_with_dome);
    }
}

/// Global atmosphere settings, shared by the sky, the fog and the lights.
#[derive(Resource, Clone, Debug)]
pub struct Weather {
    pub horizon: Color,
    pub zenith: Color,
    /// Metres at which things fade to ~5% contrast.
    pub visibility: f32,
    pub sun_dir: Vec3,
    pub sun_lux: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            horizon: Color::srgb(0.50, 0.53, 0.55),
            zenith: Color::srgb(0.34, 0.37, 0.41),
            visibility: 230.0,
            // Low in the south-west, behind the cloud.
            sun_dir: Vec3::new(0.45, -0.38, 0.62).normalize(),
            sun_lux: 2800.0,
        }
    }
}

impl Weather {
    pub fn fog(&self) -> DistanceFog {
        DistanceFog {
            color: self.horizon,
            directional_light_color: Color::srgba(0.75, 0.72, 0.65, 0.25),
            directional_light_exponent: 12.0,
            falloff: FogFalloff::from_visibility_squared(self.visibility),
        }
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SkyParams {
    pub horizon: Vec4,
    pub zenith: Vec4,
    pub sun_dir: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SkyMaterial {
    #[uniform(0)]
    pub params: SkyParams,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }
}

#[derive(Component)]
pub struct SkyDome;

#[derive(Component)]
pub struct Sun;

fn spawn_sky(
    mut commands: Commands,
    weather: Res<Weather>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let to_vec4 = |c: Color| {
        let l = c.to_linear();
        Vec4::new(l.red, l.green, l.blue, 1.0)
    };
    commands.spawn((
        Name::new("Sky dome"),
        SkyDome,
        Mesh3d(meshes.add(inward_sphere(900.0))),
        MeshMaterial3d(materials.add(SkyMaterial {
            params: SkyParams {
                horizon: to_vec4(weather.horizon),
                zenith: to_vec4(weather.zenith),
                sun_dir: (-weather.sun_dir).extend(0.0),
            },
        })),
        Transform::IDENTITY,
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.spawn((
        Name::new("Sun"),
        Sun,
        DirectionalLight {
            illuminance: weather.sun_lux,
            color: Color::srgb(0.92, 0.93, 0.95),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::IDENTITY.looking_to(weather.sun_dir, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            first_cascade_far_bound: 12.0,
            maximum_distance: 140.0,
            ..default()
        }
        .build(),
    ));
}

/// A UV sphere with its faces turned inward.
fn inward_sphere(radius: f32) -> Mesh {
    let mut mesh = Sphere::new(radius).mesh().uv(48, 24);
    if let Some(bevy::mesh::Indices::U32(idx)) = mesh.indices_mut() {
        for tri in idx.chunks_mut(3) {
            tri.swap(1, 2);
        }
    }
    mesh
}

fn follow_camera_with_dome(
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    mut domes: Query<&mut Transform, With<SkyDome>>,
) {
    let Some(cam) = cameras.iter().next() else {
        return;
    };
    for mut t in &mut domes {
        t.translation = cam.translation();
    }
}
