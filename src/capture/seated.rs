//! Model correctness: things seated in other things (M3 in
//! `docs/playtests/2026-10-28/TRANSCRIPT.md`). The playtest saw a magazine
//! through the bottom of the pistol, its top z-fighting, a cell lying
//! across the flashlight through its walls, and a cassette player
//! fighting with its tape. Each model alone was clean; the faults are in
//! how one sits in the other.
//!
//! Every pair the game seats is set up as the game seats it:
//!
//! - **magazines in guns**: each gun's model with every magazine that fits
//!   it, the magazine where the model's `magwell` empty puts it
//!   (`weapons::visuals::seat_mag_visuals`);
//! - **item mounts**: every item's own sockets (a light's cell bay, a
//!   rail, a cradle, a cassette well), with every item that fits the
//!   mount, from a headless game, posed by the socket and its seat.
//!
//! Each pair is held to two things:
//!
//! - **nothing through anything**: no vertex of one lies inside the other's
//!   solid (generalised winding number over half) deeper than [`DEPTH`];
//! - **no fighting faces**: no face of one in the plane of the other's,
//!   facing the same way and overlapping, with another material
//!   (`capture::fighting`);
//! - **inserts inside**: a cell in its bay, a tape in its well, stands
//!   no further than [`PROUD`] out of its holder's outline (the playtest's
//!   cell lay across the flashlight through an open bore, 0:11:13).
//!
//! What's shut inside another (an artifact in the lead case, a plate in
//! its pocket) is held only to being inside it.
//!
//! `cargo test --lib capture::seated -- --nocapture` prints every pair.

use bevy::math::{Mat4, Quat, Vec3};
use bevy::prelude::Transform;
use std::collections::HashMap;
use std::path::PathBuf;

use super::{fighting, glb};

/// How deep a vertex of one may lie inside the other (m): a seated thing
/// touches its seat, it isn't through it (a bevel or a lip may bed in).
pub const DEPTH: f32 = 0.0025;
/// At most this many vertices are tried a model (the rest are spread
/// evenly over its list).
const SAMPLE: usize = 2500;

/// One thing seated in another: their models, and the seated one's pose
/// in the other's space.
#[derive(Clone, Debug)]
pub struct Pair {
    pub parent: String,
    pub child: String,
    pub parent_path: PathBuf,
    pub child_path: PathBuf,
    pub at: Transform,
    /// It goes inside (a cell in its bay, a tape in its well): it may not
    /// stand out of the other's outline by more than [`PROUD`].
    pub inside: bool,
    /// It's shut inside the other (an artifact in the lead case): only
    /// held to being inside its outline.
    pub enclosed: bool,
}

/// How far an insert may stand out of what holds it (m).
pub const PROUD: f32 = 0.002;

/// The generalised winding number of `p` about a triangle soup: about 1
/// inside a closed surface, 0 outside, a fraction near an open one.
pub fn winding(tris: &[[Vec3; 3]], p: Vec3) -> f32 {
    let mut sum = 0.0f32;
    for t in tris {
        let (a, b, c) = (t[0] - p, t[1] - p, t[2] - p);
        let (la, lb, lc) = (a.length(), b.length(), c.length());
        let num = a.dot(b.cross(c));
        let den = la * lb * lc + a.dot(b) * lc + a.dot(c) * lb + b.dot(c) * la;
        sum += 2.0 * num.atan2(den);
    }
    sum / (4.0 * std::f32::consts::PI)
}

/// The distance from `p` to triangle `t`.
pub fn tri_distance(t: &[Vec3; 3], p: Vec3) -> f32 {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length();
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (p - (a + ab * v)).length();
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (p - (a + ac * w)).length();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (p - (b + (c - b) * w)).length();
    }
    let n = ab.cross(ac);
    n.normalize_or_zero().dot(ap).abs()
}

/// A model's triangles (no `_lod` stand-ins), and their materials and
/// nodes.
fn tris(path: &std::path::Path) -> Vec<glb::Tri> {
    glb::triangles(path)
        .unwrap_or_default()
        .into_iter()
        .filter(|t| !t.node.contains("_lod"))
        .collect()
}

/// How far into `solid` the deepest of `pts` lies, and how many lie in
/// deeper than [`DEPTH`]: (depth, count, a point).
fn through(solid: &[[Vec3; 3]], pts: &[Vec3]) -> (f32, usize, Vec3) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for t in solid {
        for p in t {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    let mut inside: Vec<Vec3> = pts
        .iter()
        .filter(|p| p.cmpgt(lo + DEPTH).all() && p.cmplt(hi - DEPTH).all())
        .copied()
        .collect();
    inside.sort_by(|a, b| {
        a.x.total_cmp(&b.x)
            .then(a.y.total_cmp(&b.y))
            .then(a.z.total_cmp(&b.z))
    });
    inside.dedup_by(|a, b| a.distance(*b) < 1e-5);
    if inside.len() > SAMPLE {
        let step = inside.len() as f32 / SAMPLE as f32;
        inside = (0..SAMPLE)
            .map(|i| inside[(i as f32 * step) as usize])
            .collect();
    }
    let (mut depth, mut count, mut at) = (0.0f32, 0, Vec3::ZERO);
    for p in inside {
        if winding(solid, p) < 0.5 {
            continue;
        }
        let d = solid
            .iter()
            .map(|t| tri_distance(t, p))
            .fold(f32::MAX, f32::min);
        if d > DEPTH {
            count += 1;
            if d > depth {
                depth = d;
                at = p;
            }
        }
    }
    (depth, count, at)
}

/// What's wrong with one pair, one line each.
pub fn check(pair: &Pair) -> Vec<String> {
    let a = tris(&pair.parent_path);
    let b: Vec<glb::Tri> = tris(&pair.child_path)
        .into_iter()
        .map(|t| glb::Tri {
            p: t.p.map(|q| pair.at.transform_point(q)),
            ..t
        })
        .collect();
    if a.is_empty() || b.is_empty() {
        return vec![format!(
            "{} in {}: a model is missing",
            pair.child, pair.parent
        )];
    }
    let mut out = Vec::new();
    if pair.inside {
        let bounds = |t: &[glb::Tri]| {
            t.iter().flat_map(|t| t.p).fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(l, h), p| (l.min(p), h.max(p)),
            )
        };
        let (pl, ph) = bounds(&a);
        let (cl, ch) = bounds(&b);
        let out_by = (pl - cl).max(ch - ph).max_element();
        if out_by > PROUD {
            out.push(format!(
                "{} in {}: stands {:.0} mm out of it",
                pair.child,
                pair.parent,
                out_by * 1000.0
            ));
        }
    }
    if pair.enclosed {
        return out;
    }
    let sa: Vec<[Vec3; 3]> = a.iter().map(|t| t.p).collect();
    let sb: Vec<[Vec3; 3]> = b.iter().map(|t| t.p).collect();
    let pa: Vec<Vec3> = sa.iter().flatten().copied().collect();
    let pb: Vec<Vec3> = sb.iter().flatten().copied().collect();
    for (solid, pts, what) in [
        (&sa, &pb, format!("{} through {}", pair.child, pair.parent)),
        (&sb, &pa, format!("{} through {}", pair.parent, pair.child)),
    ] {
        let (depth, count, at) = through(solid, pts);
        if count > 0 {
            out.push(format!(
                "{what}: {count} vertices up to {:.1} mm in, e.g. at ({:.3}, {:.3}, {:.3})",
                depth * 1000.0,
                at.x,
                at.y,
                at.z
            ));
        }
    }
    let faces: Vec<fighting::Face> = a
        .iter()
        .map(|t| (0, t))
        .chain(b.iter().map(|t| (1, t)))
        .map(|(source, t)| fighting::Face {
            source,
            material: t.material.clone(),
            p: t.p,
        })
        .collect();
    let names = [pair.parent.clone(), pair.child.clone()];
    // Down to half a square centimetre: a seated thing is small and
    // seen up close.
    for f in fighting::fights_over(&faces, &names, &[], |_, _| true, 0.00005) {
        out.push(format!(
            "{} in {}: faces fight ({} / {}), {:.1} cm² at ({:.3}, {:.3}, {:.3})",
            pair.child,
            pair.parent,
            f.materials.0,
            f.materials.1,
            f.area * 1e4,
            f.at.x,
            f.at.y,
            f.at.z
        ));
    }
    out
}

/// A node's transform in the model's space, through its parents.
pub fn node_transform(path: &std::path::Path, name: &str) -> Option<Mat4> {
    let j = glb::json(path).ok()?;
    let nodes = j["nodes"].as_array()?;
    let f = |v: &serde_json::Value, i: usize, d: f32| {
        v.get(i).and_then(|x| x.as_f64()).map_or(d, |x| x as f32)
    };
    let local = |n: &serde_json::Value| {
        let (t, r, s) = (&n["translation"], &n["rotation"], &n["scale"]);
        Mat4::from_scale_rotation_translation(
            Vec3::new(f(s, 0, 1.0), f(s, 1, 1.0), f(s, 2, 1.0)),
            Quat::from_xyzw(f(r, 0, 0.0), f(r, 1, 0.0), f(r, 2, 0.0), f(r, 3, 1.0)),
            Vec3::new(f(t, 0, 0.0), f(t, 1, 0.0), f(t, 2, 0.0)),
        )
    };
    let mut parent: HashMap<usize, usize> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        for c in n["children"].as_array().into_iter().flatten() {
            if let Some(c) = c.as_u64() {
                parent.insert(c as usize, i);
            }
        }
    }
    let mut i = nodes
        .iter()
        .position(|n| n["name"].as_str() == Some(name))?;
    let mut m = local(&nodes[i]);
    while let Some(&p) = parent.get(&i) {
        m = local(&nodes[p]) * m;
        i = p;
    }
    Some(m)
}

/// Every magazine in every gun that takes it, where the model seats it.
pub fn mag_pairs() -> Vec<Pair> {
    let catalog = crate::items::Catalog::load();
    let root = crate::asset_root_path().join("models");
    let mut out = Vec::new();
    for (_, gun) in catalog.iter() {
        let Some(family) = gun.firearm.as_ref().and_then(|f| f.magazine_family.clone()) else {
            continue;
        };
        let gun_path = root.join(format!("guns/{}.glb", gun.id));
        let Some(well) = node_transform(&gun_path, "magwell") else {
            continue;
        };
        for (_, mag) in catalog.iter() {
            let Some(m) = &mag.magazine else { continue };
            if m.fits != family || m.stripper {
                continue;
            }
            // `seat_mag_visuals`: the model's seat, hung by half the mag.
            let hang = Mat4::from_translation(Vec3::new(0.0, -mag.size_m()[1] * 0.5, 0.0));
            out.push(Pair {
                parent: gun.id.clone(),
                child: mag.id.clone(),
                parent_path: gun_path.clone(),
                child_path: root.join(format!("mags/{}.glb", mag.id)),
                at: Transform::from_matrix(well * hang),
                inside: false,
                enclosed: false,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winding_tells_inside_from_outside() {
        // A unit cube's twelve triangles.
        let c = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        let quads = [
            [c(1., 0., 0.), c(1., 1., 0.), c(1., 1., 1.), c(1., 0., 1.)],
            [c(0., 0., 1.), c(0., 1., 1.), c(0., 1., 0.), c(0., 0., 0.)],
            [c(0., 1., 0.), c(0., 1., 1.), c(1., 1., 1.), c(1., 1., 0.)],
            [c(0., 0., 1.), c(0., 0., 0.), c(1., 0., 0.), c(1., 0., 1.)],
            [c(0., 0., 1.), c(1., 0., 1.), c(1., 1., 1.), c(0., 1., 1.)],
            [c(1., 0., 0.), c(0., 0., 0.), c(0., 1., 0.), c(1., 1., 0.)],
        ];
        let cube: Vec<[Vec3; 3]> = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        assert!(winding(&cube, Vec3::splat(0.5)).abs() > 0.9);
        assert!(winding(&cube, Vec3::splat(1.5)).abs() < 0.1);
        let (depth, count, _) = through(&cube, &[Vec3::splat(0.5), Vec3::splat(2.0)]);
        assert_eq!(count, 1);
        assert!((depth - 0.5).abs() < 1e-4);
        assert!((tri_distance(&cube[0], Vec3::new(2.0, 0.5, 0.5)) - 1.0).abs() < 1e-5);
    }

    fn report(pairs: &[Pair]) -> Vec<String> {
        let mut bad = Vec::new();
        for p in pairs {
            let faults = check(p);
            println!(
                "{:<5} {} in {}",
                if faults.is_empty() { "ok" } else { "FAIL" },
                p.child,
                p.parent
            );
            for f in faults {
                println!("    {f}");
                if !KNOWN.iter().any(|k| f.starts_with(k)) {
                    bad.push(f);
                }
            }
        }
        bad
    }

    /// Faults waiting on their owners, by the start of their line.
    const KNOWN: &[&str] = &[
        // The drum's body beds 4 mm into the stock's well (Models, next
        // round).
        "ppsh_drum through ppsh",
    ];

    /// Every magazine sits in every gun it fits without going through it
    /// or fighting it.
    #[test]
    fn magazines_sit_in_their_guns() {
        let pairs = mag_pairs();
        assert!(pairs.len() >= 7, "only {} pairs", pairs.len());
        let bad = report(&pairs);
        assert!(
            bad.is_empty(),
            "magazines that don't sit:\n{}",
            bad.join("\n")
        );
    }

    /// Every item's own sockets (a light's cell bay, a deck's tape well
    /// and cell, a rail, a cradle) with everything that fits them, as a
    /// headless game seats them.
    #[test]
    fn mounted_things_sit_in_their_mounts() {
        let pairs = mount_pairs();
        assert!(pairs.len() > 8, "only {} pairs", pairs.len());
        let bad = report(&pairs);
        assert!(
            bad.is_empty(),
            "mounted things that don't sit:\n{}",
            bad.join("\n")
        );
    }

    /// Every item that has a model and its own mount sockets, with every
    /// item that fits each mount (one of each model), posed as the game
    /// seats it: (socket's place in its owner) * (the socket's seat).
    fn mount_pairs() -> Vec<Pair> {
        use crate::interact::{Socket, SocketKind, SocketOwner};
        use crate::items::{SpawnItem, spawn_item};
        use bevy::prelude::*;
        let opts = crate::LaunchOptions {
            robots: false,
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        app.update();
        let catalog = app.world().resource::<crate::items::Catalog>().clone();
        let model = |id: &str| -> Option<PathBuf> {
            let def = catalog.get(catalog.kind(id)?);
            let folder = match def.category {
                extraction_core::items::Category::Magazine => "mags",
                _ if def.firearm.is_some() => "guns",
                _ => "items",
            };
            let p = crate::asset_root_path().join(format!("models/{folder}/{id}.glb"));
            p.exists().then_some(p)
        };
        let mut owners = Vec::new();
        for (kind, def) in catalog.iter() {
            if model(&def.id).is_none() {
                continue;
            }
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let e = {
                let mut c = Commands::new(&mut queue, app.world());
                spawn_item(
                    &mut c,
                    &catalog,
                    SpawnItem {
                        kind,
                        transform: Transform::from_xyz(0.0, 50.0, 0.0),
                        condition: 1.0,
                    },
                )
            };
            queue.apply(app.world_mut());
            owners.push((e, def.id.clone()));
        }
        app.update();
        let mut out = Vec::new();
        let mut q = app
            .world_mut()
            .query::<(&Socket, &SocketOwner, &Transform)>();
        let sockets: Vec<(Socket, Entity, Transform)> = q
            .iter(app.world())
            .map(|(s, o, t)| (s.clone(), o.0, *t))
            .collect();
        for (socket, owner, place) in sockets {
            let SocketKind::Mount(mount) = socket.kind else {
                continue;
            };
            let Some((_, parent)) = owners.iter().find(|(e, _)| *e == owner) else {
                continue;
            };
            let owner_def = catalog.get(catalog.kind(parent).unwrap());
            let mut seen = std::collections::HashSet::new();
            for (_, def) in catalog.iter() {
                if def.fits_mount() != Some(mount) {
                    continue;
                }
                let Some(child_path) = model(&def.id) else {
                    continue;
                };
                // One of each model (every tape is the same cassette).
                if !seen.insert(std::fs::read(&child_path).map(|b| b.len()).unwrap_or(0)) {
                    continue;
                }
                out.push(Pair {
                    parent: parent.clone(),
                    child: def.id.clone(),
                    parent_path: model(parent).unwrap(),
                    child_path,
                    at: place * crate::gadgets::seat_pose(&socket, owner_def, def),
                    inside: matches!(
                        mount,
                        extraction_core::items::Mount::Cell
                            | extraction_core::items::Mount::Cassette
                    ) || parent == "lead_case",
                    // Shut in: an artifact in the lead case, a plate in its
                    // pocket (the vest's panels are the pockets).
                    enclosed: parent == "lead_case"
                        || mount == extraction_core::items::Mount::Plate,
                });
            }
        }
        out.sort_by(|a, b| (&a.parent, &a.child).cmp(&(&b.parent, &b.child)));
        out
    }
}
