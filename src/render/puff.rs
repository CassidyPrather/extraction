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
pub const MAX_HALF_ANGLE: f32 = 12.0 * std::f32::consts::PI / 180.0;

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

#[cfg(test)]
mod tests {
    use super::*;

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
                "robots/visuals.rs",
                10,
                "eyes, shells, envelopes, halos, a flare",
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
        // A hazard puff at its fullest (1.8) is clamped within 6.8 m, and a
        // wound's (0.5) only within 1.9 m.
        assert!(max_scale(6.5) < 1.8 && max_scale(7.0) > 1.8);
        assert!(max_scale(1.8) < 0.5 && max_scale(2.0) > 0.5);
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
