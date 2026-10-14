//! Bridges. A road's `bridges` mark where it crosses water; the terrain
//! grades a straight deck line between the two abutments
//! (`extraction_core::terrain`), and this builds the bridge on it: a deck
//! you walk on, rails either side, piers down to the bed. Road bridges are
//! concrete with iron rails, footpaths get a plank footbridge.
//!
//! Physics is part of the game (bots cross them too); the look is the
//! client's.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::RoadKind;

use super::{Layer, Zone};
use crate::render::materials::Mat;

/// One bridge: its pieces in world space.
#[derive(Component, Clone, Debug)]
pub struct Bridge {
    pub pieces: Vec<(Vec3, Transform, Mat)>,
}

/// Deck segment length, metres.
const SEG: f32 = 2.0;
const DECK: f32 = 0.3;
const RAIL: f32 = 1.0;

/// The pieces of every bridge on the map.
pub fn bridges(zone: &Zone) -> Vec<Bridge> {
    let mut out = Vec::new();
    for rl in &zone.hm.road_lines {
        let foot = matches!(rl.kind, RoadKind::Path);
        let (deck_mat, rail_mat, pier_mat) = if foot {
            (Mat::Planks, Mat::Timber, Mat::Timber)
        } else {
            (Mat::Concrete, Mat::RustyMetal, Mat::Concrete)
        };
        // Footbridges are wider than their path, roads a little wider than
        // the carriageway.
        let width = if foot { 1.8 } else { rl.width + 0.8 };
        // The grade is sampled per centreline point; find heights by arc
        // length, not by index (the samples aren't exactly a metre apart).
        let mut arc = vec![0.0f32; rl.points.len()];
        for i in 1..rl.points.len() {
            arc[i] = arc[i - 1] + extraction_core::geom::dist(rl.points[i - 1], rl.points[i]);
        }
        let height_at = |s: f32| {
            let k = arc
                .partition_point(|a| *a <= s)
                .clamp(1, arc.len().max(2) - 1);
            let (a0, a1) = (arc[k - 1], arc[k]);
            let t = ((s - a0) / (a1 - a0).max(1e-4)).clamp(0.0, 1.0);
            rl.heights[k - 1] + (rl.heights[k] - rl.heights[k - 1]) * t
        };
        for (a, b) in &rl.bridges {
            let mut pieces = Vec::new();
            let n = ((b - a) / SEG).ceil().max(1.0) as usize;
            for i in 0..n {
                let s0 = a + (b - a) * i as f32 / n as f32;
                let s1 = a + (b - a) * (i + 1) as f32 / n as f32;
                let p0 = extraction_core::geom::polyline_at(&rl.points, s0).0;
                let p1 = extraction_core::geom::polyline_at(&rl.points, s1).0;
                let (h0, h1) = (height_at(s0), height_at(s1));
                let a3 = Vec3::new(p0[0], h0, p0[1]);
                let b3 = Vec3::new(p1[0], h1, p1[1]);
                let along = b3 - a3;
                let len = along.length() + 0.04;
                // Heading, then the deck's pitch: no roll (a shortest-arc
                // rotation from +Z rolls the deck on anything but a
                // north-south bridge). Uprights take the heading only.
                let yaw = Quat::from_rotation_y(along.x.atan2(along.z));
                let run = Vec2::new(along.x, along.z).length();
                let rot = yaw * Quat::from_rotation_x(-along.y.atan2(run));
                let mid = (a3 + b3) * 0.5;
                let side = rot * Vec3::X;
                let up = rot * Vec3::Y;
                // Deck: its top on the grade line.
                pieces.push((
                    Vec3::new(width, DECK, len),
                    Transform::from_translation(mid - up * DECK * 0.5).with_rotation(rot),
                    deck_mat,
                ));
                // Rails and their posts.
                for sgn in [-1.0, 1.0] {
                    let edge = mid + side * sgn * (width * 0.5 - 0.04);
                    pieces.push((
                        Vec3::new(0.06, 0.06, len),
                        Transform::from_translation(edge + up * RAIL).with_rotation(rot),
                        rail_mat,
                    ));
                    pieces.push((
                        Vec3::new(0.06, 0.06, len),
                        Transform::from_translation(edge + up * RAIL * 0.5).with_rotation(rot),
                        rail_mat,
                    ));
                    pieces.push((
                        Vec3::new(0.08, RAIL, 0.08),
                        Transform::from_translation(
                            a3 + side * sgn * (width * 0.5 - 0.04) + Vec3::Y * RAIL * 0.5,
                        )
                        .with_rotation(yaw),
                        rail_mat,
                    ));
                }
                // A pier every few segments, down to the bed.
                if i > 0 && i % 4 == 0 {
                    let ground = zone.hm.height(p0);
                    let top = h0 - DECK;
                    let depth = top - ground + 0.6;
                    if depth > 0.4 {
                        pieces.push((
                            Vec3::new(width * 0.8, depth, 0.5),
                            Transform::from_translation(Vec3::new(p0[0], top - depth * 0.5, p0[1]))
                                .with_rotation(yaw),
                            pier_mat,
                        ));
                    }
                }
            }
            out.push(Bridge { pieces });
        }
    }
    out
}

/// The bridges, solid.
pub fn spawn_bridges(mut commands: Commands, zone: Res<Zone>) {
    for b in bridges(&zone) {
        let parts: Vec<(Vec3, Quat, Collider)> = b
            .pieces
            .iter()
            .map(|(size, t, _)| {
                (
                    t.translation,
                    t.rotation,
                    Collider::cuboid(size.x, size.y, size.z),
                )
            })
            .collect();
        let surfaces = super::surfaces::Surfaces(
            b.pieces
                .iter()
                .map(|(size, t, m)| (*size * 0.5, *t, *m))
                .collect(),
        );
        commands.spawn((
            Name::new("Bridge"),
            b,
            surfaces,
            Transform::IDENTITY,
            Visibility::default(),
            RigidBody::Static,
            Collider::compound(parts),
            CollisionLayers::new(Layer::World, LayerMask::ALL),
        ));
    }
}

/// The bridges, seen: one mesh per material.
pub fn bridge_visuals(
    mut commands: Commands,
    added: Query<(Entity, &Bridge), Added<Bridge>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut library: ResMut<crate::render::materials::MaterialLibrary>,
    server: Res<AssetServer>,
) {
    for (e, b) in &added {
        let mut merged: Vec<(Mat, Mesh)> = Vec::new();
        for (size, t, mat) in &b.pieces {
            let mut mesh = crate::render::materials::metric_box(*size);
            let _ = mesh.generate_tangents();
            let mesh = mesh.transformed_by(*t);
            match merged.iter_mut().find(|(m, _)| m == mat) {
                Some((_, into)) => {
                    let _ = into.merge(&mesh);
                }
                None => merged.push((*mat, mesh)),
            }
        }
        for (m, mesh) in merged {
            let mat = library.get(m, &server, &mut materials);
            commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(mat), ChildOf(e)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every bridge on every map crosses water, lands on dry ground at both
    /// ends, and keeps its deck above the water all the way.
    #[test]
    fn bridges_carry_you_over_the_water() {
        let mut bad = Vec::new();
        for map in crate::test_maps() {
            let zone = Zone::load(&map);
            for rl in &zone.hm.road_lines {
                for (a, b) in &rl.bridges {
                    let at = |s: f32| extraction_core::geom::polyline_at(&rl.points, s).0;
                    let name = format!("{map} bridge at {:.0?}", at((a + b) * 0.5));
                    for end in [*a - 1.0, *b + 1.0] {
                        if zone.hm.water_level(at(end)).is_some() {
                            bad.push(format!("{name}: an end is in the water"));
                        }
                    }
                    let mut wet = false;
                    let mut s = *a;
                    while s < *b {
                        if let Some(w) = zone.hm.water_level(at(s)) {
                            wet = true;
                            let k = (s as usize).min(rl.heights.len() - 1);
                            if rl.heights[k] < w + 0.2 {
                                bad.push(format!("{name}: deck under water at {s:.0}"));
                            }
                        }
                        s += 1.0;
                    }
                    if !wet {
                        bad.push(format!("{name}: crosses no water"));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
    }
}
