//! Light in the fog: the sun's shafts through windows into dark rooms,
//! and through gaps in the trees at dawn and dusk. Cheap on purpose: no
//! raymarch, just a few crossed additive cards along the light per shaft
//! (`assets/shaders/shaft.wgsl` keeps them soft, edge-on-proof and
//! brightest looking toward the sun), merged into a mesh per building and
//! per stretch of forest, drawn only near the eye.
//!
//! They need a sun to come through: strong in a clearing, faint in
//! drizzle, none in fog or a downpour or at night. The sun is fixed for a
//! raid's hour, so the shafts are built when the hour (or the weather)
//! changes and not again.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use std::collections::HashMap;

use super::Zone;
use super::buildings::{Building, Shape};
use super::weather::{Conditions, Hour, WeatherKind};
use crate::render::materials::Mat;

pub struct ShaftsPlugin;

impl Plugin for ShaftsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ShaftMaterial>::default())
            .add_systems(Update, build_shafts.after(super::sky::WeatherLook));
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct ShaftParams {
    /// rgb: the sun's colour; w: strength (0 off).
    pub color: Vec4,
    /// xyz: the light's travel; w: how far from the eye they're drawn.
    pub sun: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct ShaftMaterial {
    #[uniform(0)]
    pub params: ShaftParams,
}

impl Material for ShaftMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/shaft.wgsl".into()
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

/// How far from the eye shafts are drawn (m): past that the fog has them,
/// and every one is overdraw.
pub const REACH: f32 = 45.0;
/// How far a window's light reaches into a room (m).
const ROOM_DEPTH: f32 = 4.5;
/// The forest's canopy, where a gap's shaft starts (m up).
const CANOPY: f32 = 13.0;
/// Forest shafts are merged by squares this big (m), each its own draw
/// within reach.
const CHUNK: f32 = 64.0;

#[derive(Component)]
struct Shaft;

/// The light's strength through the cloud, for shafts: the hour's sun
/// and how much the weather lets through.
pub fn strength(c: &Conditions) -> f32 {
    // EXTRACTION_NO_SHAFTS=1: none, to see or measure the difference.
    if std::env::var_os("EXTRACTION_NO_SHAFTS").is_some() {
        return 0.0;
    }
    let through = match c.kind {
        WeatherKind::Clearing => 1.0,
        WeatherKind::Drizzle => 0.45,
        WeatherKind::PeaSouper | WeatherKind::Downpour => 0.0,
    };
    through * c.hour.sun()
}

/// Whether the trees get shafts: only with the sun low enough to come in
/// under the canopy.
pub fn forest_shafts(c: &Conditions) -> bool {
    matches!(c.hour, Hour::Dawn | Hour::Dusk) && strength(c) > 0.0
}

/// Mesh being built: cards with (across, along) uvs.
#[derive(Default)]
struct Cards {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    /// Each vertex's floor height (world y) in UV1.x: the shader fades the
    /// light out just above it (`f32::NEG_INFINITY`: no floor).
    floor: Vec<[f32; 2]>,
    /// The floor height the next cards stand on.
    floor_y: f32,
}

impl Cards {
    /// A card from `a` across to `b` (where the light enters), swept
    /// `sweep` along the light.
    fn card(&mut self, a: Vec3, b: Vec3, sweep: Vec3) {
        let n = (b - a).cross(sweep).normalize_or(Vec3::Y);
        let base = self.pos.len() as u32;
        for (p, uv) in [
            (a, [0.0, 0.0]),
            (b, [1.0, 0.0]),
            (b + sweep, [1.0, 1.0]),
            (a + sweep, [0.0, 1.0]),
        ] {
            self.pos.push(p.to_array());
            self.nrm.push(n.to_array());
            self.uv.push(uv);
            self.floor.push([self.floor_y, 0.0]);
        }
        self.idx
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A shaft of round section: three cards about its axis.
    fn round(&mut self, start: Vec3, sweep: Vec3, width: f32) {
        let axis = sweep.normalize_or(Vec3::NEG_Y);
        let side = axis.cross(Vec3::Y).normalize_or(Vec3::X);
        for k in 0..3 {
            let w = Quat::from_axis_angle(axis, k as f32 * std::f32::consts::FRAC_PI_3) * side;
            self.card(start - w * width * 0.5, start + w * width * 0.5, sweep);
        }
    }

    fn build(self) -> Option<Mesh> {
        (!self.idx.is_empty()).then(|| {
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.floor)
            .with_inserted_indices(Indices::U32(self.idx))
        })
    }
}

/// A window's sill above the floor (m), for where its shaft lands.
const SILL: f32 = 0.85;

/// A window's shaft: the pane's rectangle swept along the light into the
/// room, as three cards (across its width, its height, and a diagonal).
/// `pane` is the glass box's half size and pose (world); `out` its face
/// toward the outside.
fn window_shaft(cards: &mut Cards, half: Vec3, pose: Transform, out: Vec3, light: Vec3) {
    window_shaft_to(cards, half, pose, out, light, f32::INFINITY);
}

/// [`window_shaft`], stopping `depth` along the light at the most (a
/// hole in a roof's shaft at the floor).
fn window_shaft_to(
    cards: &mut Cards,
    half: Vec3,
    pose: Transform,
    out: Vec3,
    light: Vec3,
    depth: f32,
) {
    // The pane's two long axes.
    let axes = [
        pose.rotation * Vec3::X,
        pose.rotation * Vec3::Y,
        pose.rotation * Vec3::Z,
    ];
    let mut long: Vec<(Vec3, f32)> = (0..3).map(|k| (axes[k], half[k])).collect();
    long.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (u, hu) = long[0];
    let (v, hv) = long[1];
    let c = pose.translation;
    // In from the glass, as far as a room goes, along the light (which
    // comes in through the outer face, so travels against `out`).
    if light.dot(out) > -0.15 {
        return;
    }
    let sweep = light * (ROOM_DEPTH / (-light.dot(out)).max(0.4)).min(depth);
    // The floor it lands on: under a window its sill (the pane's lowest
    // edge, less a sill's height); a roof hole's is `depth` down.
    let low = [u * hu, v * hv].iter().map(|d| d.y.abs()).sum::<f32>();
    cards.floor_y = if depth.is_finite() {
        c.y + sweep.y
    } else {
        c.y - low - SILL
    };
    cards.card(c - u * hu, c + u * hu, sweep);
    cards.card(c - v * hv, c + v * hv, sweep);
    cards.card(c - u * hu - v * hv, c + u * hu + v * hv, sweep);
}

/// Gaps in a forest: points with no tree within 2.5 m but trees within
/// 8, on a jittered 7 m grid (deterministic per map).
fn forest_gaps(zone: &Zone) -> Vec<Vec3> {
    let trees = super::trees::scatter_trees(zone);
    let cell = 4.0f32;
    let key = |p: Vec2| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32);
    let mut bins: HashMap<(i32, i32), Vec<Vec2>> = HashMap::new();
    for t in &trees {
        let p = Vec2::new(t.pos.x, t.pos.z);
        bins.entry(key(p)).or_default().push(p);
    }
    let nearest = |p: Vec2| {
        let (i, j) = key(p);
        let mut best = f32::INFINITY;
        for di in -2..=2 {
            for dj in -2..=2 {
                if let Some(v) = bins.get(&(i + di, j + dj)) {
                    for q in v {
                        best = best.min(q.distance(p));
                    }
                }
            }
        }
        best
    };
    let mut rng = extraction_core::noise::Rng::new(0x5A_F7);
    let half = zone.hm.origin.abs();
    let mut out = Vec::new();
    let step = 7.0;
    let mut z = -half;
    while z < half {
        let mut x = -half;
        while x < half {
            let p = Vec2::new(x + rng.range(-2.5, 2.5), z + rng.range(-2.5, 2.5));
            let d = nearest(p);
            if zone.forest_cover(p.x, p.y) > 0.5 && (2.5..8.0).contains(&d) {
                out.push(Vec3::new(p.x, zone.ground(p.x, p.y), p.y));
            }
            x += step;
        }
        z += step;
    }
    out
}

#[derive(Default)]
struct Built {
    key: Option<(Hour, WeatherKind, usize)>,
    material: Option<Handle<ShaftMaterial>>,
    gaps: Option<Vec<Vec3>>,
}

fn build_shafts(
    mut commands: Commands,
    conditions: Res<Conditions>,
    weather: Res<super::sky::Weather>,
    zone: Res<Zone>,
    buildings: Query<(&Building, &GlobalTransform)>,
    old: Query<Entity, With<Shaft>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShaftMaterial>>,
    mut built: Local<Built>,
) {
    let key = (conditions.hour, conditions.kind, buildings.iter().len());
    if built.key == Some(key) {
        return;
    }
    built.key = Some(key);
    for e in &old {
        commands.entity(e).despawn();
    }
    let light = weather.sun_dir.normalize_or(Vec3::NEG_Y);
    let strength = strength(&conditions);
    let sun = weather.sun_color.to_linear();
    let params = ShaftParams {
        // Radiance of a lit shaft (cd/m2-ish, before the eye's exposure).
        color: Vec4::new(sun.red, sun.green, sun.blue, 200.0 * strength),
        sun: light.extend(REACH),
    };
    let material = match &built.material {
        Some(h) => {
            if let Some(mut m) = materials.get_mut(h) {
                m.params = params;
            }
            h.clone()
        }
        None => {
            let h = materials.add(ShaftMaterial { params });
            built.material = Some(h.clone());
            h
        }
    };
    if strength <= 0.0 {
        return;
    }
    let mut spawn = |mesh: Mesh| {
        let (mesh, aabb, at) = crate::render::centred(&mut meshes, mesh);
        commands.spawn((
            Name::new("Light shafts"),
            Shaft,
            mesh,
            aabb,
            at,
            MeshMaterial3d(material.clone()),
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: REACH..REACH + 10.0,
                use_aabb: true,
            },
            NotShadowCaster,
            NotShadowReceiver,
        ));
    };
    // Windows: each building's glass that faces the sun.
    for (b, xf) in &buildings {
        let Some(geo) = super::buildings::prefab(&b.prefab) else {
            continue;
        };
        let at = xf.compute_transform();
        let mut cards = Cards::default();
        for piece in geo.pieces.iter().filter(|p| p.mat == Mat::Glass) {
            let Shape::Box(size) = piece.shape else {
                continue;
            };
            let pose = at * piece.at;
            // The pane's thin axis, pointing away from the building's middle.
            let k = size.min_position();
            let axis = pose.rotation * Vec3::AXES[k];
            let from_mid = pose.translation - at.translation;
            let out = if axis.dot(from_mid) >= 0.0 {
                axis
            } else {
                -axis
            };
            window_shaft(&mut cards, size * 0.5, pose, out, light);
        }
        // A ruin's holed roof (`Geo::skylights`): down from the eaves to
        // the floor.
        for (c, half) in &geo.skylights {
            let pose = at * Transform::from_translation(*c);
            let depth = ROOM_DEPTH.min((c.y - 0.5).max(0.5) / (-light.y).max(0.05));
            window_shaft_to(
                &mut cards,
                Vec3::new(half.x, 0.01, half.y),
                pose,
                at.rotation * Vec3::Y,
                light,
                depth,
            );
        }
        if let Some(mesh) = cards.build() {
            spawn(mesh);
        }
    }
    // The forest at dawn and dusk.
    if forest_shafts(&conditions) {
        let gaps = built.gaps.get_or_insert_with(|| forest_gaps(&zone)).clone();
        let mut chunks: HashMap<(i32, i32), Cards> = HashMap::new();
        let mut rng = extraction_core::noise::Rng::new(0x5A_F8);
        // From the canopy down to the ground, along the light.
        let sweep = light * (CANOPY / (-light.y).max(0.25)).min(26.0);
        for g in gaps {
            let width = rng.range(0.9, 2.2);
            let c = chunks
                .entry(((g.x / CHUNK).floor() as i32, (g.z / CHUNK).floor() as i32))
                .or_default();
            c.floor_y = g.y;
            c.round(g - sweep, sweep, width);
        }
        for (_, c) in chunks {
            if let Some(mesh) = c.build() {
                spawn(mesh);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shafts_need_a_sun_through_the_cloud() {
        let at = |k, h| strength(&Conditions::compose(k, h, 1));
        assert!(at(WeatherKind::Clearing, Hour::Dusk) > at(WeatherKind::Drizzle, Hour::Dusk));
        assert_eq!(at(WeatherKind::Downpour, Hour::Day), 0.0);
        assert_eq!(at(WeatherKind::PeaSouper, Hour::Dawn), 0.0);
        assert_eq!(at(WeatherKind::Clearing, Hour::Night), 0.0);
        let forest = |k, h| forest_shafts(&Conditions::compose(k, h, 1));
        assert!(forest(WeatherKind::Clearing, Hour::Dawn));
        assert!(!forest(WeatherKind::Clearing, Hour::Day));
        assert!(!forest(WeatherKind::Downpour, Hour::Dusk));
    }

    /// Through a hole in a roof, a high sun's shaft stops at the floor.
    #[test]
    fn a_holed_roof_lets_the_light_down_to_the_floor() {
        let pose = Transform::from_xyz(1.0, 3.2, -1.0);
        let half = Vec3::new(1.5, 0.01, 1.2);
        let light = Vec3::new(-0.3, -0.95, 0.1).normalize();
        let mut cards = Cards::default();
        window_shaft_to(&mut cards, half, pose, Vec3::Y, light, (3.2 - 0.5) / 0.95);
        assert!(!cards.idx.is_empty());
        let low = cards.pos.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        assert!((low - 0.5).abs() < 0.05, "down to {low}");
        // And none up out of it.
        let mut none = Cards::default();
        window_shaft_to(&mut none, half, pose, Vec3::Y, -light, 10.0);
        assert!(none.idx.is_empty());
    }

    #[test]
    fn a_window_lets_the_light_in_and_not_out() {
        // A pane in a wall facing +X; the low sun shining in from +X.
        let pose = Transform::from_xyz(5.0, 1.5, 0.0);
        let half = Vec3::new(0.01, 0.6, 0.5);
        let light = Vec3::new(-0.9, -0.34, 0.0).normalize();
        let mut cards = Cards::default();
        window_shaft(&mut cards, half, pose, Vec3::X, light);
        assert_eq!(cards.idx.len(), 18, "three cards");
        // Every card runs into the room (toward -X) and down.
        let far: Vec<Vec3> = cards.pos.iter().map(|p| Vec3::from(*p)).collect();
        assert!(far.iter().all(|p| p.x <= 5.0 + 1e-3));
        assert!(far.iter().any(|p| p.x < 1.0 && p.y < 1.0));
        // With the sun behind the wall, no shaft.
        let mut none = Cards::default();
        window_shaft(
            &mut none,
            half,
            pose,
            Vec3::X,
            Vec3::new(0.9, -0.34, 0.0).normalize(),
        );
        assert!(none.idx.is_empty());
    }
}
