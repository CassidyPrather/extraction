//! Thunderstorms (`Conditions::storm`): lightning that lights the whole
//! valley for a few frames, from where it struck (the sun's light swung
//! round and turned up, shadows on for the flash only), the cloud lit from
//! inside, and a `Lightning` message for the thunder, which Audio delays
//! by distance (`thunder_delay`). And rain you see crossing the valley in
//! sheets (`assets/shaders/rain_sheets.wgsl`, one draw, moved entirely on
//! the GPU).
//!
//! Each flash adds to whatever the weather (or the Surge) last set and
//! takes its addition back afterwards; if something else rewrote a value
//! meanwhile, that value stands.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use extraction_core::noise::Rng;

use super::sky::{SkyDome, SkyMaterial, Sun, Weather, WeatherLook};
use super::weather::{Conditions, Lightning, WeatherKind};

pub struct StormPlugin;

impl Plugin for StormPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<SheetMaterial>::default(),
            MaterialPlugin::<BoltMaterial>::default(),
        ))
        .init_resource::<BoltNow>()
        .add_systems(Startup, (spawn_sheets, spawn_bolt))
        .add_systems(
            Update,
            (lightning, show_bolt, sheets).chain().after(WeatherLook),
        );
    }
}

/// Seconds the thunder takes to arrive from a strike this far off (m).
pub fn thunder_delay(distance: f32) -> f32 {
    distance.max(0.0) / 343.0
}

/// A strike's strokes: (seconds after the first, how bright). Lightning
/// flickers: a leader and two to four return strokes down one channel.
#[derive(Clone, Debug, PartialEq)]
pub struct Strike {
    pub strokes: Vec<(f32, f32)>,
}

/// How long a stroke's light takes to die away (s): a few frames.
const STROKE_DECAY: f32 = 0.045;

impl Strike {
    pub fn roll(rng: &mut Rng) -> Self {
        let n = 2 + rng.below(3);
        let mut t = 0.0;
        let strokes = (0..n)
            .map(|i| {
                let a = if i == 0 { 1.0 } else { rng.range(0.45, 0.95) };
                let s = (t, a);
                t += rng.range(0.05, 0.14);
                s
            })
            .collect();
        Self { strokes }
    }

    /// How lit the sky is `t` s after the first stroke, 0..~1.
    pub fn envelope(&self, t: f32) -> f32 {
        self.strokes
            .iter()
            .filter(|(at, _)| t >= *at)
            .map(|(at, a)| a * (-(t - at) / STROKE_DECAY).exp())
            .fold(0.0, f32::max)
    }

    /// When it's over (s).
    pub fn length(&self) -> f32 {
        self.strokes.last().map_or(0.0, |s| s.0) + 8.0 * STROKE_DECAY
    }
}

/// How bright a strike this far off (m) lights the valley, 0..1: near, a
/// day's light for a moment; on the far side of the hills, the cloud.
pub fn strike_strength(distance: f32) -> f32 {
    (1.3 / (1.0 + (distance / 900.0).powi(2))).clamp(0.12, 1.0)
}

/// The light at full strength (lux, on the sun's light) and in the air
/// (the ambient's brightness).
const FLASH_LUX: f32 = 7000.0;
const FLASH_AMBIENT: f32 = 1400.0;

/// For screenshots: `EXTRACTION_STORM_FLASH=1` holds a near strike's
/// light at its peak.
fn held_flash() -> bool {
    std::env::var("EXTRACTION_STORM_FLASH").is_ok_and(|v| v != "0")
}

#[derive(Default)]
struct StormState {
    rng: Option<Rng>,
    /// Seconds to the next strike.
    next: f32,
    strike: Option<(Strike, f32)>,
    /// Toward the bolt from the eye, and how bright this one is.
    toward: Vec3,
    strength: f32,

    sun_lux: Added,
    ambient: Added,
    clear: AddedRgb,
    fogs: HashMap<Entity, AddedRgb>,
    /// The sun's own way and shadows, while a flash has them.
    sun_before: Option<(Quat, bool)>,
    sun_wrote: Quat,
}

/// A value a flash added to: what it wrote, and how much of that was it.
#[derive(Default, Clone, Copy)]
struct Added {
    wrote: f32,
    added: f32,
}

impl Added {
    fn apply(&mut self, v: &mut f32, add: f32) {
        if add == 0.0 && self.added == 0.0 {
            return;
        }
        let ours = (*v - self.wrote).abs() <= 1e-3 * self.wrote.abs().max(1.0);
        let base = if ours { *v - self.added } else { *v };
        *v = base + add;
        self.wrote = *v;
        self.added = add;
    }
}

#[derive(Default, Clone, Copy)]
struct AddedRgb {
    wrote: Vec3,
    added: Vec3,
}

impl AddedRgb {
    fn apply(&mut self, c: &mut Color, add: Vec3) {
        if add == Vec3::ZERO && self.added == Vec3::ZERO {
            return;
        }
        let l = c.to_linear();
        let v = Vec3::new(l.red, l.green, l.blue);
        let ours = v.abs_diff_eq(self.wrote, 1e-4);
        let base = if ours { v - self.added } else { v };
        let out = (base + add).max(Vec3::ZERO);
        *c = Color::linear_rgba(out.x, out.y, out.z, l.alpha);
        self.wrote = out;
        self.added = add;
    }
}

#[allow(clippy::too_many_arguments)]
fn lightning(
    conditions: Res<Conditions>,
    weather: Res<Weather>,
    zone: Res<super::Zone>,
    opts: Res<crate::launch::LaunchOptions>,
    time: Res<Time>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut suns: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    mut fogs: Query<(Entity, &mut DistanceFog)>,
    domes: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
    mut sky_mats: ResMut<Assets<SkyMaterial>>,
    mut strikes: MessageWriter<Lightning>,
    mut bolt_now: ResMut<BoltNow>,
    mut st: Local<StormState>,
) {
    let dt = time.delta_secs();
    let eye = cams.iter().next().map_or(Vec3::ZERO, |g| g.translation());
    let storming = conditions.storm && conditions.kind == WeatherKind::Downpour;
    if st.rng.is_none() {
        st.rng = Some(Rng::new(opts.seed ^ 0xB017 ^ u64::from(conditions.cycle)));
    }
    let held = opts.storm_flash || held_flash();
    if storming && st.strike.is_none() {
        st.next -= dt;
        if st.next <= 0.0 || held {
            let rng = st.rng.as_mut().unwrap();
            let strike = Strike::roll(rng);
            // Mostly far off; now and then close.
            let d = if held {
                450.0
            } else {
                250.0 + 2400.0 * rng.f32().powi(2)
            };
            let a = if held {
                // In view, a little off to the side (for the shot).
                let f = cams
                    .iter()
                    .next()
                    .map_or(Vec3::NEG_Z, |g| g.forward().as_vec3());
                f.z.atan2(f.x) + 0.25
            } else {
                rng.range(0.0, std::f32::consts::TAU)
            };
            let at_xz = Vec2::new(eye.x, eye.z) + Vec2::new(a.cos(), a.sin()) * d;
            let at = zone.on_ground(at_xz.x, at_xz.y);
            // The channel stands a few hundred metres up into the cloud.
            let bolt = at + Vec3::Y * 650.0;
            st.toward = (bolt - eye).normalize_or(Vec3::Y);
            st.strength = strike_strength(d);
            let rng = st.rng.as_mut().unwrap();
            bolt_now.channel = Some(bolt_path(rng, bolt, at));
            bolt_now.distance = d;
            strikes.write(Lightning {
                at,
                distance: d,
                strength: st.strength,
            });
            st.strike = Some((strike, 0.0));
            let rng = st.rng.as_mut().unwrap();
            st.next = rng.range(6.0, 24.0);
        }
    }
    // How lit, now.
    let mut f = 0.0;
    if let Some((strike, t)) = st.strike.as_mut() {
        if held {
            f = 1.0;
        } else {
            *t += dt;
            f = strike.envelope(*t);
            if *t > strike.length() {
                st.strike = None;
            }
        }
    }
    if !storming && !held {
        st.strike = None;
        f = 0.0;
    }
    let k = f * st.strength;
    if (bolt_now.lit - f).abs() > 1e-4 {
        bolt_now.lit = f;
    }
    let toward = st.toward;

    // The light: the sun's swung round to come from the bolt.
    let Ok((mut sun, mut xf)) = suns.single_mut() else {
        return;
    };
    st.sun_lux.apply(&mut sun.illuminance, FLASH_LUX * k);
    if k > 0.01 {
        if st.sun_before.is_none() {
            st.sun_before = Some((xf.rotation, sun.shadow_maps_enabled));
        }
        let want = Transform::IDENTITY.looking_to(-toward, Vec3::Y).rotation;
        xf.rotation = want;
        st.sun_wrote = want;
        if !sun.shadow_maps_enabled {
            sun.shadow_maps_enabled = true;
        }
        sun.color = Color::srgb(0.86, 0.9, 1.0);
    } else if let Some((rot, shadows)) = st.sun_before.take() {
        // Back as the weather left it (unless it has moved on meanwhile).
        if xf.rotation == st.sun_wrote {
            xf.rotation = rot;
        }
        sun.shadow_maps_enabled = shadows;
        sun.color = weather.sun_color;
    }
    st.ambient.apply(&mut ambient.brightness, FLASH_AMBIENT * k);
    // The air lit: the fog and what shows behind it.
    let air = Vec3::new(0.72, 0.78, 1.0) * 0.24 * k * 0.9;
    let mut clear_c = clear.0;
    st.clear.apply(&mut clear_c, air);
    if clear_c != clear.0 {
        clear.0 = clear_c;
    }
    for (e, mut fog) in &mut fogs {
        let slot = st.fogs.entry(e).or_default();
        let mut c = fog.color;
        slot.apply(&mut c, air);
        if c != fog.color {
            fog.color = c;
        }
    }
    for m in &domes {
        let want = toward.extend(k);
        if sky_mats.get(&m.0).is_some_and(|s| s.params.flash != want) {
            if let Some(mut s) = sky_mats.get_mut(&m.0) {
                s.params.flash = want;
            }
        }
    }
}

/// A bolt's channel from the cloud (`top`) to the ground (`at`): a
/// jagged line that wanders less near the ground, and a fork off it
/// partway down that dies in the air.
pub fn bolt_path(rng: &mut Rng, top: Vec3, at: Vec3) -> (Vec<Vec3>, Vec<Vec3>) {
    const STEPS: usize = 24;
    let mut main = Vec::with_capacity(STEPS + 1);
    let mut off = Vec2::ZERO;
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        // Wanders as it comes down, pinned to the strike at the end.
        off += Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) * 28.0 * (1.0 - t);
        let p = top.lerp(at, t) + Vec3::new(off.x, 0.0, off.y) * (1.0 - t * t);
        main.push(p);
    }
    let from = rng.range(0.3, 0.6);
    let start = main[(from * STEPS as f32) as usize];
    let mut fork = vec![start];
    let mut dir = Vec3::new(rng.range(-1.0, 1.0), -1.2, rng.range(-1.0, 1.0)).normalize();
    let mut p = start;
    for _ in 0..8 {
        dir = (dir + Vec3::new(rng.range(-0.4, 0.4), -0.1, rng.range(-0.4, 0.4))).normalize();
        p += dir * 22.0;
        fork.push(p);
    }
    (main, fork)
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct BoltParams {
    pub color: Vec4,
    pub size: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BoltMaterial {
    #[uniform(0)]
    pub params: BoltParams,
}

impl Material for BoltMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/bolt.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/bolt.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
        ])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

/// The ribbon's quads along a channel (`main` at full width, the fork
/// thinner), the segment's direction in the normal.
fn bolt_mesh(main: &[Vec3], fork: &[Vec3]) -> Mesh {
    let mut pos = Vec::new();
    let mut axis = Vec::new();
    let mut uv = Vec::new();
    let mut idx = Vec::new();
    for (line, w) in [(main, 1.0), (fork, 0.0)] {
        for seg in line.windows(2) {
            let d = (seg[1] - seg[0]).normalize_or(Vec3::NEG_Y);
            let b = pos.len() as u32;
            for (p, u) in [(seg[0], 0.0), (seg[0], 1.0), (seg[1], 1.0), (seg[1], 0.0)] {
                pos.push(p.to_array());
                axis.push(d.to_array());
                uv.push([u, w]);
            }
            idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, axis)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

#[derive(Component)]
struct Bolt {
    mesh: Handle<Mesh>,
    material: Handle<BoltMaterial>,
}

fn spawn_bolt(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BoltMaterial>>,
) {
    let mesh = meshes.add(bolt_mesh(&[], &[]));
    let material = materials.add(BoltMaterial {
        params: BoltParams {
            color: Vec4::new(0.85, 0.85, 1.0, 0.0),
            size: Vec4::new(2.5, 0.0, 0.0, 0.0),
        },
    });
    commands.spawn((
        Name::new("Lightning bolt"),
        Bolt {
            mesh: mesh.clone(),
            material: material.clone(),
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// The bolt shows on the flash's frames: brightest close to, lost in the
/// rain far off (past about three times the sight distance, nothing).
fn show_bolt(
    conditions: Res<Conditions>,
    now: Res<BoltNow>,
    mut q: Query<(&Bolt, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BoltMaterial>>,
    mut built: Local<Option<Vec3>>,
) {
    if !now.is_changed() {
        return;
    }
    let Ok((b, mut vis)) = q.single_mut() else {
        return;
    };
    let through = (-now.distance / (conditions.visibility * 3.0)).exp();
    let k = now.lit * through;
    vis.set_if_neq(if k > 0.01 {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if let Some((main, fork)) = &now.channel {
        if *built != main.first().copied() {
            *built = main.first().copied();
            if let Some(mut m) = meshes.get_mut(&b.mesh) {
                *m = bolt_mesh(main, fork);
            }
        }
    }
    if let Some(m) = materials.get(&b.material) {
        if (m.params.color.w - k).abs() > 1e-3 {
            if let Some(mut m) = materials.get_mut(&b.material) {
                m.params.color.w = k;
                // Wider far off, so it's more than a hair.
                m.params.size.x = 1.5 + now.distance * 0.004;
            }
        }
    }
}

/// The strike as the bolt wants it (written by `lightning`): the
/// channel, cloud to ground, and its fork (`bolt_path`), new with each
/// strike; how lit it is now, and how far off.
#[derive(Resource, Default)]
pub struct BoltNow {
    channel: Option<(Vec<Vec3>, Vec<Vec3>)>,
    lit: f32,
    distance: f32,
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SheetParams {
    /// x: how much rain (0 hides them), y: storm gusts, zw: wind (m/s).
    pub rain: Vec4,
    /// x: brightness against a day's (as the eye sees it).
    pub light: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SheetMaterial {
    #[uniform(0)]
    pub params: SheetParams,
}

impl Material for SheetMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/rain_sheets.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/rain_sheets.wgsl".into()
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

/// How many sheets drift round the eye.
pub(crate) const SHEETS: usize = 6;

/// One quad per sheet: the corners share the sheet's seed (position) and
/// differ in uv; the shader places, sizes and moves them.
fn sheet_mesh() -> Mesh {
    let mut rng = Rng::new(0x5EE7);
    let mut pos = Vec::with_capacity(SHEETS * 4);
    let mut uv = Vec::with_capacity(SHEETS * 4);
    let mut idx = Vec::with_capacity(SHEETS * 6);
    for i in 0..SHEETS {
        let p = [rng.f32(), rng.f32(), rng.f32()];
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
struct Sheets(Handle<SheetMaterial>);

fn spawn_sheets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SheetMaterial>>,
) {
    let m = materials.add(SheetMaterial {
        params: SheetParams {
            rain: Vec4::ZERO,
            light: Vec4::ONE,
        },
    });
    commands.spawn((
        Name::new("Rain sheets"),
        Sheets(m.clone()),
        Mesh3d(meshes.add(sheet_mesh())),
        MeshMaterial3d(m),
        Transform::IDENTITY,
        Visibility::Hidden,
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// Sheets in a downpour, heavier and gustier in a storm; none otherwise.
pub fn sheet_rain(c: &Conditions) -> f32 {
    match c.kind {
        WeatherKind::Downpour if c.storm => 1.0,
        WeatherKind::Downpour => 0.55,
        _ => 0.0,
    }
}

fn sheets(
    conditions: Res<Conditions>,
    mut q: Query<(&Sheets, &mut Visibility)>,
    mut materials: ResMut<Assets<SheetMaterial>>,
) {
    if !conditions.is_changed() {
        return;
    }
    let rain = if std::env::var("EXTRACTION_NO_SHEETS").is_ok() {
        0.0
    } else {
        sheet_rain(&conditions)
    };
    let wind = conditions.wind();
    for (s, mut vis) in &mut q {
        let want = if rain > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        vis.set_if_neq(want);
        if let Some(mut m) = materials.get_mut(&s.0) {
            m.params.rain = Vec4::new(
                rain,
                conditions.storm as u8 as f32,
                3.0 + 7.0 * wind,
                1.5 + 3.0 * wind,
            );
            m.params.light.x = super::rain::streak_light(&conditions);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::weather::Hour;

    #[test]
    fn a_strike_flickers_and_dies_in_a_few_frames() {
        let mut rng = Rng::new(3);
        for _ in 0..20 {
            let s = Strike::roll(&mut rng);
            assert!((2..=4).contains(&s.strokes.len()));
            assert_eq!(s.envelope(0.0), 1.0, "the first stroke is the brightest");
            // A frame at 90 Hz later it's still lit; it's gone by the end.
            assert!(s.envelope(1.0 / 90.0) > 0.7);
            assert!(s.envelope(s.length()) < 0.01);
            // It flickers: between strokes the light drops, then comes back.
            let (t1, _) = s.strokes[1];
            assert!(s.envelope(t1 - 0.001) < s.envelope(t1 + 0.001));
            assert!(s.length() < 0.8);
        }
    }

    #[test]
    fn near_strikes_light_more_and_thunder_lags_by_distance() {
        assert!(strike_strength(300.0) > strike_strength(1500.0));
        assert_eq!(strike_strength(100.0), 1.0);
        assert!(
            strike_strength(1e5) > 0.0,
            "a far one still lights the cloud"
        );
        assert!((thunder_delay(343.0 * 3.0) - 3.0).abs() < 1e-4);
    }

    #[test]
    fn sheets_cross_the_valley_in_heavy_rain_only() {
        let at = |k| sheet_rain(&Conditions::compose(k, Hour::Day, 1));
        assert_eq!(at(WeatherKind::Drizzle), 0.0);
        assert_eq!(at(WeatherKind::Clearing), 0.0);
        let pour = Conditions::compose(WeatherKind::Downpour, Hour::Day, 1);
        assert!(sheet_rain(&pour.stormy()) > sheet_rain(&pour));
        assert!(sheet_rain(&pour) > 0.0);
    }

    #[test]
    fn a_bolt_comes_down_to_where_it_struck() {
        let mut rng = Rng::new(8);
        let (top, at) = (
            Vec3::new(100.0, 700.0, -50.0),
            Vec3::new(100.0, 30.0, -50.0),
        );
        let (main, fork) = bolt_path(&mut rng, top, at);
        assert!(main.first().unwrap().distance(top) < 1.0);
        assert!(
            main.last().unwrap().distance(at) < 1e-3,
            "pinned to the strike"
        );
        assert!(
            main.iter().any(|p| p.xz().distance(at.xz()) > 10.0),
            "it wanders"
        );
        assert!(
            fork.len() > 2 && fork.last().unwrap().y > at.y,
            "the fork dies in the air"
        );
        let m = bolt_mesh(&main, &fork);
        assert_eq!(m.count_vertices(), 4 * (main.len() - 1 + fork.len() - 1));
    }

    #[test]
    fn a_flash_gives_back_what_it_added() {
        let mut slot = Added::default();
        let mut v = 900.0;
        slot.apply(&mut v, 7000.0);
        assert_eq!(v, 7900.0);
        slot.apply(&mut v, 0.0);
        assert_eq!(v, 900.0);
        // Rewritten meanwhile (the weather changed): the new value stands.
        slot.apply(&mut v, 500.0);
        v = 300.0;
        slot.apply(&mut v, 0.0);
        assert_eq!(v, 300.0);
        let mut c = AddedRgb::default();
        let mut col = Color::linear_rgba(0.1, 0.1, 0.1, 0.998);
        c.apply(&mut col, Vec3::splat(0.2));
        c.apply(&mut col, Vec3::ZERO);
        let l = col.to_linear();
        assert!((l.red - 0.1).abs() < 1e-5 && l.alpha == 0.998, "{l:?}");
    }
}
