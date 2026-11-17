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
        "yard_gate" => gate(&mut b),
        _ => return None,
    }
    Some(b.finish())
}

/// Posts every [`BAY`] from `x0` to `x1`, rails between.
fn run(b: &mut Builder, x0: f32, x1: f32) {
    let n = ((x1 - x0) / BAY).round() as usize;
    for k in 0..=n {
        post(b, x0 + k as f32 * BAY);
    }
    rails(b, x0, x1);
}

fn post(b: &mut Builder, x: f32) {
    b.bx(
        Vec3::new(POST, HEIGHT + 0.05, POST),
        Vec3::new(x, (HEIGHT + 0.05) * 0.5, 0.0),
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
}
