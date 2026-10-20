//! How the heavy looks (client only): dressed from
//! `assets/models/robots/heavy.glb` (`art/robots/heavy.py`), each piece in
//! three LODs. Hull and brow ride the body; legs and arm are posed from the
//! IK every frame; bolts sit on their parts and go with them. Its plates
//! and cell are items, seated on the hull, and dress themselves.
//!
//! Its weak points are lit cyan in the house style (knees, vents). The
//! vents run toward orange as it heats, so a hot heavy reads as one before
//! it stalls; a lost vent goes dark, and so does all of it when it falls.

use bevy::camera::visibility::VisibilityRange;
use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use extraction_core::heavy::{STALL_AT, Slot};

use super::heavy::{Arm, Heavy, HeavyPart, LegPiece, SHOULDER, SIZE, ThrownDoor, arm_elbow};
use super::walker::{LOWER, LegPoses, UPPER};
use super::{Robot, RobotPart};
use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes};
use crate::render::materials::MaterialLibrary;

const MODEL: &str = "models/robots/heavy.glb";
/// Where each LOD hands over to the next, m (to the mesh's bounds).
pub const LOD_AT: [f32; 2] = [25.0, 70.0];

pub struct HeavyLookPlugin;

impl Plugin for HeavyLookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                dress_heavies,
                pose_limbs,
                light_heavies,
                dress_doors,
                gallery,
                felt_footfalls,
            ),
        );
    }
}

/// A heavy not yet dressed (its model loading).
#[derive(Component)]
struct Undressed;

/// A posed piece of a heavy that isn't a part: a leg segment, an arm
/// segment.
#[derive(Component, Clone, Copy)]
struct Limb {
    robot: Entity,
    kind: LimbKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LimbKind {
    Thigh(usize),
    Shin(usize),
    ArmUpper,
    ArmFore,
}

/// The heavy's own lights: the vents' glow (one each), its strips and eye.
#[derive(Component)]
struct Lights {
    vents: [Handle<StandardMaterial>; 2],
    strips: Handle<StandardMaterial>,
    eye: Handle<StandardMaterial>,
}

/// A mesh lit by one of a heavy's lights.
#[derive(Component, Clone, Copy)]
enum Lit {
    Vent(usize),
    Strip,
    Eye,
}

fn lod_range(lod: usize) -> Option<VisibilityRange> {
    let fade = |d: f32| (d - 2.0)..(d + 2.0);
    match lod {
        0 => Some(VisibilityRange {
            start_margin: 0.0..0.0,
            end_margin: fade(LOD_AT[0]),
            use_aabb: false,
        }),
        1 => Some(VisibilityRange {
            start_margin: fade(LOD_AT[0]),
            end_margin: fade(LOD_AT[1]),
            use_aabb: false,
        }),
        _ => Some(VisibilityRange {
            start_margin: fade(LOD_AT[1]),
            end_margin: 1.0e4..1.0e4,
            use_aabb: false,
        }),
    }
}

fn lod_name(name: &str, lod: usize) -> String {
    if lod == 0 {
        name.into()
    } else {
        format!("{name}_lod{lod}")
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn dress_heavies(
    mut commands: Commands,
    new: Query<Entity, Added<Heavy>>,
    waiting: Query<(Entity, &Children), (With<Heavy>, With<Undressed>)>,
    parts: Query<(&HeavyPart, Option<&LegPiece>, &Transform)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in &new {
        commands.entity(e).insert(Undressed);
        models.request(MODEL, &server);
    }
    if waiting.is_empty() {
        return;
    }
    let Some(h) = models.request(MODEL, &server) else {
        return;
    };
    let model = match Models::state(&h, &server, &gltfs) {
        ModelState::Ready(g) => model_nodes(g, &nodes, &gltf_meshes),
        ModelState::Loading => return,
        ModelState::Failed => {
            warn!("heavy model failed to load");
            for (e, _) in &waiting {
                commands.entity(e).remove::<Undressed>();
            }
            return;
        }
    };
    let glow = model_mats.get("Glow", &mut lib, &server, &mut materials);
    let eye = model_mats.get("Eye", &mut lib, &server, &mut materials);
    for (robot, children) in &waiting {
        commands.entity(robot).remove::<Undressed>();
        let own = |m: &Handle<StandardMaterial>, materials: &mut Assets<StandardMaterial>| {
            let copy = materials.get(m).cloned().unwrap_or_default();
            materials.add(copy)
        };
        let lights = Lights {
            vents: [own(&glow, &mut materials), own(&glow, &mut materials)],
            strips: own(&glow, &mut materials),
            eye: own(&eye, &mut materials),
        };
        let mut mat = |name: Option<&str>, lit: &mut Option<Lit>| -> Handle<StandardMaterial> {
            match (name, *lit) {
                (Some("Glow"), Some(Lit::Vent(v))) => lights.vents[v].clone(),
                (Some("Glow"), _) => {
                    *lit = Some(Lit::Strip);
                    lights.strips.clone()
                }
                (Some("Eye"), _) => {
                    *lit = Some(Lit::Eye);
                    lights.eye.clone()
                }
                (Some("Shell"), _) => {
                    let base = model_mats.get("Shell", &mut lib, &server, &mut materials);
                    // (As the other robots' shells: a faint glow of its own,
                    // so it reads pale against the forest.)
                    model_mats.derived("RobotShell", &mut materials, &base, |m| {
                        m.emissive = LinearRgba::rgb(0.22, 0.22, 0.23);
                    })
                }
                (Some(n), _) => model_mats.get(n, &mut lib, &server, &mut materials),
                (None, _) => model_mats.get("Dark", &mut lib, &server, &mut materials),
            }
        };
        // Hang `name` (all its LODs) on `parent` at `at` (local to it).
        let mut hang = |commands: &mut Commands,
                        parent: Entity,
                        name: &str,
                        at: Transform,
                        lit: Option<Lit>| {
            for lod in 0..3 {
                let Some(node) = ModelNode::find(&model, &lod_name(name, lod)) else {
                    continue;
                };
                let holder = commands
                    .spawn((at, Visibility::default(), ChildOf(parent)))
                    .id();
                for p in &node.primitives {
                    let mut l = lit;
                    let m = mat(p.material.as_deref(), &mut l);
                    let mut ec = commands.spawn((
                        Mesh3d(p.mesh.clone()),
                        MeshMaterial3d(m),
                        ChildOf(holder),
                    ));
                    if let Some(r) = lod_range(lod) {
                        ec.insert(r);
                    }
                    if let Some(l) =
                        l.filter(|_| matches!(p.material.as_deref(), Some("Glow" | "Eye")))
                    {
                        ec.insert(l);
                    }
                }
            }
        };
        // The hull and brow on the body (the model's origin is the robot's).
        hang(&mut commands, robot, "hull", Transform::IDENTITY, None);
        for c in children.iter() {
            let Ok((part, piece, pt)) = parts.get(c) else {
                continue;
            };
            // A part's mesh, its offset taken out (the part sits there).
            let local = |node: Vec3| Transform::from_translation(node - pt.translation);
            match (part.0, piece) {
                (Slot::Eye, _) => hang(&mut commands, c, "eye", local(pt.translation), None),
                (Slot::Vent(v), _) => {
                    // The right's modelled; the left is it turned round.
                    let turn = if v == 0 {
                        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    } else {
                        Transform::IDENTITY
                    };
                    hang(&mut commands, c, "vent", turn, Some(Lit::Vent(v as usize)));
                }
                (Slot::Bolt { plate, .. }, _) => {
                    let rot = super::heavy::plate_transform(plate as usize).rotation;
                    hang(
                        &mut commands,
                        c,
                        "bolt",
                        Transform::from_rotation(rot),
                        None,
                    );
                }
                (Slot::Knee(_), Some(LegPiece::Knee(_))) => {
                    hang(&mut commands, c, "knee", Transform::IDENTITY, None);
                }
                _ => {}
            }
        }
        // Legs and arm, posed every frame (`pose_limbs`).
        let limbs = (0..4)
            .flat_map(|i| [LimbKind::Thigh(i), LimbKind::Shin(i)])
            .chain([LimbKind::ArmUpper, LimbKind::ArmFore]);
        for kind in limbs {
            let name = match kind {
                LimbKind::Thigh(_) => "thigh",
                LimbKind::Shin(_) => "shin",
                LimbKind::ArmUpper => "arm_upper",
                LimbKind::ArmFore => "arm_fore",
            };
            let limb = commands
                .spawn((
                    Name::new(format!("Heavy {name}")),
                    Limb { robot, kind },
                    Transform::default(),
                    Visibility::default(),
                ))
                .id();
            hang(&mut commands, limb, name, Transform::IDENTITY, None);
        }
        commands.entity(robot).insert(lights);
    }
}

/// A segment from `a` to `b` bending about `hinge`: Y down it, X along the
/// hinge, at the body's scale (`nominal`, the model's length, stretched
/// to the joints).
fn segment(a: Vec3, b: Vec3, hinge: Vec3, nominal: f32) -> Transform {
    let len = a.distance(b).max(0.01);
    let dir = (b - a) / len;
    let x = (hinge - dir * hinge.dot(dir)).normalize_or(dir.any_orthonormal_vector());
    let z = x.cross(dir);
    Transform::from_translation(a)
        .with_rotation(Quat::from_mat3(&Mat3::from_cols(x, dir, z)))
        .with_scale(Vec3::new(SIZE, len / nominal, SIZE))
}

fn pose_limbs(
    mut commands: Commands,
    mut limbs: Query<(Entity, &Limb, &mut Transform)>,
    heavies: Query<(&LegPoses, &Arm, &GlobalTransform)>,
) {
    for (e, limb, mut t) in &mut limbs {
        let Ok((poses, arm, gt)) = heavies.get(limb.robot) else {
            commands.entity(e).despawn();
            continue;
        };
        *t = match limb.kind {
            LimbKind::Thigh(i) | LimbKind::Shin(i) => {
                let [hip, knee, foot] = poses.0[i];
                let mut hinge = (knee - hip).cross(foot - knee);
                if hinge.length_squared() < 1e-8 {
                    hinge = gt.right().as_vec3();
                }
                if matches!(limb.kind, LimbKind::Thigh(_)) {
                    segment(hip, knee, hinge, UPPER)
                } else {
                    segment(knee, foot, hinge, LOWER)
                }
            }
            LimbKind::ArmUpper | LimbKind::ArmFore => {
                let body = gt.compute_transform();
                let shoulder = body.transform_point(SHOULDER);
                let elbow = arm_elbow(&body, shoulder, arm.claw);
                let hinge = (elbow - shoulder).cross(arm.claw - elbow);
                let hinge = if hinge.length_squared() < 1e-8 {
                    gt.right().as_vec3()
                } else {
                    hinge
                };
                if limb.kind == LimbKind::ArmUpper {
                    segment(shoulder, elbow, hinge, UPPER)
                } else {
                    segment(elbow, arm.claw, hinge, LOWER)
                }
            }
        };
    }
}

/// The vents and the strips down its back glow cyan cold and run to
/// orange as it heats (flaring while it stalls); a lost vent goes dark; its eye goes out when it's blinded;
/// everything goes out when it falls.
fn light_heavies(
    heavies: Query<(&Heavy, &Robot, &Lights, &Children)>,
    parts: Query<(&HeavyPart, &RobotPart)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) {
    let cold = LinearRgba::rgb(0.2 * 6.0, 0.8 * 6.0, 1.0 * 6.0);
    let hot = LinearRgba::rgb(8.0, 1.4, 0.12);
    let dark = LinearRgba::BLACK;
    for (h, robot, lights, children) in &heavies {
        let mut vent_alive = [true; 2];
        let mut eye_alive = true;
        for c in children.iter() {
            if let Ok((hp, part)) = parts.get(c) {
                match hp.0 {
                    Slot::Vent(v) => vent_alive[v as usize] = part.alive(),
                    Slot::Eye => eye_alive = part.alive(),
                    _ => {}
                }
            }
        }
        let k = (h.heat / STALL_AT).clamp(0.0, 1.0);
        let flare = if h.stall > 0.0 {
            1.0 + 0.5 * (time.elapsed_secs() * 9.0).sin()
        } else {
            1.0
        };
        let warm = LinearRgba::rgb(
            cold.red + (hot.red - cold.red) * k,
            cold.green + (hot.green - cold.green) * k,
            cold.blue + (hot.blue - cold.blue) * k,
        ) * flare;
        for (v, handle) in lights.vents.iter().enumerate() {
            let want = if robot.wrecked || !vent_alive[v] {
                dark
            } else {
                warm
            };
            if let Some(mut m) = materials.get_mut(handle) {
                if m.emissive != want {
                    m.emissive = want;
                }
            }
        }
        if let Some(mut m) = materials.get_mut(&lights.strips) {
            // (The strips warm with it too: its vents are under plates.)
            let want = if robot.wrecked { dark } else { warm };
            if m.emissive != want {
                m.emissive = want;
            }
        }
        if let Some(mut m) = materials.get_mut(&lights.eye) {
            let want = if robot.wrecked || !eye_alive {
                dark
            } else {
                LinearRgba::rgb(9.0, 1.2, 0.6)
            };
            if m.emissive != want {
                m.emissive = want;
            }
        }
    }
}

/// A door it tore off: a slab of planks.
fn dress_doors(
    mut commands: Commands,
    doors: Query<(Entity, &ThrownDoor), Added<ThrownDoor>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, d) in &doors {
        let mat = lib.get(
            crate::render::materials::Mat::Planks,
            &server,
            &mut materials,
        );
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(d.half.x * 2.0, d.half.y * 2.0, d.half.z * 2.0))),
            MeshMaterial3d(mat),
            ChildOf(e),
        ));
    }
}

/// `--gallery heavy[:open|:hot|:down][,YAW[,DIST]]`: a heavy standing in
/// front of the `--camera`, still (its senses off). `open` has its left
/// plate and hatch off and a knee shot out; `hot` runs it near a stall;
/// `down` is its wreck.
///
/// ```text
/// scripts/shot.sh docs/shots/heavy/front.png 40,0,262,0,0 60 --gallery heavy,200
/// ```
#[allow(clippy::too_many_arguments)]
fn gallery(
    mut commands: Commands,
    opts: Res<crate::launch::LaunchOptions>,
    zone: Option<Res<crate::world::Zone>>,
    catalog: Res<crate::items::Catalog>,
    mut stage: Local<u32>,
    mut made: Local<Option<(Entity, String)>>,
    mut heavies: Query<(&mut Heavy, &mut Robot, &Children)>,
    parts: Query<(Entity, &HeavyPart)>,
    mut hits: MessageWriter<crate::weapons::ballistics::Hit>,
) {
    let (Some(spec), Some((cam, yaw, _)), Some(zone)) = (&opts.gallery, opts.camera, zone) else {
        return;
    };
    let mut fields = spec.split(',');
    let first = fields.next().unwrap_or("");
    let Some(state) = first.strip_prefix("heavy") else {
        return;
    };
    *stage += 1;
    if *stage == 3 {
        let turn: f32 = fields.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let dist: f32 = fields.next().and_then(|v| v.parse().ok()).unwrap_or(9.0);
        let fwd = Quat::from_rotation_y(-yaw.to_radians()) * Vec3::NEG_Z;
        let at = zone.on_ground(cam.x + fwd.x * dist, cam.z + fwd.z * dist);
        let e = super::heavy::spawn_heavy(
            &mut commands,
            &zone,
            &catalog,
            at,
            super::Job {
                kind: super::JobKind::Survey,
                site: at,
                duration: 1.0e6,
                exit: at,
            },
        );
        // Still: no senses, nothing to do but stand.
        commands
            .entity(e)
            .remove::<super::Senses>()
            .insert(super::Brain::Working { at, left: 1.0e6 });
        // Its yaw: facing the camera, turned by `turn`.
        let face = (-fwd).with_y(0.0);
        let yaw = (-face.x).atan2(-face.z) + turn.to_radians();
        commands.queue(move |w: &mut World| {
            if let Some(mut wk) = w.get_mut::<super::walker::Walker>(e) {
                wk.yaw = yaw;
            }
        });
        *made = Some((e, state.trim_start_matches(':').to_owned()));
    }
    let Some((e, state)) = made.as_ref() else {
        return;
    };
    let Ok((mut h, mut robot, children)) = heavies.get_mut(*e) else {
        return;
    };
    match state.as_str() {
        "hot" => h.heat = 0.9,
        "down" if *stage == 30 => robot.wrecked = true,
        "open" if *stage == 30 => {
            for c in children.iter() {
                let Ok((pe, part)) = parts.get(c) else {
                    continue;
                };
                let kill = matches!(part.0, Slot::Bolt { plate: 0 | 1, .. } | Slot::Knee(1));
                if kill {
                    hits.write(crate::weapons::ballistics::Hit {
                        entity: pe,
                        point: Vec3::ZERO,
                        normal: Vec3::Y,
                        velocity: Vec3::ZERO,
                        damage: 1000.0,
                        shooter: None,
                        mass_g: 0.0,
                        load: Default::default(),
                    });
                }
            }
        }
        _ => {}
    }
}

/// Within this of a heavy's foot coming down, you feel it in both hands:
/// a knock close (the house scale's "that took force"), fading to a tick.
pub const FELT_WITHIN: f32 = 25.0;

/// How hard a footfall `d` metres off is felt, if at all.
pub fn footfall_pulse(d: f32) -> Option<(f32, f32)> {
    (d < FELT_WITHIN).then(|| {
        let k = 1.0 - (d / FELT_WITHIN).clamp(0.0, 1.0);
        (0.12 + 0.36 * k * k, 0.04 + 0.06 * k)
    })
}

/// Its footfalls and its blows, in your hands.
fn felt_footfalls(
    mut events: MessageReader<super::heavy::HeavyEvent>,
    players: Query<&GlobalTransform, With<crate::rig::LocalPlayer>>,
    mut out: MessageWriter<crate::rig::haptics::Haptic>,
) {
    let Some(me) = players.iter().next().map(|g| g.translation()) else {
        return;
    };
    for ev in events.read() {
        let (super::heavy::HeavyAct::Step | super::heavy::HeavyAct::Slam { .. }) = ev.what else {
            continue;
        };
        let Some((amplitude, seconds)) = footfall_pulse(ev.at.distance(me)) else {
            continue;
        };
        for side in [crate::rig::Side::Left, crate::rig::Side::Right] {
            out.write(crate::rig::haptics::Haptic {
                side,
                amplitude,
                seconds,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A footfall is felt on the house scale: never a kick (that's shots
    /// and hits), a knock close by, a tick at the edge, nothing past it.
    #[test]
    fn footfalls_are_knocks_not_kicks() {
        let (close, _) = footfall_pulse(1.0).unwrap();
        assert!((0.4..0.6).contains(&close), "{close}");
        let (far, secs) = footfall_pulse(FELT_WITHIN - 0.5).unwrap();
        assert!((0.1..0.15).contains(&far), "{far}");
        assert!(secs >= 0.01);
        assert!(footfall_pulse(FELT_WITHIN + 1.0).is_none());
    }
}
