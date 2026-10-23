//! Views of a valley's anomalies for screenshots: for each kind, the first
//! of its kind (in the Surge's layout for the seed and cycle) with a clear
//! line from `dist` off, one JSON line each, as `--camera` takes it.
//!
//!   anomaly-views [map] [seed] [cycle] [dist] [edge]
//!   {"kind":"Lift","anomaly":[146.3,-22.5],"camera":"132.2,0,-36.6,135,-2"}
//!
//! With `edge`, `dist` is from each anomaly's edge (not its centre), and
//! each line also carries the eye, the radius, and ground points along
//! lines across the edge (`probes`: per line, metres from the edge,
//! positive outside, and the point; five across the near edge, then one
//! across each side), for measuring how plainly the edge shows and where.

use extraction_core::anomaly::{AnomalyKind, layout, view_of};
use extraction_core::map::MapDef;
use extraction_core::terrain::Heightmap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let map = args.get(1).map_or("marrow_creek", |s| s.as_str());
    let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1u64);
    let cycle = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0u32);
    let dist = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20.0f32);
    let edge = args.get(5).is_some_and(|s| s == "edge");
    let Some(&(_, src)) = extraction_core::map::VALLEYS.iter().find(|v| v.0 == map) else {
        eprintln!("no valley {map}");
        std::process::exit(2)
    };
    let def = MapDef::from_ron(src).expect("map parses");
    let hm = Heightmap::build(&def);
    let all = layout(&def, &hm, seed, cycle);
    let blockers = extraction_core::anomaly::sight_blockers(&def, &hm);
    for kind in AnomalyKind::ALL {
        let Some((a, v)) = all.iter().filter(|a| a.kind == kind).find_map(|a| {
            let d = if edge { dist + a.radius } else { dist };
            view_of(&def, &hm, a, &all, &blockers, d).map(|v| (a, v))
        }) else {
            eprintln!("{kind:?}: no clear view");
            continue;
        };
        let mut line = serde_json::json!({
            "kind": format!("{kind:?}"),
            "anomaly": [a.at[0], a.at[1]],
            "camera": format!("{:.1},0,{:.1},{:.0},{:.0}", v.at[0], v.at[1], v.yaw, v.pitch),
        });
        if edge {
            // The camera's yaw and pitch as printed (whole degrees), so the
            // projection matches the shot.
            let eye = [v.at[0], hm.height(v.at) + 1.7, v.at[1]];
            let toward = (v.at[1] - a.at[1]).atan2(v.at[0] - a.at[0]);
            // Five across the near edge (what you walk up to), then two
            // across its sides, left and right (where it is: a side's
            // place on the screen hardly moves with the ground's height,
            // where the near edge's does on a slope).
            let side = std::f32::consts::FRAC_PI_2;
            let lines: Vec<_> = [-0.35f32, -0.18, 0.0, 0.18, 0.35, -side, side]
                .iter()
                .map(|off| {
                    let ang = toward + off;
                    (-12..=12)
                        .map(|k| {
                            let d = k as f32 * 0.1;
                            let r = a.radius + d;
                            let p = [a.at[0] + ang.cos() * r, a.at[1] + ang.sin() * r];
                            serde_json::json!([d, [p[0], hm.height(p), p[1]]])
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            line["eye"] = serde_json::json!(eye);
            line["yaw"] = serde_json::json!(v.yaw.round());
            line["pitch"] = serde_json::json!(v.pitch.round());
            line["radius"] = serde_json::json!(a.radius);
            line["probes"] = serde_json::json!(lines);
        }
        println!("{line}");
    }
}
