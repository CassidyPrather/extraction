//! Bridges. A road's `bridges` mark where it crosses water; the terrain
//! grades a straight deck line between the two abutments
//! (`extraction_core::terrain`), and this builds the bridge on it. Road
//! bridges are an asphalted concrete slab on girders, with kerbs, an iron
//! railing, piers of two columns under a cap, and abutments; footpaths get
//! a timber footbridge of boards across stringers, on trestles.
//!
//! Physics is part of the game (bots cross them too); the look is the
//! client's.

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::RoadKind;

use super::{Layer, Zone};
use crate::render::materials::Mat;

/// One piece of a bridge, in world space: a box, what it's made of, and
/// whether it's drawn, solid, or both (a footbridge's boards are drawn; one
/// plain deck under them takes the feet).
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub size: Vec3,
    pub at: Transform,
    pub mat: Mat,
    pub solid: bool,
    pub seen: bool,
}

/// One bridge: its pieces.
#[derive(Component, Clone, Debug)]
pub struct Bridge {
    pub pieces: Vec<Piece>,
}

/// Deck segment length, metres.
const SEG: f32 = 2.0;
const RAIL: f32 = 1.0;

/// The pieces of every bridge on the map.
pub fn bridges(zone: &Zone) -> Vec<Bridge> {
    let mut out = Vec::new();
    for rl in &zone.hm.road_lines {
        let foot = matches!(rl.kind, RoadKind::Path);
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
            let mut put = |size: Vec3, at: Transform, mat: Mat, solid: bool, seen: bool| {
                pieces.push(Piece {
                    size,
                    at,
                    mat,
                    solid,
                    seen,
                });
            };
            let n = ((b - a) / SEG).ceil().max(1.0) as usize;
            for i in 0..n {
                let s0 = a + (b - a) * i as f32 / n as f32;
                let s1 = a + (b - a) * (i + 1) as f32 / n as f32;
                let p0 = extraction_core::geom::polyline_at(&rl.points, s0).0;
                let p1 = extraction_core::geom::polyline_at(&rl.points, s1).0;
                let a3 = Vec3::new(p0[0], height_at(s0), p0[1]);
                let b3 = Vec3::new(p1[0], height_at(s1), p1[1]);
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
                let flat = |at: Vec3| Transform::from_translation(at).with_rotation(rot);
                let upright = |at: Vec3| Transform::from_translation(at).with_rotation(yaw);
                let ground = zone.hm.height(p0);
                if foot {
                    footbridge_segment(
                        &mut put, i, n, mid, a3, side, up, len, along, flat, upright, ground,
                    );
                } else {
                    road_segment(
                        &mut put, rl.width, i, n, mid, a3, side, up, len, flat, upright, ground,
                    );
                }
            }
            if !foot {
                // Abutments: a wall under each end, wing walls along the
                // bank either side.
                let w = rl.width + 0.8;
                for (s, inward) in [(*a, 1.0f32), (*b, -1.0)] {
                    let p = extraction_core::geom::polyline_at(&rl.points, s);
                    let (at, dir) = (p.0, Vec2::from_array(p.1).normalize_or(Vec2::Y));
                    let top = height_at(s) - 0.4;
                    let yaw = Quat::from_rotation_y(dir.x.atan2(dir.y));
                    let c =
                        Vec3::new(at[0], top, at[1]) + Vec3::new(dir.x, 0.0, dir.y) * 0.5 * inward;
                    put(
                        Vec3::new(w + 0.6, 3.0, 0.7),
                        Transform::from_translation(c - Vec3::Y * 1.5).with_rotation(yaw),
                        Mat::Concrete,
                        true,
                        true,
                    );
                    let across = yaw * Vec3::X;
                    for sgn in [-1.0, 1.0] {
                        put(
                            Vec3::new(0.4, 2.6, 3.2),
                            Transform::from_translation(
                                c + across * sgn * (w * 0.5 + 0.1)
                                    - Vec3::new(dir.x, 0.0, dir.y) * 1.2 * inward
                                    - Vec3::Y * 1.0,
                            )
                            .with_rotation(yaw),
                            Mat::Concrete,
                            true,
                            true,
                        );
                    }
                }
            }
            out.push(Bridge { pieces });
        }
    }
    out
}

/// A concrete road bridge's segment: an asphalt-surfaced slab on three
/// girders, kerbs, an iron railing on posts, and every fourth segment a
/// pier (two columns under a cap beam).
#[allow(clippy::too_many_arguments)]
fn road_segment(
    put: &mut impl FnMut(Vec3, Transform, Mat, bool, bool),
    road: f32,
    i: usize,
    n: usize,
    mid: Vec3,
    a3: Vec3,
    side: Vec3,
    up: Vec3,
    len: f32,
    flat: impl Fn(Vec3) -> Transform,
    upright: impl Fn(Vec3) -> Transform,
    ground: f32,
) {
    const KERB: f32 = 0.4;
    let w = road + 2.0 * KERB;
    // What the feet meet: the deck, its top on the grade line.
    put(
        Vec3::new(w, 0.4, len),
        flat(mid - up * 0.2),
        Mat::Concrete,
        true,
        false,
    );
    // Asphalt laid on the slab, the slab under it.
    put(
        Vec3::new(road, 0.06, len),
        flat(mid - up * 0.03),
        Mat::Asphalt,
        false,
        true,
    );
    put(
        Vec3::new(w, 0.35, len),
        flat(mid - up * 0.235),
        Mat::Concrete,
        false,
        true,
    );
    // Girders.
    for k in [-1.0, 0.0, 1.0] {
        put(
            Vec3::new(0.35, 0.55, len),
            flat(mid + side * k * w * 0.32 - up * 0.685),
            Mat::Concrete,
            false,
            true,
        );
    }
    for sgn in [-1.0, 1.0] {
        let edge = side * sgn * (w * 0.5 - KERB * 0.5);
        // Kerb, standing a hand above the road.
        put(
            Vec3::new(KERB, 0.25, len),
            flat(mid + edge + up * 0.065),
            Mat::Concrete,
            true,
            true,
        );
        let rail_line = side * sgn * (w * 0.5 - 0.12);
        let kerb_top = 0.19;
        for (h, t) in [(RAIL, 0.07), (RAIL * 0.55, 0.05)] {
            put(
                Vec3::new(t, t, len),
                flat(mid + rail_line + up * (kerb_top + h)),
                Mat::RustyMetal,
                true,
                true,
            );
        }
        put(
            Vec3::new(0.08, RAIL, 0.08),
            upright(a3 + rail_line + Vec3::Y * (kerb_top + RAIL * 0.5)),
            Mat::RustyMetal,
            true,
            true,
        );
    }
    // A pier every fourth segment, down to the bed.
    if i > 0 && i < n && i % 4 == 0 {
        let cap_top = a3.y - 0.96;
        put(
            Vec3::new(w * 0.95, 0.6, 0.9),
            upright(Vec3::new(a3.x, cap_top - 0.3, a3.z)),
            Mat::Concrete,
            true,
            true,
        );
        let depth = cap_top - 0.6 - ground + 0.8;
        if depth > 0.3 {
            for sgn in [-1.0, 1.0] {
                let at = a3 + side * sgn * w * 0.3;
                put(
                    Vec3::new(0.6, depth, 0.6),
                    upright(Vec3::new(at.x, cap_top - 0.6 - depth * 0.5, at.z)),
                    Mat::Concrete,
                    true,
                    true,
                );
            }
        }
    }
}

/// A timber footbridge's segment: boards laid across two stringers, posts
/// with a handrail and a mid-rail, and every third segment a trestle.
#[allow(clippy::too_many_arguments)]
fn footbridge_segment(
    put: &mut impl FnMut(Vec3, Transform, Mat, bool, bool),
    i: usize,
    n: usize,
    mid: Vec3,
    a3: Vec3,
    side: Vec3,
    up: Vec3,
    len: f32,
    along: Vec3,
    flat: impl Fn(Vec3) -> Transform,
    upright: impl Fn(Vec3) -> Transform,
    ground: f32,
) {
    const W: f32 = 1.8;
    put(
        Vec3::new(W, 0.2, len),
        flat(mid - up * 0.1),
        Mat::WoodFloor,
        true,
        false,
    );
    // Boards across, with gaps between.
    let boards = ((len - 0.04) / 0.2).round().max(1.0) as usize;
    for k in 0..boards {
        let t = (k as f32 + 0.5) / boards as f32;
        let at = a3 + along * t;
        put(
            Vec3::new(W, 0.05, 0.17),
            flat(at - up * 0.025),
            Mat::WoodFloor,
            false,
            true,
        );
    }
    // Stringers under them.
    for sgn in [-1.0, 1.0] {
        put(
            Vec3::new(0.15, 0.3, len),
            flat(mid + side * sgn * 0.6 - up * 0.2),
            Mat::Timber,
            false,
            true,
        );
    }
    for sgn in [-1.0, 1.0] {
        let edge = side * sgn * (W * 0.5 - 0.05);
        for (h, t) in [
            (RAIL, Vec2::new(0.07, 0.1)),
            (RAIL * 0.5, Vec2::new(0.05, 0.08)),
        ] {
            put(
                Vec3::new(t.x, t.y, len),
                flat(mid + edge + up * h),
                Mat::Timber,
                true,
                true,
            );
        }
        put(
            Vec3::new(0.1, RAIL + 0.05, 0.1),
            upright(a3 + edge + Vec3::Y * (RAIL * 0.5)),
            Mat::Timber,
            true,
            true,
        );
    }
    // A trestle every third segment: two legs to the bed and a cross-beam.
    if i > 0 && i < n && i % 3 == 0 {
        let top = a3.y - 0.35;
        put(
            Vec3::new(W + 0.2, 0.18, 0.2),
            upright(Vec3::new(a3.x, top - 0.09, a3.z)),
            Mat::Timber,
            false,
            true,
        );
        let depth = top - ground + 0.4;
        if depth > 0.3 {
            for sgn in [-1.0, 1.0] {
                let at = a3 + side * sgn * 0.7;
                put(
                    Vec3::new(0.18, depth, 0.18),
                    upright(Vec3::new(at.x, top - depth * 0.5, at.z)),
                    Mat::Timber,
                    true,
                    true,
                );
            }
        }
    }
}

/// The bridges, solid.
pub fn spawn_bridges(mut commands: Commands, zone: Res<Zone>) {
    for b in bridges(&zone) {
        let parts: Vec<(Vec3, Quat, Collider)> = b
            .pieces
            .iter()
            .filter(|p| p.solid)
            .map(|p| {
                (
                    p.at.translation,
                    p.at.rotation,
                    Collider::cuboid(p.size.x, p.size.y, p.size.z),
                )
            })
            .collect();
        let surfaces = super::surfaces::Surfaces(
            b.pieces
                .iter()
                .map(|p| (p.size * 0.5, p.at, p.mat))
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
        for p in b.pieces.iter().filter(|p| p.seen) {
            let mut mesh = crate::render::materials::metric_box(p.size);
            let _ = mesh.generate_tangents();
            let mesh = mesh.transformed_by(p.at);
            match merged.iter_mut().find(|(m, _)| *m == p.mat) {
                Some((_, into)) => {
                    let _ = into.merge(&mesh);
                }
                None => merged.push((p.mat, mesh)),
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

    /// A bot walks across every bridge, both ways, from the road before it
    /// to the road after.
    #[test]
    fn a_bot_walks_over_every_bridge() {
        use crate::bot::{Bot, Task};
        use crate::rig::LocalPlayer;
        let mut stuck = Vec::new();
        for map in crate::test_maps() {
            let opts = crate::launch::LaunchOptions {
                robots: false,
                map: map.clone(),
                ..Default::default()
            };
            let mut app = crate::headless_app(opts, false);
            app.finish();
            app.cleanup();
            app.update();
            // Each bridge as waypoints along its centreline, every 2 m, from
            // 4 m before the deck to 4 m after it (bots follow the path).
            let routes: Vec<Vec<Vec3>> = {
                let zone = app.world().resource::<Zone>();
                zone.hm
                    .road_lines
                    .iter()
                    .flat_map(|rl| {
                        rl.bridges.iter().map(|(a, b)| {
                            let mut pts = Vec::new();
                            let mut s = a - 4.0;
                            while s <= b + 4.0 {
                                let p = extraction_core::geom::polyline_at(&rl.points, s).0;
                                pts.push(zone.on_ground(p[0], p[1]));
                                s += 2.0;
                            }
                            pts
                        })
                    })
                    .collect()
            };
            let player = app
                .world_mut()
                .query_filtered::<Entity, With<LocalPlayer>>()
                .iter(app.world())
                .next()
                .unwrap();
            for route in routes {
                let back: Vec<Vec3> = route.iter().rev().copied().collect();
                for way in [route, back] {
                    let (from, to) = (way[0], *way.last().unwrap());
                    app.world_mut()
                        .get_mut::<Transform>(player)
                        .unwrap()
                        .translation = from + Vec3::Y * 0.1;
                    app.update();
                    let tasks = way[1..]
                        .iter()
                        .map(|p| Task::WalkTo {
                            to: p.xz(),
                            within: 0.6,
                        })
                        .collect::<Vec<_>>();
                    app.world_mut().entity_mut(player).insert(Bot::new(tasks));
                    for _ in 0..4000 {
                        app.update();
                        if app.world().get::<Bot>(player).unwrap().done() {
                            break;
                        }
                    }
                    let t = app.world().get::<Transform>(player).unwrap().translation;
                    if t.xz().distance(to.xz()) > 1.5 {
                        stuck.push(format!("{map}: {from:.0} -> {to:.0} stopped at {t:.1}"));
                    }
                }
            }
        }
        assert!(stuck.is_empty(), "{stuck:#?}");
    }
}
