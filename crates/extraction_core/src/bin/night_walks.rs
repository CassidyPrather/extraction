//! The ways home at night, for reading off frames (`night_walks::walks`):
//! one JSON line a stop, where the eye is and where it looks as a `.stills`
//! line takes it, and the ground the way runs over ahead of it with the
//! ground either side, for `scripts/night_route.py` to find in the frame.
//!
//!   night-walks <map>
//!   {"walk":"scout: Valve house>Gauge house","stop":3,"still":"...","ahead":[[x,y,z],..],"left":[..],"right":[..]}

use extraction_core::map::MapDef;
use extraction_core::terrain::Heightmap;

fn main() {
    let map_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sallow_reservoir".into());
    let Some(&(_, src)) = extraction_core::map::VALLEYS.iter().find(|v| v.0 == map_id) else {
        eprintln!("no valley {map_id}");
        std::process::exit(2)
    };
    let map = MapDef::from_ron(src).expect("map parses");
    let hm = Heightmap::build(&map);
    let fmt = |v: [f32; 3]| format!("[{:.2},{:.2},{:.2}]", v[0], v[1], v[2]);
    let list = |vs: &[[f32; 3]]| vs.iter().map(|v| fmt(*v)).collect::<Vec<_>>().join(",");
    for s in extraction_core::night_walks::walks(&map, &hm, &map_id) {
        println!(
            "{{\"walk\":\"{}>{}\",\"stop\":{},\"name\":\"{}\",\"eye\":{},\"yaw\":{:.1},\"pitch\":{:.1},\"still\":\"{} {:.1},{:.2},{:.1},{:.1},{:.1}\",\"ahead\":[{}],\"left\":[{}],\"right\":[{}],\"near_left\":[{}],\"near_right\":[{}],\"to_rv\":{:.1}}}",
            s.what,
            s.rv,
            s.stop,
            s.name,
            fmt(s.eye),
            s.yaw,
            s.pitch,
            s.name,
            s.eye[0],
            s.eye[1],
            s.eye[2],
            s.yaw,
            s.pitch,
            list(&s.ahead),
            list(&s.left),
            list(&s.right),
            list(&s.near_left),
            list(&s.near_right),
            s.to_rv
        );
    }
}
