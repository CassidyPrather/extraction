//! Logistics visuals: the cargo pod and the cable it hangs on, the
//! grappler's harpoon launcher, and the line from a grappler to whoever it
//! has speared.

use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use crate::models::{ModelMaterials, ModelNode, ModelState, Models, model_nodes, spawn_primitives};
use crate::render::materials::MaterialLibrary;
use crate::robots::dirigible::{CargoPod, Dirigible, RADIUS};
use crate::robots::grappler::{Grappler, Tethered};

/// Models (see `art/robots/logistics.py`); without them, primitives.
const POD_MODEL: &str = "models/robots/cargo_pod.glb";
const HARPOON_MODEL: &str = "models/robots/harpoon.glb";

/// A pod or grappler waiting for its model to load (or fail).
#[derive(Component)]
struct Undressed(&'static str);

pub struct LogisticsVisualsPlugin;

impl Plugin for LogisticsVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (dress_pods, dress_grapplers, finish_dressing).chain(),
                cables,
                tether_lines,
            ),
        );
    }
}

fn mat(
    materials: &mut Assets<StandardMaterial>,
    c: Color,
    metal: f32,
    rough: f32,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: c,
        metallic: metal,
        perceptual_roughness: rough,
        ..default()
    })
}

fn dress_pods(
    mut commands: Commands,
    pods: Query<Entity, Added<CargoPod>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
) {
    for e in &pods {
        models.request(POD_MODEL, &server);
        commands.entity(e).insert(Undressed(POD_MODEL));
    }
}

fn dress_grapplers(
    mut commands: Commands,
    grapplers: Query<Entity, Added<Grappler>>,
    server: Res<AssetServer>,
    mut models: ResMut<Models>,
) {
    for e in &grapplers {
        models.request(HARPOON_MODEL, &server);
        commands.entity(e).insert(Undressed(HARPOON_MODEL));
    }
}

/// Hangs the loaded model's nodes on each waiting pod or grappler, or the
/// primitive shapes if there's no model.
fn finish_dressing(
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
    for (e, u) in &waiting {
        let model = match models.request(u.0, &server) {
            None => None,
            Some(h) => match Models::state(&h, &server, &gltfs) {
                ModelState::Loading => continue,
                ModelState::Failed => {
                    warn!("{} failed to load; using primitives", u.0);
                    None
                }
                ModelState::Ready(g) => Some(model_nodes(g, &nodes, &gltf_meshes)),
            },
        };
        commands.entity(e).remove::<Undressed>();
        let Some(model) = model else {
            if u.0 == POD_MODEL {
                pod_blockout(&mut commands, e, &mut meshes, &mut materials);
            } else {
                grappler_blockout(&mut commands, e, &mut meshes, &mut materials);
            }
            continue;
        };
        let mut mat = |name: Option<&str>| {
            model_mats.get(
                crate::robots::weak_points::paint(name.unwrap_or("Dark")),
                &mut lib,
                &server,
                &mut materials,
            )
        };
        // The boom first: the launcher head rides on it.
        let mut boom: Option<(Entity, Vec3)> = None;
        let mut order: Vec<&ModelNode> = model.iter().collect();
        order.sort_by_key(|n| n.name != "launcher");
        for node in order {
            // The launcher head hangs on the boom, at its offset from the
            // foot pin, so it folds with it.
            let (parent, at) = match (node.name.as_str(), boom) {
                ("launcher_head", Some((b, foot))) => (
                    b,
                    Transform {
                        translation: node.transform.translation - foot,
                        ..node.transform
                    },
                ),
                _ => (e, node.transform),
            };
            let holder = commands
                .spawn((at, Visibility::default(), ChildOf(parent)))
                .id();
            match node.name.as_str() {
                // The derrick's boom (its origin its foot pin) folds back
                // along the hull when the grappler's wrecked; the pedestal
                // and drum stand (`grappler_armour::Folds`).
                "launcher" => {
                    commands
                        .entity(holder)
                        .insert(crate::robots::grappler_armour::Folds);
                    boom = Some((holder, node.transform.translation));
                }
                // Salvage on their own (Heavy's table): hidden once taken.
                "launcher_head" => {
                    commands.entity(holder).insert(HarpoonPart {
                        grappler: e,
                        what: HarpoonPiece::LauncherHead,
                    });
                }
                "drum" => {
                    commands.entity(holder).insert(HarpoonPart {
                        grappler: e,
                        what: HarpoonPiece::Drum,
                    });
                }
                _ => {}
            }
            spawn_primitives(&mut commands, holder, node, &mut mat);
        }
    }
}

fn pod_blockout(
    commands: &mut Commands,
    e: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    {
        let shell = mat(materials, Color::srgb(0.84, 0.85, 0.86), 0.0, 0.35);
        let dark = mat(materials, Color::srgb(0.08, 0.08, 0.09), 0.7, 0.4);
        let glow = materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.4, 0.5),
            emissive: LinearRgba::rgb(0.5, 3.0, 4.0),
            ..default()
        });
        commands.entity(e).with_children(|c| {
            // An open-topped tub so you can see (and reach) what's in it.
            for (size, at, m) in [
                (
                    Vec3::new(1.3, 0.06, 1.8),
                    Vec3::new(0.0, -0.45, 0.0),
                    &shell,
                ),
                (
                    Vec3::new(0.06, 0.9, 1.8),
                    Vec3::new(-0.65, 0.0, 0.0),
                    &shell,
                ),
                (Vec3::new(0.06, 0.9, 1.8), Vec3::new(0.65, 0.0, 0.0), &shell),
                (Vec3::new(1.3, 0.9, 0.06), Vec3::new(0.0, 0.0, -0.9), &shell),
                (Vec3::new(1.3, 0.9, 0.06), Vec3::new(0.0, 0.0, 0.9), &shell),
                (
                    Vec3::new(1.36, 0.08, 0.08),
                    Vec3::new(0.0, 0.44, -0.9),
                    &dark,
                ),
                (
                    Vec3::new(1.36, 0.08, 0.08),
                    Vec3::new(0.0, 0.44, 0.9),
                    &dark,
                ),
                (
                    Vec3::new(1.32, 0.03, 0.06),
                    Vec3::new(0.0, 0.25, 0.92),
                    &glow,
                ),
            ] {
                c.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(size))),
                    MeshMaterial3d(m.clone()),
                    Transform::from_translation(at),
                ));
            }
        });
    }
}

/// A piece of a grappler's harpoon drawn as its own node, to salvage on
/// its own (Heavy's table): its holder, which the salvage hides once the
/// piece is taken (its Visibility). `grappler` is the robot it's on.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HarpoonPart {
    pub grappler: Entity,
    pub what: HarpoonPiece,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarpoonPiece {
    /// The launcher on the boom's head (with its harpoon): it folds with
    /// the boom on a wreck.
    LauncherHead,
    /// The line drum behind the yoke.
    Drum,
}

/// The dirigible's pod cable: steel, its weak-link shackle just above the
/// pod, a grip sleeve over it (`robots::lines`, #101).
#[derive(Component)]
struct Cable;

fn cables(
    mut commands: Commands,
    ships: Query<(Entity, &Dirigible, &GlobalTransform)>,
    mut cables: Query<
        (
            &mut crate::robots::lines::Line,
            &mut crate::robots::lines::LineSpan,
            &mut Visibility,
        ),
        With<Cable>,
    >,
    mut made: Local<Vec<(Entity, Entity)>>,
) {
    use crate::robots::lines::{Gauge, Line, LineSpan};
    made.retain(|(ship, cable)| {
        if ships.contains(*ship) {
            true
        } else {
            commands.entity(*cable).try_despawn();
            false
        }
    });
    for (e, d, gt) in &ships {
        let top = gt.transform_point(Vec3::new(0.0, -RADIUS - 1.1, 0.0));
        let len = d.cable.max(0.0);
        let span = LineSpan {
            from: top,
            to: top - Vec3::Y * len.max(0.01),
            slack: 0.0,
        };
        // The shackle 0.6 m over the pod's lifting eye, the sleeve a hand's
        // reach over that: where they are on a cable reeled any length.
        let from_end = |m: f32| (1.0 - m / len.max(0.01)).clamp(0.0, 1.0);
        let (link, grip) = (from_end(0.6), from_end(1.5));
        let shown = if len > 0.05 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        match made.iter().find(|(s, _)| *s == e) {
            None => {
                let c = commands
                    .spawn((
                        Name::new("Pod cable"),
                        Cable,
                        Line::new(Gauge::Cable).with_weak_link(link).with_grip(grip),
                        span,
                        Transform::default(),
                        shown,
                    ))
                    .id();
                made.push((e, c));
            }
            Some(&(_, c)) => {
                if let Ok((mut l, mut s, mut v)) = cables.get_mut(c) {
                    if *s != span {
                        *s = span;
                    }
                    if l.weak_link != Some(link) || l.grips.first() != Some(&grip) {
                        l.weak_link = Some(link);
                        l.grips = vec![grip];
                    }
                    v.set_if_neq(shown);
                }
            }
        }
    }
}

fn grappler_blockout(
    commands: &mut Commands,
    e: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    {
        let dark = mat(materials, Color::srgb(0.08, 0.08, 0.09), 0.7, 0.4);
        let warn = materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.6, 0.1),
            emissive: LinearRgba::rgb(3.0, 1.6, 0.2),
            ..default()
        });
        // A stubby launcher on top with a reel, and a yellow band.
        commands.entity(e).with_children(|c| {
            c.spawn((
                Mesh3d(
                    meshes.add(
                        Cylinder::new(0.06, 0.5)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                    ),
                ),
                MeshMaterial3d(dark.clone()),
                Transform::from_xyz(0.0, 0.48, -0.1),
            ));
            c.spawn((
                Mesh3d(
                    meshes.add(
                        Cylinder::new(0.13, 0.1)
                            .mesh()
                            .build()
                            .rotated_by(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ),
                ),
                MeshMaterial3d(dark),
                Transform::from_xyz(0.0, 0.44, 0.18),
            ));
            c.spawn((
                Mesh3d(meshes.add(Torus::new(0.38, 0.41))),
                MeshMaterial3d(warn),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
        });
    }
}

#[derive(Component)]
struct TetherLine(Entity);

fn tether_lines(
    mut commands: Commands,
    tethered: Query<(Entity, &Tethered, &GlobalTransform)>,
    robots: Query<&GlobalTransform>,
    mut lines: Query<(Entity, &TetherLine, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (e, line, mut t) in &mut lines {
        let Ok((_, teth, pgt)) = tethered.get(line.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let Ok(rgt) = robots.get(teth.by) else {
            continue;
        };
        // From the launcher's muzzle on its derrick.
        let a = rgt.transform_point(crate::robots::grappler::MUZZLE);
        let b = pgt.translation() + Vec3::Y * 1.1;
        let len = a.distance(b).max(0.01);
        *t = Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, (b - a) / len))
            .with_scale(Vec3::new(1.0, len, 1.0));
    }
    for (p, _, _) in &tethered {
        if lines.iter().any(|(_, l, _)| l.0 == p) {
            continue;
        }
        let m = mat(&mut materials, Color::srgb(0.9, 0.6, 0.1), 0.3, 0.6);
        commands.spawn((
            TetherLine(p),
            Mesh3d(meshes.add(Cylinder::new(0.008, 1.0))),
            MeshMaterial3d(m),
            Transform::default(),
            NotShadowCaster,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grappler's harpoon is dressed with its launcher head and line
    /// drum as their own pieces, tagged for the salvage (Heavy's table),
    /// the head riding on the boom (it folds with it): a model that drops
    /// either node, or a redesign that forgets them (#100), fails here.
    #[test]
    fn the_harpoon_dresses_its_head_and_drum_for_the_salvage() {
        let opts = crate::LaunchOptions {
            map: "training_yard".into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            gallery: Some("robots:grappler,0,12".into()),
            camera: Some((Vec3::new(0.0, 0.0, 30.0), 0.0, -1.0)),
            ..Default::default()
        };
        let mut app = crate::soak::drive::client_app(opts);
        for _ in 0..400 {
            app.update();
        }
        let w = app.world_mut();
        let grappler = w
            .query_filtered::<Entity, With<Grappler>>()
            .iter(w)
            .next()
            .expect("the line-up stood a grappler");
        let parts: Vec<(Entity, HarpoonPart)> = w
            .query::<(Entity, &HarpoonPart)>()
            .iter(w)
            .map(|(e, p)| (e, *p))
            .collect();
        for what in [HarpoonPiece::LauncherHead, HarpoonPiece::Drum] {
            assert!(
                parts
                    .iter()
                    .any(|(_, p)| p.grappler == grappler && p.what == what),
                "{what:?} dressed as its own piece: {parts:?}"
            );
        }
        let (head, _) = parts
            .iter()
            .find(|(_, p)| p.what == HarpoonPiece::LauncherHead)
            .unwrap();
        let on = w.get::<ChildOf>(*head).map(|c| c.parent());
        assert!(
            on.is_some_and(|b| w.get::<crate::robots::grappler_armour::Folds>(b).is_some()),
            "the head rides on the boom (it folds with it)"
        );
    }
}
