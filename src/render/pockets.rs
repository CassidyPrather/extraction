//! How the pocket spheres look (`interact::pockets` says what they
//! show). One mesh and one material for all of them and their icons: each
//! one's grey and alpha ride in its `MeshTag` (`pockets.wgsl`), so they
//! draw together.
//!
//! The icon in a full slot's sphere is what's in it, small and white: the
//! thing's own meshes merged into one (built once a kind, so a sphere's
//! icon is one draw), turned so its broad side faces the eye, swaying a
//! little.

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
/// normals: an icon is flat white). None until its meshes are in.
fn build_icon(
    occupant: Entity,
    meshes: &Assets<Mesh>,
    children: &Query<&Children>,
    parts: &Query<(&Mesh3d, &GlobalTransform), Without<PocketSphere>>,
    globals: &Query<&GlobalTransform>,
) -> Option<(Mesh, Vec3, f32, Quat)> {
    let base = globals.get(occupant).ok()?.affine().inverse();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut stack = vec![occupant];
    while let Some(e) = stack.pop() {
        if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
        let Ok((m, g)) = parts.get(e) else {
            continue;
        };
        let mesh = meshes.get(&m.0)?;
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
    let face = if size.x <= size.y && size.x <= size.z {
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
    } else if size.y <= size.z {
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    Some((mesh, (lo + hi) * 0.5, size.max_element().max(0.01), face))
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
    parts: Query<(&Mesh3d, &GlobalTransform), Without<PocketSphere>>,
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
            let Some((mesh, mid, across, face)) =
                build_icon(occupant, &meshes, &children, &parts, &globals)
            else {
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
