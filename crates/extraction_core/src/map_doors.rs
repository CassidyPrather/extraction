//! Door clearance: the ground in front of every outside door, as built.
//!
//! Each prefab's doors (`map::prefab_entrances`) carry their steps and the
//! room to stand past them (`clear`). Placed on a map, the strip in front
//! of a door ([`Door::strip`]: the opening's width and a shoulder either
//! side, from the wall's face [`DOOR_REACH`] or `clear` out, whichever is
//! further) must be free of other buildings and kiosks, trunks, solid
//! rocks, supply crates and other solid markers, and of water; the ground
//! past the steps must meet them within a step ([`DOOR_STEP`]) and be no
//! steeper than is comfortable; and the walkers' grid must be open there
//! and reached from the insertion. `map_lint` reports what this finds as
//! `Rule::Door`; the forest and rock scatter keep off the strips
//! ([`clear_of_doors`]).
//!
//! A door is placed as its building is (`Heightmap::footing`, the game's
//! `Quat::from_rotation_y(-rot)`, the lint's `Rect`): prefab-space
//! `(x, z)` goes to `at + (x cos r - z sin r, x sin r + z cos r)`.

use crate::geom::{self, P2, dist};
use crate::map::{MapDef, MarkerKind, prefab_entrances, prefab_footprint, prefab_lift};
use crate::map_lint::{COMFORT_SLOPE, DOOR_REACH, DOOR_SHOULDER, DOOR_STEP, WALKER_STEP};
use crate::nav::{BLOCKED, NavGrid};
use crate::terrain::Heightmap;

/// Prefabs whose doorway is a climb on purpose: a goods wagon's floor is
/// 1.2 m up on its bogies, its door over the rail bed's shoulder. Their
/// strip is still kept clear; their ground isn't held to a step or a
/// slope.
const CLIMB_IN: &[&str] = &["boxcar"];

/// An outside door of a placement, in map space.
#[derive(Clone, Debug)]
pub struct Door {
    /// Index of the placement in `map.placements`.
    pub placement: usize,
    /// The middle of the opening on the wall's outer face.
    pub face: P2,
    /// The way out, unit.
    pub out: P2,
    pub width: f32,
    /// Its steps, then a metre to stand in, from the face.
    pub clear: f32,
}

impl Door {
    /// How far out from the face the strip runs.
    pub fn reach(&self) -> f32 {
        self.clear.max(DOOR_REACH)
    }

    /// Where the steps end and the ground begins, from the face.
    pub fn foot(&self) -> f32 {
        (self.clear - 1.0).max(0.0)
    }

    /// Half the strip's width: the opening and a shoulder either side.
    pub fn half_width(&self) -> f32 {
        self.width * 0.5 + DOOR_SHOULDER
    }

    /// The point `s` out from the face and `t` across (to the door's right
    /// looking out).
    pub fn point(&self, s: f32, t: f32) -> P2 {
        let side = [-self.out[1], self.out[0]];
        geom::add(
            self.face,
            geom::add(geom::scale(self.out, s), geom::scale(side, t)),
        )
    }

    /// Where you stand in front of it, past its steps.
    pub fn stand(&self) -> P2 {
        self.point(self.reach(), 0.0)
    }

    /// Signed distance from `p` to the strip (negative inside).
    pub fn strip(&self, p: P2) -> f32 {
        let d = geom::sub(p, self.face);
        let side = [-self.out[1], self.out[0]];
        let (s, t) = (geom::dot(d, self.out), geom::dot(d, side));
        let half_len = self.reach() * 0.5;
        let q = [(s - half_len).abs() - half_len, t.abs() - self.half_width()];
        geom::len([q[0].max(0.0), q[1].max(0.0)]) + q[0].max(q[1]).min(0.0)
    }

    /// Points over the strip every `step` metres, from `s0` out, `half`
    /// across either side.
    fn samples(&self, s0: f32, half: f32, step: f32) -> Vec<P2> {
        let (ns, nt) = (
            ((self.reach() - s0) / step).ceil().max(1.0) as usize,
            (2.0 * half / step).ceil().max(1.0) as usize,
        );
        let mut out = Vec::new();
        for i in 0..=ns {
            for j in 0..=nt {
                let s = s0 + (self.reach() - s0) * i as f32 / ns as f32;
                let t = -half + 2.0 * half * j as f32 / nt as f32;
                out.push(self.point(s, t));
            }
        }
        out
    }
}

/// Every outside door of every placement on the map's ground (not of
/// `absolute` ones, which stand on whatever was built for them).
pub fn doors(map: &MapDef) -> Vec<Door> {
    let mut out = Vec::new();
    for (i, p) in map.placements.iter().enumerate() {
        if p.absolute {
            continue;
        }
        let (s, c) = p.rot.to_radians().sin_cos();
        let turn = |v: [f32; 2]| [v[0] * c - v[1] * s, v[0] * s + v[1] * c];
        for e in prefab_entrances(&p.prefab) {
            out.push(Door {
                placement: i,
                face: geom::add(p.at, turn(e.at)),
                out: turn(e.out),
                width: e.width,
                clear: e.clear,
            });
        }
    }
    out
}

/// Whether `p` is within `r` of a door: of its strip.
pub fn near_door(doors: &[Door], p: P2, r: f32) -> bool {
    doors.iter().any(|d| d.strip(p) < r)
}

/// Whether a round thing of radius `r` at `p` keeps off every door strip:
/// the forest and rock scatter keep these (after the scatter, so the rest
/// of the map's trees and rocks fall as they did).
pub fn clear_of_doors(doors: &[Door], p: P2, r: f32) -> bool {
    !doors.iter().any(|d| d.strip(p) < r)
}

/// What the door check found and measured.
#[derive(Clone, Debug, Default)]
pub struct DoorCheck {
    pub findings: Vec<(String, P2)>,
    /// Doors checked.
    pub doors: usize,
    /// The worst step between a door's steps and the ground in front of
    /// it, over the doors held to a step.
    pub worst_step: f32,
    /// Doors a walker (which steps [`WALKER_STEP`]) couldn't step to.
    pub walker_blocked: usize,
}

/// A rectangle in map space (as `map_lint`'s, local here).
fn inside_rect(at: P2, half: P2, rot_deg: f32, p: P2) -> bool {
    let d = geom::sub(p, at);
    let (s, c) = rot_deg.to_radians().sin_cos();
    let l = [d[0] * c + d[1] * s, -d[0] * s + d[1] * c];
    l[0].abs() < half[0] && l[1].abs() < half[1]
}

/// Checks every door on the map (see the module docs). `walk` is the
/// walkers' grid and `seen` the cells flooded from the insertion on it.
pub fn check(map: &MapDef, hm: &Heightmap, walk: &NavGrid, seen: &[bool]) -> DoorCheck {
    let all = doors(map);
    let trees = crate::forest::scatter_trees(map, hm);
    let rocks: Vec<_> = crate::rocks::scatter_rocks(map, hm)
        .into_iter()
        .filter(|r| r.radius >= crate::rocks::SOLID_RADIUS)
        .collect();
    let kiosks = crate::underground::kiosks(map);
    // Solid things markers make, as (what, centre, radius).
    let mut solids: Vec<(String, P2, f32)> = Vec::new();
    for m in &map.markers {
        let r = match &m.kind {
            MarkerKind::Loot { .. } if m.y < 0.3 => 0.55,
            MarkerKind::Offering => 0.75,
            MarkerKind::Sign { .. } => 0.1,
            MarkerKind::Terminal { .. } if m.y < 0.3 => 0.3,
            MarkerKind::Anomaly { radius, .. } => *radius,
            _ => continue,
        };
        let what = match &m.kind {
            MarkerKind::Loot { .. } => format!("{}'s supply crate", m.name),
            MarkerKind::Anomaly { kind, .. } => format!("a standing {kind}"),
            _ if m.name.is_empty() => "a marker".into(),
            _ => m.name.clone(),
        };
        solids.push((what, m.at, r));
    }
    let mut out = DoorCheck {
        doors: all.len(),
        ..Default::default()
    };
    let (mut worst_step, mut walker_blocked) = (0.0f32, 0);
    for d in &all {
        let p = &map.placements[d.placement];
        let name = format!("the {} door at ({:.0}, {:.0})", p.prefab, p.at[0], p.at[1]);
        let mut found: Vec<(String, P2)> = Vec::new();
        let mut find = |what: String, at: P2| found.push((format!("{name}: {what}"), at));
        // Obstacles in the strip.
        for (k, o) in map.placements.iter().enumerate() {
            if k == d.placement
                || o.prefab == "footbridge"
                || crate::underground::is_site(&o.prefab)
            {
                continue;
            }
            let f = prefab_footprint(&o.prefab);
            let half = [f[0] * 0.5, f[2] * 0.5];
            if dist(o.at, d.face) > geom::len(half) + d.reach() + d.half_width() {
                continue;
            }
            let hit = d
                .samples(0.0, d.half_width(), 0.25)
                .into_iter()
                .find(|q| inside_rect(o.at, half, o.rot, *q));
            if let Some(q) = hit {
                find(format!("the {} stands in front of it", o.prefab), q);
            }
        }
        for (at, rot) in &kiosks {
            let hit = d
                .samples(0.0, d.half_width(), 0.25)
                .into_iter()
                .find(|q| inside_rect(*at, crate::underground::SHELL_HALF, *rot, *q));
            if let Some(q) = hit {
                find("a site's kiosk stands in front of it".into(), q);
            }
        }
        if let Some(t) = trees
            .iter()
            .find(|t| d.strip([t.pos[0], t.pos[2]]) < 0.3 * t.scale)
        {
            find("a tree in the doorway".into(), [t.pos[0], t.pos[2]]);
        }
        if let Some(r) = rocks
            .iter()
            .find(|r| d.strip([r.pos[0], r.pos[2]]) < r.radius * 0.9)
        {
            find(
                format!("a {:.1} m rock in the doorway", r.radius),
                [r.pos[0], r.pos[2]],
            );
        }
        for (what, at, r) in &solids {
            if d.strip(*at) < *r {
                find(format!("{what} in the doorway"), *at);
            }
        }
        // Water anywhere in it.
        if let Some(q) = d
            .samples(0.0, d.half_width(), 0.5)
            .into_iter()
            .find(|q| hm.water_level(*q).is_some())
        {
            find("water in the doorway".into(), q);
        }
        // The ground past the steps meets them: the steps reach from the
        // floor down to the building's footing less its lift (the lowest
        // the ground under it may be).
        let base = hm.footing(&p.prefab, p.at, p.rot) + p.y;
        let low = base - prefab_lift(&p.prefab);
        let ground = d.samples(d.foot(), d.width * 0.5, 0.5);
        let climb = CLIMB_IN.contains(&p.prefab.as_str());
        if !climb {
            let (mut worst, mut at) = (0.0f32, d.stand());
            for q in &ground {
                let h = hm.height(*q);
                let step = (h - base).max(low - h);
                if step > worst {
                    (worst, at) = (step, *q);
                }
            }
            worst_step = worst_step.max(worst);
            walker_blocked += usize::from(worst > WALKER_STEP);
            if worst > DOOR_STEP {
                let h = hm.height(at);
                let how = if h > base { "rises" } else { "drops" };
                find(format!("the ground {how} {worst:.2} m from its steps"), at);
            }
        }
        if let Some(q) = ground
            .iter()
            .find(|q| !climb && hm.slope_deg(**q) > COMFORT_SLOPE)
        {
            find(
                format!("a {:.0}° slope in front of it", hm.slope_deg(*q)),
                *q,
            );
        }
        // The walkers' grid is open in front of it, and reached: on the way
        // out, as far as it takes to clear the building's own blocked
        // footprint (a chapel's steps lie inside its footprint, so its
        // strip can be blocked cells all through, more so turned off the
        // grid's axes).
        let f = prefab_footprint(&p.prefab);
        let margin = 0.6 + walk.cell;
        let half = [f[0] * 0.5 + margin, f[2] * 0.5 + margin];
        let mut out_to = d.reach();
        while out_to < d.reach() + 30.0 && inside_rect(p.at, half, p.rot, d.point(out_to, 0.0)) {
            out_to += 0.5;
        }
        let side = [-d.out[1], d.out[0]];
        let k = (out_to / walk.cell).ceil() as i64 + 1;
        let reached = walk.cell_of(d.face).is_some_and(|(ci, cj)| {
            let n = walk.n as i64;
            (-k..=k).any(|dj| {
                (-k..=k).any(|di| {
                    let (i, j) = (ci as i64 + di, cj as i64 + dj);
                    if i < 0 || j < 0 || i >= n || j >= n {
                        return false;
                    }
                    let c = j as usize * walk.n + i as usize;
                    let q = geom::sub(walk.centre(i as usize, j as usize), d.face);
                    let (s, t) = (geom::dot(q, d.out), geom::dot(q, side));
                    walk.cost[c] != BLOCKED
                        && seen[c]
                        && (-walk.cell..=out_to + walk.cell).contains(&s)
                        && t.abs() < d.half_width() + walk.cell
                })
            })
        });
        if !reached {
            find("no way up to it on the walkers' grid".into(), d.stand());
        }
        out.findings.extend(found);
    }
    out.worst_step = worst_step;
    out.walker_blocked = walker_blocked;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Marker, Placement, TRAINING_YARD};
    use crate::map_lint::{Rule, lint};

    /// The Training Yard and its practice house's door.
    fn yard() -> (MapDef, usize, Door) {
        let map = MapDef::from_ron(TRAINING_YARD).unwrap();
        let i = map
            .placements
            .iter()
            .position(|p| p.prefab == "house_plank")
            .unwrap();
        let d = doors(&map).into_iter().find(|d| d.placement == i).unwrap();
        (map, i, d)
    }

    fn door_findings(map: &MapDef) -> Vec<String> {
        let hm = Heightmap::build(map);
        lint(map, &hm)
            .findings
            .into_iter()
            .filter(|f| f.rule == Rule::Door)
            .map(|f| f.what)
            .collect()
    }

    /// A chapel's steps lie inside its long footprint: turned off the
    /// grid's axes, its door is still found reachable (the check looks out
    /// past the building's own blocked cells, not just along the strip).
    #[test]
    fn a_turned_chapel_can_be_walked_up_to() {
        for rot in [0.0, 75.0, 165.0, 255.0, 345.0] {
            let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
            map.placements.push(Placement {
                prefab: "chapel".into(),
                at: [-45.0, 30.0],
                rot,
                y: 0.0,
                absolute: false,
                tag: String::new(),
            });
            let found = door_findings(&map);
            assert!(
                !found.iter().any(|f| f.contains("walkers' grid")),
                "rot {rot}: {found:?}"
            );
        }
    }

    /// A door turns with its building as the game turns it
    /// (`Quat::from_rotation_y(-rot)`): a shed turned a quarter has its
    /// door, at (-1, -1.55) facing -Z unturned, on its +X side facing +X.
    #[test]
    fn doors_turn_with_their_building() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        map.placements = vec![Placement {
            prefab: "shed".into(),
            at: [10.0, 20.0],
            rot: 90.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        }];
        let d = &doors(&map)[0];
        let near = |a: P2, b: P2| dist(a, b) < 1e-4;
        assert!(near(d.face, [11.55, 19.0]), "{:?}", d.face);
        assert!(near(d.out, [1.0, 0.0]), "{:?}", d.out);
    }

    #[test]
    fn the_yards_door_is_clear() {
        let (map, _, _) = yard();
        assert_eq!(door_findings(&map), Vec::<String>::new());
    }

    #[test]
    fn a_shed_in_front_of_a_door_is_found() {
        let (mut map, i, d) = yard();
        let rot = map.placements[i].rot;
        map.placements.push(Placement {
            prefab: "shed".into(),
            at: d.point(3.5, 0.0),
            rot,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let found = door_findings(&map);
        assert!(
            found
                .iter()
                .any(|f| f.contains("house_plank") && f.contains("shed stands in front")),
            "{found:?}"
        );
    }

    #[test]
    fn a_crate_in_a_doorway_is_found() {
        let (mut map, _, d) = yard();
        map.markers.push(Marker {
            kind: MarkerKind::Loot {
                table: "food_common".into(),
            },
            at: d.point(3.0, 0.4),
            rot: 0.0,
            name: "Doorstep".into(),
            y: 0.0,
        });
        let found = door_findings(&map);
        assert!(
            found.iter().any(|f| f.contains("Doorstep's supply crate")),
            "{found:?}"
        );
    }

    /// Lifted 1.6 m off its footing, the house's steps (which reach down
    /// only to its footing less its lift) end in the air.
    #[test]
    fn a_door_over_a_drop_is_found() {
        let (mut map, i, _) = yard();
        map.placements[i].y = 1.6;
        let found = door_findings(&map);
        assert!(
            found
                .iter()
                .any(|f| f.contains("house_plank") && f.contains("drops")),
            "{found:?}"
        );
    }
}
