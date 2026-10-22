//! Wounds on the drawn body (`interact::wounds`): a dark patch where each
//! open wound is, a white band round each dressed one, two slats strapped
//! along a splinted shin, and blood on the palm of a glove that's touched
//! a wound (`wounds::Bloodied`).
//!
//! Only a look: the marks are laid on the body's limbs as the stance has
//! them each frame, and cast no shadow.

use bevy::prelude::*;

use crate::interact::Hand;
use crate::interact::wounds::{Bloodied, Fracture, Limb, Wounds, player_segments};
use crate::rig::stance::{Stance, StanceSystems};
use crate::rig::{LocalPlayer, RigHand, RigInput};

pub struct WoundMarksPlugin;

impl Plugin for WoundMarksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Marks>().add_systems(
            PostUpdate,
            (mark_wounds, mark_splint, mark_palms)
                .after(StanceSystems)
                .before(TransformSystems::Propagate),
        );
    }
}

/// The marks drawn, and their looks.
#[derive(Resource, Default)]
struct Marks {
    wounds: Vec<Entity>,
    slats: Vec<Entity>,
    palms: Vec<(Entity, Entity)>,
    look: Option<Looks>,
}

struct Looks {
    patch: Handle<Mesh>,
    band: Handle<Mesh>,
    slat: Handle<Mesh>,
    blood: Handle<StandardMaterial>,
    bandage: Handle<StandardMaterial>,
    wood: Handle<StandardMaterial>,
}

fn looks(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Looks {
    Looks {
        patch: meshes.add(Sphere::new(1.0).mesh().uv(12, 8)),
        band: meshes.add(
            Torus::new(0.85, 1.0)
                .mesh()
                .major_resolution(20)
                .minor_resolution(6),
        ),
        slat: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        blood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.02, 0.02),
            perceptual_roughness: 0.35,
            ..default()
        }),
        bandage: materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.84, 0.78),
            perceptual_roughness: 0.95,
            ..default()
        }),
        wood: materials.add(StandardMaterial {
            base_color: Color::srgb(0.48, 0.36, 0.22),
            perceptual_roughness: 0.85,
            ..default()
        }),
    }
}

fn mark(commands: &mut Commands, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>) -> Entity {
    commands
        .spawn((
            Name::new("Wound mark"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
        ))
        .id()
}

/// Each wound: a patch on the limb's surface where it is, or, dressed, a
/// band round the limb there.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn mark_wounds(
    mut commands: Commands,
    mut marks: ResMut<Marks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    players: Query<(Entity, &Wounds), With<LocalPlayer>>,
    bodies: Query<(&Transform, &RigInput, &Stance, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    mut parts: Query<
        (
            &mut Transform,
            &mut Visibility,
            &mut Mesh3d,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<LocalPlayer>,
    >,
) {
    let marks = &mut *marks;
    let look = marks
        .look
        .get_or_insert_with(|| looks(&mut meshes, &mut materials));
    let Some((player, wounds)) = players.iter().next() else {
        return;
    };
    let segs = player_segments(player, &bodies, &hands).unwrap_or_default();
    while marks.wounds.len() < wounds.0.len() {
        let e = mark(&mut commands, &look.patch, &look.blood);
        marks.wounds.push(e);
    }
    for (i, e) in marks.wounds.iter().enumerate() {
        let Ok((mut t, mut vis, mut mesh, mut mat)) = parts.get_mut(*e) else {
            continue;
        };
        let placed = wounds
            .0
            .get(i)
            .and_then(|w| segs.iter().find(|s| s.limb == w.limb).map(|s| (*w, *s)));
        let Some((w, s)) = placed else {
            *vis = Visibility::Hidden;
            continue;
        };
        let axis = (s.b - s.a).normalize_or(Vec3::NEG_Y);
        if w.dressed {
            // A band round the limb (the torus lies in XZ: its axis is Y).
            let at = s.a.lerp(s.b, w.along);
            *t = Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis))
                .with_scale(Vec3::new(s.r + 0.012, 0.35, s.r + 0.012));
            mesh.0 = look.band.clone();
            mat.0 = look.bandage.clone();
        } else {
            let at = s.point(w.along, w.around);
            let out = (at - s.a.lerp(s.b, w.along)).normalize_or(Vec3::Z);
            // Bigger the more it bleeds; a flat patch on the surface.
            let size = 0.02 + 0.025 * w.bleed.min(1.0);
            *t = Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, out))
                .with_scale(Vec3::new(size, 0.004, size * 1.4));
            mesh.0 = look.patch.clone();
            mat.0 = look.blood.clone();
        }
        // The head and the trunk's marks would sit in the view: only the
        // limbs' show.
        *vis = if matches!(w.limb, Limb::Head) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// A splinted shin: a slat either side of it, strapped.
#[allow(clippy::type_complexity)]
fn mark_splint(
    mut commands: Commands,
    mut marks: ResMut<Marks>,
    players: Query<(Entity, &Fracture), With<LocalPlayer>>,
    bodies: Query<(&Transform, &RigInput, &Stance, &Children), With<LocalPlayer>>,
    hands: Query<(&RigHand, &Hand)>,
    mut parts: Query<(&mut Transform, &mut Visibility), (Without<LocalPlayer>, Without<RigHand>)>,
) {
    let marks = &mut *marks;
    let Some(look) = marks.look.as_ref() else {
        return;
    };
    if marks.slats.is_empty() {
        let a = mark(&mut commands, &look.slat, &look.wood);
        let b = mark(&mut commands, &look.slat, &look.wood);
        marks.slats = vec![a, b];
    }
    let shin = players.iter().next().and_then(|(p, f)| {
        let segs = player_segments(p, &bodies, &hands)?;
        segs.into_iter()
            .find(|s| f.splinted && s.limb == Limb::Shin(f.leg))
    });
    for (i, e) in marks.slats.iter().enumerate() {
        let Ok((mut t, mut vis)) = parts.get_mut(*e) else {
            continue;
        };
        let Some(s) = shin else {
            *vis = Visibility::Hidden;
            continue;
        };
        let axis = (s.b - s.a).normalize_or(Vec3::NEG_Y);
        let x = axis.any_orthonormal_vector();
        let side = if i == 0 { 1.0 } else { -1.0 };
        *t = Transform::from_translation(s.a.lerp(s.b, 0.5) + x * side * (s.r + 0.012))
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis))
            .with_scale(Vec3::new(0.016, s.a.distance(s.b) * 0.9, 0.012));
        *vis = Visibility::Inherited;
    }
}

/// A palm that's touched a wound: blood on it, as much as there is.
#[allow(clippy::type_complexity)]
fn mark_palms(
    mut commands: Commands,
    mut marks: ResMut<Marks>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hands: Query<(Entity, &Hand, Option<&Bloodied>), With<RigHand>>,
    mut parts: Query<
        (
            &mut Transform,
            &mut Visibility,
            &MeshMaterial3d<StandardMaterial>,
        ),
        (Without<RigHand>, Without<LocalPlayer>),
    >,
) {
    let marks = &mut *marks;
    let Some(look) = marks.look.as_ref() else {
        return;
    };
    for (hand_e, hand, blood) in &hands {
        let patch = match marks.palms.iter().find(|(h, _)| *h == hand_e) {
            Some((_, p)) => *p,
            None => {
                // Each palm its own material: how bloody is its alpha.
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgba(0.24, 0.02, 0.02, 0.0),
                    perceptual_roughness: 0.35,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
                let p = mark(&mut commands, &look.patch, &mat);
                marks.palms.push((hand_e, p));
                p
            }
        };
        let Ok((mut t, mut vis, mat)) = parts.get_mut(patch) else {
            continue;
        };
        let amount = blood.map_or(0.0, |b| b.0);
        if amount < 0.03 {
            *vis = Visibility::Hidden;
            continue;
        }
        // On the palm: the glove's -Y side, a little toward the fingers.
        let world = hand.felt.unwrap_or(hand.world);
        *t = world
            .mul_transform(Transform::from_xyz(0.0, -0.028, -0.03))
            .with_scale(Vec3::new(0.034, 0.005, 0.045));
        *vis = Visibility::Inherited;
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color.set_alpha(0.85 * amount);
        }
    }
}
