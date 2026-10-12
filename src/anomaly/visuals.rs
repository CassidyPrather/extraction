//! How anomalies look: a lift is a slow column of circling leaves and grit
//! over a flattened ring; a burner is a charred patch shedding embers under
//! an orange glow you can see through the fog; a snare is a knot of blue
//! arcs that lashes out when something touches it. Plus the glass eye's
//! x-ray, which shows robots through walls while you hold it to your eye.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::VisibilityRange;
use bevy::light::NotShadowCaster;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef;
use extraction_core::anomaly::AnomalyKind;

use super::artifacts::ArtifactEffects;
use super::{Anomaly, AnomalyAct, AnomalyEvent};
use crate::rig::{LocalPlayer, RigHead};
use crate::robots::RobotPart;

pub struct AnomalyVisualsPlugin;

impl Plugin for AnomalyVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<XrayMaterial>::default())
            .add_systems(Startup, init_assets)
            .add_systems(
                Update,
                (
                    dress_anomalies,
                    animate_motes,
                    snare_arcs,
                    react_to_events,
                    xray_parts,
                    glass_eye,
                ),
            );
    }
}

/// A drifting particle belonging to an anomaly; its motion is a pure
/// function of time and its seed.
#[derive(Component)]
struct Mote {
    kind: AnomalyKind,
    seed: f32,
    radius: f32,
}

#[derive(Component)]
struct Glow {
    base: f32,
    flash: f32,
}

#[derive(Component)]
struct ArcSegment {
    arc: usize,
    index: usize,
}

#[derive(Component, Default)]
struct Arcs {
    timer: f32,
    flash: f32,
}

const ARC_SEGMENTS: usize = 6;
const ARCS: usize = 4;

#[derive(Resource)]
struct AnomalyAssets {
    quad: Handle<Mesh>,
    leaf: Handle<StandardMaterial>,
    grit: Handle<StandardMaterial>,
    ember: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
    segment: Handle<Mesh>,
    disc: Handle<Mesh>,
    scorch: Handle<StandardMaterial>,
    flattened: Handle<StandardMaterial>,
    glassy: Handle<StandardMaterial>,
}

/// A soft round alpha falloff, for ground marks.
fn radial_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 128u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            // Ragged edge: a few lobes.
            let a = v.atan2(u);
            let edge = 0.82 + 0.08 * (a * 5.0).sin() + 0.05 * (a * 11.0 + 1.3).sin();
            let alpha = (1.0 - ((r - edge * 0.55) / (edge * 0.45)).clamp(0.0, 1.0)).powf(1.6);
            data.extend_from_slice(&[255, 255, 255, (alpha * 255.0) as u8]);
        }
    }
    images.add(Image::new(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

fn init_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    server: Res<AssetServer>,
) {
    let a = assets(&mut meshes, &mut materials, &mut images, &server);
    commands.insert_resource(a);
}

fn assets(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    server: &AssetServer,
) -> AnomalyAssets {
    let leaf_tex: Handle<Image> = server
        .load_builder()
        .with_settings(|s: &mut bevy::image::ImageLoaderSettings| s.is_srgb = true)
        .load("textures/foliage/birch_leaves/albedo.ktx2");
    let radial = radial_texture(images);
    let ground = |c: Color, emissive: LinearRgba, materials: &mut Assets<StandardMaterial>| {
        materials.add(StandardMaterial {
            base_color: c,
            base_color_texture: Some(radial.clone()),
            emissive,
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.95,
            depth_bias: 50.0,
            ..default()
        })
    };
    let a = AnomalyAssets {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        leaf: materials.add(StandardMaterial {
            base_color: Color::srgb(0.75, 0.62, 0.38),
            base_color_texture: Some(leaf_tex),
            alpha_mode: AlphaMode::Mask(0.5),
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.9,
            ..default()
        }),
        grit: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.18, 0.15),
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        ember: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.5, 0.15),
            emissive: LinearRgba::rgb(40.0, 9.0, 1.5),
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        spark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.8, 1.0),
            emissive: LinearRgba::rgb(6.0, 14.0, 40.0),
            unlit: true,
            ..default()
        }),
        segment: meshes.add(Cuboid::new(0.012, 0.012, 1.0)),
        disc: meshes.add(
            Rectangle::new(2.0, 2.0)
                .mesh()
                .build()
                .rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        ),
        scorch: ground(
            Color::srgba(0.02, 0.018, 0.015, 0.92),
            LinearRgba::rgb(0.25, 0.05, 0.0),
            materials,
        ),
        flattened: ground(
            Color::srgba(0.08, 0.07, 0.05, 0.6),
            LinearRgba::BLACK,
            materials,
        ),
        glassy: ground(
            Color::srgba(0.22, 0.26, 0.32, 0.55),
            LinearRgba::rgb(0.0, 0.02, 0.06),
            materials,
        ),
    };
    a
}

fn dress_anomalies(
    mut commands: Commands,
    added: Query<(Entity, &Anomaly), Added<Anomaly>>,
    a: Res<AnomalyAssets>,
) {
    let near = VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: 70.0..90.0,
        use_aabb: false,
    };
    for (e, anomaly) in &added {
        let r = anomaly.radius;
        let mut rng = extraction_core::noise::Rng::new(e.to_bits());
        let (mark, motes, mote_mat, mote_size) = match anomaly.kind {
            AnomalyKind::Lift => (&a.flattened, 64, None, 0.2),
            AnomalyKind::Burner => (&a.scorch, 36, Some(&a.ember), 0.035),
            AnomalyKind::Snare => (&a.glassy, 14, Some(&a.spark), 0.025),
        };
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(a.disc.clone()),
                MeshMaterial3d(mark.clone()),
                Transform::from_xyz(0.0, 0.04, 0.0).with_scale(Vec3::new(r * 1.1, 1.0, r * 1.1)),
                NotShadowCaster,
                near.clone(),
            ));
            for i in 0..motes {
                let mat = match (anomaly.kind, mote_mat) {
                    (_, Some(m)) => m.clone(),
                    (AnomalyKind::Lift, None) if i % 3 == 0 => a.grit.clone(),
                    _ => a.leaf.clone(),
                };
                let grit = anomaly.kind == AnomalyKind::Lift && i % 3 == 0;
                let size = mote_size * rng.range(0.6, 1.4) * if grit { 0.2 } else { 1.0 };
                c.spawn((
                    Mote {
                        kind: anomaly.kind,
                        seed: rng.f32() * 100.0,
                        radius: r,
                    },
                    Mesh3d(a.quad.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_scale(Vec3::splat(size)),
                    NotShadowCaster,
                    near.clone(),
                ));
            }
            match anomaly.kind {
                AnomalyKind::Burner => {
                    c.spawn((
                        Glow {
                            base: 60_000.0,
                            flash: 0.0,
                        },
                        PointLight {
                            color: Color::srgb(1.0, 0.45, 0.15),
                            intensity: 60_000.0,
                            range: r * 4.0 + 4.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.5, 0.0),
                    ));
                }
                AnomalyKind::Snare => {
                    c.spawn((
                        Glow {
                            base: 4_000.0,
                            flash: 0.0,
                        },
                        PointLight {
                            color: Color::srgb(0.45, 0.65, 1.0),
                            intensity: 4_000.0,
                            range: r * 3.0 + 3.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, 0.8, 0.0),
                    ));
                    c.spawn((
                        Arcs::default(),
                        Transform::from_xyz(0.0, 0.8, 0.0),
                        Visibility::default(),
                    ))
                    .with_children(|arcs| {
                        for arc in 0..ARCS {
                            for index in 0..ARC_SEGMENTS {
                                arcs.spawn((
                                    ArcSegment { arc, index },
                                    Mesh3d(a.segment.clone()),
                                    MeshMaterial3d(a.spark.clone()),
                                    Transform::default(),
                                    NotShadowCaster,
                                    near.clone(),
                                ));
                            }
                        }
                    });
                }
                AnomalyKind::Lift => {}
            }
        });
    }
}

fn eye_position(
    heads: &Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: &Query<(), With<LocalPlayer>>,
    cams: &Query<&GlobalTransform, With<Camera3d>>,
) -> Option<Vec3> {
    cams.iter().next().map(|g| g.translation()).or_else(|| {
        heads
            .iter()
            .find(|(_, p)| locals.contains(p.parent()))
            .map(|(g, _)| g.translation())
    })
}

fn animate_motes(
    mut motes: Query<(&Mote, &mut Transform, &ChildOf)>,
    anomalies: Query<&GlobalTransform, With<Anomaly>>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    let eye = eye_position(&heads, &locals, &cams);
    for (m, mut tf, parent) in &mut motes {
        let Ok(origin) = anomalies.get(parent.parent()) else {
            continue;
        };
        let o = origin.translation();
        if eye.is_some_and(|e| e.distance(o) > 95.0) {
            continue;
        }
        let s = m.seed;
        let fr = |k: f32| (s * k).fract().abs();
        let local = match m.kind {
            AnomalyKind::Lift => {
                let speed = 0.5 + fr(1.7) * 0.7;
                let a = s * 6.283 + t * speed;
                let r = m.radius * (0.15 + 0.75 * fr(7.1));
                let h =
                    0.2 + 2.6 * (0.5 + 0.5 * (t * 0.35 + s * 11.0).sin()) * (0.4 + 0.6 * fr(3.3));
                Vec3::new(a.cos() * r, h, a.sin() * r)
            }
            AnomalyKind::Burner => {
                let life = (t * (0.3 + 0.25 * fr(2.9)) + fr(5.3)).fract();
                let a = s * 6.283;
                let r = m.radius * 0.8 * fr(4.7).sqrt();
                let wob =
                    Vec3::new((t * 2.0 + s).sin(), 0.0, (t * 1.7 + s * 2.0).cos()) * 0.12 * life;
                Vec3::new(a.cos() * r, life * 3.2, a.sin() * r) + wob
            }
            AnomalyKind::Snare => {
                // Sparks blink in and out around the core.
                let phase = (t * (3.0 + 4.0 * fr(1.3)) + s).fract();
                let a = s * 6.283 + t * 0.4;
                let b = fr(8.9) * 3.14 - 1.57;
                let r = m.radius * 0.35 * (0.6 + 0.4 * phase);
                Vec3::new(
                    a.cos() * b.cos() * r,
                    0.8 + b.sin() * r,
                    a.sin() * b.cos() * r,
                )
            }
        };
        tf.translation = local;
        let world = o + local;
        let size = tf.scale.x;
        tf.rotation = match (m.kind, eye) {
            // Leaves tumble; everything else faces the eye.
            (AnomalyKind::Lift, _) => {
                Quat::from_euler(EulerRot::XYZ, t * 1.3 + s, t * 0.7 + s * 2.0, s)
            }
            (_, Some(e)) => {
                Transform::from_translation(world)
                    .looking_at(e, Vec3::Y)
                    .rotation
            }
            _ => tf.rotation,
        };
        tf.scale = Vec3::splat(size);
    }
}

/// Arcs crawl about the snare's core, re-striking every few frames; when
/// it discharges they reach out to its full radius.
fn snare_arcs(
    mut arcs: Query<(&mut Arcs, &Children, &ChildOf)>,
    mut segments: Query<(&ArcSegment, &mut Transform, &mut Visibility)>,
    anomalies: Query<&Anomaly>,
    time: Res<Time>,
    mut seed: Local<u64>,
) {
    let dt = time.delta_secs();
    for (mut a, children, parent) in &mut arcs {
        let Ok(anomaly) = anomalies.get(parent.parent()) else {
            continue;
        };
        a.flash = (a.flash - dt * 2.5).max(0.0);
        a.timer -= dt;
        if a.timer > 0.0 {
            continue;
        }
        a.timer = 0.06;
        *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut rng = extraction_core::noise::Rng::new(*seed);
        let reach =
            anomaly.radius * (0.3 + 0.7 * a.flash) * (0.4 + 0.6 * anomaly.charge.max(a.flash));
        // One jagged polyline per arc, core to a point on the sphere.
        let mut paths = [[Vec3::ZERO; ARC_SEGMENTS + 1]; ARCS];
        for path in &mut paths {
            let dir = Vec3::new(
                rng.range(-1.0, 1.0),
                rng.range(-0.6, 1.0),
                rng.range(-1.0, 1.0),
            )
            .normalize_or(Vec3::Y);
            let end = dir * reach;
            for (k, p) in path.iter_mut().enumerate() {
                let f = k as f32 / ARC_SEGMENTS as f32;
                let jag = if k == 0 || k == ARC_SEGMENTS {
                    Vec3::ZERO
                } else {
                    Vec3::new(
                        rng.range(-1.0, 1.0),
                        rng.range(-1.0, 1.0),
                        rng.range(-1.0, 1.0),
                    ) * reach
                        * 0.12
                };
                *p = end * f + jag;
            }
        }
        let live = rng.chance(0.25 + 0.75 * anomaly.charge.max(a.flash));
        for c in children.iter() {
            if let Ok((seg, mut tf, mut vis)) = segments.get_mut(c) {
                let p0 = paths[seg.arc][seg.index];
                let p1 = paths[seg.arc][seg.index + 1];
                let len = p0.distance(p1).max(1e-3);
                *tf = Transform::from_translation((p0 + p1) * 0.5)
                    .looking_at(p1, Vec3::Y)
                    .with_scale(Vec3::new(1.0 + a.flash * 2.0, 1.0 + a.flash * 2.0, len));
                *vis = if live {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

fn react_to_events(
    mut events: MessageReader<AnomalyEvent>,
    children: Query<&Children>,
    mut glows: Query<(&mut Glow, &mut PointLight)>,
    mut arcs: Query<&mut Arcs>,
    time: Res<Time>,
) {
    for ev in events.read() {
        let Ok(kids) = children.get(ev.anomaly) else {
            continue;
        };
        for c in kids.iter() {
            if let Ok((mut g, _)) = glows.get_mut(c) {
                g.flash = match ev.what {
                    AnomalyAct::Discharge => 25.0,
                    AnomalyAct::Flare => 3.0,
                    _ => 1.0,
                };
            }
            if let Ok(mut a) = arcs.get_mut(c) {
                if ev.what == AnomalyAct::Discharge {
                    a.flash = 1.0;
                    a.timer = 0.0;
                }
            }
        }
    }
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    for (mut g, mut light) in &mut glows {
        g.flash = (g.flash - dt * g.flash.max(1.0) * 3.0).max(0.0);
        let flicker = 0.85 + 0.1 * (t * 13.0).sin() * (t * 7.3).sin() + 0.05 * (t * 31.0).sin();
        light.intensity = g.base * flicker * (1.0 + g.flash);
    }
}

// --- The glass eye ---------------------------------------------------------

/// Unlit, additive, drawn on top of everything: robots seen through walls.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct XrayMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for XrayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/xray.wgsl".into()
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
        if let Some(ds) = descriptor.depth_stencil.as_mut() {
            ds.depth_compare = Some(CompareFunction::Always);
            ds.depth_write_enabled = Some(false);
        }
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
struct XrayGhost;

/// Every robot part gets a hidden x-ray ghost the size of its collider.
fn xray_parts(
    mut commands: Commands,
    parts: Query<(Entity, &avian3d::prelude::Collider), Added<RobotPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<XrayMaterial>>,
    mut mat: Local<Option<Handle<XrayMaterial>>>,
) {
    for (e, collider) in &parts {
        let m = mat
            .get_or_insert_with(|| {
                materials.add(XrayMaterial {
                    color: LinearRgba::rgb(0.9, 0.25, 0.2),
                })
            })
            .clone();
        let aabb = avian3d::prelude::SimpleCollider::aabb(collider, Vec3::ZERO, Quat::IDENTITY);
        let size = (aabb.max - aabb.min).max(Vec3::splat(0.05));
        commands.entity(e).with_child((
            XrayGhost,
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(m),
            Transform::from_translation((aabb.max + aabb.min) * 0.5),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }
}

/// Holding the glass eye up to your face shows the ghosts.
fn glass_eye(
    players: Query<&ArtifactEffects, With<LocalPlayer>>,
    eyes: Query<&GlobalTransform>,
    heads: Query<(&GlobalTransform, &ChildOf), With<RigHead>>,
    locals: Query<(), With<LocalPlayer>>,
    cams: Query<&GlobalTransform, With<Camera3d>>,
    mut ghosts: Query<&mut Visibility, With<XrayGhost>>,
    mut was_on: Local<bool>,
) {
    let head = eye_position(&heads, &locals, &cams);
    let on = players
        .iter()
        .next()
        .and_then(|fx| fx.glass_eye)
        .is_some_and(|eye| {
            let (Ok(eg), Some(h)) = (eyes.get(eye), head) else {
                return false;
            };
            eg.translation().distance(h) < 0.22
        });
    if on == *was_on {
        return;
    }
    *was_on = on;
    for mut v in &mut ghosts {
        *v = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}
