//! A searchlight's look (`searchlight`): the lamp head, the beam's haze
//! down its throw, and the pool where it lands, by day as well as night.
//!
//! - **The lamp.** A dark drum with a pale lens and a warning rim (orange
//!   red) round it, both lit through fog: at 100 m by day you can see
//!   which way it looks.
//! - **The beam.** A long cone of lit haze (`shaders/searchlight.wgsl`),
//!   stopping where the lamp's ray lands. Pale and milky by day, bright at
//!   night.
//! - **The pool.** Where it lands, a pale patch with a warning rim
//!   crawling round it: what it's watching, on the ground.
//! - **On you.** Set on something (`Beam::target`), the beam goes
//!   takes a red cast and the rims brighten: it has seen, or means to.
//!
//! Never warm: the user's rule for lit tells is warning, not shelter.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

use super::searchlight::Beam;

pub struct SearchlightLookPlugin;

impl Plugin for SearchlightLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SearchMaterial>::default())
            .add_systems(
                Update,
                ((dress_lamps, light_lamps).chain(), drop_pools, hold_heads),
            );
    }
}

/// The beam's pale light (linear), and with a red cast when it's on
/// something (the warning is the rims: the beam stays a cold light, never
/// a warm one).
pub const PALE: Vec3 = Vec3::new(0.85, 0.92, 1.0);
pub const ON_YOU: Vec3 = Vec3::new(1.0, 0.8, 0.78);
/// The warning rim, on the lamp and round the pool.
pub const RIM: Vec3 = Vec3::new(1.0, 0.28, 0.06);
/// The lamp drum's radius and depth (m).
pub const DRUM_R: f32 = 0.32;
const DRUM_DEEP: f32 = 0.45;

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct SearchParams {
    /// rgb: the light; w: how strong (0 off).
    pub light: Vec4,
    /// rgb: the warning rim; w: how much.
    pub rim: Vec4,
    /// x: 0 beam, 1 pool; y: the beam's length; z: daylight 0..1; w: how
    /// much the air shows.
    pub shape: Vec4,
    /// xyz: the way it throws (world).
    pub axis: Vec4,
    /// xyz: the lamp (world); w: tan of the half-angle.
    pub origin: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct SearchMaterial {
    #[uniform(0)]
    pub params: SearchParams,
}

impl Material for SearchMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/searchlight.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
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

/// A lamp's drawn pieces.
#[derive(Component)]
struct LampLook {
    beam: Entity,
    pool: Entity,
    beam_mat: Handle<SearchMaterial>,
    pool_mat: Handle<SearchMaterial>,
    lens: Handle<StandardMaterial>,
    rim: Handle<StandardMaterial>,
}

#[derive(Default)]
struct Shared {
    cone: Handle<Mesh>,
    plane: Handle<Mesh>,
    drum: Handle<Mesh>,
    lens: Handle<Mesh>,
    ring: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
}

/// A unit cone, apex at the origin, opening down -Z to radius 1 at z = -1:
/// uv x round it, y along it.
fn cone() -> Mesh {
    const ROUND: u32 = 24;
    const ALONG: u32 = 8;
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
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

fn dress_lamps(
    mut commands: Commands,
    lamps: Query<(Entity, &Beam), Added<Beam>>,
    mut shared: Local<Option<Shared>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut searches: ResMut<Assets<SearchMaterial>>,
) {
    for (e, l) in &lamps {
        let s = shared.get_or_insert_with(|| Shared {
            cone: meshes.add(cone()),
            plane: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(1.0))),
            // (Its axis down Z: the cylinders are turned onto it.)
            drum: meshes.add(Cylinder::new(DRUM_R, DRUM_DEEP)),
            lens: meshes.add(Cylinder::new(DRUM_R * 0.82, 0.02)),
            ring: meshes.add(Torus::new(DRUM_R * 0.84, DRUM_R * 1.02)),
            dark: materials.add(StandardMaterial {
                base_color: Color::srgb(0.08, 0.085, 0.09),
                perceptual_roughness: 0.55,
                metallic: 0.6,
                ..default()
            }),
        });
        let along = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        // A lamp is seen through fog, as a lamp is: it's the tell of
        // where it looks.
        let lens = materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.95, 1.0),
            emissive: LinearRgba::rgb(30.0, 33.0, 36.0),
            fog_enabled: false,
            unlit: true,
            ..default()
        });
        // The warning rim: lit through fog for the same reason.
        let rim = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.3, 0.06),
            emissive: LinearRgba::rgb(RIM.x * 14.0, RIM.y * 14.0, RIM.z * 14.0),
            fog_enabled: false,
            unlit: true,
            ..default()
        });
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(s.drum.clone()),
                MeshMaterial3d(s.dark.clone()),
                Transform::from_xyz(0.0, 0.0, DRUM_DEEP * 0.5).with_rotation(along),
            ));
            c.spawn((
                Mesh3d(s.lens.clone()),
                MeshMaterial3d(lens.clone()),
                Transform::from_xyz(0.0, 0.0, -0.005).with_rotation(along),
                NotShadowCaster,
                HeldHead(Vec3::new(0.0, 0.0, -0.005)),
            ));
            c.spawn((
                Mesh3d(s.ring.clone()),
                MeshMaterial3d(rim.clone()),
                Transform::from_xyz(0.0, 0.0, -0.01).with_rotation(along),
                NotShadowCaster,
                HeldHead(Vec3::new(0.0, 0.0, -0.01)),
            ));
        });
        let params = |pool: bool| SearchParams {
            light: PALE.extend(1.0),
            rim: RIM.extend(if pool { 1.0 } else { 0.0 }),
            shape: Vec4::new(if pool { 1.0 } else { 0.0 }, l.reach, 1.0, 1.0),
            axis: Vec3::NEG_Y.extend(0.0),
            origin: Vec4::new(0.0, 0.0, 0.0, l.half_angle.tan()),
        };
        let beam_mat = searches.add(SearchMaterial {
            params: params(false),
        });
        let pool_mat = searches.add(SearchMaterial {
            params: params(true),
        });
        let beam = commands
            .spawn((
                Name::new("Searchlight beam"),
                Mesh3d(s.cone.clone()),
                MeshMaterial3d(beam_mat.clone()),
                beam_transform(l.half_angle, l.landed),
                NotShadowCaster,
                NotShadowReceiver,
                ChildOf(e),
            ))
            .id();
        // The pool lies on the ground, not under the lamp: its own entity.
        let pool = commands
            .spawn((
                Name::new("Searchlight pool"),
                Mesh3d(s.plane.clone()),
                MeshMaterial3d(pool_mat.clone()),
                Transform::from_scale(Vec3::ZERO),
                Visibility::Hidden,
                NotShadowCaster,
                NotShadowReceiver,
                LampPool(e),
            ))
            .id();
        commands.entity(e).insert(LampLook {
            beam,
            pool,
            beam_mat,
            pool_mat,
            lens,
            rim,
        });
    }
}

/// A lamp's pool (its own entity, on the ground), and whose: despawned
/// with it.
#[derive(Component)]
struct LampPool(Entity);

/// Past this a lamp's lens and rim hold the size they look here: which
/// way it looks has to read at 50-100 m, by day.
pub const HEAD_HOLDS_AT: f32 = 15.0;

/// A lamp's lens or rim (held at its size past [`HEAD_HOLDS_AT`]), and its
/// place on the lamp.
#[derive(Component)]
struct HeldHead(Vec3);

fn hold_heads(
    mut heads: Query<(&mut Transform, &GlobalTransform, &HeldHead)>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
) {
    let Some(eye) = cameras.iter().next().map(|g| g.translation()) else {
        return;
    };
    for (mut t, gt, h) in &mut heads {
        let k = (eye.distance(gt.translation()) / HEAD_HOLDS_AT).max(1.0);
        if (t.scale.x - k).abs() > 0.01 * k {
            t.scale = Vec3::splat(k);
            // (Out in front of the drum as it grows.)
            t.translation = h.0 * k;
        }
    }
}

/// A pool whose lamp has gone (its machine despawned) goes too.
fn drop_pools(
    mut commands: Commands,
    pools: Query<(Entity, &LampPool)>,
    lamps: Query<(), With<Beam>>,
) {
    for (e, p) in &pools {
        if !lamps.contains(p.0) {
            commands.entity(e).despawn();
        }
    }
}

fn beam_transform(half_angle: f32, length: f32) -> Transform {
    let r = half_angle.tan() * length;
    Transform::from_scale(Vec3::new(r, r, length))
}

/// The pool where a beam `dir` (unit) lands at `hit`, `half_angle` wide,
/// `length` from the lamp: flat on the ground, stretched along the way it
/// comes in (an oblique beam lands long).
pub fn pool_transform(hit: Vec3, dir: Vec3, half_angle: f32, length: f32) -> Transform {
    let r = half_angle.tan() * length;
    let flat = Vec3::new(dir.x, 0.0, dir.z);
    let yaw = if flat.length_squared() > 1e-6 {
        (-flat.x).atan2(-flat.z)
    } else {
        0.0
    };
    let long = (1.0 / dir.y.abs().max(0.35)).min(2.8);
    Transform::from_translation(hit + Vec3::Y * 0.08)
        .with_rotation(Quat::from_rotation_y(yaw))
        .with_scale(Vec3::new(r, 1.0, r * long))
}

/// Each frame: the beam to where it lands, the pool there, on or off,
/// pale or on you, by the hour and the air.
#[allow(clippy::type_complexity)]
fn light_lamps(
    lamps: Query<(&Beam, &LampLook, &GlobalTransform)>,
    mut places: Query<(&mut Transform, &mut Visibility), Without<Beam>>,
    mut searches: ResMut<Assets<SearchMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    weather: Option<Res<crate::world::sky::Weather>>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
) {
    let day = weather
        .as_ref()
        .map_or(1.0, |w| 1.0 - w.night.clamp(0.0, 1.0));
    // More haze in fog and rain, a little on a clear day.
    let air = conditions.as_ref().map_or(0.6, |c| {
        0.45 + 0.55 * crate::world::beams::air(c).0.min(1.0)
    });
    for (l, look, gt) in &lamps {
        let on = l.on;
        let (from, dir) = super::searchlight::aim(gt);
        let set = l.target.is_some();
        let light = if set { ON_YOU } else { PALE };
        let strength = if on { 1.0 } else { 0.0 };
        // The beam: its length and its params.
        if let Ok((mut t, mut v)) = places.get_mut(look.beam) {
            let want = beam_transform(l.half_angle, l.landed);
            if (t.scale - want.scale).length() > 0.02 {
                *t = want;
            }
            let vis = if on {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != vis {
                *v = vis;
            }
        }
        // The pool: only where the beam lands on something.
        let landed = on && l.landed < l.reach - 0.5;
        if let Ok((mut t, mut v)) = places.get_mut(look.pool) {
            let vis = if landed {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *v != vis {
                *v = vis;
            }
            if landed {
                let want = pool_transform(from + dir * l.landed, dir, l.half_angle, l.landed);
                if t.translation.distance(want.translation) > 0.03
                    || (t.scale - want.scale).length() > 0.02
                    || t.rotation.angle_between(want.rotation) > 0.003
                {
                    *t = want;
                }
            }
        }
        let rim = if set { 1.0 } else { 0.7 };
        for (mat, pool) in [(&look.beam_mat, false), (&look.pool_mat, true)] {
            let want = SearchParams {
                light: light.extend(strength),
                rim: RIM.extend(if pool { rim } else { 0.0 }),
                shape: Vec4::new(if pool { 1.0 } else { 0.0 }, l.landed, day, air),
                axis: dir.extend(0.0),
                origin: from.extend(l.half_angle.tan()),
            };
            let Some(m) = searches.get(mat) else {
                continue;
            };
            let p = m.params;
            let changed = (p.light - want.light).length() > 0.01
                || (p.rim - want.rim).length() > 0.01
                || (p.shape - want.shape).length() > 0.02
                || (p.axis - want.axis).length() > 0.003
                || (p.origin - want.origin).length() > 0.02;
            if changed {
                if let Some(mut m) = searches.get_mut(mat) {
                    m.params = want;
                }
            }
        }
        // The lens and rim: dark when it's out.
        let (lens_k, rim_k) = if on {
            (30.0, if set { 22.0 } else { 14.0 })
        } else {
            (0.0, 0.0)
        };
        for (h, want) in [
            (
                &look.lens,
                LinearRgba::rgb(lens_k, lens_k * 1.1, lens_k * 1.2),
            ),
            (
                &look.rim,
                LinearRgba::rgb(RIM.x * rim_k, RIM.y * rim_k, RIM.z * rim_k),
            ),
        ] {
            if materials.get(h).is_some_and(|m| m.emissive != want) {
                if let Some(mut m) = materials.get_mut(h) {
                    m.emissive = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A beam coming in steep lands round; one coming in low lands long,
    /// along the way it comes, and never longer than a few times wide.
    #[test]
    fn the_pool_lands_long_on_a_low_beam() {
        let half = 0.13;
        let steep = pool_transform(Vec3::ZERO, Vec3::NEG_Y, half, 40.0);
        assert!((steep.scale.x - steep.scale.z).abs() < 1e-3);
        let low = pool_transform(
            Vec3::ZERO,
            Vec3::new(0.0, -0.4, -0.9).normalize(),
            half,
            40.0,
        );
        assert!(low.scale.z > low.scale.x * 2.0, "{:?}", low.scale);
        assert!(low.scale.z <= low.scale.x * 2.81);
        // About 5 m across at 40 m: big enough to read, small enough to
        // step out of.
        assert!((4.0..7.0).contains(&steep.scale.x), "{}", steep.scale.x);
    }
}
