//! Client: Lev on foot, jointed. Kit models' `lev.glb` is one node per
//! limb, each origin on its joint (rest pose, feet at the origin, facing
//! -Z). Here they are hung from each other (pelvis, spine, chest, head;
//! arms from the chest, legs from the pelvis) and turned each frame from
//! a gait: a slow man's walk, stiff in the knee, the arms swinging a
//! little. His right arm stops at the elbow in the model; Hands' `lev_hand`
//! is his forearm and hand from there. His torch hand carries a light.
//!
//! Without the file he stays `lev_body` (or boxes): see `dress_lev`.

use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use std::f32::consts::TAU;

use super::epilogue::LevWalker;
use crate::models::{ModelNode, ModelState, Models, model_nodes};
use crate::render::world_props::{Blockout, PropModels, Resolved, path};

/// The rigged model, under `models/props/`.
pub const RIG: &str = "lev";

/// Each joint, its parent and (for one with no node of its own) where it
/// sits. `forearm_r` is not in the model: it is the elbow `lev_hand`
/// hangs from.
const JOINTS: [(&str, Option<&str>); 15] = [
    ("pelvis", None),
    ("spine", Some("pelvis")),
    ("chest", Some("spine")),
    ("head", Some("chest")),
    ("upper_arm_l", Some("chest")),
    ("forearm_l", Some("upper_arm_l")),
    ("hand_l", Some("forearm_l")),
    ("upper_arm_r", Some("chest")),
    ("forearm_r", Some("upper_arm_r")),
    ("thigh_l", Some("pelvis")),
    ("shin_l", Some("thigh_l")),
    ("foot_l", Some("shin_l")),
    ("thigh_r", Some("pelvis")),
    ("shin_r", Some("thigh_r")),
    ("foot_r", Some("shin_r")),
];

/// The right elbow (the left forearm's origin, mirrored).
const ELBOW_R: Vec3 = Vec3::new(0.215, 1.13, 0.0);
/// `lev_hand` from the elbow: 0.304 m down the forearm, palm to his side.
const HAND_FROM_ELBOW: f32 = 0.304;

/// Where his levels of detail change, metres from the eye.
const LOD1: f32 = 9.0;
const LOD2: f32 = 24.0;

/// One gait cycle (two steps), metres.
const CYCLE: f32 = 1.4;
/// Hip swing, knee bend in the swing, arm swing (radians).
const HIP: f32 = 0.34;
const KNEE: f32 = 0.42;
const ARM: f32 = 0.26;

/// Waiting for the model, on a Lev that has it on disk.
#[derive(Component)]
pub struct PendingRig;

/// His joints, by name, and the gait.
#[derive(Component, Default)]
pub struct LevRig {
    joints: Vec<(&'static str, Entity, Transform)>,
    last: Option<Vec3>,
    phase: f32,
    amp: f32,
}

/// Whether the rig is there to be loaded.
pub fn on_disk() -> bool {
    crate::asset_root_path().join(path(RIG)).exists()
}

/// The draw range of a node named `<x>`, `<x>_lod1` or `<x>_lod2`.
fn lod_range(name: &str) -> Option<VisibilityRange> {
    let fade = |d: f32| (d - 0.5)..(d + 0.5);
    let far = 1.0e4..1.0e4;
    Some(if name.ends_with("_lod2") {
        VisibilityRange {
            start_margin: fade(LOD2),
            end_margin: far,
            use_aabb: false,
        }
    } else if name.ends_with("_lod1") {
        VisibilityRange {
            start_margin: fade(LOD1),
            end_margin: fade(LOD2),
            use_aabb: false,
        }
    } else {
        VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: fade(LOD1),
            use_aabb: false,
        }
    })
}

fn base_name(name: &str) -> &str {
    name.trim_end_matches("_lod1").trim_end_matches("_lod2")
}

/// Builds the rig once `lev.glb` (and `lev_hand`) are in; boxes until then.
pub fn rig_lev(
    mut commands: Commands,
    pending: Query<(Entity, Option<&Children>), With<PendingRig>>,
    blockouts: Query<(), With<Blockout>>,
    mut props: PropModels,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, children) in &pending {
        let hand = props.resolve("lev_hand");
        let Some(h) = props.models.request(&path(RIG), &props.server) else {
            commands.entity(e).remove::<PendingRig>();
            continue;
        };
        let nodes: Vec<ModelNode> = match Models::state(&h, &props.server, &props.gltfs) {
            ModelState::Loading => continue,
            ModelState::Failed => {
                warn!("Lev's rig failed to load; keeping his boxes");
                commands.entity(e).remove::<PendingRig>();
                continue;
            }
            ModelState::Ready(g) => model_nodes(g, &props.nodes, &props.gltf_meshes),
        };
        if matches!(hand, Resolved::Loading) {
            continue;
        }
        commands.entity(e).remove::<PendingRig>();
        for c in children.into_iter().flatten() {
            if blockouts.contains(*c) {
                commands.entity(*c).despawn();
            }
        }
        // Each joint's origin in his space: its node's, or the elbow.
        let origin = |name: &str| -> Vec3 {
            if name == "forearm_r" {
                return ELBOW_R;
            }
            ModelNode::find(&nodes, name)
                .map(|n| n.transform.translation)
                .unwrap_or(Vec3::new(0.0, 0.95, 0.0))
        };
        let mut rig = LevRig::default();
        let mut by_name: Vec<(&str, Entity)> = Vec::new();
        for (name, parent) in JOINTS {
            let at = origin(name);
            let (parent_e, local) = match parent {
                Some(p) => (
                    by_name.iter().find(|(n, _)| *n == p).map(|(_, e)| *e),
                    at - origin(p),
                ),
                None => (None, at),
            };
            let rest = Transform::from_translation(local);
            let j = commands
                .spawn((
                    Name::new(name),
                    rest,
                    Visibility::default(),
                    ChildOf(parent_e.unwrap_or(e)),
                ))
                .id();
            by_name.push((name, j));
            rig.joints.push((name, j, rest));
        }
        let joint = |name: &str| by_name.iter().find(|(n, _)| *n == name).map(|(_, e)| *e);
        // The meshes, each on its joint (the bag rides the pelvis), every
        // level of detail drawn in its own range.
        for node in &nodes {
            let base = base_name(&node.name);
            let on = match base {
                "bag" => joint("pelvis"),
                b => joint(b),
            };
            let Some(on) = on else { continue };
            // In the joint's space: the bag is offset from the pelvis.
            let offset =
                node.transform.translation - origin(if base == "bag" { "pelvis" } else { base });
            let local = Transform {
                translation: offset,
                ..node.transform
            };
            for p in &node.primitives {
                let mat = props.material(p.material.as_deref().unwrap_or("Cloth"), &mut materials);
                let mut s = commands.spawn((
                    Mesh3d(p.mesh.clone()),
                    MeshMaterial3d(mat),
                    local,
                    ChildOf(on),
                ));
                if let Some(r) = lod_range(&node.name) {
                    s.insert(r);
                }
            }
        }
        // His torch: a light at the lens, pointing ahead and a little down.
        if let (Some(hand_l), Some(lens)) = (joint("hand_l"), ModelNode::find(&nodes, "pt_torch")) {
            let at = lens.transform.translation - origin("hand_l");
            commands.spawn((
                SpotLight {
                    color: Color::srgb(1.0, 0.92, 0.8),
                    intensity: 60_000.0,
                    range: 16.0,
                    inner_angle: 0.25,
                    outer_angle: 0.5,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(at).looking_to(Vec3::new(0.0, -0.3, -1.0), Vec3::Y),
                ChildOf(hand_l),
            ));
        }
        // His right forearm and hand.
        if let (Resolved::Ready, Some(fore)) = (hand, joint("forearm_r")) {
            let at = Transform::from_xyz(0.005, -HAND_FROM_ELBOW, 0.0).with_rotation(
                Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
                    * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            );
            for (mesh, t, mat) in props.primitives("lev_hand") {
                let material = props.material(&mat, &mut materials);
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    at * t,
                    ChildOf(fore),
                ));
            }
        }
        commands.entity(e).insert(rig);
    }
}

/// The pose at gait `phase` with swing `amp` (0 standing, 1 walking), as
/// each joint's turn from its rest and the pelvis's drop.
pub fn pose(name: &str, phase: f32, amp: f32) -> (Quat, f32) {
    let (s, c) = phase.sin_cos();
    // The left leg leads on the rising half; the right is half a cycle on.
    let side = |left: bool| if left { (s, c) } else { (-s, -c) };
    let leg = |left: bool| {
        let (s, c) = side(left);
        let hip = HIP * amp * s;
        // The knee bends as the leg swings through, and little otherwise.
        let knee = KNEE * amp * c.max(0.0) + 0.05 * amp;
        (hip, knee)
    };
    let rx = Quat::from_rotation_x;
    let q = match name {
        "thigh_l" => rx(leg(true).0),
        "thigh_r" => rx(leg(false).0),
        "shin_l" => rx(-leg(true).1),
        "shin_r" => rx(-leg(false).1),
        // The foot kept near flat.
        "foot_l" => {
            let (h, k) = leg(true);
            rx(0.7 * (k - h))
        }
        "foot_r" => {
            let (h, k) = leg(false);
            rx(0.7 * (k - h))
        }
        // Arms against the legs; the torch arm held a little forward.
        "upper_arm_l" => rx(-ARM * amp * s + 0.1 * amp),
        "upper_arm_r" => rx(ARM * amp * s),
        "forearm_l" => rx(0.35 * amp),
        "forearm_r" => rx(0.15 * amp + 0.08 * amp * s),
        "spine" => rx(0.05 * amp),
        "head" => rx(-0.04 * amp),
        _ => Quat::IDENTITY,
    };
    // Lowest with the legs apart.
    let drop = if name == "pelvis" {
        0.025 * amp * s * s
    } else {
        0.0
    };
    (q, drop)
}

/// Turns his joints from how far he has come.
pub fn walk_lev(
    time: Res<Time>,
    mut lev: Query<(&GlobalTransform, &mut LevRig), With<LevWalker>>,
    mut joints: Query<&mut Transform, Without<LevWalker>>,
) {
    let dt = time.delta_secs();
    for (t, mut rig) in &mut lev {
        let at = t.translation();
        let moved = rig.last.map_or(0.0, |l| (at - l).with_y(0.0).length());
        rig.last = Some(at);
        if dt <= 0.0 {
            continue;
        }
        let speed = (moved / dt).min(3.0);
        let target = (speed / super::epilogue::PACE).clamp(0.0, 1.0);
        // Eases in and out of the walk over about a third of a second.
        rig.amp += (target - rig.amp) * (dt * 6.0).min(1.0);
        rig.phase = (rig.phase + moved / CYCLE * TAU) % TAU;
        let (phase, amp) = (rig.phase, rig.amp);
        for (name, e, rest) in &rig.joints {
            let Ok(mut jt) = joints.get_mut(*e) else {
                continue;
            };
            let (q, drop) = pose(name, phase, amp);
            jt.translation = rest.translation - Vec3::Y * drop;
            jt.rotation = rest.rotation * q;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_he_is_at_rest() {
        for (name, _) in JOINTS {
            for phase in [0.0, 1.0, 2.5, 4.0] {
                let (q, drop) = pose(name, phase, 0.0);
                assert!(q.angle_between(Quat::IDENTITY) < 1e-5, "{name}");
                assert_eq!(drop, 0.0);
            }
        }
    }

    #[test]
    fn walking_his_legs_and_arms_go_opposite() {
        let a = std::f32::consts::FRAC_PI_2;
        let x = |name: &str| pose(name, a, 1.0).0.to_euler(EulerRot::XYZ).0;
        // Left leg forward, right back; the left arm back, the right forward.
        assert!(x("thigh_l") > 0.2 && x("thigh_r") < -0.2);
        assert!(x("upper_arm_r") > 0.1 && x("upper_arm_l") < 0.0);
        // Knees only ever bend one way.
        for p in 0..32 {
            let ph = p as f32 / 32.0 * TAU;
            for k in ["shin_l", "shin_r"] {
                assert!(pose(k, ph, 1.0).0.to_euler(EulerRot::XYZ).0 <= 0.0);
            }
        }
    }
}
