//! The training yard's bays (#89, `docs/YARD_SHAPE.md`): bare timber
//! fences, waist high, that you can see over but not walk through, and a
//! gate in each that only the yard opens.
//!
//! - `yard_fence` (a 6 m run) and `yard_fence_2` (a 2 m filler): posts every
//!   2 m and three rails, all solid. Nothing in them breaks, so the range
//!   lane's fence stops rounds and stands (a garden fence's panels are
//!   breakables, `buildings::fence_garden`).
//! - `yard_gate`: two posts 4 m apart (with the rails out to the fence
//!   either side) and a 3 m leaf hung on the west post, opening north. The
//!   leaf is an ordinary [`Door`] (its own collider, swung by
//!   `buildings::swing_doors`); tagged `gate_N` in the map, it carries
//!   [`YardGate`] `{ index: N }`. Set its `Door::open` to open or shut it.
//!   No hand opens one: a press, a grip and a palm all leave it be.
//! - `yard_hoarding` (6 m) and `yard_hoarding_2` (2 m): a bay's gate line
//!   boarded up [`HOARDING`] high, above a standing eye, so from inside a
//!   bay you see that bay and, through its gate, the next; not every lamp
//!   and board down the field (the playtest of 22 November). The same runs
//!   as the fences, posts every 2 m, a solid board face; nothing breaks.
//! - `yard_store`: a squat concrete store, 4 m by 3 m, its flat roof
//!   [`STORE_ROOF`] up: low enough for a lift's pop to land you on it
//!   (#108), its rim kerbed pale so the roof reads as somewhere to land.

use bevy::prelude::*;

use super::Zone;
use super::buildings::{Builder, Building, Door, DoorDef, DoorLeaf, Geo};
use crate::render::materials::Mat;

/// The fences' height: waist high (the plan's 1.3 m), the next bay seen
/// over it.
pub const HEIGHT: f32 = 1.3;
/// Posts this far apart along a run.
pub const BAY: f32 = 2.0;
/// The gate's leaf: 3 m across, hung between posts [`GATE_POSTS`] apart.
pub const LEAF: f32 = 3.0;
pub const GATE_POSTS: f32 = 3.2;

const POST: f32 = 0.12;
const RAIL: (f32, f32) = (0.1, 0.05);
/// Rail heights (their middles): knee, hip, top.
const RAILS: [f32; 3] = [0.35, 0.8, HEIGHT - 0.06];

/// A yard gate's leaf, by its tag's number (`gate_1` is the first ahead of
/// the start).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct YardGate {
    pub index: u8,
}

pub struct YardPlugin;

impl Plugin for YardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tag_gates);
    }
}

/// A yard prefab's geometry, building-local, the run along x.
pub fn geo(id: &str) -> Option<Geo> {
    let mut b = Builder::default();
    match id {
        "yard_fence" => run(&mut b, -3.0, 3.0),
        "yard_fence_2" => run(&mut b, -1.0, 1.0),
        "yard_hoarding" => hoarding(&mut b, -3.0, 3.0),
        "yard_hoarding_2" => hoarding(&mut b, -1.0, 1.0),
        "yard_gate" => gate(&mut b),
        "yard_store" => store(&mut b),
        _ => return None,
    }
    Some(b.finish())
}

/// The store's flat roof, its top over the ground (m): under the 2.2 m a
/// lift's pop clears (2.8 m up at the top of its arc).
pub const STORE_ROOF: f32 = 2.0;
/// The store's walls (along x, along z), and the roof's overhang.
const STORE: (f32, f32) = (4.0, 3.0);
const EAVE: f32 = 0.1;

/// The store: a solid block, a roof slab over it with a pale kerb round
/// its rim (paint for the eye, no lip to trip on), a steel door.
fn store(b: &mut Builder) {
    let (sx, sz) = STORE;
    let slab = 0.15;
    let wall = STORE_ROOF - slab;
    b.bx(
        Vec3::new(sx, wall, sz),
        Vec3::new(0.0, wall * 0.5, 0.0),
        Mat::Concrete,
    );
    let (rx, rz) = (sx + 2.0 * EAVE, sz + 2.0 * EAVE);
    b.bx(
        Vec3::new(rx, slab, rz),
        Vec3::new(0.0, STORE_ROOF - slab * 0.5, 0.0),
        Mat::Concrete,
    );
    let (k, kh) = (0.18, 0.03);
    let y = STORE_ROOF + kh * 0.5;
    for s in [-1.0f32, 1.0] {
        b.deco(
            Vec3::new(rx, kh, k),
            Vec3::new(0.0, y, s * (rz - k) * 0.5),
            Mat::Plaster,
        );
        b.deco(
            Vec3::new(k, kh, rz - 2.0 * k),
            Vec3::new(s * (rx - k) * 0.5, y, 0.0),
            Mat::Plaster,
        );
    }
    // The door, shut, on the north face (-z).
    b.deco(
        Vec3::new(0.9, 1.7, 0.05),
        Vec3::new(0.8, 0.85, -sz * 0.5 - 0.02),
        Mat::PaintedMetal,
    );
}

/// Posts every [`BAY`] from `x0` to `x1`, rails between.
fn run(b: &mut Builder, x0: f32, x1: f32) {
    let n = ((x1 - x0) / BAY).round() as usize;
    for k in 0..=n {
        // Runs are laid end to end, each with a post at both ends, or
        // butt onto another's post: its end posts are inset all round,
        // its first by 5 mm (inside a post it meets), its last by 10 mm
        // (inside the next run's first). (Two the same, in the same
        // place, fought: the 22 November playtest, 8:43.)
        post(b, x0 + k as f32 * BAY, end_inset(k, n));
    }
    rails(b, x0, x1);
}

/// How far a run's `k`th of `n + 1` posts is inset all round (m).
pub(crate) fn end_inset(k: usize, n: usize) -> f32 {
    if k == n {
        0.01
    } else if k == 0 {
        0.005
    } else {
        0.0
    }
}

fn post(b: &mut Builder, x: f32, inset: f32) {
    b.bx(
        Vec3::new(
            POST - 2.0 * inset,
            HEIGHT + 0.05 - inset,
            POST - 2.0 * inset,
        ),
        Vec3::new(x, (HEIGHT + 0.05 - inset) * 0.5, 0.0),
        Mat::Timber,
    );
}

fn rails(b: &mut Builder, x0: f32, x1: f32) {
    for y in RAILS {
        b.bx(
            Vec3::new(x1 - x0, RAIL.0, RAIL.1),
            Vec3::new((x0 + x1) * 0.5, y, 0.0),
            Mat::Timber,
        );
    }
}

/// The hoarding's height: over a tall player's eye (1.85 m), and over
/// every station's props, so a bay's ground is hidden from the next; the
/// lamps (`onboarding::LAMP_Y`, 2.32 m) show over it only from near.
pub const HOARDING: f32 = 2.1;

/// Posts every [`BAY`] from `x0` to `x1`, a face of boards on their north
/// side from a hand above the ground to the top, a rail along the top and
/// another at the middle on the south side.
fn hoarding(b: &mut Builder, x0: f32, x1: f32) {
    let n = ((x1 - x0) / BAY).round() as usize;
    let h = HOARDING;
    for k in 0..=n {
        b.bx(
            Vec3::new(POST, h + 0.05, POST),
            Vec3::new(x0 + k as f32 * BAY, (h + 0.05) * 0.5, 0.0),
            Mat::Timber,
        );
    }
    // The boards, standing on the ground (it can't be crawled under), a
    // hair short of the posts' tops.
    let face = 0.04;
    b.bx(
        Vec3::new(x1 - x0 + POST, h - 0.02, face),
        Vec3::new((x0 + x1) * 0.5, (h - 0.02) * 0.5, -(POST + face) * 0.5),
        Mat::Planks,
    );
    for y in [h * 0.5, h - 0.08] {
        b.bx(
            Vec3::new(x1 - x0, RAIL.0, RAIL.1),
            Vec3::new((x0 + x1) * 0.5, y, (POST + RAIL.1) * 0.5),
            Mat::Timber,
        );
    }
}

/// How high a lamp hangs on a gate's flanking post (m): under the
/// hoarding's top, above a standing eye.
pub const GATE_POST_LAMP_Y: f32 = 1.9;

/// The posts either side of a yard gate where its rails meet the gate
/// line's hoarding, world (x, z): 2 m each way along the line from the
/// gate's middle, the one to its local -x (west, unturned) first. A lamp
/// on one hangs [`GATE_POST_LAMP_Y`] up, on whichever face looks into the
/// bay it lights (`onboarding`'s trail).
pub fn gate_post_anchors(at: Vec2, rot_deg: f32) -> [Vec2; 2] {
    let r = rot_deg.to_radians();
    let along = Vec2::new(r.cos(), -r.sin());
    // Half the gate's 4 m gap in the line (its footprint).
    let h = 2.0;
    [at - along * h, at + along * h]
}

/// The gate's posts, the rails out from them to the fence either side
/// (the gate fills a 4 m gap in the line), and the leaf.
fn gate(b: &mut Builder) {
    let h = GATE_POSTS * 0.5;
    for s in [-1.0f32, 1.0] {
        // A stouter post: the leaf hangs on it.
        b.bx(
            Vec3::new(0.16, HEIGHT + 0.2, 0.16),
            Vec3::new(s * h, (HEIGHT + 0.2) * 0.5, 0.0),
            Mat::Timber,
        );
        let (a, z) = if s < 0.0 { (-2.0, -h) } else { (h, 2.0) };
        rails(b, a, z);
    }
    b.geo.doors.push(DoorDef {
        hinge: Vec3::new(-LEAF * 0.5, 0.06, 0.0),
        width: LEAF,
        height: HEIGHT - 0.1,
        yaw: 0.0,
        mat: Mat::Planks,
    });
}

/// The `n` of a yard gate's tag (`gate_n`), if it has one.
pub fn gate_index(tag: &str) -> Option<u8> {
    tag.strip_prefix("gate_")?.parse().ok()
}

/// Each yard gate's leaf, once spawned: its number, and no press or grip
/// on it (`hinge`, `swing_doors`); `physics_hands::push_doors` leaves it be
/// too.
fn tag_gates(
    mut commands: Commands,
    zone: Res<Zone>,
    leaves: Query<(Entity, &ChildOf), (Added<DoorLeaf>, With<Door>)>,
    buildings: Query<(&Building, &Transform)>,
) {
    for (leaf, parent) in &leaves {
        let Ok((b, t)) = buildings.get(parent.parent()) else {
            continue;
        };
        if b.prefab != "yard_gate" {
            continue;
        }
        let at = t.translation.xz();
        let index = zone
            .map
            .placements
            .iter()
            .filter(|p| p.prefab == "yard_gate")
            .min_by(|a, c| {
                Vec2::from_array(a.at)
                    .distance(at)
                    .total_cmp(&Vec2::from_array(c.at).distance(at))
            })
            .and_then(|p| gate_index(&p.tag))
            .unwrap_or(0);
        commands
            .entity(leaf)
            .insert(YardGate { index })
            .remove::<(crate::raid::truck::Pressable, crate::interact::Grabbable)>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No two placements on a map draw the same box in the same place:
    /// two alike, coplanar all round, fight (a fence's runs laid end to end,
    /// each with a post at both ends: the 22 November playtest, 8:43).
    #[test]
    fn no_two_placements_draw_the_same_box() {
        use crate::world::buildings::{Shape, prefab};
        let maps = [
            "training_yard",
            "arcade",
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "rime_fell",
            "sallow_reservoir",
        ];
        let mut bad = Vec::new();
        for name in maps {
            let map = crate::world::load_map_def(name);
            let hm = extraction_core::terrain::Heightmap::build(&map);
            let mut seen: std::collections::HashMap<[i64; 6], usize> = Default::default();
            for (i, p) in map.placements.iter().enumerate() {
                let Some(geo) = prefab(&p.prefab) else {
                    continue;
                };
                let y = if p.absolute {
                    p.y
                } else {
                    hm.footing(&p.prefab, p.at, p.rot) + p.y
                };
                let t = Transform::from_xyz(p.at[0], y, p.at[1])
                    .with_rotation(Quat::from_rotation_y(-p.rot.to_radians()));
                for piece in geo.pieces.iter() {
                    let Shape::Box(size) = piece.shape else {
                        continue;
                    };
                    let c = t.transform_point(piece.at.translation);
                    let e = ((t.rotation * piece.at.rotation) * size).abs();
                    let q = |v: f32| (v * 200.0).round() as i64;
                    let key = [q(c.x), q(c.y), q(c.z), q(e.x), q(e.y), q(e.z)];
                    if let Some(&j) = seen.get(&key) {
                        if j != i {
                            bad.push(format!(
                                "{name}: {} and {} both draw {size:?} at {c:?}",
                                map.placements[j].prefab, p.prefab
                            ));
                        }
                    } else {
                        seen.insert(key, i);
                    }
                }
            }
        }
        bad.dedup();
        assert!(bad.is_empty(), "{} alike:\n{}", bad.len(), bad.join("\n"));
    }

    /// A run is solid all along and through its height: its rails are
    /// colliders, and nothing in it breaks.
    #[test]
    fn a_yard_fence_is_solid_and_breaks_nowhere() {
        for (id, len) in [("yard_fence", 6.12), ("yard_fence_2", 2.12)] {
            let g = geo(id).unwrap();
            assert!(g.breakables.is_empty(), "{id}: breakables");
            assert!(
                g.pieces.iter().all(|p| p.collide),
                "{id}: a piece that isn't solid"
            );
            let f = extraction_core::map::prefab_footprint(id);
            assert_eq!(f[0], len, "{id}: footprint length");
            assert!(extraction_core::map::is_fence(id));
        }
    }

    /// A hoarding is solid, breaks nowhere, and stands over a tall
    /// player's eye along its whole run.
    #[test]
    fn a_yard_hoarding_hides_the_next_bay() {
        for (id, len) in [("yard_hoarding", 6.12), ("yard_hoarding_2", 2.12)] {
            let g = geo(id).unwrap();
            assert!(g.breakables.is_empty(), "{id}: breakables");
            assert!(
                g.pieces.iter().all(|p| p.collide),
                "{id}: a piece that isn't solid"
            );
            let f = extraction_core::map::prefab_footprint(id);
            assert_eq!(f[0], len, "{id}: footprint length");
            assert!(f[1] >= HOARDING && HOARDING > 1.9, "{id}: {f:?}");
            assert!(extraction_core::map::is_fence(id));
        }
    }

    /// The gate fills its 4 m in the line: rails out to ±2 m, a 3 m leaf
    /// between posts, hung low enough that nothing walks under it.
    #[test]
    fn a_yard_gate_fills_its_gap_with_a_leaf() {
        let g = geo("yard_gate").unwrap();
        assert_eq!(g.doors.len(), 1);
        let d = g.doors[0];
        assert_eq!(d.width, LEAF);
        assert!(d.hinge.y < 0.1 && d.height > 1.1);
        // The leaf spans the posts' gap less their own width.
        assert!((d.hinge.x + LEAF * 0.5).abs() < 1e-3);
        assert!(LEAF <= GATE_POSTS - 0.16 + 1e-3);
        assert_eq!(extraction_core::map::prefab_footprint("yard_gate")[0], 4.0);
        assert_eq!(gate_index("gate_3"), Some(3));
        assert_eq!(gate_index("wait_north_post"), None);
    }

    /// Every yard gate has a hoarding's post at both its anchors: the
    /// gate line is boarded right up to its rails.
    #[test]
    fn each_yard_gate_has_a_hoarding_post_either_side() {
        let map = extraction_core::map::MapDef::from_ron(extraction_core::map::TRAINING_YARD)
            .unwrap();
        let ends: Vec<Vec2> = map
            .placements
            .iter()
            .filter(|p| p.prefab.starts_with("yard_hoarding"))
            .flat_map(|p| {
                let half = extraction_core::map::prefab_footprint(&p.prefab)[0] * 0.5 - 0.06;
                let r = p.rot.to_radians();
                let along = Vec2::new(r.cos(), -r.sin());
                let at = Vec2::new(p.at[0], p.at[1]);
                [at - along * half, at + along * half]
            })
            .collect();
        let gates: Vec<_> = map.placements.iter().filter(|p| p.prefab == "yard_gate").collect();
        assert!(gates.len() >= 6);
        for g in gates {
            for a in gate_post_anchors(Vec2::new(g.at[0], g.at[1]), g.rot) {
                assert!(
                    ends.iter().any(|e| e.distance(a) < 0.05),
                    "{:?}: no hoarding post at {a}",
                    g.tag
                );
            }
        }
    }

    /// The store's roof is where the core's footprint says, low enough for
    /// a lift's pop, and solid to land on; nothing on it to trip over.
    #[test]
    fn the_store_roof_is_low_flat_and_solid() {
        let g = geo("yard_store").unwrap();
        let f = extraction_core::map::prefab_footprint("yard_store");
        assert!((f[1] - STORE_ROOF).abs() < 0.05, "{f:?}");
        assert!(STORE_ROOF <= 2.2);
        let top = g
            .pieces
            .iter()
            .filter(|p| p.collide)
            .filter_map(|p| match p.shape {
                super::super::buildings::Shape::Box(s) => Some(p.at.translation.y + s.y * 0.5),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        assert!((top - STORE_ROOF).abs() < 0.01, "solid top {top}");
        assert!(f[0] >= STORE.0 && f[2] >= STORE.1);
        assert!(g.loot.is_empty());
    }
}
