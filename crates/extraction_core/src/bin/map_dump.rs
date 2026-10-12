//! Dumps a map's built terrain as raw little-endian arrays plus JSON
//! metadata, for `scripts/map_preview.py` (and anything else that wants to
//! look at a map without the game).
//!
//! Usage: `map-dump <map.ron> <out-dir>`

use extraction_core::map::MapDef;
use extraction_core::terrain::Heightmap;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(src), Some(out)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: map-dump <map.ron> <out-dir>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(src).expect("read map");
    let map = MapDef::from_ron(&text).unwrap_or_else(|e| {
        eprintln!("{src}: {e}");
        std::process::exit(1);
    });
    let hm = Heightmap::build(&map);
    std::fs::create_dir_all(out).expect("create out dir");
    let write_f32 = |name: &str, data: &[f32]| {
        let mut f = std::fs::File::create(format!("{out}/{name}")).unwrap();
        for v in data {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
    };
    write_f32("height.f32", &hm.heights);
    write_f32("water.f32", &hm.water);
    write_f32("wet.f32", &hm.wet);
    let roads: Vec<f32> = hm
        .roads
        .iter()
        .map(|r| match r.kind {
            None => 0.0,
            Some(k) => (k as u8 as f32 + 1.0) * 1000.0 + r.weight as f32,
        })
        .collect();
    write_f32("roads.f32", &roads);
    let slope: Vec<f32> = (0..hm.n * hm.n)
        .map(|k| hm.slope_deg(hm.pos(k % hm.n, k / hm.n)))
        .collect();
    write_f32("slope.f32", &slope);

    // Metadata as hand-rolled JSON (core has no JSON dependency).
    let pts = |ps: &[[f32; 2]]| {
        ps.iter()
            .map(|p| format!("[{},{}]", p[0], p[1]))
            .collect::<Vec<_>>()
            .join(",")
    };
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut j = String::new();
    j += &format!(
        "{{\"name\":\"{}\",\"size\":{},\"n\":{},\"cell\":{},\"origin\":{},",
        esc(&map.name),
        map.size,
        hm.n,
        hm.cell,
        hm.origin
    );
    j += "\"forests\":[";
    j += &map
        .forests
        .iter()
        .map(|f| format!("{{\"name\":\"{}\",\"polygon\":[{}]}}", esc(&f.name), pts(&f.polygon)))
        .collect::<Vec<_>>()
        .join(",");
    j += "],\"placements\":[";
    j += &map
        .placements
        .iter()
        .map(|p| {
            format!(
                "{{\"prefab\":\"{}\",\"at\":[{},{}],\"rot\":{}}}",
                esc(&p.prefab),
                p.at[0],
                p.at[1],
                p.rot
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    j += "],\"markers\":[";
    j += &map
        .markers
        .iter()
        .map(|m| {
            let kind = format!("{:?}", m.kind);
            let kind = kind.split(['(', ' ', '{']).next().unwrap_or("").to_string();
            format!(
                "{{\"kind\":\"{}\",\"name\":\"{}\",\"at\":[{},{}]}}",
                kind,
                esc(&m.name),
                m.at[0],
                m.at[1]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    j += "],\"regions\":[";
    j += &map
        .regions
        .iter()
        .map(|r| {
            format!(
                "{{\"name\":\"{}\",\"danger\":{},\"polygon\":[{}]}}",
                esc(&r.name),
                r.danger,
                pts(&r.polygon)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    j += "],\"bridges\":[";
    let mut bridges = Vec::new();
    for rl in &hm.road_lines {
        for (a, b) in &rl.bridges {
            let pa = extraction_core::geom::polyline_at(&rl.points, *a).0;
            let pb = extraction_core::geom::polyline_at(&rl.points, *b).0;
            bridges.push(format!("[[{},{}],[{},{}]]", pa[0], pa[1], pb[0], pb[1]));
        }
    }
    j += &bridges.join(",");
    j += "]}";
    std::fs::write(format!("{out}/map.json"), j).unwrap();
    println!("{} ({}x{} samples) -> {out}", map.name, hm.n, hm.n);
}
