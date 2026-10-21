//! `door_shot`: a bot hears nothing, sees nothing, and shoots a walker
//! through a shut plank door anyway. The round goes through the boards
//! (`firearm_terminal`, the door's [`Surfaces`](crate::world::surfaces))
//! and into the machine behind them.
//!
//! `extraction --headless --scenario door_shot`

use bevy::prelude::*;

use crate::bot::{Expect, Task};
use crate::rig::Side;
use crate::weapons::flat::MacroKind;
use crate::world::Zone;
use crate::world::buildings::{placement_transform, prefab};

pub const SCENARIO: &str = "door_shot";

/// A plank house's door nearest the insertion with a clear line through
/// it to the room behind (no furniture, nothing on a shelf in the way):
/// where to stand out front, the doorway's middle, and the way out.
pub fn front_door(zone: &Zone) -> Option<(Vec3, Vec3, Vec3)> {
    let start = zone
        .marker(&extraction_core::map::MarkerKind::Insertion)
        .map(|m| Vec2::from(m.at))
        .unwrap_or_default();
    let mut houses: Vec<_> = zone
        .map
        .placements
        .iter()
        .filter(|p| p.prefab == "house_plank")
        .collect();
    houses.sort_by(|a, b| {
        Vec2::from(a.at)
            .distance(start)
            .total_cmp(&Vec2::from(b.at).distance(start))
    });
    houses.into_iter().find_map(|p| {
        let geo = prefab(&p.prefab)?;
        let t = placement_transform(zone, p);
        geo.doors
            .iter()
            .filter(|d| d.mat == crate::render::materials::Mat::Planks)
            .find_map(|d| {
                let rot = Quat::from_rotation_y(d.yaw);
                let mid = d.hinge + rot * Vec3::new(d.width * 0.5, 1.4, 0.0);
                // Out: away from the footprint's centre.
                let n = rot * Vec3::Z;
                let n = if Vec2::new(mid.x, mid.z).dot(Vec2::new(n.x, n.z)) >= 0.0 {
                    n
                } else {
                    -n
                };
                // Building space: from 4 m out to 2.5 m in, at the aim's height.
                let (a, b) = (mid + n * 4.0, mid - n * INSIDE);
                let blocked = geo.pieces.iter().filter(|q| q.collide).any(|q| {
                    let half = match q.shape {
                        crate::world::buildings::Shape::Box(s) => s * 0.5,
                        _ => return false,
                    };
                    let inv = q.at.compute_affine().inverse();
                    segment_hits_box(inv.transform_point3(a), inv.transform_point3(b), half)
                }) || geo.loot.iter().any(|l| {
                    let ab = b - a;
                    let t = ((l.at - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
                    (a + ab * t).distance(l.at) < 0.6
                });
                if blocked {
                    return None;
                }
                let mid = t.transform_point(mid);
                let out = (t.rotation * n).with_y(0.0).normalize_or(Vec3::Z);
                Some((mid + out * 4.0, mid, out))
            })
    })
}

/// How far behind the door the walker stands.
const INSIDE: f32 = 2.5;

/// Whether segment a-b meets the box of half extents `half` at the origin.
fn segment_hits_box(a: Vec3, b: Vec3, half: Vec3) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..3 {
        if d[i].abs() < 1e-6 {
            if a[i].abs() > half[i] {
                return false;
            }
            continue;
        }
        let (u, v) = ((-half[i] - a[i]) / d[i], (half[i] - a[i]) / d[i]);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
        if t0 > t1 {
            return false;
        }
    }
    true
}

pub fn tasks(zone: &Zone) -> Vec<Task> {
    let s = |x: &str| x.to_string();
    let Some((stand, mid, out)) = front_door(zone) else {
        return vec![Task::Log(s("door shot: no plank house in this valley"))];
    };
    // The walker stands two and a half metres in, square behind the door.
    let inside = mid - out * INSIDE;
    let mut tasks = vec![
        Task::Log(s(
            "door shot: a walker behind a shut plank door, a Mosin round through the boards",
        )),
        Task::PickUp {
            id: s("mosin"),
            side: Side::Right,
        },
        Task::PickUp {
            id: s("ammo_762r"),
            side: Side::Left,
        },
        Task::Stow(Side::Left),
        Task::Gun(MacroKind::Reload),
        Task::Teleport(stand),
        Task::Wait(1.0),
        Task::Spawn {
            kind: crate::robots::RobotKind::Walker,
            at: inside,
        },
        Task::Wait(0.5),
    ];
    // One round, at the walker's body through the door's middle, before
    // it can hear anything and come to open the door: it has to go
    // through the boards to count.
    // Level through the door's middle: the top of its orb.
    let at = Vec3::new(inside.x, mid.y, inside.z);
    tasks.push(Task::Fire { at, shots: 1 });
    tasks.push(Task::Wait(0.3));
    tasks.push(Task::Expect(Expect::RobotHits(1)));
    tasks
}
