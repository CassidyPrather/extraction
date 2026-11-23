//! How the pocket spheres look (`interact::pockets` says what they
//! show). One mesh and one material for all of them and their icons: each
//! one's grey and alpha ride in its `MeshTag` (`pockets.wgsl`), so they
//! draw together.
//!
//! The icon in a full slot's sphere is what's in it, small, in its own
//! colours a little dimmed (each part its material's): the
//! thing's own meshes merged into one (built once a kind, so a sphere's
//! icon is one draw), turned so its broad side faces the eye and its long side hangs
//! down (a gun barrel down, [`icon_face`]), swaying a little.

use bevy::asset::RenderAssetUsages;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, MeshTag, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Face, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;
use std::collections::HashMap;

use crate::interact::pockets::PocketSphere;
use crate::items::Item;
use extraction_core::items::ItemKind;

/// An icon's size across, in its sphere's radii.
pub const ICON_ACROSS: f32 = 1.3;
/// A part further than this from its thing's origin isn't its (m).
const PART_REACH: f32 = 0.6;

/// A mesh under a thing, and its own visibility.
type IconPart = (
    &'static Mesh3d,
    &'static GlobalTransform,
    Option<&'static Visibility>,
    Option<&'static MeshMaterial3d<StandardMaterial>>,
);

/// An icon's alpha, from its sphere's (up to this).
pub const ICON_ALPHA: f32 = 0.9;
/// How far an icon sways either way (rad), and how fast (rad/s).
pub const ICON_SWAY: (f32, f32) = (0.35, 0.9);

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct PocketMaterial {}

impl Material for PocketMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/pockets.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Back);
        Ok(())
    }
}

/// A sphere's look as its tag: alpha, grey, and whether it's an icon.
pub fn tag(alpha: f32, grey: f32, icon: bool) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    q(alpha) | (q(grey) << 8) | (u32::from(icon) << 16)
}

/// The shared mesh and material, and each kind's icon.
#[derive(Resource)]
struct PocketArt {
    sphere: Handle<Mesh>,
    material: Handle<PocketMaterial>,
    icons: HashMap<ItemKind, Icon>,
}

#[derive(Clone)]
struct Icon {
    mesh: Handle<Mesh>,
    /// Its middle and its largest extent, in its thing's own frame.
    mid: Vec3,
    across: f32,
    /// Turns its thinnest axis to +Z (its broad side to the eye).
    face: Quat,
}

/// The icon drawn in a sphere, for `occupant`.
#[derive(Component)]
struct IconOf {
    occupant: Entity,
    face: Quat,
}

pub struct PocketVisualsPlugin;

impl Plugin for PocketVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<PocketMaterial>::default())
            .add_systems(Startup, make_art)
            .add_observer(dress_sphere)
            .add_systems(
                PostUpdate,
                (tag_spheres, icons)
                    .chain()
                    .after(crate::interact::pockets::PocketsSystems)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn make_art(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PocketMaterial>>,
) {
    let sphere = meshes.add(Sphere::new(1.0).mesh().ico(3).expect("an icosphere"));
    commands.insert_resource(PocketArt {
        sphere,
        material: materials.add(PocketMaterial::default()),
        icons: HashMap::new(),
    });
}

fn dress_sphere(add: On<Add, PocketSphere>, mut commands: Commands, art: Option<Res<PocketArt>>) {
    let Some(art) = art else {
        return;
    };
    commands.entity(add.entity).insert((
        Mesh3d(art.sphere.clone()),
        MeshMaterial3d(art.material.clone()),
        MeshTag(0),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn tag_spheres(mut spheres: Query<(&PocketSphere, &mut MeshTag), Changed<PocketSphere>>) {
    for (s, mut t) in &mut spheres {
        let want = tag(s.look.alpha, s.look.grey, false);
        if t.0 != want {
            t.0 = want;
        }
    }
}

/// The occupant's meshes merged into one, in its own frame (positions and
/// normals, and each part's colour: an icon is flat, in its own colours). None until its meshes are in.
fn build_icon(
    occupant: Entity,
    meshes: &Assets<Mesh>,
    children: &Query<&Children>,
    parts: &Query<IconPart, Without<PocketSphere>>,
    globals: &Query<&GlobalTransform>,
    materials: &Assets<StandardMaterial>,
) -> Option<(Mesh, Vec3, f32, Quat)> {
    let base = globals.get(occupant).ok()?.affine().inverse();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut stack = vec![occupant];
    while let Some(e) = stack.pop() {
        if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
        let Ok((m, g, vis, mat)) = parts.get(e) else {
            continue;
        };
        // Not a part kept hidden (the thing itself is: stowed,
        // `render::kit` hides it).
        if e != occupant && vis == Some(&Visibility::Hidden) {
            continue;
        }
        // A part drawn from the render world only (a sight's reticle, a
        // compass's face) is left out: the rest is the shape.
        let Some(mesh) = meshes.get(&m.0) else {
            continue;
        };
        if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
            continue;
        }
        let (Some(VertexAttributeValues::Float32x3(p)), Some(VertexAttributeValues::Float32x3(n))) = (
            mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
        ) else {
            continue;
        };
        let to = base * g.affine();
        // Nor one left somewhere else (a far stand-in at the world's
        // origin).
        if Vec3::from(to.translation).length() > PART_REACH {
            continue;
        }
        let start = positions.len() as u32;
        positions.extend(
            p.iter()
                .map(|v| to.transform_point3(Vec3::from(*v)).to_array()),
        );
        normals.extend(n.iter().map(|v| {
            to.transform_vector3(Vec3::from(*v))
                .normalize_or_zero()
                .to_array()
        }));
        // Each part in its own colour (its material's: playtest
        // 2026-11-22, a white copy read as a bug).
        let tint = icon_tint(
            mat.and_then(|m| materials.get(&m.0))
                .map_or(LinearRgba::gray(0.6), |m| m.base_color.to_linear()),
        );
        colors.extend(std::iter::repeat_n(
            [tint.red, tint.green, tint.blue, 1.0],
            p.len(),
        ));
        match mesh.indices() {
            Some(Indices::U16(i)) => indices.extend(i.iter().map(|&i| start + u32::from(i))),
            Some(Indices::U32(i)) => indices.extend(i.iter().map(|&i| start + i)),
            None => indices.extend(start..start + p.len() as u32),
        }
    }
    if positions.is_empty() {
        return None;
    }
    let (lo, hi) = positions.iter().fold(
        (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
        |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))),
    );
    let size = hi - lo;
    let face = icon_face(size);
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    Some((mesh, (lo + hi) * 0.5, size.max_element().max(0.01), face))
}

/// The lightest a part of an icon may be dark (linear luminance, before
/// the shader's dimming): a blued pistol in its own colour was gone
/// against the ground in its sphere. Darker is lifted with grey, its hue
/// kept.
pub const ICON_FLOOR: f32 = 0.18;

/// A part's colour as its icon draws it: its own, lifted to
/// [`ICON_FLOOR`] if darker.
pub fn icon_tint(c: LinearRgba) -> LinearRgba {
    let lum = 0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue;
    let lift = (ICON_FLOOR - lum).max(0.0);
    LinearRgba::rgb(c.red + lift, c.green + lift, c.blue + lift)
}

/// How an icon of a thing `size` (its own frame) is turned in its sphere:
/// its thinnest axis to +Z (its broad side to the eye), then about that so
/// its longest hangs straight down, the end that leads first (a gun's
/// muzzle, -Z) at the bottom: a pistol in the holster barrel down, as the
/// playtest of 2026-11-22 asked, not lying sideways.
pub fn icon_face(size: Vec3) -> Quat {
    let face = if size.x <= size.y && size.x <= size.z {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    } else if size.y <= size.z {
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    let long = if size.z >= size.x && size.z >= size.y {
        Vec3::NEG_Z
    } else if size.y >= size.x {
        Vec3::NEG_Y
    } else {
        Vec3::NEG_X
    };
    let v = face * long;
    if v.truncate().length_squared() < 1e-6 {
        return face;
    }
    let roll = -std::f32::consts::FRAC_PI_2 - v.y.atan2(v.x);
    Quat::from_rotation_z(roll) * face
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn icons(
    mut commands: Commands,
    mut art: Option<ResMut<PocketArt>>,
    mut meshes: ResMut<Assets<Mesh>>,
    spheres: Query<(Entity, &PocketSphere, &GlobalTransform, Option<&Children>)>,
    mut existing: Query<(&IconOf, &mut Transform, Option<&Children>), Without<PocketSphere>>,
    mut icon_tags: Query<&mut MeshTag, (With<IconTag>, Without<PocketSphere>)>,
    items: Query<&Item>,
    children: Query<&Children>,
    parts: Query<IconPart, Without<PocketSphere>>,
    std_materials: Res<Assets<StandardMaterial>>,
    globals: Query<&GlobalTransform>,
    locals: Query<(&GlobalTransform, &crate::rig::RigInput), With<crate::rig::LocalPlayer>>,
    time: Res<Time>,
) {
    let Some(art) = art.as_deref_mut() else {
        return;
    };
    let eye = locals
        .iter()
        .next()
        .map(|(root, input)| root.transform_point(input.head.translation));
    let sway = Quat::from_rotation_y(ICON_SWAY.0 * (time.elapsed_secs() * ICON_SWAY.1).sin());
    for (sphere_e, sphere, sg, kids) in &spheres {
        let want = sphere.holds.filter(|_| sphere.look.alpha > 0.01);
        // The icon it has, if it's the right one; others go.
        let mut have = false;
        for c in kids.into_iter().flatten() {
            let Ok((icon, mut tf, meshes_in)) = existing.get_mut(*c) else {
                continue;
            };
            if Some(icon.occupant) != want {
                commands.entity(*c).despawn();
                continue;
            }
            have = true;
            let alpha = (sphere.look.alpha * 4.0).min(ICON_ALPHA);
            let tg = tag(alpha, 0.97, true);
            for m in meshes_in.into_iter().flatten() {
                if let Ok(mut t) = icon_tags.get_mut(*m)
                    && t.0 != tg
                {
                    t.0 = tg;
                }
            }
            if let Some(eye) = eye {
                let at = sg.translation();
                let to_eye = Transform::IDENTITY.looking_to(at - eye, Vec3::Y).rotation;
                let (_, parent_rot, _) = sg.to_scale_rotation_translation();
                tf.rotation = parent_rot.inverse() * to_eye * sway * icon.face;
            }
        }
        let Some(occupant) = want else {
            continue;
        };
        if have {
            continue;
        }
        let Ok(kind) = items.get(occupant).map(|i| i.kind()) else {
            continue;
        };
        if !art.icons.contains_key(&kind) {
            let Some((mesh, mid, across, face)) = build_icon(
                occupant,
                &meshes,
                &children,
                &parts,
                &globals,
                &std_materials,
            ) else {
                continue;
            };
            let icon = Icon {
                mesh: meshes.add(mesh),
                mid,
                across,
                face,
            };
            art.icons.insert(kind, icon);
        }
        let icon = art.icons[&kind].clone();
        let k = ICON_ACROSS / icon.across;
        // Holder (turned to the eye) and the mesh in it, centred.
        let holder = commands
            .spawn((
                Name::new("Pocket icon"),
                IconOf {
                    occupant,
                    face: icon.face,
                },
                Transform::IDENTITY,
                Visibility::Inherited,
                ChildOf(sphere_e),
            ))
            .id();
        commands.spawn((
            Mesh3d(icon.mesh.clone()),
            MeshMaterial3d(art.material.clone()),
            MeshTag(tag(0.0, 0.97, true)),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_scale(Vec3::splat(k)).with_translation(-icon.mid * k),
            IconTag,
            ChildOf(holder),
        ));
    }
}

/// An icon's mesh: its tag follows its holder's.
#[derive(Component)]
struct IconTag;

#[cfg(test)]
mod tests {
    use super::*;

    /// A part's own colour stays its own; one too dark to see is lifted
    /// to the floor, its hue kept.
    #[test]
    fn a_dark_part_is_lifted_a_light_one_kept() {
        let brass = LinearRgba::rgb(0.5, 0.3, 0.08);
        assert_eq!(icon_tint(brass), brass);
        let blued = icon_tint(LinearRgba::rgb(0.01, 0.012, 0.02));
        let lum = 0.2126 * blued.red + 0.7152 * blued.green + 0.0722 * blued.blue;
        assert!((lum - ICON_FLOOR).abs() < 1e-4, "{lum}");
        assert!(blued.blue > blued.red, "its blue kept");
    }

    /// An icon shows its broad side and hangs its long axis down, a gun's
    /// muzzle (-Z) lowest: a pistol (long in Z, thin in X), a magazine
    /// (tall in Y), a can (any).
    #[test]
    fn an_icon_hangs_long_side_down_broad_side_out() {
        for size in [
            Vec3::new(0.03, 0.13, 0.19),
            Vec3::new(0.02, 0.12, 0.06),
            Vec3::new(0.2, 0.05, 0.08),
        ] {
            let q = icon_face(size);
            let thin = if size.x <= size.y && size.x <= size.z {
                Vec3::X
            } else if size.y <= size.z {
                Vec3::Y
            } else {
                Vec3::Z
            };
            assert!((q * thin).z.abs() > 0.999, "{size}: broad side out");
            let long = if size.z >= size.x && size.z >= size.y {
                Vec3::NEG_Z
            } else if size.y >= size.x {
                Vec3::NEG_Y
            } else {
                Vec3::NEG_X
            };
            assert!((q * long).y < -0.999, "{size}: hangs down, {}", q * long);
        }
    }
}
