//! Model correctness: parts attached to what holds them (M3 in
//! `docs/playtests/2026-10-28/TRANSCRIPT.md`). The playtest saw the
//! truck's mud flaps hanging off nothing, a latch floating by its window,
//! a gap through a house wall and a door hung off the side of its
//! building.
//!
//! - **Islands**: every model is split into its connected pieces (welded
//!   vertices). Pieces touch when they come within [`TOUCH`] of each other
//!   or pass through each other. Starting from the pieces on the model's
//!   floor (and the biggest), every piece must be reached through touching
//!   ones; one that isn't hangs in the air, however near its box comes to
//!   another (`checks::models_hold_together` compares boxes, which can't
//!   see a flap 2 cm off its bumper).
//! - **Slits**: in every building, no two boxes face each other across a
//!   gap between [`SLIT_MIN`] and [`SLIT_MAX`] over more than [`SLIT_AREA`]:
//!   light shows through a wall's crack.
//! - **Hinges**: every door's hinge is in its jamb, and its leaf's far edge
//!   meets the other jamb.
//!
//! `cargo test --lib capture::attach -- --nocapture` prints every finding.

use bevy::math::Vec3;
use bevy::prelude::Transform;
use std::collections::{HashMap, HashSet, VecDeque};

use super::glb;
use crate::render::materials::Mat;
use crate::world::buildings::{self, Shape};

/// Pieces nearer than this touch (m).
pub const TOUCH: f32 = 0.002;

/// A model's connected pieces: for each, its triangles' indices.
pub fn islands(tris: &[[Vec3; 3]]) -> Vec<Vec<usize>> {
    let key = |p: Vec3| {
        (
            (p.x * 1e4).round() as i64,
            (p.y * 1e4).round() as i64,
            (p.z * 1e4).round() as i64,
        )
    };
    let mut ids: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut tri_v = Vec::with_capacity(tris.len());
    for t in tris {
        let v = t.map(|p| {
            let n = ids.len();
            let id = *ids.entry(key(p)).or_insert(n);
            if id == parent.len() {
                parent.push(id);
            }
            id
        });
        let (a, b, c) = (
            find(&mut parent, v[0]),
            find(&mut parent, v[1]),
            find(&mut parent, v[2]),
        );
        parent[a] = c;
        parent[b] = c;
        tri_v.push(v[0]);
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, v) in tri_v.into_iter().enumerate() {
        let r = find(&mut parent, v);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by_key(|g| g[0]);
    out
}

/// Whether segment `a`-`b` passes through triangle `t`.
fn segment_hits(a: Vec3, b: Vec3, t: &[Vec3; 3]) -> bool {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let d = b - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return false;
    }
    let inv = 1.0 / det;
    let s = a - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let w = e2.dot(q) * inv;
    (0.0..=1.0).contains(&w)
}

/// Whether two triangles touch: within [`TOUCH`] or crossing.
fn tris_touch(a: &[Vec3; 3], b: &[Vec3; 3]) -> bool {
    use super::seated::tri_distance;
    a.iter().any(|p| tri_distance(b, *p) <= TOUCH)
        || b.iter().any(|p| tri_distance(a, *p) <= TOUCH)
        || (0..3).any(|i| segment_hits(a[i], a[(i + 1) % 3], b))
        || (0..3).any(|i| segment_hits(b[i], b[(i + 1) % 3], a))
}

/// Pieces of a model held by nothing: (piece's bounds' middle, its size,
/// its gap to the nearest held piece), largest first.
///
/// Held: the pieces on the model's floor and its biggest, and whatever
/// touches a held piece. Pieces with vertices within [`TOUCH`] of each
/// other touch (cheap, and most do); only those still unheld then have
/// their triangles tried against their neighbours' (near, or crossing).
pub fn detached(tris: &[[Vec3; 3]]) -> Vec<(Vec3, Vec3, f32)> {
    use bevy::math::IVec3;
    let groups = islands(tris);
    if groups.len() < 2 {
        return Vec::new();
    }
    let mut owner = vec![0usize; tris.len()];
    for (g, list) in groups.iter().enumerate() {
        for &t in list {
            owner[t] = g;
        }
    }
    let bounds: Vec<(Vec3, Vec3)> = groups
        .iter()
        .map(|g| {
            g.iter().flat_map(|&t| tris[t]).fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(l, h), p| (l.min(p), h.max(p)),
            )
        })
        .collect();
    let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); groups.len()];
    // Vertices near another piece's vertices.
    let vc = TOUCH * 2.0;
    let vk = |p: Vec3| (p / vc).floor().as_ivec3();
    let mut verts: HashMap<IVec3, Vec<(Vec3, usize)>> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for p in t {
            verts.entry(vk(*p)).or_default().push((*p, owner[i]));
        }
    }
    for (k, list) in &verts {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(other) = verts.get(&(*k + IVec3::new(dx, dy, dz))) else {
                        continue;
                    };
                    for (p, a) in list {
                        for (q, b) in other {
                            if a != b && !adj[*a].contains(b) && p.distance(*q) <= TOUCH {
                                adj[*a].insert(*b);
                                adj[*b].insert(*a);
                            }
                        }
                    }
                }
            }
        }
    }
    let floor = bounds.iter().map(|b| b.0.y).fold(f32::MAX, f32::min);
    let biggest = (0..groups.len())
        .max_by(|a, b| {
            let s = |i: usize| (bounds[i].1 - bounds[i].0).length();
            s(*a).total_cmp(&s(*b))
        })
        .unwrap();
    let spread = |adj: &[HashSet<usize>]| {
        let mut held = vec![false; groups.len()];
        let mut queue = VecDeque::new();
        for (i, b) in bounds.iter().enumerate() {
            // On the floor: within a centimetre of the model's lowest
            // point (bars leaning on the ground end a hair apart).
            if b.0.y <= floor + 0.01 || i == biggest {
                held[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            for &j in &adj[i] {
                if !held[j] {
                    held[j] = true;
                    queue.push_back(j);
                }
            }
        }
        held
    };
    let mut held = spread(&adj);
    // Pieces through each other's middles (a chain's links) are linked
    // though they never touch: their bounds overlap by more than a
    // millimetre every way.
    if held.iter().any(|h| !h) {
        for a in 0..groups.len() {
            for b in a + 1..groups.len() {
                let (l1, h1) = bounds[a];
                let (l2, h2) = bounds[b];
                if (h1.min(h2) - l1.max(l2)).min_element() > 0.001 {
                    adj[a].insert(b);
                    adj[b].insert(a);
                }
            }
        }
        held = spread(&adj);
    }
    if held.iter().any(|h| !h) {
        // A grid of triangles, for the pieces still unheld to look in.
        let cell = 0.02f32;
        let k = |p: Vec3| (p / cell).floor().as_ivec3();
        let tbox = |t: &[Vec3; 3]| {
            (
                t[0].min(t[1]).min(t[2]) - Vec3::splat(TOUCH),
                t[0].max(t[1]).max(t[2]) + Vec3::splat(TOUCH),
            )
        };
        let mut grid: HashMap<IVec3, Vec<usize>> = HashMap::new();
        let mut big = Vec::new();
        for (i, t) in tris.iter().enumerate() {
            let (lo, hi) = tbox(t);
            let (lo, hi) = (k(lo), k(hi));
            let n = (hi - lo + 1).as_vec3();
            if n.x * n.y * n.z > 64.0 {
                big.push(i);
                continue;
            }
            for x in lo.x..=hi.x {
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        grid.entry(IVec3::new(x, y, z)).or_default().push(i);
                    }
                }
            }
        }
        let overlap = |a: usize, b: usize| {
            let (l1, h1) = tbox(&tris[a]);
            let (l2, h2) = tbox(&tris[b]);
            l1.cmple(h2).all() && l2.cmple(h1).all()
        };
        let mut stamp = vec![usize::MAX; tris.len()];
        for g in 0..groups.len() {
            if held[g] {
                continue;
            }
            for &a in &groups[g] {
                let (lo, hi) = tbox(&tris[a]);
                let (lo, hi) = (k(lo), k(hi));
                let mut near: Vec<usize> = big.clone();
                for x in lo.x..=hi.x {
                    for y in lo.y..=hi.y {
                        for z in lo.z..=hi.z {
                            if let Some(list) = grid.get(&IVec3::new(x, y, z)) {
                                near.extend(list);
                            }
                        }
                    }
                }
                for b in near {
                    let gb = owner[b];
                    if gb == g || stamp[b] == a || adj[g].contains(&gb) {
                        continue;
                    }
                    stamp[b] = a;
                    if overlap(a, b) && tris_touch(&tris[a], &tris[b]) {
                        adj[g].insert(gb);
                        adj[gb].insert(g);
                    }
                }
            }
        }
        held = spread(&adj);
    }
    let mut out: Vec<(Vec3, Vec3, f32)> = (0..groups.len())
        .filter(|i| !held[*i])
        .map(|i| {
            let (lo, hi) = bounds[i];
            // Its gap: box to box, to the nearest held piece (a floor).
            let gap = (0..groups.len())
                .filter(|j| held[*j])
                .map(|j| {
                    let (l2, h2) = bounds[j];
                    (l2 - hi).max(lo - h2).max(Vec3::ZERO).length()
                })
                .fold(f32::MAX, f32::min);
            ((lo + hi) * 0.5, hi - lo, gap)
        })
        .collect();
    out.sort_by(|a, b| b.1.length().total_cmp(&a.1.length()));
    out
}

/// Models assembled from their parts in the game (a robot, an animal, the
/// rig's slot pieces, a truck's one wheel), whose pieces are placed apart.
pub fn assembled(folder: &str, name: &str) -> bool {
    matches!(folder, "animals" | "robots" | "hands" | "kit")
        // Landmarks and works dressing their buildings' boxes, and things
        // hung on a wall: held by what the building draws round them.
        // (A window's or a door's own pieces still hold each other: the
        // playtest's latch floated off its casement.)
        || (folder == "props"
            && super::footing::exempt(name)
            && !["rubble", "window_", "door_", "_burnt", "_charred", "ash_"]
                .iter()
                .any(|k| name.contains(k)))
}

/// Pieces too small to see from arm's length (m, their box's diagonal).
pub const CRUMB: f32 = 0.004;

/// Every model's detached pieces, one line a model.
pub fn detached_findings() -> Vec<String> {
    let models: Vec<(String, String, std::path::PathBuf)> = glb::all_models()
        .into_iter()
        .filter(|(folder, name, _)| !assembled(folder, name))
        .collect();
    // Four at a time: every model's pieces against their neighbours is the
    // suite's slowest check.
    let chunks: Vec<_> = models.chunks(models.len().div_ceil(4).max(1)).collect();
    let mut out: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|(folder, name, path)| model_detached(folder, name, path))
                        .collect::<Vec<String>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    out.sort();
    out
}

/// One model's detached pieces, as a line, if it has any.
fn model_detached(folder: &str, name: &str, path: &std::path::Path) -> Option<String> {
    let all = glb::triangles(path).ok()?;
    // Nodes that move apart (a gun's bolt, a wreck's door) by themselves;
    // a vehicle's one road wheel is copied to its stations by the game,
    // so it's left out; `_lod` stand-ins never count.
    let moving = matches!(folder, "guns" | "mags") || name.starts_with("wreck_");
    let mut nodes: Vec<String> = all.iter().map(|t| t.node.clone()).collect();
    nodes.sort();
    nodes.dedup();
    let keep =
        |t: &glb::Tri| !t.node.contains("_lod") && !(folder == "vehicles" && t.node == "wheel");
    let sets: Vec<(String, Vec<[Vec3; 3]>)> = if moving {
        nodes
            .iter()
            .filter(|n| !n.contains("_lod"))
            .map(|n| {
                (
                    n.clone(),
                    all.iter().filter(|t| &t.node == n).map(|t| t.p).collect(),
                )
            })
            .collect()
    } else {
        vec![(
            String::new(),
            all.iter().filter(|t| keep(t)).map(|t| t.p).collect(),
        )]
    };
    let mut lines = Vec::new();
    for (node, tris) in sets {
        for (at, size, gap) in detached(&tris) {
            if size.length() < CRUMB {
                continue;
            }
            lines.push(format!(
                "{}{:.3}x{:.3}x{:.3} m at ({:.3}, {:.3}, {:.3}), {:.0} mm off",
                if node.is_empty() {
                    String::new()
                } else {
                    format!("{node}: ")
                },
                size.x,
                size.y,
                size.z,
                at.x,
                at.y,
                at.z,
                gap * 1000.0
            ));
        }
    }
    (!lines.is_empty()).then(|| format!("{folder}/{name}: {}", lines.join("; ")))
}

/// A box's six faces: (outward normal, centre, half extents along two
/// axes in the face, those axes).
fn faces(size: Vec3, t: &Transform) -> [(Vec3, Vec3, Vec3, Vec3); 6] {
    let h = size * 0.5;
    let r = t.rotation;
    let (x, y, z) = (r * Vec3::X, r * Vec3::Y, r * Vec3::Z);
    let c = t.translation;
    [
        (x, c + x * h.x, y * h.y, z * h.z),
        (-x, c - x * h.x, y * h.y, z * h.z),
        (y, c + y * h.y, x * h.x, z * h.z),
        (-y, c - y * h.y, x * h.x, z * h.z),
        (z, c + z * h.z, x * h.x, y * h.y),
        (-z, c - z * h.z, x * h.x, y * h.y),
    ]
}

/// The narrowest a crack must be to show (m): closer, the boxes' own
/// rounding closes it.
pub const SLIT_MIN: f32 = 0.002;
/// The widest gap that's a crack and not a gap meant to be there (m).
pub const SLIT_MAX: f32 = 0.04;
/// The least area two faces must share across the crack (m²).
pub const SLIT_AREA: f32 = 0.02;

/// Two boxes of a building facing each other across a crack: (a, b,
/// gap, shared area, where).
pub fn slits(geo: &buildings::Geo) -> Vec<(usize, usize, f32, f32, Vec3)> {
    // Boxes a model is drawn in place of aren't drawn.
    let mut replaced = vec![false; geo.pieces.len()];
    for d in &geo.props {
        for i in d.pieces.clone() {
            replaced[i] = true;
        }
    }
    // Each piece's faces: a box's six, a gable's underside (it stands on
    // its wall's top: the playtest's crack, 0:22:50, was under one).
    type Faces = Vec<(Vec3, Vec3, Vec3, Vec3)>;
    let solid: Vec<(usize, Vec3, Transform, Faces)> = geo
        .pieces
        .iter()
        .enumerate()
        .filter(|(i, p)| p.mat != Mat::Glass && !replaced[*i])
        .filter_map(|(i, p)| match p.shape {
            Shape::Box(s) => Some((i, s, p.at, faces(s, &p.at).to_vec())),
            Shape::Prism(s) => {
                let r = p.at.rotation;
                let under = (
                    -(r * Vec3::Y),
                    p.at.translation,
                    r * Vec3::X * s.x * 0.5,
                    r * Vec3::Z * s.z * 0.5,
                );
                Some((i, s, p.at, vec![under]))
            }
            _ => None,
        })
        .collect();
    let inside = |p: Vec3| {
        geo.pieces.iter().any(|q| {
            let s = match q.shape {
                Shape::Box(s) | Shape::Prism(s) => s,
                Shape::Cylinder(r, h) => Vec3::new(2.0 * r, h, 2.0 * r),
            };
            let l = q.at.rotation.inverse() * (p - q.at.translation);
            // On a face counts: a crack's middle on the seam between two
            // boxes that fill it (a wall's end and its corner cap).
            (l.abs() - s * 0.5).max_element() < 0.0005
        })
    };
    let mut out = Vec::new();
    for x in 0..solid.len() {
        for y in x + 1..solid.len() {
            let (ia, sa, ta, fa) = &solid[x];
            let (ib, sb, tb, fb) = &solid[y];
            let (ia, ib) = (*ia, *ib);
            // Boards spaced apart on purpose (a deck, a stack of sawn
            // planks, a fence): same stuff, both thin.
            let thin = |s: &Vec3| s.min_element() <= 0.1;
            if geo.pieces[ia].mat == geo.pieces[ib].mat && thin(sa) && thin(sb) {
                continue;
            }
            if ta.translation.distance(tb.translation)
                > (sa.length() + sb.length()) * 0.5 + SLIT_MAX
            {
                continue;
            }
            for &(na, ca, ua, va) in fa {
                for &(nb, cb, ub, vb) in fb {
                    // Facing each other, parallel.
                    if na.dot(nb) > -0.999 {
                        continue;
                    }
                    let gap = na.dot(cb - ca);
                    if !(SLIT_MIN..=SLIT_MAX).contains(&gap) {
                        continue;
                    }
                    // The shared rectangle (axis-aligned in a's face frame,
                    // both boxes turned the same way about it).
                    let (du, dv) = (ua.normalize(), va.normalize());
                    let span = |c: Vec3, u: Vec3, v: Vec3, d: Vec3| {
                        let m = d.dot(c - ca);
                        let r = u.dot(d).abs() + v.dot(d).abs();
                        (m - r, m + r)
                    };
                    let (a0, a1) = span(ca, ua, va, du);
                    let (b0, b1) = span(cb, ub, vb, du);
                    let (c0, c1) = span(ca, ua, va, dv);
                    let (e0, e1) = span(cb, ub, vb, dv);
                    let (lu, hu) = (a0.max(b0), a1.min(b1));
                    let (lv, hv) = (c0.max(e0), c1.min(e1));
                    if hu <= lu || hv <= lv {
                        continue;
                    }
                    let area = (hu - lu) * (hv - lv);
                    if area < SLIT_AREA {
                        continue;
                    }
                    // The crack's middle, unless something fills it.
                    let mid = ca + du * (lu + hu) * 0.5 + dv * (lv + hv) * 0.5 + na * gap * 0.5;
                    if inside(mid) {
                        continue;
                    }
                    out.push((ia, ib, gap, area, mid));
                }
            }
        }
    }
    out
}

/// A building's doors whose hinge isn't in a jamb or whose leaf doesn't
/// reach the other: (door, what's wrong).
pub fn hinges(geo: &buildings::Geo) -> Vec<(usize, String)> {
    let solid = |p: Vec3| {
        geo.pieces.iter().any(|q| {
            if q.mat == Mat::Glass {
                return false;
            }
            let s = match q.shape {
                Shape::Box(s) | Shape::Prism(s) => s,
                Shape::Cylinder(r, h) => Vec3::new(2.0 * r, h, 2.0 * r),
            };
            let l = q.at.rotation.inverse() * (p - q.at.translation);
            (l.abs() - s * 0.5).max_element() < 0.005
        })
    };
    let mut out = Vec::new();
    for (i, d) in geo.doors.iter().enumerate() {
        let along = bevy::math::Quat::from_rotation_y(d.yaw) * Vec3::X;
        let mid = d.hinge + Vec3::Y * d.height * 0.5;
        if !solid(mid - along * 0.03) {
            out.push((
                i,
                format!(
                    "hinge at ({:.2}, {:.2}, {:.2}) is in no jamb",
                    d.hinge.x, d.hinge.y, d.hinge.z
                ),
            ));
        }
        if !solid(mid + along * (d.width + 0.03)) {
            out.push((
                i,
                format!("leaf's far edge ({:.2} m) meets no jamb", d.width),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(c: Vec3, h: f32) -> Vec<[Vec3; 3]> {
        let p = |x: f32, y: f32, z: f32| c + Vec3::new(x, y, z) * h;
        let q = [
            [
                p(1., -1., -1.),
                p(1., 1., -1.),
                p(1., 1., 1.),
                p(1., -1., 1.),
            ],
            [
                p(-1., -1., 1.),
                p(-1., 1., 1.),
                p(-1., 1., -1.),
                p(-1., -1., -1.),
            ],
            [
                p(-1., 1., -1.),
                p(-1., 1., 1.),
                p(1., 1., 1.),
                p(1., 1., -1.),
            ],
            [
                p(-1., -1., 1.),
                p(-1., -1., -1.),
                p(1., -1., -1.),
                p(1., -1., 1.),
            ],
            [
                p(-1., -1., 1.),
                p(1., -1., 1.),
                p(1., 1., 1.),
                p(-1., 1., 1.),
            ],
            [
                p(1., -1., -1.),
                p(-1., -1., -1.),
                p(-1., 1., -1.),
                p(1., 1., -1.),
            ],
        ];
        q.iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect()
    }

    #[test]
    fn a_piece_off_its_neighbour_is_detached() {
        // A body, a flap touching it, a flap 2 cm off, and a pin through.
        let mut t = cube(Vec3::new(0.0, 0.5, 0.0), 0.5);
        t.extend(cube(Vec3::new(0.6, 0.5, 0.0), 0.1));
        t.extend(cube(Vec3::new(0.0, 0.5, 0.65), 0.1));
        t.extend(
            cube(Vec3::new(-0.5, 0.5, 0.0), 0.05)
                .into_iter()
                .map(|t| t.map(|p| Vec3::new((p.x + 0.5) * 8.0 - 0.5, p.y, p.z))),
        );
        assert_eq!(islands(&t).len(), 4);
        let d = detached(&t);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!((d[0].0.z - 0.65).abs() < 1e-4);
        assert!((d[0].2 - 0.05).abs() < 1e-3, "{}", d[0].2);
    }

    /// Every model's pieces hold together, by what touches what. Prints
    /// every detached piece.
    #[test]
    fn model_pieces_touch_what_holds_them() {
        let all = detached_findings();
        for l in &all {
            println!("{l}");
        }
        for k in DETACHED_KNOWN {
            if !all.iter().any(|l| l.starts_with(k)) {
                println!("fixed, out of DETACHED_KNOWN: {k}");
            }
        }
        let bad: Vec<&String> = all
            .iter()
            .filter(|l| !DETACHED_KNOWN.iter().any(|k| l.starts_with(k)))
            .collect();
        assert!(
            bad.is_empty(),
            "models with pieces in the air:\n{}",
            bad.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// The rig's flaps lie against their slot pieces: each touches its
    /// piece, and an open one (shown while the slot holds something)
    /// stands no further out in front than the piece itself, so the front
    /// doesn't pop out when it fills (the 28 October playtest, 0:35:26 and
    /// 0:35:45).
    #[test]
    fn rig_flaps_lie_against_their_pieces() {
        let root = crate::asset_root_path().join("models/kit");
        let closed = glb::triangles(&root.join("rig.glb")).unwrap();
        let open = glb::triangles(&root.join("rig_open.glb")).unwrap();
        let of = |t: &[glb::Tri], node: &str| -> Vec<[Vec3; 3]> {
            t.iter().filter(|t| t.node == node).map(|t| t.p).collect()
        };
        let lo = |t: &[[Vec3; 3]]| t.iter().flatten().fold(Vec3::MAX, |a, p| a.min(*p));
        for kind in ["holster", "pouch", "pocket"] {
            let piece = of(&closed, kind);
            assert!(!piece.is_empty(), "no {kind}");
            for (file, flap) in [
                (&closed, format!("{kind}_flap_closed")),
                (&open, format!("{kind}_flap_open")),
            ] {
                let f = of(file, &flap);
                assert!(!f.is_empty(), "no {flap}");
                let both: Vec<[Vec3; 3]> = piece.iter().chain(&f).copied().collect();
                let loose = detached(&both);
                assert!(loose.is_empty(), "{flap} is off its {kind}: {loose:?}");
                if flap.ends_with("_open") {
                    // Slot space: -Z is out, away from the body.
                    let out = lo(&piece).z - lo(&f).z;
                    assert!(
                        out <= 0.0075,
                        "{flap} stands {:.0} mm out of its {kind}'s front",
                        out * 1000.0
                    );
                }
            }
        }
    }

    /// Detached pieces waiting on their owners, by the start of their line.
    const DETACHED_KNOWN: &[&str] = &[
        // The first run's tally (30 October), to work down: mostly pieces
        // 2-3 mm off what holds them, and parts shut inside shells (an
        // eye's iris in its glass). Take a model out when it's clean.
        "guns/mp133:",
        "guns/pm_pistol:",
        "guns/rpk74:",
        "guns/sks:",
        "guns/svd:",
        "items/car_bonnet:",
        "items/car_door:",
        "items/stock_sks:",
        "props/bar_rack:",
        "props/beam_charred:",
        "props/books_charred:",
        "props/cellar_vent:",
        "props/charcoal_sack:",
        "props/chopping_block:",
        "props/clothes_line:",
        "props/counter_drawers:",
        "props/depot_conveyor:",
        "props/depot_crate_long_open:",
        "props/depot_crate_small_open:",
        "props/dishes_broken:",
        "props/filter_unit:",
        "props/kept_bicycle:",
        "props/rope_coil:",
        "props/snowshoes:",
        "props/table_burnt:",
        "props/washstand:",
        "props/woodpile:",
        "props/woodpile_lean:",
        "vehicles/bench:",
        "vehicles/stash_crate:",
        "vehicles/truck:",
    ];

    /// No building has a crack between its boxes, and every door hangs in
    /// its jambs.
    #[test]
    fn buildings_have_no_cracks_and_doors_hang_in_their_jambs() {
        let mut bad = Vec::new();
        for id in buildings::PREFABS {
            let Some(geo) = buildings::prefab(id) else {
                continue;
            };
            for (a, b, gap, area, at) in slits(&geo) {
                bad.push(format!(
                    "{id}: a {:.0} mm crack between boxes {a} ({:?}) and {b} ({:?}), {area:.2} m², at ({:.2}, {:.2}, {:.2})",
                    gap * 1000.0,
                    geo.pieces[a].mat,
                    geo.pieces[b].mat,
                    at.x,
                    at.y,
                    at.z
                ));
            }
            for (i, what) in hinges(&geo) {
                bad.push(format!("{id}: door {i}: {what}"));
            }
        }
        for b in &bad {
            println!("{b}");
        }
        for k in BUILDINGS_KNOWN {
            if !bad.iter().any(|l| l.starts_with(k)) {
                println!("fixed, out of BUILDINGS_KNOWN: {k}");
            }
        }
        let left: Vec<&String> = bad
            .iter()
            .filter(|l| !BUILDINGS_KNOWN.iter().any(|k| l.starts_with(k)))
            .collect();
        assert!(
            left.is_empty(),
            "cracks and hinges:\n{}",
            left.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Building faults waiting on their owners, by the start of their line.
    /// Every prefab is crack-free (30 October, the third round): keep it
    /// that way, and fix a new one in its builder.
    const BUILDINGS_KNOWN: &[&str] = &[];
}
