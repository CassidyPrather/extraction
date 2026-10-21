//! The local player's body, seen looking down: a jacket from the hips to
//! the neck, trousers to the knees and shins, and boots, laid each frame
//! on the stance (`rig::stance`) so the kit on the torso anchor (the belt,
//! the holster, the pouches: `render::kit`) sits on it. The arms are
//! `render::arms`, from the same stance's shoulders.
//!
//! Only a look: nothing here collides, and none of it casts a shadow (the
//! body's shadow is the proxy in `render::arms`, on the sun's own layer).
//! `Comfort::body` off hides it.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::models::ModelMaterials;
use crate::render::materials::MaterialLibrary;
use crate::rig::LocalPlayer;
use crate::rig::comfort::Comfort;
use crate::rig::stance::{ANKLE, Stance, StanceSystems};

/// The spine (pelvis to the neck's base) the jacket's rings are drawn
/// for; a longer or shorter one stretches it.
const SPINE: f32 = 0.63;
/// The jacket's rings up the spine from the pelvis: (metres up, half
/// width, half depth).
const JACKET: [(f32, f32, f32); 7] = [
    (-0.11, 0.15, 0.1),
    (0.0, 0.17, 0.11),
    (0.2, 0.155, 0.1),
    (0.42, 0.175, 0.115),
    (0.54, 0.165, 0.1),
    (0.6, 0.1, 0.075),
    (0.63, 0.06, 0.055),
];
/// Trouser legs: radius at the hip, the knee and the ankle.
const THIGH_R: (f32, f32) = (0.078, 0.056);
const SHIN_R: (f32, f32) = (0.052, 0.042);
/// The boot: sole length and width, height at the heel, and how far the
/// toe runs ahead of the ankle.
const BOOT: Vec3 = Vec3::new(0.105, 0.1, 0.28);
const TOE_AHEAD: f32 = 0.06;

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, grow_body).add_systems(
            PostUpdate,
            pose_body
                .after(StanceSystems)
                .before(TransformSystems::Propagate),
        );
    }
}

/// The drawn body of a local player.
#[derive(Component)]
struct DrawnBody {
    player: Entity,
    jacket: Entity,
    thighs: [Entity; 2],
    knees: [Entity; 2],
    shins: [Entity; 2],
    boots: [Entity; 2],
}

/// A piece of the drawn body.
#[derive(Component)]
pub struct BodyPart;

/// A closed loft along +Y through rings of (y, half width, half depth),
/// each a rounded rectangle (a superellipse), normals out from the axis.
pub fn loft(rings: &[(f32, f32, f32)], segments: usize) -> Mesh {
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut uv = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let around = |i: usize| {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (s, c) = a.sin_cos();
        // A superellipse, |x|^2.6 + |z|^2.6 = 1: squarer than an ellipse.
        let e = 2.0 / 2.6;
        (c.signum() * c.abs().powf(e), s.signum() * s.abs().powf(e))
    };
    let n = rings.len();
    for (r, &(y, hx, hz)) in rings.iter().enumerate() {
        // The slope of the side here, for the normals.
        let (y0, hx0, hz0) = rings[r.saturating_sub(1)];
        let (y1, hx1, hz1) = rings[(r + 1).min(n - 1)];
        let dy = (y1 - y0).max(1e-4);
        for i in 0..=segments {
            let (x, z) = around(i);
            pos.push([x * hx, y, z * hz]);
            let out = Vec2::new(x / hx.max(1e-3), z / hz.max(1e-3)).normalize_or(Vec2::X);
            let slope = -((hx1 - hx0) * x.abs() + (hz1 - hz0) * z.abs()) / dy;
            nor.push(Vec3::new(out.x, slope, out.y).normalize().to_array());
            uv.push([i as f32 / segments as f32, y]);
        }
    }
    let row = segments as u32 + 1;
    for r in 0..n as u32 - 1 {
        for i in 0..segments as u32 {
            let a = r * row + i;
            let b = a + row;
            idx.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    // Caps.
    for (r, down) in [(0, true), (n - 1, false)] {
        let (y, ..) = rings[r];
        let centre = pos.len() as u32;
        pos.push([0.0, y, 0.0]);
        nor.push([0.0, if down { -1.0 } else { 1.0 }, 0.0]);
        uv.push([0.5, y]);
        let base = r as u32 * row;
        for i in 0..segments as u32 {
            if down {
                idx.extend([centre, base + i, base + i + 1]);
            } else {
                idx.extend([centre, base + i + 1, base + i]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

/// A limb's tube from y 0 (radius `r0`) to y 1 (`r1`): placed by
/// [`bone`], stretched to the bone.
fn tube(r0: f32, r1: f32) -> Mesh {
    loft(
        &[
            (0.0, r0, r0),
            (0.5, (r0 + r1) / 2.0 + 0.004, (r0 + r1) / 2.0),
            (1.0, r1, r1),
        ],
        12,
    )
}

/// The boot, its sole's middle at the origin, the toe toward -Z.
fn boot() -> Mesh {
    let (w, h, l) = (BOOT.x / 2.0, BOOT.y, BOOT.z / 2.0);
    // Laid lengthwise: rings along +Y from the heel to the toe, then
    // turned so +Y is -Z.
    let rings = [
        (-l, w * 0.8, h * 0.45),
        (-l + 0.03, w, h * 0.5),
        (0.0, w, h * 0.42),
        (l - 0.05, w * 0.95, h * 0.3),
        (l, w * 0.6, h * 0.2),
    ];
    let mut m = loft(&rings, 12);
    m.rotate_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    m.translate_by(Vec3::Y * h * 0.4);
    m
}

/// A limb's transform: its tube's y 0 at `a`, y 1 at `b`.
fn bone(a: Vec3, b: Vec3, toward: Vec3) -> Transform {
    let d = b - a;
    let len = d.length().max(0.01);
    let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
    // Keep the tube's front (-Z) toward `toward`, so the seam doesn't spin.
    let front = rot * Vec3::NEG_Z;
    let want = (toward - d / len * toward.dot(d / len)).normalize_or(front);
    let twist = Quat::from_rotation_arc(front, want);
    Transform::from_translation(a)
        .with_rotation(twist * rot)
        .with_scale(Vec3::new(1.0, len, 1.0))
}

fn grow_body(
    mut commands: Commands,
    players: Query<Entity, Added<LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for player in &players {
        let jacket_mat = model_mats.get("Canvas", &mut lib, &server, &mut materials);
        let trousers = materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.21, 0.17),
            perceptual_roughness: 0.9,
            ..default()
        });
        let leather = materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.09, 0.07),
            perceptual_roughness: 0.7,
            ..default()
        });
        let mut part = |mesh: Handle<Mesh>, mat: &Handle<StandardMaterial>| {
            commands
                .spawn((
                    Name::new("Body part"),
                    Mesh3d(mesh),
                    MeshMaterial3d(mat.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::light::NotShadowCaster,
                    BodyPart,
                ))
                .id()
        };
        let jacket = part(meshes.add(loft(&JACKET, 20)), &jacket_mat);
        let thigh = meshes.add(tube(THIGH_R.0, THIGH_R.1));
        let shin = meshes.add(tube(SHIN_R.0, SHIN_R.1));
        let knee = meshes.add(Sphere::new(THIGH_R.1 + 0.002).mesh().uv(12, 8));
        let boot_mesh = meshes.add(boot());
        let thighs = [part(thigh.clone(), &trousers), part(thigh, &trousers)];
        let knees = [part(knee.clone(), &trousers), part(knee, &trousers)];
        let shins = [part(shin.clone(), &trousers), part(shin, &trousers)];
        let boots = [part(boot_mesh.clone(), &leather), part(boot_mesh, &leather)];
        commands.spawn((
            Name::new("Drawn body"),
            DrawnBody {
                player,
                jacket,
                thighs,
                knees,
                shins,
                boots,
            },
        ));
    }
}

fn pose_body(
    bodies: Query<&DrawnBody>,
    stances: Query<&Stance>,
    mut parts: Query<(&mut Transform, &mut Visibility), With<BodyPart>>,
    comfort: Option<Res<Comfort>>,
) {
    let shown = comfort.as_deref().is_none_or(|c| c.body);
    for body in &bodies {
        let Ok(st) = stances.get(body.player) else {
            continue;
        };
        let show = if shown && st.ready {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let spine = st.neck - st.pelvis.translation;
        let jacket = Transform::from_translation(st.pelvis.translation)
            .with_rotation(st.chest.rotation)
            .with_scale(Vec3::new(
                1.0,
                (spine.length() / SPINE).clamp(0.6, 1.4),
                1.0,
            ));
        let ahead = st.pelvis.rotation * Vec3::NEG_Z;
        let mut place = vec![(body.jacket, jacket)];
        for i in 0..2 {
            let foot = st.feet[i];
            let toes = Quat::from_rotation_y(foot.yaw) * Vec3::NEG_Z;
            place.push((body.thighs[i], bone(st.hips[i], st.knees[i], ahead)));
            place.push((body.knees[i], Transform::from_translation(st.knees[i])));
            place.push((body.shins[i], bone(st.knees[i], st.ankles[i], ahead)));
            // The sole under the ankle, the toe ahead of it.
            let sole = st.ankles[i] - Vec3::Y * ANKLE + toes * (TOE_AHEAD - 0.03);
            place.push((
                body.boots[i],
                Transform::from_translation(sole).with_rotation(Quat::from_rotation_y(foot.yaw)),
            ));
        }
        for (e, t) in place {
            if let Ok((mut pt, mut vis)) = parts.get_mut(e) {
                *pt = t;
                *vis = show;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The jacket is closed and its normals point out of it.
    #[test]
    fn the_jacket_is_closed_and_faces_out() {
        let m = loft(&JACKET, 20);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(pos)) =
            m.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(nor)) =
            m.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("no normals");
        };
        for (p, n) in pos.iter().zip(nor) {
            let side = Vec2::new(p[0], p[2]);
            if side.length() > 0.01 {
                assert!(side.dot(Vec2::new(n[0], n[2])) > 0.0, "{p:?} {n:?}");
            }
        }
        // Every edge shared by two triangles: no holes.
        let Some(Indices::U32(idx)) = m.indices() else {
            panic!("no indices");
        };
        let mut edges = std::collections::HashMap::new();
        for t in idx.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                // Seam vertices repeat (the uv's wrap); weld by position.
                let key = |v: u32| {
                    let p = pos[v as usize];
                    (
                        (p[0] * 1e4) as i32,
                        (p[1] * 1e4) as i32,
                        (p[2] * 1e4) as i32,
                    )
                };
                let (ka, kb) = (key(a), key(b));
                *edges
                    .entry(if ka < kb { (ka, kb) } else { (kb, ka) })
                    .or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|&n| n == 2), "a hole in the jacket");
    }
}
