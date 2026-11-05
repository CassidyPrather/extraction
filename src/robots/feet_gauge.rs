//! Feet that hold the ground: a gauge. Every legged machine (`anim::walk`
//! drives them all: the walker, the crawler, the hound, the heavy; the
//! grappler and the collector are walkers in body) is walked over each
//! valley's own ground, where it's hardest: its steepest walkable slopes,
//! in and out of its doors (their steps), across its ruins' rubble, down
//! its banks to the water, and along Sallow's dam. Three things are held:
//!
//! - a foot on the ground stays where it came down: it slides less than
//!   [`SLIDE`] over a stance;
//! - a foot comes down on the surface: within [`TOUCH`] of it, not in the
//!   air over it nor in it;
//! - the body rides on its feet: never more than [`RIDE`] off the height
//!   its planted feet hold it at.
//!
//! The report is the worst of each per valley, with where it was, to look
//! at (`reel:feetat`).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::anim::Gait;
use super::walker::Walker;
use super::{Brain, Job, JobKind, Senses};
use crate::world::{Layer, Zone};

/// The most a planted foot may slide over one stance (m, at a walker's
/// size).
pub const SLIDE: f32 = 0.05;
/// How near the surface a foot comes down (m, at a walker's size: 5 cm
/// on a crawler).
pub const TOUCH: f32 = 0.08;
/// The most the body may ride off what holds it up, its planted feet and
/// the ground under it (m, at a walker's size): its bob, sag and sway are
/// inside [`RIDE_USUALLY`]; coming down off a raised doorway it's brought
/// back at `anim::RIDE_CATCH`, a drop, not a jump, and is out to this.
pub const RIDE: f32 = 0.6;
/// What the body rides inside all but [`RIDE_RARELY`] of the time.
pub const RIDE_USUALLY: f32 = 0.3;
/// The share of its ticks a body may be past [`RIDE_USUALLY`].
pub const RIDE_RARELY: f32 = 0.002;

/// The valleys.
pub const VALLEYS: [&str; 5] = [
    "marrow_creek",
    "kettle_pass",
    "cinder_works",
    "sallow_reservoir",
    "rime_fell",
];

/// The legged machines.
pub const KINDS: [&str; 4] = ["walker", "crawler", "hound", "heavy"];

/// A way over hard ground: what it is, and its points.
#[derive(Clone, Debug)]
pub struct Route {
    pub what: String,
    pub pts: Vec<Vec3>,
}

fn on(zone: &Zone, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, zone.ground(x, z), z)
}

/// The hard ground of a valley.
pub fn routes(
    zone: &Zone,
    entrances: &super::doors::Entrances,
    nav: &extraction_core::nav::NavGrid,
) -> Vec<Route> {
    let mut out = Vec::new();
    let half = zone.map.half() - 25.0;
    // Its steepest walkable slopes: three, apart.
    let mut slopes: Vec<(f32, Vec3, Vec3)> = Vec::new();
    let mut x = -half;
    while x <= half {
        let mut z = -half;
        while z <= half {
            if nav.walkable([x, z]) {
                let gx = zone.ground(x + 1.0, z) - zone.ground(x - 1.0, z);
                let gz = zone.ground(x, z + 1.0) - zone.ground(x, z - 1.0);
                let g = Vec2::new(gx, gz) * 0.5;
                slopes.push((g.length(), Vec3::new(x, 0.0, z), Vec3::new(g.x, 0.0, g.y)));
            }
            z += 6.0;
        }
        x += 6.0;
    }
    slopes.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut taken: Vec<Vec3> = Vec::new();
    for (g, p, d) in slopes {
        if taken.len() >= 3 {
            break;
        }
        if taken.iter().any(|t| t.distance(p) < 40.0) {
            continue;
        }
        taken.push(p);
        let d = d.normalize_or(Vec3::X);
        // Down it and back up.
        let (a, b) = (p + d * 7.0, p - d * 7.0);
        out.push(Route {
            what: format!("slope {:.0}% at ({:.0}, {:.0})", g * 100.0, p.x, p.z),
            pts: vec![on(zone, a.x, a.z), on(zone, b.x, b.z), on(zone, a.x, a.z)],
        });
    }
    // In and out of its doors: four, apart.
    let mut doors: Vec<Vec3> = Vec::new();
    for e in &entrances.doors {
        if doors.len() >= 4 || !e.outside {
            continue;
        }
        if doors.iter().any(|d| d.distance(e.mid) < 25.0) {
            continue;
        }
        doors.push(e.mid);
        let out_dir = Vec3::new(e.out.x, 0.0, e.out.z).normalize_or(Vec3::X);
        // Over its step and 1 m in (a shed is 2 m deep: a route on to
        // 3 m walked out through its back wall).
        let (a, m, b) = (e.mid + out_dir * 4.0, e.mid, e.mid - out_dir * 1.0);
        out.push(Route {
            what: format!("door at ({:.0}, {:.0})", e.mid.x, e.mid.z),
            pts: vec![
                on(zone, a.x, a.z),
                on(zone, m.x, m.z),
                on(zone, b.x, b.z),
                on(zone, m.x, m.z),
                on(zone, a.x, a.z),
            ],
        });
    }
    // Across its ruins: three.
    for p in zone
        .map
        .placements
        .iter()
        .filter(|p| {
            extraction_core::map::RUIN_STATES
                .iter()
                .any(|s| p.prefab.ends_with(s))
        })
        .take(3)
    {
        let c = Vec3::new(p.at[0], 0.0, p.at[1]);
        let a = c - Vec3::X * 7.0;
        let b = c + Vec3::X * 7.0;
        out.push(Route {
            what: format!("{} rubble at ({:.0}, {:.0})", p.prefab, c.x, c.z),
            pts: vec![on(zone, a.x, a.z), on(zone, c.x, c.z), on(zone, b.x, b.z)],
        });
    }
    // Down its banks to the water: two each.
    for w in &zone.map.water {
        let n = w.points.len();
        for k in [n / 3, 2 * n / 3] {
            if k + 1 >= n {
                continue;
            }
            let (p, q) = (w.points[k], w.points[k + 1]);
            let along = Vec2::new(q[0] - p[0], q[1] - p[1]).normalize_or(Vec2::X);
            let side = Vec3::new(-along.y, 0.0, along.x);
            let c = Vec3::new(p[0], 0.0, p[1]);
            let dry = c + side * (w.width * 0.5 + 4.0);
            let wet = c + side * (w.width * 0.5 - 0.3);
            out.push(Route {
                what: format!("{} bank at ({:.0}, {:.0})", w.name, c.x, c.z),
                pts: vec![
                    on(zone, dry.x, dry.z),
                    on(zone, wet.x, wet.z),
                    on(zone, dry.x, dry.z),
                ],
            });
        }
    }
    // (Over open ground a machine goes the way its paths take it, round
    // what the nav keeps it off, boulders and trunks: a straight line walked
    // it up a 2 m rock no path would. A door's threshold it takes as it is.)
    for r in out.iter_mut().filter(|r| !r.what.starts_with("door")) {
        let mut pts: Vec<Vec3> = vec![r.pts[0]];
        for w in r.pts.windows(2) {
            if let Some(p) = nav.path([w[0].x, w[0].z], [w[1].x, w[1].z]) {
                pts.extend(p.iter().skip(1).map(|q| on(zone, q[0], q[1])));
            }
        }
        r.pts = pts;
    }
    // Along the dam.
    for r in zone
        .map
        .roads
        .iter()
        .filter(|r| r.name == "Crest road" || r.name == "Face stair")
    {
        let pts: Vec<Vec3> = r.points.iter().map(|p| on(zone, p[0], p[1])).collect();
        if pts.len() >= 2 {
            out.push(Route {
                what: format!("the dam's {}", r.name),
                pts,
            });
        }
    }
    out
}

/// The worst seen of one measure.
#[derive(Clone, Debug, Default)]
pub struct Worst {
    pub by: f32,
    pub kind: &'static str,
    pub route: String,
    pub at: Vec3,
    pub note: String,
}

impl Worst {
    fn see(&mut self, by: f32, kind: &'static str, route: &str, at: Vec3) {
        self.see_why(by, kind, route, at, String::new);
    }
    fn see_why(
        &mut self,
        by: f32,
        kind: &'static str,
        route: &str,
        at: Vec3,
        note: impl FnOnce() -> String,
    ) {
        if by > self.by {
            *self = Worst {
                by,
                kind,
                route: route.into(),
                at,
                note: note(),
            };
        }
    }
}

/// A valley's worst: a planted foot's slide over a stance, a foot come
/// down over and under the surface, and the body above and below its
/// feet (each m at a walker's size).
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Ticks a body rode past [`RIDE`], of all machines' ticks.
    pub ride_over: usize,
    pub ticks: usize,
    pub slide: Worst,
    pub float: Worst,
    pub sink: Worst,
    pub high: Worst,
    pub low: Worst,
    pub steps: usize,
}

/// Where the surface is under `p`: the highest thing a foot stands on
/// (the world, rubble, things), else the terrain.
fn surface(spatial: &SpatialQuery, zone: &Zone, p: Vec3) -> f32 {
    let terrain = zone.ground(p.x, p.z);
    // (Not loose things: kicked about by sixty feet, one moved under a
    // foot between its probe and its landing reads as the foot in it.)
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Debris]);
    // (From just over the foot: from higher it started inside a bridge's
    // railing or a trunk beside it, and found nothing.)
    let from = p + Vec3::Y * 0.06;
    spatial
        .cast_ray(from, Dir3::NEG_Y, 1.5, true, &filter)
        .filter(|h| h.distance > 0.02)
        .map(|h| from.y - h.distance)
        .map_or(terrain, |y| y.max(terrain))
}

/// The ground under a body at `p`: the first thing down from it (the
/// world, rubble, things; not robots), else the terrain.
fn ground_under(spatial: &SpatialQuery, zone: &Zone, p: Vec3) -> f32 {
    let terrain = zone.ground(p.x, p.z);
    let filter = SpatialQueryFilter::from_mask([Layer::World, Layer::Debris, Layer::Item]);
    spatial
        .cast_ray(
            p,
            Dir3::NEG_Y,
            (p.y - terrain).max(0.0) + 0.5,
            true,
            &filter,
        )
        .filter(|h| h.distance > 0.02)
        .map(|h| p.y - h.distance)
        .map_or(terrain, |y| y.max(terrain))
}

/// Walks every legged machine over `map`'s hard ground and reports the
/// worst.
pub fn gauge(map: &str, secs: f32) -> (Report, Vec<Route>) {
    let opts = crate::LaunchOptions {
        map: map.into(),
        robots: false,
        ..Default::default()
    };
    let mut app = crate::headless_app(opts, false);
    app.finish();
    app.cleanup();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f32(1.0 / 90.0),
    ));
    for _ in 0..3 {
        app.update();
    }
    let zone = app.world().resource::<Zone>().clone();
    let nav = app.world().resource::<super::walker::Nav>().0.clone();
    let rs = {
        let entrances = app.world().resource::<super::doors::Entrances>();
        routes(&zone, entrances, &nav)
    };
    let catalog = app.world().resource::<crate::items::Catalog>().clone();
    // Each machine on each way (they don't stand on one another: a foot
    // finds the world, rubble and things, not robots).
    let mut bots: Vec<(Entity, &'static str, usize)> = Vec::new();
    {
        let mut q = bevy::ecs::system::SystemState::<Commands>::new(app.world_mut());
        let mut c = q.get_mut(app.world_mut()).unwrap();
        for (ri, r) in rs.iter().enumerate() {
            let at = r.pts[0];
            let job = Job {
                kind: JobKind::Survey,
                site: at,
                duration: 1.0e6,
                exit: at,
            };
            for kind in KINDS {
                let e = match kind {
                    "walker" => super::walker::spawn_walker(&mut c, &zone, at, job.clone()),
                    "crawler" => super::walker::spawn_walker_sized(
                        &mut c,
                        &zone,
                        at,
                        job.clone(),
                        false,
                        40.0,
                        super::indoor::CRAWLER_SIZE,
                        None,
                    ),
                    "hound" => super::hound::spawn_hound(&mut c, &zone, at, job.clone()),
                    _ => super::heavy::spawn_heavy(&mut c, &zone, &catalog, at, job.clone()),
                };
                c.entity(e).remove::<Senses>().remove::<Brain>();
                bots.push((e, kind, ri));
            }
        }
        q.apply(app.world_mut());
    }
    for _ in 0..30 {
        app.update();
    }
    for (e, _, ri) in &bots {
        if let Some(mut w) = app.world_mut().get_mut::<Walker>(*e) {
            w.path = rs[*ri].pts[1..].iter().map(|p| p.with_y(0.0)).collect();
        }
    }
    let mut report = Report::default();
    // Per machine, per foot: where it stood and how far it has slid this
    // stance.
    // Per machine, per foot, this stance: where its drawn foot came down,
    // the furthest it has gone from there, its step's progress last tick,
    // and whether it's slipping on ice (`anim::slip_feet`: meant to slide).
    let mut stood: Vec<[(Vec3, f32, f32, bool); super::anim::LEGS]> =
        vec![[(Vec3::NAN, 0.0, 1.0, false); super::anim::LEGS]; bots.len()];
    let mut spatial = bevy::ecs::system::SystemState::<SpatialQuery>::new(app.world_mut());
    let ticks = (secs * 90.0) as usize;
    for tick in 0..ticks {
        app.update();
        let world = app.world();
        let sq = spatial.get(world).unwrap();
        for (b, (e, kind, ri)) in bots.iter().enumerate() {
            let (Some(w), Some(g), Some(t), Some(poses)) = (
                world.get::<Walker>(*e),
                world.get::<Gait>(*e),
                world.get::<Transform>(*e),
                world.get::<super::walker::LegPoses>(*e),
            ) else {
                continue;
            };
            let s = g.size.max(0.1);
            let route = &rs[*ri].what;
            let mut feet_y = Vec::new();
            for (i, f) in w.feet.iter().enumerate() {
                if !f.alive {
                    continue;
                }
                let (was, slid, t0, icy) = stood[b][i];
                let drawn = poses.0[i][2];
                if f.t >= 1.0 {
                    feet_y.push(f.planted.y);
                }
                let icy = icy || g.slid[i] > 0.0;
                stood[b][i] = if f.t < 1.0 {
                    (was, slid, f.t, false)
                } else if t0 < 1.0 || !was.is_finite() {
                    // Come down: on the surface?
                    if t0 < 1.0 {
                        let off = f.planted.y - surface(&sq, &zone, f.planted);
                        report.float.see(off / s, kind, route, f.planted);
                        report.sink.see(-off / s, kind, route, f.planted);
                    }
                    (drawn, 0.0, f.t, false)
                } else {
                    let slid = slid.max((drawn - was).with_y(0.0).length());
                    if !icy {
                        report.slide.see(slid / s, kind, route, drawn);
                    }
                    (was, slid, f.t, icy)
                };
            }
            if let Ok(tr) = std::env::var("FEET_TRACE") {
                if tr == format!("{kind}:{route}") {
                    let fs: Vec<String> = w
                        .feet
                        .iter()
                        .map(|f| {
                            format!(
                                "t{:.2} p{:.2} fr{:.2} to{:.2}",
                                f.t, f.planted.y, f.from.y, f.to.y
                            )
                        })
                        .collect();
                    eprintln!(
                        "TRACE {tick} body {:.3} terrain {:.3} | {}",
                        t.translation.y,
                        zone.ground(t.translation.x, t.translation.z),
                        fs.join(" | ")
                    );
                }
            }
            if !feet_y.is_empty() && tick > 90 {
                // What holds it: its planted feet and the ground under it (a
                // step, a slope it's over); its height over them, between
                // the lowest and the highest.
                // (Probed from the body: from under it, over a rock, it
                // started inside the rock and found the terrain.)
                let under = ground_under(&sq, &zone, t.translation);
                let lo = feet_y.iter().copied().fold(under, f32::min);
                let hi = feet_y.iter().copied().fold(under, f32::max);
                let h = g.shape.height() * s;
                let (over, under) = (t.translation.y - (hi + h), (lo + h) - t.translation.y);
                let why = || {
                    format!(
                        "body {:.2}, feet {:?}, terrain under {:.2}, steps {:?}, sag {:.2} crouch {:.2} down {}",
                        t.translation.y,
                        feet_y
                            .iter()
                            .map(|y| (y * 100.0).round() / 100.0)
                            .collect::<Vec<_>>(),
                        zone.ground(t.translation.x, t.translation.z),
                        w.feet
                            .iter()
                            .map(|f| (f.t * 10.0).round() / 10.0)
                            .collect::<Vec<_>>(),
                        g.sag,
                        g.crouch,
                        g.down(),
                    )
                };
                report.ticks += 1;
                if over.max(under) / s > RIDE_USUALLY {
                    report.ride_over += 1;
                }
                report
                    .high
                    .see_why(over / s, kind, route, t.translation, why);
                report
                    .low
                    .see_why(under / s, kind, route, t.translation, why);
            }
        }
        report.steps += 1;
    }
    (report, rs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every legged machine holds every valley's hard ground: its planted
    /// feet stay put, its feet come down on the surface, its body rides
    /// on its feet. (`--nocapture` prints each valley's worst and where.)
    #[test]
    fn every_legged_machine_holds_every_valleys_ground() {
        let mut bad = Vec::new();
        // (`FEET_MAP=name` gauges just that valley.)
        let only = std::env::var("FEET_MAP").ok();
        for map in VALLEYS
            .into_iter()
            .filter(|m| only.as_deref().is_none_or(|o| o == *m))
        {
            let (r, rs) = gauge(map, 14.0);
            let rarely = r.ride_over as f32 / r.ticks.max(1) as f32;
            println!(
                "{map}: {} ways; a body past {RIDE_USUALLY} for {:.2}% of its ticks",
                rs.len(),
                100.0 * rarely
            );
            if rarely > RIDE_RARELY {
                bad.push(format!(
                    "{map}: a body past {RIDE_USUALLY} for {:.2}% of its ticks (limit {:.1}%)",
                    100.0 * rarely,
                    100.0 * RIDE_RARELY
                ));
            }
            for rt in &rs {
                let pts: Vec<String> = rt
                    .pts
                    .iter()
                    .map(|p| format!("{:.1},{:.1}", p.x, p.z))
                    .collect();
                println!("    {}: {}", rt.what, pts.join(" "));
            }
            for (name, w, limit) in [
                ("slide", &r.slide, SLIDE),
                ("float", &r.float, TOUCH),
                ("sink", &r.sink, TOUCH),
                ("ride high", &r.high, RIDE),
                ("ride low", &r.low, RIDE),
            ] {
                println!(
                    "  {name:9} {:6.3} m  {:7} {} ({:.1}, {:.1}, {:.1}) {}",
                    w.by, w.kind, w.route, w.at.x, w.at.y, w.at.z, w.note
                );
                if w.by > limit {
                    bad.push(format!(
                        "{map}: {name} {:.3} m (limit {limit}): {} on {}",
                        w.by, w.kind, w.route
                    ));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "feet that don't hold the ground:\n{}",
            bad.join("\n")
        );
    }
}
