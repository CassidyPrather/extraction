//! Model correctness: what's stowed on the body against the kit it's
//! stowed in (M3 in `docs/playtests/2026-10-28/TRANSCRIPT.md`, parts
//! through parts, on the rig). A pistol in its holster, a magazine in a
//! chest pouch, a cell in a hip pocket, a gadget on the chest mount: each
//! seated as the game seats it (`interact::body::slot_layout`), against
//! the slot's piece of the rig as `render::kit` hangs it (the piece and
//! its open flap, shown while the slot is full), the harness, its chest
//! panel and the vest.
//!
//! A thing is held to two things: nothing of it through the kit deeper
//! than [`DEPTH`], and its middle in the slot's piece (it sits in its
//! pouch, not beside it).
//!
//! `cargo test --lib capture::stowed -- --nocapture` prints every finding.

use bevy::math::{Mat4, Vec3};
use extraction_core::inventory::{DEFAULT_SLOTS, SlotKind};
use std::path::PathBuf;

use super::glb;

/// How deep a vertex of the thing may lie inside the kit, or the kit's in
/// the thing (m): canvas gives, a little.
pub const DEPTH: f32 = 0.004;

/// How far below a bag's top its mouth is (m): its rim's bevel.
const MOUTH: f32 = 0.006;

const RIG: &str = "models/kit/rig.glb";
const RIG_OPEN: &str = "models/kit/rig_open.glb";
const VEST: &str = "models/kit/vest.glb";

fn piece(kind: SlotKind) -> Option<&'static str> {
    match kind {
        SlotKind::Holster => Some("holster"),
        SlotKind::Pouch => Some("pouch"),
        SlotKind::Pocket => Some("pocket"),
        SlotKind::Mount => Some("mount"),
        _ => None,
    }
}

/// A model's triangles by node, in the model's space.
fn nodes(path: &str) -> Vec<glb::Tri> {
    glb::triangles(&crate::asset_root_path().join(path)).unwrap_or_default()
}

/// A slot piece's triangles in its own slot's space (the node's place in
/// the model only lays the rig out for the art checks).
fn piece_tris(path: &str, node: &str) -> Vec<[Vec3; 3]> {
    let Some(m) = super::seated::node_transform(&crate::asset_root_path().join(path), node) else {
        return Vec::new();
    };
    let back = m.inverse();
    nodes(path)
        .into_iter()
        .filter(|t| t.node == node)
        .map(|t| t.p.map(|p| back.transform_point3(p)))
        .collect()
}

/// One thing in one slot: the slot, the thing, its model, its pose in the
/// torso's space and the slot's place.
pub struct Stowed {
    pub slot: SlotKind,
    pub index: usize,
    pub item: String,
    pub path: PathBuf,
    pub at: Mat4,
    pub place: Mat4,
}

/// Every distinct model that fits each slot with a piece, seated in it.
pub fn stowed() -> Vec<Stowed> {
    let catalog = crate::items::Catalog::load();
    let model = |def: &extraction_core::items::ItemDef| -> Option<PathBuf> {
        let folder = match def.category {
            extraction_core::items::Category::Magazine => "mags",
            _ if def.firearm.is_some() => "guns",
            _ => "items",
        };
        let p = crate::asset_root_path().join(format!("models/{folder}/{}.glb", def.id));
        p.exists().then_some(p)
    };
    let mut out = Vec::new();
    let mut done = std::collections::HashSet::new();
    for (index, slot) in DEFAULT_SLOTS.iter().enumerate() {
        if piece(*slot).is_none() || !done.insert(*slot) {
            continue;
        }
        let (pos, seat, _) = crate::interact::body::slot_layout(*slot, index);
        let place = Mat4::from_translation(pos);
        let kind = crate::interact::SocketKind::Body(*slot);
        let mut seen = std::collections::HashSet::new();
        for (_, def) in catalog.iter() {
            if !crate::interact::accepts_item(&kind, def.size, None, def.firearm.is_some(), None) {
                continue;
            }
            let Some(path) = model(def) else {
                continue;
            };
            if !seen.insert(std::fs::read(&path).map(|b| b.len()).unwrap_or(0)) {
                continue;
            }
            out.push(Stowed {
                slot: *slot,
                index,
                item: def.id.clone(),
                path,
                at: place * seat.to_matrix(),
                place,
            });
        }
    }
    out
}

/// What's wrong with one stowed thing, one line each.
pub fn check(s: &Stowed) -> Vec<String> {
    let name = piece(s.slot).unwrap();
    let to = |m: Mat4, tris: Vec<[Vec3; 3]>| -> Vec<[Vec3; 3]> {
        tris.into_iter()
            .map(|t| t.map(|p| m.transform_point3(p)))
            .collect()
    };
    let bag = to(s.place, piece_tris(RIG, name));
    let flap = to(s.place, piece_tris(RIG_OPEN, &format!("{name}_flap_open")));
    // The harness and the vest in the torso's own space.
    let worn = |path: &str, node: &str| -> Vec<[Vec3; 3]> {
        let m = super::seated::node_transform(&crate::asset_root_path().join(path), node)
            .unwrap_or(Mat4::IDENTITY);
        // Hung at the node's place for the vest, at identity for the rig.
        let keep = if path == VEST {
            Mat4::IDENTITY
        } else {
            m.inverse()
        };
        nodes(path)
            .into_iter()
            .filter(|t| t.node == node)
            .map(|t| t.p.map(|p| keep.transform_point3(p)))
            .collect()
    };
    let thing: Vec<[Vec3; 3]> = glb::triangles(&s.path)
        .unwrap_or_default()
        .into_iter()
        .map(|t| t.p.map(|p| s.at.transform_point3(p)))
        .collect();
    let mut out = Vec::new();
    let label = format!("{} in the {:?}", s.item, s.slot);
    if thing.is_empty() {
        return vec![format!("{label}: no model")];
    }
    let pts: Vec<Vec3> = thing.iter().flatten().copied().collect();
    // A pouch, a pocket and a holster are bags: what's in one is in its
    // solid, and may stand out of its mouth, not out of its walls or
    // bottom. The mount is an open loop: nothing goes through its ring.
    let bagged = s.slot != SlotKind::Mount;
    // Only what's out of the bag shows: what's in it is hidden in it.
    let shows = |p: &Vec3| !bagged || super::seated::winding(&bag, *p) < 0.5;
    let pts: Vec<Vec3> = pts.into_iter().filter(|p| shows(p)).collect();
    let mut solids = vec![
        (format!("{name}'s open flap"), flap),
        ("harness".to_string(), worn(RIG, "harness")),
        ("chest panel".into(), worn(RIG, "harness_panel")),
        ("vest".into(), worn(VEST, "vest")),
    ];
    if !bagged {
        solids.insert(0, (name.to_string(), bag.clone()));
    }
    for (what, solid) in solids {
        if solid.is_empty() {
            continue;
        }
        let kit_pts: Vec<Vec3> = solid
            .iter()
            .flatten()
            .copied()
            .filter(|p| shows(p))
            .collect();
        let (d1, _, a1) = super::seated::through(&solid, &pts);
        let (d2, _, a2) = super::seated::through(&thing, &kit_pts);
        let (d, at) = if d1 >= d2 { (d1, a1) } else { (d2, a2) };
        if d > DEPTH {
            out.push(format!(
                "{label}: through the {what} {:.0} mm at ({:.3}, {:.3}, {:.3})",
                d * 1000.0,
                at.x,
                at.y,
                at.z
            ));
        }
    }
    let bounds = |t: &[[Vec3; 3]]| {
        t.iter().flatten().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(l, h), p| (l.min(*p), h.max(*p)),
        )
    };
    let (bl, bh) = bounds(&bag);
    if bagged {
        // Out of its walls or its bottom, under the mouth.
        let mouth = bh.y - MOUTH;
        let (mut worst, mut at) = (0.0f32, Vec3::ZERO);
        for p in pts.iter().filter(|p| p.y < mouth) {
            let out_by = (bl - *p).max(*p - bh).max_element();
            if out_by > worst {
                worst = out_by;
                at = *p;
            }
        }
        if worst > DEPTH {
            out.push(format!(
                "{label}: out of the {name}'s side {:.0} mm at ({:.3}, {:.3}, {:.3})",
                worst * 1000.0,
                at.x,
                at.y,
                at.z
            ));
        }
    }
    // Its middle in its piece's outline.
    let (tl, th) = bounds(&thing);
    let mid = (tl + th) / 2.0;
    if !(mid.cmpge(bl).all() && mid.cmple(bh).all()) {
        out.push(format!(
            "{label}: its middle ({:.3}, {:.3}, {:.3}) is outside the {name} ({:.3}..{:.3}, {:.3}..{:.3}, {:.3}..{:.3})",
            mid.x, mid.y, mid.z, bl.x, bh.x, bl.y, bh.y, bl.z, bh.z
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything stowed on the body sits in its slot's piece of the kit,
    /// and through none of it.
    #[test]
    #[ignore = "body-slot seats are the slot's origin, not fitted: waiting on slot_fit (Hands, B1)"]
    fn stowed_things_sit_in_their_kit() {
        let all = stowed();
        assert!(all.len() > 10, "only {} stowed", all.len());
        let mut bad = Vec::new();
        for s in &all {
            for f in check(s) {
                println!("{f}");
                if !KNOWN.iter().any(|k| f.starts_with(k)) {
                    bad.push(f);
                }
            }
        }
        println!("{} stowed things", all.len());
        assert!(
            bad.is_empty(),
            "stowed things off their kit:\n{}",
            bad.join("\n")
        );
    }

    const KNOWN: &[&str] = &[];

    /// Each distinct body-slot item once: (id, model bounds).
    fn items() -> Vec<(String, Vec3, Vec3)> {
        let mut seen = std::collections::HashSet::new();
        stowed()
            .into_iter()
            .filter(|s| seen.insert(s.item.clone()))
            .map(|s| {
                let (lo, hi) = glb::read(&s.path).unwrap().bounds().unwrap();
                (s.item, lo, hi)
            })
            .collect()
    }

    /// What goes in a body slot has a catalog size its model's box agrees
    /// with, axis by axis (its box about its origin, the model on its floor) (slots fit things by it: `kit_cavities`).
    #[test]
    fn body_slot_things_are_their_catalog_size() {
        let catalog = crate::items::Catalog::load();
        let mut bad = Vec::new();
        for (id, lo, hi) in items() {
            let def = catalog.get(catalog.kind(&id).unwrap());
            // Its box is centred on its origin, the model resting on the
            // box's floor (`models::every_item_model_fits_its_box`): its
            // height may run from the model's to twice its depth under
            // the origin. A gun's or an attachment's is placed by its
            // mount, not its box.
            let (m, d) = (hi - lo, Vec3::from(def.size_m()));
            let tol = (d * 0.25).max(Vec3::splat(0.01));
            let mut off = (m - d).abs().cmpgt(tol);
            let boxed = def.firearm.is_none()
                && def.category != extraction_core::items::Category::Attachment;
            if boxed {
                let (short, tall) = (m.y - tol.y, 2.0 * -lo.y + tol.y);
                off = bevy::math::BVec3::new(off.x, d.y < short || d.y > tall, off.z);
            }
            if off.any() {
                bad.push(format!(
                    "{id}: model {:.3} {:.3} {:.3}, catalog {:.3} {:.3} {:.3}",
                    m.x, m.y, m.z, d.x, d.y, d.z
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "sizes off their models:\n{}",
            bad.join("\n")
        );
    }

    /// `kit_cavities::ORIGIN_OFFSET` is the models': every body-slot model
    /// more than 5 mm off centre is in it, by its offset, and nothing else.
    #[test]
    fn origin_offsets_are_the_models() {
        use crate::kit_cavities::ORIGIN_OFFSET;
        let mut bad = Vec::new();
        let all = items();
        for (id, lo, hi) in &all {
            let c = (*lo + *hi) / 2.0;
            let listed = ORIGIN_OFFSET.iter().find(|(k, _)| k == id);
            match listed {
                Some((_, v)) if v.distance(c) > 0.002 => {
                    bad.push(format!("{id}: listed {v:.3}, model {c:.3}"))
                }
                None if c.abs().max_element() > 0.005 => {
                    bad.push(format!("{id}: {c:.3} off centre, not listed"))
                }
                _ => {}
            }
        }
        for (k, _) in ORIGIN_OFFSET {
            if !all.iter().any(|(id, ..)| id == k) {
                bad.push(format!("{k}: listed, but goes in no body slot"));
            }
        }
        assert!(bad.is_empty(), "origin offsets:\n{}", bad.join("\n"));
    }

    /// The rig's slot pieces hold the cavities `kit_cavities` gives logic:
    /// each cavity's middle and its faces (2 mm in) are in its bag's solid
    /// (the loop's hole, for the mount), and its walls and floor are no
    /// thicker than [`WALL`].
    #[test]
    fn rig_holds_its_cavities() {
        use super::super::seated::winding;
        const WALL: f32 = 0.012;
        let mut bad = Vec::new();
        for slot in [
            SlotKind::Pouch,
            SlotKind::Pocket,
            SlotKind::Holster,
            SlotKind::Mount,
        ] {
            let cav = crate::kit_cavities::cavity(slot).unwrap();
            let name = piece(slot).unwrap();
            let tris = piece_tris(RIG, name);
            assert!(!tris.is_empty(), "no {name} in the rig");
            let c = cav.centre();
            let h = cav.half;
            // The faces round it: four walls and the floor (the mouth's open).
            let faces = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z, Vec3::NEG_Y];
            if slot == SlotKind::Mount {
                // An open loop: its hole is empty, its ring just past it.
                for f in &faces[..4] {
                    let p = c + *f * h;
                    let ring = c + *f * (h + Vec3::splat(0.0025));
                    if winding(&tris, c) > 0.5 || winding(&tris, p - *f * 0.002) > 0.5 {
                        bad.push(format!("{name}: its cavity's {f} side is in the loop"));
                    }
                    if winding(&tris, ring) < 0.5 {
                        bad.push(format!("{name}: no loop just past its cavity's {f} side"));
                    }
                }
                continue;
            }
            if winding(&tris, c) < 0.5 {
                bad.push(format!("{name}: its cavity's middle is out of it"));
            }
            for f in faces {
                let p = c + f * h;
                if winding(&tris, p - f * 0.002) < 0.5 {
                    bad.push(format!("{name}: its cavity's {f} face is out of it"));
                }
                if winding(&tris, p + f * WALL) > 0.5 {
                    bad.push(format!(
                        "{name}: its wall past the cavity's {f} face is over {WALL} m"
                    ));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "rig against kit_cavities:\n{}",
            bad.join("\n")
        );
    }
}
