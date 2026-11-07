//! The ways home at night, for reading off frames: from each of a valley's
//! first jobs (`map_lint::first_jobs`) to its nearest rendezvous, by the
//! way a newcomer keeps to easy ground (the lint's walks home), a stop
//! every `STEP` metres. One JSON line a stop: where the eye is and where
//! it looks, as a `.stills` line takes it, and the ground the way runs
//! over ahead of it with the ground either side (4 m off, and 2 m off for
//! inside a torch's pool), for
//! `scripts/night_route.py` to find in the frame.
//!
//! And along each line of snow poles (the poles nearest one rendezvous),
//! home from its far pole, so a torch's case covers the poles where no
//! first job's way runs beside them.
//!
//!   night-walks <map>
//!   {"walk":"scout: Valve house>Gauge house","stop":3,"still":"...","ahead":[[x,y,z],..],"left":[..],"right":[..]}

use extraction_core::geom::{P2, dist, lerp};
use extraction_core::map::{MapDef, MarkerKind};
use extraction_core::terrain::Heightmap;

/// Metres between stops along a way home.
const STEP: f32 = 50.0;
/// The eye over the ground (m), and how far down it looks (degrees):
/// a walker watching the way ahead.
const EYE: f32 = 1.7;
const LOOK_DOWN: f32 = 6.0;
/// Where along the way ahead the ground is sampled (m), and how far to
/// either side of it the ground beside the way is.
const AHEAD: [f32; 8] = [6.0, 9.0, 12.0, 16.0, 20.0, 25.0, 30.0, 40.0];
const BESIDE: f32 = 4.0;
/// Close beside the way (m): inside a torch's pool, where a worn track or
/// an edge (a wall, a kerb, a fence) tells it from what's next to it.
const NEAR: f32 = 2.0;
/// How far along the way out of a kiosk its first stop may wait for the
/// way to come round it (m).
const ROUND_KIOSK: f32 = 12.0;

/// The point `s` metres along a polyline, and the way it runs there.
fn along(path: &[P2], s: f32) -> Option<(P2, P2)> {
    let mut left = s;
    for w in path.windows(2) {
        let d = dist(w[0], w[1]);
        if d <= 0.0 {
            continue;
        }
        if left <= d {
            let dir = [(w[1][0] - w[0][0]) / d, (w[1][1] - w[0][1]) / d];
            return Some((lerp(w[0], w[1], left / d), dir));
        }
        left -= d;
    }
    None
}

/// The height a walker's feet are at: the ground, or a bridge's deck over
/// it (the way crosses on the deck; the ground under is a gully, where the
/// eye saw only the deck's underside).
fn walked_height(hm: &Heightmap, p: P2) -> f32 {
    let mut h = hm.height(p);
    for rl in &hm.road_lines {
        if rl.bridges.is_empty() {
            continue;
        }
        let mut s = 0.0;
        for (k, q) in rl.points.iter().enumerate() {
            if k > 0 {
                s += dist(rl.points[k - 1], *q);
            }
            if rl.on_bridge(s) && dist(*q, p) <= rl.width * 0.5 + 0.5 {
                h = h.max(rl.heights[k]);
            }
        }
    }
    h
}

/// Each line of snow poles, by the rendezvous they're nearest: its far
/// pole, to walk home from.
fn pole_lines(map: &MapDef, rvs: &[(String, P2)]) -> Vec<(String, P2)> {
    let nearest = |p: P2| {
        rvs.iter()
            .enumerate()
            .min_by(|a, b| dist(a.1.1, p).total_cmp(&dist(b.1.1, p)))
            .map(|(i, _)| i)
    };
    let mut far: Vec<Option<P2>> = vec![None; rvs.len()];
    for p in map.placements.iter().filter(|p| p.prefab == "snow_pole") {
        let Some(i) = nearest(p.at) else { continue };
        if far[i].is_none_or(|f| dist(p.at, rvs[i].1) > dist(f, rvs[i].1)) {
            far[i] = Some(p.at);
        }
    }
    far.into_iter()
        .flatten()
        .map(|at| (format!("pole line ({:.0}, {:.0})", at[0], at[1]), at))
        .collect()
}

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
    let grid =
        extraction_core::map_nav::zone_grid(&map, &hm, extraction_core::map_lint::COMFORT_SLOPE);
    let rvs: Vec<(String, P2)> = map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.clone(), m.at))
        .collect();
    let height = |p: P2| walked_height(&hm, p);
    let ground = |p: P2| [p[0], height(p), p[1]];
    let fmt = |v: [f32; 3]| format!("[{:.2},{:.2},{:.2}]", v[0], v[1], v[2]);
    // Each walk's start, and for one out of a kiosk the way its door faces
    // (its first stop is where the way has come round the kiosk).
    let mut starts: Vec<(String, P2, Option<P2>)> =
        extraction_core::map_lint::first_jobs(&map, &map_id)
            .into_iter()
            .map(|(what, at)| {
                let (at, out) = surfaced(&map, &hm, &grid, &rvs, at);
                (what, at, out)
            })
            .collect();
    starts.extend(
        pole_lines(&map, &rvs)
            .into_iter()
            .map(|(w, at)| (w, at, None)),
    );
    for (walk, (what, at, out)) in starts.into_iter().enumerate() {
        let Some((rv, to)) = rvs
            .iter()
            .min_by(|a, b| dist(a.1, at).total_cmp(&dist(b.1, at)))
        else {
            continue;
        };
        let Some(path) = grid.path(at, *to) else {
            eprintln!("{what}: no way home to {rv}");
            continue;
        };
        let length: f32 = path.windows(2).map(|w| dist(w[0], w[1])).sum();
        let mut stop = 0;
        let mut s = 0.0;
        while s + AHEAD[0] < length {
            let Some((p, dir)) = along(&path, s) else {
                break;
            };
            // Look along the way 15 m on, not the way's bearing here. Out of
            // a kiosk, while that looks back over the kiosk (the way going
            // round it), no stop yet: you turn and go round it first.
            let on = along(&path, (s + 15.0).min(length)).map_or(dir, |(q, _)| {
                let d = dist(p, q).max(0.1);
                [(q[0] - p[0]) / d, (q[1] - p[1]) / d]
            });
            if out
                .is_some_and(|o| stop == 0 && s < ROUND_KIOSK && o[0] * on[0] + o[1] * on[1] < 0.0)
            {
                s += 1.0;
                continue;
            }
            let look = on;
            let yaw = look[0].atan2(-look[1]).to_degrees().rem_euclid(360.0);
            let eye = [p[0], height(p) + EYE, p[1]];
            // A file name: the walk's number and the stop's.
            let name = format!("w{walk:02}_{stop:02}");
            let (mut ahead, mut lefts, mut rights) = (Vec::new(), Vec::new(), Vec::new());
            let (mut near_l, mut near_r) = (Vec::new(), Vec::new());
            for a in AHEAD {
                let Some((q, d)) = along(&path, s + a) else {
                    break;
                };
                let side = [-d[1], d[0]];
                ahead.push(fmt(ground(q)));
                lefts.push(fmt(ground([
                    q[0] + side[0] * BESIDE,
                    q[1] + side[1] * BESIDE,
                ])));
                rights.push(fmt(ground([
                    q[0] - side[0] * BESIDE,
                    q[1] - side[1] * BESIDE,
                ])));
                near_l.push(fmt(ground([q[0] + side[0] * NEAR, q[1] + side[1] * NEAR])));
                near_r.push(fmt(ground([q[0] - side[0] * NEAR, q[1] - side[1] * NEAR])));
            }
            println!(
                "{{\"walk\":\"{what}>{rv}\",\"stop\":{stop},\"name\":\"{name}\",\"eye\":{},\"yaw\":{yaw:.1},\"pitch\":{:.1},\"still\":\"{name} {:.1},{:.2},{:.1},{yaw:.1},{:.1}\",\"ahead\":[{}],\"left\":[{}],\"right\":[{}],\"near_left\":[{}],\"near_right\":[{}],\"to_rv\":{:.1}}}",
                fmt(eye),
                -LOOK_DOWN,
                eye[0],
                eye[1],
                eye[2],
                -LOOK_DOWN,
                ahead.join(","),
                lefts.join(","),
                rights.join(","),
                near_l.join(","),
                near_r.join(","),
                dist(p, *to)
            );
            stop += 1;
            s += STEP;
        }
    }
}

/// How far under the ground a job's floor lies before it's down in a site
/// and not on the surface over it (m).
const BELOW: f32 = 2.5;

/// Where the way home from a job starts on the surface: for one down in an
/// underground site (the depot's ledger), a step out of whichever of the
/// site's kiosks, its entrance or an exit, makes the shorter way home, the
/// way to it underground (as the crow flies) and the walk on from it to
/// the nearest rendezvous together; not the ground over the job's room,
/// which no one stands on. With it, the way that kiosk's door faces.
fn surfaced(
    map: &MapDef,
    hm: &Heightmap,
    grid: &extraction_core::nav::NavGrid,
    rvs: &[(String, P2)],
    at: P2,
) -> (P2, Option<P2>) {
    use extraction_core::underground;
    let ways = extraction_core::map_lint::kiosk_ways_out(map);
    let home = |p: P2| {
        rvs.iter()
            .filter_map(|(_, rv)| grid.path(p, *rv))
            .map(|path| path.windows(2).map(|w| dist(w[0], w[1])).sum::<f32>())
            .min_by(f32::total_cmp)
    };
    for site in underground::placed(map) {
        let Some(r) = underground::raster(&site.id) else {
            continue;
        };
        let Some(floor) = r.floor_at(site.to_site(at)) else {
            continue;
        };
        if hm.footing(&site.id, site.at, site.rot) + floor > hm.height(at) - BELOW {
            continue;
        }
        let up = ways
            .iter()
            .filter(|(id, _, _)| *id == site.id)
            .filter_map(|(_, p, o)| home(*p).map(|h| (*p, *o, dist(*p, at) + h)))
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((up, out, _)) = up {
            eprintln!("start {at:?} is down in {}: from {up:?}", site.id);
            return (up, Some(out));
        }
    }
    (at, None)
}
