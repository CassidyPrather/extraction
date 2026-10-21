//! Visuals for the truck, the hands, the backpack and body rig, remains.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;

use crate::interact::body::Backpack;
use crate::interact::{Grabbable, GripRole, Hand, Held};
use crate::models::{ModelMaterials, ModelState, Models, model_nodes, spawn_primitives};
use crate::raid::Remains;
use crate::raid::truck::{Pressable, Stash, TRUCK_PARTS, Truck, WHEELS};
use crate::render::materials::{Mat, MaterialLibrary, metric_box};
use crate::render::world_props::{Blockout, SwapIn};
use crate::rig::{LocalPlayer, RigHand, RigInput, Side};

pub struct PropVisualsPlugin;

/// Where the gloves take this frame's pose (`PostUpdate`, before
/// propagation): anything following them (the sleeves) runs after it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlovePose;

impl Plugin for PropVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (dress_truck, dress_containers, finish_models).chain(),
                spawn_hand_visuals,
                dress_gloves,
                dress_camp,
                dress_capstone,
            ),
        )
        // Before propagation, from this frame's poses: posed after it, the
        // gloves drew where the hands were a frame ago (11 ms behind at 90 Hz).
        .add_systems(
            PostUpdate,
            pose_hands
                .in_set(GlovePose)
                .after(crate::rig::apply_rig_poses)
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            Update,
            (
                tint_gloves,
                ghost_through_walls,
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
            | Pressable::Camp
            | Pressable::Logbook
            | Pressable::Note => {}
            Pressable::BellPull => {
                let rope = mat(&mut materials, Color::srgb(0.55, 0.47, 0.32), 0.9, 0.0);
                commands.entity(e).insert((
                    Mesh3d(meshes.add(Cylinder::new(0.012, 0.5))),
                    MeshMaterial3d(rope.clone()),
                ));
                // A red-painted toggle to pull on: against the canvas and its
                // own rope, a rope-coloured one couldn't be told from the
                // rope (`docs/LEGIBILITY.md`, affordances).
                let red = mat(&mut materials, Color::srgb(0.68, 0.08, 0.05), 0.5, 0.0);
                commands.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.022, 0.09))),
                    MeshMaterial3d(red),
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
        }
        // The worn pack is `kit::dress_pack`'s.
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

/// A gloved hand: a palm, four three-jointed fingers and a two-jointed
/// thumb, curling to fit what they hold.
#[derive(Component)]
pub(crate) struct HandVisual {
    pub(crate) hand: Entity,
    pub(crate) side: Side,
    /// Current curl per finger (index, middle, ring, little, thumb), 0..1,
    /// eased toward the target.
    curl: [f32; 5],
    /// Current spread of index..little, radians toward the thumb (eased;
    /// tracked fingers set it, otherwise `rest_spread`).
    spread: [f32; 4],
    /// A tracked thumb: which way its first bone points on the unrolled
    /// glove and how far its last joint bends (radians); `None` swings it
    /// by its curl.
    thumb: Option<(Vec3, f32)>,
    /// The faint glove at the tracked pose, shown when the real hand has
    /// gone where the glove can't (into a wall).
    pub(crate) ghost: bool,
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
/// The thumb's second joint only: its first swings (`finger_aim_spread`).
const THUMB_BEND: [f32; 3] = [0.0, 0.9, 0.0];
/// The thumb's curl on a pistol grip (`hold_curl`), where it lies along
/// the far side of the frame.
const THUMB_ON_GRIP: f32 = 0.62;

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
        commands.entity(root).insert((
            HandVisual {
                hand: e,
                side: rh.side,
                curl: [0.0; 5],
                spread: std::array::from_fn(rest_spread),
                thumb: None,
                ghost: false,
            },
            PlayerGlove,
        ));
        // The ghost: where your real hand is when a wall stops the glove
        // (`interact::physics_hands`), faint, hidden otherwise.
        let ghost = spawn_hand(
            &mut commands,
            &mut meshes,
            &mut materials,
            rh.side,
            [0.0; 5],
        );
        commands.entity(ghost).insert((
            HandVisual {
                hand: e,
                side: rh.side,
                curl: [0.0; 5],
                spread: std::array::from_fn(rest_spread),
                thumb: None,
                ghost: true,
            },
            Visibility::Hidden,
        ));
    }
}

/// The local player's gloves (the gallery hides them to show its own).
#[derive(Component)]
pub(crate) struct PlayerGlove;

/// A glove's own material, tinted while the hand is in reach of something
/// it could take.
#[derive(Component)]
struct GloveMaterial(Vec<Handle<StandardMaterial>>);

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
        for h in &glove.0 {
            if let Some(mut m) = materials.get_mut(h) {
                m.emissive = if on {
                    LinearRgba::rgb(0.06, 0.16, 0.08)
                } else {
                    LinearRgba::BLACK
                };
            }
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

/// A ring around what an empty hand is pointing at from afar, or would
/// take where it is, facing the hand, in the glove's tint: point, squeeze,
/// flick and it comes to you; or just squeeze.
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
        // What a pointed hand would pull in, else what the hand would take
        // where it is: the near hover lit only the glove, faintly, and the
        // thing itself never showed it could be taken.
        let target = hand.far.or(hand.hover).and_then(|e| items.get(e).ok());
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

/// The glove's palm and fingers, rolled onto the hand's frame (see
/// `spawn_hand`).
#[derive(Component)]
struct GloveModel;

/// Where a glove's modelled parts go, and the boxes it wears until (or
/// unless) its model is in.
#[derive(Component)]
struct GloveParts {
    model: Entity,
    joints: Vec<((usize, usize), Entity)>,
    boxes: Vec<Entity>,
}

/// The glove models (`art/hands/gloves.py`), built in the frame
/// `spawn_hand` builds its joints in.
fn glove_model(side: Side) -> &'static str {
    match side {
        Side::Left => "models/hands/glove_left.glb",
        Side::Right => "models/hands/glove_right.glb",
    }
}

/// A finger's first joint on the (unrolled) glove: fingers point along -Z
/// from the palm's front edge and curl toward its underside (-Y); the index
/// is on the thumb's side.
fn finger_root(side: Side, finger: usize) -> Vec3 {
    let thumb_side = -side.sign();
    if finger == 4 {
        Vec3::new(0.038 * thumb_side, -0.008, -0.005)
    } else {
        let x = (0.03 - finger as f32 * 0.02) * thumb_side;
        Vec3::new(x, 0.0, -0.048)
    }
}

/// How far the fingers spread from the index as the hand opens, radians per
/// finger. The index doesn't turn, so a pointing finger ends on
/// `interact::poke::FINGERTIP`.
const FAN: f32 = 0.04;

/// A finger's own spread with the hand open, radians toward the thumb: the
/// fingers fan away from the index (see `FAN`).
fn rest_spread(finger: usize) -> f32 {
    -(finger.min(3) as f32) * FAN
}

/// A finger's first joint's turn, curled `c` (0 open, 1 closed) and spread
/// `spread` radians toward the thumb (tracked fingers give their own; less
/// as it curls): its aim and then its curl.
fn finger_aim_spread(side: Side, finger: usize, c: f32, spread: f32) -> Quat {
    let thumb_side = -side.sign();
    let curl = Quat::from_rotation_x(-JOINT_BEND[0] * c);
    if finger == 4 {
        // The thumb swings rather than bends at its root: out beside the
        // index when open, down along the far side of a grip, across the
        // fingers in a fist; its pad turned toward them.
        let t = |x: f32, y: f32, z: f32| Vec3::new(x * thumb_side, y, z);
        let (open, grip, fist) = (
            t(0.45, -0.25, -0.86),
            t(-0.15, -0.55, -0.82),
            t(-0.75, -0.45, -0.5),
        );
        let d = if c < THUMB_ON_GRIP {
            open.lerp(grip, c / THUMB_ON_GRIP)
        } else {
            grip.lerp(fist, (c - THUMB_ON_GRIP) / (1.0 - THUMB_ON_GRIP))
        };
        Quat::from_rotation_arc(Vec3::NEG_Z, d.normalize())
            * Quat::from_rotation_z(-0.6 * thumb_side)
    } else {
        Quat::from_rotation_y(-spread * (1.0 - c) * thumb_side) * curl
    }
}

/// Where a held grip runs through the closed glove (unrolled): inside the
/// fingers' wrap, against the palm under the knuckles.
const GRIP: Vec3 = Vec3::new(0.0, -0.028, -0.041);

/// The glove on the hand. The hand's frame points -Z with +Y up (a hand
/// holding a pistol: see `rig::xr`), so the palm faces inward and the thumb
/// up: the glove is built palm-down and rolled onto its side (the first
/// real headset showed the backs of the hands with palms facing each
/// other). Its origin is the controller's grip, the middle of the fist, so
/// the glove sits with `GRIP` there: centred on the palm, a gun's grip went
/// through the palm and the fingers closed on air beside it.
pub(crate) fn glove_frame(side: Side) -> Transform {
    let roll = Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2 * side.sign());
    Transform::from_translation(roll * -GRIP).with_rotation(roll)
}

/// Where the wrist display lies on a glove, in the hand's frame: flat over
/// the housing on the cuff's back (`art/hands/gloves.py`), its text
/// running from the elbow to the fingers, so it reads level with the
/// forearm held across the chest like a watch. The right glove's is the
/// left's mirrored, its text turned round to read the same way (the
/// forearm across the chest points the other way).
pub(crate) fn wrist_mount(side: Side) -> Transform {
    let axes = match side {
        Side::Left => Mat3::from_cols(Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
        Side::Right => Mat3::from_cols(Vec3::Z, Vec3::X, Vec3::Y),
    };
    let on_glove = Transform::from_xyz(0.0, 0.0295, 0.0625).with_rotation(Quat::from_mat3(&axes));
    glove_frame(side) * on_glove
}

/// A joint's turn, its finger curled `c`.
fn joint_rotation(side: Side, finger: usize, joint: usize, c: f32) -> Quat {
    joint_rotation_spread(side, finger, joint, c, rest_spread(finger))
}

/// `joint_rotation` with the finger spread `spread` (see `finger_aim_spread`).
fn joint_rotation_spread(side: Side, finger: usize, joint: usize, c: f32, spread: f32) -> Quat {
    if joint == 0 {
        return finger_aim_spread(side, finger, c, spread);
    }
    let bend = if finger == 4 {
        THUMB_BEND[joint]
    } else {
        JOINT_BEND[joint]
    };
    Quat::from_rotation_x(-bend * c)
}

/// The ghost glove shows through whatever stopped the hand you see: your
/// real hand is in the wall, so a ghost drawn behind its face would never
/// be seen. Its meshes swap to the glass eye's material once dressed: a
/// faint rim, additive, with no depth test. (Without that material, in
/// tests, it keeps its plain translucent one.)
fn ghost_through_walls(
    mut commands: Commands,
    ghosts: Query<(Entity, &HandVisual)>,
    children: Query<&Children>,
    plain: Query<(), With<MeshMaterial3d<StandardMaterial>>>,
    xray: Option<ResMut<Assets<crate::anomaly::visuals::XrayMaterial>>>,
    mut handle: Local<Option<Handle<crate::anomaly::visuals::XrayMaterial>>>,
) {
    let Some(mut xray) = xray else {
        return;
    };
    for (e, v) in &ghosts {
        if !v.ghost {
            continue;
        }
        for d in children.iter_descendants(e) {
            if !plain.contains(d) {
                continue;
            }
            let m = handle
                .get_or_insert_with(|| {
                    xray.add(crate::anomaly::visuals::XrayMaterial {
                        color: LinearRgba::rgb(0.11, 0.12, 0.13),
                    })
                })
                .clone();
            commands
                .entity(d)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(m), bevy::light::NotShadowCaster));
        }
    }
}

/// A glove: the palm at the root, joints below it. Without a tracked hand
/// (the gallery's), it keeps the curl it's given. It wears boxes until its
/// model (`glove_model`) is in, and keeps them if it never comes.
pub(crate) fn spawn_hand(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    side: Side,
    curl: [f32; 5],
) -> Entity {
    let glove = mat(materials, Color::srgb(0.2, 0.2, 0.17), 0.85, 0.0);
    let palm = meshes.add(Cuboid::new(0.085, 0.03, 0.1));
    let root = commands
        .spawn((
            HandVisual {
                hand: Entity::PLACEHOLDER,
                side,
                curl,
                spread: std::array::from_fn(rest_spread),
                thumb: None,
                ghost: false,
            },
            GloveMaterial(vec![glove.clone()]),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    let model = commands
        .spawn((
            GloveModel,
            glove_frame(side),
            Visibility::default(),
            ChildOf(root),
        ))
        .id();
    let mut boxes = vec![
        commands
            .spawn((
                Mesh3d(palm),
                MeshMaterial3d(glove.clone()),
                Transform::default(),
                ChildOf(model),
            ))
            .id(),
    ];
    let mut joints = Vec::new();
    for finger in 0..5 {
        let mut parent = model;
        let mut offset = finger_root(side, finger);
        for (joint, len) in FINGER_BONES[finger].iter().enumerate() {
            if *len <= 0.0 {
                break;
            }
            let radius = if finger == 4 { 0.0105 } else { 0.009 } * (1.0 - joint as f32 * 0.1);
            let seg = meshes.add(Capsule3d::new(radius, (len - radius).max(0.004)));
            let rotation = joint_rotation(side, finger, joint, curl[finger]);
            let j = commands
                .spawn((
                    Finger { finger, joint },
                    Transform::from_translation(offset).with_rotation(rotation),
                    Visibility::default(),
                    ChildOf(parent),
                ))
                .id();
            joints.push(((finger, joint), j));
            // The bone itself, laid along the joint's -Z.
            boxes.push(
                commands
                    .spawn((
                        Mesh3d(seg),
                        MeshMaterial3d(glove.clone()),
                        Transform::from_xyz(0.0, 0.0, -len * 0.5)
                            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                        ChildOf(j),
                    ))
                    .id(),
            );
            parent = j;
            offset = Vec3::new(0.0, 0.0, -len);
        }
    }
    commands.entity(root).insert(GloveParts {
        model,
        joints,
        boxes,
    });
    root
}

/// A node anywhere under `nodes`, by name.
fn find_node<'a>(
    nodes: &'a [crate::models::ModelNode],
    name: &str,
) -> Option<&'a crate::models::ModelNode> {
    nodes.iter().find_map(|n| {
        (n.name == name)
            .then_some(n)
            .or_else(|| find_node(&n.children, name))
    })
}

/// Swaps a glove's boxes for its model once it's loaded: the palm onto the
/// glove, each segment onto its joint (the joints keep their own
/// transforms; the model's are only its rest pose). The glove gets its own
/// copies of its materials, so `tint_gloves` lights one hand at a time.
fn dress_gloves(
    mut commands: Commands,
    mut gloves: Query<(Entity, &HandVisual, &GloveParts, &mut GloveMaterial)>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, v, parts, mut glove_mat) in &mut gloves {
        let path = glove_model(v.side);
        let model = match models.request(path, &server) {
            None => None,
            Some(h) => match Models::state(&h, &server, &gltfs) {
                ModelState::Loading => continue,
                ModelState::Failed => {
                    warn!("{path} failed to load; using boxes");
                    None
                }
                ModelState::Ready(g) => Some(model_nodes(g, &nodes, &gltf_meshes)),
            },
        };
        commands.entity(e).remove::<GloveParts>();
        let Some(model) = model else {
            // A ghost of boxes would read as solid hands: none at all.
            if v.ghost {
                commands.entity(e).despawn();
            }
            continue;
        };
        let Some(palm) = find_node(&model, "palm") else {
            warn!("{path} has no palm; using boxes");
            continue;
        };
        let mut own: Vec<(String, Handle<StandardMaterial>)> = Vec::new();
        let ghost = v.ghost.then(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.8, 0.84, 0.86, 0.2),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            })
        });
        let mut mat = |name: Option<&str>| {
            if let Some(g) = &ghost {
                return g.clone();
            }
            let name = name.unwrap_or("Glove");
            if let Some((_, h)) = own.iter().find(|(n, _)| n == name) {
                return h.clone();
            }
            let shared = model_mats.get(name, &mut lib, &server, &mut materials);
            let copy = materials.get(&shared).cloned().unwrap_or_default();
            let h = materials.add(copy);
            own.push((name.to_owned(), h.clone()));
            h
        };
        crate::models::spawn_primitives(&mut commands, parts.model, palm, &mut mat);
        for &((finger, joint), j) in &parts.joints {
            match find_node(&model, &format!("finger_{finger}_{joint}")) {
                Some(node) => crate::models::spawn_primitives(&mut commands, j, node, &mut mat),
                None => warn!("{path} has no finger_{finger}_{joint}"),
            }
        }
        for b in &parts.boxes {
            commands.entity(*b).despawn();
        }
        glove_mat.0 = own.into_iter().map(|(_, h)| h).collect();
    }
}

/// A finger pointed: the index out, bent a little so its tip comes in line
/// with the grip (on `interact::poke::FINGERTIP`), the others closed.
pub(crate) const POINTING: [f32; 5] = [0.134, 0.9, 0.92, 0.95, 0.65];

/// How near a push button an empty hand's fingertip must come for the hand
/// to point, metres.
const POINT_REACH: f32 = 0.2;

/// The index's curl on a gun's grip, its trigger pulled `trigger`: from
/// lying along the frame to where its tip meets the gun's own trigger (the
/// layout's `Trigger` part, seen from its grip), so each gun's trigger
/// takes the finger's crease or pad rather than one curl for all.
pub(crate) fn gun_index_curl(
    side: Side,
    gun: &crate::weapons::layout::GunLayout,
    trigger: f32,
) -> f32 {
    let rest = hold_curl(Some(GripRole::Primary), 0.0, 0.0)[0];
    let Some(part) = gun
        .parts
        .iter()
        .find(|p| p.moves == crate::weapons::layout::Moves::Trigger)
    else {
        return hold_curl(Some(GripRole::Primary), trigger, 0.0)[0];
    };
    // The trigger in the unrolled glove's frame, where the index curls in
    // its own (y, z) plane.
    let in_hand = gun
        .grip
        .compute_affine()
        .inverse()
        .transform_point3(part.at.translation);
    let at = glove_frame(side)
        .compute_affine()
        .inverse()
        .transform_point3(in_hand);
    let goal = Vec2::new(at.y, at.z);
    let miss = |c: f32| index_miss(side, c, goal);
    let reach = (0..=100)
        .map(|i| i as f32 / 100.0)
        .min_by(|a, b| miss(*a).total_cmp(&miss(*b)))
        .unwrap_or(rest);
    rest + (reach - rest) * trigger
}

/// How far the index's last two segments, curled `c`, pass from `p` in the
/// unrolled glove's (y, z) plane: a trigger sits in the finger's crease or
/// under its pad, not at its very tip.
fn index_miss(side: Side, c: f32, p: Vec2) -> f32 {
    let root = finger_root(side, 0);
    let mut at = Vec2::new(root.y, root.z);
    let mut a = 0.0;
    let mut miss = f32::MAX;
    for (joint, (bend, len)) in JOINT_BEND.iter().zip(FINGER_BONES[0]).enumerate() {
        a += bend * c;
        let next = at - Vec2::new(a.sin(), a.cos()) * len;
        if joint > 0 {
            miss = miss.min(Segment2 { a: at, b: next }.distance(p));
        }
        at = next;
    }
    miss
}

#[derive(Clone, Copy)]
struct Segment2 {
    a: Vec2,
    b: Vec2,
}

impl Segment2 {
    fn distance(self, p: Vec2) -> f32 {
        let d = self.b - self.a;
        let t = ((p - self.a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
        (self.a + d * t).distance(p)
    }
}

/// How far each finger curls (index, middle, ring, little, thumb) round
/// what it holds by a point of `role`, with the trigger pulled `trigger`;
/// `thinnest` is the held thing's smallest size, for anything held free.
pub(crate) fn hold_curl(role: Option<GripRole>, trigger: f32, thinnest: f32) -> [f32; 5] {
    match role {
        // A pistol or rifle grip: the fingers round it, the index along
        // the frame until it takes up the trigger.
        Some(GripRole::Primary) => [0.22 + 0.18 * trigger, 0.74, 0.76, 0.78, 0.62],
        Some(GripRole::Support) => [0.55, 0.6, 0.62, 0.64, 0.5],
        Some(GripRole::Handle(_)) => [0.7, 0.72, 0.74, 0.76, 0.7],
        // Anything else: wrapped as far as its size lets the fingers
        // close (a round pinched, a can held round, a box flat-handed).
        _ => {
            let wrap = (0.95 - thinnest * 5.0).clamp(0.25, 0.9);
            let index = (wrap + 0.25 * trigger).min(0.95);
            [index, wrap, wrap, wrap, (wrap * 0.85).max(0.35)]
        }
    }
}

/// How far each finger wants to curl, given what the hand is doing.
#[allow(clippy::too_many_arguments)]
fn curl_targets(
    hand: &Hand,
    side: Side,
    grip: f32,
    trigger: f32,
    thumb_rest: bool,
    held: &Query<(&Held, &Transform, &Grabbable), (Without<HandVisual>, Without<Finger>)>,
    bounds: &Query<&avian3d::prelude::ColliderAabb>,
    guns: &Query<&crate::weapons::Gun>,
) -> [f32; 5] {
    // [index, middle, ring, little, thumb]
    if let Some(item) = hand.holding {
        let role = held
            .get(item)
            .ok()
            .and_then(|(h, _, g)| g.points.get(h.point).map(|p| p.role));
        let thinnest = bounds
            .get(item)
            .map(|b| {
                let s = b.max - b.min;
                s.x.min(s.y).min(s.z)
            })
            .unwrap_or(0.06);
        let mut curl = hold_curl(role, trigger, thinnest);
        if role == Some(GripRole::Primary) {
            if let Ok(gun) = guns.get(item) {
                curl[0] = gun_index_curl(side, &gun.layout, trigger);
            }
        }
        return curl;
    }
    if hand.supporting.is_some() {
        return hold_curl(Some(GripRole::Support), trigger, 0.0);
    }
    if hand.handle.is_some() {
        return [0.72, 0.74, 0.76, 0.78, 0.7];
    }
    // Empty: the grip closes the hand, the trigger the index finger; a
    // thumb resting on the controller's buttons lies across over them.
    let thumb = if thumb_rest {
        (grip * 0.8).max(0.5)
    } else {
        grip * 0.8
    };
    [grip.max(trigger), grip, grip, grip, thumb]
}

/// Hands sit on what they hold (so the glove wraps the grip even when the
/// gun is blocked by a wall), otherwise on the tracked pose.
fn pose_hands(
    time: Res<Time>,
    mut visuals: Query<(&mut HandVisual, &mut Transform, Option<&mut Visibility>), Without<Finger>>,
    mut fingers: Query<(&Finger, &ChildOf, &mut Transform), Without<HandVisual>>,
    hands: Query<(&Hand, &Transform, &ChildOf), (Without<HandVisual>, Without<Finger>)>,
    roots_t: Query<&Transform, (Without<Hand>, Without<HandVisual>, Without<Finger>)>,
    held: Query<(&Held, &Transform, &Grabbable), (Without<HandVisual>, Without<Finger>)>,
    bounds: Query<&avian3d::prelude::ColliderAabb>,
    inputs: Query<&RigInput>,
    roots: Query<&ChildOf, With<Finger>>,
    models: Query<&ChildOf, With<GloveModel>>,
    buttons: Query<&GlobalTransform, With<crate::interact::poke::PokeButton>>,
    guns: Query<&crate::weapons::Gun>,
) {
    let ease = 1.0 - (-time.delta_secs() * 18.0).exp();
    for (mut v, mut t, mut vis) in &mut visuals {
        let Ok((hand, local, parent)) = hands.get(v.hand) else {
            continue;
        };
        // The rig hand sits on the player's root; held things are loose in
        // the world: their transforms are this frame's world poses.
        let Ok(root) = roots_t.get(parent.parent()) else {
            continue;
        };
        let mut pose = root.mul_transform(*local);
        let input = inputs.get(parent.parent()).map(|i| i.hand(v.side));
        let grip = input.map_or(0.0, |i| i.grip);
        let trigger = input.map_or(0.0, |i| i.trigger);
        if let Some(item) = hand.holding {
            if let Ok((h, igt, g)) = held.get(item) {
                if let Some(p) = g.points.get(h.point).filter(|p| p.role != GripRole::Free) {
                    pose = igt.mul_transform(p.local);
                }
            }
        }
        // A wall stops the glove where the hand's body stopped; the ghost
        // stays on the real hand and shows only when the two are apart.
        let tracked = pose;
        if let Some(felt) = hand.felt.filter(|_| hand.holding.is_none() && !v.ghost) {
            pose = felt;
        }
        if let (true, Some(vis)) = (v.ghost, vis.as_mut()) {
            let apart = hand.felt.is_some_and(|f| {
                f.translation.distance(tracked.translation)
                    > crate::interact::physics_hands::GHOST_GAP
            });
            vis.set_if_neq(if apart && hand.holding.is_none() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
        *t = pose;
        let thumb_rest = input.is_ok_and(|i| i.thumb_rest);
        let mut target = curl_targets(
            hand, v.side, grip, trigger, thumb_rest, &held, &bounds, &guns,
        );
        let mut spread = std::array::from_fn(rest_spread);
        let free = hand.holding.is_none() && hand.handle.is_none() && hand.supporting.is_none();
        let tracked = input.ok().and_then(|i| i.fingers);
        // An empty hand reaching for a push button points at it, so the
        // finger the button feels is the one you see.
        let tip = pose.transform_point(crate::interact::poke::FINGERTIP);
        let mut thumb = None;
        if let Some(real) = tracked.filter(|_| free) {
            // Tracked fingers (Index controllers): a free hand is the real
            // one; what's held keeps the glove's wrap round it.
            target = real.curl;
            spread = real.splay;
            thumb = (real.thumb_dir != Vec3::ZERO).then_some((real.thumb_dir, real.thumb_bend));
        } else if free
            && buttons
                .iter()
                .any(|b| b.translation().distance_squared(tip) < POINT_REACH * POINT_REACH)
        {
            target = POINTING;
            target[0] += (1.0 - POINTING[0]) * trigger;
        }
        for (c, want) in v.curl.iter_mut().zip(target) {
            *c += (want - *c) * ease;
        }
        for (s, want) in v.spread.iter_mut().zip(spread) {
            *s += (want - *s) * ease;
        }
        v.thumb = thumb.map(|(dir, bend)| match v.thumb {
            Some((d, b)) => (d.lerp(dir, ease).normalize_or(dir), b + (bend - b) * ease),
            None => (dir, bend),
        });
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
        let Ok((v, _, _)) = visuals.get(up) else {
            continue;
        };
        let spread = v.spread.get(f.finger).copied().unwrap_or(0.0);
        ft.rotation = match (f.finger, f.joint, v.thumb) {
            // A tracked thumb points where the real one does (the glove's
            // unrolled frame is OpenXR's palm frame), pad toward the fingers.
            (4, 0, Some((dir, _))) => {
                Quat::from_rotation_arc(Vec3::NEG_Z, dir)
                    * Quat::from_rotation_z(0.6 * v.side.sign())
            }
            (4, _, Some((_, bend))) => Quat::from_rotation_x(-bend),
            _ => joint_rotation_spread(v.side, f.finger, f.joint, v.curl[f.finger], spread),
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

#[cfg(test)]
mod glove_tests {
    use super::*;

    /// The right wrist's display is the left's mirrored across the hand's
    /// X (a left-handed player's watch), its text turned so it reads the
    /// right way round, not as a mirror image.
    #[test]
    fn the_right_wrist_display_is_the_left_mirrored_and_reads() {
        let m = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        let (l, r) = (wrist_mount(Side::Left), wrist_mount(Side::Right));
        let close = |a: Vec3, b: Vec3| a.distance(b) < 1e-4;
        assert!(close(r.translation, m(l.translation)), "{r:?} vs {l:?}");
        // Facing out of the back of the hand, the same way up...
        assert!(close(r.rotation * Vec3::Z, m(l.rotation * Vec3::Z)));
        assert!(close(r.rotation * Vec3::Y, m(l.rotation * Vec3::Y)));
        // ...and reading left to right, not mirrored.
        assert!(close(r.rotation * Vec3::X, -m(l.rotation * Vec3::X)));
    }

    /// A node of a .glb: its name, local transform, children and, for a
    /// mesh, its POSITION bounds.
    struct Node {
        name: String,
        local: Transform,
        children: Vec<usize>,
        bounds: Option<(Vec3, Vec3)>,
    }

    fn glb_nodes(side: Side) -> Vec<Node> {
        let path = crate::asset_root_path().join(glove_model(side));
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (run art/build.py gloves)", path.display()));
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        let v3 = |v: &serde_json::Value, d: Vec3| {
            v.as_array().map_or(d, |a| {
                Vec3::new(
                    a[0].as_f64().unwrap() as f32,
                    a[1].as_f64().unwrap() as f32,
                    a[2].as_f64().unwrap() as f32,
                )
            })
        };
        json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                let r = n["rotation"].as_array().map_or(Quat::IDENTITY, |a| {
                    let f = |i: usize| a[i].as_f64().unwrap() as f32;
                    Quat::from_xyzw(f(0), f(1), f(2), f(3))
                });
                let bounds = n["mesh"].as_u64().and_then(|m| {
                    let prims = json["meshes"][m as usize]["primitives"].as_array()?;
                    let mut b: Option<(Vec3, Vec3)> = None;
                    for p in prims {
                        let acc =
                            &json["accessors"][p["attributes"]["POSITION"].as_u64()? as usize];
                        let (lo, hi) = (v3(&acc["min"], Vec3::ZERO), v3(&acc["max"], Vec3::ZERO));
                        b = Some(b.map_or((lo, hi), |(a, c)| (a.min(lo), c.max(hi))));
                    }
                    b
                });
                Node {
                    name: n["name"].as_str().unwrap_or_default().to_owned(),
                    local: Transform::from_translation(v3(&n["translation"], Vec3::ZERO))
                        .with_rotation(r),
                    children: n["children"]
                        .as_array()
                        .map(|c| c.iter().map(|i| i.as_u64().unwrap() as usize).collect())
                        .unwrap_or_default(),
                    bounds,
                }
            })
            .collect()
    }

    /// Both gloves have the palm and every finger segment, each with a mesh,
    /// hung in a chain at the joints `spawn_hand` makes: the game puts each
    /// segment on its own joint, so a model built to other joints would
    /// come apart at the knuckles.
    #[test]
    fn every_glove_segment_is_modelled_at_its_joint() {
        for side in [Side::Left, Side::Right] {
            let nodes = glb_nodes(side);
            let find = |name: &str| {
                nodes
                    .iter()
                    .position(|n| n.name == name)
                    .unwrap_or_else(|| panic!("{side:?} glove has no {name}"))
            };
            let palm = find("palm");
            assert!(nodes[palm].bounds.is_some(), "{side:?} palm has no mesh");
            for (finger, bones) in FINGER_BONES.iter().enumerate() {
                let mut parent = palm;
                for (joint, _) in bones.iter().enumerate().filter(|(_, l)| **l > 0.0) {
                    let name = format!("finger_{finger}_{joint}");
                    let i = find(&name);
                    let n = &nodes[i];
                    assert!(n.bounds.is_some(), "{side:?} {name} has no mesh");
                    assert!(
                        nodes[parent].children.contains(&i),
                        "{side:?} {name} isn't hung on {}",
                        nodes[parent].name
                    );
                    let want = if joint == 0 {
                        finger_root(side, finger)
                    } else {
                        Vec3::new(0.0, 0.0, -bones[joint - 1])
                    };
                    assert!(
                        n.local.translation.distance(want) < 2e-4,
                        "{side:?} {name} at {} but its joint is at {want}",
                        n.local.translation
                    );
                    let rest = joint_rotation(side, finger, joint, 0.0);
                    assert!(
                        n.local.rotation.dot(rest).abs() > 0.9999,
                        "{side:?} {name} rests turned {:?}, its joint {rest:?}",
                        n.local.rotation
                    );
                    parent = i;
                }
            }
        }
    }

    /// The modelled index finger, pointing, ends where a push button feels
    /// the fingertip (`poke::FINGERTIP`): the button goes in when the glove
    /// touches it, not a centimetre before or after.
    #[test]
    fn pointing_index_ends_on_the_poke_fingertip() {
        for side in [Side::Left, Side::Right] {
            let nodes = glb_nodes(side);
            let (lo, hi) = nodes
                .iter()
                .find(|n| n.name == "finger_0_2")
                .and_then(|n| n.bounds)
                .expect("index tip");
            // The segment's far end, on its axis.
            let tip = Vec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, lo.z);
            let mut at = glove_frame(side);
            let mut offset = finger_root(side, 0);
            for joint in 0..3 {
                at = at
                    * Transform::from_translation(offset).with_rotation(joint_rotation(
                        side,
                        0,
                        joint,
                        POINTING[0],
                    ));
                offset = Vec3::new(0.0, 0.0, -FINGER_BONES[0][joint]);
            }
            let got = at.transform_point(tip);
            let want = crate::interact::poke::FINGERTIP;
            assert!(
                got.distance(want) < 0.0015,
                "{side:?} index tip at {got}, the poke feels {want}"
            );
        }
    }

    /// An empty hand whose fingertip comes near a push button points at it
    /// (and still closes its index with the trigger); away from buttons the
    /// grip closes the whole hand as before.
    #[test]
    fn an_empty_hand_points_at_a_near_button() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(11),
            ))
            .add_systems(Update, pose_hands);
        let player = app
            .world_mut()
            .spawn((Transform::default(), RigInput::default()))
            .id();
        let hand = app
            .world_mut()
            .spawn((Hand::default(), Transform::default(), ChildOf(player)))
            .id();
        let glove = app
            .world_mut()
            .spawn((
                HandVisual {
                    hand,
                    side: Side::Right,
                    curl: [0.0; 5],
                    spread: std::array::from_fn(rest_spread),
                    thumb: None,
                    ghost: false,
                },
                Transform::default(),
            ))
            .id();
        let button = app
            .world_mut()
            .spawn((
                crate::interact::poke::PokeButton {
                    target: Entity::PLACEHOLDER,
                    depth: 0.0,
                    down: false,
                },
                GlobalTransform::from_translation(
                    crate::interact::poke::FINGERTIP + Vec3::Z * -0.05,
                ),
            ))
            .id();
        let settle = |app: &mut App| {
            for _ in 0..200 {
                app.update();
            }
            app.world().get::<HandVisual>(glove).unwrap().curl
        };
        app.world_mut()
            .get_mut::<RigInput>(player)
            .unwrap()
            .hand_mut(Side::Right)
            .grip = 1.0;
        let near = settle(&mut app);
        for (got, want) in near.iter().zip(POINTING) {
            assert!(
                (got - want).abs() < 0.05,
                "by a button the hand curls {near:?}, not {POINTING:?}"
            );
        }
        app.world_mut()
            .get_mut::<GlobalTransform>(button)
            .unwrap()
            .clone_from(&GlobalTransform::from_translation(Vec3::new(
                0.0, 0.0, -1.0,
            )));
        let far = settle(&mut app);
        assert!(
            far[0] > 0.9,
            "away from buttons the grip closes the index too: {far:?}"
        );
    }

    /// With the trigger pulled, the index of the hand on each gun's grip
    /// comes down on that gun's own trigger.
    #[test]
    fn a_pulled_index_meets_each_guns_trigger() {
        use crate::weapons::layout::{Moves, layout};
        for id in ["pm_pistol", "ak74", "mosin", "toz_double"] {
            let gun = layout(id).unwrap();
            let trigger = gun
                .parts
                .iter()
                .find(|p| p.moves == Moves::Trigger)
                .unwrap();
            let c = gun_index_curl(Side::Right, &gun, 1.0);
            // The trigger in the unrolled glove, against the curled index
            // (about 8 mm in radius), in its plane: how high the trigger sits
            // is the grip's business, not the finger's.
            let in_hand = gun
                .grip
                .compute_affine()
                .inverse()
                .transform_point3(trigger.at.translation);
            let at = glove_frame(Side::Right)
                .compute_affine()
                .inverse()
                .transform_point3(in_hand);
            let miss = index_miss(Side::Right, c, Vec2::new(at.y, at.z));
            assert!(miss < 0.012, "{id}: the index misses the trigger by {miss}");
        }
    }

    /// Tracked fingers drive a free glove, curl and spread; a held gun
    /// keeps the glove's wrap round its grip.
    #[test]
    fn tracked_fingers_drive_a_free_hand_not_a_held_gun() {
        use crate::rig::fingers::Fingers;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(11),
            ))
            .add_systems(Update, pose_hands);
        let player = app
            .world_mut()
            .spawn((Transform::default(), RigInput::default()))
            .id();
        let hand = app
            .world_mut()
            .spawn((Hand::default(), Transform::default(), ChildOf(player)))
            .id();
        let glove = app
            .world_mut()
            .spawn((
                HandVisual {
                    hand,
                    side: Side::Right,
                    curl: [0.0; 5],
                    spread: std::array::from_fn(rest_spread),
                    thumb: None,
                    ghost: false,
                },
                Transform::default(),
            ))
            .id();
        let real = Fingers {
            curl: [0.1, 0.6, 0.8, 0.9, 0.3],
            splay: [0.15, 0.0, -0.1, -0.2],
            thumb_dir: Vec3::new(0.5, -0.4, -0.77).normalize(),
            thumb_bend: 0.4,
        };
        app.world_mut()
            .get_mut::<RigInput>(player)
            .unwrap()
            .hand_mut(Side::Right)
            .fingers = Some(real);
        let settle = |app: &mut App| {
            for _ in 0..200 {
                app.update();
            }
            let v = app.world().get::<HandVisual>(glove).unwrap();
            (v.curl, v.spread, v.thumb)
        };
        let (curl, spread, thumb) = settle(&mut app);
        let (dir, bend) = thumb.expect("a tracked thumb");
        assert!(
            dir.distance(real.thumb_dir) < 0.01 && (bend - real.thumb_bend).abs() < 0.01,
            "free thumb {dir} {bend}"
        );
        for f in 0..5 {
            assert!((curl[f] - real.curl[f]).abs() < 0.02, "free curl {curl:?}");
        }
        for f in 0..4 {
            assert!(
                (spread[f] - real.splay[f]).abs() < 0.02,
                "free spread {spread:?}"
            );
        }
        // Now holding a pistol by its grip: the wrap, whatever the fingers do.
        let gun = app
            .world_mut()
            .spawn((
                Grabbable {
                    points: vec![crate::interact::GrabPoint {
                        local: Transform::IDENTITY,
                        role: GripRole::Primary,
                        radius: 0.1,
                    }],
                },
                Held {
                    player,
                    hand,
                    side: Side::Right,
                    point: 0,
                    offset: Transform::IDENTITY,
                },
                Transform::default(),
            ))
            .id();
        app.world_mut().get_mut::<Hand>(hand).unwrap().holding = Some(gun);
        let (curl, _, thumb) = settle(&mut app);
        assert!(thumb.is_none(), "a held gun's thumb wraps it, not tracked");
        let wrap = hold_curl(Some(GripRole::Primary), 0.0, 0.0);
        for f in 0..5 {
            assert!(
                (curl[f] - wrap[f]).abs() < 0.02,
                "held curl {curl:?} vs {wrap:?}"
            );
        }
    }
}
