//! Rain you can see: a field of streaks animated entirely in a vertex
//! shader (`assets/shaders/rain.wgsl`), thickened or thinned by the
//! weather's rain intensity. Under the roofs nearest the camera nothing
//! falls (`Shelters`).

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
            .add_systems(
                Update,
                (rain_intensity, ash_fall, snow_fall, shelter_nearby),
            );
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct RainParams {
    pub params: Vec4,
    pub wind: Vec4,
    /// x: how bright the streaks are against a day's (the hour's light,
    /// as the adapted eye sees it: at dusk they don't glare).
    pub light: Vec4,
}

/// Roofed volumes near the camera: the shader drops any streak inside
/// one. Two vec4 each: (centre x, centre z, bottom y, top y) and (half
/// width, half depth, cos yaw, sin yaw); a zero half width is unused.
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct Shelters {
    pub boxes: [Vec4; 2 * SHELTERS],
}

/// How many roofs the shader checks: the nearest to the camera.
pub const SHELTERS: usize = 8;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct RainMaterial {
    #[uniform(0)]
    pub rain: RainParams,
    #[uniform(1)]
    pub shelters: Shelters,
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

pub(crate) const STREAKS: usize = 2500;
pub(crate) const BOX: f32 = 24.0;

pub(crate) fn streak_mesh() -> Mesh {
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
            light: Vec4::ONE,
        },
        shelters: Shelters::default(),
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

/// Ash drifting down, in a valley that burned (`MapDef::ash`): the rain
/// field again, slowed to a drift, flakes for streaks. Rain washes it out.
#[derive(Component)]
struct Ash(Handle<RainMaterial>);

fn ash_fall(
    mut commands: Commands,
    conditions: Res<Conditions>,
    zone: Res<super::Zone>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RainMaterial>>,
    mut ash: Query<(&Ash, &mut Visibility)>,
    mut spawned: Local<bool>,
) {
    let amount = zone.map.ash;
    if amount <= 0.0 {
        return;
    }
    if !*spawned {
        *spawned = true;
        let m = materials.add(RainMaterial {
            rain: RainParams {
                // Slow, short, and a fraction as many.
                params: Vec4::new(
                    amount * (1.0 - conditions.rain).clamp(0.0, 1.0),
                    0.55,
                    0.035,
                    BOX,
                ),
                // y: flakes (pale, wobbling); w: flake width.
                wind: Vec4::new(0.35, 1.0, 0.15, 0.03),
                // The hour's light from the start: the weather may have
                // settled before the ash fell (a night raid's flakes were
                // left at a day's brightness, white points like stars).
                light: Vec4::new(streak_light(&conditions), 1.0, 1.0, 1.0),
            },
            shelters: Shelters::default(),
        });
        commands.spawn((
            Name::new("Ash"),
            Ash(m.clone()),
            Mesh3d(meshes.add(streak_mesh())),
            MeshMaterial3d(m),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
        ));
        return;
    }
    if !conditions.is_changed() {
        return;
    }
    for (a, mut vis) in &mut ash {
        *vis = if conditions.rain < 0.5 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(mut m) = materials.get_mut(&a.0) {
            m.rain.params.x = amount * (1.0 - conditions.rain).clamp(0.0, 1.0);
            m.rain.light.x = streak_light(&conditions);
        }
    }
}

/// Snow falling round the eye, where it's above the snow line: the ash's
/// field again, white, slower and larger.
#[derive(Component)]
struct Flakes(Handle<RainMaterial>);

fn snow_fall(
    mut commands: Commands,
    conditions: Res<Conditions>,
    snow: Option<Res<super::snow::Snow>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RainMaterial>>,
    mut flakes: Query<(&Flakes, &mut Visibility)>,
    mut done: Local<Option<f32>>,
) {
    let Some(snow) = snow else {
        return;
    };
    let eye_y = cams.iter().next().map_or(0.0, |g| g.translation().y);
    let here = snow.falling * snow.above(eye_y);
    if done.is_some_and(|d| (d - here).abs() < 0.02) && !conditions.is_changed() {
        return;
    }
    if done.is_none() && here <= 0.0 {
        return;
    }
    *done = Some(here);
    let params = |here: f32| RainParams {
        params: Vec4::new(here, 1.1, 0.05, BOX),
        // A blizzard drives it nearly flat.
        wind: Vec4::new(
            0.25 + 3.0 * conditions.wind() * here,
            2.0,
            0.1 + conditions.wind() * here,
            0.045,
        ),
        light: Vec4::new(streak_light(&conditions), 1.0, 1.0, 1.0),
    };
    if flakes.is_empty() {
        let m = materials.add(RainMaterial {
            rain: params(here),
            shelters: Shelters::default(),
        });
        commands.spawn((
            Name::new("Snow"),
            Flakes(m.clone()),
            Mesh3d(meshes.add(streak_mesh())),
            MeshMaterial3d(m),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
        ));
        return;
    }
    for (f, mut vis) in &mut flakes {
        vis.set_if_neq(if here > 0.01 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
        if let Some(mut m) = materials.get_mut(&f.0) {
            m.rain = params(here);
        }
    }
}

/// The streaks' brightness at the hour: the light, less what the eyes
/// make up (`render::grade::Adaptation`). At night there's next to no
/// light for a flake to catch: ash or a drop shows only near a lamp or in
/// a torch's beam, not as a field of white points like stars.
pub(crate) fn streak_light(c: &Conditions) -> f32 {
    let stops = crate::render::grade::Adaptation::target(c.light);
    let dark = (c.light / 0.1).clamp(0.25, 1.0);
    (c.light * 2f32.powf(stops) * dark).clamp(0.02, 1.0)
}

/// How much of the rain still falls as rain where the eye is: above the
/// snow line while it snows it comes down as flakes instead
/// (`world::snow`).
fn rain_here(c: &Conditions, snow: Option<&super::snow::Snow>, eye_y: f32) -> f32 {
    let flakes = snow.map_or(0.0, |s| s.falling * s.above(eye_y));
    c.rain * (1.0 - flakes)
}

fn rain_intensity(
    conditions: Res<Conditions>,
    snow: Option<Res<super::snow::Snow>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    rain: Query<(&Rain, &mut Visibility)>,
    mut materials: ResMut<Assets<RainMaterial>>,
    mut done: Local<Option<f32>>,
) {
    let eye_y = cams.iter().next().map_or(0.0, |g| g.translation().y);
    let here = rain_here(&conditions, snow.as_deref(), eye_y);
    if !conditions.is_changed() && done.is_some_and(|d| (d - here).abs() < 0.02) {
        return;
    }
    *done = Some(here);
    for (r, mut vis) in rain {
        *vis = if here > 0.01 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Some(mut m) = materials.get_mut(&r.0) {
            m.rain.params.x = here;
            m.rain.light.x = streak_light(&conditions);
            m.rain.wind = Vec4::new(1.2 + 2.0 * conditions.rain, 0.0, 0.6 + conditions.rain, 0.0);
        }
    }
}

/// The least share of a building's footprint under its roof for rain to
/// keep out of it: a burnt or drowned house open to the sky lets it in.
pub const SHELTER_ROOFED: f32 = 0.8;

/// A building's box for the shader, if it's roofed: (centre x, centre z,
/// bottom y, top y), (half width, half depth, cos yaw, sin yaw), just
/// inside its walls (rain still falls past the eaves and in the doorway).
/// From the buildings the game takes for indoors (`robots::doors`
/// `Entrances`), every one with a way in: a list of prefabs kept by hand
/// went stale (Models' new plans, `house_plank_b`, `post_office`, and every
/// damaged and drowned variant, had it raining in the front room).
pub fn shelter(shell: &crate::robots::doors::Shell) -> Option<[Vec4; 2]> {
    if let Some(cover) = shell.cover {
        let (roofed, cells) = cover.roofed();
        if cells == 0 || (roofed as f32) < SHELTER_ROOFED * cells as f32 {
            return None;
        }
    }
    let at = shell.xf.translation;
    // The game turns a placement by -rot about +Y; the shader undoes it.
    let (yaw, _, _) = shell.xf.rotation.to_euler(EulerRot::YXZ);
    let (s, c) = (-yaw).sin_cos();
    Some([
        Vec4::new(at.x, at.z, at.y - 2.0, at.y + shell.height),
        Vec4::new(shell.half.x - 0.15, shell.half.y - 0.15, c, s),
    ])
}

/// The CPU twin of `rain.wgsl`'s `sheltered`, for one box.
pub fn sheltered(b: &[Vec4; 2], p: Vec3) -> bool {
    let (a, b) = (b[0], b[1]);
    let d = Vec2::new(p.x - a.x, p.z - a.y);
    let lx = d.x * b.z + d.y * b.w;
    let lz = -d.x * b.w + d.y * b.z;
    lx.abs() < b.x && lz.abs() < b.y && p.y > a.z && p.y < a.w
}

/// Every roof on the map, as the shader wants it, with its centre, and how
/// many buildings it was made from.
#[derive(Default)]
struct Roofs(Vec<(Vec2, [Vec4; 2])>, usize);

/// Hands the roofs nearest the camera to the rain and ash, whenever the
/// camera has moved a few metres (or the buildings have come in).
fn shelter_nearby(
    entrances: Option<Res<crate::robots::doors::Entrances>>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    rain: Query<&MeshMaterial3d<RainMaterial>>,
    mut materials: ResMut<Assets<RainMaterial>>,
    mut roofs: Local<Roofs>,
    mut last: Local<Option<Vec3>>,
    mut count: Local<usize>,
) {
    let shells = entrances.as_ref().map_or(&[][..], |e| &e.shells[..]);
    let fresh = roofs.1 != shells.len();
    if fresh {
        *roofs = Roofs(
            shells
                .iter()
                .filter_map(|s| shelter(s).map(|b| (s.xf.translation.xz(), b)))
                .collect(),
            shells.len(),
        );
    }
    let Some(eye) = cams.iter().next().map(|g| g.translation()) else {
        return;
    };
    // Re-sort when the camera moves, or a new rain material appears.
    let n = rain.iter().count();
    if !fresh && last.is_some_and(|l| l.distance(eye) < 3.0) && *count == n {
        return;
    }
    *last = Some(eye);
    *count = n;
    let mut near: Vec<&(Vec2, [Vec4; 2])> = roofs.0.iter().collect();
    near.sort_by(|a, b| {
        a.0.distance_squared(eye.xz())
            .total_cmp(&b.0.distance_squared(eye.xz()))
    });
    let mut shelters = Shelters::default();
    for (i, (_, b)) in near.iter().take(SHELTERS).enumerate() {
        shelters.boxes[2 * i] = b[0];
        shelters.boxes[2 * i + 1] = b[1];
    }
    for m in &rain {
        if let Some(mut mat) = materials.get_mut(&m.0) {
            mat.shelters = shelters;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::robots::doors::Shell;
    use crate::world::buildings;

    /// Under any roofed building with a way in (every prefab, at any turn)
    /// no rain falls in the room, and past its walls it does; a building
    /// open to the sky (burnt, drowned) lets it in. The game's own
    /// indoors (`Shell::contains`) agrees about the room's middle.
    #[test]
    fn no_rain_falls_under_any_roof() {
        // Models' plans and the variants a hand-kept list had missed (the
        // plans aren't in `PREFABS` either: named too).
        let named = [
            "house_plank_b",
            "house_plank_c",
            "post_office",
            "house_brick_damaged",
        ];
        let mut checked = Vec::new();
        for id in buildings::PREFABS.into_iter().chain(named) {
            let Some(geo) = buildings::prefab(id) else {
                continue;
            };
            if geo.doors.is_empty() {
                continue;
            }
            for rot in [0.0f32, 90.0, 180.0, 237.0] {
                let xf = Transform::from_xyz(12.0, 3.0, -7.0)
                    .with_rotation(Quat::from_rotation_y(-rot.to_radians()));
                let shell = Shell::of(Entity::PLACEHOLDER, id, xf);
                let Some(b) = shelter(&shell) else {
                    // Open to the sky: rain is right.
                    let (r, n) = shell.cover.map_or((1, 1), |c| c.roofed());
                    assert!(
                        (r as f32) < SHELTER_ROOFED * n as f32,
                        "{id}: roofed but no shelter"
                    );
                    continue;
                };
                let room = xf.transform_point(Vec3::new(0.0, 1.5, 0.0));
                assert!(sheltered(&b, room), "{id} at {rot}: raining in the room");
                assert!(
                    shell.contains(room),
                    "{id} at {rot}: the room's middle isn't indoors"
                );
                let past = xf.transform_point(Vec3::new(shell.half.x + 1.5, 1.5, 0.0));
                assert!(!sheltered(&b, past), "{id} at {rot}: dry past the wall");
            }
            checked.push(id);
        }
        // Models' plans and the variants a hand-kept list had missed.
        for id in [
            "house_plank_b",
            "house_plank_c",
            "post_office",
            "house_brick_damaged",
        ] {
            assert!(checked.contains(&id), "{id} not checked");
        }
    }
}
