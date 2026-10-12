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
                brightness: 1100.0,
                ..default()
            })
            .add_message::<SurgeFlash>()
            .add_systems(Startup, spawn_sky)
            .add_systems(Update, surge_atmosphere)
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
            visibility: 320.0,
            // Low in the south-west, behind the cloud.
            sun_dir: Vec3::new(0.45, -0.38, 0.62).normalize(),
            sun_lux: 1600.0,
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
                sun_dir: (-weather.sun_dir).extend(1.0),
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
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut domes: Query<&mut Transform, With<SkyDome>>,
) {
    let Some(cam) = cameras.iter().next() else {
        return;
    };
    for mut t in &mut domes {
        t.translation = cam.translation();
    }
}

/// A discharge in the Surge sky: a flash and a crack (audio listens).
#[derive(Message, Clone, Copy, Debug)]
pub struct SurgeFlash {
    /// Where in the sky, as a direction from the listener.
    pub dir: Vec3,
    pub strength: f32,
}

/// The Surge recolours the world: the sky bruises violet, the light drains,
/// the fog closes in, and the sky discharges now and then.
fn surge_atmosphere(
    raid: Res<crate::raid::Raid>,
    weather: Res<Weather>,
    time: Res<Time>,
    mut sky_mats: ResMut<Assets<SkyMaterial>>,
    domes: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
    mut fogs: Query<&mut DistanceFog>,
    mut suns: Query<&mut DirectionalLight, With<Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut flashes: MessageWriter<SurgeFlash>,
    mut state: Local<SurgeFx>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let level = raid.surge_level();
    // Ease toward the target so phase changes never snap.
    state.level += (level - state.level) * (dt * 0.8).min(1.0);
    let s = state.level;
    if s < 1e-3 && state.applied_calm {
        return;
    }
    state.applied_calm = s < 1e-3;
    // Discharges while it rages.
    state.flash = (state.flash - dt * 5.0).max(0.0);
    if raid.surging() {
        state.next_flash -= dt;
        if state.next_flash <= 0.0 {
            let r = |k: f32| ((t * 12.9898 + k).sin() * 43_758.547).fract().abs();
            state.next_flash = 1.5 + 6.0 * r(1.0);
            let strength = 0.5 + 0.5 * r(2.0);
            state.flash = strength;
            let a = r(3.0) * std::f32::consts::TAU;
            flashes.write(SurgeFlash {
                dir: Vec3::new(a.cos(), 0.35 + 0.4 * r(4.0), a.sin()).normalize(),
                strength,
            });
        }
    }
    let pulse = 1.0 + 0.12 * s * (t * 1.7).sin() * (t * 0.53).sin();
    let mix = |calm: Color, surge: Color, k: f32| {
        let a = calm.to_linear().to_vec3();
        let b = surge.to_linear().to_vec3();
        let c = a.lerp(b, k.clamp(0.0, 1.0)) * pulse + Vec3::splat(state.flash * 0.25);
        Color::linear_rgb(c.x, c.y, c.z)
    };
    let horizon = mix(weather.horizon, Color::srgb(0.46, 0.30, 0.40), s);
    let zenith = mix(weather.zenith, Color::srgb(0.22, 0.10, 0.20), s);
    clear.0 = horizon;
    for m in &domes {
        if let Some(mut mat) = sky_mats.get_mut(&m.0) {
            let h = horizon.to_linear();
            let z = zenith.to_linear();
            mat.params.horizon = Vec4::new(h.red, h.green, h.blue, 1.0);
            mat.params.zenith = Vec4::new(z.red, z.green, z.blue, 1.0);
            mat.params.sun_dir.w = 1.0 - 0.9 * s;
        }
    }
    for mut fog in &mut fogs {
        fog.color = horizon;
        fog.falloff = FogFalloff::from_visibility_squared(weather.visibility * (1.0 - 0.4 * s));
    }
    for mut sun in &mut suns {
        sun.illuminance = weather.sun_lux * (1.0 - 0.6 * s) + 9000.0 * state.flash;
        sun.color = mix(
            Color::srgb(0.92, 0.93, 0.95),
            Color::srgb(1.0, 0.75, 0.85),
            s,
        );
    }
    ambient.color = mix(
        Color::srgb(0.62, 0.68, 0.76),
        Color::srgb(0.70, 0.52, 0.66),
        s,
    );
    ambient.brightness = 1100.0 * (1.0 - 0.35 * s) + 2500.0 * state.flash;
}

#[derive(Default)]
struct SurgeFx {
    level: f32,
    flash: f32,
    next_flash: f32,
    applied_calm: bool,
}
