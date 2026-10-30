//! Forearms and sleeves: the local player's arms from a shoulder estimated
//! on the body to the glove's cuff, bent at the elbow by two-bone IK, so a
//! look at a gun or the wrist display shows your sleeves, not gloves
//! floating at the end of nothing.
//!
//! Only a look: nothing collides, and the sleeves cast no shadow (like the
//! rest of the body, see `props::body_casts_no_shadow`). The body's shadow
//! comes from a proxy instead: capsules for the head, torso, legs and these
//! same arms on a render layer only the sun sees (`SHADOW_ONLY`), so the
//! gloves' shadows hang off arms and a body, not off nothing. The shoulders ride the torso
//! anchor (`interact::body::Torso`, where the rig's straps are), dropping
//! with the head when you bend over; the elbow hangs down and out and is
//! kept out of the body.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::interact::body::Torso;
use crate::models::ModelMaterials;
use crate::render::materials::MaterialLibrary;
use crate::render::props::{GlovePose, HandVisual, PlayerGlove, glove_frame};
use crate::rig::{RigInput, Side};

/// Shoulder to elbow, and elbow to the sleeve's hem at the wrist, metres.
pub const UPPER: f32 = 0.29;
pub const FOREARM: f32 = 0.25;
/// The shoulder joint in the torso's frame (origin mid-chest, -Z ahead),
/// on the right; mirrored for the left.
const SHOULDER: Vec3 = Vec3::new(0.17, 0.12, 0.03);
/// Eyes to shoulders, for when the head drops below the torso's height.
const SHOULDER_BELOW_EYES: f32 = 0.26;
/// The body the elbow keeps out of: a column this wide round the neck.
const BODY_R: f32 = 0.19;
/// Where the sleeve's hem sits on the unrolled glove: just past the cuff's
/// end, over it, round the wrist's centre.
const HEM: Vec3 = Vec3::new(0.0, -0.0034, 0.1);
/// On the watch arm (not the gun hand's) the hem sits further back, clear of the wrist
/// readout's elbow end (`render::comfort`, its panel to z 0.1015 on the
/// cuff's housing): over it, the sleeve hid the start of its lines.
const WATCH_HEM: Vec3 = Vec3::new(0.0, -0.0034, 0.112);
const SLEEVE_WRIST_R: f32 = 0.047;
const SLEEVE_ELBOW_R: f32 = 0.052;
const SLEEVE_SHOULDER_R: f32 = 0.058;

/// The render layer only the sun has besides the default: the body's
/// shadow proxy lives on it, drawn into the shadow map and by no camera.
pub const SHADOW_ONLY: usize = 8;

/// The layers the sun casts shadows from (`world::sky`).
pub fn sun_layers() -> bevy::camera::visibility::RenderLayers {
    bevy::camera::visibility::RenderLayers::from_layers(&[0, SHADOW_ONLY])
}

pub struct ArmsPlugin;

impl Plugin for ArmsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (grow_arms, grow_body_shadow))
            .add_systems(
                PostUpdate,
                (pose_arms, pose_body_shadow)
                    .chain()
                    .after(GlovePose)
                    .after(crate::rig::stance::StanceSystems)
                    .before(TransformSystems::Propagate),
            );
    }
}

/// One of the local player's arms, following its glove.
#[derive(Component)]
pub(crate) struct Arm {
    glove: Entity,
    side: Side,
    upper: Entity,
    forearm: Entity,
    elbow: Entity,
    shoulder: Entity,
    /// The arm's two bones in the shadow proxy.
    shadow: [Entity; 2],
}

/// How much further than its length an arm may straighten to meet its
/// hand before the shoulder is pulled after it.
pub const STRETCH: f32 = 1.04;

/// Where the elbow goes, given the shoulder, the wrist, which way the elbow
/// should point (`hint`), the body's column (its axis in x, z) and the
/// arm's length (`scale`: `rig::stance` learns it from the reach): the
/// shoulder and the elbow. Just out of reach the arm straightens a little
/// further ([`STRETCH`]); only past that is the shoulder pulled forward,
/// the arm straight. (The stance has already brought the shoulder forward
/// as far as a shoulder goes.)
pub fn arm_joints(shoulder: Vec3, wrist: Vec3, hint: Vec3, body: Vec2, scale: f32) -> (Vec3, Vec3) {
    let (mut upper, mut fore) = (UPPER * scale, FOREARM * scale);
    let mut s = shoulder;
    let reach = upper + fore - 0.002;
    let to = wrist - s;
    let d = to.length();
    if d > reach {
        let k = (d / reach).min(STRETCH);
        upper *= k;
        fore *= k;
        let reach = reach * k;
        if d > reach {
            s = wrist - to / d * reach;
        }
    }
    let to = wrist - s;
    let d = to.length().max(0.02);
    let dir = to / d;
    let along = (upper * upper - fore * fore + d * d) / (2.0 * d);
    let up = (upper * upper - along * along).max(0.0).sqrt();
    let mut bend = hint - dir * hint.dot(dir);
    if bend.length_squared() < 1e-6 {
        bend = dir.any_orthonormal_vector();
    }
    let mut elbow = s + dir * along + bend.normalize() * up;
    // Never into the body: out of the column round the neck, sideways.
    let flat = Vec2::new(elbow.x, elbow.z) - body;
    if flat.length() < BODY_R {
        let out = if flat.length_squared() > 1e-8 {
            flat.normalize()
        } else {
            Vec2::new(hint.x, hint.z).normalize_or(Vec2::X)
        };
        let fixed = body + out * BODY_R;
        elbow.x = fixed.x;
        elbow.z = fixed.y;
    }
    (s, elbow)
}

/// A sleeve: a tapered tube along -Z from its open hem (z 0, radius `r0`,
/// a lip turned in) to z -1 (radius `r1`, closed: the elbow's ball covers
/// it), with a couple of soft folds. Stretched along Z to the bone.
fn sleeve_mesh(r0: f32, r1: f32, hem: bool) -> Mesh {
    const SEG: usize = 16;
    const RINGS: usize = 14;
    let mut pos = Vec::new();
    let mut nrm = Vec::new();
    let mut uv = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    let ring = |pos: &mut Vec<[f32; 3]>,
                nrm: &mut Vec<[f32; 3]>,
                uv: &mut Vec<[f32; 2]>,
                r: f32,
                z: f32,
                inward: bool| {
        for k in 0..=SEG {
            let a = k as f32 / SEG as f32 * std::f32::consts::TAU;
            let (sn, c) = a.sin_cos();
            pos.push([c * r, sn * r, z]);
            let n = if inward { [-c, -sn, 0.0] } else { [c, sn, 0.0] };
            nrm.push(n);
            uv.push([a * r, -z * 0.25]);
        }
    };
    let mut rings = 0u32;
    if hem {
        // The lip: inside the hem, turned back out to it.
        ring(&mut pos, &mut nrm, &mut uv, r0 - 0.004, -0.04, true);
        ring(&mut pos, &mut nrm, &mut uv, r0 - 0.004, 0.0, true);
        rings += 2;
    }
    for i in 0..=RINGS {
        let t = i as f32 / RINGS as f32;
        let fold =
            0.0025 * (t * std::f32::consts::PI * 5.0).sin() * (t * std::f32::consts::PI).sin();
        ring(
            &mut pos,
            &mut nrm,
            &mut uv,
            r0 + (r1 - r0) * t + fold,
            -t,
            false,
        );
        rings += 1;
    }
    // Consecutive rings joined (with a hem, the lip's mouth meets the outer
    // hem in a flat ring facing the hand).
    let row = (SEG + 1) as u32;
    for rr in 0..rings - 1 {
        for k in 0..SEG as u32 {
            let a = rr * row + k;
            let b = a + row;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(idx))
}

fn grow_arms(
    mut commands: Commands,
    gloves: Query<(Entity, &HandVisual), Added<PlayerGlove>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for (glove, v) in &gloves {
        // The jacket's cloth, both faces drawn: a sleeve seen from inside
        // (the cuff turned toward the eye) is cloth, not a hole.
        let canvas = model_mats.get("Canvas", &mut lib, &server, &mut materials);
        let jacket = match materials.get(&canvas).cloned() {
            Some(mut m) => {
                m.cull_mode = None;
                m.double_sided = true;
                materials.add(m)
            }
            None => canvas,
        };
        let mut part = |commands: &mut Commands, mesh: Mesh| {
            commands
                .spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(jacket.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::light::NotShadowCaster,
                    ArmPart,
                ))
                .id()
        };
        let forearm = part(
            &mut commands,
            sleeve_mesh(SLEEVE_WRIST_R, SLEEVE_ELBOW_R, true),
        );
        let upper = part(
            &mut commands,
            sleeve_mesh(SLEEVE_ELBOW_R, SLEEVE_SHOULDER_R, false),
        );
        let elbow = part(
            &mut commands,
            Sphere::new(SLEEVE_ELBOW_R + 0.001).mesh().uv(16, 10),
        );
        let shoulder = part(
            &mut commands,
            Sphere::new(SLEEVE_SHOULDER_R).mesh().uv(16, 10),
        );
        let shadow = [
            shadow_part(&mut commands, &mut meshes, &mut materials, &mut look),
            shadow_part(&mut commands, &mut meshes, &mut materials, &mut look),
        ];
        commands.spawn((
            Name::new(format!("{:?} arm", v.side)),
            Arm {
                glove,
                side: v.side,
                upper,
                forearm,
                elbow,
                shoulder,
                shadow,
            },
        ));
    }
}

/// Lays each arm from its shoulder to its glove's cuff, from this frame's
/// glove pose (so it runs after `pose_hands`, before propagation).
#[allow(clippy::type_complexity)]
fn pose_arms(
    arms: Query<&Arm>,
    gloves: Query<(&HandVisual, &Transform, &Visibility), (Without<ArmPart>, Without<ShadowPart>)>,
    hands: Query<&ChildOf, With<crate::interact::Hand>>,
    players: Query<
        (&Transform, &RigInput, &Children),
        (Without<HandVisual>, Without<ArmPart>, Without<ShadowPart>),
    >,
    torsos: Query<
        &Transform,
        (
            With<Torso>,
            Without<HandVisual>,
            Without<ArmPart>,
            Without<ShadowPart>,
        ),
    >,
    mut parts: Query<
        (&mut Transform, &mut Visibility),
        (Or<(With<ArmPart>, With<ShadowPart>)>, Without<HandVisual>),
    >,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    stances: Query<&crate::rig::stance::Stance>,
) {
    let watch = comfort
        .as_deref()
        .map_or(crate::rig::Side::Left, |c| c.gun_side().other());
    let body = comfort.as_deref().is_none_or(|c| c.body);
    for arm in &arms {
        let Ok((v, glove, glove_vis)) = gloves.get(arm.glove) else {
            continue;
        };
        let player = hands.get(v.hand).ok().map(|p| p.parent());
        let Some((root, input, children)) = player.and_then(|p| players.get(p).ok()) else {
            continue;
        };
        let Some(torso) = children.iter().find_map(|c| torsos.get(c).ok()) else {
            continue;
        };
        let torso = root.mul_transform(*torso);
        let head = root.mul_transform(input.head);
        let s = v.side.sign();
        let stance = player
            .and_then(|p| stances.get(p).ok())
            .filter(|st| st.ready);
        let shoulder = match stance {
            Some(st) => st.shoulder(v.side),
            None => {
                let mut sh = torso.transform_point(SHOULDER * Vec3::new(s, 1.0, 1.0));
                sh.y = sh.y.min(head.translation.y - SHOULDER_BELOW_EYES);
                sh
            }
        };
        let scale = stance.map_or(1.0, |st| st.arm);
        let hint = torso.rotation * Vec3::new(0.6 * s, -1.0, 0.3).normalize();
        let hem = glove.mul_transform(glove_frame(arm.side));
        let wrist = hem.transform_point(if arm.side == watch { WATCH_HEM } else { HEM });
        let axis = Vec2::new(torso.translation.x, torso.translation.z);
        let (shoulder, elbow) = arm_joints(shoulder, wrist, hint, axis, scale);
        let show = if *glove_vis == Visibility::Hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        // The sleeves only with the body drawn (`Comfort::body`): an arm the
        // IK can only guess at is worse than none. Their shadow stays.
        let sleeves = if body { show } else { Visibility::Hidden };
        // A bone from `a` toward `b`: the sleeve's -Z along it, its up kept
        // near the glove's so the folds don't spin.
        let bone = |a: Vec3, b: Vec3| {
            let len = a.distance(b).max(0.01);
            let mut t = Transform::from_translation(a).looking_at(b, hem.rotation * Vec3::Y);
            t.scale = Vec3::new(1.0, 1.0, len);
            t
        };
        for (e, t) in [
            (arm.shadow[0], limb(shoulder, elbow, 0.055)),
            (arm.shadow[1], limb(elbow, wrist, 0.045)),
            (arm.forearm, bone(wrist, elbow)),
            (arm.upper, bone(elbow, shoulder)),
            (arm.elbow, Transform::from_translation(elbow)),
            (arm.shoulder, Transform::from_translation(shoulder)),
        ] {
            if let Ok((mut pt, mut vis)) = parts.get_mut(e) {
                *pt = t;
                *vis = if arm.shadow.contains(&e) {
                    show
                } else {
                    sleeves
                };
            }
        }
    }
}

/// A capsule of the body's shadow proxy, on the layer only the sun sees.
#[derive(Component)]
struct ShadowPart;

/// One unit capsule and one material for every proxy piece, so the shadow
/// pass draws them as one batch.
fn shadow_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    look: &mut Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
) -> Entity {
    let (mesh, mat) = look
        .get_or_insert_with(|| {
            (
                meshes.add(
                    Capsule3d::new(0.5, 1.0)
                        .mesh()
                        .rings(2)
                        .latitudes(8)
                        .longitudes(10),
                ),
                materials.add(StandardMaterial::default()),
            )
        })
        .clone();
    commands
        .spawn((
            ShadowPart,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::default(),
            Visibility::Hidden,
            bevy::camera::visibility::RenderLayers::layer(SHADOW_ONLY),
            bevy::light::NotShadowReceiver,
            // Moved every frame: its bounds are never stale for the cascades.
            bevy::camera::visibility::NoFrustumCulling,
        ))
        .id()
}

/// A capsule from `a` to `b`, `r` round (the unit capsule, radius 0.5 and
/// 2 long, stretched onto it).
fn limb(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(0.01);
    Transform::from_translation((a + b) / 2.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, d / len))
        .with_scale(Vec3::new(2.0 * r, len / 2.0, 2.0 * r))
}

/// The rest of the local player's shadow proxy: head, torso, legs.
#[derive(Component)]
struct BodyShadow {
    player: Entity,
    head: Entity,
    torso: Entity,
    /// Thighs, then shins.
    legs: [Entity; 4],
}

fn grow_body_shadow(
    mut commands: Commands,
    players: Query<Entity, Added<crate::rig::LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut look: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    for player in &players {
        let mut part = || shadow_part(&mut commands, &mut meshes, &mut materials, &mut look);
        let (head, torso) = (part(), part());
        let legs = [part(), part(), part(), part()];
        commands.spawn((
            Name::new("Body shadow"),
            BodyShadow {
                player,
                head,
                torso,
                legs,
            },
        ));
    }
}

/// Head on the eyes, a torso leaning from the neck (which follows the
/// head) down to the hips (which stay with the torso anchor), legs bent at
/// the knees to the planted feet: the stance's (`rig::stance`), as the
/// body's drawn.
fn pose_body_shadow(
    bodies: Query<&BodyShadow>,
    players: Query<
        (
            &Transform,
            &RigInput,
            &Children,
            Option<&crate::rig::stance::Stance>,
        ),
        Without<ShadowPart>,
    >,
    torsos: Query<&Transform, (With<Torso>, Without<ShadowPart>)>,
    mut parts: Query<(&mut Transform, &mut Visibility), With<ShadowPart>>,
) {
    for body in &bodies {
        let Ok((root, input, children, stance)) = players.get(body.player) else {
            continue;
        };
        if let Some(st) = stance.filter(|st| st.ready) {
            let head = root.mul_transform(input.head);
            let hips = st.pelvis.translation;
            let torso_t = Transform::from_translation((st.neck + hips) / 2.0)
                .with_rotation(st.chest.rotation)
                .with_scale(Vec3::new(
                    0.34,
                    (st.neck.distance(hips) / 2.0).max(0.05),
                    0.22,
                ));
            let placed = [
                (
                    body.head,
                    limb(
                        head.translation - Vec3::Y * 0.04,
                        head.translation + Vec3::Y * 0.06,
                        0.1,
                    ),
                ),
                (body.torso, torso_t),
                (body.legs[0], limb(st.hips[0], st.knees[0], 0.075)),
                (body.legs[1], limb(st.hips[1], st.knees[1], 0.075)),
                (body.legs[2], limb(st.knees[0], st.ankles[0], 0.055)),
                (body.legs[3], limb(st.knees[1], st.ankles[1], 0.055)),
            ];
            for (e, t) in placed {
                if let Ok((mut pt, mut vis)) = parts.get_mut(e) {
                    *pt = t;
                    *vis = Visibility::Inherited;
                }
            }
            continue;
        }
        let Some(torso) = children.iter().find_map(|c| torsos.get(c).ok()) else {
            continue;
        };
        let torso = root.mul_transform(*torso);
        let head = root.mul_transform(input.head);
        let fwd = head.rotation * Vec3::NEG_Z;
        let flat = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or(torso.rotation * Vec3::NEG_Z);
        let neck = head.translation - Vec3::Y * 0.2 - flat * 0.06;
        let hips = torso.translation - Vec3::Y * 0.45;
        let side = torso.rotation * Vec3::X;
        let feet = root.translation + Vec3::Y * 0.08;
        let up = (neck - hips).normalize_or(Vec3::Y);
        let torso_t = Transform::from_translation((neck + hips) / 2.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up) * torso.rotation)
            .with_scale(Vec3::new(0.34, (neck.distance(hips) / 2.0).max(0.05), 0.22));
        for (e, t) in [
            (
                body.head,
                limb(
                    head.translation - Vec3::Y * 0.04,
                    head.translation + Vec3::Y * 0.06,
                    0.1,
                ),
            ),
            (body.torso, torso_t),
            (
                body.legs[0],
                limb(hips - side * 0.1, feet - side * 0.12, 0.075),
            ),
            (
                body.legs[1],
                limb(hips + side * 0.1, feet + side * 0.12, 0.075),
            ),
        ] {
            if let Ok((mut pt, mut vis)) = parts.get_mut(e) {
                *pt = t;
                *vis = Visibility::Inherited;
            }
        }
    }
}

/// A piece of a sleeve.
#[derive(Component)]
struct ArmPart;

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: Vec2 = Vec2::ZERO;

    /// In reach, the bones keep their lengths, the elbow bends the way it's
    /// told (down and out), and it stays out of the body.
    #[test]
    fn the_elbow_bends_down_and_out_and_keeps_out_of_the_body() {
        let shoulder = Vec3::new(0.17, 1.42, 0.03);
        let hint = Vec3::new(0.6, -1.0, 0.3).normalize();
        for wrist in [
            Vec3::new(0.15, 1.1, -0.35),  // gun hand, low and ahead
            Vec3::new(0.0, 1.3, -0.3),    // across in front
            Vec3::new(0.05, 1.45, -0.25), // the left wrist up to read (mirrored)
            Vec3::new(0.25, 1.0, -0.1),   // hanging by the hip
        ] {
            let (s, e) = arm_joints(shoulder, wrist, hint, BODY, 1.0);
            assert_eq!(s, shoulder, "{wrist}: in reach, the shoulder stays");
            assert!(
                (e.distance(s) - UPPER).abs() < 0.03
                    || Vec2::new(e.x, e.z).length() >= BODY_R - 1e-4,
                "{wrist}: upper arm {}",
                e.distance(s)
            );
            let mid = (s + wrist) / 2.0;
            assert!(
                (e - mid).dot(hint) > 0.0,
                "{wrist}: elbow {e} not down and out"
            );
            assert!(
                Vec2::new(e.x, e.z).length() >= BODY_R - 1e-4,
                "{wrist}: elbow {e} in the body"
            );
        }
    }

    /// Just out of reach the arm straightens a little and the shoulder
    /// stays; far out, the shoulder is pulled after the hand, the arm
    /// straight and stretched no further. A longer arm (`scale`) reaches
    /// without either.
    #[test]
    fn out_of_reach_the_arm_runs_straight() {
        let shoulder = Vec3::new(0.17, 1.42, 0.03);
        let dir = Vec3::new(0.1, -0.1, -1.0).normalize();
        let just = shoulder + dir * (UPPER + FOREARM) * 1.03;
        let (s, e) = arm_joints(shoulder, just, Vec3::NEG_Y, BODY, 1.0);
        assert_eq!(s, shoulder, "just out of reach, the shoulder stays");
        assert!((e.distance(s) + e.distance(just) - s.distance(just)).abs() < 0.01);
        let wrist = shoulder + dir * 0.9;
        let (s, e) = arm_joints(shoulder, wrist, Vec3::NEG_Y, BODY, 1.0);
        let most = (UPPER + FOREARM) * STRETCH;
        assert!(s.distance(wrist) <= most + 1e-3);
        assert!(
            e.distance(wrist) <= FOREARM * STRETCH + 0.01,
            "forearm {}",
            e.distance(wrist)
        );
        assert!(
            e.distance(s) <= UPPER * STRETCH + 0.01,
            "upper {}",
            e.distance(s)
        );
        let (s, e) = arm_joints(shoulder, just, Vec3::NEG_Y, BODY, 1.1);
        assert_eq!(s, shoulder);
        assert!(
            (e.distance(s) - UPPER * 1.1).abs() < 0.01,
            "a long arm bends"
        );
    }
}
