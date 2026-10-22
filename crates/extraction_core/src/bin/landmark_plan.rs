//! Where a new landmark would do the most for the orientation gauge
//! (`map_lint`'s "two landmarks in sight"): for each candidate prefab, the
//! spots where placing it would turn the most blind ground into ground
//! with two landmarks in sight. A designer's aid: it suggests, the map
//! says what fits the valley's story.
//!
//! Usage: `landmark-plan <map.ron> <prefab,prefab,...> [picks] [grid m]
//! [--avoid x,z,r ...]` (no spot within `r` of `x,z`: a view already as
//! busy as `map_lint`'s ViewLoad allows).
//!
//! Prints the valley's landmarks, then picks greedily: the best (spot,
//! prefab) by ground gained, added to the map, and again.

use extraction_core::geom::{P2, dist};
use extraction_core::map::{MapDef, Placement, prefab_footprint};
use extraction_core::map_landmarks::{self, Canopy};
use extraction_core::map_lint::{CLEAR_SIGHT, TWO_MARKS_STEP, UNIQUE_SIGHT, WOOD_EDGE};
use extraction_core::terrain::Heightmap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(src), Some(kinds)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: landmark-plan <map.ron> <prefab,prefab,...> [picks] [grid m]");
        std::process::exit(2);
    };
    let picks: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(3);
    let avoid: Vec<(P2, f32)> = args
        .windows(2)
        .filter(|w| w[0] == "--avoid")
        .filter_map(|w| {
            let v: Vec<f32> = w[1].split(',').filter_map(|t| t.parse().ok()).collect();
            (v.len() == 3).then(|| ([v[0], v[1]], v[2]))
        })
        .collect();
    let grid: f32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(40.0);
    let text = std::fs::read_to_string(src).expect("read map");
    let mut map = MapDef::from_ron(&text).expect("parse map");
    let hm = Heightmap::build(&map);
    let canopy = Canopy::new(&map, &hm);
    let walk = extraction_core::map_nav::zone_grid(&map, &hm, extraction_core::map_nav::MAX_SLOPE);
    // The lint's own sampling, so the plan's numbers are its numbers.
    let (cells, _) = map_landmarks::gauge_cells(&map, &walk, TWO_MARKS_STEP, WOOD_EDGE);
    let share = |map: &MapDef| {
        let marks = map_landmarks::landmarks(map, &hm);
        let two = cells
            .iter()
            .filter(|p| {
                map_landmarks::kinds_in_sight(&marks, &hm, &canopy, **p, CLEAR_SIGHT, 2) >= 2
            })
            .count();
        two as f32 / cells.len().max(1) as f32
    };
    for m in map_landmarks::landmarks(&map, &hm) {
        let at = m.at();
        println!(
            "landmark {} ({}) at ({:.0}, {:.0})",
            m.name, m.kind, at[0], at[1]
        );
    }
    let mut now = share(&map);
    println!("now {:.0}%", now * 100.0);
    let kinds: Vec<&str> = kinds.split(',').collect();
    // Candidate spots: walkable, flat enough to stand a tower, clear of
    // roads and of every placement's footprint by a margin.
    let mut spots: Vec<P2> = Vec::new();
    let n = (640.0 / grid) as i32;
    for j in 0..=n {
        for i in 0..=n {
            let p = [-320.0 + i as f32 * grid, -320.0 + j as f32 * grid];
            if !walk.walkable(p)
                || hm.slope_deg(p) > 12.0
                || hm.road_at(p).weight > 0
                || avoid.iter().any(|(c, r)| dist(*c, p) < *r)
            {
                continue;
            }
            if map.placements.iter().any(|q| {
                let f = prefab_footprint(&q.prefab);
                dist(q.at, p) < f[0].max(f[2]) * 0.5 + 15.0
            }) {
                continue;
            }
            if !map
                .regions
                .iter()
                .any(|r| extraction_core::geom::point_in_polygon(&r.polygon, p))
            {
                continue;
            }
            spots.push(p);
        }
    }
    println!("{} candidate spots", spots.len());
    for _ in 0..picks {
        let marks = map_landmarks::landmarks(&map, &hm);
        let mut best: Option<(f32, P2, &str)> = None;
        for &k in &kinds {
            let kind = map_landmarks::kind(k);
            for &p in &spots {
                // Not a twin of one of its kind.
                if marks
                    .iter()
                    .any(|m| m.kind == kind && m.tops.iter().any(|t| dist(t.0, p) < UNIQUE_SIGHT))
                {
                    continue;
                }
                let mut trial = map.clone();
                trial.placements.push(Placement {
                    prefab: k.to_string(),
                    at: p,
                    ..map.placements[0].clone()
                });
                trial.placements.last_mut().unwrap().rot = 0.0;
                trial.placements.last_mut().unwrap().y = 0.0;
                trial.placements.last_mut().unwrap().absolute = false;
                trial.placements.last_mut().unwrap().tag = String::new();
                let s = share(&trial);
                if best.is_none_or(|b| s > b.0) {
                    best = Some((s, p, k));
                }
            }
        }
        let Some((s, p, k)) = best else { break };
        println!(
            "pick {k} at ({:.0}, {:.0}): {:.0}% -> {:.0}%",
            p[0],
            p[1],
            now * 100.0,
            s * 100.0
        );
        now = s;
        let mut add = map.placements[0].clone();
        add.prefab = k.to_string();
        add.at = p;
        add.rot = 0.0;
        add.y = 0.0;
        add.absolute = false;
        add.tag = String::new();
        map.placements.push(add);
    }
}
