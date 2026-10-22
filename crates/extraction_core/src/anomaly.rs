//! Anomalies: where they lie each Surge cycle and what they leave behind.
//!
//! Every cycle the Surge rearranges the Zone's anomalies. Most cluster in
//! the map's artifact fields; a few lie loose elsewhere, off the roads and
//! out of the water. The layout is a pure function of the map, the seed and
//! the cycle, so a saved game rebuilds the same anomalies without saving
//! them, and the director can add temporary "blooms" on top.

use crate::geom;
use crate::map::{MapDef, MarkerKind};
use crate::noise::Rng;
use crate::terrain::Heightmap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnomalyKind {
    /// Gravity runs the wrong way: things float, swirl, and get flung.
    Lift,
    /// Heat with no fire: burns whatever stands in it.
    Burner,
    /// A static snare: discharges into whatever touches it, then recharges.
    Snare,
    /// A knot of weight in the air: drags everything loose toward it,
    /// crushes what reaches its heart, and bends bullets that pass close.
    Well,
    /// A fold in space: whatever crosses it is blinked out the far side, a
    /// few metres on, still moving.
    Mirror,
    /// Dead air: it eats every sound made in it, so nothing outside hears
    /// a shot or a step, and nothing inside hears anything. Stay too long
    /// and the silence presses on you.
    Hush,
    /// Still cold that steals heat: in it you stiffen, slower and slower,
    /// to a crawl, and at the last it bites. A live hearth keeps it off.
    Frost,
}

impl AnomalyKind {
    pub const ALL: [AnomalyKind; 7] = [
        AnomalyKind::Lift,
        AnomalyKind::Burner,
        AnomalyKind::Snare,
        AnomalyKind::Well,
        AnomalyKind::Mirror,
        AnomalyKind::Hush,
        AnomalyKind::Frost,
    ];

    /// What the Surge scatters each cycle. Wells and mirrors don't move:
    /// the maps place them (`MarkerKind::Anomaly`), and the director can
    /// bloom one.
    pub const SURGE: [AnomalyKind; 3] =
        [AnomalyKind::Lift, AnomalyKind::Burner, AnomalyKind::Snare];

    /// By its name in a map or site file ("Lift", "Well", ...).
    pub fn from_name(name: &str) -> Option<AnomalyKind> {
        Self::ALL.into_iter().find(|k| format!("{k:?}") == name)
    }

    /// The artifact this kind grows, by item id.
    pub fn artifact(self) -> &'static str {
        match self {
            AnomalyKind::Lift => "art_featherstone",
            AnomalyKind::Burner => "art_hearth",
            AnomalyKind::Snare => "art_aegis",
            AnomalyKind::Well => "art_lodestone",
            AnomalyKind::Mirror => "art_shard",
            AnomalyKind::Hush => "art_hushstone",
            AnomalyKind::Frost => "art_rimestone",
        }
    }

    /// (min, max) radius in metres.
    pub fn radius_range(self) -> (f32, f32) {
        match self {
            AnomalyKind::Lift => (2.5, 4.0),
            AnomalyKind::Burner => (1.4, 2.4),
            AnomalyKind::Snare => (1.8, 3.0),
            AnomalyKind::Well => (1.6, 2.4),
            AnomalyKind::Mirror => (1.2, 1.8),
            AnomalyKind::Hush => (2.5, 4.0),
            AnomalyKind::Frost => (2.5, 3.5),
        }
    }

    /// How far round its centre it reaches out and does something: a
    /// well's pull goes well past its heart.
    pub fn reach(self, radius: f32) -> f32 {
        match self {
            AnomalyKind::Well => radius * WELL_REACH,
            _ => radius,
        }
    }

    /// How wide a berth a careful walker gives it: inside this, it has you.
    pub fn danger_radius(self, radius: f32) -> f32 {
        match self {
            AnomalyKind::Well => radius * 1.8,
            AnomalyKind::Mirror => radius + MIRROR_THROW,
            _ => radius,
        }
    }
}

// --- Wells -----------------------------------------------------------------

/// A well's pull reaches this many radii out.
pub const WELL_REACH: f32 = 3.0;
/// Height of a well's heart above the ground, metres.
pub const WELL_HEIGHT: f32 = 1.3;
/// Pull on loose things at one radius out, m/s².
pub const WELL_PULL: f32 = 16.0;
/// Pull on a walker at one radius out, m/s²: you can still walk out from
/// the edge, and not from the heart.
pub const WELL_DRAG: f32 = 3.2;
/// Health a second at the heart.
pub const WELL_CRUSH: f32 = 30.0;
/// The heart, as a share of the radius: what reaches it is crushed.
pub const WELL_HEART: f32 = 0.45;
/// The lens: `K` in `K / (d² + s²)`, m³/s². A rifle round (700 m/s) a
/// metre off the heart turns a few degrees; a pistol round, at half the
/// speed, four times as much.
pub const WELL_LENS: f32 = 18_000.0;

/// How much of a well's pull is felt at `d` from its heart: all of it
/// out to 60% of its reach, fading smoothly to nothing at the reach, so
/// nothing snaps as it crosses in.
pub fn well_falloff(d: f32, radius: f32) -> f32 {
    let reach = radius * WELL_REACH;
    let t = ((1.0 - d / reach) / 0.4).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Acceleration (m/s²) toward the heart for a loose body at `rel` from
/// it: rises toward the heart and then softens, so things fall in and
/// circle instead of flying through.
pub fn well_pull(rel: [f32; 3], radius: f32, strength: f32) -> [f32; 3] {
    let d = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
    if d < 1e-4 {
        return [0.0; 3];
    }
    let r2 = radius * radius;
    let a = strength * r2 * 1.25 / (d * d + r2 * 0.25) * well_falloff(d, radius);
    [-rel[0] / d * a, -rel[1] / d * a, -rel[2] / d * a]
}

/// Acceleration (m/s²) on a bullet at `rel` from the heart: the lens,
/// independent of the round's mass, so a slow round bends more.
pub fn well_bend(rel: [f32; 3], radius: f32) -> [f32; 3] {
    let d = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt();
    if d < 1e-4 {
        return [0.0; 3];
    }
    let soft = 0.3 * radius;
    let a = WELL_LENS / (d * d + soft * soft) * well_falloff(d, radius);
    [-rel[0] / d * a, -rel[1] / d * a, -rel[2] / d * a]
}

/// The change in velocity a well at `rel_from` (the round's position less
/// the heart's) gives a round flying `velocity` for `dt`: the lens summed
/// along the tick's whole segment in short steps, since a rifle round
/// crosses a well's reach in a tick or two.
pub fn well_impulse(rel_from: [f32; 3], velocity: [f32; 3], dt: f32, radius: f32) -> [f32; 3] {
    let step = [velocity[0] * dt, velocity[1] * dt, velocity[2] * dt];
    let len = (step[0] * step[0] + step[1] * step[1] + step[2] * step[2]).sqrt();
    let n = ((len / (0.1 * radius)).ceil() as usize).clamp(1, 400);
    let mut dv = [0.0f32; 3];
    for i in 0..n {
        let f = (i as f32 + 0.5) / n as f32;
        let p = [
            rel_from[0] + step[0] * f,
            rel_from[1] + step[1] * f,
            rel_from[2] + step[2] * f,
        ];
        let a = well_bend(p, radius);
        for k in 0..3 {
            dv[k] += a[k] * dt / n as f32;
        }
    }
    dv
}

/// Whether a round flying `step` from `rel_from` passes within a well's
/// reach this tick (the cheap test before [`well_impulse`]).
pub fn well_in_reach(rel_from: [f32; 3], step: [f32; 3], radius: f32) -> bool {
    let reach = radius * WELL_REACH;
    let l2 = step[0] * step[0] + step[1] * step[1] + step[2] * step[2];
    let t = if l2 < 1e-9 {
        0.0
    } else {
        (-(rel_from[0] * step[0] + rel_from[1] * step[1] + rel_from[2] * step[2]) / l2)
            .clamp(0.0, 1.0)
    };
    let c = [
        rel_from[0] + step[0] * t,
        rel_from[1] + step[1] * t,
        rel_from[2] + step[2] * t,
    ];
    c[0] * c[0] + c[1] * c[1] + c[2] * c[2] < reach * reach
}

// --- Hushes ----------------------------------------------------------------

/// Seconds in a Hush before its silence starts to press on you.
pub const HUSH_GRACE: f32 = 8.0;
/// Health a second it takes after that.
pub const HUSH_PRESS: f32 = 3.0;
/// A carried hushstone is a small Hush round its carrier, this wide.
pub const HUSHSTONE_RADIUS: f32 = 1.2;

/// Whether a sound made at `at` is eaten by any of `hushes` (centre,
/// radius): it never reaches anything outside.
pub fn hushed(at: [f32; 3], hushes: &[([f32; 3], f32)]) -> bool {
    hushes.iter().any(|(c, r)| {
        let d = [at[0] - c[0], at[1] - c[1], at[2] - c[2]];
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < r * r
    })
}

/// What `secs` seconds in a Hush cost, health a second.
pub fn hush_press(secs: f32) -> f32 {
    if secs > HUSH_GRACE { HUSH_PRESS } else { 0.0 }
}

// --- Frosts ----------------------------------------------------------------

/// A frost feeds the body's one chill, the valley's (`cold`: 0 warm ..= 1
/// frozen through), so a frost and the weather add up, and a roof (or
/// going down) warms you back, slowly. Inside one, the chill stiffens you
/// and, frozen through, bites.
///
/// Chill a second at a frost's heart: a second and a quarter from warm to
/// frozen through, so even a dash in for its stone and out costs you.
pub const FROST_CHILL: f32 = 0.8;
/// And at its edge.
pub const FROST_CHILL_EDGE: f32 = 0.3;
/// Pace a second the stiffness leaves you once you're out of it (the cold
/// itself stays: it's the valley's to warm off).
pub const FROST_THAW: f32 = 0.25;
/// How much of your pace full chill takes in a frost: never all of it, so
/// you can always crawl out.
pub const FROST_SLOW: f32 = 0.7;
/// Chill at which you're told your legs are going.
pub const FROST_WARN: f32 = 0.4;
/// Health a second a frost takes from whoever's in it frozen through.
pub const FROST_BITE: f32 = 8.0;
/// A live hearth on you: a frost chills you at this share of the rate, and
/// never past `FROST_HEARTH_CAP`; it stiffens you no more than that, and
/// never bites.
pub const FROST_HEARTH: f32 = 0.25;
pub const FROST_HEARTH_CAP: f32 = 0.3;
/// A live rimestone on you keeps this much chill on you always (a little
/// stiff), and a burner's heat can't touch you.
pub const RIMESTONE_CHILL: f32 = 0.15;

/// Chill a second `d` from a frost's centre (radius `r`): its heart is
/// coldest; nothing outside it.
pub fn frost_rate(d: f32, r: f32) -> f32 {
    if d >= r {
        return 0.0;
    }
    FROST_CHILL_EDGE + (FROST_CHILL - FROST_CHILL_EDGE) * (1.0 - d / r)
}

/// One step of a frost on a body's chill: `inside_rate` from
/// [`frost_rate`] (0 out of any frost: the chill is left to the valley),
/// with or without a live hearth or rimestone on it.
pub fn chill_step(chill: f32, inside_rate: f32, hearth: bool, rimestone: bool, dt: f32) -> f32 {
    let floor = if rimestone { RIMESTONE_CHILL } else { 0.0 };
    let next = if inside_rate > 0.0 {
        let rate = if hearth {
            inside_rate * FROST_HEARTH
        } else {
            inside_rate
        };
        let cap = if hearth { FROST_HEARTH_CAP } else { 1.0 };
        (chill + rate * dt).min(cap.max(chill))
    } else {
        chill
    };
    next.clamp(floor, 1.0)
}

/// The share of pace a frost takes from a body at `chill` standing in it.
pub fn frost_stiffness(chill: f32, hearth: bool) -> f32 {
    let c = chill.clamp(0.0, 1.0);
    FROST_SLOW * if hearth { c.min(FROST_HEARTH_CAP) } else { c }
}

/// Pace kept at `chill` in a frost.
pub fn frost_pace(chill: f32) -> f32 {
    1.0 - frost_stiffness(chill, false)
}

// --- Mirrors ---------------------------------------------------------------

/// How far past its far edge a mirror sets down what it blinks, metres.
pub const MIRROR_THROW: f32 = 3.0;
/// Seconds a mirror takes to fold back up after a blink.
pub const MIRROR_RECHARGE: f32 = 1.5;

/// Where a mirror at `centre` sends something that came in at `entry`:
/// out the far side, `MIRROR_THROW` past its edge. `ok` says whether a
/// spot is somewhere it can land (dry, open ground); if the straight-
/// through spot isn't, it swings round a sixth of a turn at a time.
pub fn mirror_exit(
    centre: [f32; 2],
    entry: [f32; 2],
    radius: f32,
    ok: impl Fn([f32; 2]) -> bool,
) -> Option<[f32; 2]> {
    let mut dir = geom::sub(centre, entry);
    let l = geom::len(dir);
    dir = if l < 1e-3 {
        [1.0, 0.0]
    } else {
        geom::scale(dir, 1.0 / l)
    };
    let out = radius + MIRROR_THROW;
    for k in [0i32, 1, -1, 2, -2, 3] {
        let a = k as f32 * std::f32::consts::FRAC_PI_3;
        let (s, c) = a.sin_cos();
        let d = [dir[0] * c - dir[1] * s, dir[0] * s + dir[1] * c];
        let p = geom::add(centre, geom::scale(d, out));
        if ok(p) {
            return Some(p);
        }
    }
    None
}

// --- The detector ----------------------------------------------------------

/// The detector hears anomalies this far from their edge, metres.
pub const DETECTOR_RANGE: f32 = 25.0;

/// Seconds between the detector's clicks with the nearest anomaly's edge
/// `edge` metres away (negative: inside it): a lazy tick at the limit of
/// its range, a rattle at the edge. None when there is nothing in range.
pub fn detector_interval(edge: f32) -> Option<f32> {
    if edge >= DETECTOR_RANGE {
        return None;
    }
    let t = (edge / DETECTOR_RANGE).clamp(0.0, 1.0);
    Some(0.06 + 1.4 * t * t)
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnomalySpec {
    pub kind: AnomalyKind,
    /// Ground position (x, z).
    pub at: [f32; 2],
    pub radius: f32,
    /// An artifact has grown at its heart this cycle.
    pub artifact: Option<&'static str>,
}

/// Chance that an anomaly in a field holds an artifact this cycle.
pub const FIELD_ARTIFACT_CHANCE: f32 = 0.35;
/// Chance for a loose anomaly elsewhere.
pub const LOOSE_ARTIFACT_CHANCE: f32 = 0.12;
/// The rarest find: a glass eye, in place of the usual artifact.
pub const GLASS_EYE_CHANCE: f32 = 0.1;

/// Somewhere an anomaly can sit: dry, off-road, not too steep.
pub fn placeable(map: &MapDef, hm: &Heightmap, p: [f32; 2]) -> bool {
    let half = map.size * 0.5 - map.fence_inset - 4.0;
    if p[0].abs() > half || p[1].abs() > half {
        return false;
    }
    if hm.water_level(p).is_some() || hm.road_at(p).kind.is_some() {
        return false;
    }
    if hm.slope_deg(p) > 24.0 {
        return false;
    }
    // Not inside a building.
    !map.placements.iter().any(|pl| {
        let f = crate::map::prefab_footprint(&pl.prefab);
        geom::dist(pl.at, p) < f[0].max(f[2]) * 0.6 + 2.0
    })
}

/// Margin an anomaly keeps from the edge of any road or path, metres.
pub const ROAD_MARGIN: f32 = 1.0;

/// Whether an anomaly of `radius` at `p` keeps off every road and path:
/// the road is the safe, exposed way, and a shimmer across it is a trap
/// nobody walking it would see coming.
pub fn clear_of_roads(hm: &Heightmap, p: [f32; 2], radius: f32) -> bool {
    hm.road_lines.iter().all(|rl| {
        let (d, _, _) = geom::polyline_closest(&rl.points, p);
        d > rl.width * 0.5 + radius + ROAD_MARGIN
    })
}

// --- The detector ladder ----------------------------------------------------

/// What a detector can tell you, cheapest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectorTier {
    /// Ticks for the nearest anomaly's edge, wherever it is.
    Clicker,
    /// Held, hears only what it points at, and pings for a stone in its
    /// cone.
    Sweep,
    /// Held, a screen shows the stones and anomalies round you.
    Bear,
}

impl DetectorTier {
    pub fn of(id: &str) -> Option<DetectorTier> {
        match id {
            "detector" => Some(DetectorTier::Clicker),
            "detector_sweep" => Some(DetectorTier::Sweep),
            "detector_bear" => Some(DetectorTier::Bear),
            _ => None,
        }
    }
}

/// A sweep's cone, half-angle in degrees: what it points at, it hears.
pub const SWEEP_CONE_DEG: f32 = 25.0;
/// How far a sweep hears a stone, and a Bear shows one, metres.
pub const STONE_RANGE: f32 = 20.0;
/// How far a Bear's screen reaches for anomalies, metres.
pub const BEAR_RANGE: f32 = 25.0;

/// Whether `to` (from the detector) lies within `half_deg` of `dir`.
pub fn in_cone(dir: [f32; 3], to: [f32; 3], half_deg: f32) -> bool {
    let l = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (ld, lt) = (l(dir), l(to));
    if ld < 1e-6 || lt < 1e-6 {
        return lt < 1e-6;
    }
    let cos = (dir[0] * to[0] + dir[1] * to[1] + dir[2] * to[2]) / (ld * lt);
    cos >= half_deg.to_radians().cos()
}

/// Seconds between a sweep's stone pings `d` metres from a stone in its
/// cone: slow at the edge of its range, quick close by.
pub fn stone_ping_interval(d: f32) -> Option<f32> {
    if d >= STONE_RANGE {
        return None;
    }
    let t = (d / STONE_RANGE).clamp(0.0, 1.0);
    Some(0.15 + 1.1 * t)
}

/// Where something at `rel` (x, z from the detector, world axes) shows on
/// a Bear's round screen held facing `yaw` (radians, 0 = -Z): screen x
/// right, y up (the way you face), both -1..1 across `range`. None past it.
pub fn bear_blip(rel: [f32; 2], yaw: f32, range: f32) -> Option<[f32; 2]> {
    let d = (rel[0] * rel[0] + rel[1] * rel[1]).sqrt();
    if d > range {
        return None;
    }
    // Into the holder's frame: forward is -Z turned by yaw.
    let (s, c) = yaw.sin_cos();
    let fwd = [-s, -c];
    let right = [c, -s];
    let x = rel[0] * right[0] + rel[1] * right[1];
    let y = rel[0] * fwd[0] + rel[1] * fwd[1];
    Some([x / range, y / range])
}

// --- What artifacts cost ----------------------------------------------------

/// A carried featherstone wants home: lifts within this many metres of it
/// wind up toward popping.
pub const FEATHER_WAKE_RANGE: f32 = 15.0;

/// How fast a lift `d` metres from a carried featherstone winds up, charge
/// a second (a lift with nothing in it unwinds at 1/6 a second, so this
/// beats that only within about two thirds of the range).
pub fn feather_wake(d: f32) -> f32 {
    if d >= FEATHER_WAKE_RANGE {
        0.0
    } else {
        0.6 * (1.0 - d / FEATHER_WAKE_RANGE)
    }
}

/// Whether a carried aegis is wet enough to bite: soaked ground (it's
/// been raining) or wading.
pub fn aegis_wet(wetness: f32, wading: f32) -> bool {
    wetness > 0.35 || wading > 0.1
}

/// Seconds to a wet aegis's next shock, `roll` in 0..1.
pub fn aegis_shock_in(roll: f32) -> f32 {
    8.0 + 7.0 * roll
}

/// Health a wet aegis's shock takes.
pub const AEGIS_SHOCK: f32 = 5.0;
/// A wet aegis's shocks don't take you below this: they still hurt, crack
/// and draw the machines, but a stone you can't get dry in a downpour isn't
/// a slow death for carrying it.
pub const AEGIS_FLOOR: f32 = 20.0;

/// A carried hearth runs you hot: stamina a second it takes back (a
/// rested walker gets 1/8 a second, so you still recover, at half speed).
pub const HEARTH_TIRE: f32 = 1.0 / 16.0;

/// Seconds between a carried shard's glints, `roll` in 0..1.
pub fn shard_glint_in(roll: f32) -> f32 {
    20.0 + 20.0 * roll
}

/// How far a shard's glint gives you away, metres.
pub const SHARD_GLINT_RADIUS: f32 = 35.0;

// --- Harvesting ------------------------------------------------------------

/// An artifact in a lift rides the column this high over its core, out of
/// reach, until the lift pops and throws it.
pub const LIFT_STONE_RIDE: f32 = 1.8;
/// Faster than this (m/s), a stone in a lift is being thrown, not ridden.
pub const LIFT_STONE_FLUNG: f32 = 3.0;
/// Seconds a stone stays hot out of a burner's coals.
pub const HOT_SECS: f32 = 20.0;
/// Health a second a hot stone takes from the hand that holds it.
pub const HOT_BURN: f32 = 10.0;

/// How hot a stone still is, 0..1, `out` seconds out of the coals (and
/// none at all once it has been in water).
pub fn heat_left(out: f32, quenched: bool) -> f32 {
    if quenched {
        0.0
    } else {
        (1.0 - out / HOT_SECS).clamp(0.0, 1.0)
    }
}

// --- The Surge ---------------------------------------------------------------

/// How agitated the anomalies are with `surge_in` seconds to the Surge
/// (negative: it's raging), given the warning's length: nothing before
/// the warning, rising to all of it as the Surge breaks, all of it while
/// it rages, and calm again once it has passed.
pub fn surge_agitation(surge_in: f32, warning: f32, length: f32) -> f32 {
    if surge_in > warning || surge_in <= -length {
        0.0
    } else if surge_in > 0.0 {
        let t = 1.0 - surge_in / warning;
        t * t
    } else {
        1.0
    }
}

/// How much an agitated anomaly swells: up to this share again.
pub const SURGE_SWELL: f32 = 0.3;
/// How much faster agitated anomalies charge and recharge, at full.
pub const SURGE_HURRY: f32 = 2.0;
/// Seconds before the Surge at which a new anomaly blooms near you.
pub const SURGE_BLOOMS: [f32; 5] = [240.0, 170.0, 110.0, 60.0, 25.0];
/// Chance a Surge bloom grows a stormglass at its heart (the only place
/// one grows): about one a Surge.
pub const STORMGLASS_CHANCE: f32 = 0.25;
/// Seconds a Surge bloom spends forming (visible, harmless) first.
pub const SURGE_FORMING: f32 = 4.0;

/// How many Surge blooms fall due as the clock goes from `before` to
/// `after` seconds to the Surge.
pub fn surge_blooms_due(before: f32, after: f32) -> usize {
    SURGE_BLOOMS
        .iter()
        .filter(|t| before > **t && after <= **t)
        .count()
}

/// Where a Surge bloom opens round someone at `at` heading `heading`
/// (x, z; zero if standing): 10 to 25 m off, and off to a side (50 to 130
/// degrees from the heading), never across the way they're going; on
/// ground `ok` allows. `roll` gives numbers in 0..1.
pub fn surge_bloom_spot(
    at: [f32; 2],
    heading: [f32; 2],
    mut roll: impl FnMut() -> f32,
    ok: impl Fn([f32; 2]) -> bool,
) -> Option<[f32; 2]> {
    let h = if geom::len(heading) > 0.1 {
        geom::scale(heading, 1.0 / geom::len(heading))
    } else {
        [0.0, -1.0]
    };
    let base = h[1].atan2(h[0]);
    for _ in 0..24 {
        let side = if roll() < 0.5 { -1.0 } else { 1.0 };
        let off = (50.0 + 80.0 * roll()).to_radians() * side;
        let d = 10.0 + 15.0 * roll();
        let a = base + off;
        let p = [at[0] + a.cos() * d, at[1] + a.sin() * d];
        if ok(p) {
            return Some(p);
        }
    }
    None
}

/// Whether an anomaly reaching `reach` at `p` keeps off the walls (every
/// building's footprint, every underground kiosk's, the exits' too) by
/// `map_surge::WALL_CLEAR` and out of the strip in front of every door
/// (`map_surge::anomaly_room`). `placeable`'s round clearance measures
/// from a building's middle, so along a long hall's side wall it let a
/// snare open 2 m from the wall; and it never knew the exit kiosks.
pub fn clear_of_buildings(map: &MapDef, p: [f32; 2], reach: f32) -> bool {
    crate::map_surge::anomaly_room(map, p).0 >= reach
}

/// Somewhere a run-time bloom (the director's, the Surge's) may open: the
/// same ground as the Surge's own layout, `placeable` (dry, not steep,
/// clear of every building by its footprint), `clear_of_roads` and
/// `clear_of_buildings` for how far the kind reaches.
pub fn bloom_ok(map: &MapDef, hm: &Heightmap, p: [f32; 2], kind: AnomalyKind, radius: f32) -> bool {
    let reach = kind.danger_radius(radius);
    placeable(map, hm, p) && clear_of_roads(hm, p, reach) && clear_of_buildings(map, p, reach)
}

/// Cycles between the Surge's great scatters. In between, what it scattered
/// creeps downhill a little each cycle, along the terrain's lows.
pub const SURGE_EPOCH: u32 = 6;
/// How far a Surge-born anomaly creeps in a cycle, metres, at most.
pub const MIGRATE_STEP: f32 = 7.0;
/// Below this gradient (rise over run) the ground is a low: what reaches it
/// stays, stirring in place.
const MIGRATE_FLAT: f32 = 0.03;

/// A camera looking at an anomaly: where it stands (x, z, eye height
/// over the ground), and its yaw and pitch in degrees (yaw 0 looks down
/// -z, 90 down +x; pitch up is positive), as `--camera` takes them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub at: [f32; 2],
    pub yaw: f32,
    pub pitch: f32,
}

/// What stands in a line of sight across a valley, (x/z, how near the line
/// may pass): every trunk, every rock (by its long side), every building
/// and prop the map places.
pub fn sight_blockers(map: &MapDef, hm: &Heightmap) -> Vec<([f32; 2], f32)> {
    let trees = crate::forest::scatter_trees(map, hm)
        .into_iter()
        .map(|t| ([t.pos[0], t.pos[2]], 1.5));
    let rocks = crate::rocks::scatter_rocks(map, hm).into_iter().map(|r| {
        let long = r.radius * r.stretch.max(1.0 / r.stretch);
        ([r.pos[0], r.pos[2]], long + 0.5)
    });
    let built = map.placements.iter().map(|p| {
        let f = crate::map::prefab_footprint(&p.prefab);
        (p.at, f[0].max(f[2]) * 0.5 + 0.5)
    });
    trees.chain(rocks).chain(built).collect()
}

/// A view of the anomaly `a` from `dist` off: of sixteen bearings, the one
/// on the gentlest ground whose line to a metre over its centre is clear of
/// rises, of `blockers` ([`sight_blockers`]), of every wall and of the
/// other anomalies, and off roads and water. None if every side is
/// blocked.
pub fn view_of(
    map: &MapDef,
    hm: &Heightmap,
    a: &AnomalySpec,
    others: &[AnomalySpec],
    blockers: &[([f32; 2], f32)],
    dist: f32,
) -> Option<View> {
    let target = hm.height(a.at) + 1.0;
    let mut best: Option<(f32, [f32; 2])> = None;
    for k in 0..16 {
        let b = k as f32 / 16.0 * std::f32::consts::TAU;
        let cam = [a.at[0] + b.cos() * dist, a.at[1] + b.sin() * dist];
        if hm.road_at(cam).weight > 0 || hm.water_level(cam).is_some() || hm.slope_deg(cam) > 18.0 {
            continue;
        }
        let eye = hm.height(cam) + 1.7;
        let line = [cam, a.at];
        let clear = (1..40).all(|i| {
            let t = i as f32 / 40.0;
            hm.height(geom::lerp(cam, a.at, t)) < eye + (target - eye) * t - 0.4
        }) && blockers
            .iter()
            .all(|(b, r)| geom::polyline_closest(&line, *b).0 > *r)
            // Walls by their faces (a long hall's side is far from its
            // middle): every half metre of the line clear of them.
            && (1..(dist * 2.0) as i32).all(|i| {
                let t = i as f32 / (dist * 2.0);
                clear_of_buildings(map, geom::lerp(cam, a.at, t), 0.3)
            })
            && others
                .iter()
                .filter(|o| geom::dist(o.at, a.at) > 0.5)
                .all(|o| geom::polyline_closest(&line, o.at).0 > o.radius * 2.5 + 2.0);
        let slope = hm.slope_deg(cam);
        if clear && best.is_none_or(|(s, _)| slope < s) {
            best = Some((slope, cam));
        }
    }
    let (_, cam) = best?;
    let d = [a.at[0] - cam[0], a.at[1] - cam[1]];
    let eye = hm.height(cam) + 1.7;
    Some(View {
        at: cam,
        yaw: d[0].atan2(-d[1]).to_degrees().rem_euclid(360.0),
        pitch: ((target - eye) / dist).atan().to_degrees(),
    })
}

/// The anomalies of a Surge cycle. Each [`SURGE_EPOCH`]-th cycle scatters
/// them afresh; the cycles after it walk that scatter downhill
/// ([`migrate`]), and every cycle grows its own artifacts.
pub fn layout(map: &MapDef, hm: &Heightmap, seed: u64, cycle: u32) -> Vec<AnomalySpec> {
    let start = cycle - cycle % SURGE_EPOCH;
    let (mut out, chances, standing) = scatter(map, hm, seed, start);
    if cycle == start {
        return out;
    }
    for c in start + 1..=cycle {
        migrate(map, hm, seed, c, &mut out, standing);
    }
    // This cycle's stones: the standing ones on their own stream (as at a
    // scatter), the Surge's on the cycle's.
    let mut srng = Rng::new(seed ^ 0x57A4D ^ (cycle as u64).wrapping_mul(0xD1B5_4A32_D192_ED03));
    let mut rng = Rng::new(seed ^ 0xA77F ^ (cycle as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    for (i, a) in out.iter_mut().enumerate() {
        a.artifact = if i < standing {
            srng.chance(FIELD_ARTIFACT_CHANCE)
                .then(|| a.kind.artifact())
        } else {
            rng.chance(chances[i]).then(|| {
                if rng.chance(GLASS_EYE_CHANCE) {
                    "art_glasseye"
                } else {
                    a.kind.artifact()
                }
            })
        };
    }
    out
}

/// One cycle's creep: each Surge-born anomaly (those after the first
/// `standing`) steps down the slope it sits on, wobbling, and stops short
/// of anything it may not reach: a road, water, a building, another
/// anomaly. On the flat of a low it only stirs.
pub fn migrate(
    map: &MapDef,
    hm: &Heightmap,
    seed: u64,
    cycle: u32,
    out: &mut [AnomalySpec],
    standing: usize,
) {
    let mut rng = Rng::new(seed ^ 0x316A ^ (cycle as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    for i in standing..out.len() {
        let a = out[i].clone();
        let h = |dx: f32, dz: f32| hm.height([a.at[0] + dx, a.at[1] + dz]);
        let g = [
            (h(3.0, 0.0) - h(-3.0, 0.0)) / 6.0,
            (h(0.0, 3.0) - h(0.0, -3.0)) / 6.0,
        ];
        let steep = (g[0] * g[0] + g[1] * g[1]).sqrt();
        let wobble = rng.range(-0.45, 0.45);
        let (dir, far) = if steep > MIGRATE_FLAT {
            (
                g[1].atan2(g[0]) + std::f32::consts::PI + wobble,
                MIGRATE_STEP,
            )
        } else {
            (rng.range(0.0, std::f32::consts::TAU), MIGRATE_STEP * 0.25)
        };
        for f in [1.0, 0.6, 0.3] {
            let p = [a.at[0] + dir.cos() * far * f, a.at[1] + dir.sin() * far * f];
            let clear = out.iter().enumerate().all(|(j, b)| {
                j == i || geom::dist(b.at, p) > b.kind.danger_radius(b.radius) + a.radius + 2.5
            });
            // Creeping, it keeps off walls and doorways as it was laid.
            if clear
                && placeable(map, hm, p)
                && clear_of_roads(hm, p, a.radius)
                && clear_of_buildings(map, p, a.kind.danger_radius(a.radius))
            {
                out[i].at = p;
                break;
            }
        }
    }
}

/// A great scatter: the standing anomalies (the first returned count),
/// then the Surge's, each with its chance of a stone.
fn scatter(
    map: &MapDef,
    hm: &Heightmap,
    seed: u64,
    cycle: u32,
) -> (Vec<AnomalySpec>, Vec<f32>, usize) {
    let mut rng = Rng::new(seed ^ 0xA70A ^ (cycle as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut out: Vec<AnomalySpec> = Vec::new();
    let mut chances: Vec<f32> = Vec::new();
    let fits = |out: &[AnomalySpec], p: [f32; 2], r: f32| {
        out.iter()
            .all(|a| geom::dist(a.at, p) > a.kind.danger_radius(a.radius) + r + 2.5)
    };
    let place = |rng: &mut Rng,
                 out: &mut Vec<AnomalySpec>,
                 chances: &mut Vec<f32>,
                 centre: [f32; 2],
                 spread: f32,
                 chance: f32| {
        for _ in 0..30 {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let d = spread * rng.f32().sqrt();
            let p = [centre[0] + a.cos() * d, centre[1] + a.sin() * d];
            let kind = AnomalyKind::SURGE[rng.below(3)];
            let (lo, hi) = kind.radius_range();
            let radius = rng.range(lo, hi);
            if !placeable(map, hm, p)
                || !clear_of_roads(hm, p, radius)
                || !fits(out, p, radius)
                || !clear_of_buildings(map, p, kind.danger_radius(radius))
            {
                continue;
            }
            let artifact = rng.chance(chance).then(|| {
                if rng.chance(GLASS_EYE_CHANCE) {
                    "art_glasseye"
                } else {
                    kind.artifact()
                }
            });
            out.push(AnomalySpec {
                kind,
                at: p,
                radius,
                artifact,
            });
            chances.push(chance);
            return;
        }
    };
    // The standing anomalies first, where the map puts them, so the Surge's
    // keep clear of them. Their own stream: adding one to a map leaves the
    // Surge's draws as they were.
    let mut srng = Rng::new(seed ^ 0x57A4D ^ (cycle as u64).wrapping_mul(0xD1B5_4A32_D192_ED03));
    for m in &map.markers {
        let MarkerKind::Anomaly { kind, radius } = &m.kind else {
            continue;
        };
        let Some(kind) = AnomalyKind::from_name(kind) else {
            continue;
        };
        let artifact = srng.chance(FIELD_ARTIFACT_CHANCE).then(|| kind.artifact());
        out.push(AnomalySpec {
            kind,
            at: m.at,
            radius: *radius,
            artifact,
        });
        chances.push(0.0);
    }
    let standing = out.len();
    for m in map.markers_of(&MarkerKind::ArtifactField) {
        let n = 5 + rng.below(4);
        for _ in 0..n {
            place(
                &mut rng,
                &mut out,
                &mut chances,
                m.at,
                28.0,
                FIELD_ARTIFACT_CHANCE,
            );
        }
    }
    let half = map.size * 0.5 - map.fence_inset - 10.0;
    for _ in 0..(4 + rng.below(3)) {
        let c = [rng.range(-half, half), rng.range(-half, half)];
        place(
            &mut rng,
            &mut out,
            &mut chances,
            c,
            0.0,
            LOOSE_ARTIFACT_CHANCE,
        );
    }
    (out, chances, standing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MARROW_CREEK;

    fn marrow() -> (MapDef, Heightmap) {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let hm = Heightmap::build(&map);
        (map, hm)
    }

    #[test]
    fn every_kind_has_a_clear_view_from_20_m_in_marrow() {
        let (map, hm) = marrow();
        let all = layout(&map, &hm, 1, 0);
        let blockers = sight_blockers(&map, &hm);
        // Every kind the valley has (a frost is Rime Fell's own).
        for kind in AnomalyKind::ALL {
            if !all.iter().any(|a| a.kind == kind) {
                continue;
            }
            let view = all
                .iter()
                .filter(|a| a.kind == kind)
                .find_map(|a| view_of(&map, &hm, a, &all, &blockers, 20.0).map(|v| (a, v)));
            let (a, v) = view.unwrap_or_else(|| panic!("no clear view of a {kind:?}"));
            assert!((geom::dist(a.at, v.at) - 20.0).abs() < 0.01);
            // It looks at it.
            let (s, c) = v.yaw.to_radians().sin_cos();
            let ahead = [v.at[0] + s * 20.0, v.at[1] - c * 20.0];
            assert!(geom::dist(ahead, a.at) < 0.05, "{kind:?}: looks away");
        }
    }

    #[test]
    fn a_frost_stiffens_you_to_a_crawl_and_a_hearth_keeps_it_off() {
        let dt = 1.0 / 90.0;
        let soak = |hearth: bool, secs: f32| {
            let mut c = 0.0;
            for _ in 0..(secs / dt) as usize {
                c = chill_step(c, frost_rate(0.0, 3.0), hearth, false, dt);
            }
            c
        };
        let three = soak(false, 3.0);
        assert!(
            three > 0.95,
            "three seconds at its heart and you're stiff ({three})"
        );
        assert!(frost_pace(1.0) > 0.1, "but you can always crawl");
        assert!(
            soak(true, 60.0) <= FROST_HEARTH_CAP + 1e-4,
            "a hearth: never past its cap"
        );
        assert!(frost_pace(FROST_HEARTH_CAP) > 0.7);
        assert!(
            frost_stiffness(0.9, true) < 0.3,
            "chilled by the weather, a hearth still keeps you moving"
        );
        assert_eq!(frost_rate(3.1, 3.0), 0.0, "nothing outside it");
        // Out of it the cold is the valley's to warm off; a rimestone keeps
        // a little on you whatever.
        assert_eq!(chill_step(0.8, 0.0, false, false, dt), 0.8);
        assert_eq!(chill_step(0.0, 0.0, false, true, dt), RIMESTONE_CHILL);
    }

    #[test]
    fn layout_is_deterministic_and_changes_with_the_cycle() {
        let (map, hm) = marrow();
        let a = layout(&map, &hm, 1, 0);
        let b = layout(&map, &hm, 1, 0);
        let c = layout(&map, &hm, 1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(
            a.len() >= 10,
            "fields are populated ({} anomalies)",
            a.len()
        );
    }

    #[test]
    fn between_great_scatters_the_surge_s_anomalies_creep_downhill() {
        let (map, hm) = marrow();
        let standing = map
            .markers
            .iter()
            .filter(|m| matches!(m.kind, MarkerKind::Anomaly { .. }))
            .count();
        let first = layout(&map, &hm, 3, 0);
        let mut prev = first.clone();
        for cycle in 1..SURGE_EPOCH {
            let now = layout(&map, &hm, 3, cycle);
            assert_eq!(now.len(), prev.len(), "cycle {cycle}: the same anomalies");
            for (i, (a, b)) in prev.iter().zip(&now).enumerate() {
                assert_eq!((a.kind, a.radius), (b.kind, b.radius));
                let d = geom::dist(a.at, b.at);
                if i < standing {
                    assert_eq!(a.at, b.at, "standing anomalies stay put");
                } else {
                    assert!(d <= MIGRATE_STEP + 0.01, "{a:?} jumped {d} m");
                }
            }
            prev = now;
        }
        let surge = standing..first.len();
        let moved = surge
            .clone()
            .filter(|&i| geom::dist(first[i].at, prev[i].at) > 2.0)
            .count();
        let fell: f32 = surge
            .clone()
            .map(|i| hm.height(first[i].at) - hm.height(prev[i].at))
            .sum::<f32>()
            / surge.len() as f32;
        assert!(
            moved * 2 > surge.len(),
            "most of the Surge's have moved ({moved} of {})",
            surge.len()
        );
        assert!(
            fell > 1.0,
            "and they've gone downhill ({fell:.1} m on average)"
        );
        let fresh = layout(&map, &hm, 3, SURGE_EPOCH);
        let kept = surge
            .filter(|&i| {
                fresh
                    .get(i)
                    .is_some_and(|f| geom::dist(f.at, prev[i].at) < 3.0)
            })
            .count();
        assert!(kept < 3, "a great scatter starts over ({kept} stayed)");
    }

    #[test]
    fn anomalies_keep_off_roads_water_and_each_other() {
        let (map, hm) = marrow();
        for cycle in 0..SURGE_EPOCH * 2 + 1 {
            let all = layout(&map, &hm, 7, cycle);
            for (i, a) in all.iter().enumerate() {
                assert!(placeable(&map, &hm, a.at), "{a:?}");
                assert!(clear_of_roads(&hm, a.at, a.radius), "{a:?} reaches a road");
                for b in &all[i + 1..] {
                    assert!(
                        geom::dist(a.at, b.at) > a.radius + b.radius,
                        "{a:?} overlaps {b:?}"
                    );
                }
            }
        }
    }

    /// Flies a round straight past a well's heart at `miss` metres and
    /// returns how far it turned, degrees.
    fn deflection(speed: f32, miss: f32, radius: f32) -> f32 {
        let dt = 1.0 / 2000.0;
        let mut p = [-40.0f32, miss, 0.0];
        let mut v = [speed, 0.0, 0.0];
        while p[0] < 40.0 {
            let a = well_bend(p, radius);
            for i in 0..3 {
                v[i] += a[i] * dt;
                p[i] += v[i] * dt;
            }
        }
        v[1].atan2(v[0]).abs().to_degrees()
    }

    #[test]
    fn a_well_bends_close_shots_and_slow_ones_most() {
        let r = 2.0;
        let rifle_close = deflection(700.0, 1.0, r);
        let rifle_edge = deflection(700.0, r, r);
        let pistol_close = deflection(350.0, 1.0, r);
        let rifle_far = deflection(700.0, r * WELL_REACH + 0.1, r);
        assert!(
            (2.0..12.0).contains(&rifle_close),
            "a rifle round a metre off turns a few degrees ({rifle_close:.2})"
        );
        assert!(
            rifle_edge < rifle_close && rifle_edge > 0.5,
            "{rifle_edge:.2}"
        );
        assert!(
            pistol_close > rifle_close * 3.0,
            "half the speed, about four times the turn ({pistol_close:.2} vs {rifle_close:.2})"
        );
        assert!(rifle_far < 1e-3, "out of reach, untouched ({rifle_far})");
    }

    #[test]
    fn a_tick_of_lens_matches_flying_it_finely() {
        // The game's fixed tick: a rifle round crosses the well in about a
        // tick, and the summed impulse turns it as the fine flight does.
        let (r, speed, miss) = (2.0, 700.0, 1.0);
        let dt = 1.0 / 64.0;
        let mut p = [-40.0f32, miss, 0.0];
        let mut v = [speed, 0.0, 0.0];
        while p[0] < 40.0 {
            let step = [v[0] * dt, v[1] * dt, v[2] * dt];
            if well_in_reach(p, step, r) {
                let dv = well_impulse(p, v, dt, r);
                for i in 0..3 {
                    v[i] += dv[i];
                }
            }
            for i in 0..3 {
                p[i] += v[i] * dt;
            }
        }
        let coarse = v[1].atan2(v[0]).abs().to_degrees();
        let fine = deflection(speed, miss, r);
        assert!(
            (coarse - fine).abs() < fine * 0.25,
            "a tick at a time ({coarse:.2}°) as finely ({fine:.2}°)"
        );
        assert!(!well_in_reach([-40.0, 7.0, 0.0], [80.0, 0.0, 0.0], r));
        assert!(well_in_reach([-40.0, 1.0, 0.0], [80.0, 0.0, 0.0], r));
    }

    #[test]
    fn a_well_drags_things_in_and_lets_go_past_its_reach() {
        let r = 2.0;
        let at = |d: f32| well_pull([d, 0.0, 0.0], r, WELL_PULL)[0];
        assert!(
            at(r) < -WELL_PULL * 0.3,
            "pulled in at the edge ({})",
            at(r)
        );
        assert!(at(r * 0.5) < at(r), "harder nearer the heart");
        assert!(at(r * 2.0) < 0.0 && at(r * 2.0) > at(r));
        assert_eq!(at(r * WELL_REACH), 0.0);
        // A walker at the edge can still walk out (about 1.5 m/s² of
        // headroom against a walker's few m/s² of push), not at the heart.
        let walker = |d: f32| -well_pull([d, 0.0, 0.0], r, WELL_DRAG)[0];
        assert!(walker(r * 2.0) < 2.0, "{}", walker(r * 2.0));
        assert!(walker(r * WELL_HEART) > 4.0, "{}", walker(r * WELL_HEART));
    }

    #[test]
    fn a_mirror_blinks_you_out_the_far_side() {
        let c = [10.0, 5.0];
        let exit = mirror_exit(c, [8.0, 5.0], 1.5, |_| true).unwrap();
        assert!(
            (exit[0] - (10.0 + 1.5 + MIRROR_THROW)).abs() < 1e-4,
            "{exit:?}"
        );
        assert!((exit[1] - 5.0).abs() < 1e-4);
        // Water straight through: it swings round to open ground.
        let dry = |p: [f32; 2]| p[1] > 5.5;
        let e = mirror_exit(c, [8.0, 5.0], 1.5, dry).unwrap();
        assert!(
            e[1] > 5.5 && geom::dist(e, c) > 1.5 + MIRROR_THROW - 1e-3,
            "{e:?}"
        );
        assert!(mirror_exit(c, [8.0, 5.0], 1.5, |_| false).is_none());
    }

    #[test]
    fn the_surge_stirs_them_up_and_lets_them_go() {
        let (w, l) = (300.0, 120.0);
        assert_eq!(surge_agitation(400.0, w, l), 0.0);
        assert_eq!(surge_agitation(w, w, l), 0.0);
        let early = surge_agitation(200.0, w, l);
        let late = surge_agitation(30.0, w, l);
        assert!(early > 0.0 && late > early && late < 1.0, "{early} {late}");
        assert_eq!(surge_agitation(-10.0, w, l), 1.0);
        assert_eq!(surge_agitation(-l - 1.0, w, l), 0.0);
        // Every bloom falls due once over the warning, ticking through it.
        let mut n = 0;
        let mut t = w + 5.0;
        while t > -5.0 {
            n += surge_blooms_due(t, t - 1.0 / 64.0);
            t -= 1.0 / 64.0;
        }
        assert_eq!(n, SURGE_BLOOMS.len());
    }

    #[test]
    fn surge_blooms_open_to_the_side_not_in_your_way() {
        let mut rng = Rng::new(9);
        for _ in 0..200 {
            let p = surge_bloom_spot([0.0, 0.0], [0.0, -2.0], || rng.f32(), |_| true).unwrap();
            let d = geom::len(p);
            assert!((10.0..=25.0).contains(&d), "{d}");
            // Heading -Z: the angle off it is 50..130 degrees.
            let off = (-p[1] / d).acos().to_degrees();
            assert!(
                (49.0..=131.0).contains(&off),
                "{p:?} is {off:.0} degrees off"
            );
        }
        assert!(surge_bloom_spot([0.0, 0.0], [0.0, 0.0], || 0.5, |_| false).is_none());
    }

    #[test]
    fn blooms_never_open_by_a_building_or_a_road_in_any_valley() {
        for src in crate::map::VALLEYS.map(|v| v.1) {
            let map = MapDef::from_ron(src).unwrap();
            let hm = Heightmap::build(&map);
            let mut rng = Rng::new(3);
            let half = map.size * 0.5 - map.fence_inset - 30.0;
            let mut n = 0;
            for i in 0..400 {
                let at = [rng.range(-half, half), rng.range(-half, half)];
                let kind = AnomalyKind::ALL[i % AnomalyKind::ALL.len()];
                let r = kind.radius_range().1;
                let heading = [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)];
                let Some(p) = surge_bloom_spot(
                    at,
                    heading,
                    || rng.f32(),
                    |p| bloom_ok(&map, &hm, p, kind, r),
                ) else {
                    continue;
                };
                n += 1;
                for pl in &map.placements {
                    let f = crate::map::prefab_footprint(&pl.prefab);
                    assert!(
                        geom::dist(pl.at, p) >= f[0].max(f[2]) * 0.6 + 2.0,
                        "{}: a {kind:?} bloom at {p:?} is inside {}'s clearance",
                        map.name,
                        pl.prefab
                    );
                }
                assert!(
                    clear_of_roads(&hm, p, kind.danger_radius(r)),
                    "{}: {p:?} by a road",
                    map.name
                );
                // Nor against a wall, a kiosk or in front of a door.
                let (room, what) = crate::map_surge::anomaly_room(&map, p);
                assert!(
                    room >= kind.danger_radius(r),
                    "{}: a {kind:?} bloom at {p:?} reaches {what}",
                    map.name
                );
            }
            assert!(n > 100, "{}: blooms found ground ({n})", map.name);
        }
    }

    #[test]
    fn artifacts_cost_what_they_should() {
        // A featherstone wakes a lift it's near, faster than an empty
        // lift settles (1/6 a second), but not from across a field.
        assert!(feather_wake(3.0) > 1.0 / 6.0);
        assert!(feather_wake(12.0) < 1.0 / 6.0);
        assert_eq!(feather_wake(FEATHER_WAKE_RANGE), 0.0);
        // An aegis bites wet, not dry.
        assert!(aegis_wet(0.8, 0.0) && aegis_wet(0.0, 0.3));
        assert!(!aegis_wet(0.1, 0.0));
        assert!((8.0..=15.0).contains(&aegis_shock_in(0.0)));
        assert!((8.0..=15.0).contains(&aegis_shock_in(1.0)));
        // A hearth halves recovery: still positive at rest.
        assert!(1.0 / 8.0 - HEARTH_TIRE > 0.0);
        assert!((20.0..=40.0).contains(&shard_glint_in(0.5)));
    }

    #[test]
    fn a_stone_from_the_coals_cools_in_time_or_at_once_in_water() {
        assert_eq!(heat_left(0.0, false), 1.0);
        assert!(heat_left(HOT_SECS * 0.5, false) > 0.4);
        assert_eq!(heat_left(HOT_SECS, false), 0.0);
        assert_eq!(heat_left(0.0, true), 0.0);
    }

    #[test]
    fn the_detector_ladder_reads_what_it_should() {
        assert_eq!(DetectorTier::of("detector"), Some(DetectorTier::Clicker));
        assert_eq!(DetectorTier::of("detector_bear"), Some(DetectorTier::Bear));
        assert_eq!(DetectorTier::of("bolt"), None);
        // A sweep hears ahead, not to the side.
        assert!(in_cone([0.0, 0.0, -1.0], [1.0, 0.0, -5.0], SWEEP_CONE_DEG));
        assert!(!in_cone([0.0, 0.0, -1.0], [5.0, 0.0, -1.0], SWEEP_CONE_DEG));
        assert!(stone_ping_interval(2.0).unwrap() < stone_ping_interval(15.0).unwrap());
        assert_eq!(stone_ping_interval(STONE_RANGE), None);
        // Facing -Z: a stone 10 m ahead is up the screen, one to the east
        // is to the right; turned to face east, the east one is ahead.
        let ahead = bear_blip([0.0, -10.0], 0.0, 20.0).unwrap();
        assert!(
            ahead[0].abs() < 1e-5 && (ahead[1] - 0.5).abs() < 1e-5,
            "{ahead:?}"
        );
        let east = bear_blip([10.0, 0.0], 0.0, 20.0).unwrap();
        assert!(
            (east[0] - 0.5).abs() < 1e-5 && east[1].abs() < 1e-5,
            "{east:?}"
        );
        let turned = bear_blip([10.0, 0.0], -std::f32::consts::FRAC_PI_2, 20.0).unwrap();
        assert!((turned[1] - 0.5).abs() < 1e-4, "{turned:?}");
        assert!(bear_blip([30.0, 0.0], 0.0, 20.0).is_none());
    }

    #[test]
    fn a_hush_eats_what_is_made_in_it_and_presses_on_who_stays() {
        let h = [([0.0, 1.0, 0.0], 3.0)];
        assert!(hushed([1.0, 1.5, 1.0], &h));
        assert!(!hushed([4.0, 1.0, 0.0], &h));
        assert!(!hushed([0.0, 1.0, 0.0], &[]));
        assert_eq!(hush_press(HUSH_GRACE - 1.0), 0.0);
        assert_eq!(hush_press(HUSH_GRACE + 1.0), HUSH_PRESS);
        assert_eq!(AnomalyKind::from_name("Hush"), Some(AnomalyKind::Hush));
    }

    #[test]
    fn the_detector_clicks_faster_as_you_close() {
        assert_eq!(detector_interval(DETECTOR_RANGE + 1.0), None);
        let far = detector_interval(20.0).unwrap();
        let mid = detector_interval(8.0).unwrap();
        let edge = detector_interval(0.0).unwrap();
        let inside = detector_interval(-2.0).unwrap();
        assert!(far > mid && mid > edge, "{far} {mid} {edge}");
        assert!(
            far > 0.8 && edge < 0.1,
            "a lazy tick far off, a rattle at the edge"
        );
        assert_eq!(edge, inside);
    }

    #[test]
    fn standing_anomalies_lie_where_the_map_puts_them_every_cycle() {
        let (mut map, hm) = marrow();
        map.markers
            .retain(|m| !matches!(m.kind, MarkerKind::Anomaly { .. }));
        let before = layout(&map, &hm, 4, 0);
        map.markers.push(crate::map::Marker {
            kind: MarkerKind::Anomaly {
                kind: "Well".into(),
                radius: 2.0,
            },
            at: [1000.0, 1000.0],
            rot: 0.0,
            name: "test well".into(),
            y: 0.0,
        });
        for cycle in 0..4 {
            let all = layout(&map, &hm, 4, cycle);
            let wells: Vec<_> = all.iter().filter(|a| a.kind == AnomalyKind::Well).collect();
            assert_eq!(wells.len(), 1);
            assert_eq!(wells[0].at, [1000.0, 1000.0]);
        }
        // Out of everyone's way, it leaves the Surge's own layout alone.
        let after = layout(&map, &hm, 4, 0);
        assert_eq!(&after[1..], &before[..]);
        assert!(AnomalyKind::from_name("Mirror").is_some());
        assert!(AnomalyKind::from_name("Teapot").is_none());
    }

    #[test]
    fn every_valley_has_a_well_and_a_mirror_somewhere_sensible() {
        for src in crate::map::VALLEYS.map(|v| v.1) {
            let map = MapDef::from_ron(src).unwrap();
            let hm = Heightmap::build(&map);
            let mut kinds = Vec::new();
            for m in &map.markers {
                let MarkerKind::Anomaly { kind, radius } = &m.kind else {
                    continue;
                };
                let kind = AnomalyKind::from_name(kind)
                    .unwrap_or_else(|| panic!("{}: no anomaly kind {kind:?}", map.name));
                let (lo, hi) = kind.radius_range();
                assert!(
                    (lo..=hi).contains(radius),
                    "{}: {} r{radius}",
                    map.name,
                    m.name
                );
                assert!(
                    placeable(&map, &hm, m.at),
                    "{}: {} is in the way",
                    map.name,
                    m.name
                );
                assert!(
                    clear_of_roads(&hm, m.at, kind.danger_radius(*radius)),
                    "{}: {} reaches a road",
                    map.name,
                    m.name
                );
                kinds.push(kind);
            }
            for k in [AnomalyKind::Well, AnomalyKind::Mirror, AnomalyKind::Hush] {
                assert!(kinds.contains(&k), "{} has no {k:?}", map.name);
            }
        }
    }

    #[test]
    fn some_cycles_grow_artifacts() {
        let (map, hm) = marrow();
        let n: usize = (0..10)
            .map(|c| {
                layout(&map, &hm, 3, c)
                    .iter()
                    .filter(|a| a.artifact.is_some())
                    .count()
            })
            .sum();
        assert!(
            n >= 10,
            "about a handful of artifacts per cycle (got {n} over 10)"
        );
    }
}
#[cfg(test)]
mod print_layout {
    #[test]
    #[ignore]
    fn print() {
        let map = crate::map::MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let hm = crate::terrain::Heightmap::build(&map);
        for a in super::layout(&map, &hm, 1, 0) {
            eprintln!(
                "{:?} {:.1?} r{:.1} {:?} h{:.1}",
                a.kind,
                a.at,
                a.radius,
                a.artifact,
                hm.height(a.at)
            );
        }
    }
}
