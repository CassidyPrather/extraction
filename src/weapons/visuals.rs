//! Gun visuals: the layout's parts as meshes, with the moving ones (slide,
//! bolt, barrels, trigger, selector) animated from the mechanism.

use bevy::prelude::*;
use extraction_core::firearm::FireMode;

use super::Gun;
use super::layout::{Mat, Moves, Shape};
use crate::items::Item;

pub struct GunVisualsPlugin;

impl Plugin for GunVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (build_gun_meshes, animate_parts)
                .chain()
                .before(TransformSystems::Propagate),
        )
        .add_systems(Update, super::casing_visuals);
    }
}

/// A visual part of a gun and its rest pose.
#[derive(Component)]
pub struct GunPart {
    pub moves: Moves,
    pub rest: Transform,
}

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

fn build_gun_meshes(
    mut commands: Commands,
    guns: Query<(Entity, &Gun), (Added<Gun>, With<Item>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut palette: Local<Palette>,
) {
    for (e, gun) in &guns {
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
                },
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                part.at,
                ChildOf(e),
            ));
        }
    }
}

fn animate_parts(guns: Query<(&Gun, &Children)>, mut parts: Query<(&GunPart, &mut Transform)>) {
    for (gun, children) in &guns {
        let f = &gun.firearm;
        let travel = gun.layout.handle.as_ref().map_or(0.0, |h| h.travel);
        for child in children.iter() {
            let Ok((part, mut t)) = parts.get_mut(child) else {
                continue;
            };
            *t = match part.moves {
                Moves::Static => part.rest,
                Moves::WithBolt => {
                    let mut r = part.rest;
                    r.translation.z += f.bolt_travel * travel;
                    r
                }
                Moves::TurnBolt => {
                    // Rotate the handle up around the bore, then slide back.
                    let bore = Vec3::new(0.0, 0.064, 0.0);
                    let turn = Quat::from_rotation_z(f.handle_lift * std::f32::consts::FRAC_PI_2);
                    let mut r = part.rest;
                    let rel = r.translation - bore;
                    r.translation = bore + turn * Vec3::new(rel.x, rel.y, 0.0) + Vec3::Z * rel.z;
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
        }
    }
}
