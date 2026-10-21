//! Looking through a scope as a scope looks: an eye box, parallax, and a
//! glint robots can see.
//!
//! - **Eye box** ([`eye_box`]): a scope shows its picture only to an eye
//!   at the right distance behind the eyepiece (its eye relief) and on its
//!   axis. Drift off-axis and a black edge closes in from the side you
//!   moved away from; too near or too far, the picture shrinks to a
//!   disc and blacks out. The [`EyeBox`] on each optic says how much of
//!   the picture the nearest eye gets and how it's shifted; the lens draws
//!   it (`vignette_lenses`, client side).
//! - **Parallax**: off-axis, the picture slides against the reticle
//!   (`EyeBox::shift`): the reticle on a target only means the round goes
//!   there with your eye on the axis.
//! - **Glint** ([`Glint`]): at dawn and dusk, a scope pointed toward the
//!   low sun flashes its objective back the way it points. A robot it
//!   points at, out to [`GLINT_REACH`], sees the flash
//!   ([`glint_seen`], read by the robots' senses): don't glass a valley
//!   with the sun in your face.
//! - **Iron sights** line up at the cheek: a gun with no scope brought up
//!   so its rear sight is just in front of your eye settles so the eye,
//!   the rear sight and the front post are in line (`feel`'s
//!   `sight_fix`). Your eye focuses on the target, so the front post goes
//!   soft as the gun comes up ([`post_blur`]; the comfort setting
//!   `post_blur` turns it off).

use bevy::prelude::*;

use crate::gadgets::{MountedOn, Optic};

/// The eye's best distance behind the eyepiece, metres (a PSO's is 68 mm).
pub const RELIEF: f32 = 0.07;
/// Within this of the axis the picture is whole; by this, black.
pub const AXIS_FULL: f32 = 0.004;
pub const AXIS_BLACK: f32 = 0.014;
/// Within this of the eye relief the picture is whole; by this, black.
pub const RELIEF_FULL: f32 = 0.025;
pub const RELIEF_BLACK: f32 = 0.065;
/// How far the picture slides against the reticle per metre off-axis,
/// radians.
pub const PARALLAX: f32 = 0.35;

/// What an eye gets through an optic: how much of the picture (0 none, 1
/// all) and how it slides against the reticle (radians, the optic's x and
/// y), and where the black edge's centre sits (in lens radii).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct EyeBox {
    pub picture: f32,
    pub shift: Vec2,
    pub hole: Vec2,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The picture an eye at `eye` gets through an optic whose eyepiece is at
/// `eyepiece`, facing +Z of `rot` (toward the eye).
pub fn eye_box(eyepiece: Vec3, rot: Quat, eye: Vec3) -> EyeBox {
    let local = rot.inverse() * (eye - eyepiece);
    let off = local.truncate();
    let lateral = off.length();
    let relief = (local.z - RELIEF).abs();
    let axis = 1.0 - smoothstep(AXIS_FULL, AXIS_BLACK, lateral);
    let depth = if local.z <= 0.0 {
        0.0
    } else {
        1.0 - smoothstep(RELIEF_FULL, RELIEF_BLACK, relief)
    };
    EyeBox {
        picture: axis * depth,
        shift: -off * PARALLAX,
        hole: -off / AXIS_BLACK,
    }
}

/// An optic's objective flashing back the low sun along its axis (0..1).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Glint {
    pub strength: f32,
    pub at: Vec3,
    /// Which way the objective faces (the flash goes back along it).
    pub toward: Vec3,
}

/// The sun's height under which it glints off a lens, degrees.
pub const GLINT_SUN: f32 = 30.0;
/// The objective must face within this of the sun to catch it, radians.
pub const GLINT_FACING: f32 = 1.05;
/// A robot within this of the objective's axis (radians) and this range
/// (metres) sees the flash...
pub const GLINT_CONE: f32 = 0.26;
pub const GLINT_REACH: f32 = 400.0;
/// ...as this much exposure at full strength, near.
pub const GLINT_GLARE: f32 = 0.5;

/// Where the low sun is (a unit vector toward it), at dawn and dusk; none
/// by day (high behind the cloud) or at night. Bearings as the sky draws
/// them (`world::sky`).
pub fn low_sun(hour: crate::world::weather::Hour, elevation_deg: f32) -> Option<Vec3> {
    use crate::world::weather::Hour;
    let bearing: f32 = match hour {
        Hour::Dawn => 11.0,
        Hour::Dusk => 197.0,
        _ => return None,
    };
    let (b, e) = (bearing.to_radians(), elevation_deg.to_radians());
    Some(Vec3::new(b.cos() * e.cos(), e.sin(), b.sin() * e.cos()))
}

/// How hard a lens facing `toward` glints with the sun at `sun` (toward
/// it): low and in front of it.
pub fn glint(toward: Vec3, sun: Vec3) -> f32 {
    let height = sun.y.clamp(-1.0, 1.0).asin().to_degrees();
    if height <= 0.0 || height >= GLINT_SUN {
        return 0.0;
    }
    let low = 1.0 - height / GLINT_SUN;
    let facing = toward.angle_between(sun);
    if facing >= GLINT_FACING {
        return 0.0;
    }
    low * (1.0 - facing / GLINT_FACING)
}

/// The exposure a flash gives a robot's eye at `eye`.
pub fn glint_seen(g: &Glint, eye: Vec3) -> f32 {
    if g.strength <= 0.0 {
        return 0.0;
    }
    let to = eye - g.at;
    let dist = to.length();
    if dist > GLINT_REACH || g.toward.angle_between(to) > GLINT_CONE {
        return 0.0;
    }
    GLINT_GLARE * g.strength * (1.0 - dist / GLINT_REACH).max(0.2)
}

/// The glints out there this tick, by who carries the optic (for robots'
/// eyes).
#[derive(Resource, Default, Debug)]
pub struct Glints(pub Vec<(Entity, Glint)>);

impl Glints {
    /// The strongest flash `player`'s optics put into an eye at `eye`.
    pub fn seen_by(&self, player: Entity, eye: Vec3) -> f32 {
        self.0
            .iter()
            .filter(|(p, _)| *p == player)
            .map(|(_, g)| glint_seen(g, eye))
            .fold(0.0, f32::max)
    }
}

pub struct OpticsPlugin;

impl Plugin for OpticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Glints>()
            .add_systems(Update, (eye_boxes, glints));
    }
}

/// Each optic's eye box, for the nearest eye: the headset's eye cameras,
/// or (with none, headless) the rig's head.
fn eye_boxes(
    mut commands: Commands,
    optics: Query<(Entity, &Optic, &GlobalTransform, Option<&EyeBox>)>,
    cams: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<crate::render::OffscreenCamera>,
            Without<Optic>,
        ),
    >,
    heads: Query<&GlobalTransform, (With<crate::rig::RigHead>, Without<Optic>)>,
) {
    let mut eyes: Vec<Vec3> = cams.iter().map(|g| g.translation()).collect();
    if eyes.is_empty() {
        eyes = heads.iter().map(|g| g.translation()).collect();
    }
    for (e, o, gt, have) in &optics {
        let eyepiece = gt.transform_point(o.eye);
        let best = eyes
            .iter()
            .map(|eye| eye_box(eyepiece, gt.rotation(), *eye))
            .max_by(|a, b| a.picture.total_cmp(&b.picture))
            .unwrap_or_default();
        if have != Some(&best) {
            commands.entity(e).try_insert(best);
        }
    }
}

/// Which optics glint now, and whose they are.
fn glints(
    mut commands: Commands,
    optics: Query<(Entity, &GlobalTransform, Option<&MountedOn>, Option<&Glint>), With<Optic>>,
    held: Query<&crate::interact::Held>,
    conditions: Option<Res<crate::world::weather::Conditions>>,
    opts: Res<crate::launch::LaunchOptions>,
    mut out: ResMut<Glints>,
) {
    out.0.clear();
    let sun = conditions.and_then(|c| low_sun(c.hour, opts.sun_elevation));
    for (e, gt, mounted, have) in &optics {
        // The objective faces out of the optic's front (-Z).
        let toward = gt.rotation() * Vec3::NEG_Z;
        let g = Glint {
            strength: sun.map_or(0.0, |s| glint(toward, s)),
            at: gt.translation(),
            toward,
        };
        if have.map(|h| h.strength) != Some(g.strength) {
            commands.entity(e).try_insert(g);
        }
        // Whose: the gun it's on, or the optic itself, in a hand.
        let holder = held.get(mounted.map_or(e, |m| m.0)).ok().map(|h| h.player);
        if let (Some(p), true) = (holder, g.strength > 0.0) {
            out.0.push((p, g));
        }
    }
}

/// A lens's black edge (client side): a disc just in front of the
/// eyepiece, black but for a clear round hole that shrinks and slides with
/// the eye box.
#[derive(Component)]
pub struct Vignette {
    material: Handle<StandardMaterial>,
}

/// The hole texture: clear within the disc, black beyond its soft edge.
fn hole_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 128u32;
    let c = n as f32 / 2.0;
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let d = Vec2::new(x as f32 + 0.5 - c, y as f32 + 0.5 - c).length() / c;
            let a = smoothstep(0.86, 1.0, d);
            data[((y * n + x) * 4 + 3) as usize] = (a * 255.0) as u8;
        }
    }
    let mut image = Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    // Outside the texture is black too.
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::ClampToEdge,
        address_mode_v: bevy::image::ImageAddressMode::ClampToEdge,
        ..default()
    });
    images.add(image)
}

pub fn add_vignettes(
    mut commands: Commands,
    optics: Query<(Entity, &Optic), Added<Optic>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hole: Local<Option<Handle<Image>>>,
) {
    for (e, o) in &optics {
        let tex = hole
            .get_or_insert_with(|| hole_texture(&mut images))
            .clone();
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            base_color_texture: Some(tex),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Name::new("Vignette"),
            ChildOf(e),
            Vignette {
                material: material.clone(),
            },
            Mesh3d(meshes.add(Circle::new(0.017))),
            MeshMaterial3d(material),
            // In front of the lens and its reticle.
            Transform::from_translation(o.eye + Vec3::Z * 0.0022),
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The hole follows the eye box: whole on the axis, shrinking and sliding
/// off it.
pub fn vignette_lenses(
    vignettes: Query<(&Vignette, &ChildOf)>,
    boxes: Query<&EyeBox>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (v, parent) in &vignettes {
        let Ok(b) = boxes.get(parent.parent()) else {
            continue;
        };
        let k = 1.0 / b.picture.max(0.02);
        let off = b.hole * 0.5;
        // About the texture's middle: scale (a smaller hole), then slide.
        let want = bevy::math::Affine2::from_translation(Vec2::splat(0.5) - off * k)
            * bevy::math::Affine2::from_scale(Vec2::splat(k))
            * bevy::math::Affine2::from_translation(Vec2::splat(-0.5));
        if let Some(m) = materials.get(&v.material) {
            if m.uv_transform == want {
                continue;
            }
        }
        if let Some(mut m) = materials.get_mut(&v.material) {
            m.uv_transform = want;
        }
    }
}

/// How dark a front post's soft edge is at its middle with the eye down
/// behind the sights, and how far it spreads (metres).
pub const POST_BLUR: f32 = 0.7;
pub const POST_SPREAD: f32 = 0.006;

/// How soft the front post looks with the eye `at_eye` (0..1) down behind
/// the sights: none with the comfort setting off.
pub fn post_blur(at_eye: f32, on: bool) -> f32 {
    if on {
        at_eye.clamp(0.0, 1.0) * POST_BLUR
    } else {
        0.0
    }
}

/// A front post's soft edge (client side): a dark disc round the post,
/// turned to the eye, clear but for a soft middle that darkens as the eye
/// comes down behind the sights.
#[derive(Component)]
pub struct PostBlur {
    material: Handle<StandardMaterial>,
}

/// The soft edge's texture: dark in the middle, fading out.
fn blur_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let n = 64u32;
    let c = n as f32 / 2.0;
    let mut data = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let d = Vec2::new(x as f32 + 0.5 - c, y as f32 + 0.5 - c).length() / c;
            let a = (-(d / 0.45).powi(2)).exp() * (1.0 - smoothstep(0.85, 1.0, d));
            let i = ((y * n + x) * 4) as usize;
            data[i..i + 3].fill(255);
            data[i + 3] = (a * 255.0) as u8;
        }
    }
    images.add(Image::new(
        bevy::render::render_resource::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    ))
}

pub fn add_post_blurs(
    mut commands: Commands,
    guns: Query<(Entity, &super::Gun), Added<super::Gun>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tex: Local<Option<Handle<Image>>>,
    mut disc: Local<Option<Handle<Mesh>>>,
) {
    for (e, gun) in &guns {
        let tex = tex.get_or_insert_with(|| blur_texture(&mut images)).clone();
        let disc = disc
            .get_or_insert_with(|| meshes.add(Circle::new(POST_SPREAD)))
            .clone();
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(0.06, 0.06, 0.065, 0.0),
            base_color_texture: Some(tex),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            Name::new("Front post blur"),
            ChildOf(e),
            PostBlur {
                material: material.clone(),
            },
            Mesh3d(disc),
            MeshMaterial3d(material),
            Transform::from_translation(super::feel::front_post(gun)),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
        ));
    }
}

/// The soft edge darkens with the eye behind the sights, and turns to it.
pub fn blur_posts(
    mut posts: Query<(&PostBlur, &ChildOf, &mut Transform, &mut Visibility)>,
    guns: Query<(&super::feel::Handling, &GlobalTransform)>,
    cams: Query<
        &GlobalTransform,
        (
            With<Camera3d>,
            Without<crate::render::OffscreenCamera>,
            Without<PostBlur>,
        ),
    >,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let on = comfort.as_deref().is_none_or(|c| c.post_blur);
    let eye = cams.iter().next().map(|g| g.translation());
    for (p, parent, mut tf, mut vis) in &mut posts {
        let Ok((handling, gt)) = guns.get(parent.parent()) else {
            continue;
        };
        let a = post_blur(handling.at_eye, on);
        let show = a > 0.01 && eye.is_some();
        let want = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        let Some(eye) = eye.filter(|_| show) else {
            continue;
        };
        // Facing the eye, in the gun's space.
        let to_eye = gt.rotation().inverse() * (eye - gt.transform_point(tf.translation));
        tf.rotation = Quat::from_rotation_arc(Vec3::Z, to_eye.normalize_or(Vec3::Z));
        if let Some(m) = materials.get(&p.material) {
            if (m.base_color.alpha() - a).abs() < 0.005 {
                continue;
            }
        }
        if let Some(mut m) = materials.get_mut(&p.material) {
            m.base_color.set_alpha(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On the axis at the eye relief, the whole picture; off-axis it closes
    /// from the side and slides; too near or too far, black.
    #[test]
    fn the_eye_box() {
        let at = |x: f32, z: f32| eye_box(Vec3::ZERO, Quat::IDENTITY, Vec3::new(x, 0.0, z));
        assert_eq!(at(0.0, RELIEF).picture, 1.0);
        let off = at(0.008, RELIEF);
        assert!(off.picture > 0.1 && off.picture < 0.9, "{off:?}");
        assert!(
            off.shift.x < 0.0 && off.hole.x < 0.0,
            "slides away: {off:?}"
        );
        assert_eq!(at(0.02, RELIEF).picture, 0.0, "well off-axis");
        assert_eq!(at(0.0, RELIEF + 0.08).picture, 0.0, "too far back");
        assert_eq!(at(0.0, -0.01).picture, 0.0, "in front of the eyepiece");
    }

    /// The low sun in front of a scope makes it glint, back along its axis;
    /// by day, with the sun behind it, or off its axis, nothing.
    #[test]
    fn a_scope_glints_into_the_low_sun() {
        use crate::world::weather::Hour;
        let dusk = low_sun(Hour::Dusk, 12.0).unwrap();
        let facing = Vec3::new(dusk.x, 0.0, dusk.z).normalize();
        assert!(glint(facing, dusk) > 0.4);
        assert_eq!(glint(-facing, dusk), 0.0, "sun behind");
        assert!(low_sun(Hour::Day, 20.0).is_none());
        let g = Glint {
            strength: glint(facing, dusk),
            at: Vec3::ZERO,
            toward: facing,
        };
        assert!(
            glint_seen(&g, facing * 150.0) > 0.12,
            "a robot it points at"
        );
        assert_eq!(
            glint_seen(&g, facing.cross(Vec3::Y) * 150.0),
            0.0,
            "off axis"
        );
        assert_eq!(glint_seen(&g, facing * 600.0), 0.0, "too far");
    }

    /// Through the headset: a PSO held up in front of you. The eye on its
    /// axis at the eye relief gets the whole picture; a centimetre to the
    /// side, the black edge closes in; leaning back off the eyepiece, it
    /// blacks out.
    #[test]
    fn the_eye_box_through_the_headset() {
        use crate::vr_harness::Vr;
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        let scope = vr.spawn(
            "pso_scope",
            Transform::from_translation(feet + Vec3::new(0.0, 1.6, -0.45)),
        );
        vr.app
            .world_mut()
            .entity_mut(scope)
            .insert(avian3d::prelude::RigidBody::Static);
        vr.step(3);
        let eyepiece = {
            let o = vr.app.world().get::<Optic>(scope).unwrap().eye;
            vr.app
                .world()
                .get::<GlobalTransform>(scope)
                .unwrap()
                .transform_point(o)
        };
        // The head's pose is in the playspace: from where the player
        // actually stands.
        let root = vr
            .app
            .world()
            .get::<Transform>(vr.player)
            .unwrap()
            .translation;
        let look = |vr: &mut Vr, off: Vec3| {
            vr.head(eyepiece + Vec3::Z * RELIEF + off - root);
            vr.step(3);
            *vr.app.world().get::<EyeBox>(scope).unwrap()
        };
        assert!(look(&mut vr, Vec3::ZERO).picture > 0.95, "on the axis");
        let side = look(&mut vr, Vec3::X * 0.009);
        assert!(
            side.picture > 0.05 && side.picture < 0.8,
            "to the side: {side:?}"
        );
        assert_eq!(look(&mut vr, Vec3::Z * 0.1).picture, 0.0, "leaning back");
    }

    /// A scoped gun held toward a dusk sun puts out a glint, and a robot
    /// along its line sees it; by day, nothing.
    #[test]
    fn a_held_scope_glints_at_dusk() {
        use crate::rig::Side;
        use crate::vr_harness::Vr;
        use crate::world::weather::Hour;
        let mut vr = Vr::new("training_yard");
        let feet = vr
            .app
            .world()
            .resource::<crate::world::Zone>()
            .on_ground(0.0, 30.0);
        vr.stand(feet, 0.0);
        let elevation = vr
            .app
            .world()
            .resource::<crate::launch::LaunchOptions>()
            .sun_elevation;
        let sun = low_sun(Hour::Dusk, elevation).unwrap();
        let flat = Vec3::new(sun.x, 0.0, sun.z).normalize();
        let at = feet + Vec3::new(0.2, 1.3, -0.3);
        let scope = vr.spawn(
            "pso_scope",
            Transform::from_translation(at).looking_to(flat, Vec3::Y),
        );
        vr.app
            .world_mut()
            .entity_mut(scope)
            .insert(avian3d::prelude::RigidBody::Static);
        vr.hand(Side::Right, at, Quat::IDENTITY, 0.0, 0.0);
        vr.step(2);
        vr.grip(Side::Right, 1.0);
        vr.step(3);
        assert_eq!(vr.hand_state(Side::Right).holding, Some(scope));
        let seen = |vr: &mut Vr| {
            let player = vr.player;
            vr.app
                .world()
                .resource::<Glints>()
                .seen_by(player, at + flat * 150.0)
        };
        let set_hour = |vr: &mut Vr, h: &str| {
            vr.app
                .world_mut()
                .resource_mut::<crate::launch::LaunchOptions>()
                .hour = Some(h.into());
            vr.step(3);
        };
        set_hour(&mut vr, "day");
        assert_eq!(seen(&mut vr), 0.0, "by day");
        set_hour(&mut vr, "dusk");
        assert!(
            seen(&mut vr) > crate::robots::senses::GLIMPSE,
            "at dusk: {}",
            seen(&mut vr)
        );
    }
}
