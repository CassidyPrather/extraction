//! The "looks wrong" sweep's cheap half: checks on the models' data that
//! need no renderer, so they run as tests. The rendered half (z-fighting,
//! fallback colours, blank views) is `capture::sweep` and
//! `scripts/capture_sweep.py`.
//!
//! - **Believable size**: every model's bounds against what its folder
//!   holds (a gun is not 4 m long, a prop not 2 mm; a model exported in
//!   centimetres is a hundred times too big).
//! - **Furniture where its boxes were**: a building's furniture models take
//!   the place of blockout boxes (`PropDef::pieces`); each must stand on
//!   the floor its boxes stood on and be about their size, or it floats,
//!   sinks or swamps the room once the boxes are gone.
//! - **No outside images**: a model's textures are inside the file, or a
//!   missing one renders as flat grey.
//! - **Footprint, clearance and collider agree**: a prefab as drawn and
//!   as it collides reaches no further from its footing than the radius
//!   the map clears round it, or it stands in the trees and rocks the map
//!   put just outside; the clearance is not much bigger than that (it
//!   costs cover); nothing collides where nothing's drawn; its footprint
//!   claims no more ground than it stands on.
//! - **Held together**: every part of a model touches another part or the
//!   ground; a nail over its loop or a board in mid-air shows from across
//!   a room.

use bevy::math::Vec3;

use super::glb;
use crate::world::buildings::{self, Shape};

/// What a model in each folder may measure, metres: (smallest largest
/// side, largest largest side).
pub fn size_range(folder: &str) -> (f32, f32) {
    match folder {
        // The Mosin with its bayonet fixed is 1.76 m.
        "guns" => (0.15, 1.8),
        "mags" => (0.04, 0.5),
        "items" => (0.01, 1.6),
        "kit" => (0.15, 1.0),
        "hands" => (0.08, 0.4),
        // The blast furnace is the tallest landmark, at 59 m.
        "props" => (0.03, 70.0),
        "robots" => (0.3, 40.0),
        "vehicles" => (0.3, 12.0),
        _ => (0.01, 60.0),
    }
}

/// Models whose largest side is out of their folder's range.
pub fn size_findings() -> Vec<String> {
    let mut bad = Vec::new();
    for (folder, name, path) in glb::all_models() {
        let info = match glb::read(&path) {
            Ok(i) => i,
            Err(e) => {
                bad.push(format!("{folder}/{name}: unreadable: {e}"));
                continue;
            }
        };
        let Some((lo, hi)) = info.bounds() else {
            bad.push(format!("{folder}/{name}: no meshes"));
            continue;
        };
        let size = (hi - lo).max_element();
        let (min, max) = size_range(&folder);
        if !(min..=max).contains(&size) {
            bad.push(format!(
                "{folder}/{name}: {size:.3} m across, outside {min}..{max} m for {folder}"
            ));
        }
    }
    bad
}

/// A blockout piece's bounds in building space.
fn piece_bounds(p: &buildings::Piece) -> Option<(Vec3, Vec3)> {
    let half = match p.shape {
        Shape::Box(s) => s * 0.5,
        Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
        // Gables are roofs, never furniture.
        Shape::Prism(_) => return None,
    };
    Some(corners_bounds(half, &p.at))
}

fn corners_bounds(half: Vec3, at: &bevy::prelude::Transform) -> (Vec3, Vec3) {
    corners(-half, half, at)
}

fn corners(lo: Vec3, hi: Vec3, at: &bevy::prelude::Transform) -> (Vec3, Vec3) {
    let (mut a, mut b) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for k in 0..8 {
        let c = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let w = at.transform_point(c);
        a = a.min(w);
        b = b.max(w);
    }
    (a, b)
}

/// One furniture group against the boxes it replaces.
#[derive(Debug, Clone)]
pub struct Fit {
    pub prefab: String,
    pub models: String,
    /// Model bottom minus the boxes' bottom, metres (+ floats, - sinks).
    pub gap: f32,
    /// Model extent over the boxes', per axis (building space).
    pub ratio: Vec3,
    /// Furniture (inside), or a fitting or shell outside.
    pub inside: bool,
}

/// Every furniture group in every prefab, measured against its boxes.
pub fn furniture_fits() -> Vec<Fit> {
    let mut cache = std::collections::HashMap::new();
    let mut model = |name: &str| -> Option<(Vec3, Vec3)> {
        cache
            .entry(name.to_string())
            .or_insert_with(|| {
                let path = crate::asset_root_path().join(format!("models/props/{name}.glb"));
                glb::read(&path)
                    .ok()?
                    .bounds_of(|n| !n.name.contains("_lod"))
            })
            .clone()
    };
    let mut out = Vec::new();
    let ids = buildings::PREFABS
        .into_iter()
        .chain(crate::world::underground::ids());
    for id in ids {
        let Some(geo) = buildings::prefab(id) else {
            continue;
        };
        for d in &geo.props {
            if d.pieces.is_empty() || d.models.is_empty() {
                continue;
            }
            let Some((blo, bhi)) = geo.pieces[d.pieces.clone()]
                .iter()
                .filter_map(piece_bounds)
                .reduce(|(a, b), (c, e)| (a.min(c), b.max(e)))
            else {
                continue;
            };
            let mut mb: Option<(Vec3, Vec3)> = None;
            for (name, at) in &d.models {
                let Some((lo, hi)) = model(name) else {
                    continue;
                };
                let (lo, hi) = corners(lo, hi, at);
                mb = Some(match mb {
                    Some((a, b)) => (a.min(lo), b.max(hi)),
                    None => (lo, hi),
                });
            }
            let Some((mlo, mhi)) = mb else {
                continue;
            };
            let names: Vec<&str> = d.models.iter().map(|(n, _)| *n).collect();
            out.push(Fit {
                prefab: id.to_string(),
                models: names.join("+"),
                gap: mlo.y - blo.y,
                ratio: (mhi - mlo) / (bhi - blo).max(Vec3::splat(0.01)),
                inside: d.inside,
            });
        }
    }
    out
}

/// A prefab's three measures of itself, building-local, metres: what it
/// draws, what collides, and what the map thinks it is.
#[derive(Clone, Debug)]
pub struct Measures {
    pub id: &'static str,
    /// Drawn (boxes, gables, cylinders and models): bounds, low then high.
    pub drawn: (Vec3, Vec3),
    /// Its compound collider's bounds (`buildings::body`).
    pub collides: Option<(Vec3, Vec3)>,
    /// How far from its footing it reaches as drawn, across the ground.
    pub reach: f32,
    /// `map::prefab_footprint`.
    pub footprint: [f32; 3],
    /// `map::prefab_clearance`.
    pub clearance: f32,
}

/// The corners of a piece's shape where it stands.
fn piece_corners(p: &buildings::Piece) -> [Vec3; 8] {
    let half = match p.shape {
        Shape::Box(s) => s * 0.5,
        Shape::Prism(s) => Vec3::new(s.x, s.y * 0.5, s.z) * 0.5,
        Shape::Cylinder(r, h) => Vec3::new(r, h * 0.5, r),
    };
    let mut out = [Vec3::ZERO; 8];
    for (k, c) in out.iter_mut().enumerate() {
        let sign = Vec3::new(
            if k & 1 == 0 { -1.0 } else { 1.0 },
            if k & 2 == 0 { -1.0 } else { 1.0 },
            if k & 4 == 0 { -1.0 } else { 1.0 },
        );
        *c = p.at.transform_point(sign * half);
    }
    out
}

fn bounds(points: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    points.fold(None, |b, p| match b {
        None => Some((p, p)),
        Some((lo, hi)) => Some((lo.min(p), hi.max(p))),
    })
}

/// Every surface prefab's [`Measures`].
pub fn measures() -> Vec<Measures> {
    buildings::PREFABS
        .into_iter()
        .filter_map(|id| {
            let geo = buildings::prefab(id)?;
            let (faces, _, _) = super::fighting::assemble(id)?;
            // `assemble` draws the boxes and models; gables and cylinders
            // are drawn too.
            let mut replaced = vec![false; geo.pieces.len()];
            for d in &geo.props {
                for i in d.pieces.clone() {
                    replaced[i] = true;
                }
            }
            let shapes: Vec<Vec3> = geo
                .pieces
                .iter()
                .zip(&replaced)
                .filter(|(p, r)| !**r && !matches!(p.shape, Shape::Box(_)))
                .flat_map(|(p, _)| piece_corners(p))
                .collect();
            let drawn: Vec<Vec3> = faces.iter().flat_map(|f| f.p).chain(shapes).collect();
            let crates = crate::destruction::wood::crate_pieces(&geo);
            let collides = bounds(
                geo.pieces
                    .iter()
                    .zip(&crates)
                    .filter(|(p, c)| p.collide && !**c)
                    .flat_map(|(p, _)| piece_corners(p)),
            );
            Some(Measures {
                id,
                reach: drawn.iter().map(|p| p.x.hypot(p.z)).fold(0.0, f32::max),
                drawn: bounds(drawn.into_iter())?,
                collides,
                footprint: extraction_core::map::prefab_footprint(id),
                clearance: extraction_core::map::prefab_clearance(id),
            })
        })
        .collect()
}

/// Images a model expects next to it (embedded ones are fine).
pub fn outside_images() -> Vec<String> {
    glb::all_models()
        .into_iter()
        .filter_map(|(folder, name, path)| {
            let info = glb::read(&path).ok()?;
            let outside: Vec<String> = info.images.into_iter().filter(|u| !u.is_empty()).collect();
            (!outside.is_empty()).then(|| format!("{folder}/{name}: {}", outside.join(", ")))
        })
        .collect()
}

/// Folders whose models aren't one piece: animals are pose sheets (each
/// pose its own node, side by side, one shown at a time), robots and the
/// open rig are kits of parts the game places itself.
fn assembled_in_game(folder: &str, name: &str) -> bool {
    matches!(folder, "animals" | "robots") || (folder == "kit" && name == "rig_open")
}

/// How far a part may stand off the rest before it counts as loose,
/// metres: crumbs a few millimetres off a charred beam don't show; a nail
/// two centimetres over its loop does.
pub const LOOSE_GAP: f32 = 0.01;

/// A model's parts more than [`LOOSE_GAP`] from any other part and from
/// its lowest point's level, as `node/material` with the gap to the
/// nearest grounded part (metres). A part is a node's triangles in one
/// material, bounded by its box; boxes are generous, so this can miss a
/// part floating near another, never flag one that touches.
pub fn loose_parts(path: &std::path::Path) -> Result<Vec<(String, f32)>, String> {
    let mut parts: Vec<(String, Vec3, Vec3)> = Vec::new();
    for t in glb::triangles(path)?
        .into_iter()
        .filter(|t| !t.node.contains("_lod"))
    {
        let key = format!("{}/{}", t.node, t.material);
        let (lo, hi) = (
            t.p[0].min(t.p[1]).min(t.p[2]),
            t.p[0].max(t.p[1]).max(t.p[2]),
        );
        match parts.iter_mut().find(|p| p.0 == key) {
            Some(p) => {
                p.1 = p.1.min(lo);
                p.2 = p.2.max(hi);
            }
            None => parts.push((key, lo, hi)),
        }
    }
    let Some(floor) = parts.iter().map(|p| p.1.y).reduce(f32::min) else {
        return Ok(Vec::new());
    };
    // Grown so boxes within LOOSE_GAP of each other, or of the floor,
    // touch.
    let grow = Vec3::splat((LOOSE_GAP - super::sweep::TOUCH) * 0.5);
    let boxes: Vec<(usize, Vec3, Vec3)> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.1 - grow, p.2 + grow))
        .collect();
    let loose = super::sweep::floating(&boxes, floor + grow.y);
    let gap = |a: &(String, Vec3, Vec3), b: &(String, Vec3, Vec3)| {
        (b.1 - a.2).max(a.1 - b.2).max_element()
    };
    Ok(loose
        .iter()
        .map(|&i| {
            let near = (0..parts.len())
                .filter(|j| !loose.contains(j))
                .map(|j| gap(&parts[i], &parts[j]))
                .fold(f32::MAX, f32::min);
            (parts[i].0.clone(), near)
        })
        .collect())
}

/// Every model with a loose part, one line each.
pub fn loose_findings() -> Vec<String> {
    let mut bad = Vec::new();
    for (folder, name, path) in glb::all_models() {
        if assembled_in_game(&folder, &name) {
            continue;
        }
        match loose_parts(&path) {
            Ok(l) if l.is_empty() => {}
            Ok(l) => {
                let parts: Vec<String> = l
                    .iter()
                    .map(|(p, g)| format!("{p} {:.0} mm off", g * 1000.0))
                    .collect();
                bad.push(format!("{folder}/{name}: {}", parts.join(", ")));
            }
            Err(e) => bad.push(format!("{folder}/{name}: unreadable: {e}")),
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_are_a_believable_size() {
        assert!(glb::all_models().len() > 100, "models not found");
        let bad = size_findings();
        assert!(bad.is_empty(), "models of an odd size:\n{}", bad.join("\n"));
    }

    #[test]
    fn models_carry_their_own_textures() {
        let bad = outside_images();
        assert!(
            bad.is_empty(),
            "models with outside images:\n{}",
            bad.join("\n")
        );
    }

    /// Prints each prefab's measures (`-- --nocapture`) and holds them to
    /// agreeing: what's drawn and what collides inside the map's clearance,
    /// the clearance not much more than it needs (trees cleared for
    /// nothing are cover lost), nothing colliding where nothing's drawn,
    /// and the footprint the map lays out and walks round no bigger than
    /// what stands on it. A ropeway's cables run on to the next pylon,
    /// over whatever is in between; they don't collide.
    #[test]
    fn prefabs_footprint_clearance_and_collider_agree() {
        let all = measures();
        assert!(all.len() > 30, "only {} prefabs", all.len());
        let mut bad = Vec::new();
        for m in &all {
            let (lo, hi) = m.drawn;
            let clear = m.clearance;
            // (Its collider's box: a corner of it may be open ground.)
            let solid = m.collides.map_or(0.0, |(a, b)| {
                [a, b]
                    .into_iter()
                    .flat_map(|c| [c.x, c.z])
                    .map(f32::abs)
                    .fold(0.0, f32::max)
            });
            println!(
                "{:<22} reaches {:5.1} m drawn, collides {solid:5.1} m out, clears {clear:5.1} m",
                m.id, m.reach
            );
            let cables = m.id.starts_with("ropeway");
            if m.reach > clear && !cables {
                bad.push(format!(
                    "{}: drawn reaches {:.1} m, the map clears {clear:.1} m",
                    m.id, m.reach
                ));
            }
            if solid > clear {
                bad.push(format!(
                    "{}: collides {solid:.1} m out, the map clears {clear:.1} m",
                    m.id
                ));
            }
            if !cables && clear > m.reach + CLEAR_SPARE {
                bad.push(format!(
                    "{}: clears {clear:.1} m, {:.1} m past all it draws ({:.1} m)",
                    m.id,
                    clear - m.reach,
                    m.reach
                ));
            }
            if let Some((a, b)) = m.collides {
                let past = (lo - a).max(b - hi).max_element();
                if past > COLLIDE_PAST {
                    bad.push(format!(
                        "{}: collides {past:.2} m past what it draws ({a:.1?}..{b:.1?} against {lo:.1?}..{hi:.1?})",
                        m.id
                    ));
                }
            }
            for (axis, half, drawn) in [
                ("x", m.footprint[0] * 0.5, lo.x.abs().max(hi.x.abs())),
                ("z", m.footprint[2] * 0.5, lo.z.abs().max(hi.z.abs())),
            ] {
                if half > drawn + FOOTPRINT_SPARE {
                    bad.push(format!(
                        "{}: footprint {:.1} m along {axis}, it stands {:.1} m",
                        m.id,
                        half * 2.0,
                        drawn * 2.0
                    ));
                }
            }
        }
        assert!(bad.is_empty(), "prefabs that disagree:\n{}", bad.join("\n"));
    }

    /// How much further than a prefab draws the map may clear round it:
    /// `prefab_clearance` gives a wall 4 m of open ground, and a little over.
    const CLEAR_SPARE: f32 = 5.0;
    /// How far a collider may stand out of what's drawn (a box's rounding,
    /// a sill under a model's).
    const COLLIDE_PAST: f32 = 0.15;
    /// How much more ground than it stands on a footprint may claim, each
    /// way along an axis.
    const FOOTPRINT_SPARE: f32 = 1.0;

    /// Prints every loose part (`-- --nocapture`) and fails on any not in
    /// `LOOSE_KNOWN`.
    #[test]
    fn models_hold_together() {
        let all = loose_findings();
        for l in &all {
            println!("{l}");
        }
        let bad: Vec<&String> = all
            .iter()
            .filter(|b| !LOOSE_KNOWN.iter().any(|k| b.starts_with(k)))
            .collect();
        assert!(
            bad.is_empty(),
            "models with parts in the air:\n{}",
            bad.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Loose parts waiting on their owners, by the start of their line.
    /// Take one out when it's fixed.
    const LOOSE_KNOWN: &[&str] = &[
        // Not loose in game: the name board hangs on the brick shell the
        // building code draws round the prop.
        "props/signal_box:",
    ];

    /// Prints every furniture group's fit (`-- --nocapture`), and holds
    /// them to standing on their boxes' floor and being about their size.
    #[test]
    fn furniture_sits_where_its_boxes_were() {
        let fits = furniture_fits();
        assert!(fits.len() > 20, "only {} furniture groups", fits.len());
        let mut bad = Vec::new();
        for f in &fits {
            // Rubble lies how it fell: into the floor, over its boxes. Burnt
            // furniture is half-ash wreckage, its boxes the heap you step
            // over (Models: world).
            let rubble = [
                "rubble",
                "ash_heap",
                "beam_fallen",
                "beam_charred",
                "_burnt",
            ]
            .iter()
            .any(|r| f.models.contains(r));
            println!(
                "{:<22} {:<40} gap {:+.3} m  size x{:.2} y{:.2} z{:.2}",
                f.prefab, f.models, f.gap, f.ratio.x, f.ratio.y, f.ratio.z
            );
            // Outside, a model may reach deeper than its boxes did (more
            // foundation), not stop short of them (a gap under it on a
            // slope); its size may differ (a ropeway's cables run off to
            // its first pylon).
            if f.gap > FLOOR_GAP || (f.inside && !rubble && f.gap < -FLOOR_GAP) {
                let how = if f.gap > 0.0 { "floats" } else { "sinks" };
                bad.push(format!(
                    "{} {}: {how} {:.3} m off its boxes' floor",
                    f.prefab,
                    f.models,
                    f.gap.abs()
                ));
            }
            let worst = f
                .ratio
                .max_element()
                .max(1.0 / f.ratio.min_element().max(1e-3));
            if f.inside && !rubble && worst > SIZE_RATIO {
                bad.push(format!(
                    "{} {}: {:.2} x {:.2} x {:.2} its boxes' size",
                    f.prefab, f.models, f.ratio.x, f.ratio.y, f.ratio.z
                ));
            }
        }
        let bad: Vec<String> = bad
            .into_iter()
            .filter(|b| !KNOWN.iter().any(|k| b.starts_with(k)))
            .collect();
        assert!(
            bad.is_empty(),
            "furniture off its boxes:\n{}",
            bad.join("\n")
        );
    }

    /// Findings waiting on their owners (reported to Models: world), by
    /// the start of their line. Take one out when it's fixed.
    const KNOWN: &[&str] = &[];

    /// How far a furniture model's bottom may sit from its boxes' (m).
    const FLOOR_GAP: f32 = 0.03;
    /// How much bigger or smaller than its boxes it may be, per axis.
    const SIZE_RATIO: f32 = 2.0;
}
