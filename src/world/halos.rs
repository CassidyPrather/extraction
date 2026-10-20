//! Halos: the glow round a lamp in the dark, the lights you navigate by at
//! night (the Depot's lamps, the camp's lantern, the truck's work lamp).
//! A bright point in damp air spreads into a soft halo, wider in fog; in
//! a headset that halation is what makes a lamp read as a light and not
//! a dot, and it costs one small sphere per lamp, not a bloom pass per
//! eye (`assets/shaders/halo.wgsl`).
//!
//! Only lamps that stay get one: a light seen on two scans a second apart.
//! A muzzle flash or a blast is gone by then.

use bevy::camera::visibility::VisibilityRange;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use std::collections::{HashMap, HashSet};

use super::weather::Conditions;

pub struct HalosPlugin;

impl Plugin for HalosPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<HaloMaterial>::default())
            .add_systems(Update, (add_halos, size_halos, halo_air).chain());
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct HaloParams {
    /// rgb: the lamp's colour; w: its core's radiance.
    pub color: Vec4,
    /// x: how much the air spreads it (`beams::air`); y: how far you see.
    pub air: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct HaloMaterial {
    #[uniform(0)]
    pub params: HaloParams,
}

impl Material for HaloMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/halo.wgsl".into()
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
        // The far side only: the halo still draws with the eye inside it.
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// A lamp's halo (a child of the light).
#[derive(Component)]
struct Halo;

/// The core's radiance (cd/m2-ish, before the eye's exposure).
const CORE: f32 = 300.0;
/// How far off a halo is drawn (m): past that a lamp is a point the fog
/// has, and every halo is a draw.
const REACH: f32 = 140.0;

/// Whether halos show: only once the light's going (by day a lamp's
/// halo is lost in the daylight, and every one is a draw).
pub fn halos_show(c: &Conditions) -> bool {
    c.light < 0.5
}

/// How big a lamp's halo is for its lumens (m): the lantern's 9 klm
/// about a third of a metre, the Depot's lamps more.
pub fn halo_radius(lumens: f32) -> f32 {
    if lumens <= 0.0 {
        return 0.0;
    }
    (0.35 * (lumens / 9_000.0).sqrt()).clamp(0.12, 1.0)
}

#[derive(Default)]
struct Scan {
    timer: f32,
    seen: HashSet<Entity>,
    mesh: Option<Handle<Mesh>>,
    materials: HashMap<[u8; 3], Handle<HaloMaterial>>,
}

fn add_halos(
    mut commands: Commands,
    lights: Query<(Entity, &PointLight, Option<&Children>)>,
    halos: Query<(), With<Halo>>,
    conditions: Res<Conditions>,
    weather: Res<super::sky::Weather>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HaloMaterial>>,
    mut scan: Local<Scan>,
) {
    if std::env::var_os("EXTRACTION_NO_HALOS").is_some() {
        return;
    }
    scan.timer -= time.delta_secs();
    if scan.timer > 0.0 {
        return;
    }
    scan.timer = 1.0;
    let mut now = HashSet::new();
    for (e, light, children) in &lights {
        let has = children.is_some_and(|c| c.iter().any(|c| halos.contains(c)));
        if has {
            continue;
        }
        now.insert(e);
        if !scan.seen.contains(&e) {
            continue;
        }
        let mesh = scan
            .mesh
            .get_or_insert_with(|| meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()))
            .clone();
        let c = light.color.to_srgba();
        let key = [
            (c.red * 15.0) as u8,
            (c.green * 15.0) as u8,
            (c.blue * 15.0) as u8,
        ];
        let air = super::beams::air(&conditions).0;
        let material = scan
            .materials
            .entry(key)
            .or_insert_with(|| {
                let l = light.color.to_linear();
                materials.add(HaloMaterial {
                    params: HaloParams {
                        color: Vec4::new(l.red, l.green, l.blue, CORE),
                        air: Vec4::new(air, weather.visibility, 0.0, 0.0),
                    },
                })
            })
            .clone();
        commands.entity(e).with_child((
            Name::new("Lamp halo"),
            Halo,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_scale(Vec3::splat(halo_radius(light.intensity))),
            if halos_show(&conditions) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: REACH..REACH + 10.0,
                use_aabb: false,
            },
            NotShadowCaster,
            NotShadowReceiver,
        ));
    }
    scan.seen = now;
}

/// A lamp that brightens or dims takes its halo with it (a pylon's work
/// light, a flare); off, the halo's gone.
fn size_halos(
    lights: Query<(&PointLight, &Children), Changed<PointLight>>,
    mut halos: Query<&mut Transform, With<Halo>>,
) {
    for (light, children) in &lights {
        for c in children.iter() {
            if let Ok(mut xf) = halos.get_mut(c) {
                let r = halo_radius(light.intensity);
                if (xf.scale.x - r).abs() > 1e-3 {
                    xf.scale = Vec3::splat(r);
                }
            }
        }
    }
}

fn halo_air(
    conditions: Res<Conditions>,
    weather: Res<super::sky::Weather>,
    mut materials: ResMut<Assets<HaloMaterial>>,
    mut halos: Query<&mut Visibility, With<Halo>>,
) {
    if !conditions.is_changed() {
        return;
    }
    let show = if halos_show(&conditions) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut halos {
        v.set_if_neq(show);
    }
    let air = super::beams::air(&conditions).0;
    for (_, m) in materials.iter_mut() {
        m.params.air = Vec4::new(air, weather.visibility, 0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brighter_lamps_have_wider_halos() {
        assert_eq!(halo_radius(0.0), 0.0);
        let lantern = halo_radius(9_000.0);
        let lamp = halo_radius(40_000.0);
        assert!(lantern > 0.3 && lantern < 0.4);
        assert!(lamp > lantern && lamp <= 1.0);
    }

    #[test]
    fn halos_are_for_the_dark() {
        use super::super::weather::{Hour, WeatherKind};
        let at = |h| halos_show(&Conditions::compose(WeatherKind::Drizzle, h, 0));
        assert!(at(Hour::Night) && at(Hour::Dusk));
        assert!(!at(Hour::Day));
    }
}
