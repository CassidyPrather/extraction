//! Views of a valley's anomalies for screenshots: for each kind, the first
//! of its kind (in the Surge's layout for the seed and cycle) with a clear
//! line from `dist` off, one JSON line each, as `--camera` takes it.
//!
//!   anomaly-views [map] [seed] [cycle] [dist]
//!   {"kind":"Lift","anomaly":[146.3,-22.5],"camera":"132.2,0,-36.6,135,-2"}

use extraction_core::anomaly::{AnomalyKind, layout, view_of};
use extraction_core::map::MapDef;
use extraction_core::terrain::Heightmap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let map = args.get(1).map_or("marrow_creek", |s| s.as_str());
    let seed = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1u64);
    let cycle = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0u32);
    let dist = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20.0f32);
    let Some(&(_, src)) = extraction_core::map::VALLEYS.iter().find(|v| v.0 == map) else {
        eprintln!("no valley {map}");
        std::process::exit(2)
    };
    let def = MapDef::from_ron(src).expect("map parses");
    let hm = Heightmap::build(&def);
    let all = layout(&def, &hm, seed, cycle);
    let blockers = extraction_core::anomaly::sight_blockers(&def, &hm);
    for kind in AnomalyKind::ALL {
        let Some((a, v)) = all
            .iter()
            .filter(|a| a.kind == kind)
            .find_map(|a| view_of(&hm, a, &all, &blockers, dist).map(|v| (a, v)))
        else {
            eprintln!("{kind:?}: no clear view");
            continue;
        };
        println!(
            "{}",
            serde_json::json!({
                "kind": format!("{kind:?}"),
                "anomaly": [a.at[0], a.at[1]],
                "camera": format!("{:.1},0,{:.1},{:.0},{:.0}", v.at[0], v.at[1], v.yaw, v.pitch),
            })
        );
    }
}
