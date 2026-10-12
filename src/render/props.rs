//! Visuals for the truck, the hands, the backpack and body rig, remains.

use bevy::prelude::*;

use crate::interact::body::{Backpack, BodySlot};
use crate::interact::{Grabbable, GripRole, Hand, Held};
use crate::raid::Remains;
use crate::raid::truck::{Pressable, Stash, TRUCK_PARTS, Truck, WHEELS};
use crate::rig::{LocalPlayer, RigHand, RigInput, Side};

pub struct PropVisualsPlugin;

impl Plugin for PropVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                dress_truck,
                dress_containers,
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

fn dress_truck(
    mut commands: Commands,
    trucks: Query<Entity, Added<Truck>>,
    pressables: Query<(Entity, &Pressable), Added<Pressable>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for truck in &trucks {
        let paint = mat(&mut materials, Color::srgb(0.22, 0.26, 0.17), 0.65, 0.2);
        let dark = mat(&mut materials, Color::srgb(0.06, 0.06, 0.06), 0.7, 0.4);
        let planks = mat(&mut materials, Color::srgb(0.32, 0.25, 0.17), 0.85, 0.0);
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
            | Pressable::FoldCover => {}
            Pressable::BellPull => {
                let rope = mat(&mut materials, Color::srgb(0.55, 0.47, 0.32), 0.9, 0.0);
                commands.entity(e).insert((
                    Mesh3d(meshes.add(Cylinder::new(0.012, 0.5))),
                    MeshMaterial3d(rope),
                ));
            }
        }
    }
}

fn dress_containers(
    mut commands: Commands,
    bags: Query<(Entity, Has<Stash>, Has<Remains>), Added<Backpack>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, stash, remains) in &bags {
        if stash {
            let wood = mat(&mut materials, Color::srgb(0.36, 0.28, 0.18), 0.8, 0.0);
            // An open-topped crate: floor and four walls.
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

/// A gloved hand: palm and fingers that curl with the grip.
#[derive(Component)]
struct HandVisual {
    hand: Entity,
    side: Side,
}

#[derive(Component)]
struct Finger {
    index: usize,
}

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
        let glove = mat(&mut materials, Color::srgb(0.1, 0.1, 0.09), 0.8, 0.0);
        let palm = meshes.add(Cuboid::new(0.085, 0.03, 0.1));
        let finger = meshes.add(Capsule3d::new(0.01, 0.06));
        let root = commands
            .spawn((
                HandVisual {
                    hand: e,
                    side: rh.side,
                },
                Mesh3d(palm),
                MeshMaterial3d(glove.clone()),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        for i in 0..5 {
            let x = if i == 4 {
                0.05 * -rh.side.sign()
            } else {
                -0.03 + i as f32 * 0.02
            };
            let z = if i == 4 { 0.0 } else { -0.06 };
            commands.spawn((
                Finger { index: i },
                Mesh3d(finger.clone()),
                MeshMaterial3d(glove.clone()),
                Transform::from_xyz(x * rh.side.sign().abs(), 0.0, z),
                ChildOf(root),
            ));
        }
    }
}

/// Hands sit on what they hold (so the glove wraps the grip even when the
/// gun is blocked by a wall), otherwise on the tracked pose.
fn pose_hands(
    mut visuals: Query<(&HandVisual, &mut Transform, &Children), Without<Finger>>,
    mut fingers: Query<(&Finger, &mut Transform), Without<HandVisual>>,
    hands: Query<(&Hand, &GlobalTransform, &ChildOf)>,
    held: Query<(&Held, &GlobalTransform, &Grabbable)>,
    inputs: Query<&RigInput>,
) {
    for (v, mut t, children) in &mut visuals {
        let Ok((hand, gt, parent)) = hands.get(v.hand) else {
            continue;
        };
        let mut pose = gt.compute_transform();
        let mut grip = inputs
            .get(parent.parent())
            .map(|i| i.hand(v.side).grip)
            .unwrap_or(0.0);
        if let Some(item) = hand.holding {
            if let Ok((h, igt, g)) = held.get(item) {
                if let Some(p) = g.points.get(h.point).filter(|p| p.role != GripRole::Free) {
                    pose = igt.mul_transform(p.local).compute_transform();
                }
            }
            grip = 1.0;
        }
        if hand.supporting.is_some() || hand.handle.is_some() {
            grip = 1.0;
        }
        *t = pose;
        for c in children.iter() {
            if let Ok((f, mut ft)) = fingers.get_mut(c) {
                let curl = grip * if f.index == 4 { 0.9 } else { 1.4 };
                let x = ft.translation.x;
                let base_z = if f.index == 4 { 0.0 } else { -0.06 };
                ft.translation = Vec3::new(x, -0.02 * grip, base_z + 0.025 * grip);
                ft.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 - curl);
            }
        }
    }
}
