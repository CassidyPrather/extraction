//! A soft puff: the one way a puff of smoke, dust, steam or powder is
//! drawn. A quad turned to face the eye, with a soft round falloff (no rim,
//! a little cloud in it), tinted per use; its owner moves, grows and fades
//! it as before. (Puffs were alpha-blended spheres, which read at any
//! distance as flat grey discs with a hard edge: above a hurt walker, a
//! bug, not smoke.)
//!
//! It faces the eye's position, not its view plane, and in VR the middle
//! of the two eyes: one quad, the same to both eyes, so it doesn't swim
//! between them.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::transform::TransformSystems;

/// The texture's side, pixels.
const SIDE: u32 = 64;

/// The quad's side, m, at scale 1 (see [`PuffAssets`]).
pub const QUAD: f32 = 1.6;

/// The most of the view one puff may span from its middle, radians: a
/// puff near the eye looks smaller, not thicker. Without it, standing in
/// a hazard's cloud put 64 quads each across most of the view, 39 layers
/// of blended fragments through both eyes (`render::vr_budget`'s puff
/// gauge); in VR a wall of grey against the face reads worse besides.
pub const MAX_HALF_ANGLE: f32 = 10.0 * std::f32::consts::PI / 180.0;

/// The largest scale a puff `dist` m from the eye may have
/// ([`MAX_HALF_ANGLE`]).
pub fn max_scale(dist: f32) -> f32 {
    dist.max(0.0) * MAX_HALF_ANGLE.tan() / (QUAD * 0.5)
}

/// Turned to face the eye each frame (`face_puffs`).
#[derive(Component, Default)]
pub struct SoftPuff;

/// The quad and its falloff, shared by every puff. The quad is [`QUAD`]
/// across so that its body (where it's at least half as thick as its
/// heart) is 1 m across, as the unit sphere it replaced was: owners scale
/// it as they scaled that.
#[derive(Resource, Clone)]
pub struct PuffAssets {
    pub quad: Handle<Mesh>,
    pub falloff: Handle<Image>,
}

impl FromWorld for PuffAssets {
    fn from_world(world: &mut World) -> Self {
        let quad = world
            .get_resource_mut::<Assets<Mesh>>()
            .map(|mut meshes| meshes.add(Rectangle::new(QUAD, QUAD)))
            .unwrap_or_default();
        let falloff = world
            .get_resource_mut::<Assets<Image>>()
            .map(|mut images| images.add(falloff_image()))
            .unwrap_or_default();
        PuffAssets { quad, falloff }
    }
}

/// A soft puff thins to nothing at its edge where a sphere stayed whole
/// to it: its heart is this much thicker, so it shows as much.
pub const HEART: f32 = 1.6;

impl PuffAssets {
    /// A puff's material: `tint` (its colour and how thick a sphere of it
    /// was), `lit` by the scene or carrying its own light.
    pub fn material(&self, tint: Color, lit: bool) -> StandardMaterial {
        self.soften(StandardMaterial {
            base_color: tint,
            alpha_mode: AlphaMode::Blend,
            unlit: !lit,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            ..default()
        })
    }

    /// `m` as a soft puff's: the falloff on it, both faces, and its heart
    /// [`HEART`] times as thick (its blend, emission and light kept).
    pub fn soften(&self, m: StandardMaterial) -> StandardMaterial {
        let mut c = m.base_color.to_srgba();
        c.alpha = (c.alpha * HEART).min(1.0);
        StandardMaterial {
            base_color: c.into(),
            base_color_texture: Some(self.falloff.clone()),
            double_sided: true,
            cull_mode: None,
            ..m
        }
    }
}

/// How thick a puff is `r` from its middle (0 at the middle, 1 at the
/// quad's inscribed circle's edge): full at the heart, easing to nothing
/// at the edge with no step, and a little cloud in it so it isn't a
/// perfect disc. `x`, `y` place the cloud.
pub fn thickness(r: f32, x: f32, y: f32) -> f32 {
    if r >= 1.0 {
        return 0.0;
    }
    // (The last twentieth of the way out eases it to nothing: the quad's
    // own edge is clear.)
    let body = (1.0 - r * r).powf(1.5) * ((1.0 - r) / 0.05).min(1.0);
    // Fixed lumps (sines at odd angles), strongest toward the edge.
    let cloud = ((x * 7.3 + y * 2.1).sin() * (y * 6.7 - x * 3.9).sin()
        + 0.5 * (x * 13.1 - y * 11.3).sin())
        * 0.5;
    (body * (1.0 + 0.3 * cloud * r)).clamp(0.0, 1.0)
}

/// The falloff: white, its alpha [`thickness`].
pub fn falloff_image() -> Image {
    let mut data = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for j in 0..SIDE {
        for i in 0..SIDE {
            let x = (i as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let y = (j as f32 + 0.5) / SIDE as f32 * 2.0 - 1.0;
            let a = thickness((x * x + y * y).sqrt(), x, y);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}

/// The cameras [`eye`] reads: the eyes, not a camera drawing into an
/// image (`render::mark_offscreen` marks those).
pub type Eyes<'w, 's> =
    Query<'w, 's, (&'static GlobalTransform, &'static Camera), crate::render::EyeCamera>;

/// Where the eye is: the middle of the active eye cameras (both eyes in
/// VR, the one camera on a screen or in a capture).
pub fn eye(cams: &Eyes) -> Option<Vec3> {
    let (sum, n) = cams
        .iter()
        .filter(|(_, c)| c.is_active)
        .fold((Vec3::ZERO, 0.0), |(s, n), (g, _)| {
            (s + g.translation(), n + 1.0)
        });
    (n > 0.0).then(|| sum / n)
}

/// The rotation that turns a quad at `at` to face `eye`, upright.
pub fn facing(at: Vec3, eye: Vec3) -> Quat {
    let to_eye = eye - at;
    if to_eye.length_squared() < 1e-8 {
        return Quat::IDENTITY;
    }
    // Upright unless looked at from straight above or below.
    let up = if to_eye.normalize().y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    // `looking_to` points -Z along its direction; the quad faces +Z.
    Transform::IDENTITY.looking_to(-to_eye, up).rotation
}

fn face_puffs(
    cams: Eyes,
    mut puffs: Query<(&mut Transform, Option<&ChildOf>), With<SoftPuff>>,
    parents: Query<&GlobalTransform>,
) {
    let Some(eye) = eye(&cams) else {
        return;
    };
    for (mut t, parent) in &mut puffs {
        let parent = parent.and_then(|p| parents.get(p.parent()).ok());
        let (at, turn) = match parent {
            Some(g) => (g.transform_point(t.translation), g.rotation()),
            None => (t.translation, Quat::IDENTITY),
        };
        t.rotation = turn.inverse() * facing(at, eye);
        // Its owner wrote its scale this frame; near the eye, no wider
        // than `MAX_HALF_ANGLE`.
        let most = max_scale(at.distance(eye));
        if t.scale.max_element() > most {
            t.scale = t.scale.min(Vec3::splat(most));
        }
    }
}

fn make_assets(world: &mut World) {
    if !world.contains_resource::<PuffAssets>() {
        let a = PuffAssets::from_world(world);
        world.insert_resource(a);
    }
}

/// The puff's facing, and its assets: each owner adds it (once).
pub struct PuffPlugin;

impl Plugin for PuffPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, make_assets)
            .add_systems(PostUpdate, face_puffs.before(TransformSystems::Propagate));
        // (`EXTRACTION_PUFF_BATCH=0`: each puff its own draw, to compare.)
        if !std::env::var("EXTRACTION_PUFF_BATCH").is_ok_and(|v| v == "0") {
            app.init_resource::<Batches>().add_systems(
                PostUpdate,
                batch_puffs
                    .after(TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            );
        }
    }
}

/// Adds [`PuffPlugin`] unless an earlier owner has.
pub fn add_to(app: &mut App) {
    if !app.is_plugin_added::<PuffPlugin>() {
        app.add_plugins(PuffPlugin);
    }
}

/// What a puff spawns with besides its own state: the quad, its material,
/// and the facing. (Puffs move: no culling against a stale box.)
pub fn bundle(a: &PuffAssets, material: Handle<StandardMaterial>) -> impl Bundle {
    (
        Mesh3d(a.quad.clone()),
        MeshMaterial3d(material),
        SoftPuff,
        NoFrustumCulling,
    )
}

/// A puff drawn in its batch, not as its own mesh (the quad it had).
#[derive(Component)]
pub struct Batched;

/// A batch's mesh entity: what's drawn for every puff in it.
#[derive(Component)]
pub struct PuffBatch;

/// The batches, by what their puffs' materials share (all but their
/// colour): each one mesh, one material, rebuilt every frame.
#[derive(Resource, Default)]
struct Batches {
    by: std::collections::HashMap<u64, Batch>,
}

struct Batch {
    entity: Entity,
    mesh: Handle<Mesh>,
}

/// What a puff's material shares with others its batch may hold: all
/// but its colour (each quad carries that as vertex colours, which the
/// shader multiplies by the material's, as it did the material's own).
fn batch_key(m: &StandardMaterial, receives: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", m.alpha_mode).hash(&mut h);
    m.unlit.hash(&mut h);
    m.double_sided.hash(&mut h);
    format!("{:?}", m.cull_mode).hash(&mut h);
    m.base_color_texture.as_ref().map(|t| t.id()).hash(&mut h);
    m.emissive_texture.as_ref().map(|t| t.id()).hash(&mut h);
    for f in [
        m.emissive.red,
        m.emissive.green,
        m.emissive.blue,
        m.emissive.alpha,
        m.perceptual_roughness,
        m.metallic,
        m.reflectance,
        m.diffuse_transmission,
        m.depth_bias,
    ] {
        f.to_bits().hash(&mut h);
    }
    m.fog_enabled.hash(&mut h);
    receives.hash(&mut h);
    h.finish()
}

/// `m` as its batch's material: white, the puffs' colours carried by
/// their vertices.
fn batch_material(m: &StandardMaterial) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        ..m.clone()
    }
}

/// Every puff drawn in one mesh a batch (its material, all but its
/// colour) instead of a draw each: a firefight's 64 puffs were 64 draws an
/// eye (`render::vr_budget`'s worst views, 5 November). Each quad is the
/// puff as it was, where its owner put it, faced, sized and coloured as
/// before; a batch's quads are drawn far to near from the eye, the order
/// the puffs were sorted in as their own draws.
#[allow(clippy::type_complexity)]
fn batch_puffs(
    mut commands: Commands,
    mut batches: ResMut<Batches>,
    cams: Eyes,
    new: Query<
        Entity,
        (
            With<SoftPuff>,
            With<Mesh3d>,
            Without<Batched>,
            Without<PuffBatch>,
        ),
    >,
    puffs: Query<
        (
            &GlobalTransform,
            &InheritedVisibility,
            &MeshMaterial3d<StandardMaterial>,
            Has<bevy::light::NotShadowReceiver>,
        ),
        With<Batched>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &new {
        commands.entity(e).remove::<Mesh3d>().insert(Batched);
    }
    let eye = eye(&cams).unwrap_or(Vec3::ZERO);
    // This frame's quads, by batch: (distance from the eye, corners,
    // normal, colour).
    let mut quads: std::collections::HashMap<
        u64,
        (
            Handle<StandardMaterial>,
            Vec<(f32, [Vec3; 4], Vec3, [f32; 4])>,
        ),
    > = Default::default();
    for (g, shown, mat, no_shadow) in &puffs {
        if !shown.get() {
            continue;
        }
        let Some(m) = materials.get(&mat.0) else {
            continue;
        };
        let key = batch_key(m, !no_shadow);
        let colour = m.base_color.to_linear().to_f32_array();
        let a = g.affine();
        let h = QUAD * 0.5;
        let corners = [
            Vec2::new(-h, -h),
            Vec2::new(h, -h),
            Vec2::new(h, h),
            Vec2::new(-h, h),
        ]
        .map(|c| a.transform_point3(Vec3::new(c.x, c.y, 0.0)));
        let normal = a.transform_vector3(Vec3::Z).normalize_or(Vec3::Z);
        let at = g.translation();
        quads
            .entry(key)
            .or_insert_with(|| (mat.0.clone(), Vec::new()))
            .1
            .push((at.distance_squared(eye), corners, normal, colour));
    }
    // Batches with nothing this frame draw nothing.
    for (key, b) in batches.by.iter_mut() {
        if !quads.contains_key(key) {
            if let Some(mut m) = meshes.get_mut(&b.mesh) {
                *m = empty_batch();
            }
        }
    }
    for (key, (from, mut qs)) in quads {
        // Far to near.
        qs.sort_by(|a, b| b.0.total_cmp(&a.0));
        // Where its quads are: so it's sorted among other blended things
        // as they were, about.
        let centre = qs.iter().map(|q| (q.1[0] + q.1[2]) * 0.5).sum::<Vec3>() / qs.len() as f32;
        let mut pos = Vec::with_capacity(qs.len() * 4);
        let mut nor = Vec::with_capacity(qs.len() * 4);
        let mut uv = Vec::with_capacity(qs.len() * 4);
        let mut col = Vec::with_capacity(qs.len() * 4);
        let mut idx = Vec::with_capacity(qs.len() * 6);
        for (_, corners, n, c) in &qs {
            let base = pos.len() as u32;
            for (k, p) in corners.iter().enumerate() {
                pos.push((*p - centre).to_array());
                nor.push(n.to_array());
                col.push(*c);
                // As `Rectangle`'s: v down from the top.
                uv.push([[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]][k]);
            }
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mut mesh = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
        mesh.insert_indices(bevy::mesh::Indices::U32(idx));
        match batches.by.get_mut(&key) {
            Some(b) => {
                if let Some(mut m) = meshes.get_mut(&b.mesh) {
                    *m = mesh;
                }
                commands
                    .entity(b.entity)
                    .insert(Transform::from_translation(centre));
                // (Its material is the puffs' but white: what the key
                // holds is all the same, and the colour rides the
                // vertices, so it never needs writing again.)
            }
            None => {
                let src = materials.get(&from).cloned().unwrap_or_default();
                let receives = batch_key(&src, true) == key;
                let material = materials.add(batch_material(&src));
                let handle = meshes.add(mesh);
                let mut ec = commands.spawn((
                    Name::new("Puffs"),
                    PuffBatch,
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(centre),
                    Visibility::default(),
                    NoFrustumCulling,
                    bevy::light::NotShadowCaster,
                ));
                if !receives {
                    ec.insert(bevy::light::NotShadowReceiver);
                }
                batches.by.insert(
                    key,
                    Batch {
                        entity: ec.id(),
                        mesh: handle,
                    },
                );
            }
        }
    }
}

/// A batch with no puffs: a degenerate triangle (a mesh with nothing in
/// it isn't handed to the GPU, `render::empty_meshes`).
fn empty_batch() -> Mesh {
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 3]);
    mesh.insert_indices(bevy::mesh::Indices::U32(vec![0, 1, 2]));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fight's puffs cost a draw a batch (their materials, all but the
    /// colour), not a draw each, and each is drawn as it was: its quad
    /// where its owner put it, faced and sized, its material's colour; a
    /// batch's quads far to near from the eye.
    #[test]
    fn a_fights_puffs_are_a_draw_a_batch_each_drawn_as_it_was() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            TransformPlugin,
            bevy::camera::visibility::VisibilityPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>();
        add_to(&mut app);
        app.update();
        let a = app.world().resource::<PuffAssets>().clone();
        let mut mats = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        // Smoke at two thicknesses (one batch), and dust lit by the scene
        // (another).
        let thick = mats.add(a.material(Color::srgba(0.4, 0.4, 0.4, 0.5), false));
        let thin = mats.add(a.material(Color::srgba(0.4, 0.4, 0.4, 0.2), false));
        let dust = mats.add(a.material(Color::srgba(0.6, 0.5, 0.3, 0.4), true));
        let w = app.world_mut();
        let mut puffs = Vec::new();
        for i in 0..64 {
            let m = [&thick, &thin, &dust][i % 3].clone();
            let at = Vec3::new(
                (i % 8) as f32 * 1.3 - 5.0,
                1.0 + (i / 8) as f32 * 0.2,
                -6.0 - i as f32 * 0.1,
            );
            let e = w
                .spawn((
                    bundle(&a, m.clone()),
                    Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_y(0.3 * i as f32))
                        .with_scale(Vec3::splat(0.5 + 0.01 * i as f32)),
                ))
                .id();
            puffs.push((e, m));
        }
        app.update();
        app.update();
        let w = app.world_mut();
        let drawn: Vec<Entity> = w.query_filtered::<Entity, With<Mesh3d>>().iter(w).collect();
        let batches: Vec<Entity> = w
            .query_filtered::<Entity, With<PuffBatch>>()
            .iter(w)
            .collect();
        assert_eq!(batches.len(), 2, "64 puffs of two kinds: two batches");
        assert_eq!(drawn.len(), 2, "64 puffs drawn in {} meshes", drawn.len());
        // Every puff's quad in a batch, where it is, its colour.
        let mut quads: Vec<([Vec3; 4], [f32; 4])> = Vec::new();
        for b in &batches {
            let at = w.get::<Transform>(*b).unwrap().translation;
            let m = w.get::<Mesh3d>(*b).unwrap().0.clone();
            let mesh = w.resource::<Assets<Mesh>>().get(&m).unwrap();
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("no positions");
            };
            let Some(bevy::mesh::VertexAttributeValues::Float32x4(c)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("no colours");
            };
            for (k, q) in p.chunks(4).enumerate() {
                quads.push(([0, 1, 2, 3].map(|i| Vec3::from(q[i]) + at), c[k * 4]));
            }
            // Far to near from the eye (no eye here: from the origin).
            let d: Vec<f32> = p
                .chunks(4)
                .map(|q| ((Vec3::from(q[0]) + Vec3::from(q[2])) * 0.5 + at).length())
                .collect();
            assert!(
                d.windows(2).all(|x| x[0] >= x[1] - 1e-3),
                "not far to near: {d:?}"
            );
        }
        assert_eq!(quads.len(), 64);
        for (e, m) in &puffs {
            let g = w.get::<GlobalTransform>(*e).unwrap().affine();
            let h = QUAD * 0.5;
            let want = [(-h, -h), (h, -h), (h, h), (-h, h)]
                .map(|(x, y)| g.transform_point3(Vec3::new(x, y, 0.0)));
            let colour = w
                .resource::<Assets<StandardMaterial>>()
                .get(m)
                .unwrap()
                .base_color
                .to_linear()
                .to_f32_array();
            assert!(
                quads.iter().any(|(q, c)| {
                    q.iter().zip(&want).all(|(a, b)| a.distance(*b) < 1e-3) && *c == colour
                }),
                "{e:?}'s quad isn't drawn where and as it is"
            );
            assert!(
                w.get::<Mesh3d>(*e).is_none(),
                "a batched puff still drawn on its own"
            );
        }
    }

    /// The falloff is full at the heart, nothing at and past the edge, and
    /// never steps: no rim. Half as thick as its heart 1 m across at scale
    /// 1, as the sphere it replaced was.
    #[test]
    fn a_puff_is_soft_to_its_edge() {
        assert!(thickness(0.0, 0.0, 0.0) > 0.95);
        let half = (1.0 - 0.5f32.powf(1.0 / 1.5)).sqrt() * QUAD;
        assert!((half - 1.0).abs() < 0.05, "its body is {half} m across");
        assert_eq!(thickness(1.0, 0.7, 0.7), 0.0);
        let img = falloff_image();
        let data = img.data.as_ref().unwrap();
        let a = |i: u32, j: u32| data[((j * SIDE + i) * 4 + 3) as usize] as i32;
        // The quad's border is clear, and across the middle row no pixel
        // jumps by more than an eighth of full from its neighbour.
        for k in 0..SIDE {
            assert_eq!(a(k, 0), 0);
            assert_eq!(a(0, k), 0);
        }
        let mid = SIDE / 2;
        for i in 1..SIDE {
            let d = (a(i, mid) - a(i - 1, mid)).abs();
            assert!(d <= 32, "a step of {d} at {i}");
        }
    }

    /// No puff anywhere is a sphere: every sphere outside tests is one of
    /// these, by file and count, and none is a puff. A new one fails here
    /// until it's said what it is (a puff is a soft puff: `bundle`).
    #[test]
    fn no_puff_is_a_sphere() {
        const SPHERES: &[(&str, usize, &str)] = &[
            (
                "anomaly/visuals.rs",
                9,
                "lens, core glow, sparks, pebbles, drops",
            ),
            (
                "gadgets/visuals.rs",
                3,
                "a lamp, the EMP's flash, the bubble's shell",
            ),
            ("dev.rs", 1, "the metals gallery's roughness ladder"),
            ("render/arms.rs", 2, "elbow and shoulder joints"),
            ("render/bench.rs", 1, "a knob"),
            ("render/body.rs", 1, "a knee"),
            ("render/landmarks.rs", 1, "a lamp"),
            ("render/rv_beacons.rs", 1, "a lamp"),
            ("render/way_lamps.rs", 1, "a yard lamp's bulb"),
            ("render/wounds.rs", 1, "a wound's patch, on the skin"),
            (
                "robots/fx.rs",
                1,
                "a hit's flash, glowing (its smoke and dust are soft)",
            ),
            ("robots/minelayer_look.rs", 1, "a 12 mm lamp"),
            (
                "robots/plough_look.rs",
                1,
                "snow chunks, 5 cm lumps (its powder is soft)",
            ),
            ("robots/recall.rs", 1, "a tally lamp"),
            (
                "robots/sentry_look.rs",
                1,
                "the lens's chirp glow, additive",
            ),
            (
                "robots/visuals.rs",
                12,
                "eyes, shells, envelopes, halos, a flare, a drone's lock glow, a melee machine's lens glow",
            ),
            ("robots/weak_points.rs", 1, "a knee's mark"),
            ("weapons/flares.rs", 1, "a flare's burning ball"),
            (
                "weapons/impacts.rs",
                1,
                "drops of oil and water (its dust and smoke are soft)",
            ),
            ("weapons/legibility.rs", 1, "a 6 mm dot"),
            ("world/buildings.rs", 1, "a door knob"),
            ("world/cage.rs", 1, "a lamp"),
            ("world/halos.rs", 2, "light halos"),
            ("world/lockdown.rs", 1, "a lamp"),
            ("world/rocks.rs", 1, "a rock's base shape"),
            ("world/sky.rs", 1, "the sky dome"),
            ("world/underground.rs", 1, "a lamp"),
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = std::collections::BTreeMap::new();
        let mut dirs = vec![root.clone()];
        while let Some(p) = dirs.pop() {
            if p.is_dir() {
                dirs.extend(std::fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
                continue;
            }
            if p.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p).unwrap();
            // What `#[cfg(test)]` marks is skipped: a line, or a block.
            let (mut marked, mut depth, mut n) = (false, 0i32, 0);
            for line in text.lines() {
                if depth > 0 {
                    depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    continue;
                }
                if line.trim() == "#[cfg(test)]" {
                    marked = true;
                    continue;
                }
                if std::mem::take(&mut marked) {
                    depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
                    continue;
                }
                n += line.matches("Sphere::new(").count();
            }
            if n > 0 {
                found.insert(rel, n);
            }
        }
        let mut bad = Vec::new();
        for (file, n) in &found {
            match SPHERES.iter().find(|(f, ..)| f == file) {
                Some((_, m, _)) if m == n => {}
                Some((_, m, _)) => bad.push(format!("src/{file}: {n} spheres, {m} known")),
                None => bad.push(format!("src/{file}: {n} spheres, none known")),
            }
        }
        for (file, ..) in SPHERES {
            if !found.contains_key(*file) {
                bad.push(format!("src/{file}: no spheres now; take it off"));
            }
        }
        assert!(
            bad.is_empty(),
            "say what each is (a puff is `render::puff`):\n{}",
            bad.join("\n")
        );
    }

    /// A puff near the eye looks smaller, not thicker: clamped, it spans
    /// no more than `MAX_HALF_ANGLE` from its middle, however close; far
    /// off, an owner's scale stands.
    #[test]
    fn a_puff_near_the_eye_looks_smaller_not_thicker() {
        for dist in [0.3, 1.0, 2.0, 3.0] {
            let half = max_scale(dist) * QUAD * 0.5;
            let angle = (half / dist).atan();
            assert!((angle - MAX_HALF_ANGLE).abs() < 1e-4, "{dist} m: {angle}");
        }
        // A hazard puff at its fullest (1.8) is clamped within 8.2 m, and a
        // wound's (0.5) only within 2.3 m.
        assert!(max_scale(7.5) < 1.8 && max_scale(8.5) > 1.8);
        assert!(max_scale(2.0) < 0.5 && max_scale(2.5) > 0.5);
        assert_eq!(max_scale(-1.0), 0.0);
    }

    /// A puff faces the eye from wherever it is, upright, its face (+Z)
    /// toward the eye; the eye is the middle of both eyes.
    #[test]
    fn a_puff_faces_the_eye() {
        let eye = Vec3::new(0.0, 1.7, 0.0);
        for at in [
            Vec3::new(3.0, 1.0, -4.0),
            Vec3::new(-2.0, 2.5, 1.0),
            Vec3::new(0.0, 0.5, 5.0),
        ] {
            let q = facing(at, eye);
            let face = q * Vec3::Z;
            assert!(face.dot((eye - at).normalize()) > 0.999, "{at}");
            assert!((q * Vec3::X).y.abs() < 1e-4, "it rolled at {at}");
        }
    }
}
