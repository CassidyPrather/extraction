//! Headless scenarios: a bot harvests a stone from each kind of anomaly,
//! the way a person would (`extraction --scenario harvest_snare`, or in
//! `all`). Each stages its own anomaly on open ground near the insertion
//! point, does the kind's trick, and must come away holding the stone and
//! still standing.

use bevy::prelude::*;
use extraction_core::anomaly::{AnomalyKind, bloom_ok};
use extraction_core::map::MarkerKind;

use crate::bot::Task;
use crate::rig::Side;
use crate::weapons::flat::MacroKind;
use crate::world::Zone;

pub const NAMES: [&str; 6] = [
    "harvest_snare",
    "harvest_lift",
    "harvest_burner",
    "harvest_well",
    "harvest_mirror",
    "harvest_hush",
];

/// Where each scenario's anomaly goes, from the insertion point: west of
/// it, apart from each other, a short walk.
const SPOTS: [[f32; 2]; 6] = [
    [-22.0, -12.0],
    [-22.0, -36.0],
    [-48.0, -12.0],
    [-48.0, -38.0],
    [-35.0, -60.0],
    [-10.0, -60.0],
];

/// How far east of an anomaly's edge the bot may stand, metres: every
/// scenario's stand points lie within it, and it starts at [`ARRIVE`].
const APRON: f32 = 14.0;
/// Where the bot is put down to start, metres east of the anomaly's edge.
const ARRIVE: f32 = 12.0;

/// Open ground for the `i`th scenario's anomaly near its spot: off the
/// roads, clear of buildings and trees, not steep, and its apron (east of
/// it, where the bot stands and starts) walkable all the way in.
fn spot(zone: &Zone, i: usize, kind: AnomalyKind, radius: f32) -> Vec3 {
    let trees = extraction_core::forest::scatter_trees(&zone.map, &zone.hm);
    let nav = extraction_core::map_nav::zone_grid(
        &zone.map,
        &zone.hm,
        extraction_core::map_nav::MAX_SLOPE,
    );
    let m = zone
        .marker(&MarkerKind::Insertion)
        .map(|m| m.at)
        .unwrap_or([0.0, 0.0]);
    let base = Vec2::new(m[0] + SPOTS[i][0], m[1] + SPOTS[i][1]);
    let p = (0..300)
        .map(|k| {
            let a = k as f32 * 2.4;
            let d = 1.2 * (k as f32).sqrt();
            base + Vec2::new(a.cos(), a.sin()) * d
        })
        .find(|p| {
            bloom_ok(&zone.map, &zone.hm, p.to_array(), kind, radius + 3.0)
                && zone.hm.slope_deg(p.to_array()) < 10.0
                && trees
                    .iter()
                    .all(|t| Vec2::new(t.pos[0], t.pos[2]).distance(*p) > radius + 7.0)
                && (0..=APRON as i32).all(|k| nav.walkable([p.x + radius + k as f32, p.y]))
        })
        .unwrap_or(base);
    zone.on_ground(p.x, p.y)
}

pub fn tasks(name: &str, zone: &Zone) -> Option<Vec<Task>> {
    use Task::*;
    let s = |x: &str| x.to_string();
    let stand = |c: Vec3, d: f32| {
        // On the near (east) side, toward the insertion point.
        let p = Vec2::new(c.x + d, c.z);
        WalkTo { to: p, within: 0.6 }
    };
    let kit_at = |c: Vec3, d: f32| zone.on_ground(c.x + d, c.z + 0.4) + Vec3::Y * 0.1;
    let end = |id: &str| {
        vec![
            Expect(crate::bot::Expect::Holding(Side::Right, s(id))),
            Stow(Side::Right),
            Expect(crate::bot::Expect::Alive),
        ]
    };
    Some(match name {
        "harvest_snare" => {
            let r = 2.0;
            let c = spot(zone, 0, AnomalyKind::Snare, r);
            let mut t = vec![
                Log(s(
                    "harvest snare: ground it with a bolt, reach in while it recharges",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r + 2.0),
                Stage {
                    kind: AnomalyKind::Snare,
                    at: c,
                    radius: r,
                    artifact: s("art_aegis"),
                    kit: vec![(s("bolt"), kit_at(c, r + 2.0))],
                },
                Wait(1.0),
                PickUp {
                    id: s("bolt"),
                    side: Side::Right,
                },
                stand(c, r + 1.5),
                Throw {
                    side: Side::Right,
                    at: c + Vec3::Y * 0.8,
                },
                Wait(0.2),
                PickUp {
                    id: s("art_aegis"),
                    side: Side::Right,
                },
                stand(c, r + 3.0),
            ];
            t.extend(end("art_aegis"));
            t
        }
        "harvest_lift" => {
            let r = 3.0;
            let c = spot(zone, 1, AnomalyKind::Lift, r);
            let mut t = vec![
                Log(s(
                    "harvest lift: wind it up with a bolt, stand clear, catch the stone it throws",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r + 2.5),
                Stage {
                    kind: AnomalyKind::Lift,
                    at: c,
                    radius: r,
                    artifact: s("art_featherstone"),
                    kit: vec![(s("bolt"), kit_at(c, r + 2.5))],
                },
                Wait(3.0),
                PickUp {
                    id: s("bolt"),
                    side: Side::Right,
                },
                Throw {
                    side: Side::Right,
                    at: c + Vec3::Y * 1.2,
                },
                stand(c, r + 6.0),
                Wait(9.0),
                PickUp {
                    id: s("art_featherstone"),
                    side: Side::Right,
                },
                stand(c, r + 6.0),
            ];
            t.extend(end("art_featherstone"));
            t
        }
        "harvest_burner" => {
            let r = 1.8;
            let c = spot(zone, 2, AnomalyKind::Burner, r);
            let stone = c + Vec3::Y * 0.3;
            let mut t = vec![
                Log(s(
                    "harvest burner: shoot the stone out of the coals, let it cool, take it",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r + 4.0),
                Stage {
                    kind: AnomalyKind::Burner,
                    at: c,
                    radius: r,
                    artifact: s("art_hearth"),
                    kit: vec![
                        (s("pm_pistol"), kit_at(c, r + 4.0)),
                        (s("pm_mag"), kit_at(c, r + 4.6)),
                    ],
                },
                Wait(1.0),
                PickUp {
                    id: s("pm_pistol"),
                    side: Side::Right,
                },
                PickUp {
                    id: s("pm_mag"),
                    side: Side::Left,
                },
                Stow(Side::Left),
                Gun(MacroKind::Reload),
                stand(c, r + 4.0),
                Fire {
                    at: stone,
                    shots: 3,
                },
                Stow(Side::Right),
                // Twenty seconds out of the coals and it's cool.
                Wait(22.0),
                PickUp {
                    id: s("art_hearth"),
                    side: Side::Right,
                },
            ];
            t.extend(end("art_hearth"));
            t
        }
        "harvest_well" => {
            let r = 2.0;
            let c = spot(zone, 3, AnomalyKind::Well, r);
            let mut t = vec![
                Log(s(
                    "harvest well: dash in for the stone at its heart, and out",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r * 3.2),
                Stage {
                    kind: AnomalyKind::Well,
                    at: c,
                    radius: r,
                    artifact: s("art_lodestone"),
                    kit: Vec::new(),
                },
                Wait(2.0),
                PickUp {
                    id: s("art_lodestone"),
                    side: Side::Right,
                },
                stand(c, r * 3.5),
            ];
            t.extend(end("art_lodestone"));
            t
        }
        "harvest_mirror" => {
            let r = 1.5;
            let c = spot(zone, 4, AnomalyKind::Mirror, r);
            let mut t = vec![
                Log(s(
                    "harvest mirror: throw a bolt through the fold, reach in while it closes",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r + 2.0),
                Stage {
                    kind: AnomalyKind::Mirror,
                    at: c,
                    radius: r,
                    artifact: s("art_shard"),
                    kit: vec![(s("bolt"), kit_at(c, r + 2.0))],
                },
                Wait(2.0),
                PickUp {
                    id: s("bolt"),
                    side: Side::Right,
                },
                stand(c, r + 0.4),
                Throw {
                    side: Side::Right,
                    at: c + Vec3::Y * 0.9,
                },
                PickUp {
                    id: s("art_shard"),
                    side: Side::Right,
                },
                stand(c, r + 3.0),
            ];
            t.extend(end("art_shard"));
            t
        }
        "harvest_hush" => {
            let r = 3.0;
            let c = spot(zone, 5, AnomalyKind::Hush, r);
            let mut t = vec![
                Log(s(
                    "harvest hush: in for the stone and out before the silence presses",
                )),
                Teleport(c + Vec3::X * (r + ARRIVE)),
                stand(c, r + 2.0),
                Stage {
                    kind: AnomalyKind::Hush,
                    at: c,
                    radius: r,
                    artifact: s("art_hushstone"),
                    kit: Vec::new(),
                },
                Wait(1.0),
                PickUp {
                    id: s("art_hushstone"),
                    side: Side::Right,
                },
                stand(c, r + 2.0),
            ];
            t.extend(end("art_hushstone"));
            t
        }
        _ => return None,
    })
}
