//! Geometry gauges for QA: checks over every generated prefab that catch
//! what a screenshot tour shows only by luck.
//!
//! - **Fighting faces**: two box faces in the same plane, facing the same
//!   way, overlapping, with different materials. The depth buffer can't
//!   choose between them; they flicker as you move (z-fighting).
//! - **Floating pieces**: a piece whose underside rests on nothing (not
//!   the ground, not another piece's top or body), that nothing holds from
//!   the side or hangs from above.
//!
//! `cargo test --lib qa -- --nocapture` prints the tally per prefab;
//! `qa::tests` holds the line at zero. How buildings meet the terrain is
//! `extraction_core::fit`'s gauge.

use bevy::prelude::*;

use crate::render::materials::Mat;
use crate::world::buildings::{Geo, Shape};

/// One face of a box, in the prefab's space.
#[derive(Clone, Copy, Debug)]
struct Face {
    n: Vec3,
    c: Vec3,
    u: Vec3,
    v: Vec3,
    hu: f32,
    hv: f32,
}

fn faces(size: Vec3, t: &Transform) -> [Face; 6] {
    let h = size * 0.5;
    let r = t.rotation;
    let (x, y, z) = (r * Vec3::X, r * Vec3::Y, r * Vec3::Z);
    let c = t.translation;
    [
        Face {
            n: x,
            c: c + x * h.x,
            u: y,
            v: z,
            hu: h.y,
            hv: h.z,
        },
        Face {
            n: -x,
            c: c - x * h.x,
            u: y,
            v: z,
            hu: h.y,
            hv: h.z,
        },
        Face {
            n: y,
            c: c + y * h.y,
            u: x,
            v: z,
            hu: h.x,
            hv: h.z,
        },
        Face {
            n: -y,
            c: c - y * h.y,
            u: x,
            v: z,
            hu: h.x,
            hv: h.z,
        },
        Face {
            n: z,
            c: c + z * h.z,
            u: x,
            v: y,
            hu: h.x,
            hv: h.y,
        },
        Face {
            n: -z,
            c: c - z * h.z,
            u: x,
            v: y,
            hu: h.x,
            hv: h.y,
        },
    ]
}

/// A face's corners in a 2D basis of its plane.
fn corners(f: &Face, origin: Vec3, bu: Vec3, bv: Vec3) -> Vec<Vec2> {
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|(a, b)| {
            let p = f.c + f.u * f.hu * a + f.v * f.hv * b - origin;
            Vec2::new(p.dot(bu), p.dot(bv))
        })
        .collect()
}

fn area(poly: &[Vec2]) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    (a * 0.5).abs()
}

/// Sutherland–Hodgman: the part of `subject` inside convex `clip`.
fn clip(subject: &[Vec2], clip: &[Vec2]) -> Vec<Vec2> {
    let ccw = {
        let mut a = 0.0;
        for i in 0..clip.len() {
            let (p, q) = (clip[i], clip[(i + 1) % clip.len()]);
            a += p.x * q.y - q.x * p.y;
        }
        a > 0.0
    };
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let inside = |p: Vec2| {
            let s = (b - a).perp_dot(p - a);
            if ccw { s >= -1e-6 } else { s <= 1e-6 }
        };
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (pi, qi) = (inside(p), inside(q));
            if pi {
                out.push(p);
            }
            if pi != qi {
                let d = q - p;
                let e = b - a;
                let t = (a - p).perp_dot(e) / d.perp_dot(e);
                out.push(p + d * t);
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// A pair of faces fighting: which pieces, and the shared area.
#[derive(Debug)]
pub struct Fight {
    pub a: usize,
    pub b: usize,
    pub mats: (Mat, Mat),
    pub area: f32,
    pub at: Vec3,
}

fn boxes(geo: &Geo) -> Vec<(usize, Vec3, Transform, Mat)> {
    geo.pieces
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match p.shape {
            Shape::Box(s) => Some((i, s, p.at, p.mat)),
            _ => None,
        })
        .collect()
}

/// Every piece as a box (prisms and cylinders by their bounds): what can
/// hide a face or hold a piece up.
fn solids(geo: &Geo) -> Vec<(usize, Vec3, Transform)> {
    geo.pieces
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = match p.shape {
                Shape::Box(s) | Shape::Prism(s) => s,
                Shape::Cylinder(r, h) => Vec3::new(2.0 * r, h, 2.0 * r),
            };
            (i, s, p.at)
        })
        .collect()
}

fn inside(p: Vec3, s: Vec3, t: &Transform, pad: f32) -> bool {
    let l = t.rotation.inverse() * (p - t.translation);
    (l.abs() - s * 0.5).max_element() <= pad
}

/// Coplanar, same-facing, overlapping faces of different materials.
pub fn fighting_faces(geo: &Geo) -> Vec<Fight> {
    fighting_faces_above(geo, f32::NEG_INFINITY)
}

/// [`fighting_faces`], leaving out faces lower than `buried`: on the most
/// a building is ever lifted, the ground still covers everything below
/// that (foundations and the steps down to the ground end there).
pub fn fighting_faces_above(geo: &Geo, buried: f32) -> Vec<Fight> {
    let b = boxes(geo);
    let all = solids(geo);
    // A face nobody sees: just outside it is inside some other piece.
    let hidden = |f: &Face, a: usize, b: usize| {
        let out = f.c + f.n * 0.01;
        all.iter()
            .any(|(j, s, t)| *j != a && *j != b && inside(out, *s, t, -0.001))
    };
    let mut out = Vec::new();
    for x in 0..b.len() {
        for y in x + 1..b.len() {
            let (ia, sa, ta, ma) = b[x];
            let (ib, sb, tb, mb) = b[y];
            if ma == mb {
                continue;
            }
            // Far apart: no shared faces.
            if ta.translation.distance(tb.translation) > (sa.length() + sb.length()) * 0.5 + 0.01 {
                continue;
            }
            for fa in faces(sa, &ta) {
                for fb in faces(sb, &tb) {
                    if fa.n.dot(fb.n) < 0.999 || fa.n.dot(fb.c - fa.c).abs() > 0.002 {
                        continue;
                    }
                    if fa.c.y.max(fb.c.y) < buried {
                        continue;
                    }
                    let (bu, bv) = (fa.u, fa.v);
                    let pa = corners(&fa, fa.c, bu, bv);
                    let pb = corners(&fb, fa.c, bu, bv);
                    let shared = area(&clip(&pa, &pb));
                    if shared > 0.005 && !hidden(&fa, ia, ib) {
                        out.push(Fight {
                            a: ia,
                            b: ib,
                            mats: (ma, mb),
                            area: shared,
                            at: fa.c,
                        });
                    }
                }
            }
        }
    }
    out
}

/// Pieces whose underside rests on nothing: not the ground (y near 0 or
/// below), not inside or on another piece.
pub fn floating(geo: &Geo) -> Vec<(usize, Vec3)> {
    let b = boxes(geo);
    let all = solids(geo);
    let mut out = Vec::new();
    for &(i, s, t, _) in &b {
        // A grid over the underside (whichever face is lowest), just below.
        let h = s * 0.5;
        let mut best: Option<(f32, Vec<Vec3>)> = None;
        for f in faces(s, &t) {
            let pts: Vec<Vec3> = (0..5)
                .flat_map(|a| (0..5).map(move |b| (a as f32 / 2.0 - 1.0, b as f32 / 2.0 - 1.0)))
                .map(|(a, b)| f.c + f.u * f.hu * a * 0.95 + f.v * f.hv * b * 0.95)
                .collect();
            let y = pts.iter().map(|p| p.y).sum::<f32>() / pts.len() as f32;
            if best.as_ref().is_none_or(|b| y < b.0) {
                best = Some((y, pts));
            }
        }
        let (bottom, under) = best.unwrap();
        if bottom <= 0.05 {
            continue;
        }
        let under: Vec<Vec3> = under.iter().map(|p| *p - Vec3::Y * 0.02).collect();
        let grid = all
            .iter()
            .any(|&(j, s2, t2)| j != i && under.iter().any(|p| inside(*p, s2, &t2, 0.04)));
        // Or another piece's top meets the underside somewhere (a roof on
        // posts the grid misses).
        let low = faces(s, &t)
            .into_iter()
            .min_by(|a, b| a.c.y.total_cmp(&b.c.y))
            .unwrap();
        let tops = all.iter().any(|&(j, s2, t2)| {
            j != i
                && faces(s2, &t2).iter().any(|f| {
                    f.n.y > 0.9 && (f.c.y - low.c.y).abs() < 0.04 && {
                        let (bu, bv) = (low.u, low.v);
                        let pa = corners(&low, low.c, bu, bv);
                        let pb = corners(f, low.c, bu, bv);
                        area(&clip(&pa, &pb)) > 1e-4
                    }
                })
        });
        // Or it hangs: just above its top face is inside another piece (a
        // hook on its chain, a lamp from a beam).
        let top = faces(s, &t)
            .into_iter()
            .max_by(|a, b| a.c.y.total_cmp(&b.c.y))
            .unwrap();
        let hung = all.iter().any(|&(j, s2, t2)| {
            j != i && {
                let p = top.c + Vec3::Y * 0.02;
                inside(p, s2, &t2, 0.04)
            }
        });
        let rests = grid || tops || hung;
        // Or something holds it from the side (a shelf on a wall): its
        // own body touches another piece.
        let held = all.iter().any(|&(j, s2, t2)| {
            j != i && {
                let probe = [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z]
                    .map(|d| t.translation + t.rotation * (d * h));
                probe.iter().any(|p| inside(*p, s2, &t2, 0.03))
            }
        });
        if !rests && !held {
            out.push((i, t.translation));
        }
    }
    out
}

/// The tally for every prefab: (prefab, fights, floaters).
pub fn report() -> Vec<(String, Vec<Fight>, Vec<(usize, Vec3)>)> {
    crate::world::buildings::PREFABS
        .iter()
        .filter_map(|id| {
            let geo = crate::world::buildings::prefab(id)?;
            let buried = -extraction_core::map::prefab_lift(id) - 0.05;
            Some((
                id.to_string(),
                fighting_faces_above(&geo, buried),
                floating(&geo),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_finds_the_overlap() {
        let a = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let b = [
            Vec2::new(1.0, 1.0),
            Vec2::new(3.0, 1.0),
            Vec2::new(3.0, 3.0),
            Vec2::new(1.0, 3.0),
        ];
        assert!((area(&clip(&a, &b)) - 1.0).abs() < 1e-4);
    }

    /// Prefabs allowed to float pieces: the anomaly's slabs hang in the
    /// air on purpose.
    const MAY_FLOAT: &[&str] = &["anomaly_core"];

    #[test]
    fn no_prefab_fights_or_floats() {
        let mut bad = Vec::new();
        for (id, fights, floaters) in report() {
            if !fights.is_empty() {
                bad.push(format!("{id}: {} fighting faces", fights.len()));
            }
            if !floaters.is_empty() && !MAY_FLOAT.contains(&id.as_str()) {
                bad.push(format!("{id}: {} floating pieces", floaters.len()));
            }
        }
        assert!(
            bad.is_empty(),
            "geometry gauges: {bad:#?} (see geometry_report)"
        );
    }

    /// A building may sit as high above the lowest ground under it as
    /// `map::prefab_foundation` says its foundation reaches down: the model
    /// must reach that deep, or the lifted building floats.
    #[test]
    fn foundations_reach_as_deep_as_claimed() {
        for id in crate::world::buildings::PREFABS {
            let geo = crate::world::buildings::prefab(id).unwrap();
            // Lowest point of any piece, rotation included (a log
            // lying on its side reaches no deeper than its radius).
            let low = geo
                .pieces
                .iter()
                .map(|p| {
                    let s = match p.shape {
                        Shape::Box(s) | Shape::Prism(s) => s,
                        Shape::Cylinder(r, h) => Vec3::new(2.0 * r, h, 2.0 * r),
                    };
                    let m = Mat3::from_quat(p.at.rotation);
                    let reach =
                        m.x_axis.y.abs() * s.x + m.y_axis.y.abs() * s.y + m.z_axis.y.abs() * s.z;
                    p.at.translation.y - reach * 0.5
                })
                .fold(f32::INFINITY, f32::min);
            println!("{id:16} reaches {:.2} below its footing", -low);
            let claim = extraction_core::map::prefab_foundation(id);
            assert!(
                claim <= 0.0 || -low >= claim - 1e-3,
                "{id}: foundation claimed {claim} m deep, the model reaches {:.2}",
                -low
            );
        }
    }

    /// Prints the tally (run with --nocapture).
    #[test]
    fn geometry_report() {
        for (id, fights, floaters) in report() {
            let fight_area: f32 = fights.iter().map(|f| f.area).sum();
            println!(
                "{id:16} fighting faces {:3} ({fight_area:.2} m2)  floating {:3}",
                fights.len(),
                floaters.len()
            );
            for f in fights.iter().take(3) {
                println!("    fight {:?} {:.3} m2 at {:.2}", f.mats, f.area, f.at);
            }
            let geo = crate::world::buildings::prefab(&id).unwrap();
            for (i, at) in floaters.iter().take(3) {
                let p = geo.pieces[*i];
                println!(
                    "    floating piece {i} {:?} {:?} at {at:.2}",
                    p.shape, p.mat
                );
            }
        }
    }
}
