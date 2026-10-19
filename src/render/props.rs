//! Visuals for the truck, the hands, the backpack and body rig, remains.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use crate::interact::body::{Backpack, BodySlot};
use crate::interact::{Grabbable, GripRole, Hand, Held};
use crate::models::{ModelMaterials, ModelState, Models, model_nodes, spawn_primitives};
use crate::raid::Remains;
use crate::raid::truck::{Pressable, Stash, TRUCK_PARTS, Truck, WHEELS};
use crate::render::materials::{Mat, MaterialLibrary, metric_box};
use crate::render::world_props::{Blockout, SwapIn};
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
                dress_camp,
                dress_capstone,
            ),
        )
        .add_systems(PostUpdate, pose_hands.after(TransformSystems::Propagate))
        .add_systems(
            Update,
            (
                tint_gloves,
                far_cue,
                body_casts_no_shadow,
                dress_buttons,
                push_caps,
            ),
        );
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
            | Pressable::Terminal
            | Pressable::Camp => {}
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
    crates: Query<Entity, Added<crate::items::loot::SupplyCrate>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
) {
    for e in &crates {
        let planks = lib.get(Mat::Planks, &server, &mut materials);
        commands.entity(e).insert(SwapIn::one("supply_crate"));
        commands.spawn((
            Mesh3d(meshes.add(metric_box(crate::items::loot::SUPPLY_CRATE))),
            MeshMaterial3d(planks),
            ChildOf(e),
            Blockout,
        ));
    }
    for (e, stash, remains) in &bags {
        if stash {
            models.request(STASH_MODEL, &server);
            commands
                .entity(e)
                .insert((Undressed(STASH_MODEL), SwapIn::one("stash_stand")));
            // Its stand, and the shelf the kit sits on in its top.
            let raise = crate::raid::truck::STASH_RAISE;
            let timber = lib.get(Mat::Timber, &server, &mut materials);
            commands.spawn((
                Mesh3d(meshes.add(metric_box(Vec3::new(0.86, raise, 1.16)))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(0.0, -0.3 - raise * 0.5, 0.0),
                ChildOf(e),
                Blockout,
            ));
            commands.spawn((
                Mesh3d(meshes.add(metric_box(Vec3::new(0.84, 0.02, 1.14)))),
                MeshMaterial3d(timber),
                Transform::from_xyz(0.0, 0.0, 0.0),
                ChildOf(e),
                Blockout,
            ));
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

/// Put on our own player's body (the pack, the pouches, what's stowed in
/// them) by this system, so it can take it off again.
#[derive(Component)]
struct OnBody;

/// What rides on your own body casts no shadow: without a body to go with
/// it, the first playtest saw a floating box (the pack) and hands in the
/// sun's shadow on the ground. Hands still cast theirs.
fn body_casts_no_shadow(
    mut commands: Commands,
    torsos: Query<(Entity, &ChildOf), With<crate::interact::body::Torso>>,
    locals: Query<(), With<LocalPlayer>>,
    children: Query<&Children>,
    meshes: Query<(), (With<Mesh3d>, Without<OnBody>)>,
    marked: Query<Entity, With<OnBody>>,
    time: Res<Time>,
    mut next: Local<f32>,
) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 0.25;
    let mut on = std::collections::HashSet::new();
    for (torso, parent) in &torsos {
        if !locals.contains(parent.parent()) {
            continue;
        }
        for e in children.iter_descendants(torso) {
            on.insert(e);
            if meshes.contains(e) {
                commands
                    .entity(e)
                    .try_insert((OnBody, bevy::light::NotShadowCaster));
            }
        }
    }
    for e in &marked {
        if !on.contains(&e) {
            commands
                .entity(e)
                .try_remove::<(OnBody, bevy::light::NotShadowCaster)>();
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

/// A glove's own material, tinted while the hand is in reach of something
/// it could take.
#[derive(Component)]
struct GloveMaterial(Handle<StandardMaterial>);

fn tint_gloves(
    visuals: Query<(&HandVisual, &GloveMaterial)>,
    hands: Query<&Hand>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lit: Local<std::collections::HashMap<Entity, bool>>,
) {
    for (v, glove) in &visuals {
        let Ok(hand) = hands.get(v.hand) else {
            continue;
        };
        let on = hand.hover.is_some();
        if lit.insert(v.hand, on) == Some(on) {
            continue;
        }
        if let Some(mut m) = materials.get_mut(&glove.0) {
            m.emissive = if on {
                LinearRgba::rgb(0.06, 0.16, 0.08)
            } else {
                LinearRgba::BLACK
            };
        }
    }
}

/// A push button's moving cap (see `interact::poke`).
#[derive(Component)]
struct ButtonCap;

/// A dark collar on the wall and an amber cap standing proud of it.
fn dress_buttons(
    mut commands: Commands,
    buttons: Query<Entity, Added<crate::interact::poke::PokeButton>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut looks: Local<
        Option<(
            Handle<Mesh>,
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    use crate::interact::poke::{BUTTON_R, CAP};
    for e in &buttons {
        let (collar, cap, dark, amber) = looks
            .get_or_insert_with(|| {
                (
                    meshes.add(Cylinder::new(BUTTON_R + 0.008, 0.008)),
                    meshes.add(Cylinder::new(BUTTON_R, CAP)),
                    mat(&mut materials, Color::srgb(0.06, 0.06, 0.06), 0.6, 0.6),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.85, 0.45, 0.08),
                        emissive: LinearRgba::rgb(0.12, 0.05, 0.0),
                        perceptual_roughness: 0.4,
                        ..default()
                    }),
                )
            })
            .clone();
        // Cylinders stand along Y; the button faces +Z.
        let face = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        commands.spawn((
            Mesh3d(collar),
            MeshMaterial3d(dark),
            Transform::from_xyz(0.0, 0.0, 0.004).with_rotation(face),
            ChildOf(e),
        ));
        commands.spawn((
            ButtonCap,
            Mesh3d(cap),
            MeshMaterial3d(amber),
            Transform::from_xyz(0.0, 0.0, CAP * 0.5).with_rotation(face),
            ChildOf(e),
        ));
    }
}

fn push_caps(
    buttons: Query<(&crate::interact::poke::PokeButton, &Children)>,
    mut caps: Query<&mut Transform, With<ButtonCap>>,
) {
    for (button, children) in &buttons {
        for c in children.iter() {
            if let Ok(mut t) = caps.get_mut(c) {
                t.translation.z = crate::interact::poke::CAP * 0.5 - button.depth;
            }
        }
    }
}

/// A ring around what an empty hand is pointing at from afar, facing the
/// hand, in the glove's tint: point, squeeze, flick and it comes to you.
#[derive(Component)]
struct FarCue;

fn far_cue(
    mut commands: Commands,
    hands: Query<(Entity, &Hand)>,
    items: Query<(&GlobalTransform, Option<&avian3d::prelude::ColliderAabb>)>,
    mut cues: Query<(&mut Transform, &mut Visibility), With<FarCue>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<std::collections::HashMap<Entity, Entity>>,
    time: Res<Time>,
) {
    for (hand_e, hand) in &hands {
        let target = hand.far.and_then(|e| items.get(e).ok());
        let cue = match made.get(&hand_e) {
            Some(c) => *c,
            None if target.is_some() => {
                let c = commands
                    .spawn((
                        Name::new("Far grab cue"),
                        FarCue,
                        Mesh3d(meshes.add(Torus::new(0.94, 1.0))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgba(0.3, 0.9, 0.4, 0.5),
                            emissive: LinearRgba::rgb(0.3, 0.9, 0.4),
                            unlit: true,
                            alpha_mode: AlphaMode::Add,
                            ..default()
                        })),
                        bevy::light::NotShadowCaster,
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                made.insert(hand_e, c);
                continue;
            }
            None => continue,
        };
        let Ok((mut t, mut vis)) = cues.get_mut(cue) else {
            continue;
        };
        let Some((gt, aabb)) = target else {
            *vis = Visibility::Hidden;
            continue;
        };
        let at = gt.translation();
        let size = aabb.map_or(0.15, |a| (a.max - a.min).max_element());
        let pulse = 1.0 + 0.08 * (time.elapsed_secs() * 6.0).sin();
        t.translation = at;
        t.rotation =
            Quat::from_rotation_arc(Vec3::Y, (hand.world.translation - at).normalize_or(Vec3::Y));
        t.scale = Vec3::splat((size * 0.65 + 0.04).clamp(0.08, 0.6) * pulse);
        *vis = Visibility::Visible;
    }
}

/// The glove's mesh and fingers, rolled onto the hand's frame (see
/// `spawn_hand`).
#[derive(Component)]
struct GloveModel;

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
            GloveMaterial(glove.clone()),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    // The hand's frame points -Z with +Y up (a hand holding a pistol: see
    // `rig::xr`), so the palm faces inward and the thumb up. The model is
    // built palm-down and rolled onto its side here; the first real
    // headset showed the backs of the hands with palms facing each other.
    let model = commands
        .spawn((
            GloveModel,
            Mesh3d(palm),
            MeshMaterial3d(glove.clone()),
            Transform::from_rotation(Quat::from_rotation_z(
                -std::f32::consts::FRAC_PI_2 * side.sign(),
            )),
            Visibility::default(),
            ChildOf(root),
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
        let mut parent = model;
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
    models: Query<&ChildOf, With<GloveModel>>,
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
        if let Ok(p) = models.get(up) {
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

/// The camp's folding table (with a lantern burning on it), the board
/// propped at its back, and the post the yard's skip card hangs on.
fn dress_camp(
    mut commands: Commands,
    props: Query<(Entity, &crate::camp::CampProps), Added<crate::camp::CampProps>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    use crate::camp::{BOARD, CampProps, POST_HEIGHT, TABLE};
    use crate::render::materials::{Mat, metric_box};
    for (e, p) in &props {
        // Boxes until the models are in (`art/props/outdoor.py`).
        let models = match p {
            CampProps::Table => vec![
                ("camp_table", Transform::IDENTITY),
                (
                    "lantern",
                    Transform::from_translation(Vec3::new(-0.48, TABLE.y, 0.12)),
                ),
            ],
            CampProps::Board => vec![("camp_board", Transform::IDENTITY)],
            CampProps::Post => vec![("camp_post", Transform::IDENTITY)],
        };
        commands.entity(e).insert(SwapIn(models));
        let mut part = |m: Mat, size: Vec3, at: Transform| {
            let material = lib.get(m, &server, &mut materials);
            commands.spawn((
                Mesh3d(meshes.add(metric_box(size))),
                MeshMaterial3d(material),
                at,
                ChildOf(e),
                Blockout,
            ));
        };
        match p {
            CampProps::Table => {
                part(
                    Mat::Planks,
                    Vec3::new(TABLE.x, 0.04, TABLE.z),
                    Transform::from_xyz(0.0, TABLE.y - 0.02, 0.0),
                );
                for (x, z) in [(-0.58, -0.28), (0.58, -0.28), (-0.58, 0.28), (0.58, 0.28)] {
                    part(
                        Mat::DarkMetal,
                        Vec3::new(0.035, TABLE.y - 0.04, 0.035),
                        Transform::from_xyz(x, (TABLE.y - 0.04) * 0.5, z),
                    );
                }
                // A stool to stand by, a crate under the table.
                part(
                    Mat::Planks,
                    Vec3::new(0.5, 0.4, 0.4),
                    Transform::from_xyz(-0.35, 0.2, -0.05),
                );
                part(
                    Mat::Timber,
                    Vec3::new(0.34, 0.04, 0.34),
                    Transform::from_xyz(0.75, 0.45, 0.65),
                );
                part(
                    Mat::Timber,
                    Vec3::new(0.04, 0.43, 0.04),
                    Transform::from_xyz(0.75, 0.215, 0.65),
                );
                // The lantern: a dark body, a warm glass, its light.
                let glass = materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.8, 0.5),
                    emissive: LinearRgba::rgb(6.0, 3.2, 1.0),
                    ..default()
                });
                let dark = lib.get(Mat::DarkMetal, &server, &mut materials);
                let lantern = Vec3::new(-0.48, TABLE.y, 0.12);
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.06, 0.03))),
                    MeshMaterial3d(dark.clone()),
                    Transform::from_translation(lantern + Vec3::Y * 0.015),
                    ChildOf(e),
                    Blockout,
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.045, 0.13))),
                    MeshMaterial3d(glass),
                    Transform::from_translation(lantern + Vec3::Y * 0.095),
                    ChildOf(e),
                    Blockout,
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.055, 0.025))),
                    MeshMaterial3d(dark),
                    Transform::from_translation(lantern + Vec3::Y * 0.17),
                    ChildOf(e),
                    Blockout,
                ));
                commands.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.72, 0.42),
                        intensity: 9_000.0,
                        range: 7.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(lantern + Vec3::Y * 0.1),
                    ChildOf(e),
                ));
            }
            CampProps::Board => {
                part(
                    Mat::Planks,
                    Vec3::new(BOARD.x, BOARD.y, 0.025),
                    Transform::default(),
                );
                // A frame round it, and a strut behind.
                for (size, at) in [
                    (
                        Vec3::new(BOARD.x + 0.06, 0.04, 0.04),
                        Vec3::new(0.0, BOARD.y * 0.5, 0.0),
                    ),
                    (
                        Vec3::new(BOARD.x + 0.06, 0.04, 0.04),
                        Vec3::new(0.0, -BOARD.y * 0.5, 0.0),
                    ),
                    (
                        Vec3::new(0.04, BOARD.y, 0.04),
                        Vec3::new(BOARD.x * 0.5 + 0.01, 0.0, 0.0),
                    ),
                    (
                        Vec3::new(0.04, BOARD.y, 0.04),
                        Vec3::new(-BOARD.x * 0.5 - 0.01, 0.0, 0.0),
                    ),
                ] {
                    part(Mat::Timber, size, Transform::from_translation(at));
                }
                part(
                    Mat::Timber,
                    Vec3::new(0.04, BOARD.y * 0.9, 0.04),
                    Transform::from_xyz(0.0, -0.05, -0.2)
                        .with_rotation(Quat::from_rotation_x(0.45)),
                );
            }
            CampProps::Post => {
                part(
                    Mat::Timber,
                    Vec3::new(0.08, POST_HEIGHT, 0.08),
                    Transform::from_xyz(0.0, POST_HEIGHT * 0.5, 0.0),
                );
                part(
                    Mat::Planks,
                    Vec3::new(0.42, 0.28, 0.03),
                    Transform::from_xyz(0.0, 1.35, 0.025),
                );
            }
        }
    }
}

/// The depot lock's pedestal (cradle, screen housing, lamp) and the
/// strongbox a valley key lies in (a beacon blinking amber on it).
fn dress_capstone(
    mut commands: Commands,
    props: Query<(Entity, &crate::capstone::CapstoneProps), Added<crate::capstone::CapstoneProps>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut lib: ResMut<MaterialLibrary>,
    server: Res<AssetServer>,
) {
    use crate::capstone::CapstoneProps;
    use crate::render::materials::{Mat, metric_box};
    for (e, p) in &props {
        let lamp = |materials: &mut Assets<StandardMaterial>, c: LinearRgba| {
            materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: c,
                ..default()
            })
        };
        let mut part = |m: Mat, size: Vec3, at: Vec3| {
            let material = lib.get(m, &server, &mut materials);
            commands.spawn((
                Mesh3d(meshes.add(metric_box(size))),
                MeshMaterial3d(material),
                Transform::from_translation(at),
                ChildOf(e),
            ));
        };
        match p {
            CapstoneProps::Pedestal => {
                // A depot machine: a concrete plinth with hazard banding,
                // a painted cabinet with vents and status lamps, and its
                // cable going into the ground behind.
                part(
                    Mat::Concrete,
                    Vec3::new(0.95, 0.12, 0.8),
                    Vec3::new(0.0, 0.06, 0.0),
                );
                part(
                    Mat::PaintedMetal,
                    Vec3::new(0.7, 0.78, 0.55),
                    Vec3::new(0.0, 0.51, 0.0),
                );
                part(
                    Mat::DarkMetal,
                    Vec3::new(0.66, 0.1, 0.5),
                    Vec3::new(0.0, 0.95, -0.01),
                );
                // Vents down each side.
                for side in [-1.0f32, 1.0] {
                    for k in 0..4 {
                        part(
                            Mat::DarkMetal,
                            Vec3::new(0.01, 0.025, 0.36),
                            Vec3::new(side * 0.352, 0.35 + k as f32 * 0.07, 0.0),
                        );
                    }
                }
                // The cradle: a plate with a rim and two dividers.
                part(
                    Mat::DarkMetal,
                    Vec3::new(0.62, 0.03, 0.46),
                    Vec3::new(0.0, 1.015, 0.0),
                );
                for (size, at) in [
                    (Vec3::new(0.62, 0.06, 0.02), Vec3::new(0.0, 1.06, 0.22)),
                    (Vec3::new(0.62, 0.06, 0.02), Vec3::new(0.0, 1.06, -0.22)),
                    (Vec3::new(0.02, 0.06, 0.42), Vec3::new(0.3, 1.06, 0.0)),
                    (Vec3::new(0.02, 0.06, 0.42), Vec3::new(-0.3, 1.06, 0.0)),
                    (Vec3::new(0.015, 0.05, 0.42), Vec3::new(0.1, 1.055, 0.0)),
                    (Vec3::new(0.015, 0.05, 0.42), Vec3::new(-0.1, 1.055, 0.0)),
                ] {
                    part(Mat::DarkMetal, size, at);
                }
                // The screen's mast and housing, behind the cradle.
                part(
                    Mat::DarkMetal,
                    Vec3::new(0.06, 0.3, 0.06),
                    Vec3::new(0.0, 1.15, -0.25),
                );
                part(
                    Mat::RobotShell,
                    Vec3::new(0.48, 0.36, 0.08),
                    Vec3::new(0.0, 1.45, -0.24),
                );
                let yellow = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.75, 0.6, 0.12),
                    perceptual_roughness: 0.7,
                    ..default()
                });
                let black = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.05, 0.05, 0.05),
                    perceptual_roughness: 0.7,
                    ..default()
                });
                // Stripes along the plinth's front, standing proud of it.
                for k in 0..8 {
                    let x = -0.42 + k as f32 * 0.12;
                    commands.spawn((
                        Mesh3d(meshes.add(metric_box(Vec3::new(0.12, 0.1, 0.01)))),
                        MeshMaterial3d(if k % 2 == 0 {
                            yellow.clone()
                        } else {
                            black.clone()
                        }),
                        Transform::from_xyz(x, 0.06, 0.405),
                        ChildOf(e),
                    ));
                }
                // A sloped lid over the cabinet's front, under the cradle.
                let lid = lib.get(Mat::PaintedMetal, &server, &mut materials);
                commands.spawn((
                    Mesh3d(meshes.add(metric_box(Vec3::new(0.7, 0.03, 0.2)))),
                    MeshMaterial3d(lid),
                    Transform::from_xyz(0.0, 0.93, 0.25).with_rotation(Quat::from_rotation_x(0.5)),
                    ChildOf(e),
                ));
                // Three status lamps on the front, one per valley's key.
                let dim = lamp(&mut materials, LinearRgba::rgb(2.2, 0.45, 0.1));
                for k in 0..3 {
                    commands.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.022, 0.02))),
                        MeshMaterial3d(dim.clone()),
                        Transform::from_xyz(-0.12 + k as f32 * 0.12, 0.72, 0.28)
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        ChildOf(e),
                    ));
                }
                // The cable: out of the back and down into the ground.
                let cable = lib.get(Mat::DarkMetal, &server, &mut materials);
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.035, 0.5))),
                    MeshMaterial3d(cable.clone()),
                    Transform::from_xyz(0.18, 0.35, -0.3),
                    ChildOf(e),
                ));
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.035, 0.35))),
                    MeshMaterial3d(cable),
                    Transform::from_xyz(0.18, 0.08, -0.45)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ChildOf(e),
                ));
            }
            CapstoneProps::Strongbox => {
                part(
                    Mat::DarkMetal,
                    Vec3::new(0.5, 0.04, 0.36),
                    Vec3::new(0.0, 0.02, 0.0),
                );
                for (size, at) in [
                    (Vec3::new(0.5, 0.18, 0.02), Vec3::new(0.0, 0.11, 0.17)),
                    (Vec3::new(0.5, 0.18, 0.02), Vec3::new(0.0, 0.11, -0.17)),
                    (Vec3::new(0.02, 0.18, 0.32), Vec3::new(0.24, 0.11, 0.0)),
                    (Vec3::new(0.02, 0.18, 0.32), Vec3::new(-0.24, 0.11, 0.0)),
                ] {
                    part(Mat::PaintedMetal, size, at);
                }
                // The lid, thrown back.
                let lid = lib.get(Mat::PaintedMetal, &server, &mut materials);
                commands.spawn((
                    Mesh3d(meshes.add(metric_box(Vec3::new(0.5, 0.02, 0.36)))),
                    MeshMaterial3d(lid),
                    Transform::from_xyz(0.0, 0.3, -0.3).with_rotation(Quat::from_rotation_x(1.2)),
                    ChildOf(e),
                ));
                // A survey beacon still blinking on it: you see it first.
                let glow = lamp(&mut materials, LinearRgba::rgb(8.0, 3.5, 0.6));
                commands.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.025, 0.05))),
                    MeshMaterial3d(glow),
                    Transform::from_xyz(0.22, 0.225, 0.15),
                    ChildOf(e),
                ));
                commands.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.6, 0.2),
                        intensity: 6_000.0,
                        range: 8.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.22, 0.35, 0.15),
                    ChildOf(e),
                ));
            }
        }
    }
}
