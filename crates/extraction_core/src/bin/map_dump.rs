//! Dumps a map's built terrain as raw little-endian arrays plus JSON
//! metadata, for `scripts/map_preview.py` (and anything else that wants to
//! look at a map without the game).
//!
//! Usage: `map-dump <map.ron> <out-dir>`
//!
//! Also runs the map lint and writes its main routes (`routes.json`) and
//! findings (`lint.json`) for the preview to draw, and the Surge's walk to
//! a roof past a minute's (`surge.f32`, for `--heat`).

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
        .map(|f| {
            format!(
                "{{\"name\":\"{}\",\"polygon\":[{}]}}",
                esc(&f.name),
                pts(&f.polygon)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    j += "],\"placements\":[";
    j += &map
        .placements
        .iter()
        .map(|p| {
            let f = extraction_core::map::prefab_footprint(&p.prefab);
            format!(
                "{{\"prefab\":\"{}\",\"at\":[{},{}],\"rot\":{},\"size\":[{},{},{}]}}",
                esc(&p.prefab),
                p.at[0],
                p.at[1],
                p.rot,
                f[0],
                f[1],
                f[2]
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
            // A sentry's sweep (its middle as sentry::bearing measures) and box, for the preview's wedge.
            let extra = match extraction_core::map_sentry::Sentry::of(m) {
                Some(s) => format!(
                    ",\"rot\":{},\"arc\":{},\"power\":[{},{}]",
                    s.centre, s.arc, s.power[0], s.power[1]
                ),
                None => String::new(),
            };
            format!(
                "{{\"kind\":\"{}\",\"name\":\"{}\",\"at\":[{},{}]{}}}",
                kind,
                esc(&m.name),
                m.at[0],
                m.at[1],
                extra
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
    // Trees (x, z) and rocks big enough to block (x, z, radius), for the
    // preview to dot in.
    let trees: Vec<f32> = extraction_core::forest::scatter_trees(&map, &hm)
        .iter()
        .flat_map(|t| [t.pos[0], t.pos[2]])
        .collect();
    write_f32("trees.f32", &trees);
    let rocks: Vec<f32> = extraction_core::rocks::scatter_rocks(&map, &hm)
        .iter()
        .filter(|r| r.radius >= extraction_core::rocks::SOLID_RADIUS)
        .flat_map(|r| [r.pos[0], r.pos[2], r.radius])
        .collect();
    write_f32("rocks.f32", &rocks);
    let lint = extraction_core::map_lint::lint(&map, &hm);
    std::fs::write(format!("{out}/routes.json"), lint.routes_json()).unwrap();
    std::fs::write(format!("{out}/lint.json"), lint.findings_json()).unwrap();
    eprint!("{}", lint.text());
    // The Surge: how far past a minute's walk the nearest roof is, on the
    // heightmap's grid (0 where one is nearer, or the ground isn't
    // comfortable, or is past the fence), for `map_preview.py --heat`.
    let (g, d) = extraction_core::map_surge::roof_field(&map, &hm);
    let surge: Vec<f32> = (0..hm.n * hm.n)
        .map(|k| {
            let at = hm.pos(k % hm.n, k / hm.n);
            let half = map.half() - map.fence_inset;
            if at[0].abs() > half || at[1].abs() > half {
                return 0.0;
            }
            g.cell_of(at)
                .map(|(i, j)| d[j * g.n + i] - extraction_core::map_surge::ROUTE_ROOF)
                .filter(|m| m.is_finite())
                .map_or(0.0, |m| m.max(0.0))
        })
        .collect();
    write_f32("surge.f32", &surge);
    let sight = extraction_core::map_sight::audit(&map, &hm);
    std::fs::write(format!("{out}/sight.json"), sight.json()).unwrap();
    println!("{} ({}x{} samples) -> {out}", map.name, hm.n, hm.n);
}
