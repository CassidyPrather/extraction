//! Exports a map for Unity (the VRChat port): a terrain heightmap, splat
//! maps, every tree, and the map's placements, markers, regions, roads and
//! water as JSON, all in Unity's coordinates (see `extraction_core::unity`
//! and the `terrain.json` this writes). `unity/Editor/ExtractionImporter.cs`
//! builds a scene from the folder.
//!
//! Usage: `unity-export <map.ron> <out-dir>`

use extraction_core::forest::{self, TREE_SINK};
use extraction_core::geom::{self, P2};
use extraction_core::map::{MapDef, Marker, MarkerKind, prefab_footprint, prefab_wants_pad};
use extraction_core::terrain::Heightmap;
use extraction_core::unity::{self, ALPHA_RES, SplatLayer, TERRAIN_RES};
use serde_json::{Value, json};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(src), Some(out)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: unity-export <map.ron> <out-dir>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(src).unwrap_or_else(|e| {
        eprintln!("{src}: {e}");
        std::process::exit(1);
    });
    let map = MapDef::from_ron(&text).unwrap_or_else(|e| {
        eprintln!("{src}: {e}");
        std::process::exit(1);
    });
    let out = Path::new(out);
    std::fs::create_dir_all(out).expect("create out dir");
    let hm = Heightmap::build(&map);

    // Terrain heights.
    let heights = unity::unity_heights(&hm, TERRAIN_RES);
    let (lo, hi) = unity::min_max(&heights);
    let raw: Vec<u8> = unity::quantize_u16(&heights, lo, hi)
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    std::fs::write(out.join("heightmap.r16"), raw).expect("write heightmap");

    // Splats, and a water mask beside them.
    let splats = unity::splat_weights(&map, &hm, ALPHA_RES);
    let mut layers = Vec::new();
    for (layer, w) in SplatLayer::ALL.iter().zip(&splats) {
        let file = format!("splat_{}.png", layer.name());
        let bytes: Vec<u8> = w.iter().map(|v| (v * 255.0).round() as u8).collect();
        write_png(&out.join(&file), ALPHA_RES, &bytes);
        let coverage = w.iter().sum::<f32>() / w.len() as f32;
        layers.push(json!({
            "index": *layer as usize,
            "name": layer.name(),
            "file": file,
            "coverage": r3(coverage),
        }));
    }
    let texel = map.size / ALPHA_RES as f32;
    let water_mask: Vec<u8> = (0..ALPHA_RES * ALPHA_RES)
        .map(|k| {
            let p = [
                -map.half() + ((k % ALPHA_RES) as f32 + 0.5) * texel,
                -map.half() + ((k / ALPHA_RES) as f32 + 0.5) * texel,
            ];
            if hm.water_level(p).is_some() { 255 } else { 0 }
        })
        .collect();
    write_png(&out.join("water_mask.png"), ALPHA_RES, &water_mask);

    // Trees.
    let trees = forest::scatter_trees(&map, &hm);
    let mut species_counts = std::collections::BTreeMap::<String, usize>::new();
    let tree_json: Vec<Value> = trees
        .iter()
        .map(|t| {
            let species = format!("{:?}", t.species).to_lowercase();
            *species_counts.entry(species.clone()).or_default() += 1;
            json!({
                "species": species,
                "variant": t.variant,
                "pos": v3(unity::to_unity(t.pos)),
                "yaw_deg": r3(unity::yaw_to_unity(t.yaw).to_degrees()),
                "scale": r3(t.scale),
                "game_pos": v3(t.pos),
                "game_yaw_rad": r3(t.yaw),
            })
        })
        .collect();
    write_json(
        out,
        "trees.json",
        &json!({
            "count": trees.len(),
            "species": species_counts,
            "sink_m": r3(TREE_SINK),
            "trees": tree_json,
        }),
    );

    // Placements.
    let placements: Vec<Value> = map
        .placements
        .iter()
        .map(|p| {
            let footprint = prefab_footprint(&p.prefab);
            let ground = unity::placement_ground(&hm, p.at, p.rot, footprint);
            let base = if p.absolute { p.y } else { ground + p.y };
            json!({
                "prefab": p.prefab,
                "tag": p.tag,
                "pos": v3(unity::to_unity([p.at[0], base, p.at[1]])),
                "yaw_deg": r3(unity::placement_yaw_to_unity(p.rot)),
                "footprint": v3(footprint),
                "ground_y": r3(ground),
                "wants_pad": prefab_wants_pad(&p.prefab),
                "game": {
                    "at": v2(p.at),
                    "rot_deg": r3(p.rot),
                    "y": r3(p.y),
                    "absolute": p.absolute,
                },
            })
        })
        .collect();
    write_json(out, "placements.json", &json!({ "placements": placements }));

    // Markers.
    let markers: Vec<Value> = map.markers.iter().map(|m| marker_json(&hm, m)).collect();
    write_json(out, "markers.json", &json!({ "markers": markers }));

    // Regions.
    let regions: Vec<Value> = map
        .regions
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "danger": r.danger,
                "loot_richness": r3(r.richness()),
                "polygon": r.polygon.iter().map(|p| v2(unity::to_unity_2d(*p))).collect::<Vec<_>>(),
                "game_polygon": r.polygon.iter().map(|p| v2(*p)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let forests: Vec<Value> = map
        .forests
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "density_per_100m2": r3(f.density),
                "undergrowth": r3(f.undergrowth),
                "mix": f.mix.iter().map(|(s, w)| json!([format!("{s:?}").to_lowercase(), w])).collect::<Vec<_>>(),
                "polygon": f.polygon.iter().map(|p| v2(unity::to_unity_2d(*p))).collect::<Vec<_>>(),
            })
        })
        .collect();
    write_json(
        out,
        "regions.json",
        &json!({ "regions": regions, "forests": forests }),
    );

    // Roads, as built: the centreline every 2 m at its graded height.
    let roads: Vec<Value> = hm
        .road_lines
        .iter()
        .map(|rl| {
            let def = &map.roads[rl.road];
            let line: Vec<Value> = rl
                .points
                .iter()
                .zip(&rl.heights)
                .step_by(2)
                .chain(rl.points.last().zip(rl.heights.last()))
                .map(|(p, h)| v3(unity::to_unity([p[0], *h, p[1]])))
                .collect();
            let bridges: Vec<Value> = rl
                .bridges
                .iter()
                .map(|(a, b)| {
                    let end = |s: f32| {
                        let p = geom::polyline_at(&rl.points, s).0;
                        let seg = geom::polyline_closest(&rl.points, p).2;
                        let h = rl.heights[seg.min(rl.heights.len() - 1)];
                        v3(unity::to_unity([p[0], h, p[1]]))
                    };
                    json!({ "from": end(*a), "to": end(*b), "length_m": r3(b - a) })
                })
                .collect();
            json!({
                "name": def.name,
                "kind": format!("{:?}", rl.kind).to_lowercase(),
                "width_m": r3(rl.width),
                "shoulder_m": r3(def.shoulder),
                "centreline": line,
                "bridges": bridges,
                "game_control_points": def.points.iter().map(|p| v2(*p)).collect::<Vec<_>>(),
            })
        })
        .collect();
    write_json(out, "roads.json", &json!({ "roads": roads }));

    // Water: the centreline every 2 m at the surface level the game uses.
    let water: Vec<Value> = map
        .water
        .iter()
        .map(|w| {
            let line = geom::catmull_rom(&w.points, 2.0);
            let mut last = f32::NAN;
            let pts: Vec<Value> = line
                .iter()
                .map(|p| {
                    let level = water_level_near(&hm, *p).unwrap_or(last);
                    last = level;
                    v3(unity::to_unity([p[0], level, p[1]]))
                })
                .collect();
            json!({
                "name": w.name,
                "width_m": r3(w.width),
                "depth_m": r3(w.depth),
                "centreline": pts,
                "game_control_points": w.points.iter().map(|p| v2(*p)).collect::<Vec<_>>(),
            })
        })
        .collect();
    write_json(
        out,
        "water.json",
        &json!({ "mask": "water_mask.png", "water": water }),
    );

    // The terrain, and how to read everything else.
    let half = map.half();
    let range = (hi - lo).max(1e-6);
    write_json(
        out,
        "terrain.json",
        &json!({
            "map": map.name,
            "description": map.description,
            "unity_version": "2022.3 LTS",
            "size_m": map.size,
            "terrain": {
                "position": [-half, lo, -half],
                "size": [map.size, range, map.size],
                "comment": "Place the Terrain at `position` with TerrainData.size = `size` (the same min height and range the RAW was normalised with); Unity world heights then equal game heights.",
            },
            "heightmap": {
                "file": "heightmap.r16",
                "format": "raw, unsigned 16-bit little-endian (Unity 'Windows' byte order), no header",
                "resolution": TERRAIN_RES,
                "min_height_m": lo,
                "max_height_m": hi,
                "decode": "height_m = min_height_m + value / 65535 * (max_height_m - min_height_m)",
                "row_order": "south to north: row 0 is the south edge (Unity terrain local z = 0); columns run west to east. This is TerrainData.SetHeights' [row, column] order, so 'Flip Vertically' stays off in Unity's Import Raw.",
                "source": { "samples_per_side": hm.n, "cell_m": r3(hm.cell), "resampling": "bilinear" },
            },
            "alphamap": {
                "resolution": ALPHA_RES,
                "format": "8-bit greyscale PNG per layer; weights at a texel sum to 255 (rounding aside)",
                "row_order": "image order: the top row of each PNG is the north edge. Texture2D.LoadImage puts the top row at the highest y, which is north (Unity +z), so texture (x, y) maps straight to alphamap [y, x].",
                "texel_centres": "texel (col, row-from-south) covers game x = -size/2 + (col + 0.5) * size / resolution",
                "layers": layers,
                "quest_hint": "Android/Quest terrain shaders want four layers or fewer: merge forest into grass, mud into dirt, asphalt into gravel.",
            },
            "water_mask": "water_mask.png (same layout as the splats; 255 under water)",
            "coordinates": {
                "game": "right-handed, +y up, metres, origin at the map centre, +x east, +z SOUTH (north is -z)",
                "unity": "left-handed, +y up, metres, origin at the map centre, +x east, +z NORTH",
                "position": "unity = (game.x, game.y, -game.z): flip z, nothing else",
                "placement_yaw": "Unity Euler Y degrees = map `rot` (both are clockwise from north looking down)",
                "game_yaw_radians": "Unity yaw = -game yaw (Bevy's from_rotation_y is counter-clockwise looking down; Unity's Y rotation is clockwise)",
                "gltf_models": "glTF importers (glTFast, UnityGLTF) convert by negating X, not Z, so a model built for the game and imported from glTF needs 180 degrees added to its yaw to face the way it does in the game. The importer's 'Model Yaw Offset' does this.",
                "all_json_positions": "Unity world coordinates unless a field says `game_`",
            },
            "files": {
                "trees": "trees.json", "placements": "placements.json", "markers": "markers.json",
                "regions": "regions.json", "roads": "roads.json", "water": "water.json",
            },
            "counts": {
                "trees": trees.len(), "placements": map.placements.len(), "markers": map.markers.len(),
                "regions": map.regions.len(), "roads": map.roads.len(), "water": map.water.len(),
            },
        }),
    );

    println!(
        "{}: terrain {TERRAIN_RES}² ({lo:.2}..{hi:.2} m), {} splat layers at {ALPHA_RES}², {} trees, {} placements, {} markers -> {}",
        map.name,
        SplatLayer::ALL.len(),
        trees.len(),
        map.placements.len(),
        map.markers.len(),
        out.display()
    );
}

fn marker_json(hm: &Heightmap, m: &Marker) -> Value {
    let ground = hm.height(m.at);
    let (kind, table) = match &m.kind {
        MarkerKind::Loot { table } => ("Loot".to_string(), Some(table.clone())),
        k => (format!("{k:?}"), None),
    };
    json!({
        "kind": kind,
        "loot_table": table,
        "name": m.name,
        "pos": v3(unity::to_unity([m.at[0], ground + m.y, m.at[1]])),
        "yaw_deg": r3(unity::placement_yaw_to_unity(m.rot)),
        "ground_y": r3(ground),
        "game": { "at": v2(m.at), "rot_deg": r3(m.rot), "y": r3(m.y) },
    })
}

/// The water surface at or right next to a point, from the game's grid.
fn water_level_near(hm: &Heightmap, p: P2) -> Option<f32> {
    let i = ((p[0] - hm.origin) / hm.cell).round() as isize;
    let j = ((p[1] - hm.origin) / hm.cell).round() as isize;
    let n = hm.n as isize;
    let mut best: Option<(isize, f32)> = None;
    for dj in -2..=2 {
        for di in -2..=2 {
            let (a, b) = (i + di, j + dj);
            if a < 0 || b < 0 || a >= n || b >= n {
                continue;
            }
            let w = hm.water[hm.idx(a as usize, b as usize)];
            let d = di * di + dj * dj;
            if !w.is_nan() && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, w));
            }
        }
    }
    best.map(|(_, w)| w)
}

/// Rounds to millimetres, which is plenty and keeps the JSON small. (As
/// an `f64`: an `f32` goes into JSON with all its binary noise.)
fn r3(v: f32) -> f64 {
    (f64::from(v) * 1000.0).round() / 1000.0
}

fn v2(p: [f32; 2]) -> Value {
    json!([r3(p[0]), r3(p[1])])
}

fn v3(p: [f32; 3]) -> Value {
    json!([r3(p[0]), r3(p[1]), r3(p[2])])
}

fn write_json(dir: &Path, name: &str, value: &Value) {
    // Trees run to tens of thousands: one line each rather than twenty.
    let text = if name == "trees.json" {
        let mut v = value.clone();
        let trees = v
            .as_object_mut()
            .and_then(|o| o.remove("trees"))
            .unwrap_or_default();
        let head = serde_json::to_string_pretty(&v).expect("serialise");
        let rows: Vec<String> = trees
            .as_array()
            .map(|a| a.iter().map(|t| format!("    {t}")).collect())
            .unwrap_or_default();
        let head = head.trim_end().trim_end_matches('}').trim_end();
        format!("{head},\n  \"trees\": [\n{}\n  ]\n}}\n", rows.join(",\n"))
    } else {
        serde_json::to_string_pretty(value).expect("serialise")
    };
    std::fs::write(dir.join(name), text).expect("write json");
}

fn write_png(path: &Path, side: usize, grey: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), side as u32, side as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    writer.write_image_data(grey).expect("png data");
    writer.finish().expect("png finish");
}
