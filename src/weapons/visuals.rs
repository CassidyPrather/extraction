//! Gun visuals: the modelled gun from `assets/models/guns/<id>.glb` (see
//! `art/guns/`), or the layout's blockout parts when there is no model,
//! with the moving parts (slide, bolt, barrels, trigger, selector) animated
//! from the mechanism.
//!
//! A model's top-level objects are named for how they move (`static*`,
//! `bolt*`, `turnbolt*`, `barrels*`, `trigger*`, `selector*`); each one's
//! origin is where it moves from. An empty named `magwell` is where the
//! model's magazine really seats, which magazines are drawn at (the
//! layout's seat stays the gameplay one).

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use extraction_core::firearm::{FireMode, Round};

use super::feel::Handling;
use super::layout::{Mat, Moves, Shape};
use super::{Gun, Mag};
use crate::interact::{InSocket, SocketOwner};
use crate::items::{Catalog, Item, ItemModelRoot};
use crate::models::{ModelMaterials, ModelState, Models, model_nodes, spawn_primitives};
use crate::render::materials::MaterialLibrary;

pub struct GunVisualsPlugin;

impl Plugin for GunVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (
                build_gun_meshes,
                finish_gun_models,
                animate_parts,
                seat_mag_visuals,
                show_mag_rounds,
                show_chambers,
            )
                .chain()
                .before(TransformSystems::Propagate),
        )
        .add_systems(
            Update,
            (
                super::casing_visuals,
                super::feel::jam_visuals,
                super::muzzle::muzzle_fx,
                super::grenades::show_grenade_parts,
                super::grenades::explosion_fx,
            ),
        )
        .add_plugins((
            super::impacts::ImpactFxPlugin,
            super::sling::SlingPlugin,
            super::tracers::TracerPlugin,
        ));
    }
}

/// A visual part of a gun and its rest pose.
#[derive(Component)]
pub struct GunPart {
    pub moves: Moves,
    pub rest: Transform,
    /// For a turn-bolt: a point on the axis it turns about (x, y used).
    pub pivot: Vec3,
}

/// A gun whose model file is still loading.
#[derive(Component)]
struct PendingGunModel(Handle<Gltf>);

/// Where the gun's model seats a magazine (its `magwell` empty), in gun
/// space.
#[derive(Component, Clone, Copy)]
pub struct ModelMagSeat(pub Transform);

/// The blockout's turn-bolt axis (its bolt is drawn above the bore).
const BLOCKOUT_BOLT_AXIS: Vec3 = Vec3::new(0.0, 0.064, 0.0);

#[derive(Default)]
struct Palette {
    mats: std::collections::HashMap<u8, Handle<StandardMaterial>>,
}

fn material(mat: Mat) -> StandardMaterial {
    let (c, metal, rough) = match mat {
        Mat::Steel => (Color::srgb(0.32, 0.32, 0.33), 1.0, 0.42),
        Mat::Blued => (Color::srgb(0.07, 0.075, 0.085), 0.9, 0.38),
        Mat::Wood => (Color::srgb(0.36, 0.17, 0.08), 0.0, 0.5),
        Mat::Polymer => (Color::srgb(0.27, 0.1, 0.11), 0.0, 0.55),
        Mat::Bakelite => (Color::srgb(0.2, 0.08, 0.05), 0.0, 0.45),
        Mat::Black => (Color::srgb(0.03, 0.03, 0.03), 0.6, 0.5),
    };
    StandardMaterial {
        base_color: c,
        metallic: metal,
        perceptual_roughness: rough,
        ..default()
    }
}

fn moves_for(name: &str) -> Option<Moves> {
    // `turnbolt` before `bolt`: neither is a prefix of the other, but keep
    // the more specific names first anyway.
    [
        ("turnbolt", Moves::TurnBolt),
        ("bolt", Moves::WithBolt),
        ("barrels", Moves::Barrels),
        ("trigger", Moves::Trigger),
        ("selector", Moves::Selector),
        ("hammer", Moves::Hammer),
        ("cylinder", Moves::Cylinder),
        ("gate", Moves::Gate),
        ("ejector", Moves::Rod),
        ("bayonet", Moves::Bayonet),
        ("static", Moves::Static),
    ]
    .into_iter()
    .find(|(p, _)| name.starts_with(p))
    .map(|(_, m)| m)
}

fn build_gun_meshes(
    mut commands: Commands,
    guns: Query<(Entity, &Gun, &Item), Added<Gun>>,
    catalog: Res<Catalog>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut palette: Local<Palette>,
) {
    for (e, gun, item) in &guns {
        let id = &catalog.get(item.kind()).id;
        if let Some(h) = models.request(&format!("models/guns/{id}.glb"), &server) {
            commands.entity(e).insert(PendingGunModel(h));
            continue;
        }
        build_blockout(
            &mut commands,
            e,
            gun,
            &mut meshes,
            &mut materials,
            &mut palette,
        );
    }
}

fn build_blockout(
    commands: &mut Commands,
    e: Entity,
    gun: &Gun,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    palette: &mut Palette,
) {
    for part in &gun.layout.parts {
        let mesh = match part.shape {
            Shape::Box(s) => meshes.add(Cuboid::from_size(s)),
            Shape::Cylinder(r, l) => meshes.add(
                Cylinder::new(r, l)
                    .mesh()
                    .resolution(14)
                    .build()
                    .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            ),
        };
        let mat = palette
            .mats
            .entry(part.mat as u8)
            .or_insert_with(|| materials.add(material(part.mat)))
            .clone();
        commands.spawn((
            GunPart {
                moves: part.moves,
                rest: part.at,
                pivot: BLOCKOUT_BOLT_AXIS,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            part.at,
            ChildOf(e),
        ));
    }
}

fn finish_gun_models(
    mut commands: Commands,
    pending: Query<(Entity, &Gun, &PendingGunModel)>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut model_mats: ResMut<ModelMaterials>,
    mut lib: ResMut<MaterialLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut palette: Local<Palette>,
) {
    for (e, gun, p) in &pending {
        let gltf = match Models::state(&p.0, &server, &gltfs) {
            ModelState::Loading => continue,
            ModelState::Failed => {
                warn!("gun model failed to load; drawing the blockout");
                commands.entity(e).remove::<PendingGunModel>();
                build_blockout(
                    &mut commands,
                    e,
                    gun,
                    &mut meshes,
                    &mut materials,
                    &mut palette,
                );
                continue;
            }
            ModelState::Ready(g) => g,
        };
        commands.entity(e).remove::<PendingGunModel>();
        for node in model_nodes(gltf, &nodes, &gltf_meshes) {
            if node.name == "magwell" {
                commands.entity(e).insert(ModelMagSeat(node.transform));
                continue;
            }
            let Some(moves) = moves_for(&node.name) else {
                continue;
            };
            if node.primitives.is_empty() {
                continue;
            }
            let part = commands
                .spawn((
                    Name::new(node.name.clone()),
                    GunPart {
                        moves,
                        rest: node.transform,
                        // Model objects sit with their origin on their axis.
                        pivot: node.transform.translation,
                    },
                    node.transform,
                    Visibility::default(),
                    ChildOf(e),
                ))
                .id();
            spawn_primitives(&mut commands, part, &node, |name| {
                model_mats.get(name.unwrap_or("Blued"), &mut lib, &server, &mut materials)
            });
        }
    }
}

/// How far back a cocked hammer tips, radians.
const HAMMER_BACK: f32 = 0.5;
/// How far a revolver's gate swings open, radians.
const GATE_OPEN: f32 = 1.4;
/// A revolver's ejector rod's stroke, metres.
const ROD_STROKE: f32 = 0.042;

fn animate_parts(
    guns: Query<(
        &Gun,
        &Children,
        Option<&Handling>,
        Option<&super::melee::Bayonet>,
        Option<&super::sling::SlingSwing>,
        &Item,
    )>,
    mut parts: Query<(&GunPart, &mut Transform)>,
    catalog: Res<Catalog>,
) {
    for (gun, children, handling, bayonet, swing, item) in &guns {
        // A slung gun's parts swing on its strap, about the rear swivel.
        let swung = swing
            .filter(|s| s.angle != Vec2::ZERO)
            .zip(super::sling::swivels(&catalog.get(item.kind()).id, gun))
            .map(|(s, (_, rear))| s.about(rear));
        let f = &gun.firearm;
        let travel = gun.layout.handle.as_ref().map_or(0.0, |h| h.travel);
        // A self-loader's carrier or slide runs back and home after a shot
        // (faster than the mechanism's state shows it).
        let stroke = handling.map_or(0.0, |h| {
            if h.cycle > 0.0 {
                (std::f32::consts::PI * (1.0 - h.cycle)).sin()
            } else {
                0.0
            }
        });
        let bolt = f.bolt_travel.max(stroke);
        for child in children.iter() {
            let Ok((part, mut t)) = parts.get_mut(child) else {
                continue;
            };
            *t = match part.moves {
                Moves::Static => part.rest,
                Moves::WithBolt => {
                    let mut r = part.rest;
                    r.translation.z += bolt * travel;
                    r
                }
                Moves::Cylinder => {
                    let n = f.chambers.len().max(1) as f32;
                    let turn = f.cylinder as f32 * std::f32::consts::TAU / n;
                    let mut r = part.rest;
                    r.rotation = Quat::from_rotation_z(-turn) * r.rotation;
                    r
                }
                Moves::Gate => {
                    let mut r = part.rest;
                    let open = if f.gate_open { GATE_OPEN } else { 0.0 };
                    r.rotation = Quat::from_rotation_y(open) * r.rotation;
                    r
                }
                Moves::Bayonet => {
                    // A self-loader's (the SKS's) folds; a bolt gun's is fixed.
                    let mut r = part.rest;
                    let folding = f.def.action == extraction_core::firearm::ActionKind::SemiAuto;
                    if folding && bayonet.is_some_and(|b| b.fixed) {
                        r.rotation = Quat::from_rotation_x(std::f32::consts::PI) * r.rotation;
                    }
                    r
                }
                Moves::Rod => {
                    let mut r = part.rest;
                    r.translation.z += f.rod * ROD_STROKE;
                    r
                }
                Moves::Hammer => {
                    // Back with the slide's stroke too, as the slide rides
                    // over it.
                    let back = if f.hammers_cocked.first() == Some(&true) {
                        1.0
                    } else {
                        stroke.min(1.0)
                    };
                    let mut r = part.rest;
                    r.rotation = Quat::from_rotation_x(HAMMER_BACK * back) * r.rotation;
                    r
                }
                Moves::TurnBolt => {
                    // Rotate the handle up around the bolt's axis, then slide back.
                    let axis = Vec3::new(part.pivot.x, part.pivot.y, 0.0);
                    let turn = Quat::from_rotation_z(f.handle_lift * std::f32::consts::FRAC_PI_2);
                    let mut r = part.rest;
                    let rel = r.translation - axis;
                    r.translation = axis + turn * Vec3::new(rel.x, rel.y, 0.0) + Vec3::Z * rel.z;
                    r.rotation = turn * r.rotation;
                    r.translation.z += f.bolt_travel * travel;
                    r
                }
                Moves::Barrels => {
                    let hinge = gun.layout.hinge;
                    let angle = -0.6 * f.bolt_travel;
                    let rot = Quat::from_rotation_x(angle);
                    let mut r = part.rest;
                    r.translation = hinge + rot * (r.translation - hinge);
                    r.rotation = rot * r.rotation;
                    r
                }
                Moves::Trigger => {
                    let mut r = part.rest;
                    r.translation.z += gun.trigger * 0.006;
                    r
                }
                Moves::Selector => {
                    let k = match f.fire_mode() {
                        FireMode::Safe => 0.0,
                        FireMode::Auto => 0.5,
                        FireMode::Semi => 1.0,
                    };
                    let mut r = part.rest;
                    r.rotation = Quat::from_rotation_x(-0.5 * k) * r.rotation;
                    r
                }
            };
            if let Some(sw) = swung {
                *t = sw * *t;
            }
        }
    }
}

/// A seated magazine is placed by the layout's seat (gameplay); its drawn
/// model goes where the gun model's own `magwell` says, so the two line up
/// whatever the layout's numbers are.
fn seat_mag_visuals(
    mags: Query<(&Mag, Option<&InSocket>, &Children)>,
    sockets: Query<&SocketOwner>,
    guns: Query<(&Gun, Option<&ModelMagSeat>)>,
    mut roots: Query<&mut Transform, With<ItemModelRoot>>,
) {
    for (mag, in_socket, children) in &mags {
        let fix = in_socket
            .and_then(|s| sockets.get(s.socket).ok())
            .and_then(|owner| guns.get(owner.0).ok())
            .and_then(|(gun, seat)| Some((gun.layout.magwell?, seat?.0)))
            .map(|(layout, model)| {
                // Mag space -> gun space is layout * hang; we want model * hang.
                let hang = Transform::from_xyz(0.0, -mag.height * 0.5, 0.0);
                let actual = layout.mul_transform(hang);
                let wanted = model.mul_transform(hang);
                Transform::from_matrix(Mat4::from(
                    actual.compute_affine().inverse() * wanted.compute_affine(),
                ))
            })
            .unwrap_or_default();
        for c in children.iter() {
            if let Ok(mut t) = roots.get_mut(c) {
                if *t != fix {
                    *t = fix;
                }
            }
        }
    }
}

/// A revolver model's `cylinder_round_N` / `cylinder_case_N` nodes: each
/// chamber shows the live round or the spent case it holds, or nothing.
fn show_chambers(guns: Query<(&Gun, &Children)>, mut nodes: Query<(&Name, &mut Visibility)>) {
    for (gun, children) in &guns {
        if gun.firearm.def.action != extraction_core::firearm::ActionKind::Revolver {
            continue;
        }
        for c in children.iter() {
            let Ok((name, mut vis)) = nodes.get_mut(c) else {
                continue;
            };
            let (want, n) = if let Some(n) = name.as_str().strip_prefix("cylinder_round_") {
                (Round::Live, n)
            } else if let Some(n) = name.as_str().strip_prefix("cylinder_case_") {
                (Round::Spent, n)
            } else {
                continue;
            };
            let Ok(n) = n.parse::<usize>() else {
                continue;
            };
            let shown = gun.firearm.chambers.get(n).copied().flatten() == Some(want);
            let v = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *vis != v {
                *vis = v;
            }
        }
    }
}

/// A magazine model's `round_N` nodes (the rounds you see in its lips, or
/// standing in a clip) show while it holds N rounds or more.
fn show_mag_rounds(
    mags: Query<(&Item, &Children), With<Mag>>,
    roots: Query<&Children, With<ItemModelRoot>>,
    mut nodes: Query<(&Name, &mut Visibility)>,
) {
    for (item, children) in &mags {
        for root in children.iter().filter_map(|c| roots.get(c).ok()) {
            for node in root.iter() {
                let Ok((name, mut vis)) = nodes.get_mut(node) else {
                    continue;
                };
                let Some(n) = name
                    .as_str()
                    .strip_prefix("round_")
                    .and_then(|n| n.parse::<u32>().ok())
                else {
                    continue;
                };
                let want = if item.rounds >= n {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use extraction_core::items::{BUILTIN_CATALOG, ItemCatalog, ItemState};

    /// A Nagant with a live round in chamber 0 and a spent case in 3
    /// shows exactly those.
    #[test]
    fn a_revolver_shows_what_its_chambers_hold() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let def = cat.get(cat.expect("nagant")).firearm.clone().unwrap();
        let mut firearm = extraction_core::firearm::Firearm::new(def, 1.0);
        firearm.chambers[0] = Some(Round::Live);
        firearm.chambers[3] = Some(Round::Spent);
        let mut app = App::new();
        app.add_systems(Update, show_chambers);
        let gun = app
            .world_mut()
            .spawn(Gun {
                firearm,
                layout: super::super::layout::layout("nagant").unwrap(),
                magwell: None,
                trigger: 0.0,
            })
            .id();
        let node = |app: &mut App, name: String| {
            app.world_mut()
                .spawn((Name::new(name), Visibility::default(), ChildOf(gun)))
                .id()
        };
        let live: Vec<Entity> = (0..7)
            .map(|n| node(&mut app, format!("cylinder_round_{n}")))
            .collect();
        let spent: Vec<Entity> = (0..7)
            .map(|n| node(&mut app, format!("cylinder_case_{n}")))
            .collect();
        app.update();
        let shown = |app: &App, es: &[Entity]| -> Vec<usize> {
            es.iter()
                .enumerate()
                .filter(|(_, e)| *app.world().get::<Visibility>(**e).unwrap() != Visibility::Hidden)
                .map(|(i, _)| i)
                .collect()
        };
        assert_eq!(shown(&app, &live), [0]);
        assert_eq!(shown(&app, &spent), [3]);
    }

    /// A clip with three rounds shows its first three, not the last two;
    /// emptied, it shows none.
    #[test]
    fn a_magazine_shows_the_rounds_it_holds() {
        let cat = ItemCatalog::from_ron(BUILTIN_CATALOG).unwrap();
        let mut state = ItemState::fresh(&cat, cat.expect("mosin_clip"), 1.0);
        state.rounds = 3;
        let mut app = App::new();
        app.add_systems(Update, show_mag_rounds);
        let mag = app
            .world_mut()
            .spawn((
                Item(state),
                Mag {
                    family: "mosin".into(),
                    capacity: 5,
                    height: 0.08,
                },
            ))
            .id();
        let root = app.world_mut().spawn((ItemModelRoot, ChildOf(mag))).id();
        let rounds: Vec<Entity> = (1..=5)
            .map(|n| {
                app.world_mut()
                    .spawn((
                        Name::new(format!("round_{n}")),
                        Visibility::default(),
                        ChildOf(root),
                    ))
                    .id()
            })
            .collect();
        let shown = |app: &App| -> Vec<bool> {
            rounds
                .iter()
                .map(|e| *app.world().get::<Visibility>(*e).unwrap() != Visibility::Hidden)
                .collect()
        };
        app.update();
        assert_eq!(shown(&app), [true, true, true, false, false]);
        app.world_mut().get_mut::<Item>(mag).unwrap().0.rounds = 0;
        app.update();
        assert_eq!(shown(&app), [false; 5]);
    }
}
