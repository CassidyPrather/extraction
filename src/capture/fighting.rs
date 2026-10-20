//! Z-fighting from the geometry: two surfaces in the same plane, facing
//! the same way and overlapping, drawn in different materials. The depth
//! buffer can't choose between them, and they flicker as you move. A
//! still picture doesn't show it (the sweep found that out), so this looks
//! at the triangles themselves:
//!
//! - **assembled buildings**: each prefab as the game draws it close up
//!   (its blockout boxes, less those its furniture and fittings replace,
//!   plus those models where they're placed), every pair of pieces from
//!   different sources. `qa`'s gauge covers boxes against boxes; this adds
//!   the models placed against the boxes and against each other;
//! - **every model**: its own nodes against each other (as `art/check.py`
//!   does in Blender, here in `cargo test` too).
//!
//! `cargo test --lib capture::fighting -- --nocapture` prints the tally.

use bevy::math::{Vec2, Vec3};
use std::collections::HashMap;

use super::glb;
use crate::world::buildings::{self, Shape};

/// How close two planes must be to fight, metres.
pub const PLANE_GAP: f32 = 0.0005;
/// Overlaps smaller than this are ignored, m² (a pin's head).
pub const MIN_AREA: f32 = 0.0004;

/// A triangle and where it came from.
#[derive(Clone, Debug)]
pub struct Face {
    /// Which piece or model instance: faces of one source never fight
    /// each other here.
    pub source: usize,
    pub material: String,
    pub p: [Vec3; 3],
}

/// Two sources fighting: their names, materials and how much overlaps.
#[derive(Clone, Debug)]
pub struct Fight {
    pub a: String,
    pub b: String,
    pub materials: (String, String),
    pub area: f32,
    /// A point in the overlap.
    pub at: Vec3,
    /// Which way the faces face.
    pub normal: Vec3,
}

fn normal(p: &[Vec3; 3]) -> Option<Vec3> {
    let n = (p[1] - p[0]).cross(p[2] - p[0]);
    (n.length() > 1e-9).then(|| n.normalize())
}

/// Convex polygon `subject` clipped by convex polygon `clip` (both
/// counter-clockwise), Sutherland-Hodgman.
fn clip(subject: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let inside = |p: Vec2| (b - a).perp_dot(p - a) >= -1e-9;
        let input = std::mem::take(&mut out);
        for k in 0..input.len() {
            let (p, q) = (input[k], input[(k + 1) % input.len()]);
            let (pi, qi) = (inside(p), inside(q));
            if pi {
                out.push(p);
            }
            if pi != qi {
                let d = (b - a).perp_dot(q - p);
                if d.abs() > 1e-12 {
                    let t = (b - a).perp_dot(a - p) / d;
                    out.push(p + (q - p) * t);
                }
            }
        }
    }
    out
}

fn area(poly: &[Vec2]) -> f32 {
    (0..poly.len())
        .map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()]))
        .sum::<f32>()
        * 0.5
}

/// A solid box, for hiding faces: (world from box, half size).
pub type Solid = (bevy::prelude::Transform, Vec3);

/// Whether there's somewhere to stand under `p`: a solid's top within
/// `reach` below it. Underground, a face looking out into the earth (or
/// the underside of the lowest floor) has none: nobody sees it.
pub fn floor_under(solids: &[Solid], p: Vec3, reach: f32) -> bool {
    solids.iter().any(|(at, half)| {
        let l = at.compute_affine().inverse().transform_point3(p);
        l.x.abs() < half.x && l.z.abs() < half.z && l.y >= half.y && l.y - half.y < reach
    })
}

fn inside(solids: &[Solid], p: Vec3) -> bool {
    solids.iter().any(|(at, half)| {
        let l = at.compute_affine().inverse().transform_point3(p);
        (l.abs() - *half).max_element() < -0.0005
    })
}

/// Every fight between faces of different sources (summed per pair of
/// sources and materials). `names` names the sources. An overlap whose
/// middle is buried in one of `solids` (just in front of the faces), or
/// that `seen` (given its middle and its facing) says nobody sees, doesn't
/// count.
pub fn fights(
    faces: &[Face],
    names: &[String],
    solids: &[Solid],
    seen: impl Fn(Vec3, Vec3) -> bool,
) -> Vec<Fight> {
    // Bucket by plane: the normal to 1/40, the offset to a millimetre.
    let mut buckets: HashMap<([i32; 3], i64), Vec<(usize, Vec3, f32)>> = HashMap::new();
    for (i, f) in faces.iter().enumerate() {
        let Some(n) = normal(&f.p) else { continue };
        let d = n.dot(f.p[0]);
        let key = (
            [
                (n.x * 40.0).round() as i32,
                (n.y * 40.0).round() as i32,
                (n.z * 40.0).round() as i32,
            ],
            (d / 0.001).round() as i64,
        );
        buckets.entry(key).or_default().push((i, n, d));
    }
    let mut total: HashMap<(usize, usize, String, String), (f32, Vec3, Vec3)> = HashMap::new();
    for ((nk, dk), list) in &buckets {
        // Planes a hair apart can land in neighbouring buckets.
        for dk2 in [*dk, dk + 1] {
            let Some(other) = buckets.get(&(*nk, dk2)) else {
                continue;
            };
            for (ai, an, ad) in list {
                for (bi, bn, bd) in other {
                    if dk2 == *dk && bi <= ai {
                        continue;
                    }
                    let (fa, fb) = (&faces[*ai], &faces[*bi]);
                    if fa.source == fb.source
                        || fa.material == fb.material
                        || an.dot(*bn) < 0.9995
                        || (ad - bd).abs() > PLANE_GAP
                    {
                        continue;
                    }
                    // Into the plane's own 2D frame.
                    let u = an.any_orthonormal_vector();
                    let v = an.cross(u);
                    let flat = |p: &[Vec3; 3]| {
                        let mut t: Vec<Vec2> =
                            p.iter().map(|q| Vec2::new(q.dot(u), q.dot(v))).collect();
                        if area(&t) < 0.0 {
                            t.reverse();
                        }
                        t
                    };
                    let overlap = clip(&flat(&fa.p), &flat(&fb.p));
                    let a = area(&overlap);
                    if a <= 1e-7 {
                        continue;
                    }
                    let c = overlap.iter().copied().sum::<Vec2>() / overlap.len() as f32;
                    let at = u * c.x + v * c.y + *an * *ad;
                    if !seen(at, *an) || inside(solids, at + *an * 0.002) {
                        continue;
                    }
                    let (s1, s2, m1, m2) = if fa.source < fb.source {
                        (fa.source, fb.source, &fa.material, &fb.material)
                    } else {
                        (fb.source, fa.source, &fb.material, &fa.material)
                    };
                    // The point reported: the overlaps' middle, by area.
                    let e = total.entry((s1, s2, m1.clone(), m2.clone())).or_insert((
                        0.0,
                        Vec3::ZERO,
                        *an,
                    ));
                    e.0 += a;
                    e.1 += at * a;
                }
            }
        }
    }
    let mut out: Vec<Fight> = total
        .into_iter()
        .filter(|(_, (a, _, _))| *a >= MIN_AREA)
        .map(|((s1, s2, m1, m2), (area, at, normal))| Fight {
            at: at / area,
            normal,
            a: names.get(s1).cloned().unwrap_or_default(),
            b: names.get(s2).cloned().unwrap_or_default(),
            materials: (m1, m2),
            area,
        })
        .collect();
    out.sort_by(|a, b| {
        b.area
            .total_cmp(&a.area)
            .then_with(|| a.a.cmp(&b.a))
            .then_with(|| a.b.cmp(&b.b))
    });
    out
}

/// A box's twelve triangles, outward-facing.
fn box_faces(size: Vec3, at: &bevy::prelude::Transform) -> Vec<[Vec3; 3]> {
    let h = size * 0.5;
    let c = |x: f32, y: f32, z: f32| at.transform_point(Vec3::new(x * h.x, y * h.y, z * h.z));
    let quads = [
        // +X, -X, +Y, -Y, +Z, -Z (counter-clockwise seen from outside)
        [
            c(1., -1., -1.),
            c(1., 1., -1.),
            c(1., 1., 1.),
            c(1., -1., 1.),
        ],
        [
            c(-1., -1., 1.),
            c(-1., 1., 1.),
            c(-1., 1., -1.),
            c(-1., -1., -1.),
        ],
        [
            c(-1., 1., -1.),
            c(-1., 1., 1.),
            c(1., 1., 1.),
            c(1., 1., -1.),
        ],
        [
            c(-1., -1., 1.),
            c(-1., -1., -1.),
            c(1., -1., -1.),
            c(1., -1., 1.),
        ],
        [
            c(-1., -1., 1.),
            c(1., -1., 1.),
            c(1., 1., 1.),
            c(-1., 1., 1.),
        ],
        [
            c(1., -1., -1.),
            c(-1., -1., -1.),
            c(-1., 1., -1.),
            c(1., 1., -1.),
        ],
    ];
    quads
        .iter()
        .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
        .collect()
}

/// A prefab as drawn close up: the boxes no model replaces, and the
/// models where they're placed (near nodes only).
pub fn assemble(id: &str) -> Option<(Vec<Face>, Vec<String>, Vec<Solid>)> {
    let geo = buildings::prefab(id)?;
    let mut cache: HashMap<String, Vec<glb::Tri>> = HashMap::new();
    let mut faces = Vec::new();
    let mut names = Vec::new();
    let mut solids = Vec::new();
    let mut replaced = vec![false; geo.pieces.len()];
    for d in &geo.props {
        for i in d.pieces.clone() {
            replaced[i] = true;
        }
    }
    for (i, p) in geo.pieces.iter().enumerate() {
        if replaced[i] {
            continue;
        }
        let Shape::Box(size) = p.shape else { continue };
        if p.mat != crate::render::materials::Mat::Glass {
            solids.push((p.at, size * 0.5));
        }
        let source = names.len();
        names.push(format!("box {i} ({:?})", p.mat));
        for t in box_faces(size, &p.at) {
            faces.push(Face {
                source,
                material: format!("{:?}", p.mat),
                p: t,
            });
        }
    }
    for d in &geo.props {
        for (name, at) in &d.models {
            let tris = cache.entry(name.to_string()).or_insert_with(|| {
                let path = crate::asset_root_path().join(format!("models/props/{name}.glb"));
                glb::triangles(&path).unwrap_or_default()
            });
            let source = names.len();
            names.push(format!(
                "{name} at ({:.2}, {:.2}, {:.2})",
                at.translation.x, at.translation.y, at.translation.z
            ));
            for t in tris.iter().filter(|t| !t.node.contains("_lod")) {
                faces.push(Face {
                    source,
                    material: t.material.clone(),
                    p: t.p.map(|q| at.transform_point(q)),
                });
            }
        }
    }
    Some((faces, names, solids))
}

/// A model's own nodes against each other.
pub fn model_fights(path: &std::path::Path) -> Vec<Fight> {
    let tris = glb::triangles(path).unwrap_or_default();
    let mut names: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let faces: Vec<Face> = tris
        .into_iter()
        .filter(|t| !t.node.contains("_lod"))
        .map(|t| {
            let source = *index.entry(t.node.clone()).or_insert_with(|| {
                names.push(t.node.clone());
                names.len() - 1
            });
            Face {
                source,
                material: t.material,
                p: t.p,
            }
        })
        .collect();
    fights(&faces, &names, &[], |_, _| true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Transform;

    fn quad(source: usize, mat: &str, lo: Vec2, hi: Vec2, y: f32) -> Vec<Face> {
        let p = |x: f32, z: f32| Vec3::new(x, y, z);
        // Facing up.
        [
            [p(lo.x, lo.y), p(lo.x, hi.y), p(hi.x, hi.y)],
            [p(lo.x, lo.y), p(hi.x, hi.y), p(hi.x, lo.y)],
        ]
        .into_iter()
        .map(|p| Face {
            source,
            material: mat.into(),
            p,
        })
        .collect()
    }

    #[test]
    fn finds_overlapping_coplanar_faces() {
        let names = ["floor".to_string(), "rug".to_string(), "lid".to_string()];
        let mut faces = quad(0, "Wood", Vec2::ZERO, Vec2::splat(2.0), 0.0);
        // Half a metre square lying exactly on it: fights, 0.25 m².
        faces.extend(quad(1, "Cloth", Vec2::splat(0.5), Vec2::splat(1.0), 0.0));
        // A lid 5 mm up: clear.
        faces.extend(quad(2, "Metal", Vec2::splat(1.2), Vec2::splat(1.8), 0.005));
        let f = fights(&faces, &names, &[], |_, _| true);
        assert_eq!(f.len(), 1, "{f:?}");
        // Buried in a solid just above: unseen.
        let lid = (
            Transform::from_xyz(0.75, 0.05, 0.75),
            Vec3::new(0.5, 0.05, 0.5),
        );
        assert!(fights(&faces, &names, &[lid], |_, _| true).is_empty());
        // Or below where anyone looks.
        assert!(fights(&faces, &names, &[], |p, _| p.y > 0.1).is_empty());
        assert!((f[0].area - 0.25).abs() < 1e-3, "{}", f[0].area);
        assert_eq!((f[0].a.as_str(), f[0].b.as_str()), ("floor", "rug"));
        // The same material doesn't show, and one source never fights itself.
        let mut same = quad(0, "Wood", Vec2::ZERO, Vec2::ONE, 0.0);
        same.extend(quad(1, "Wood", Vec2::ZERO, Vec2::ONE, 0.0));
        same.extend(quad(1, "Cloth", Vec2::ZERO, Vec2::ONE, 0.0));
        assert_eq!(fights(&same, &names, &[], |_, _| true).len(), 1);
    }

    #[test]
    fn boxes_face_outward() {
        for t in box_faces(
            Vec3::new(2.0, 1.0, 3.0),
            &Transform::from_xyz(5.0, 0.0, 0.0),
        ) {
            let n = normal(&t).unwrap();
            let c = (t[0] + t[1] + t[2]) / 3.0 - Vec3::new(5.0, 0.0, 0.0);
            assert!(n.dot(c) > 0.0, "{t:?}");
        }
    }

    /// Fights found when this gauge was written, waiting on their owners
    /// (reported to Models: world): (prefab or `*`, text in one side's
    /// name, text in the other's). Take a line out when it's fixed; a new
    /// fight fails the test.
    const KNOWN: &[(&str, &str, &str)] = &[
        // Window casings flush with the timber trim round the opening.
        ("*", "(Timber)", "window_casing"),
        ("pump_station", "(Plaster)", "window_sash"),
        ("bus_depot", "workbench", "table"),
        ("chapel", "(Concrete)", "steeple"),
        ("mill_race", "mill_race", "crate_slat"),
        ("house_plank_damaged", "(WoodFloor)", "rubble_planks"),
        ("shed", "(Timber)", "(Concrete)"),
        // Roof ends: shingles or tin against the gable's planks.
        ("*", "(Shingles)", "(Planks)"),
        ("*", "(Corrugated)", "(Planks)"),
        ("farmhouse_burnt", "(RustyMetal)", "(Charred)"),
        // The foundation's face flush with the bottom log, above ground
        // (`qa` judges burial by a face's centre, which is underground).
        ("cabin_log", "(Concrete)", "(Timber)"),
    ];

    fn known(id: &str, f: &Fight) -> bool {
        KNOWN.iter().any(|(p, a, b)| {
            (*p == "*" || *p == id)
                && ((f.a.contains(a) && f.b.contains(b)) || (f.a.contains(b) && f.b.contains(a)))
        })
    }

    /// Buildings as the game draws them close up: no model placed against
    /// a box, or another model, in its plane with another material.
    /// Underground sites are printed, not held: outside their boxes is
    /// earth. A face counts there only with somewhere to stand in front of
    /// it (`floor_under`), but that's rougher than a building's ground.
    #[test]
    fn assembled_buildings_dont_fight() {
        let mut bad = Vec::new();
        let mut waiting = 0;
        for id in buildings::PREFABS
            .into_iter()
            .chain(crate::world::underground::ids())
        {
            let Some((faces, names, solids)) = assemble(id) else {
                continue;
            };
            // Below the ground round a building (as `qa` takes it) nobody
            // looks.
            let site = crate::world::underground::ids().any(|u| u == id);
            let buried = if site {
                f32::MIN
            } else {
                -extraction_core::map::prefab_lift(id) - 0.05
            };
            // Underground, a face is seen only from somewhere to stand.
            let fights = fights(&faces, &names, &solids, |p, n| {
                p.y > buried && (!site || floor_under(&solids, p + n * 0.05, 4.0))
            });
            let sum: f32 = fights.iter().map(|f| f.area).sum();
            println!(
                "{id:<22} {:>6} faces  {:>3} fights  {sum:.3} m²",
                faces.len(),
                fights.len()
            );
            for f in fights {
                let line = format!(
                    "{id}: {} ({}) vs {} ({}), {:.4} m² at ({:.2}, {:.2}, {:.2}) facing ({:.0}, {:.0}, {:.0})",
                    f.a,
                    f.materials.0,
                    f.b,
                    f.materials.1,
                    f.area,
                    f.at.x,
                    f.at.y,
                    f.at.z,
                    f.normal.x,
                    f.normal.y,
                    f.normal.z
                );
                println!("    {line}");
                if known(id, &f) {
                    waiting += 1;
                } else if !site {
                    bad.push(line);
                }
            }
        }
        println!("{waiting} known fights waiting on their owners");
        assert!(
            bad.is_empty(),
            "fighting faces in assembled buildings:\n{}",
            bad.join("\n")
        );
    }

    /// Every model's nodes against each other.
    #[test]
    fn models_dont_fight_themselves() {
        let mut bad = Vec::new();
        for (folder, name, path) in glb::all_models() {
            for f in model_fights(&path) {
                bad.push(format!(
                    "{folder}/{name}: {} ({}) vs {} ({}), {:.4} m² at ({:.3}, {:.3}, {:.3})",
                    f.a, f.materials.0, f.b, f.materials.1, f.area, f.at.x, f.at.y, f.at.z
                ));
            }
        }
        for b in &bad {
            println!("{b}");
        }
        assert!(
            bad.is_empty(),
            "models whose own parts fight:\n{}",
            bad.join("\n")
        );
    }
}
