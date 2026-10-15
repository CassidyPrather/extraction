//! Visuals for the truck, the hands, the backpack and body rig, remains.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use crate::interact::body::{Backpack, BodySlot};
use crate::interact::{Grabbable, GripRole, Hand, Held};
use crate::models::{ModelMaterials, ModelState, Models, model_nodes, spawn_primitives};
use crate::raid::Remains;
use crate::raid::truck::{Pressable, Stash, TRUCK_PARTS, Truck, WHEELS};
use crate::render::materials::MaterialLibrary;
use crate::rig::{LocalPlayer, RigHand, RigInput, Side};

pub struct PropVisualsPlugin;

impl Plugin for PropVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (dress_truck, dress_containers, finish_models).chain(),
                dress_slots,
                spawn_hand_visuals,
            ),
        )
        .add_systems(PostUpdate, pose_hands.after(TransformSystems::Propagate));
    }
}

fn mat(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    rough: f32,
    metal: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        metallic: metal,
        ..default()
    })
}

/// The truck and stash crate models (see `art/vehicles/`); without them,
/// boxes.
const TRUCK_MODEL: &str = "models/vehicles/truck.glb";
const STASH_MODEL: &str = "models/vehicles/stash_crate.glb";

/// The truck or the stash crate, waiting for its model to load (or fail).
#[derive(Component)]
struct Undressed(&'static str);

fn dress_truck(
    mut commands: Commands,
    trucks: Query<Entity, Added<Truck>>,
    pressables: Query<(Entity, &Pressable), Added<Pressable>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for truck in &trucks {
        models.request(TRUCK_MODEL, &server);
        commands.entity(truck).insert(Undressed(TRUCK_MODEL));
        // A work lamp in the bed so you can see the stash at night.
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.82, 0.55),
                intensity: 25_000.0,
                range: 6.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 2.9, 0.0),
            ChildOf(truck),
        ));
    }
    for (e, p) in &pressables {
        match p {
            Pressable::Door
            | Pressable::BenchSelect
            | Pressable::BenchRun
            | Pressable::Cook
            | Pressable::FoldCover
            | Pressable::Destination
            | Pressable::Setting
            | Pressable::Request
            | Pressable::Terminal => {}
            Pressable::BellPull => {
                let rope = mat(&mut materials, Color::srgb(0.55, 0.47, 0.32), 0.9, 0.0);
                commands.entity(e).insert((
                    Mesh3d(meshes.add(Cylinder::new(0.012, 0.5))),
                    MeshMaterial3d(rope.clone()),
                ));
                // A wooden toggle to pull on.
                commands.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.018, 0.07))),
                    MeshMaterial3d(rope),
                    Transform::from_xyz(0.0, -0.26, 0.0)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ChildOf(e),
                ));
            }
        }
    }
}

/// Dresses the truck and the stash from their models once loaded: every
/// node in place, and the truck's one modelled wheel copied to each wheel
/// station (turned round on the left). The old boxes if there's no model.
fn finish_models(
    mut commands: Commands,
    waiting: Query<(Entity, &Undressed)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (truck, u) in &waiting {
        let model = match models.request(u.0, &server) {
            None => None,
            Some(h) => match Models::state(&h, &server, &gltfs) {
                ModelState::Loading => continue,
                ModelState::Failed => {
                    warn!("{} failed to load; using boxes", u.0);
                    None
                }
                ModelState::Ready(g) => Some(model_nodes(g, &nodes, &gltf_meshes)),
            },
        };
        commands.entity(truck).remove::<Undressed>();
        let Some(model) = model else {
            if u.0 == TRUCK_MODEL {
                truck_blockout(&mut commands, truck, &mut meshes, &mut materials);
            } else {
                stash_blockout(&mut commands, truck, &mut meshes, &mut materials);
            }
            continue;
        };
        let mut mat = |name: Option<&str>| {
            model_mats.get(name.unwrap_or("Paint"), &mut lib, &server, &mut materials)
        };
        for node in &model {
            let at: Vec<Transform> = if node.name == "wheel" {
                WHEELS
                    .iter()
                    .map(|w| {
                        let turn = if w.x < 0.0 {
                            Quat::from_rotation_y(std::f32::consts::PI)
                        } else {
                            Quat::IDENTITY
                        };
                        Transform::from_translation(*w)
                            .with_rotation(turn * node.transform.rotation)
                    })
                    .collect()
            } else {
                vec![node.transform]
            };
            for t in at {
                let holder = commands
                    .spawn((t, Visibility::default(), ChildOf(truck)))
                    .id();
                spawn_primitives(&mut commands, holder, node, &mut mat);
            }
        }
    }
}

/// The truck as boxes, for when there is no model.
fn truck_blockout(
    commands: &mut Commands,
    truck: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let paint = mat(materials, Color::srgb(0.22, 0.26, 0.17), 0.65, 0.2);
    let dark = mat(materials, Color::srgb(0.06, 0.06, 0.06), 0.7, 0.4);
    let planks = mat(materials, Color::srgb(0.32, 0.25, 0.17), 0.85, 0.0);
    let canvas = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.31, 0.21),
        perceptual_roughness: 0.95,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgba(0.1, 0.12, 0.13, 0.7),
        perceptual_roughness: 0.05,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let lamp = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.85, 0.7),
        emissive: LinearRgba::rgb(3.0, 2.6, 1.8),
        ..default()
    });
    let slots = [paint.clone(), dark.clone(), planks, canvas];
    for (size, centre, slot) in TRUCK_PARTS {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(*size))),
            MeshMaterial3d(slots[*slot as usize].clone()),
            Transform::from_translation(*centre),
            ChildOf(truck),
        ));
    }
    let wheel = meshes.add(
        Cylinder::new(0.55, 0.38)
            .mesh()
            .resolution(24)
            .build()
            .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
    );
    for w in WHEELS {
        commands.spawn((
            Mesh3d(wheel.clone()),
            MeshMaterial3d(dark.clone()),
            Transform::from_translation(*w),
            ChildOf(truck),
        ));
    }
    // Windscreen and headlamps.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.9, 0.6, 0.04))),
        MeshMaterial3d(glass),
        Transform::from_xyz(0.0, 2.25, -3.92).with_rotation(Quat::from_rotation_x(-0.15)),
        ChildOf(truck),
    ));
    for x in [-0.75, 0.75] {
        commands.spawn((
            Mesh3d(
                meshes.add(
                    Cylinder::new(0.11, 0.08)
                        .mesh()
                        .build()
                        .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ),
            ),
            MeshMaterial3d(lamp.clone()),
            Transform::from_xyz(x, 1.45, -4.97),
            ChildOf(truck),
        ));
    }
}

/// The stash as boxes: an open-topped crate, floor and four walls.
fn stash_blockout(
    commands: &mut Commands,
    e: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let wood = mat(materials, Color::srgb(0.36, 0.28, 0.18), 0.8, 0.0);
    let (w, h, d) = (0.95, 0.65, 1.25);
    for (size, at) in [
        (Vec3::new(w, 0.03, d), Vec3::new(0.0, -h * 0.5, 0.0)),
        (Vec3::new(0.03, h, d), Vec3::new(-w * 0.5, 0.0, 0.0)),
        (Vec3::new(0.03, h, d), Vec3::new(w * 0.5, 0.0, 0.0)),
        (Vec3::new(w, h, 0.03), Vec3::new(0.0, 0.0, -d * 0.5)),
        (Vec3::new(w, h, 0.03), Vec3::new(0.0, 0.0, d * 0.5)),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(wood.clone()),
            Transform::from_translation(at),
            ChildOf(e),
        ));
    }
}

fn dress_containers(
    mut commands: Commands,
    bags: Query<(Entity, Has<Stash>, Has<Remains>), Added<Backpack>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, stash, remains) in &bags {
        if stash {
            models.request(STASH_MODEL, &server);
            commands.entity(e).insert(Undressed(STASH_MODEL));
        } else if remains {
            let bag = mat(&mut materials, Color::srgb(0.12, 0.13, 0.12), 0.6, 0.0);
            commands.entity(e).insert((
                Mesh3d(
                    meshes.add(
                        Capsule3d::new(0.25, 1.2)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ),
                ),
                MeshMaterial3d(bag),
            ));
        } else {
            // The backpack: a canvas pack with a lid flap.
            let canvas = mat(&mut materials, Color::srgb(0.27, 0.29, 0.20), 0.95, 0.0);
            commands.entity(e).insert((
                Mesh3d(meshes.add(Cuboid::new(0.34, 0.48, 0.2))),
                MeshMaterial3d(canvas.clone()),
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.35, 0.04, 0.22))),
                MeshMaterial3d(canvas),
                Transform::from_xyz(0.0, 0.25, 0.01),
                ChildOf(e),
            ));
        }
    }
}

/// Pouches and the holster on the chest rig, so you can see where to reach.
fn dress_slots(
    mut commands: Commands,
    slots: Query<(Entity, &BodySlot), Added<BodySlot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    locals: Query<(), With<LocalPlayer>>,
) {
    let _ = locals;
    for (e, slot) in &slots {
        let size = match slot.0 {
            0 | 1 => continue,
            2 => Vec3::new(0.05, 0.16, 0.06),
            3..=6 => Vec3::new(0.07, 0.11, 0.035),
            _ => Vec3::new(0.06, 0.07, 0.03),
        };
        let m = mat(&mut materials, Color::srgb(0.24, 0.26, 0.19), 0.95, 0.0);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(m),
            Transform::from_xyz(0.0, -0.04, 0.0),
            ChildOf(e),
        ));
    }
}

/// A gloved hand: a palm, four three-jointed fingers and a two-jointed
/// thumb, curling to fit what they hold.
#[derive(Component)]
struct HandVisual {
    hand: Entity,
    side: Side,
    /// Current curl per finger (index, middle, ring, little, thumb), 0..1,
    /// eased toward the target.
    curl: [f32; 5],
}

/// One joint of one finger: `finger` 0..3 index to little, 4 the thumb;
/// `joint` 0 at the knuckle.
#[derive(Component)]
struct Finger {
    finger: usize,
    joint: usize,
}

/// Segment lengths per finger, knuckle outwards, metres.
const FINGER_BONES: [[f32; 3]; 5] = [
    [0.040, 0.025, 0.020],
    [0.044, 0.028, 0.022],
    [0.041, 0.026, 0.020],
    [0.033, 0.020, 0.018],
    [0.036, 0.030, 0.0],
];
/// Full-curl bend per joint, radians (knuckle, middle, tip).
const JOINT_BEND: [f32; 3] = [1.4, 1.65, 1.1];
const THUMB_BEND: [f32; 3] = [0.7, 0.9, 0.0];

fn spawn_hand_visuals(
    mut commands: Commands,
    hands: Query<(Entity, &RigHand, &ChildOf), Added<RigHand>>,
    locals: Query<(), With<LocalPlayer>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, rh, parent) in &hands {
        if !locals.contains(parent.parent()) {
            continue;
        }
        let root = spawn_hand(
            &mut commands,
            &mut meshes,
            &mut materials,
            rh.side,
            [0.0; 5],
        );
        commands.entity(root).insert(HandVisual {
            hand: e,
            side: rh.side,
            curl: [0.0; 5],
        });
    }
}

/// A glove: the palm at the root, joints below it. Without a tracked hand
/// (the gallery's), it keeps the curl it's given.
pub(crate) fn spawn_hand(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    side: Side,
    curl: [f32; 5],
) -> Entity {
    let glove = mat(materials, Color::srgb(0.1, 0.1, 0.09), 0.8, 0.0);
    let palm = meshes.add(Cuboid::new(0.085, 0.03, 0.1));
    let root = commands
        .spawn((
            HandVisual {
                hand: Entity::PLACEHOLDER,
                side,
                curl,
            },
            Mesh3d(palm),
            MeshMaterial3d(glove.clone()),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    // Fingers point along -Z from the palm's front edge and curl toward its
    // underside (-Y). The index is on the thumb's side.
    let thumb_side = -side.sign();
    for finger in 0..5 {
        let at = if finger == 4 {
            Vec3::new(0.038 * thumb_side, -0.008, -0.005)
        } else {
            let x = (0.03 - finger as f32 * 0.02) * thumb_side;
            Vec3::new(x, 0.0, -0.048)
        };
        let mut parent = root;
        let mut offset = at;
        for (joint, len) in FINGER_BONES[finger].iter().enumerate() {
            if *len <= 0.0 {
                break;
            }
            let radius = if finger == 4 { 0.0105 } else { 0.009 } * (1.0 - joint as f32 * 0.1);
            let seg = meshes.add(Capsule3d::new(radius, (len - radius).max(0.004)));
            let j = commands
                .spawn((
                    Finger { finger, joint },
                    Transform::from_translation(offset),
                    Visibility::default(),
                    ChildOf(parent),
                ))
                .id();
            // The bone itself, laid along the joint's -Z.
            commands.spawn((
                Mesh3d(seg),
                MeshMaterial3d(glove.clone()),
                Transform::from_xyz(0.0, 0.0, -len * 0.5)
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ChildOf(j),
            ));
            parent = j;
            offset = Vec3::new(0.0, 0.0, -len);
        }
    }
    root
}

/// How far each finger wants to curl, given what the hand is doing.
fn curl_targets(
    hand: &Hand,
    grip: f32,
    trigger: f32,
    held: &Query<(&Held, &GlobalTransform, &Grabbable)>,
    bounds: &Query<&avian3d::prelude::ColliderAabb>,
) -> [f32; 5] {
    // [index, middle, ring, little, thumb]
    if let Some(item) = hand.holding {
        let role = held
            .get(item)
            .ok()
            .and_then(|(h, _, g)| g.points.get(h.point).map(|p| p.role));
        return match role {
            // A pistol or rifle grip: the fingers round it, the index along
            // the frame until it takes up the trigger.
            Some(GripRole::Primary) => [0.22 + 0.33 * trigger, 0.74, 0.76, 0.78, 0.62],
            Some(GripRole::Support) => [0.55, 0.6, 0.62, 0.64, 0.5],
            Some(GripRole::Handle(_)) => [0.7, 0.72, 0.74, 0.76, 0.7],
            // Anything else: wrapped as far as its size lets the fingers
            // close (a round pinched, a can held round, a box flat-handed).
            _ => {
                let t = bounds
                    .get(item)
                    .map(|b| {
                        let s = b.max - b.min;
                        s.x.min(s.y).min(s.z)
                    })
                    .unwrap_or(0.06);
                let wrap = (0.95 - t * 5.0).clamp(0.25, 0.9);
                let index = (wrap + 0.25 * trigger).min(0.95);
                [index, wrap, wrap, wrap, (wrap * 0.85).max(0.35)]
            }
        };
    }
    if hand.supporting.is_some() {
        return [0.55, 0.6, 0.62, 0.64, 0.5];
    }
    if hand.handle.is_some() {
        return [0.72, 0.74, 0.76, 0.78, 0.7];
    }
    // Empty: the grip closes the hand, the trigger the index finger.
    [grip.max(trigger), grip, grip, grip, grip * 0.8]
}

/// Hands sit on what they hold (so the glove wraps the grip even when the
/// gun is blocked by a wall), otherwise on the tracked pose.
fn pose_hands(
    time: Res<Time>,
    mut visuals: Query<(&mut HandVisual, &mut Transform), Without<Finger>>,
    mut fingers: Query<(&Finger, &ChildOf, &mut Transform), Without<HandVisual>>,
    hands: Query<(&Hand, &GlobalTransform, &ChildOf)>,
    held: Query<(&Held, &GlobalTransform, &Grabbable)>,
    bounds: Query<&avian3d::prelude::ColliderAabb>,
    inputs: Query<&RigInput>,
    roots: Query<&ChildOf, With<Finger>>,
) {
    let ease = 1.0 - (-time.delta_secs() * 18.0).exp();
    for (mut v, mut t) in &mut visuals {
        let Ok((hand, gt, parent)) = hands.get(v.hand) else {
            continue;
        };
        let mut pose = gt.compute_transform();
        let input = inputs.get(parent.parent()).map(|i| i.hand(v.side));
        let grip = input.map_or(0.0, |i| i.grip);
        let trigger = input.map_or(0.0, |i| i.trigger);
        if let Some(item) = hand.holding {
            if let Ok((h, igt, g)) = held.get(item) {
                if let Some(p) = g.points.get(h.point).filter(|p| p.role != GripRole::Free) {
                    pose = igt.mul_transform(p.local).compute_transform();
                }
            }
        }
        *t = pose;
        let target = curl_targets(hand, grip, trigger, &held, &bounds);
        for (c, want) in v.curl.iter_mut().zip(target) {
            *c += (want - *c) * ease;
        }
    }
    for (f, parent, mut ft) in &mut fingers {
        // Up the chain to the hand this joint belongs to.
        let mut up = parent.parent();
        while let Ok(p) = roots.get(up) {
            up = p.parent();
        }
        let Ok((v, _)) = visuals.get(up) else {
            continue;
        };
        let c = v.curl[f.finger];
        let bend = if f.finger == 4 {
            THUMB_BEND[f.joint]
        } else {
            JOINT_BEND[f.joint]
        };
        let curl = Quat::from_rotation_x(-bend * c);
        ft.rotation = if f.joint == 0 {
            let thumb_side = -v.side.sign();
            let aim = if f.finger == 4 {
                Quat::from_rotation_y(0.75 * thumb_side) * Quat::from_rotation_z(-0.6 * thumb_side)
            } else {
                // Fingers fan a little as they open.
                Quat::from_rotation_y((1.5 - f.finger as f32) * 0.06 * (1.0 - c) * thumb_side)
            };
            aim * curl
        } else {
            curl
        };
    }
}
