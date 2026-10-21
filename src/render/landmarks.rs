//! Lamps on the landmarks. Everything tall enough to steer by
//! (`map_lint::LANDMARK_HEIGHT`) carries a steady red lamp at its highest
//! point, as masts and chimneys do: the wayfinding gauge
//! (`render::wayfind_shots`) found the valley's landmarks gone in drizzle
//! past about 200 m and all but two gone at night, so a rendezvous had
//! nothing to steer by. A lamp a few pixels across outshines the fog and
//! the dark where the structure's own few percent of contrast doesn't.
//! Steady and red: the drones' beacons are amber and blink.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use extraction_core::map::prefab_footprint;
use extraction_core::map_lint::LANDMARK_HEIGHT;

use crate::world::buildings::{Building, Shape, prefab};

pub struct LandmarkLampsPlugin;

impl Plugin for LandmarkLampsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, light_landmarks);
    }
}

/// Prefabs whose models carry a lamp bright enough to steer by (the radio
/// mast's own is too small to carry past 150 m at night: it gets one too).
const LIT: &[&str] = &[];

/// The lamp's radius, and its halo's (m).
const LAMP_R: f32 = 0.35;
const HALO_R: f32 = 1.4;

/// The highest point of a prefab, in its own frame: the top middle of its
/// highest piece.
pub fn top_of(id: &str) -> Option<Vec3> {
    let geo = prefab(id)?;
    geo.pieces
        .iter()
        .map(|p| {
            let half_h = match p.shape {
                Shape::Box(s) | Shape::Prism(s) => s.y * 0.5,
                Shape::Cylinder(_, h) => h * 0.5,
            };
            p.at.translation + p.at.rotation * Vec3::Y * half_h
        })
        .max_by(|a, b| a.y.total_cmp(&b.y))
}

fn light_landmarks(
    mut commands: Commands,
    buildings: Query<(Entity, &Building), Added<Building>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<
        Option<(
            Handle<Mesh>,
            Handle<StandardMaterial>,
            Handle<Mesh>,
            Handle<StandardMaterial>,
        )>,
    >,
) {
    for (e, b) in &buildings {
        if prefab_footprint(&b.prefab)[1] < LANDMARK_HEIGHT || LIT.contains(&b.prefab.as_str()) {
            continue;
        }
        let Some(top) = top_of(&b.prefab) else {
            continue;
        };
        let (lamp_mesh, lamp, halo_mesh, halo) = made
            .get_or_insert_with(|| {
                (
                    meshes.add(Sphere::new(LAMP_R).mesh().uv(12, 8)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.5, 0.02, 0.01),
                        emissive: LinearRgba::rgb(120.0, 6.0, 2.0),
                        ..default()
                    }),
                    meshes.add(Sphere::new(HALO_R).mesh().uv(12, 8)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.1, 0.05, 0.18),
                        emissive: LinearRgba::rgb(3.0, 0.15, 0.05),
                        unlit: true,
                        alpha_mode: AlphaMode::Add,
                        ..default()
                    }),
                )
            })
            .clone();
        let at = top + Vec3::Y * (LAMP_R + 0.05);
        commands.entity(e).with_children(|c| {
            c.spawn((
                Name::new("Landmark lamp"),
                Mesh3d(lamp_mesh),
                MeshMaterial3d(lamp),
                Transform::from_translation(at),
                NotShadowCaster,
            ));
            c.spawn((
                Mesh3d(halo_mesh),
                MeshMaterial3d(halo),
                Transform::from_translation(at),
                NotShadowCaster,
            ));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every landmark the valleys place has a top to carry a lamp, near the
    /// height it's listed at.
    #[test]
    fn every_landmark_has_a_top_for_its_lamp() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/maps");
        let mut bad = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let text = std::fs::read_to_string(e.path()).unwrap();
            let Ok(map) = extraction_core::map::MapDef::from_ron(&text) else {
                continue;
            };
            for p in &map.placements {
                let h = prefab_footprint(&p.prefab)[1];
                if h < LANDMARK_HEIGHT || LIT.contains(&p.prefab.as_str()) {
                    continue;
                }
                if !seen.insert(p.prefab.clone()) {
                    continue;
                }
                match top_of(&p.prefab) {
                    None => bad.push(format!("{}: no geometry", p.prefab)),
                    Some(t) if t.y < h * 0.6 => bad.push(format!(
                        "{}: top at {:.1} m, listed {h:.1} m",
                        p.prefab, t.y
                    )),
                    _ => {}
                }
            }
        }
        assert!(!seen.is_empty());
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }
}
