//! First snow: in a valley with high ground (`snow_line`), on a cold
//! cycle, it starts some way into the raid and lies on what faces up above
//! the line, deepening over minutes and melting slowly after (`Snow`).
//! What you walk through keeps your prints for a while (`Tracks`): a tell
//! on you, and one the machines can read. And in the cold your breath
//! shows.
//!
//! The surfaces' snow goes to the shaders the way the wet does
//! (`render::fog`): its amount on the fog's lobe exponent, the line baked
//! into the valley's patch; the terrain takes both in its own uniform.

use std::collections::VecDeque;

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use extraction_core::noise::Rng;

use extraction_core::map::MapDef;

use super::weather::{Conditions, WeatherKind};

/// Where snow lies in a valley (m): above this, and it comes down as
/// flakes; below, rain. None: the valley's too low for it. The map's own
/// `snow_line`, else the built-in ones.
pub fn snow_line(map: &MapDef) -> Option<f32> {
    map.snow_line.or(match map.name.as_str() {
        // The pass, the lodge and the tops; the valley floor (11-24 m)
        // stays wet.
        "Kettle Pass" => Some(30.0),
        _ => None,
    })
}

/// How hard it's snowing (0..1) at this point of the raid (the raid
/// clock, s). A cold valley (the map's `snow` lying all the time) snows
/// whenever its weather brings anything down: light in drizzle or fog, a
/// blizzard in a downpour. Elsewhere above a snow line, only the first
/// snow (`first_snow`).
pub fn snowfall(seed: u64, cycle: u32, map: &MapDef, c: &Conditions, clock: f32) -> f32 {
    if snow_line(map).is_none() {
        return 0.0;
    }
    if map.snow > 0.0 {
        return match c.kind {
            WeatherKind::Downpour => 1.0,
            WeatherKind::Drizzle => 0.55,
            WeatherKind::PeaSouper => 0.25,
            WeatherKind::Clearing => 0.0,
        };
    }
    match first_snow(seed, cycle, map, c) {
        Some(t) if clock >= t => 1.0,
        _ => 0.0,
    }
}

/// When in this cycle's raid the first snow starts (the raid clock, s),
/// if it snows at all: a valley with a snow line, past the first couple
/// of cycles (the season turning), under cloud that can snow (not a
/// downpour's warm rain, not a clear sky), one cycle in three, from a
/// stream of its own. It comes late: twenty to forty minutes in, with the
/// Surge at sixty.
pub fn first_snow(seed: u64, cycle: u32, map: &MapDef, c: &Conditions) -> Option<f32> {
    snow_line(map)?;
    if cycle < 2 || !matches!(c.kind, WeatherKind::Drizzle | WeatherKind::PeaSouper) {
        return None;
    }
    let mut rng = Rng::new(seed ^ 0x5_0011 ^ (cycle as u64).wrapping_mul(0x2545_F491));
    if rng.below(3) != 0 {
        return None;
    }
    Some(60.0 * rng.range(20.0, 40.0))
}

/// Minutes falling snow takes to lie most of the way, and to melt.
pub const COVER_MINUTES: f32 = 9.0;
pub const MELT_MINUTES: f32 = 40.0;

/// The snow now: how hard it's falling (0..1), how much lies (0..1), and
/// the valley's line.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Snow {
    pub falling: f32,
    pub cover: f32,
    pub line: Option<f32>,
    /// What lies whatever the weather (the map's `snow`): the cover never
    /// melts below it.
    pub floor: f32,
    settled: bool,
}

impl Snow {
    /// How far above the line a height is, 0..1 (a few metres' blend; a
    /// valley without a line, 0).
    pub fn above(&self, y: f32) -> f32 {
        self.line.map_or(0.0, |l| {
            extraction_core::geom::smoothstep(l - 3.0, l + 4.0, y)
        })
    }

    /// How much snow lies at this height, 0..1.
    pub fn lies(&self, y: f32) -> f32 {
        self.cover * self.above(y)
    }

    /// Whether it's cold enough here to see your breath.
    pub fn cold(&self, y: f32) -> bool {
        (self.cover > 0.05 || self.falling > 0.05) && self.line.is_some_and(|l| y > l - 15.0)
    }

    /// `dt` seconds with snow falling this hard (0..1).
    pub fn step(&mut self, falling: f32, dt: f32) {
        // It thickens and eases off over a minute or so.
        self.falling += (falling - self.falling) * (1.0 - (-dt / 40.0).exp());
        let (target, minutes) = if self.falling > 0.05 {
            (1.0, COVER_MINUTES / self.falling.max(0.3))
        } else {
            (self.floor, MELT_MINUTES)
        };
        self.cover += (target - self.cover) * (1.0 - (-dt / (minutes * 60.0 / 3.0)).exp());
        self.cover = self.cover.max(self.floor);
    }
}

/// A footprint in the snow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Print {
    pub at: Vec3,
    /// Which way the walker was going (radians about +Y, 0 = -Z).
    pub yaw: f32,
    pub left: bool,
    /// Who made it.
    pub by: Entity,
    /// How far it has filled in or blown over, 0 (fresh) to 1 (gone).
    pub fill: f32,
}

/// Prints in the snow, oldest first: the walker's way through it, for as
/// long as the snow keeps them. Robot minds can follow them
/// (`newest_near`, then `after` along the trail).
#[derive(Resource, Debug, Default)]
pub struct Tracks {
    prints: VecDeque<Print>,
    /// Bumped whenever the prints change (for whoever draws them).
    pub version: u32,
}

/// How many prints are kept (the oldest go first).
pub const MAX_PRINTS: usize = 240;
/// Seconds a print lasts with no snow falling (wind fills it slowly), and
/// in falling snow at its hardest.
pub const PRINT_LIFE: f32 = 12.0 * 60.0;
pub const PRINT_LIFE_FALLING: f32 = 3.0 * 60.0;

impl Tracks {
    pub fn prints(&self) -> impl DoubleEndedIterator<Item = &Print> {
        self.prints.iter()
    }

    pub fn len(&self) -> usize {
        self.prints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prints.is_empty()
    }

    pub fn clear(&mut self) {
        self.prints.clear();
        self.version = self.version.wrapping_add(1);
    }

    pub fn add(&mut self, p: Print) {
        if self.prints.len() == MAX_PRINTS {
            self.prints.pop_front();
        }
        self.prints.push_back(p);
        self.version = self.version.wrapping_add(1);
    }

    /// The freshest print within `r` of `p` (any walker's).
    pub fn newest_near(&self, p: Vec3, r: f32) -> Option<&Print> {
        self.prints
            .iter()
            .rev()
            .find(|q| q.at.distance_squared(p) < r * r)
    }

    /// The same walker's next print after `print`: where they went.
    pub fn after(&self, print: &Print) -> Option<&Print> {
        let i = self.prints.iter().position(|q| q == print)?;
        self.prints.iter().skip(i + 1).find(|q| q.by == print.by)
    }

    /// `dt` seconds of weather: prints fill, faster while it snows, and
    /// the full ones go.
    pub fn weather(&mut self, falling: f32, dt: f32) {
        if self.prints.is_empty() {
            return;
        }
        let rate = 1.0 / PRINT_LIFE + falling / PRINT_LIFE_FALLING;
        for p in &mut self.prints {
            p.fill += dt * rate;
        }
        let before = self.prints.len();
        self.prints.retain(|p| p.fill < 1.0);
        if self.prints.len() != before {
            self.version = self.version.wrapping_add(1);
        }
    }
}

pub struct SnowPlugin;

impl Plugin for SnowPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<crate::player::Footstep>()
            .init_resource::<Snow>()
            .init_resource::<Tracks>()
            .add_systems(Update, (snow_with_weather, tread).chain());
    }
}

fn snow_with_weather(
    conditions: Res<Conditions>,
    raid: Res<crate::raid::Raid>,
    zone: Res<super::Zone>,
    opts: Res<crate::launch::LaunchOptions>,
    time: Res<Time>,
    mut snow: ResMut<Snow>,
    mut tracks: ResMut<Tracks>,
) {
    let line = snow_line(&zone.map);
    if snow.line != line {
        snow.line = line;
    }
    if line.is_none() {
        return;
    }
    let dt = time.delta_secs();
    if let Some(held) = opts.snow {
        // Dev: falling now, this much already lying.
        let want = Snow {
            falling: if held > 0.0 { 1.0 } else { 0.0 },
            cover: held.max(zone.map.snow),
            line,
            floor: zone.map.snow,
            settled: true,
        };
        snow.set_if_neq(want);
    } else {
        let falling = snowfall(opts.seed, raid.cycle, &zone.map, &conditions, raid.clock);
        snow.floor = zone.map.snow;
        if !snow.settled {
            // A raid starts with the snow as the weather has left it: a
            // cold valley's falling snow has lain a while.
            snow.settled = true;
            snow.falling = falling;
            snow.cover = if falling > 0.0 && zone.map.snow > 0.0 {
                0.85_f32.max(zone.map.snow)
            } else {
                zone.map.snow
            };
        }
        snow.step(falling, dt);
    }
    // (A screenshot's demo trail stays as laid: a software-rendered shot
    // takes minutes of clock, and the prints filled in before the frame.)
    if std::env::var("EXTRACTION_DEMO_TRACKS").is_err() {
        tracks.weather(snow.falling, dt);
    }
}

/// Footsteps leave prints where snow lies.
fn tread(
    snow: Res<Snow>,
    mut steps: MessageReader<crate::player::Footstep>,
    mut tracks: ResMut<Tracks>,
    mut last: Local<Vec<(Entity, Vec3, bool)>>,
) {
    for s in steps.read() {
        let prev = last.iter_mut().find(|l| l.0 == s.entity);
        let (dir, left) = match prev {
            Some(l) => {
                let d = s.position - l.1;
                let left = !l.2;
                l.1 = s.position;
                l.2 = left;
                (d, left)
            }
            None => {
                last.push((s.entity, s.position, false));
                (Vec3::ZERO, false)
            }
        };
        if snow.lies(s.position.y) < 0.25 || dir.xz().length() < 0.2 {
            continue;
        }
        tracks.add(Print {
            at: s.position,
            yaw: f32::atan2(-dir.x, -dir.z),
            left,
            by: s.entity,
            fill: 0.0,
        });
    }
}

/// Client side: the prints drawn, breath in the cold.
pub struct SnowVisualsPlugin;

impl Plugin for SnowVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_prints, spawn_breath))
            .add_systems(Update, (demo_trail, draw_prints, breathe));
    }
}

#[derive(Component)]
struct PrintMesh(Handle<Mesh>);

/// A boot's print, as an alpha mask: sole and heel, a tread across them.
fn print_texture() -> Image {
    let (w, h) = (32u32, 64u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 - 0.5;
            let v = (y as f32 + 0.5) / h as f32;
            // The sole (toe end at v = 0) and the heel, two soft ovals.
            let sole = 1.0 - ((u / 0.42).powi(2) + ((v - 0.33) / 0.33).powi(2)).sqrt();
            let heel = 1.0 - ((u / 0.36).powi(2) + ((v - 0.8) / 0.17).powi(2)).sqrt();
            let m = (sole.max(heel) * 6.0).clamp(0.0, 1.0);
            // The tread: ridges across the sole.
            let tread = 0.8 + 0.2 * ((v * 40.0).sin() * 0.5 + 0.5);
            let a = (m * tread * 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn spawn_prints(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mesh = meshes.add(print_mesh(&[], |_, _| 0.0, |_, _| Vec3::Y));
    let material = materials.add(StandardMaterial {
        // Trodden snow: packed, in its own shade.
        base_color: Color::srgb(0.52, 0.56, 0.64),
        base_color_texture: Some(images.add(print_texture())),
        perceptual_roughness: 0.6,
        alpha_mode: AlphaMode::Blend,
        depth_bias: 40.0,
        ..default()
    });
    commands.spawn((
        Name::new("Footprints"),
        PrintMesh(mesh.clone()),
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::Hidden,
        // Its bounds change with every print (Bevy computes them once).
        bevy::camera::visibility::NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// The prints as quads laid on the ground (a boot's size), each fading as
/// it fills.
fn print_mesh(
    prints: &[Print],
    ground: impl Fn(f32, f32) -> f32,
    normal: impl Fn(f32, f32) -> Vec3,
) -> Mesh {
    let mut pos = Vec::with_capacity(prints.len() * 4);
    let mut nrm = Vec::with_capacity(prints.len() * 4);
    let mut uv = Vec::with_capacity(prints.len() * 4);
    let mut col = Vec::with_capacity(prints.len() * 4);
    let mut idx = Vec::with_capacity(prints.len() * 6);
    for (i, p) in prints.iter().enumerate() {
        let fwd = Vec3::new(-p.yaw.sin(), 0.0, -p.yaw.cos());
        let side = Vec3::new(-fwd.z, 0.0, fwd.x);
        let foot = p.at + side * if p.left { -0.11 } else { 0.11 };
        let n = normal(foot.x, foot.z);
        let a = (1.0 - p.fill).clamp(0.0, 1.0).powf(0.7);
        let b = (i * 4) as u32;
        for (s, f, u, v) in [
            (-0.065, 0.15, 0.0, 0.0),
            (0.065, 0.15, 1.0, 0.0),
            (0.065, -0.15, 1.0, 1.0),
            (-0.065, -0.15, 0.0, 1.0),
        ] {
            let c = foot + side * s + fwd * f;
            pos.push([c.x, ground(c.x, c.z) + 0.03, c.z]);
            nrm.push(n.to_array());
            uv.push([u, v]);
            col.push([1.0, 1.0, 1.0, a]);
        }
        idx.extend_from_slice(&[b, b + 2, b + 1, b, b + 3, b + 2]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
    .with_inserted_indices(Indices::U32(idx))
}

/// For screenshots (`EXTRACTION_DEMO_TRACKS=1`, with `--snow`): a
/// walker's trail from just ahead of the camera, curving away, as if
/// someone had come this way a minute ago.
fn demo_trail(
    snow: Res<Snow>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    zone: Res<super::Zone>,
    mut tracks: ResMut<Tracks>,
    mut laid: Local<Option<Vec3>>,
) {
    if snow.cover <= 0.0 || std::env::var("EXTRACTION_DEMO_TRACKS").is_err() {
        return;
    }
    let Some(eye) = cams.iter().next() else {
        return;
    };
    // Laid again wherever the camera goes (a tour's views).
    if laid.is_some_and(|l| l.distance(eye.translation()) < 5.0) {
        return;
    }
    *laid = Some(eye.translation());
    tracks.clear();
    let f = eye.forward().as_vec3();
    let mut dir = Vec2::new(f.x, f.z).normalize_or(Vec2::NEG_Y);
    let mut at = eye.translation().xz() + dir * 1.5;
    for i in 0..36 {
        dir = Vec2::from_angle(0.025).rotate(dir);
        at += dir * 0.8;
        let ground = zone.on_ground(at.x, at.y);
        // Only where snow lies, as a walker's would be.
        if snow.lies(ground.y) < 0.25 {
            continue;
        }
        tracks.add(Print {
            at: ground,
            yaw: f32::atan2(-dir.x, -dir.y),
            left: i % 2 == 0,
            by: Entity::PLACEHOLDER,
            fill: 0.05 + 0.01 * (36 - i) as f32,
        });
    }
}

/// Rebuilt when a print is made or goes, and every few seconds as they
/// fill (a couple of hundred quads: nothing).
fn draw_prints(
    tracks: Res<Tracks>,
    zone: Res<super::Zone>,
    time: Res<Time>,
    mut q: Query<(&PrintMesh, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut last: Local<(u32, f32)>,
) {
    let now = time.elapsed_secs();
    if tracks.version == last.0 && now - last.1 < 3.0 {
        return;
    }
    *last = (tracks.version, now);
    let prints: Vec<Print> = tracks.prints().copied().collect();
    for (m, mut vis) in &mut q {
        vis.set_if_neq(if prints.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
        if let Some(mut mesh) = meshes.get_mut(&m.0) {
            *mesh = print_mesh(
                &prints,
                |x, z| zone.ground(x, z),
                |x, z| Vec3::from_array(zone.hm.normal([x, z])),
            );
        }
    }
}

#[derive(Component)]
struct Breath(Handle<StandardMaterial>);

/// A soft round puff, as an alpha mask.
fn puff_texture() -> Image {
    let n = 32u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = ((1.0 - r).max(0.0).powf(1.6) * 255.0) as u8;
            data.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

fn spawn_breath(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.92, 0.94, 0.97, 0.0),
        base_color_texture: Some(images.add(puff_texture())),
        perceptual_roughness: 1.0,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Name::new("Breath"),
        Breath(material.clone()),
        Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
        MeshMaterial3d(material),
        Transform::from_scale(Vec3::ZERO),
        Visibility::Hidden,
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

/// One breath's puff, `t` s into the breath (`period` s long): how big
/// (m) and how thick (0..1), and how far it has drifted out (m). Out on
/// the first part of the breath, nothing while breathing in.
pub fn puff(t: f32, period: f32) -> (f32, f32, f32) {
    let out = 0.4 * period;
    if t > out {
        return (0.0, 0.0, 0.0);
    }
    let k = t / out;
    let size = 0.07 + 0.3 * k.sqrt();
    let thick = (k / 0.15).min(1.0) * (1.0 - k).powf(1.5);
    (size, thick, 0.45 * k * out)
}

/// In the cold, a puff with each breath out: low in front of the face,
/// drifting off and thinning. Quicker when running.
fn breathe(
    snow: Res<Snow>,
    conditions: Res<Conditions>,
    time: Res<Time>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    bodies: Query<&crate::player::Body, With<crate::rig::LocalPlayer>>,
    mut q: Query<(&Breath, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut st: Local<(f32, Vec3, Vec3)>,
) {
    let Ok((b, mut xf, mut vis)) = q.single_mut() else {
        return;
    };
    let Some(eye) = cams.iter().next() else {
        return;
    };
    if !snow.cold(eye.translation().y) {
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    let running = bodies
        .iter()
        .next()
        .is_some_and(|b| b.velocity.length() > 3.5);
    let period = if running { 2.2 } else { 3.8 };
    let dt = time.delta_secs();
    let (t, from, way) = &mut *st;
    *t += dt;
    if *t >= period {
        *t -= period;
    }
    if *t < dt * 1.5 {
        // A new breath: from the mouth, the way the head faces.
        let f = eye.forward().as_vec3();
        let flat = Vec3::new(f.x, 0.0, f.z).normalize_or(Vec3::NEG_Z);
        *from = eye.translation() + flat * 0.16 - Vec3::Y * 0.12;
        *way = (flat + Vec3::Y * 0.05).normalize();
    }
    let (size, thick, drift) = puff(*t, period);
    let wind = Vec3::new(0.3, 0.0, 0.15) * conditions.wind() * *t;
    let at = *from + *way * drift + wind;
    let to_eye = (eye.translation() - at).normalize_or(Vec3::Z);
    *xf = Transform::from_translation(at)
        .looking_to(-to_eye, Vec3::Y)
        .with_scale(Vec3::splat(size.max(1e-3)));
    if let Some(mut m) = materials.get_mut(&b.0) {
        m.base_color.set_alpha(0.32 * thick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::weather::Hour;

    fn map(name: &str) -> MapDef {
        crate::world::load_map_def(name)
    }

    #[test]
    fn the_first_snow_comes_late_to_high_valleys_on_cold_cycles() {
        let c = |k, cycle| Conditions::compose(k, Hour::Day, cycle);
        let kettle = map("kettle_pass");
        let marrow = map("marrow_creek");
        let snowy: Vec<u32> = (0..300)
            .filter(|cy| first_snow(4, *cy, &kettle, &c(WeatherKind::Drizzle, *cy)).is_some())
            .collect();
        assert!((60..=140).contains(&snowy.len()), "{} cycles", snowy.len());
        assert!(snowy.iter().all(|cy| *cy >= 2), "not in the first raids");
        for cy in &snowy {
            let d = c(WeatherKind::Drizzle, *cy);
            let t = first_snow(4, *cy, &kettle, &d).unwrap();
            assert!(
                (20.0 * 60.0..=40.0 * 60.0).contains(&t),
                "late in the raid: {t}"
            );
            assert_eq!(snowfall(4, *cy, &kettle, &d, t - 1.0), 0.0);
            assert_eq!(snowfall(4, *cy, &kettle, &d, t + 1.0), 1.0);
        }
        let first = snowy[0];
        assert!(first_snow(4, first, &marrow, &c(WeatherKind::Drizzle, first)).is_none());
        assert!(first_snow(4, first, &kettle, &c(WeatherKind::Downpour, first)).is_none());
        assert!(first_snow(4, first, &kettle, &c(WeatherKind::Clearing, first)).is_none());
    }

    #[test]
    fn a_cold_valley_snows_with_its_weather_and_never_melts_bare() {
        let mut cold = map("marrow_creek");
        cold.snow_line = Some(-100.0);
        cold.snow = 0.6;
        let c = |k| Conditions::compose(k, Hour::Day, 5);
        assert_eq!(
            snowfall(1, 5, &cold, &c(WeatherKind::Downpour), 0.0),
            1.0,
            "a blizzard"
        );
        assert!(snowfall(1, 5, &cold, &c(WeatherKind::Drizzle), 0.0) > 0.0);
        assert_eq!(snowfall(1, 5, &cold, &c(WeatherKind::Clearing), 0.0), 0.0);
        let mut s = Snow {
            line: snow_line(&cold),
            floor: cold.snow,
            cover: 1.0,
            ..default()
        };
        for _ in 0..(120 * 60) {
            s.step(0.0, 1.0);
        }
        assert!(
            (s.cover - 0.6).abs() < 1e-3,
            "melts to its snowfields: {}",
            s.cover
        );
    }

    #[test]
    fn snow_lies_above_the_line_over_minutes_and_melts_slower() {
        let mut s = Snow {
            line: snow_line(&map("kettle_pass")),
            ..default()
        };
        for _ in 0..(3 * 60) {
            s.step(1.0, 1.0);
        }
        assert!(s.cover > 0.2 && s.cover < 0.7, "three minutes: {}", s.cover);
        for _ in 0..(15 * 60) {
            s.step(1.0, 1.0);
        }
        assert!(s.cover > 0.9);
        assert!(
            s.lies(60.0) > 0.9 && s.lies(15.0) == 0.0,
            "the tops, not the valley"
        );
        assert!(s.cold(32.0) && !s.cold(5.0));
        let lying = s.cover;
        for _ in 0..(5 * 60) {
            s.step(0.0, 1.0);
        }
        assert!(
            s.cover < lying && s.cover > 0.6,
            "melting slowly: {}",
            s.cover
        );
        let none = Snow::default();
        assert_eq!(none.lies(500.0), 0.0);
        assert!(!none.cold(500.0));
    }

    #[test]
    fn prints_keep_the_way_and_fill_in() {
        let mut t = Tracks::default();
        let (a, b) = (
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(2).unwrap(),
        );
        for i in 0..10 {
            for (who, x) in [(a, 0.0), (b, 50.0)] {
                t.add(Print {
                    at: Vec3::new(x, 40.0, -(i as f32)),
                    yaw: 0.0,
                    left: i % 2 == 0,
                    by: who,
                    fill: 0.0,
                });
            }
        }
        // Found where they were, followed the way they went.
        let first = *t.newest_near(Vec3::new(0.0, 40.0, -3.0), 0.5).unwrap();
        assert_eq!(first.by, a);
        let next = t.after(&first).unwrap();
        assert_eq!(next.by, a);
        assert!(next.at.z < first.at.z);
        // Falling snow fills them in minutes; with none, they keep longer.
        let mut still = Tracks::default();
        still.add(first);
        still.weather(0.0, 5.0 * 60.0);
        assert_eq!(still.len(), 1);
        t.weather(1.0, 3.0 * 60.0);
        assert!(t.is_empty());
        // The ring keeps the newest.
        let mut r = Tracks::default();
        for i in 0..(MAX_PRINTS + 10) {
            r.add(Print {
                at: Vec3::X * i as f32,
                yaw: 0.0,
                left: false,
                by: a,
                fill: 0.0,
            });
        }
        assert_eq!(r.len(), MAX_PRINTS);
        assert_eq!(r.prints().next().unwrap().at.x, 10.0);
    }

    #[test]
    fn breath_puffs_out_then_nothing_while_breathing_in() {
        let (s0, k0, _) = puff(0.0, 4.0);
        let (s1, k1, d1) = puff(0.5, 4.0);
        assert!(s1 > s0 && k1 > k0 && d1 > 0.0);
        assert_eq!(puff(3.0, 4.0).1, 0.0);
        assert!(
            puff(1.55, 4.0).1 < 0.05,
            "thinned to nothing at the end of the breath out"
        );
    }
}
