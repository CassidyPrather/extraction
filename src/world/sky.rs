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
            .add_systems(
                Update,
                (apply_weather, surge_atmosphere)
                    .chain()
                    .in_set(WeatherLook),
            )
            .add_systems(PostUpdate, (follow_camera_with_dome, carry_wetness));
    }
}

/// The systems that set the look from the weather (the eyes adapt after).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct WeatherLook;

/// Global atmosphere settings, shared by the sky, the fog and the lights.
#[derive(Resource, Clone, Debug)]
pub struct Weather {
    pub horizon: Color,
    pub zenith: Color,
    /// Metres at which things fade to ~5% contrast.
    pub visibility: f32,
    pub sun_dir: Vec3,
    pub sun_lux: f32,
    /// The calm hour's sun and sky light (the Surge mixes from these).
    pub sun_color: Color,
    pub ambient_color: Color,
    pub ambient: f32,
    /// The low sun's glow along the horizon on its side (linear rgb,
    /// strength in w), and the fog's scattering toward it.
    pub glow: Vec4,
    /// How much of the sun's smear shows through the deck (the hour's).
    pub sun_through: f32,
    /// At night, how much the moon lights the air (0 by day, or with no
    /// moon up): the fog toward it, the cloud's edges round it.
    pub moon: f32,
    /// How much of the night sky there is (0 by day, 1 at night): stars.
    pub night: f32,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            horizon: Color::srgb(0.50, 0.53, 0.55),
            zenith: Color::srgb(0.34, 0.37, 0.41),
            visibility: 320.0,
            // High in the south-west, behind the cloud. (Low, its crisp
            // shadows ran twice a tree's height across the meadows: in the
            // headset, the forest edges' shadows read as ruled lines on the
            // ground.)
            sun_dir: Vec3::new(0.35, -0.8, 0.5).normalize(),
            sun_lux: 1350.0,
            sun_color: Color::srgb(0.92, 0.93, 0.95),
            ambient_color: Color::srgb(0.62, 0.68, 0.76),
            ambient: 1100.0,
            glow: Vec4::ZERO,
            sun_through: 1.0,
            moon: 0.0,
            night: 0.0,
        }
    }
}

impl Weather {
    pub fn fog(&self) -> DistanceFog {
        DistanceFog {
            color: self.horizon,
            // Toward the sun the fog lights up: faintly through the deck by
            // day, warm along a low sun's side of the valley.
            directional_light_color: if self.moon > 0.0 {
                // The moon's side of the fog, silvered: the air round a
                // bright moon lit, a compass even when the deck hides it.
                let h = self.horizon.to_linear();
                let k = 1.0 + 2.5 * self.moon;
                Color::linear_rgba(
                    h.red * k * 0.85,
                    h.green * k * 0.95,
                    h.blue * k * 1.15,
                    0.6 * self.moon,
                )
            } else if self.glow.w > 0.0 {
                Color::linear_rgba(
                    self.glow.x,
                    self.glow.y,
                    self.glow.z,
                    0.25 + 0.35 * self.glow.w,
                )
            } else {
                Color::srgba(0.75, 0.72, 0.65, 0.25)
            },
            // The moon stands high (a full one where the noon sun would):
            // a tight lobe left the horizon toward it barely lighter than
            // away (1.1x); a wide one makes the fog that side a compass.
            directional_light_exponent: if self.moon > 0.0 { 4.0 } else { 12.0 },
            falloff: FogFalloff::from_visibility_squared(self.visibility),
        }
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SkyParams {
    pub horizon: Vec4,
    pub zenith: Vec4,
    pub sun_dir: Vec4,
    /// x: how heavy the deck is (0 broken and high, 1 the reference
    /// overcast, more is lower and darker), y: how much scud runs under it.
    pub cloud: Vec4,
    /// A low sun's glow on its side of the horizon: rgb, strength in w.
    pub glow: Vec4,
    /// Lightning: toward the strike (xyz), how bright (w; `world::storm`).
    pub flash: Vec4,
    /// x: how far you see (m; the fog's visibility), for the sky's own
    /// fog.
    pub air: Vec4,
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
    opts: Res<crate::launch::LaunchOptions>,
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
                cloud: Vec4::new(1.0, 1.0, 0.0, 0.0),
                glow: Vec4::ZERO,
                flash: Vec4::ZERO,
                air: Vec4::new(weather.visibility, 0.0, 0.0, 0.0),
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
        // The world, and the body's shadow proxy (on a layer no camera has).
        crate::render::arms::sun_layers(),
        // Cascades are per view: in a headset, two eyes each. Two cascades
        // out to 90 m (past that the fog has the shadows anyway), less at
        // lower `--detail`.
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            first_cascade_far_bound: 15.0,
            maximum_distance: 40.0 + 50.0 * opts.detail,
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
            mat.params.sun_dir.w = (1.0 - 0.9 * s) * weather.sun_through;
            mat.params.air.x = weather.visibility * (1.0 - 0.4 * s);
        }
    }
    for mut fog in &mut fogs {
        fog.color = horizon;
        fog.falloff = FogFalloff::from_visibility_squared(weather.visibility * (1.0 - 0.4 * s));
    }
    for mut sun in &mut suns {
        sun.illuminance = weather.sun_lux * (1.0 - 0.6 * s) + 9000.0 * state.flash;
        sun.color = mix(weather.sun_color, Color::srgb(1.0, 0.75, 0.85), s);
    }
    ambient.color = mix(weather.ambient_color, Color::srgb(0.70, 0.52, 0.66), s);
    ambient.brightness = weather.ambient * (1.0 - 0.35 * s) + 2500.0 * state.flash;
}

#[derive(Default)]
struct SurgeFx {
    level: f32,
    flash: f32,
    next_flash: f32,
    applied_calm: bool,
}

/// How much the weather darkens the sky's light (the ambient, the sky's
/// colours) against a drizzle's: rain and murk darken it, clearing lifts
/// it.
pub fn sky_dim(conditions: &super::weather::Conditions) -> f32 {
    use super::weather::WeatherKind;
    let storm = if conditions.storm { 0.88 } else { 1.0 };
    storm
        * match conditions.kind {
            WeatherKind::Downpour => 0.72,
            WeatherKind::PeaSouper => 0.95,
            WeatherKind::Clearing => 1.12,
            WeatherKind::Drizzle => 1.0,
        }
}

/// The cycle's weather sets the calm look: how far you see, how much ground
/// fog, how strong the sun, how dark the sky.
fn apply_weather(
    conditions: Res<super::weather::Conditions>,
    mut weather: ResMut<Weather>,
    mut height_fog: ResMut<crate::render::fog::HeightFog>,
    mut fogs: Query<(
        &mut DistanceFog,
        Option<&mut bevy::render::view::ColorGrading>,
    )>,
    mut suns: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut sky_mats: ResMut<Assets<SkyMaterial>>,
    domes: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    zone: Res<super::Zone>,
    mut adapt: ResMut<crate::render::grade::Adaptation>,
    opts: Res<crate::launch::LaunchOptions>,
) {
    // New cameras (the player's, a scope's) take the weather as they appear.
    let fresh = fogs.iter_mut().any(|(f, _)| f.is_added());
    if !conditions.is_changed() && !fresh {
        return;
    }
    let base = Weather::default();
    let k = sky_dim(&conditions);
    let haze = zone.map.haze.unwrap_or([1.0; 3]);
    let moon = super::weather::moon_position(conditions.moon_phase);
    let hour = HourLook::of(conditions.hour, opts.sun_elevation, moon);
    let light = conditions.light;
    let scale = |c: Color, tint: Vec3| {
        let l = c.to_linear();
        Color::linear_rgb(
            l.red * k * haze[0] * tint.x * light,
            l.green * k * haze[1] * tint.y * light,
            l.blue * k * haze[2] * tint.z * light,
        )
    };
    weather.horizon = scale(base.horizon, hour.horizon);
    weather.zenith = scale(base.zenith, hour.zenith);
    weather.visibility = conditions.visibility;
    weather.sun_lux = conditions.sun;
    weather.sun_dir = hour.sun_dir.unwrap_or(base.sun_dir);
    weather.sun_color = hour.sun_color;
    weather.ambient_color = hour.ambient_color;
    weather.ambient = base.ambient * k * light;
    weather.sun_through = conditions.hour.sun();
    // How much the moon lights the air depends on how much cloud is
    // between: in a clearing, all of it; under murk, nothing.
    let moon_through = match conditions.kind {
        super::weather::WeatherKind::Clearing => 1.0,
        super::weather::WeatherKind::Drizzle => 0.45,
        _ => 0.0,
    };
    weather.moon = conditions.moon * moon_through;
    weather.night = (conditions.hour == super::weather::Hour::Night) as u8 as f32;
    // The glow needs a sun to come from: thick weather smothers it.
    let through = match conditions.kind {
        super::weather::WeatherKind::Clearing => 1.0,
        super::weather::WeatherKind::Drizzle => 0.6,
        super::weather::WeatherKind::PeaSouper => 0.3,
        super::weather::WeatherKind::Downpour => 0.2,
    };
    weather.glow = hour.glow * Vec4::new(light, light, light, through);
    valley_air(&zone.map, &conditions).apply(&mut height_fog);
    clear.0 = weather.horizon;
    ambient.brightness = weather.ambient;
    ambient.color = weather.ambient_color;
    let look = crate::render::grade::Look::for_map(&zone.map.name);
    adapt.base = look.exposure;
    adapt.base_lift = look.shadow_lift;
    let grading = look.grading();
    for (mut fog, grade) in &mut fogs {
        *fog = weather.fog();
        if let Some(mut grade) = grade {
            *grade = grading.clone();
        }
    }
    // Under a thick sky, or with the sun all but gone, it casts nothing
    // worth drawing (and the shadow passes are the dearest thing in a
    // headset frame). A bright moon in a clear sky does: long, soft-edged
    // and blue, the way to tell which way you face.
    let enough = if conditions.hour == super::weather::Hour::Night {
        80.0
    } else {
        300.0
    };
    let shadows = !matches!(
        conditions.kind,
        super::weather::WeatherKind::PeaSouper | super::weather::WeatherKind::Downpour
    ) && weather.sun_lux > enough;
    for (mut sun, mut xf) in &mut suns {
        sun.illuminance = weather.sun_lux;
        sun.color = weather.sun_color;
        if sun.shadow_maps_enabled != shadows {
            sun.shadow_maps_enabled = shadows;
        }
        let want = Transform::IDENTITY.looking_to(weather.sun_dir, Vec3::Y);
        if xf.rotation != want.rotation {
            *xf = want;
        }
    }
    for m in &domes {
        if let Some(mut mat) = sky_mats.get_mut(&m.0) {
            let h = weather.horizon.to_linear();
            let z = weather.zenith.to_linear();
            mat.params.horizon = Vec4::new(h.red, h.green, h.blue, 1.0);
            mat.params.zenith = Vec4::new(z.red, z.green, z.blue, 1.0);
            mat.params.sun_dir = (-weather.sun_dir).extend(weather.sun_through);
            mat.params.air.x = weather.visibility;
            // At night the glow's strength is the moon's in the air.
            mat.params.glow = if weather.night > 0.0 {
                Vec4::new(0.0, 0.0, 0.0, weather.moon)
            } else {
                weather.glow
            };
            mat.params.cloud = match conditions.kind {
                super::weather::WeatherKind::Downpour if conditions.storm => {
                    Vec4::new(1.6, 2.0, 0.0, 0.0)
                }
                super::weather::WeatherKind::Downpour => Vec4::new(1.35, 1.6, 0.0, 0.0),
                super::weather::WeatherKind::PeaSouper => Vec4::new(1.1, 0.3, 0.0, 0.0),
                super::weather::WeatherKind::Clearing => Vec4::new(0.45, 0.5, 0.0, 0.0),
                super::weather::WeatherKind::Drizzle => Vec4::new(1.0, 1.0, 0.0, 0.0),
            };
            // z: the moon's lit face (its phase, 0..1, sign: waxing +),
            // w: how much night sky (stars).
            let lit = super::weather::moon_lit(conditions.moon_phase);
            let waxing = conditions.moon_phase < 0.5;
            mat.params.cloud.z = if moon.is_some() && weather.night > 0.0 {
                if waxing {
                    lit.max(0.02)
                } else {
                    -lit.max(0.02)
                }
            } else {
                0.0
            };
            mat.params.cloud.w = weather.night;
        }
    }
}

/// What an hour does to the sky and its light (`weather::Hour`). Colours
/// multiply the day's (linear); the light's strength is the hour's
/// `light`, applied with them.
struct HourLook {
    horizon: Vec3,
    zenith: Vec3,
    /// None: the day's sun, high behind the cloud.
    sun_dir: Option<Vec3>,
    sun_color: Color,
    ambient_color: Color,
    glow: Vec4,
}

impl HourLook {
    /// `elevation`: the low sun's height at dawn and dusk, degrees.
    /// `moon`: where the moon is, if it's up (`weather::moon_position`).
    fn of(hour: super::weather::Hour, elevation: f32, moon: Option<(f32, f32)>) -> Self {
        use super::weather::Hour;
        // Light travelling from a sun at this bearing (degrees from +X,
        // toward +Z) and height.
        let from = |bearing: f32, up: f32| {
            let (b, e) = (bearing.to_radians(), up.to_radians());
            -Vec3::new(b.cos() * e.cos(), e.sin(), b.sin() * e.cos())
        };
        match hour {
            Hour::Day => Self {
                horizon: Vec3::ONE,
                zenith: Vec3::ONE,
                sun_dir: None,
                sun_color: Color::srgb(0.92, 0.93, 0.95),
                ambient_color: Color::srgb(0.62, 0.68, 0.76),
                glow: Vec4::ZERO,
            },
            // Low in the east, pale gold through the cloud; the air cool
            // and milky. (20 degrees up by default, `--sun-elevation`:
            // lower, its shadows ran in ruled lines across the meadows.)
            Hour::Dawn => Self {
                horizon: Vec3::new(1.06, 1.02, 1.0),
                zenith: Vec3::new(0.9, 0.95, 1.08),
                sun_dir: Some(from(11.0, elevation)),
                sun_color: Color::srgb(1.0, 0.9, 0.78),
                ambient_color: Color::srgb(0.66, 0.69, 0.76),
                glow: Vec4::new(1.0, 0.82, 0.66, 0.7),
            },
            // Low in the west, the cloud lit amber from under on that side,
            // the rest of the sky going blue-grey.
            Hour::Dusk => Self {
                horizon: Vec3::new(1.22, 0.95, 0.8),
                zenith: Vec3::new(0.8, 0.84, 1.08),
                sun_dir: Some(from(197.0, elevation)),
                sun_color: Color::srgb(1.0, 0.72, 0.5),
                ambient_color: Color::srgb(0.6, 0.6, 0.74),
                glow: Vec4::new(1.0, 0.52, 0.3, 1.0),
            },
            // What light there is, blue; the "sun" is the moon, where it
            // stands on the sun's path, cool and silver.
            Hour::Night => Self {
                horizon: Vec3::new(0.7, 0.8, 1.1),
                zenith: Vec3::new(0.6, 0.7, 1.1),
                sun_dir: moon.map(|(b, e)| from(b, e)),
                sun_color: Color::srgb(0.66, 0.76, 1.0),
                ambient_color: Color::srgb(0.5, 0.58, 0.8),
                glow: Vec4::ZERO,
            },
        }
    }
}

/// What the weather and the valley want of the ground fog.
struct HeightFogTarget {
    density: f32,
    base: f32,
    cap_density: f32,
    cap_base: f32,
    snow_line: f32,
}

impl HeightFogTarget {
    /// Only touches the resource when something changed: each change
    /// re-patches the shader.
    fn apply(&mut self, fog: &mut crate::render::fog::HeightFog) {
        let want = crate::render::fog::HeightFog {
            density: self.density,
            base: self.base,
            cap_density: self.cap_density,
            cap_base: self.cap_base,
            snow_line: self.snow_line,
            ..*fog
        };
        if want != *fog {
            *fog = want;
        }
    }
}

/// The valley's ground fog under this weather: how thick, and from what
/// height it pools (16 m suits the three valleys; the Training Yard lies
/// lower and was under it whole). Where a valley has tops, cloud sits on
/// them: Kettle Pass's summits go into it and the pass is in its skirts
/// (see `docs/ART_DIRECTION.md`), lower and thicker in murk, lifted in a
/// clearing spell.
fn valley_air(
    map: &extraction_core::map::MapDef,
    conditions: &super::weather::Conditions,
) -> HeightFogTarget {
    use super::weather::WeatherKind;
    let (ground, base, cap, lift) = match map.name.as_str() {
        "Kettle Pass" => (1.0, 16.0, 0.009, 52.0),
        // The practice ground is kinder: you should see what you're shown.
        "Training Yard" => (0.55, 8.0, 0.0, 0.0),
        // The drained reservoir: wet fog pooling from the bed's floor (31-34 m
        // by the dam, 41 at the head; the crest at 52), thin on the crest.
        // (Based at 40, everything under it was at full density: the valley
        // below the dam is as low, and the dam vanished from the gate.)
        "Sallow Reservoir" => (1.1, 32.0, 0.0, 0.0),
        // High and cold: thin ground fog in the Bottoms (51 m), the cloud
        // cap down on the summit and the col (136-152 m) with the bench
        // (93 m) under it.
        "Rime Fell" => (0.7, 54.0, 0.011, 50.0),
        _ => (1.0, 16.0, 0.0, 0.0),
    };
    let (k, dz) = match conditions.kind {
        WeatherKind::PeaSouper => (1.4, -8.0),
        WeatherKind::Downpour => (1.2, -4.0),
        WeatherKind::Clearing => (0.5, 12.0),
        WeatherKind::Drizzle => (1.0, 0.0),
    };
    HeightFogTarget {
        density: conditions.ground_fog * ground,
        base,
        cap_density: cap * k,
        cap_base: map.base_height + lift + dz,
        snow_line: super::snow::snow_line(map).unwrap_or(crate::render::fog::NO_SNOW),
    }
}

/// How wet the world is, on its way to the surfaces' shaders: the fog
/// colour's alpha (`render::fog::WET_CODE`), the one per-frame value every
/// PBR surface already reads. While the eye is indoors the rooms are dry
/// (`render::grade::Adaptation::indoors`).
fn carry_wetness(
    wet: Res<super::weather::Wetness>,
    snow: Option<Res<super::snow::Snow>>,
    adapt: Res<crate::render::grade::Adaptation>,
    mut fogs: Query<&mut DistanceFog>,
) {
    let dry = 1.0 - adapt.indoors;
    let w = (wet.level * dry).clamp(0.0, 1.0);
    let a = 1.0 - w * crate::render::fog::WET_CODE;
    // And the snow lying, on the lobe exponent's fraction (`SNOW_CODE`).
    let s = snow.map_or(0.0, |s| s.cover * dry).clamp(0.0, 1.0);
    for mut fog in &mut fogs {
        if (fog.color.alpha() - a).abs() > 1e-6 {
            fog.color.set_alpha(a);
        }
        let e = fog.directional_light_exponent.floor() + s * crate::render::fog::SNOW_CODE;
        if (fog.directional_light_exponent - e).abs() > 1e-5 {
            fog.directional_light_exponent = e;
        }
    }
}
