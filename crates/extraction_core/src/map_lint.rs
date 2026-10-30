//! The map lint: the measurable rules of `docs/MAP_DESIGN.md`, checked
//! against a map as built (terrain settled, trees and rocks scattered, the
//! walkers' navigation grid laid).
//!
//! `lint` returns every finding; the tests hold the three valleys to zero
//! and print the Training Yard's (it belongs to onboarding). Each rule's
//! thresholds are the constants just above it, named as the design guide
//! names them.
//!
//! `cargo test -p extraction_core map_lint -- --nocapture` prints each
//! valley's measurements and findings; `LINT_DUMP=dir` also writes each
//! valley's main routes as `<dir>/<map>_routes.json`, which
//! `scripts/map_preview.py --paths` draws.

use crate::geom::{self, P2, dist, polyline_closest};
use crate::map::{MapDef, MarkerKind, RoadKind, prefab_footprint, prefab_wants_pad};
use crate::nav::{BLOCKED, NavGrid};
use crate::terrain::Heightmap;
use std::fmt::Write as _;

/// Which rule of the guide a finding breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// Something you're meant to get to can't be walked to.
    Reach,
    /// It can only be reached by climbing ground steeper than is
    /// comfortable to walk up in a headset.
    Comfort,
    /// A main route or road climbs too steeply.
    Grade,
    /// Points of interest crowd each other or leave a gap.
    Spacing,
    /// A long stretch of a main route with nothing to find near it.
    Pacing,
    /// Nowhere to steer by: no tall landmark in sight.
    Landmark,
    /// A long stretch of a main route out in the open.
    Cover,
    /// Loot that doesn't rise with danger.
    LootRisk,
    /// Something in the water or on a cliff.
    Footing,
    /// Buildings in each other or on a road, or a squeeze between them.
    Overlap,
    /// Anomalies on the roads or the main routes.
    Anomaly,
    /// Robots can't get from their Gate to their work, or work too close
    /// to the truck.
    Patrol,
    /// Only one sensible way to a rendezvous.
    Choice,
    /// Too many buildings in view at once.
    ViewLoad,
    /// A long walk with no roof to duck under.
    Rhythm,
    /// A passage narrower than a body with a rifle.
    Narrow,
    /// A supply crate (a loot marker in the open) in a wall or where the
    /// truck appears.
    Crate,
    /// A point of interest no road or path leads to.
    Funnel,
    /// A sentry that's a wall, not a puzzle: blind, guarding nothing, no
    /// way round, over the way in or out, or met without warning
    /// (`map_sentry`).
    Sentry,
    /// A spotter that ends a raid rather than shaping it: its winch where
    /// you can't stand to cut it, watching nothing, no way in under it
    /// while its eye is turned away, watching the way in or out, or seeing
    /// most of a sentry's field (`map_spotter`).
    Spotter,
    /// The minelayer's fields as the game lays them: a mine on a main
    /// route or in a doorway, or a site with room for under half a field
    /// (`map_mines`).
    Mines,
    /// Caught out by the Surge: no roof in reach once it's called, or one
    /// of its anomalies against a wall or a door (`map_surge`).
    Surge,
    /// Ground the walkers' grid reaches only over a bank a body can't walk
    /// (`map_pockets`).
    Pocket,
    /// A door with something in front of it: a building, a trunk, a rock,
    /// a crate, water, or ground that doesn't meet its steps
    /// (`map_doors`).
    Door,
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: Rule,
    pub what: String,
    pub at: P2,
}

/// A walk from the insertion to a rendezvous, as a player who keeps to
/// comfortable ground would take it, sampled every [`STEP`] metres.
#[derive(Clone, Debug)]
pub struct Route {
    pub to: String,
    pub points: Vec<P2>,
    pub length: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub map: String,
    pub findings: Vec<Finding>,
    /// What was measured, one line each, for the record.
    pub measures: Vec<String>,
    pub routes: Vec<Route>,
}

impl Report {
    pub fn count(&self, rule: Rule) -> usize {
        self.findings.iter().filter(|f| f.rule == rule).count()
    }

    /// The report as text: the measurements, then every finding.
    pub fn text(&self) -> String {
        let mut s = format!("== {} ==\n", self.map);
        for m in &self.measures {
            let _ = writeln!(s, "  {m}");
        }
        for f in &self.findings {
            let _ = writeln!(
                s,
                "  {:?}: {} at ({:.0}, {:.0})",
                f.rule, f.what, f.at[0], f.at[1]
            );
        }
        if self.findings.is_empty() {
            s.push_str("  clean\n");
        }
        s
    }

    /// The findings as JSON for `map_preview.py --lint`.
    pub fn findings_json(&self) -> String {
        let esc = |t: &str| {
            t.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', " ")
        };
        let items: Vec<String> = self
            .findings
            .iter()
            .map(|f| {
                format!(
                    "{{\"rule\":\"{:?}\",\"what\":\"{}\",\"at\":[{:.1},{:.1}]}}",
                    f.rule,
                    esc(&f.what),
                    f.at[0],
                    f.at[1]
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// The main routes as JSON for `map_preview.py --paths`.
    pub fn routes_json(&self) -> String {
        let colors = ["#40e0ff", "#ffe040", "#ff60c0", "#80ff80"];
        let mut s = String::from("[");
        for (i, r) in self.routes.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let pts: Vec<String> = r
                .points
                .iter()
                .map(|p| format!("[{:.1},{:.1}]", p[0], p[1]))
                .collect();
            let _ = write!(
                s,
                "{{\"color\":\"{}\",\"points\":[{}]}}",
                colors[i % colors.len()],
                pts.join(",")
            );
        }
        s.push(']');
        s
    }
}

/// Route sampling step, metres.
pub const STEP: f32 = 2.0;

// ---- Comfort -------------------------------------------------------------

/// The steepest ground a player should have to walk up to reach anything
/// that matters, degrees. The walkers path up to `map_nav::MAX_SLOPE`;
/// the body controller climbs steeper still, but a headset turns a 30°
/// scramble into a queasy one.
pub const COMFORT_SLOPE: f32 = 25.0;
/// The steepest a main route may climb, averaged over [`GRADE_RUN`]
/// metres, degrees.
pub const ROUTE_GRADE: f32 = 16.0;
pub const GRADE_RUN: f32 = 10.0;
/// The steepest a road may climb along its centreline over [`GRADE_RUN`]
/// metres, as a percentage: asphalt and gravel, then dirt tracks and
/// footpaths (rail beds are held to [`RAIL_GRADE`]: steep for a main
/// line, ordinary for a narrow-gauge mine line).
pub const ROAD_GRADE: f32 = 14.0;
pub const TRACK_GRADE: f32 = 22.0;
pub const RAIL_GRADE: f32 = 6.0;
/// Past a road's end, walking on this far, no rise or fall steeper than
/// `terrain::ROAD_END_LEDGE` in any half metre: a graded road cut into a
/// slope otherwise ends at a bank. (The builder blends ends into the land,
/// `terrain::ROAD_END_APRON`; this holds it.)
pub const END_WALK: f32 = 8.0;
/// Road grades count from this far inside the fence.
pub const FENCE_MARGIN: f32 = 12.0;
/// Narrowest footpath, and narrowest other road, metres.
pub const PATH_WIDTH: f32 = 1.4;
pub const ROAD_WIDTH: f32 = 3.0;
/// Two buildings either touch or leave at least this much room between
/// them: anything in between is a squeeze you'd turn sideways for.
pub const SQUEEZE: f32 = 1.5;
/// A gap narrower than this nobody fits into at all (a body is 0.56 m
/// across), so it isn't a passage.
pub const SEALED: f32 = 0.6;

// ---- Places ----------------------------------------------------------------

/// The closest two points of interest may be, and the farthest any may be
/// from its nearest neighbour, metres.
pub const POI_MIN: f32 = 60.0;
pub const POI_MAX: f32 = 190.0;
/// Every point of interest has a road or path within this distance,
/// unless it's a hazard you approach across country (an artifact field
/// within [`HAZARD_NEAR`] of it).
pub const FUNNEL: f32 = 30.0;
pub const HAZARD_NEAR: f32 = 40.0;
/// A thing to find counts as on a route within this distance of it.
pub const NEAR_ROUTE: f32 = 60.0;
/// Longest stretch of a main route with nothing to find near it (about a
/// minute and a half at walking pace), metres.
pub const PACING_GAP: f32 = 220.0;
/// Longest stretch of a main route with no building to duck into near it.
pub const RHYTHM_GAP: f32 = 320.0;
/// Prefabs you can walk into.
pub const ENTERABLE: &[&str] = &[
    "house_plank",
    "house_brick",
    "farmhouse",
    "cabin_log",
    "shop",
    "shed",
    "barn",
    "sawmill",
    "pump_station",
    "bus_depot",
    "rail_shed",
    "mast_hut",
    "ranger_tower",
    "survey_tower",
    "factory_hall",
    "guard_booth",
    "ore_mill",
    "observatory",
    "funicular_lower",
    "funicular_upper",
];

// ---- Orientation -----------------------------------------------------------

/// How tall a structure must be to steer by (over the tree tops), metres.
pub const LANDMARK_HEIGHT: f32 = 12.0;
/// How far you can make one out in thick fog (the pea-souper's
/// visibility), metres.
pub const FOG_SIGHT: f32 = 150.0;
/// How far in ordinary weather (drizzle). Legibility's wayfinding
/// renders (`scripts/wayfind.sh`, docs/LEGIBILITY.md "Wayfinding") put
/// every landmark in front of the headset from the insertion and each
/// rendezvous: past about 200 m in drizzle a structure keeps 3-7% of its
/// contrast, which is nothing to steer by.
pub const CLEAR_SIGHT: f32 = 200.0;
/// Two landmarks of a kind closer than this are twins you can't tell
/// apart, metres. Wider than [`CLEAR_SIGHT`]: one that's barely there
/// past it still reads as "the mast" and sends you the wrong way.
pub const UNIQUE_SIGHT: f32 = 320.0;
/// Share of each main route from which a landmark is in sight in ordinary
/// weather, and the longest blind stretch.
pub const LANDMARK_SHARE: f32 = 0.7;
pub const BLIND_GAP: f32 = 200.0;
/// Landmarks that come in a line (a ropeway's pylons): many of one kind in
/// sight read as one thing to follow, not as two you can't tell apart.
pub const SERIES: &[&str] = &["ropeway_pylon"];
/// Each region has something of its own at least this tall, or a tall
/// landmark within [`FOG_SIGHT`] of its middle.
pub const REGION_MARK: f32 = 6.0;
/// From this share of the walkable valley two landmarks you can tell apart
/// are in sight within [`CLEAR_SIGHT`], over the land and through the
/// trees (sampled every `TWO_MARKS_STEP`; the blind cells are reported by
/// `POCKET` square).
pub const TWO_MARKS_SHARE: f32 = 0.9;
pub const TWO_MARKS_STEP: f32 = 8.0;
/// A rendezvous's beacon is in sight from this share of the last
/// [`BEACON_APPROACH`] of the main route in to it (measured too: the ground
/// within [`BEACON_SIGHT`]).
pub const BEACON_SHARE: f32 = 0.7;
/// A landmark this near a rendezvous marks it as its beacon does.
pub const RV_MARK: f32 = 60.0;
pub const BEACON_APPROACH: f32 = 300.0;
pub const BEACON_SIGHT: f32 = 250.0;
/// Further into a wood than this from its edge is deep in it.
pub const WOOD_EDGE: f32 = 25.0;
/// How far a landmark's lamp and a rendezvous beacon show after dark (the
/// game's halo reaches: `render::landmarks`, `render::rv_beacons`).
pub const LAMP_REACH: f32 = 320.0;
pub const BEACON_REACH: f32 = 400.0;
/// After dark, from this share of the walkable valley (out of the deep
/// woods) a lamp is in sight, and two of different characters (lamps you
/// can name: a fix) from this...
pub const NIGHT_ONE: f32 = 0.95;
pub const NIGHT_TWO: f32 = 0.9;
/// ...and in fog a landmark or lamp within [`FOG_SIGHT`] from this.
pub const FOG_ONE: f32 = 0.9;
const POCKET: f32 = 40.0;
/// From every rendezvous a landmark is in sight within this, metres, so
/// "head for the mast" is a plan.
pub const RV_LANDMARK: f32 = 200.0;

// ---- Cover -----------------------------------------------------------------

/// Cover counts within this distance of where you stand, metres: a dash
/// of five seconds or so when a drone chirps its lock.
pub const COVER_REACH: f32 = 25.0;
/// Longest stretch of a main route with no cover within reach: a few
/// seconds' run (the four valleys before Sallow keep theirs to 20 m).
pub const COVER_GAP: f32 = 30.0;
/// Open stretches this long or longer are listed by where they start.
const COVER_NOTE: f32 = 20.0;
/// Anywhere walkable in the valley, cover is within this, and no more than
/// [`OPEN_SHARE`] of it is further than [`COVER_REACH`] from any: open ground
/// is where drones kill. (Measured on a grid of `OPEN_STEP`, by the
/// distances in `OPEN_LADDER`.)
pub const OPEN_FAR: f32 = 70.0;
pub const OPEN_SHARE: f32 = 0.03;
const OPEN_STEP: f32 = 4.0;
const OPEN_LADDER: [f32; 6] = [30.0, 35.0, 40.0, 50.0, 70.0, 100.0];
/// The first hour's walks: from the insertion to every point of interest
/// this near it (the first jobs' places, the village by the gate).
pub const FIRST_WALK: f32 = 300.0;
/// On those, the longest stretch with no cover within reach: a newcomer
/// warned by the Driver has about four seconds, and runs for the nearest
/// thing between them and the machine (Onboarding's first-fight personas
/// died in Marrow's open fields on the way to the Old farm).
pub const FIRST_COVER_GAP: f32 = 40.0;
/// The first job of each kind (`first_jobs`): every way there (the easy
/// way, straight across, by any place on the way) has cover within a
/// four-second sprint (`map_surge::SPRINT`) of every step, but for a run
/// of this many metres: three of Onboarding's four newcomers on Marrow's
/// mast relay died to a drone 7-17 s after its lock, in the open past the
/// Village.
pub const FIRST_RUN: f32 = 4.0 * crate::map_surge::SPRINT;
pub const FIRST_OPEN: f32 = 10.0;
/// A way by a place counts as on the way within this of the direct walk.
pub const FIRST_DETOUR: f32 = 1.3;
/// What `first_jobs` calls the career's first job.
pub const FIRST_OF_ALL: &str = "the career's first";
/// The career's first job is the obvious way to go from the drop: the easy
/// walk there heads for it from the start, within this many degrees of
/// its bearing for its first [`FIRST_HEAD`] metres (the designer's first
/// raid followed the road north past the Old farm's turn, which a spruce
/// stand hid)...
pub const FIRST_TURN: f32 = 40.0;
pub const FIRST_HEAD: f32 = 120.0;
/// ...and nothing with loot near the drop pulls you off it: no roof to
/// go into, kiosk or loot within [`LURE_REACH`] (what you look round at as
/// the truck goes) of the insertion further than [`LURE_OFF`] from that
/// walk (a shed 49 m up the road to the Village, and the cars by it, took
/// the designer's first ten minutes).
pub const LURE_REACH: f32 = 60.0;
pub const LURE_OFF: f32 = 25.0;
/// The first ten minutes: places a raid can reach and work by then, as a
/// walk on comfortable ground from the insertion, metres (about four
/// minutes' walking, the rest looking).
pub const FIRST_REACH: f32 = 450.0;
/// Two choices are distinct when they lie at least this far apart in
/// bearing from the insertion, degrees...
pub const CHOICE_ANGLE: f32 = 50.0;
/// ...and their walks part: no more than this share of the shorter walk
/// runs within [`CHOICE_APART`] of the other.
pub const CHOICE_SHARED: f32 = 0.5;
/// Something worth going to: a point of interest with loot or a
/// walk-in building this near it.
pub const CHOICE_LOOT: f32 = 40.0;
/// The way out: from every rich place (a point of interest or artifact
/// field in a region of danger [`FIELD_DANGER`] or worse), a second
/// rendezvous is at most this much further than the nearest, metres
/// (about two minutes more at a walk)...
pub const OUT_EXTRA: f32 = 300.0;
/// ...or within this walk of it, whichever is further (the rendezvous lie
/// 350-450 m apart: from beside one, the next is that far)...
pub const OUT_REACH: f32 = 500.0;
/// ...the walks to both have cover ([`COVER_GAP`]) and part early: they
/// are [`CHOICE_APART`] apart within this far along the shorter.
pub const OUT_PART: f32 = 150.0;
/// And a rendezvous's last stretch, where you wait laden for the truck:
pub const LAST_STRETCH: f32 = 100.0;
/// in it, no open run longer than this...
pub const LAST_OPEN: f32 = 40.0;
/// ...and no more than this share of it in a spotter's sight.
pub const LAST_SPOTTED: f32 = 0.25;
/// A rendezvous a survey drone sees while it circles a point of interest
/// (its orbit and this much more)...
pub const SURVEY_SIGHT: f32 = 90.0;
/// ...has something to wait behind this near where the truck stops: a
/// roof, or solid cover (anything 1.2 m tall but a garden fence).
pub const WAIT_COVER: f32 = 15.0;
pub const WAIT_HEIGHT: f32 = 1.2;
/// How far off a shelter's wall its lee is, metres.
pub const WAIT_LEE: f32 = 0.8;
/// How far in from its door you wait under a roof, metres.
pub const WAIT_IN: f32 = 1.2;
/// How far settling a building in may lower ground the map declares (a
/// plateau set to a height), metres.
pub const TERRACE_CUT: f32 = 0.5;
/// The truck's stops and rendezvous stand level under it: the ground
/// at its front wheels and at its rear ones (`raid::truck::parked`, the
/// front axle 3.9 m ahead of its middle, the rear 0.8 m behind) falls at
/// most this, radians (3 degrees): steeper, it stands level with its
/// wheels in the soil, or its loose kit slides in the bed.
pub const TRUCK_LEVEL: f32 = 0.05;
/// A rock big enough to crouch behind: horizontal radius and height.
pub const BOULDER_RADIUS: f32 = 0.6;
pub const BOULDER_HEIGHT: f32 = 0.8;
/// Trees this close together (a stand, not a lone tree) count as cover.
pub const STAND: usize = 3;

// ---- Risk ------------------------------------------------------------------

/// Regions this dangerous are at least this far from the insertion.
pub const DANGER_FAR: f32 = 220.0;
/// Artifact fields lie in regions at least this dangerous.
pub const FIELD_DANGER: u8 = 2;

// ---- Robots and anomalies --------------------------------------------------

/// Work sites at least this far from the insertion and rendezvous points.
pub const WORK_FROM_INSERTION: f32 = 150.0;
pub const WORK_FROM_RV: f32 = 40.0;
/// Ground round a work site that must be open and walkable (a pylon and
/// its builders, a pod and its guards), radius and share.
pub const WORK_YARD: f32 = 16.0;
pub const WORK_OPEN: f32 = 0.7;
/// The robot Gate is at least this far from the insertion.
pub const GATE_FROM_INSERTION: f32 = 300.0;
/// A rendezvous keeps this far from the robots' Gate: you wait two minutes
/// there for the truck, and every machine in the valley comes and goes by
/// the Gate. The sims killed raiders waiting at Marrow's Ranger tower,
/// 128 m from it, to whatever came out next.
pub const RV_FROM_GATE: f32 = 160.0;
/// Surge layouts tried when checking anomalies against the routes.
pub const ANOMALY_SEEDS: u64 = 32;
/// An artifact field's middle is at least this far from every main route
/// (its anomalies spread 28 m and reach 4 m more).
pub const FIELD_CLEAR: f32 = 35.0;
/// How far a field's anomalies spread from its middle (`anomaly::layout`).
pub const FIELD_SPREAD: f32 = 28.0;

// ---- Choice ----------------------------------------------------------------

/// The other way to a rendezvous keeps this far from the first (half
/// width of the corridor blocked), except near either end.
pub const CHOICE_APART: f32 = 30.0;
pub const CHOICE_ENDS: f32 = 90.0;
/// And is at most this much longer.
pub const CHOICE_DETOUR: f32 = 1.6;
/// Rendezvous points at least this far apart, and from the insertion.
pub const RV_APART: f32 = 180.0;
pub const RV_FAR: f32 = 350.0;
/// The truck parks on a rendezvous: nothing placed within this of it, and
/// the ground under it no steeper than [`RV_LEVEL`] degrees.
pub const TRUCK_ROOM: f32 = 8.0;
/// A supply crate (a loot marker in the open) stands this far clear of
/// buildings, and this far from a rendezvous.
pub const CRATE_ROOM: f32 = 1.5;
pub const CRATE_FROM_RV: f32 = 10.0;
/// Where you stand to climb into the truck, metres behind its middle.
pub const TAILGATE: f32 = 3.6;
/// Where you line up behind it to walk in, metres behind its middle.
pub const TRUCK_APPROACH: f32 = 8.0;
pub const RV_LEVEL: f32 = 12.0;

// ---- Render load -------------------------------------------------------------

/// Most placements within [`VIEW_RANGE`] of any point on a main route or
/// a point of interest.
pub const VIEW_RANGE: f32 = 150.0;
pub const VIEW_MAX: usize = 16;
/// Light props [`VIEW_MAX`] doesn't count: a garden fence's run is five
/// posts and four plank panels (nine boxes, no roof, no interior), where a
/// house is dozens of pieces and a shadow-casting shell. Counting each run
/// as a building would cost a village its gardens or its houses. They are
/// counted and printed in the render-load measure on their own.
///
/// A car wreck is one too: one body mesh and its box, and its doors,
/// bonnet, boot and wheels as small breakable parts, about a dozen pieces
/// with no roof, no rooms and no loot, under two metres tall (a lorry 2.2).
/// They stand by the roads in ones and twos; counted as buildings, every
/// wreck on a road through a village would cost it a house.
///
/// A log pile is one too: ten log cylinders, no roof, nothing inside.
pub const VIEW_LIGHT: &[&str] = &[
    "fence_garden",
    "wreck_lada",
    "wreck_uaz",
    "wreck_lorry",
    "log_pile",
];

/// Fences, 0.2 m deep: a main route keeps this far off one. The 2 m nav
/// grid blocks only the cells whose middles fall within 0.7 m of a run
/// (its 0.1 m half depth and the grid's 0.6 m clearance), so a route can
/// be drawn between two rows of cells straight through a fence.
pub const FENCES: &[&str] = &["fence_garden"];
pub const FENCE_CLEAR: f32 = 0.5;
/// A fence isn't levelled for (`map::prefab_wants_pad`): it stands on the
/// lowest ground under its run, its posts sunk 0.25 m and its panels 0.15 m
/// clear. The ground may rise this much along it, metres: the far panel's
/// foot goes 0.15 m into the grass, no more.
pub const FENCE_RISE: f32 = 0.3;

/// Car wrecks: cover on the verges (they're placements, so the cover
/// measure counts them), never on a road (`Overlap`; they aren't in
/// `ON_ROADS`). Wider than the nav grid's cell with its clearance, so a
/// main route goes round one, not through it as it could a fence.
pub const WRECKS: &[&str] = &["wreck_lada", "wreck_uaz", "wreck_lorry"];
/// A wreck isn't levelled for (`map::prefab_wants_pad`): it stands level
/// on the lowest of its corners and middle (`Heightmap::footing`). The
/// ground anywhere under it may be this much over or under that foot,
/// metres: a wheel on the high side sunk to its hub (a car's are 0.58 m
/// across), or the low side's off the grass by as much, no more.
pub const WRECK_RISE: f32 = 0.3;

/// Where a kiosk's door is checked for footing: this far out from it.
pub const KIOSK_STEP: f32 = 2.0;

/// A road's first and last metres may run up to a building's door.
const ROAD_END: usize = 8;

// ---- Doors (`map_doors`) -----------------------------------------------------

/// In front of every outside door, this far out from the wall's face (or
/// the door's own steps and standing room, if further) and a shoulder
/// either side of the opening, is kept clear, metres.
pub const DOOR_REACH: f32 = 4.0;
pub const DOOR_SHOULDER: f32 = 0.3;
/// The ground there meets the door's steps within a step: the body's
/// (`player.rs` `STEP_HEIGHT`); the walkers' smaller one is measured.
pub const DOOR_STEP: f32 = 0.4;
pub const WALKER_STEP: f32 = 0.28;
/// A road ending this near a door runs on to it (no ledge check).
pub const DOOR_ROAD_END: f32 = 5.0;

/// Prefabs a road may run into or under.
const ON_ROADS: &[&str] = &[
    "bus_depot",
    "rail_shed",
    "factory_hall",
    "sawmill",
    "barn",
    "quarantine_gate",
    "robot_gate",
    "footbridge",
    "boxcar",
    // Rime Fell's funicular car stands on its own line.
    "funicular_car",
];

/// An oriented rectangle in map space: centre, half extents, yaw (as the
/// walkers' nav grid reads a placement's `rot`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rect {
    pub(crate) at: P2,
    pub(crate) half: P2,
    pub(crate) yaw: f32,
}

impl Rect {
    pub(crate) fn of(prefab: &str, at: P2, rot: f32) -> Self {
        let f = prefab_footprint(prefab);
        Self {
            at,
            half: [f[0] * 0.5, f[2] * 0.5],
            yaw: rot.to_radians(),
        }
    }

    pub(crate) fn local(&self, p: P2) -> P2 {
        let d = geom::sub(p, self.at);
        let (s, c) = self.yaw.sin_cos();
        [d[0] * c + d[1] * s, -d[0] * s + d[1] * c]
    }

    /// Signed distance from `p` to the rectangle (negative inside).
    pub(crate) fn sdf(&self, p: P2) -> f32 {
        let l = self.local(p);
        let q = [l[0].abs() - self.half[0], l[1].abs() - self.half[1]];
        let out = [q[0].max(0.0), q[1].max(0.0)];
        geom::len(out) + q[0].max(q[1]).min(0.0)
    }

    pub(crate) fn corners(&self) -> [P2; 4] {
        let (s, c) = self.yaw.sin_cos();
        let w = |lx: f32, lz: f32| [self.at[0] + lx * c - lz * s, self.at[1] + lx * s + lz * c];
        let [hx, hz] = self.half;
        [w(-hx, -hz), w(hx, -hz), w(hx, hz), w(-hx, hz)]
    }

    /// Points round the outline every `step` metres.
    pub(crate) fn outline(&self, step: f32) -> Vec<P2> {
        let c = self.corners();
        let mut out = Vec::new();
        for k in 0..4 {
            let (a, b) = (c[k], c[(k + 1) % 4]);
            let n = (dist(a, b) / step).ceil().max(1.0) as usize;
            for i in 0..n {
                out.push(geom::lerp(a, b, i as f32 / n as f32));
            }
        }
        out
    }

    /// A point `local` (x, z) in this rectangle's own frame, in the map.
    pub(crate) fn world(&self, local: P2) -> P2 {
        let (s, c) = self.yaw.sin_cos();
        [
            self.at[0] + local[0] * c - local[1] * s,
            self.at[1] + local[0] * s + local[1] * c,
        ]
    }

    /// Distance between two rectangles (negative: they overlap by about
    /// that much).
    pub(crate) fn gap(&self, other: &Rect) -> f32 {
        let a = self
            .outline(0.25)
            .into_iter()
            .map(|p| other.sdf(p))
            .fold(f32::INFINITY, f32::min);
        let b = other
            .outline(0.25)
            .into_iter()
            .map(|p| self.sdf(p))
            .fold(f32::INFINITY, f32::min);
        a.min(b)
    }
}

/// Something solid standing on the surface: a placement, or an
/// underground site's kiosk. A site's placement *is* its entrance kiosk
/// (its footprint is the kiosk and apron), so only the exits are added,
/// and nothing is counted twice.
#[derive(Clone, Debug)]
pub(crate) struct Solid {
    /// What to call it in a finding: the prefab, or "<site> exit".
    pub(crate) name: String,
    /// The prefab id, for the prefab lists (a site's id for its exits).
    pub(crate) prefab: String,
    /// What it stands on: its footprint (a kiosk's with its apron).
    pub(crate) rect: Rect,
    /// A kiosk's shell, walls and all (its apron is open ground).
    pub(crate) shell: Option<Rect>,
}

impl Solid {
    pub(crate) fn kiosk(&self) -> bool {
        self.shell.is_some()
    }

    /// A way indoors with a roof over it: a walk-in building (`ENTERABLE`)
    /// or a kiosk's stair down.
    pub(crate) fn enterable(&self) -> bool {
        self.kiosk() || ENTERABLE.contains(&crate::map::prefab_base(&self.prefab))
    }

    /// Where a kiosk's door is, on the outside face of its -Z end (portal
    /// space: the stair runs down +Z), and the way out of it.
    pub(crate) fn kiosk_door(&self) -> Option<(P2, P2)> {
        let shell = self.shell?;
        let at = shell.world([0.0, -shell.half[1]]);
        let out = geom::sub(shell.world([0.0, -shell.half[1] - 1.0]), at);
        Some((at, out))
    }
}

/// The point of interest whose survey drones see the rendezvous at `rv`
/// (their orbit plus [`SURVEY_SIGHT`]), nearest first, and how far off.
pub fn watched_by(map: &MapDef, rv: P2) -> Option<(String, f32)> {
    map.markers_of(&MarkerKind::Poi)
        .map(|m| (m.name.clone(), dist(m.at, rv)))
        .filter(|(_, d)| *d <= crate::map_sentry::SURVEY_ORBIT + SURVEY_SIGHT)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Something to wait for the truck behind at the rendezvous at `rv`: the
/// nearest roof or solid cover within [`WAIT_COVER`] (a walk-in building
/// or a kiosk, or anything [`WAIT_HEIGHT`] high but a garden fence), a
/// roof first, how far off it is, and where to wait: under the roof, just
/// in at its door (a drone that has seen you circles a wall, not a roof),
/// else crouched in the lee, away from `from` (the drones' point of
/// interest).
pub fn wait_shelter(map: &MapDef, rv: P2, from: P2) -> Option<(String, f32, P2)> {
    let roof = |q: &Solid| {
        crate::map_surge::roofed(&q.prefab) && !crate::map::prefab_entrances(&q.prefab).is_empty()
    };
    let q = solids(map)
        .into_iter()
        .filter(|q| {
            q.rect.sdf(rv) <= WAIT_COVER
                && (q.kiosk()
                    || crate::map_surge::roofed(&q.prefab)
                    || (q.prefab != "fence_garden"
                        && crate::map::prefab_footprint(&q.prefab)[1] >= WAIT_HEIGHT))
        })
        .min_by(|a, b| {
            (!roof(a), a.rect.sdf(rv))
                .partial_cmp(&(!roof(b), b.rect.sdf(rv)))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let d = q.rect.sdf(rv).max(0.0);
    if roof(&q) {
        let door = crate::map::prefab_entrances(&q.prefab)
            .iter()
            .min_by(|a, b| dist(q.rect.world(a.at), rv).total_cmp(&dist(q.rect.world(b.at), rv)))?;
        let inside = q.rect.world([
            door.at[0] - door.out[0] * WAIT_IN,
            door.at[1] - door.out[1] * WAIT_IN,
        ]);
        return Some((q.name, d, inside));
    }
    let away = geom::sub(q.rect.at, from);
    let l = geom::dist(q.rect.at, from).max(1e-3);
    let away = [away[0] / l, away[1] / l];
    let lee = (0..120)
        .map(|k| {
            let s = k as f32 * 0.25;
            [q.rect.at[0] + away[0] * s, q.rect.at[1] + away[1] * s]
        })
        .find(|p| q.rect.sdf(*p) >= WAIT_LEE)
        .unwrap_or(q.rect.at);
    Some((q.name, d, lee))
}

/// The career's first job from the drop at `ins` to `at`: the easy walk
/// there heads for it ([`FIRST_TURN`] over [`FIRST_HEAD`]), nothing with
/// loot near the drop lies off it ([`LURE_REACH`], [`LURE_OFF`]), and,
/// measured, how far a scare's nearest roof (`roofs`, its doors) lies
/// back from the job.
fn first_way(r: &mut Report, map: &MapDef, comfy: &NavGrid, ins: P2, at: P2, roofs: &[P2]) {
    let find = |r: &mut Report, rule: Rule, what: String, at: P2| {
        r.findings.push(Finding { rule, what, at });
    };
    let Some(path) = comfy.path(ins, at) else {
        return;
    };
    let pts = resample(&path);
    let bearing = |p: P2| (p[1] - ins[1]).atan2(p[0] - ins[0]);
    let want = bearing(at);
    let (mut along, mut worst, mut worst_at) = (0.0f32, 0.0f32, ins);
    for w in pts.windows(2) {
        along += dist(w[0], w[1]);
        if along > FIRST_HEAD {
            break;
        }
        if dist(w[1], ins) < 20.0 {
            continue;
        }
        let mut off = (bearing(w[1]) - want).to_degrees().abs() % 360.0;
        if off > 180.0 {
            off = 360.0 - off;
        }
        if off > worst {
            (worst, worst_at) = (off, w[1]);
        }
    }
    if worst > FIRST_TURN {
        find(
            r,
            Rule::Choice,
            format!(
                "the easy way to the career's first job turns {worst:.0} degrees off its bearing in the first {FIRST_HEAD:.0} m"
            ),
            worst_at,
        );
    }
    let off_way = |p: P2| {
        pts.iter()
            .map(|q| dist(*q, p))
            .fold(f32::INFINITY, f32::min)
    };
    let lures = solids(map)
        .into_iter()
        .filter(|s| s.kiosk() || crate::map_surge::roofed(&s.prefab))
        .map(|s| (s.name.clone(), s.rect.at))
        .chain(
            map.markers
                .iter()
                .filter(|m| matches!(m.kind, MarkerKind::Loot { .. }))
                .map(|m| (m.name.clone(), m.at)),
        );
    for (name, p) in lures {
        if dist(p, ins) <= LURE_REACH && off_way(p) > LURE_OFF {
            find(
                r,
                Rule::Choice,
                format!(
                    "the {name} {:.0} m from the drop and {:.0} m off the way to the career's first job draws a newcomer off it",
                    dist(p, ins),
                    off_way(p)
                ),
                p,
            );
        }
    }
    // A scare on the way: does the nearest roof send you back, or on?
    let back = pts
        .iter()
        .map(|p| {
            let q = roofs
                .iter()
                .copied()
                .min_by(|a, b| dist(*a, *p).total_cmp(&dist(*b, *p)));
            q.map_or(0.0, |q| dist(q, at) - dist(*p, at))
        })
        .fold(0.0f32, f32::max);
    r.measures.push(format!(
        "career's first job: {:.0} m the easy way; the nearest roof to a scare at worst {back:.0} m further from the job",
        path_length(&pts)
    ));
}

/// The first job of each kind that sends you somewhere in the valley
/// `id`: the career's first (`progression::first_job`, named
/// [`FIRST_OF_ALL`]), the first contract step of each (`contracts::CONTRACTS`, a scout,
/// a thing to bring, one to leave) and the ladder's fixed ones (the
/// letter, the ledger), as what and where.
pub fn first_jobs(map: &MapDef, id: &str) -> Vec<(String, P2)> {
    use crate::contracts::Step;
    let mut out: Vec<(String, P2)> = Vec::new();
    // The career's very first job, here if a career can start here.
    if let Some(crate::progression::Job::Scout { place, at, .. }) =
        crate::progression::first_job(id, map)
            .filter(|_| crate::progression::FIRST_VALLEYS.contains(&id))
    {
        out.push((format!("{FIRST_OF_ALL} ({place})"), at));
    }
    let mut seen: Vec<&str> = Vec::new();
    for c in crate::contracts::CONTRACTS {
        for s in c.steps {
            let (kind, m, place, near) = match *s {
                Step::Scout { map, place } => ("scout", map, place, None),
                Step::Bring {
                    map, place, near, ..
                } => ("bring", map, place, near),
                Step::Plant { map, place, .. } => ("plant", map, place, None),
                _ => continue,
            };
            if m != id || seen.contains(&kind) {
                continue;
            }
            let Some(at) = near.or_else(|| crate::contracts::place_at(map, place)) else {
                continue;
            };
            seen.push(kind);
            out.push((format!("{kind} ({}: {place})", c.id), at));
        }
    }
    if id == crate::progression::LETTER_MAP {
        out.push(("letter".into(), crate::progression::LETTER_AT));
    }
    if id == crate::progression::LEDGER_MAP {
        let [x, _, z] = crate::progression::LEDGER_AT;
        out.push(("ledger".into(), [x, z]));
    }
    out
}

/// The span inside the fence of the valley the distance rules were set
/// for (Marrow Creek as it was: 640 m, its fence 38 m in), metres.
pub const STANDARD_SPAN: f32 = 564.0;

/// How a tight valley's span inside its fence compares with
/// [`STANDARD_SPAN`], at most 1: a valley pulled in on purpose
/// ([`crate::map_openness::TIGHT`]) keeps the rules' distances in proportion
/// (`docs/MAP_DESIGN.md`, "Tight, broken, quiet between"). Any other
/// valley keeps them as set, the Training Yard's small one too.
pub fn valley_scale(map: &MapDef) -> f32 {
    if !crate::map_openness::TIGHT.contains(&map.name.as_str()) {
        return 1.0;
    }
    ((map.size - 2.0 * map.fence_inset) / STANDARD_SPAN).clamp(0.3, 1.0)
}

/// Every solid thing on the surface, placements first (in order: index
/// `i < map.placements.len()` is placement `i`), then the sites' exit
/// kiosks.
pub(crate) fn solids(map: &MapDef) -> Vec<Solid> {
    let kiosk = |at: P2, rot: f32| {
        let f = crate::underground::FOOTPRINT;
        (
            Rect {
                at,
                half: [f[0] * 0.5, f[2] * 0.5],
                yaw: rot.to_radians(),
            },
            Rect {
                at,
                half: crate::underground::SHELL_HALF,
                yaw: rot.to_radians(),
            },
        )
    };
    let mut out: Vec<Solid> = map
        .placements
        .iter()
        .map(|p| Solid {
            name: p.prefab.clone(),
            prefab: p.prefab.clone(),
            rect: Rect::of(&p.prefab, p.at, p.rot),
            shell: crate::underground::is_site(&p.prefab).then(|| kiosk(p.at, p.rot).1),
        })
        .collect();
    for site in crate::underground::placed(map) {
        for (at, rot, _) in site.exits() {
            let (rect, shell) = kiosk(at, rot);
            out.push(Solid {
                name: format!("{} exit", site.id),
                prefab: site.id.clone(),
                rect,
                shell: Some(shell),
            });
        }
    }
    out
}

/// Fences ([`FENCES`]) a main route runs through or brushes: within
/// [`FENCE_CLEAR`] of a run. A route keeps to the gentle grid, which treats
/// footprints as blocked, but a run is thinner than its cells.
pub(crate) fn fences_across(solids: &[Solid], routes: &[Route]) -> Vec<(String, P2)> {
    let mut out = Vec::new();
    for s in solids
        .iter()
        .filter(|s| FENCES.contains(&s.prefab.as_str()))
    {
        let reach = geom::len(s.rect.half) + FENCE_CLEAR + STEP;
        for route in routes {
            // Between its samples too: a run is 0.2 m deep, they're 2 m apart.
            let hit = route
                .points
                .windows(2)
                .filter(|w| dist(w[0], s.rect.at) < reach)
                .flat_map(|w| (0..8).map(move |k| geom::lerp(w[0], w[1], k as f32 / 8.0)))
                .find(|p| s.rect.sdf(*p) < FENCE_CLEAR);
            if let Some(p) = hit {
                out.push((
                    format!(
                        "{} at ({:.0}, {:.0}) is across the route to {}",
                        s.name, s.rect.at[0], s.rect.at[1], route.to
                    ),
                    p,
                ));
            }
        }
    }
    out
}

/// Points bucketed on a coarse grid, for "anything within r of here?".
struct Buckets {
    size: f32,
    cells: std::collections::HashMap<(i32, i32), Vec<P2>>,
}

impl Buckets {
    fn new(size: f32, points: impl IntoIterator<Item = P2>) -> Self {
        let mut cells: std::collections::HashMap<(i32, i32), Vec<P2>> = Default::default();
        for p in points {
            cells
                .entry(((p[0] / size).floor() as i32, (p[1] / size).floor() as i32))
                .or_default()
                .push(p);
        }
        Self { size, cells }
    }

    fn count_within(&self, p: P2, r: f32, enough: usize) -> usize {
        let k = (r / self.size).ceil() as i32;
        let (ci, cj) = (
            (p[0] / self.size).floor() as i32,
            (p[1] / self.size).floor() as i32,
        );
        let mut n = 0;
        for i in ci - k..=ci + k {
            for j in cj - k..=cj + k {
                if let Some(v) = self.cells.get(&(i, j)) {
                    n += v.iter().filter(|q| dist(**q, p) <= r).count();
                    if n >= enough {
                        return n;
                    }
                }
            }
        }
        n
    }
}

/// Cells reachable from `from`, stepping as the pathfinder does (eight
/// ways, no cutting a blocked corner).
fn flood(g: &NavGrid, from: P2) -> Vec<bool> {
    let n = g.n;
    let mut seen = vec![false; n * n];
    let Some(start) = (if g.walkable(from) {
        Some(from)
    } else {
        g.nearest_walkable(from)
    })
    .and_then(|p| g.cell_of(p)) else {
        return seen;
    };
    let mut stack = vec![start.1 * n + start.0];
    seen[stack[0]] = true;
    while let Some(k) = stack.pop() {
        let (i, j) = ((k % n) as i64, (k / n) as i64);
        for (di, dj) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (ni, nj) = (i + di, j + dj);
            if ni < 0 || nj < 0 || ni >= n as i64 || nj >= n as i64 {
                continue;
            }
            let nk = nj as usize * n + ni as usize;
            if seen[nk] || g.cost[nk] == BLOCKED {
                continue;
            }
            if di != 0 && dj != 0 {
                let a = j as usize * n + ni as usize;
                let b = nj as usize * n + i as usize;
                if g.cost[a] == BLOCKED || g.cost[b] == BLOCKED {
                    continue;
                }
            }
            seen[nk] = true;
            stack.push(nk);
        }
    }
    seen
}

/// Whether any flooded cell lies within `r` of `p`.
fn reached_near(g: &NavGrid, seen: &[bool], p: P2, r: f32) -> bool {
    let k = (r / g.cell).ceil() as i64;
    let Some((ci, cj)) = g.cell_of(p) else {
        return false;
    };
    for j in (cj as i64 - k).max(0)..=(cj as i64 + k).min(g.n as i64 - 1) {
        for i in (ci as i64 - k).max(0)..=(ci as i64 + k).min(g.n as i64 - 1) {
            let (i, j) = (i as usize, j as usize);
            if seen[j * g.n + i] && dist(g.centre(i, j), p) <= r {
                return true;
            }
        }
    }
    false
}

/// A path resampled every [`STEP`] metres.
pub(crate) fn resample(path: &[P2]) -> Vec<P2> {
    let mut out = vec![path[0]];
    let mut carry = 0.0;
    for w in path.windows(2) {
        let l = dist(w[0], w[1]);
        let mut s = STEP - carry;
        while s <= l {
            out.push(geom::lerp(w[0], w[1], s / l));
            s += STEP;
        }
        carry = l - (s - STEP);
    }
    if out.last() != path.last() {
        out.push(*path.last().unwrap());
    }
    out
}

fn path_length(path: &[P2]) -> f32 {
    path.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// Longest run of consecutive route samples failing `ok`, in metres, and
/// where it starts.
/// Every stretch along `points` where `ok` fails for at least `min`
/// metres: its length and where it starts.
fn open_runs(points: &[P2], ok: impl Fn(P2) -> bool, min: f32) -> Vec<(f32, P2)> {
    let mut out = Vec::new();
    let (mut run, mut start) = (0.0, points[0]);
    for (i, p) in points.iter().enumerate() {
        if ok(*p) {
            if run >= min {
                out.push((run, start));
            }
            run = 0.0;
        } else {
            if run == 0.0 {
                start = *p;
            }
            if i > 0 {
                run += dist(points[i - 1], *p);
            }
        }
    }
    if run >= min {
        out.push((run, start));
    }
    out
}

fn longest_gap(points: &[P2], ok: impl Fn(P2) -> bool) -> (f32, P2) {
    let (mut best, mut at, mut run, mut start) = (0.0, points[0], 0.0, points[0]);
    for (i, p) in points.iter().enumerate() {
        if ok(*p) {
            run = 0.0;
        } else {
            if run == 0.0 {
                start = *p;
            }
            if i > 0 {
                run += dist(points[i - 1], *p);
            }
            if run > best {
                best = run;
                at = start;
            }
        }
    }
    (best, at)
}

/// Whether the top of something `height` tall at `to` can be seen over
/// the land from eye height at `from`.
pub(crate) fn sees(hm: &Heightmap, from: P2, to: P2, height: f32) -> bool {
    let d = dist(from, to);
    let eye = hm.height(from) + 1.7;
    let top = hm.height(to) + height;
    let n = (d / 2.0).ceil() as usize;
    (1..n).all(|i| {
        let t = i as f32 / n as f32;
        // The landmark's own pad and plinth don't hide it.
        d * (1.0 - t) < 10.0 || hm.height(geom::lerp(from, to, t)) < eye + (top - eye) * t - 0.3
    })
}

/// Height you walk at: a bridge deck's where there is one, else the ground.
pub(crate) fn walk_height(hm: &Heightmap, p: P2) -> f32 {
    for rl in hm.road_lines.iter().filter(|rl| !rl.bridges.is_empty()) {
        let (d, s, _) = polyline_closest(&rl.points, p);
        if d <= rl.width * 0.5 + 1.0 && rl.on_bridge(s) {
            let i = (s.floor() as usize).min(rl.heights.len() - 1);
            return rl.heights[i];
        }
    }
    hm.height(p)
}

/// Whether a point is inside the quarantine fence (the playable valley),
/// by [`FENCE_MARGIN`]: a road's last metres climb the rim to the fence.
pub(crate) fn inside_fence(map: &MapDef, p: P2) -> bool {
    let half = map.half() - map.fence_inset - FENCE_MARGIN;
    p[0].abs() <= half && p[1].abs() <= half
}

fn is_target(kind: &MarkerKind) -> bool {
    matches!(
        kind,
        MarkerKind::Rendezvous
            | MarkerKind::Poi
            | MarkerKind::ArtifactField
            | MarkerKind::Offering
            | MarkerKind::ValleyKey { .. }
            | MarkerKind::Loot { .. }
            | MarkerKind::Terminal { .. }
            | MarkerKind::Note { .. }
    )
}

/// Lints a map as built from `hm`.
pub fn lint(map: &MapDef, hm: &Heightmap) -> Report {
    let mut r = Report {
        map: map.name.clone(),
        ..Default::default()
    };
    let find = |r: &mut Report, rule: Rule, what: String, at: P2| {
        r.findings.push(Finding { rule, what, at });
    };
    let Some(ins) = map.markers_of(&MarkerKind::Insertion).next().map(|m| m.at) else {
        find(&mut r, Rule::Reach, "no insertion".into(), [0.0, 0.0]);
        return r;
    };
    // The distances the rules keep are for a valley of the standard span;
    // a tighter one keeps them in proportion (the same walking, not the
    // same metres).
    let k = valley_scale(map);
    let walk = crate::map_nav::zone_grid(map, hm, crate::map_nav::MAX_SLOPE);
    let comfy = crate::map_nav::zone_grid(map, hm, COMFORT_SLOPE);
    let walk_seen = flood(&walk, ins);
    let doors = crate::map_doors::doors(map);
    let comfy_seen = flood(&comfy, ins);
    // Main routes keep to ground no steeper than a route may climb, as a
    // player picking the easy way would.
    let gentle = crate::map_nav::zone_grid(map, hm, ROUTE_GRADE);
    // Everything solid on the surface, the sites' exit kiosks too: `rects`
    // indexes into it.
    let solids = solids(map);
    let rects: Vec<(usize, Rect)> = solids.iter().map(|s| s.rect).enumerate().collect();
    let inside_building = |p: P2| {
        rects
            .iter()
            .find(|(_, rc)| rc.sdf(p) < 0.5)
            .map(|(i, rc)| (*i, *rc))
    };

    // -- Reach and comfort ---------------------------------------------------
    let mut targets: Vec<(String, P2, f32)> = Vec::new();
    for m in map.markers.iter().filter(|m| is_target(&m.kind)) {
        // A marker in or on a building: get to the building.
        let (at, reach) = match inside_building(m.at) {
            Some((_, rc)) => (rc.at, geom::len(rc.half) + 3.0),
            None => (m.at, 4.0),
        };
        // The anomaly core and artifact fields sit in hazards you edge
        // up to, not onto.
        let reach = if matches!(m.kind, MarkerKind::ArtifactField) {
            reach.max(FIELD_SPREAD)
        } else {
            reach
        };
        let name = if m.name.is_empty() {
            format!("{:?}", std::mem::discriminant(&m.kind))
        } else {
            m.name.clone()
        };
        targets.push((name, at, reach));
    }
    for (i, rc) in &rects {
        // A building you can walk into, or a kiosk's door: somewhere to go.
        if solids[*i].enterable() {
            targets.push((solids[*i].name.clone(), rc.at, geom::len(rc.half) + 3.0));
        }
    }
    let mut unreached = 0;
    let mut uncomfy = 0;
    for (name, at, reach) in &targets {
        if !reached_near(&walk, &walk_seen, *at, *reach) {
            unreached += 1;
            find(
                &mut r,
                Rule::Reach,
                format!("{name} can't be walked to"),
                *at,
            );
        } else if !reached_near(&comfy, &comfy_seen, *at, *reach) {
            uncomfy += 1;
            find(
                &mut r,
                Rule::Comfort,
                format!("{name} only by ground steeper than {COMFORT_SLOPE}°"),
                *at,
            );
        }
    }
    r.measures.push(format!(
        "reach: {} targets, {unreached} unreachable, {uncomfy} only over steep ground",
        targets.len()
    ));

    // -- Main routes -----------------------------------------------------------
    let rvs: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    let mut routes = Vec::new();
    for (name, at) in &rvs {
        match gentle.path(ins, *at) {
            Some(p) => routes.push(Route {
                to: name.to_string(),
                length: path_length(&p),
                points: resample(&p),
            }),
            None => find(
                &mut r,
                Rule::Comfort,
                format!("no route to {name} over ground under {ROUTE_GRADE}°"),
                *at,
            ),
        }
    }
    let route_pts: Vec<P2> = routes.iter().flat_map(|x| x.points.clone()).collect();
    let near_route = |p: P2, within: f32| route_pts.iter().any(|q| dist(*q, p) <= within);

    // -- Grade -----------------------------------------------------------------
    for route in &routes {
        let k = (GRADE_RUN / STEP).round() as usize;
        let mut worst = (0.0f32, route.points[0]);
        for w in route.points.windows(k + 1) {
            let rise = (walk_height(hm, w[k]) - walk_height(hm, w[0])).abs();
            let deg = rise.atan2(path_length(w)).to_degrees();
            if deg > worst.0 {
                worst = (deg, w[0]);
            }
        }
        r.measures.push(format!(
            "route to {}: {:.0} m, steepest {:.0}° over {GRADE_RUN} m",
            route.to, route.length, worst.0
        ));
        if worst.0 > ROUTE_GRADE {
            find(
                &mut r,
                Rule::Grade,
                format!("route to {} climbs {:.0}°", route.to, worst.0),
                worst.1,
            );
        }
    }
    for rl in &hm.road_lines {
        let def = &map.roads[rl.road];
        let limit = match rl.kind {
            RoadKind::Asphalt | RoadKind::Gravel => ROAD_GRADE,
            RoadKind::Dirt | RoadKind::Path => TRACK_GRADE,
            RoadKind::Rail => RAIL_GRADE,
        };
        let k = GRADE_RUN as usize;
        let mut worst = (0.0f32, rl.points[0]);
        for i in 0..rl.points.len().saturating_sub(k) {
            // Past the fence the road only runs into the rim.
            if !inside_fence(map, rl.points[i]) || !inside_fence(map, rl.points[i + k]) {
                continue;
            }
            let g = (rl.heights[i + k] - rl.heights[i]).abs() / GRADE_RUN * 100.0;
            if g > worst.0 {
                worst = (g, rl.points[i]);
            }
        }
        if worst.0 > limit {
            find(
                &mut r,
                Rule::Grade,
                format!("{} climbs {:.0}% (limit {limit}%)", def.name, worst.0),
                worst.1,
            );
        }
        // Where a road stops, you walk on: the ground past its end must meet
        // its surface, not stand up as the bank its grading cut.
        for (end, inward) in [(0usize, 1usize), (rl.points.len() - 1, rl.points.len() - 2)] {
            let p = rl.points[end];
            let dir = geom::sub(p, rl.points[inward]);
            let l = geom::len(dir).max(1e-3);
            let at = |d: f32| geom::add(p, geom::scale(dir, d / l));
            let door = crate::map_doors::near_door(&doors, p, DOOR_ROAD_END);
            // A road that ends on another road joins it.
            let joins = hm.road_lines.iter().any(|o| {
                o.road != rl.road && polyline_closest(&o.points, p).0 < o.width * 0.5 + 2.0
            });
            if door || joins || !inside_fence(map, at(END_WALK)) || rl.on_bridge(end as f32) {
                continue;
            }
            // No ledge: a bank the grading left past the end is a wall.
            let ledge = (1..=(END_WALK / 0.5) as usize)
                .map(|k| {
                    (hm.height(at(k as f32 * 0.5)) - hm.height(at((k - 1) as f32 * 0.5))).abs()
                })
                .fold(0.0, f32::max);
            if ledge > crate::terrain::ROAD_END_LEDGE {
                find(
                    &mut r,
                    Rule::Grade,
                    format!(
                        "{} ends below a {:.1} m ledge (in half a metre)",
                        def.name, ledge
                    ),
                    p,
                );
            }
        }
        let min = if rl.kind == RoadKind::Path {
            PATH_WIDTH
        } else {
            ROAD_WIDTH
        };
        if def.width < min {
            find(
                &mut r,
                Rule::Narrow,
                format!("{} is {} m wide", def.name, def.width),
                rl.points[0],
            );
        }
    }

    // -- Points of interest: spacing and pacing ----------------------------------
    let pois: Vec<(&str, P2)> = map
        .markers_of(&MarkerKind::Poi)
        .map(|m| (m.name.as_str(), m.at))
        .collect();
    let mut nn_min = f32::INFINITY;
    let mut nn_max: f32 = 0.0;
    for (i, (name, at)) in pois.iter().enumerate() {
        let (d, other) = pois
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, (n, q))| (dist(*at, *q), *n))
            .fold((f32::INFINITY, ""), |a, b| if b.0 < a.0 { b } else { a });
        nn_min = nn_min.min(d);
        nn_max = nn_max.max(d);
        if d < POI_MIN * k && i < pois.iter().position(|p| p.0 == other).unwrap_or(0) {
            find(
                &mut r,
                Rule::Spacing,
                format!("{name} is {d:.0} m from {other}"),
                *at,
            );
        }
        if d > POI_MAX {
            find(
                &mut r,
                Rule::Spacing,
                format!("{name} is {d:.0} m from its nearest neighbour ({other})"),
                *at,
            );
        }
    }
    r.measures.push(format!(
        "points of interest: {}, nearest neighbours {nn_min:.0}-{nn_max:.0} m",
        pois.len()
    ));
    // Paths of least resistance: a road or track leads to every place.
    for (name, at) in &pois {
        let hazard = map
            .markers_of(&MarkerKind::ArtifactField)
            .any(|f| dist(f.at, *at) <= HAZARD_NEAR);
        let road = hm
            .road_lines
            .iter()
            .map(|rl| polyline_closest(&rl.points, *at).0 - rl.width * 0.5)
            .fold(f32::INFINITY, f32::min);
        if !hazard && road > FUNNEL {
            find(
                &mut r,
                Rule::Funnel,
                format!("no road or path leads to {name} ({road:.0} m to the nearest)"),
                *at,
            );
        }
    }
    let interest: Vec<P2> = map
        .markers
        .iter()
        .filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Poi
                    | MarkerKind::ArtifactField
                    | MarkerKind::Offering
                    | MarkerKind::Loot { .. }
                    | MarkerKind::Rendezvous
            )
        })
        .map(|m| m.at)
        .chain(solids.iter().filter(|s| s.enterable()).map(|s| s.rect.at))
        .collect();
    // A kiosk's stair down is a roof too.
    let shelters: Vec<P2> = solids
        .iter()
        .filter(|s| s.enterable())
        .map(|s| s.rect.at)
        .collect();
    for route in &routes {
        let (gap, at) = longest_gap(&route.points, |p| {
            interest.iter().any(|q| dist(*q, p) <= NEAR_ROUTE)
        });
        let (roofless, rat) = longest_gap(&route.points, |p| {
            shelters.iter().any(|q| dist(*q, p) <= NEAR_ROUTE)
        });
        r.measures.push(format!(
            "route to {}: longest stretch with nothing near {gap:.0} m, with no roof near {roofless:.0} m",
            route.to
        ));
        if gap > PACING_GAP {
            find(
                &mut r,
                Rule::Pacing,
                format!("{gap:.0} m with nothing near, on the way to {}", route.to),
                at,
            );
        }
        if roofless > RHYTHM_GAP {
            find(
                &mut r,
                Rule::Rhythm,
                format!(
                    "{roofless:.0} m with no roof near, on the way to {}",
                    route.to
                ),
                rat,
            );
        }
    }

    // -- Landmarks -------------------------------------------------------------
    // Whole structures, seen at the tops of their tall parts
    // (`map_landmarks`).
    let marks = crate::map_landmarks::landmarks(map, hm);
    let in_sight = |p: P2, range: f32| crate::map_landmarks::in_sight(&marks, hm, p, range);
    for route in &routes {
        let seen = route
            .points
            .iter()
            .filter(|p| in_sight(**p, CLEAR_SIGHT))
            .count() as f32
            / route.points.len() as f32;
        let fog = route
            .points
            .iter()
            .filter(|p| in_sight(**p, FOG_SIGHT))
            .count() as f32
            / route.points.len() as f32;
        let (blind, at) = longest_gap(&route.points, |p| in_sight(p, CLEAR_SIGHT));
        r.measures.push(format!(
            "route to {}: a landmark in sight {:.0}% of the way ({:.0}% in fog), longest blind {blind:.0} m",
            route.to,
            seen * 100.0,
            fog * 100.0
        ));
        let sight = crate::map_landmarks::along(&marks, hm, &route.points, CLEAR_SIGHT, FOG_SIGHT);
        let each: Vec<String> = sight
            .each
            .iter()
            .map(|(name, c, f, a, b)| {
                format!(
                    "{name} {:.0}% ({:.0}% in fog, {a:.0}-{b:.0} m)",
                    c * 100.0,
                    f * 100.0
                )
            })
            .collect();
        r.measures
            .push(format!("route to {} sees: {}", route.to, each.join(", ")));
        for (which, kind, share, _) in &sight.twins {
            if !SERIES.contains(&kind.as_str()) {
                r.measures.push(format!(
                    "route to {}: two {kind}s in one view {:.0}% of the way ({which})",
                    route.to,
                    share * 100.0
                ));
            }
        }
        if seen < LANDMARK_SHARE {
            find(
                &mut r,
                Rule::Landmark,
                format!(
                    "a landmark in sight only {:.0}% of the way to {}",
                    seen * 100.0,
                    route.to
                ),
                route.points[route.points.len() / 2],
            );
        }
        if blind > BLIND_GAP {
            find(
                &mut r,
                Rule::Landmark,
                format!("{blind:.0} m with no landmark in sight, to {}", route.to),
                at,
            );
        }
    }
    // Two of the same kind in sight at once and you can't tell which is
    // which. A structure of several pieces is one landmark.
    for (i, a) in marks.iter().enumerate() {
        for b in marks.iter().skip(i + 1) {
            let apart = a.apart(b);
            if a.kind == b.kind && !SERIES.contains(&a.kind.as_str()) && apart < UNIQUE_SIGHT {
                find(
                    &mut r,
                    Rule::Landmark,
                    format!("two {}s {apart:.0} m apart", a.kind),
                    a.at(),
                );
            }
        }
    }
    for reg in map.regions.iter().filter(|g| g.danger > 0) {
        let (lo, hi) = geom::bounds(&reg.polygon);
        let mid = geom::scale(geom::add(lo, hi), 0.5);
        let own = crate::map_landmarks::tallest_in(map, hm, &reg.polygon);
        let near = marks
            .iter()
            .any(|m| m.tops.iter().any(|(at, _)| dist(*at, mid) <= FOG_SIGHT));
        if own < REGION_MARK && !near {
            find(
                &mut r,
                Rule::Landmark,
                format!(
                    "{} has nothing taller than {own:.0} m and no landmark near",
                    reg.name
                ),
                mid,
            );
        }
    }
    // You can always tell where you are: from nearly anywhere you can walk,
    // two landmarks you can tell apart are in sight over the land and the
    // trees (in VR there's no glancing at a map).
    let canopy = crate::map_landmarks::Canopy::new(map, hm);
    let (spots, deep) = crate::map_landmarks::gauge_cells(map, &walk, TWO_MARKS_STEP, WOOD_EDGE);
    let gauge = spots.clone();
    let cells = spots.len();
    let (mut two, mut two_fog) = (0usize, 0usize);
    let mut blind: std::collections::HashMap<(i32, i32), (usize, P2)> = Default::default();
    for p in spots {
        let k = crate::map_landmarks::kinds_in_sight(&marks, hm, &canopy, p, CLEAR_SIGHT, 2);
        if k >= 2 {
            two += 1;
            if crate::map_landmarks::kinds_in_sight(&marks, hm, &canopy, p, FOG_SIGHT, 2) >= 2 {
                two_fog += 1;
            }
        } else {
            let key = (
                (p[0] / POCKET).floor() as i32,
                (p[1] / POCKET).floor() as i32,
            );
            let e = blind.entry(key).or_insert((0, p));
            e.0 += 1;
        }
    }
    let share = two as f32 / cells.max(1) as f32;
    let mut pockets: Vec<(usize, P2)> = blind.into_values().collect();
    pockets.sort_by(|a, b| b.0.cmp(&a.0));
    let cell_area = TWO_MARKS_STEP * TWO_MARKS_STEP;
    let worst: Vec<String> = pockets
        .iter()
        .take(4)
        .map(|(n, p)| {
            format!(
                "{:.0} m² at ({:.0}, {:.0})",
                *n as f32 * cell_area,
                p[0],
                p[1]
            )
        })
        .collect();
    r.measures.push(format!(
        "two landmarks in sight (land and trees): {:.1}% of the walkable valley out of the deep woods ({:.0}% in fog; {:.0}% of it deep in woods){}",
        share * 100.0,
        two_fog as f32 * 100.0 / cells.max(1) as f32,
        deep as f32 * 100.0 / (cells + deep).max(1) as f32,
        if worst.is_empty() {
            String::new()
        } else {
            format!("; blind pockets {}", worst.join(", "))
        }
    ));

    // At night and in fog: lights to steer by, the landmarks' lamps and the
    // rendezvous beacons (the game's `render::landmarks`, `rv_beacons`).
    let lamp_chars = crate::map_landmarks::assign(&marks);
    let beacons: Vec<P2> = map
        .markers_of(&MarkerKind::Rendezvous)
        .map(|m| crate::map_landmarks::beacon_at(m.at))
        .collect();
    let (mut night, mut night2, mut fog, mut fog2) = (0usize, 0usize, 0usize, 0usize);
    let mut dark: std::collections::HashMap<(i32, i32), (usize, P2)> = Default::default();
    let mut murk: std::collections::HashMap<(i32, i32), (usize, P2)> = Default::default();
    for p in &gauge {
        let key = (
            (p[0] / POCKET).floor() as i32,
            (p[1] / POCKET).floor() as i32,
        );
        let n = crate::map_landmarks::lights_in_sight(
            &marks,
            &lamp_chars,
            &beacons,
            hm,
            &canopy,
            *p,
            (LAMP_REACH, BEACON_REACH),
            false,
            2,
        );
        night += (n >= 1) as usize;
        night2 += (n >= 2) as usize;
        if n == 0 {
            dark.entry(key).or_insert((0, *p)).0 += 1;
        }
        let f = crate::map_landmarks::lights_in_sight(
            &marks,
            &lamp_chars,
            &beacons,
            hm,
            &canopy,
            *p,
            (FOG_SIGHT, FOG_SIGHT),
            true,
            2,
        );
        fog += (f >= 1) as usize;
        fog2 += (f >= 2) as usize;
        if f == 0 {
            murk.entry(key).or_insert((0, *p)).0 += 1;
        }
    }
    let pct = |n: usize| n as f32 * 100.0 / cells.max(1) as f32;
    let worst_of = |m: std::collections::HashMap<(i32, i32), (usize, P2)>| {
        let mut v: Vec<(usize, P2)> = m.into_values().collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v.iter()
            .take(3)
            .map(|(n, p)| {
                format!(
                    "{:.0} m² at ({:.0}, {:.0})",
                    *n as f32 * cell_area,
                    p[0],
                    p[1]
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    r.measures.push(format!(
        "at night a lamp in sight from {:.1}% of the walkable valley, two you can name by their characters (a fix) from {:.1}% (dark pockets {}); in fog a landmark or lamp within {FOG_SIGHT:.0} m from {:.1}% (two from {:.0}%; blind {})",
        pct(night),
        pct(night2),
        worst_of(dark),
        pct(fog),
        pct(fog2),
        worst_of(murk),
    ));

    for (got, floor, what) in [
        (pct(night), NIGHT_ONE * 100.0, "at night a lamp in sight"),
        (
            pct(night2),
            NIGHT_TWO * 100.0,
            "at night two lamps in sight",
        ),
        (
            pct(fog),
            FOG_ONE * 100.0,
            "in fog a landmark or lamp in sight",
        ),
    ] {
        if got < floor {
            find(
                &mut r,
                Rule::Landmark,
                format!("{what} from only {got:.0}% of the walkable valley (at least {floor:.0}%)"),
                ins,
            );
        }
    }
    if share < TWO_MARKS_SHARE {
        find(
            &mut r,
            Rule::Landmark,
            format!(
                "two landmarks in sight from only {:.0}% of the walkable valley (at least {:.0}%)",
                share * 100.0,
                TWO_MARKS_SHARE * 100.0
            ),
            pockets.first().map_or(ins, |p| p.1),
        );
    }

    // The rendezvous is where a landmark is: one in sight within
    // RV_LANDMARK, and (measured) one within fog range.
    for m in map.markers_of(&MarkerKind::Rendezvous) {
        let seen = crate::map_landmarks::seen_from(&marks, hm, m.at, CLEAR_SIGHT);
        let fog = seen.iter().any(|s| s.1 <= FOG_SIGHT);
        r.measures.push(format!(
            "rendezvous {}: nearest landmark in sight {}{}",
            m.name,
            seen.first()
                .map_or("none".into(), |(n, d)| format!("{n} {d:.0} m")),
            if fog { "" } else { " (none in fog)" }
        ));
        // Its beacon from the valley side: the ground within reach of it
        // that has it in sight over the land and the trees, and the last
        // stretch of the way in to it.
        let beacon = crate::map_landmarks::beacon_at(m.at);
        let top = hm.height(beacon) + crate::map_landmarks::BEACON_HEIGHT;
        // A landmark standing at the rendezvous marks it as well as its
        // beacon does (the fire lookout's tower, with its lamp).
        let own: Vec<&crate::map_landmarks::Landmark> = marks
            .iter()
            .filter(|l| l.tops.iter().any(|t| dist(t.0, m.at) <= RV_MARK))
            .collect();
        let lit = |p: P2| {
            (sees(hm, p, beacon, crate::map_landmarks::BEACON_HEIGHT)
                && !canopy.hides(hm, p, beacon, top))
                || own
                    .iter()
                    .any(|l| l.seen_through(hm, &canopy, p, BEACON_APPROACH).is_some())
        };
        let near: Vec<P2> = gauge
            .iter()
            .copied()
            .filter(|p| dist(*p, beacon) <= BEACON_SIGHT)
            .collect();
        let around = near.iter().filter(|p| lit(**p)).count() as f32 / near.len().max(1) as f32;
        let approach: Vec<P2> = routes
            .iter()
            .filter(|rt| rt.to == m.name)
            .flat_map(|rt| rt.points.iter().copied())
            .filter(|p| dist(*p, m.at) <= BEACON_APPROACH && dist(*p, m.at) > TRUCK_APPROACH)
            // (Deep in a wood nothing's in sight: as for the gauge.)
            .filter(|p| !crate::map_landmarks::deep_in_woods(map, *p, WOOD_EDGE))
            .collect();
        let way_in =
            approach.iter().filter(|p| lit(**p)).count() as f32 / approach.len().max(1) as f32;
        r.measures.push(format!(
            "rendezvous {}: its beacon (or its own landmark) in sight from {:.0}% of the way in (the last {BEACON_APPROACH:.0} m, out of the deep woods), {:.0}% of the ground within {BEACON_SIGHT:.0} m",
            m.name,
            way_in * 100.0,
            around * 100.0
        ));
        if !approach.is_empty() && way_in < BEACON_SHARE {
            find(
                &mut r,
                Rule::Landmark,
                format!(
                    "{}'s beacon is in sight from only {:.0}% of the last {BEACON_APPROACH:.0} m in",
                    m.name,
                    way_in * 100.0
                ),
                beacon,
            );
        }
        if seen.first().is_none_or(|s| s.1 > RV_LANDMARK) {
            find(
                &mut r,
                Rule::Landmark,
                format!(
                    "no landmark in sight within {RV_LANDMARK:.0} m of {}",
                    m.name
                ),
                m.at,
            );
        }
    }

    // -- Cover -------------------------------------------------------------------
    let rocks = crate::rocks::scatter_rocks(map, hm);
    let trees = crate::forest::scatter_trees(map, hm);
    let boulders = Buckets::new(
        16.0,
        rocks
            .iter()
            .filter(|k| k.radius >= BOULDER_RADIUS && 2.0 * k.radius * k.squash >= BOULDER_HEIGHT)
            .map(|k| [k.pos[0], k.pos[2]]),
    );
    let tree_b = Buckets::new(16.0, trees.iter().map(|t| [t.pos[0], t.pos[2]]));
    let covered = |p: P2| {
        rects.iter().any(|(_, rc)| rc.sdf(p) <= COVER_REACH)
            || boulders.count_within(p, COVER_REACH, 1) > 0
            || tree_b.count_within(p, COVER_REACH, STAND) >= STAND
    };
    for route in &routes {
        let (gap, at) = longest_gap(&route.points, covered);
        let share =
            route.points.iter().filter(|p| covered(**p)).count() as f32 / route.points.len() as f32;
        let runs: Vec<String> = open_runs(&route.points, covered, COVER_NOTE)
            .iter()
            .map(|(l, p)| format!("{l:.0} m from ({:.0}, {:.0})", p[0], p[1]))
            .collect();
        r.measures.push(format!(
            "route to {}: cover within {COVER_REACH} m {:.0}% of the way, longest open {gap:.0} m{}",
            route.to,
            share * 100.0,
            if runs.is_empty() {
                String::new()
            } else {
                format!(" (open: {})", runs.join(", "))
            }
        ));
        if gap > COVER_GAP {
            find(
                &mut r,
                Rule::Cover,
                format!("{gap:.0} m in the open on the way to {}", route.to),
                at,
            );
        }
    }
    // -- The first ten minutes: at least two ways to go ----------------------
    {
        // Each place worth going to early: reached, with loot, and a walk
        // to it with cover.
        let mut options: Vec<(String, f32, Vec<P2>)> = Vec::new();
        // -- Waiting for the truck under a drone's eye --------------------------
        for m in map.markers_of(&MarkerKind::Rendezvous) {
            let Some((poi, d)) = watched_by(map, m.at) else {
                continue;
            };
            let from = map
                .markers_of(&MarkerKind::Poi)
                .find(|p| p.name == poi)
                .map_or(m.at, |p| p.at);
            let shelter = wait_shelter(map, m.at, from);
            r.measures.push(format!(
                "rendezvous {}: {poi} {d:.0} m off (a survey drone's eye); {}",
                m.name,
                shelter
                    .as_ref()
                    .map_or("nothing to wait behind".to_string(), |(q, d, _)| format!(
                        "waits behind the {q} {d:.0} m off"
                    ))
            ));
            if shelter.is_none() {
                find(
                    &mut r,
                    Rule::Cover,
                    format!(
                        "nothing within {WAIT_COVER} m to wait behind at {}, under the drones that survey {poi}",
                        m.name
                    ),
                    m.at,
                );
            }
        }

        // -- The way out: two ways home from every rich place ------------------
        {
            // A hunted player heading home takes the short way, not the
            // easy road: plan on comfortable ground, every cell alike.
            let flat = {
                let mut g = comfy.clone();
                for c in g.cost.iter_mut().filter(|c| **c != BLOCKED) {
                    *c = 1;
                }
                g
            };
            let rvs: Vec<(&str, P2)> = map
                .markers_of(&MarkerKind::Rendezvous)
                .map(|m| (m.name.as_str(), m.at))
                .collect();
            let rich: Vec<(&str, P2)> = map
                .markers
                .iter()
                .filter(|m| matches!(m.kind, MarkerKind::Poi | MarkerKind::ArtifactField))
                .filter(|m| {
                    map.region_at(m.at)
                        .is_some_and(|g| g.danger >= FIELD_DANGER)
                })
                .map(|m| (m.name.as_str(), m.at))
                .collect();
            let mut worst: Option<(String, f32)> = None;
            let mut fine = 0;
            for (name, at) in &rich {
                // Each rendezvous: its walk, and whether it has cover all the way.
                let mut outs: Vec<(&str, f32, Vec<P2>, bool)> = rvs
                    .iter()
                    .filter_map(|(rv, rat)| {
                        let path = flat.path(*at, *rat)?;
                        let pts = resample(&path);
                        let ok = longest_gap(&pts, covered).0 <= COVER_GAP;
                        Some((*rv, path_length(&path), pts, ok))
                    })
                    .collect();
                outs.sort_by(|a, b| a.1.total_cmp(&b.1));
                let Some(near) = outs.first().map(|o| o.1) else {
                    continue;
                };
                let good: Vec<_> = outs
                    .iter()
                    .filter(|o| o.3 && o.1 <= (near + OUT_EXTRA).max(OUT_REACH))
                    .collect();
                // How far along either walk before it's CHOICE_APART off the
                // other (from beside one rendezvous, the walk to it is a few
                // metres and the other walk leaves it at once).
                let parts = |a: &[P2], b: &[P2]| {
                    let off = |x: &[P2], y: &[P2]| {
                        x.iter()
                            .position(|p| polyline_closest(y, *p).0 > CHOICE_APART)
                            .map(|k| k as f32 * STEP)
                    };
                    match (off(a, b), off(b, a)) {
                        (Some(p), Some(q)) => Some(p.min(q)),
                        (p, q) => p.or(q),
                    }
                };
                let two = good.iter().enumerate().any(|(i, a)| {
                    good[i + 1..]
                        .iter()
                        .any(|b| parts(&a.2, &b.2).is_some_and(|d| d <= OUT_PART))
                });
                if two {
                    fine += 1;
                } else {
                    let why = format!(
                        "{} way(s) home with cover in reach, parting within {OUT_PART} m ({})",
                        good.len(),
                        outs.iter()
                            .map(|o| format!(
                                "{} {:.0} m{}",
                                o.0,
                                o.1,
                                if o.3 { "" } else { ", open" }
                            ))
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                    if worst.is_none() {
                        worst = Some((name.to_string(), 0.0));
                    }
                    find(
                        &mut r,
                        Rule::Choice,
                        format!("from {name}, one way home: {why}"),
                        *at,
                    );
                }
            }
            r.measures.push(format!(
                "the way out: {fine} of {} rich places have two ways home that part",
                rich.len()
            ));
            // The last stretch to each rendezvous on every main route.
            let spotters = crate::map_spotter::spotters(map);
            for route in &routes {
                let mut back = 0.0;
                let mut k = route.points.len().saturating_sub(1);
                while k > 0 && back < LAST_STRETCH {
                    back += dist(route.points[k], route.points[k - 1]);
                    k -= 1;
                }
                let last = &route.points[k..];
                let (gap, at) = longest_gap(last, covered);
                let seen = last
                    .iter()
                    .filter(|p| spotters.iter().any(|s| s.sees(map, hm, **p)))
                    .count() as f32
                    / last.len().max(1) as f32;
                r.measures.push(format!(
                "last {LAST_STRETCH} m to {}: longest open {gap:.0} m, {:.0}% in a spotter's sight",
                route.to,
                seen * 100.0
            ));
                if gap > LAST_OPEN {
                    find(
                        &mut r,
                        Rule::Cover,
                        format!(
                            "{gap:.0} m in the open in the last {LAST_STRETCH} m to {}",
                            route.to
                        ),
                        at,
                    );
                }
                if seen > LAST_SPOTTED {
                    find(
                        &mut r,
                        Rule::Spotter,
                        format!(
                            "{:.0}% of the last {LAST_STRETCH} m to {} is in a spotter's sight",
                            seen * 100.0,
                            route.to
                        ),
                        *last.last().unwrap_or(&at),
                    );
                }
            }
            let _ = worst;
        }

        for poi in map.markers_of(&MarkerKind::Poi) {
            let Some(path) = comfy.path(ins, poi.at) else {
                continue;
            };
            if path_length(&path) > FIRST_REACH {
                continue;
            }
            let loot = map.markers.iter().any(|m| {
                matches!(m.kind, MarkerKind::Loot { .. }) && dist(m.at, poi.at) <= CHOICE_LOOT
            }) || solids
                .iter()
                .any(|q| q.enterable() && q.rect.sdf(poi.at) <= CHOICE_LOOT);
            let pts = resample(&path);
            let (gap, _) = longest_gap(&pts, covered);
            if !loot || gap > COVER_GAP {
                continue;
            }
            let bearing = (poi.at[0] - ins[0])
                .atan2(-(poi.at[1] - ins[1]))
                .to_degrees();
            options.push((poi.name.clone(), bearing, pts));
        }
        // The best pair: furthest apart in bearing, with walks that part.
        let shared = |a: &[P2], b: &[P2]| {
            let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
            short
                .iter()
                .filter(|p| polyline_closest(long, **p).0 <= CHOICE_APART)
                .count() as f32
                / short.len().max(1) as f32
        };
        let mut best: Option<(f32, usize, usize)> = None;
        for i in 0..options.len() {
            for j in i + 1..options.len() {
                let mut d = (options[i].1 - options[j].1).abs() % 360.0;
                if d > 180.0 {
                    d = 360.0 - d;
                }
                if shared(&options[i].2, &options[j].2) > CHOICE_SHARED {
                    continue;
                }
                if best.is_none_or(|b| d > b.0) {
                    best = Some((d, i, j));
                }
            }
        }
        let names: Vec<&str> = options.iter().map(|o| o.0.as_str()).collect();
        match best {
            Some((d, i, j)) => r.measures.push(format!(
                "first ten minutes: {} places with loot and cover ({}); widest choice {} or {}, {d:.0} degrees apart",
                options.len(),
                names.join(", "),
                options[i].0,
                options[j].0
            )),
            None => r.measures.push(format!(
                "first ten minutes: {} places with loot and cover ({}); no two whose walks part",
                options.len(),
                names.join(", ")
            )),
        }
        if best.is_none_or(|b| b.0 < CHOICE_ANGLE) {
            find(
                &mut r,
                Rule::Choice,
                format!(
                    "the first ten minutes offer no two ways to go ({} places with loot and cover within {FIRST_REACH} m)",
                    options.len()
                ),
                ins,
            );
        }
    }

    for poi in map.markers_of(&MarkerKind::Poi) {
        if dist(poi.at, ins) > FIRST_WALK {
            continue;
        }
        // The way a player who keeps to easy ground goes, and the straight
        // line a newcomer takes toward a roof they can see.
        let n = (dist(ins, poi.at) / STEP).ceil().max(1.0) as usize;
        let straight: Vec<P2> = (0..=n)
            .map(|i| geom::lerp(ins, poi.at, i as f32 / n as f32))
            .collect();
        let walks = comfy
            .path(ins, poi.at)
            .map(|p| resample(&p))
            .into_iter()
            .chain([straight]);
        let (gap, at) = walks
            .map(|pts| longest_gap(&pts, covered))
            .fold((0.0f32, ins), |a, b| if b.0 > a.0 { b } else { a });
        r.measures.push(format!(
            "first walk to {}: longest open {gap:.0} m",
            poi.name
        ));
        // Measured, not held: the lint's cover (any boulder, three trunks)
        // is more than a newcomer uses; Onboarding's first-fight scenario
        // is the judge of these walks.
        let _ = (FIRST_COVER_GAP, at);
    }

    // -- The first jobs: cover within a few seconds' run all the way ------------
    {
        let id = map.name.to_lowercase().replace(' ', "_");
        // Doors you can be in at: a roofed building's or a kiosk's.
        let roofs: Vec<P2> = crate::map_doors::doors(map)
            .into_iter()
            .filter(|d| crate::map_surge::roofed(&map.placements[d.placement].prefab))
            .map(|d| d.face)
            .chain(
                solids
                    .iter()
                    .filter_map(|s| s.kiosk_door().map(|(at, _)| at)),
            )
            .collect();
        let near_cover = |p: P2| {
            rects.iter().any(|(_, rc)| rc.sdf(p) <= FIRST_RUN)
                || boulders.count_within(p, FIRST_RUN, 1) > 0
                || tree_b.count_within(p, FIRST_RUN, STAND) >= STAND
        };
        // A newcomer cuts across as often as they keep to the road, and
        // stops at what's on the way.
        let flat = {
            let mut g = comfy.clone();
            for c in g.cost.iter_mut().filter(|c| **c != BLOCKED) {
                *c = 1;
            }
            g
        };
        let pois: Vec<P2> = map.markers_of(&MarkerKind::Poi).map(|m| m.at).collect();
        let rvs: Vec<P2> = map
            .markers_of(&MarkerKind::Rendezvous)
            .map(|m| m.at)
            .collect();
        for (what, at) in first_jobs(map, &id) {
            // There, and home from there to the two nearest rendezvous
            // (Onboarding's newcomers fell on the way home from the
            // Village, by the pumping station): the way there is held, the
            // way home against the main routes' cover.
            let mut home = rvs.clone();
            home.sort_by(|a, b| dist(*a, at).total_cmp(&dist(*b, at)));
            let legs: Vec<(P2, P2)> = std::iter::once((ins, at))
                .chain(home.iter().take(2).map(|rv| (at, *rv)))
                .collect();
            // (Home or there, the walk.)
            let mut walks: Vec<(bool, Vec<P2>)> = Vec::new();
            for (n, (from, to)) in legs.into_iter().enumerate() {
                let home = n > 0;
                let direct = dist(from, to);
                for g in [&comfy, &flat] {
                    walks.extend(g.path(from, to).map(|p| (home, resample(&p))));
                    for v in pois.iter().filter(|v| {
                        dist(from, **v) + dist(**v, to) <= direct * FIRST_DETOUR
                            && dist(**v, to) > 1.0
                            && dist(**v, from) > 1.0
                    }) {
                        if let (Some(a), Some(b)) = (g.path(from, *v), g.path(*v, to)) {
                            walks.push((home, resample(&[a, b].concat())));
                        }
                    }
                }
            }
            if what.starts_with(FIRST_OF_ALL) {
                first_way(&mut r, map, &comfy, ins, at, &roofs);
            }
            let (mut gap, mut gap_at, mut roof, mut roof_at) = (0.0f32, ins, 0.0f32, ins);
            let (mut back, mut back_at) = (0.0f32, at);
            for (home, pts) in &walks {
                // Home, laden and warned, held to the main routes' cover
                // (Fourth valley: a first job's walk home shouldn't be a
                // 54 m dash over Sallow's bed).
                if *home {
                    let (g, a) = longest_gap(pts, covered);
                    if g > back {
                        (back, back_at) = (g, a);
                    }
                } else {
                    let (g, a) = longest_gap(pts, near_cover);
                    if g > gap {
                        (gap, gap_at) = (g, a);
                    }
                }
                for p in pts {
                    let d = roofs
                        .iter()
                        .map(|q| dist(*p, *q))
                        .fold(f32::INFINITY, f32::min);
                    if d > roof {
                        (roof, roof_at) = (d, *p);
                    }
                }
            }
            r.measures.push(format!(
                "first job {what}, {} ways: longest run with no cover {FIRST_RUN} m off {gap:.0} m at ({:.0}, {:.0}), home with none {COVER_REACH} m off {back:.0} m at ({:.0}, {:.0}); farthest from a roof {roof:.0} m at ({:.0}, {:.0})",
                walks.len(),
                gap_at[0],
                gap_at[1],
                back_at[0],
                back_at[1],
                roof_at[0],
                roof_at[1]
            ));
            if back > COVER_GAP {
                find(
                    &mut r,
                    Rule::Cover,
                    format!(
                        "on the way home from the first job {what}, {back:.0} m with no cover within {COVER_REACH:.0} m"
                    ),
                    back_at,
                );
            }
            if gap > FIRST_OPEN {
                find(
                    &mut r,
                    Rule::Cover,
                    format!(
                        "on the way to the first job {what}, {gap:.0} m with nothing within a {FIRST_RUN:.0} m run"
                    ),
                    gap_at,
                );
            }
        }
    }

    // How far from cover the open ground runs, anywhere you can walk in the
    // valley (not only along the main routes: raids cross the open to
    // what they're after).
    let cover_within = |p: P2, reach: f32| {
        rects.iter().any(|(_, rc)| rc.sdf(p) <= reach)
            || boulders.count_within(p, reach, 1) > 0
            || tree_b.count_within(p, reach, STAND) >= STAND
    };
    let (mut cells, mut open, mut worst) = (0usize, 0usize, (0.0f32, [0.0, 0.0]));
    let step = (OPEN_STEP / walk.cell).round().max(1.0) as usize;
    for j in (0..walk.n).step_by(step) {
        for i in (0..walk.n).step_by(step) {
            let p = walk.centre(i, j);
            if walk.cost[j * walk.n + i] == BLOCKED
                || !map
                    .regions
                    .iter()
                    .any(|reg| geom::point_in_polygon(&reg.polygon, p))
            {
                continue;
            }
            cells += 1;
            if cover_within(p, COVER_REACH) {
                continue;
            }
            open += 1;
            let far = OPEN_LADDER
                .iter()
                .copied()
                .find(|&reach| cover_within(p, reach))
                .unwrap_or(f32::INFINITY);
            if far > worst.0 {
                worst = (far, p);
            }
        }
    }
    r.measures.push(format!(
        "open ground: {:.0}% of the walkable valley over {COVER_REACH} m from cover; the farthest from it {} at ({:.0}, {:.0})",
        open as f32 * 100.0 / cells.max(1) as f32,
        if worst.0.is_finite() {
            format!("within {:.0} m", worst.0)
        } else {
            format!("over {:.0} m", OPEN_LADDER[OPEN_LADDER.len() - 1])
        },
        worst.1[0],
        worst.1[1],
    ));
    let share = open as f32 / cells.max(1) as f32;
    if share > OPEN_SHARE {
        find(
            &mut r,
            Rule::Cover,
            format!(
                "{:.0}% of the walkable valley is over {COVER_REACH} m from cover (at most {:.0}%)",
                share * 100.0,
                OPEN_SHARE * 100.0
            ),
            worst.1,
        );
    }
    if worst.0 > OPEN_FAR {
        find(
            &mut r,
            Rule::Cover,
            format!(
                "ground {} m from any cover: a drone's open killing ground",
                if worst.0.is_finite() {
                    format!("{:.0}", worst.0)
                } else {
                    "over 100".into()
                }
            ),
            worst.1,
        );
    }

    // -- Loot against risk ---------------------------------------------------------
    let mut by_danger: Vec<(u8, f32, usize)> = Vec::new();
    for reg in &map.regions {
        match by_danger.iter_mut().find(|d| d.0 == reg.danger) {
            Some(d) => {
                d.1 += reg.richness();
                d.2 += 1;
            }
            None => by_danger.push((reg.danger, reg.richness(), 1)),
        }
    }
    by_danger.sort_by_key(|d| d.0);
    let means: Vec<(u8, f32)> = by_danger.iter().map(|d| (d.0, d.1 / d.2 as f32)).collect();
    r.measures.push(format!(
        "loot by danger: {}",
        means
            .iter()
            .map(|(d, m)| format!("{d}: {m:.2}"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for w in means.windows(2) {
        if w[1].1 <= w[0].1 {
            find(
                &mut r,
                Rule::LootRisk,
                format!(
                    "danger {} regions roll {:.2}, no more than danger {} ({:.2})",
                    w[1].0, w[1].1, w[0].0, w[0].1
                ),
                ins,
            );
        }
    }
    for reg in &map.regions {
        // A region two tiers safer than another never rolls richer.
        if let Some(o) = map
            .regions
            .iter()
            .find(|o| o.danger >= reg.danger + 2 && o.richness() <= reg.richness())
        {
            find(
                &mut r,
                Rule::LootRisk,
                format!(
                    "{} (danger {}) rolls {:.2}, as much as {} (danger {})",
                    reg.name,
                    reg.danger,
                    reg.richness(),
                    o.name,
                    o.danger
                ),
                reg.polygon[0],
            );
        }
        if reg.danger >= 2 {
            let d = reg
                .polygon
                .iter()
                .map(|p| dist(*p, ins))
                .fold(f32::INFINITY, f32::min);
            let inside = geom::point_in_polygon(&reg.polygon, ins);
            if inside || d < DANGER_FAR * k {
                find(
                    &mut r,
                    Rule::LootRisk,
                    format!(
                        "{} (danger {}) is {d:.0} m from the insertion",
                        reg.name, reg.danger
                    ),
                    reg.polygon[0],
                );
            }
        }
    }
    if map.region_at(ins).is_some_and(|g| g.danger > 0) {
        find(
            &mut r,
            Rule::LootRisk,
            "the insertion isn't safe ground".into(),
            ins,
        );
    }
    for m in map.markers_of(&MarkerKind::ArtifactField) {
        let d = map.region_at(m.at).map_or(0, |g| g.danger);
        if d < FIELD_DANGER && !map.regions.iter().all(|g| g.danger == 0) {
            find(
                &mut r,
                Rule::LootRisk,
                format!("artifact field {} is only danger {d}", m.name),
                m.at,
            );
        }
    }

    // -- Footing: water and cliffs ---------------------------------------------------
    for m in &map.markers {
        let check = matches!(
            m.kind,
            MarkerKind::Insertion
                | MarkerKind::Rendezvous
                | MarkerKind::ShortExfil
                | MarkerKind::TruckStop
                | MarkerKind::Loot { .. }
                | MarkerKind::Offering
                | MarkerKind::ValleyKey { .. }
                | MarkerKind::WorkSite
                | MarkerKind::ArtifactField
                | MarkerKind::Sign { .. }
                | MarkerKind::Note { .. }
        );
        if !check || m.y > 0.0 {
            continue;
        }
        if hm.water_level(m.at).is_some() {
            find(
                &mut r,
                Rule::Footing,
                format!("{} is in the water", m.name),
                m.at,
            );
        }
        // Inside a building, the floor is what it stands on.
        if inside_building(m.at).is_none() && hm.slope_deg(m.at) > COMFORT_SLOPE {
            find(
                &mut r,
                Rule::Footing,
                format!("{} is on a {:.0}° slope", m.name, hm.slope_deg(m.at)),
                m.at,
            );
        }
    }
    // A loot marker in the open gets a supply crate: solid, so clear of
    // walls and of where the truck appears.
    for m in &map.markers {
        if !matches!(m.kind, MarkerKind::Loot { .. }) || m.y >= 0.3 {
            continue;
        }
        if let Some((i, rc)) = rects.iter().find(|(_, rc)| rc.sdf(m.at) < CRATE_ROOM) {
            find(
                &mut r,
                Rule::Crate,
                format!(
                    "{}'s crate is {:.1} m from the {}",
                    m.name,
                    rc.sdf(m.at),
                    solids[*i].name
                ),
                m.at,
            );
        }
        if let Some((rv, at)) = rvs.iter().find(|(_, at)| dist(*at, m.at) < CRATE_FROM_RV) {
            find(
                &mut r,
                Rule::Crate,
                format!(
                    "{}'s crate is {:.0} m from the {rv} rendezvous",
                    m.name,
                    dist(*at, m.at)
                ),
                m.at,
            );
        }
    }
    // Fence runs, and the most ground over any one's foot.
    let mut fences = (0, 0.0f32);
    for p in &map.placements {
        if p.prefab == "footbridge" || p.absolute {
            continue;
        }
        let rc = Rect::of(&p.prefab, p.at, p.rot);
        let wet = rc
            .outline(1.0)
            .into_iter()
            .chain([p.at])
            .any(|q| hm.water_level(q).is_some());
        if wet {
            find(
                &mut r,
                Rule::Footing,
                format!("{} stands in water", p.prefab),
                p.at,
            );
        }
        if !prefab_wants_pad(&p.prefab) && hm.slope_deg(p.at) > COMFORT_SLOPE {
            find(
                &mut r,
                Rule::Footing,
                format!("{} is on a {:.0}° slope", p.prefab, hm.slope_deg(p.at)),
                p.at,
            );
        }
        if FENCES.contains(&p.prefab.as_str()) {
            let foot = hm.footing(&p.prefab, p.at, p.rot);
            // Every half metre along the run.
            let n = (rc.half[0] * 4.0).ceil() as usize;
            let (rise, at) = (0..=n)
                .map(|k| rc.world([-rc.half[0] + k as f32 * 0.5, 0.0]))
                .map(|q| (hm.height(q) - foot, q))
                .fold((0.0f32, p.at), |a, b| if b.0 > a.0 { b } else { a });
            fences = (fences.0 + 1, fences.1.max(rise));
            if rise > FENCE_RISE {
                find(
                    &mut r,
                    Rule::Footing,
                    format!(
                        "the {} at ({:.0}, {:.0}) has ground {rise:.2} m over its foot",
                        p.prefab, p.at[0], p.at[1]
                    ),
                    at,
                );
            }
        }
    }
    if fences.0 > 0 {
        r.measures.push(format!(
            "fences: {} runs, the ground at most {:.2} m over a run's foot",
            fences.0, fences.1
        ));
    }
    // Wrecks, and the most ground over or under any one's foot.
    let mut wrecks = (0, 0.0f32);
    for p in &map.placements {
        if !WRECKS.contains(&p.prefab.as_str()) || p.absolute {
            continue;
        }
        let rc = Rect::of(&p.prefab, p.at, p.rot);
        let foot = hm.footing(&p.prefab, p.at, p.rot);
        // Every half metre under it, edges included.
        let (nx, nz) = (
            (rc.half[0] * 4.0).ceil() as usize,
            (rc.half[1] * 4.0).ceil() as usize,
        );
        let (off, at) = (0..=nx)
            .flat_map(|i| (0..=nz).map(move |k| (i, k)))
            .map(|(i, k)| {
                rc.world([
                    -rc.half[0] + 2.0 * rc.half[0] * i as f32 / nx as f32,
                    -rc.half[1] + 2.0 * rc.half[1] * k as f32 / nz as f32,
                ])
            })
            .map(|q| ((hm.height(q) - foot).abs(), q))
            .fold((0.0f32, p.at), |a, b| if b.0 > a.0 { b } else { a });
        wrecks = (wrecks.0 + 1, wrecks.1.max(off));
        if off > WRECK_RISE {
            find(
                &mut r,
                Rule::Footing,
                format!(
                    "the {} at ({:.0}, {:.0}) has ground {off:.2} m off its foot",
                    p.prefab, p.at[0], p.at[1]
                ),
                at,
            );
        }
    }
    if wrecks.0 > 0 {
        r.measures.push(format!(
            "wrecks: {}, the ground at most {:.2} m off a wreck's foot",
            wrecks.0, wrecks.1
        ));
    }
    // The truck stands on its wheels where it stops: at the base, its
    // rear to the drop; at a rendezvous, its bed to the valley's middle
    // (`raid::truck::base_pose`, `rv_pose`).
    let mut worst_truck = 0.0f32;
    for (what, at, yaw) in map.truck_stands() {
        let fall = truck_fall(hm, at, yaw);
        worst_truck = worst_truck.max(fall.abs());
        if fall.abs() > TRUCK_LEVEL {
            find(
                &mut r,
                Rule::Footing,
                format!(
                    "the truck at {what} stands on ground falling {:.1}° from cab to tail (at most {:.1}°): level its stop",
                    fall.to_degrees(),
                    TRUCK_LEVEL.to_degrees()
                ),
                at,
            );
        }
    }
    r.measures.push(format!(
        "truck stops: the ground under the truck falls at most {:.1}°",
        worst_truck.to_degrees()
    ));
    // Kiosks stand on a levelled pad (`Heightmap::underground`; a site's
    // own placement is padded as any building), so what can go wrong is
    // under the exits' pads, and where their doors open: onto water or a
    // slope you can't walk up to the door over.
    for s in solids.iter().filter(|s| s.kiosk()) {
        let is_exit = !crate::underground::is_site(&s.name);
        let wet = is_exit
            && s.rect
                .outline(1.0)
                .into_iter()
                .chain([s.rect.at])
                .any(|q| hm.water_level(q).is_some());
        if wet {
            find(
                &mut r,
                Rule::Footing,
                format!("{} stands in water", s.name),
                s.rect.at,
            );
        }
        let (door, out) = s.kiosk_door().unwrap();
        let step = geom::add(door, geom::scale(out, KIOSK_STEP));
        if hm.water_level(step).is_some() {
            find(
                &mut r,
                Rule::Footing,
                format!("{}'s door opens onto water", s.name),
                step,
            );
        } else if hm.slope_deg(step) > COMFORT_SLOPE {
            find(
                &mut r,
                Rule::Footing,
                format!(
                    "{}'s door opens onto a {:.0}° slope",
                    s.name,
                    hm.slope_deg(step)
                ),
                step,
            );
        }
    }

    // -- Overlap: buildings in each other, on roads, squeezes -------------------------
    let solid = |id: &str| id != "anomaly_core";
    let is_fence = |id: &str| id == "fence_garden";
    for (a, ra) in &rects {
        for (b, rb) in rects.iter().filter(|(b, _)| b > a) {
            let (pa, pb) = (&solids[*a], &solids[*b]);
            if !solid(&pa.prefab) || !solid(&pb.prefab) {
                continue;
            }
            if dist(ra.at, rb.at) > geom::len(ra.half) + geom::len(rb.half) + SQUEEZE {
                continue;
            }
            let gap = ra.gap(rb);
            // Between walls: a kiosk's apron is ground you walk over.
            let walls = pa.shell.unwrap_or(*ra).gap(&pb.shell.unwrap_or(*rb));
            if gap < -0.05 {
                find(
                    &mut r,
                    Rule::Overlap,
                    format!("{} and {} overlap by {:.1} m", pa.name, pb.name, -gap),
                    ra.at,
                );
            } else if (SEALED..SQUEEZE).contains(&walls) {
                find(
                    &mut r,
                    Rule::Overlap,
                    format!("{:.1} m squeeze between {} and {}", walls, pa.name, pb.name),
                    ra.at,
                );
            } else if walls < SEALED && (is_fence(&pa.prefab) != is_fence(&pb.prefab)) {
                // A fence that stops just short of a wall leaves a slit a
                // body can't take and a planner thinks it can: the sims'
                // raiders ground against every garden built so, behind
                // the house. A fence meets a wall with a gate's width.
                find(
                    &mut r,
                    Rule::Overlap,
                    format!(
                        "a {:.1} m slit between {} and {}: leave a gate ({SQUEEZE} m)",
                        walls, pa.name, pb.name
                    ),
                    ra.at,
                );
            }
        }
    }
    for (i, rc) in &rects {
        let p = &solids[*i];
        if ON_ROADS.contains(&p.prefab.as_str()) || !solid(&p.prefab) {
            continue;
        }
        for rl in &hm.road_lines {
            let half = rl.width * 0.5;
            // A road may end at a door (a path up to a tower).
            let ends = rl.points.len().saturating_sub(ROAD_END);
            let hit = rl.points.iter().enumerate().find(|(s, q)| {
                (ROAD_END..ends).contains(s) && !rl.on_bridge(*s as f32) && rc.sdf(**q) < half
            });
            if let Some((_, q)) = hit {
                find(
                    &mut r,
                    Rule::Overlap,
                    format!("{} is on the {}", p.name, map.roads[rl.road].name),
                    *q,
                );
            }
        }
    }

    for (what, at) in fences_across(&solids, &routes) {
        find(&mut r, Rule::Overlap, what, at);
    }

    // -- Anomalies off the routes ---------------------------------------------------
    let mut on_road = 0;
    let mut on_route = 0;
    let mut total = 0;
    let mut first: Option<P2> = None;
    for seed in 0..ANOMALY_SEEDS {
        for a in crate::anomaly::layout(map, hm, seed, 0) {
            total += 1;
            let road = hm.road_lines.iter().any(|rl| {
                let (d, s, _) = polyline_closest(&rl.points, a.at);
                !rl.on_bridge(s) && d < rl.width * 0.5 + a.radius
            });
            if road {
                on_road += 1;
                first.get_or_insert(a.at);
            } else if near_route(a.at, a.radius + 1.0) {
                on_route += 1;
            }
        }
    }
    r.measures.push(format!(
        "anomalies over {ANOMALY_SEEDS} surges: {total}, {on_road} reach onto a road, {on_route} onto a main route"
    ));
    // Loose anomalies fall anywhere off the roads, a cross-country route
    // included (that is what they are for); none may spill onto a road.
    let _ = on_route;
    if on_road > 0 {
        find(
            &mut r,
            Rule::Anomaly,
            format!(
                "{on_road} anomalies reach onto roads (of {total} over {ANOMALY_SEEDS} surges)"
            ),
            first.unwrap_or(ins),
        );
    }
    for m in map.markers_of(&MarkerKind::ArtifactField) {
        let d = route_pts
            .iter()
            .map(|q| dist(*q, m.at))
            .fold(f32::INFINITY, f32::min);
        if d < FIELD_CLEAR {
            find(
                &mut r,
                Rule::Anomaly,
                format!("artifact field {} is {d:.0} m from a main route", m.name),
                m.at,
            );
        }
    }

    // -- Robots: patrol space ---------------------------------------------------------
    let gates: Vec<P2> = map
        .markers_of(&MarkerKind::RobotGate)
        .map(|m| m.at)
        .collect();
    let robots = map.regions.iter().any(|g| g.danger > 0);
    for g in &gates {
        if robots && dist(*g, ins) < GATE_FROM_INSERTION * k {
            find(
                &mut r,
                Rule::Patrol,
                format!(
                    "the robot Gate is {:.0} m from the insertion",
                    dist(*g, ins)
                ),
                *g,
            );
        }
    }
    for g in gates.iter().filter(|_| robots) {
        for m in map.markers_of(&MarkerKind::Rendezvous) {
            if dist(*g, m.at) < RV_FROM_GATE * k {
                find(
                    &mut r,
                    Rule::Patrol,
                    format!("{} is {:.0} m from the robot Gate", m.name, dist(*g, m.at)),
                    m.at,
                );
            }
        }
    }
    let gate_seen = gates.first().map(|g| flood(&walk, *g));
    for m in map.markers_of(&MarkerKind::WorkSite) {
        if let Some(seen) = &gate_seen {
            if !reached_near(&walk, seen, m.at, 4.0) {
                find(
                    &mut r,
                    Rule::Patrol,
                    format!("walkers can't get from the Gate to {}", m.name),
                    m.at,
                );
            }
        }
        if dist(m.at, ins) < WORK_FROM_INSERTION * k {
            find(
                &mut r,
                Rule::Patrol,
                format!("{} is {:.0} m from the insertion", m.name, dist(m.at, ins)),
                m.at,
            );
        }
        for (rv, at) in &rvs {
            if dist(m.at, *at) < WORK_FROM_RV {
                find(
                    &mut r,
                    Rule::Patrol,
                    format!(
                        "{} is {:.0} m from the {rv} rendezvous",
                        m.name,
                        dist(m.at, *at)
                    ),
                    m.at,
                );
            }
        }
        let (mut open, mut all) = (0, 0);
        let k = (WORK_YARD / walk.cell).ceil() as i32;
        for dj in -k..=k {
            for di in -k..=k {
                let q = [
                    m.at[0] + di as f32 * walk.cell,
                    m.at[1] + dj as f32 * walk.cell,
                ];
                if dist(q, m.at) <= WORK_YARD {
                    all += 1;
                    open += usize::from(walk.walkable(q));
                }
            }
        }
        let share = open as f32 / all as f32;
        if share < WORK_OPEN {
            find(
                &mut r,
                Rule::Patrol,
                format!(
                    "{} is cramped: {:.0}% of its yard walkable",
                    m.name,
                    share * 100.0
                ),
                m.at,
            );
        }
    }

    // -- Declared ground: settle fits buildings, not into terraces -----------------
    {
        let bare = Heightmap::build_unsettled(map);
        let mut deepest = 0.0f32;
        for (poly, level) in Heightmap::terraces(map) {
            let (lo, hi) = geom::bounds(poly);
            // At the samples settle moves, not between them (a point by a
            // step's edge reads the lowered ground beside it).
            let mut worst: Option<(f32, P2)> = None;
            let cell = |v: f32| ((v - hm.origin) / hm.cell).clamp(0.0, (hm.n - 1) as f32);
            for j in cell(lo[1]).floor() as usize..=cell(hi[1]).ceil() as usize {
                for i in cell(lo[0]).floor() as usize..=cell(hi[0]).ceil() as usize {
                    let p = hm.pos(i, j);
                    // Under a building stepped down the terraces, the
                    // ground is its bays' (`terrain::settle` keeps it under
                    // their floors), not the terrace's.
                    if Heightmap::on_terrace(poly, p) && !crate::map::under_stepped(map, p) {
                        let k = hm.idx(i, j);
                        // Below its level: what a road banked on top of
                        // it is the road's.
                        let cut = bare.heights[k].min(level) - hm.heights[k];
                        if worst.is_none_or(|(c, _)| cut > c) {
                            worst = Some((cut, p));
                        }
                    }
                }
            }
            let Some((cut, at)) = worst else { continue };
            deepest = deepest.max(cut);
            if cut > TERRACE_CUT {
                let by = map
                    .placements
                    .iter()
                    .filter(|p| crate::map::prefab_wants_pad(&p.prefab))
                    .min_by(|a, b| dist(a.at, at).total_cmp(&dist(b.at, at)))
                    .map_or("?", |p| p.prefab.as_str());
                find(
                    &mut r,
                    Rule::Footing,
                    format!(
                        "settling the {by} cuts {cut:.1} m into the terrace the map sets at {level:.1} m"
                    ),
                    at,
                );
            }
        }
        r.measures.push(format!(
            "terraces: {} declared, deepest cut settling buildings in {deepest:.2} m",
            Heightmap::terraces(map).count()
        ));
    }

    // -- Tight, broken, quiet between (map_openness) -------------------------------
    {
        use crate::map_openness as mo;
        let o = mo::openness(map, hm, &walk);
        let pull = mo::pull(map, hm);
        let mean = pull.iter().map(|p| p.2).sum::<usize>() as f32 / pull.len().max(1) as f32;
        let places = mo::pull_at_places(map, hm);
        let empty = mo::empty_walks(map, &comfy);
        r.measures.push(format!(
            "openness: a standing figure seen {:.0} m (median), {:.0} m (p90); {:.0}% of the open ground seen past a walker's {:.0} m; a fight at a hot spot heard by {mean:.1} others; places heard by: {}; longest empty walk {:.0} m",
            o.median,
            o.p90,
            o.open_share * 100.0,
            mo::OPEN_REACH,
            places
                .iter()
                .map(|(n, _, k)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", "),
            empty.iter().map(|e| e.2).fold(0.0, f32::max)
        ));
        if mo::TIGHT.contains(&map.name.as_str()) {
            if o.open_share > mo::OPEN_MAX {
                find(
                    &mut r,
                    Rule::Landmark,
                    format!(
                        "{:.0}% of the open ground is seen past a walker's sight (at most {:.0}%): break the sightlines",
                        o.open_share * 100.0,
                        mo::OPEN_MAX * 100.0
                    ),
                    ins,
                );
            }
            if mean > mo::PULL_MEAN {
                find(
                    &mut r,
                    Rule::Patrol,
                    format!(
                        "a fight at a hot spot is heard by {mean:.1} others on average (at most {:.1}): put rises between them",
                        mo::PULL_MEAN
                    ),
                    ins,
                );
            }
            let danger = |p: P2| {
                map.regions
                    .iter()
                    .filter(|reg| geom::point_in_polygon(&reg.polygon, p))
                    .map(|reg| reg.danger)
                    .max()
                    .unwrap_or(0)
            };
            for (name, at, k) in &places {
                if danger(*at) <= 1 && *k > mo::QUIET_PULL {
                    find(
                        &mut r,
                        Rule::Patrol,
                        format!(
                            "a fight at {name}, a quiet place, is heard by {k} hot spots (at most {})",
                            mo::QUIET_PULL
                        ),
                        *at,
                    );
                }
            }
            for reg in map.regions.iter().filter(|reg| reg.danger >= 2) {
                if reg.notice.is_none_or(|n| n > mo::NOTICE_MAX) {
                    find(
                        &mut r,
                        Rule::Patrol,
                        format!(
                            "{}, a hot zone, lets its machines notice you from any range: cap it (notice, at most {:.0} m)",
                            reg.name,
                            mo::NOTICE_MAX
                        ),
                        reg.polygon[0],
                    );
                }
            }
            for (a, b, len, at) in &empty {
                if *len > mo::EMPTY_MAX {
                    find(
                        &mut r,
                        Rule::Spacing,
                        format!(
                            "{len:.0} m of the walk from {a} to {b} has nothing to stop at (at most {:.0})",
                            mo::EMPTY_MAX
                        ),
                        *at,
                    );
                }
            }
        }
    }

    // -- Route choice ---------------------------------------------------------------
    for (name, at) in &rvs {
        if dist(*at, ins) < RV_FAR * k {
            find(
                &mut r,
                Rule::Choice,
                format!("{name} is {:.0} m from the insertion", dist(*at, ins)),
                *at,
            );
        }
        if let Some((_, rc)) = rects.iter().find(|(_, rc)| rc.sdf(*at) < TRUCK_ROOM) {
            find(
                &mut r,
                Rule::Footing,
                format!(
                    "no room for the truck at {name}: a building {:.0} m off",
                    rc.sdf(*at).max(0.0)
                ),
                *at,
            );
        }
        // The truck parks with its bed toward the map's middle
        // (`raid::truck::rv_pose`); you climb in from behind it.
        let back = {
            let l = (at[0] * at[0] + at[1] * at[1]).sqrt().max(1e-3);
            [-at[0] / l, -at[1] / l]
        };
        // Both where you stand to climb in and where the bots line up for
        // it (`sim::raider::extract_step`) must be walkable.
        for (what, d) in [("tailgate", TAILGATE), ("approach", TRUCK_APPROACH)] {
            let step = [at[0] + back[0] * d, at[1] + back[1] * d];
            let open = walk
                .cell_of(step)
                .is_some_and(|(i, j)| walk_seen[j * walk.n + i]);
            if !open {
                find(
                    &mut r,
                    Rule::Footing,
                    format!("the truck's {what} at {name} can't be walked to"),
                    step,
                );
            }
        }
        if hm.slope_deg(*at) > RV_LEVEL {
            find(
                &mut r,
                Rule::Footing,
                format!(
                    "the truck parks on a {:.0}° slope at {name}",
                    hm.slope_deg(*at)
                ),
                *at,
            );
        }
    }
    for (i, (a, pa)) in rvs.iter().enumerate() {
        for (b, pb) in rvs.iter().skip(i + 1) {
            if dist(*pa, *pb) < RV_APART * k {
                find(
                    &mut r,
                    Rule::Choice,
                    format!("{a} and {b} are {:.0} m apart", dist(*pa, *pb)),
                    *pa,
                );
            }
        }
    }
    for route in &routes {
        let end = *route.points.last().unwrap();
        let mut g = gentle.clone();
        let mut corridor = Vec::new();
        for p in route.points.iter().step_by(2) {
            if dist(*p, ins) < CHOICE_ENDS || dist(*p, end) < CHOICE_ENDS {
                continue;
            }
            let k = (CHOICE_APART / g.cell).ceil() as i32;
            if let Some((ci, cj)) = g.cell_of(*p) {
                for dj in -k..=k {
                    for di in -k..=k {
                        let (i, j) = (ci as i32 + di, cj as i32 + dj);
                        if i < 0 || j < 0 || i >= g.n as i32 || j >= g.n as i32 {
                            continue;
                        }
                        let (i, j) = (i as usize, j as usize);
                        if dist(g.centre(i, j), *p) <= CHOICE_APART {
                            g.cost[j * g.n + i] = BLOCKED;
                            corridor.push(j * g.n + i);
                        }
                    }
                }
            }
        }
        let alt = g.path(ins, end).map(|p| path_length(&p));
        r.measures.push(format!(
            "route to {}: other way {}",
            route.to,
            alt.map_or("none".into(), |l| format!(
                "{l:.0} m ({:.2}x)",
                l / route.length
            ))
        ));
        if alt.is_none_or(|l| l > route.length * CHOICE_DETOUR) {
            // Say what cuts the other way off: too long on gentle ground, or
            // only there over steep ground (point at its steepest cell), or
            // not there at all.
            let mid = route.points[route.points.len() / 2];
            let (why, at) = if let Some(l) = alt {
                (
                    format!("; the other way is {l:.0} m ({:.2}x)", l / route.length),
                    mid,
                )
            } else {
                let mut w = walk.clone();
                for c in &corridor {
                    w.cost[*c] = BLOCKED;
                }
                match w.path(ins, end) {
                    Some(p) => {
                        // The ground along that way only the gentle grid refuses
                        // (too steep for it), sampled every metre.
                        let mut cut: Vec<(usize, usize)> = p
                            .windows(2)
                            .flat_map(|ab| {
                                let n = dist(ab[0], ab[1]).ceil().max(1.0) as usize;
                                (0..n).map(move |k| {
                                    let t = k as f32 / n as f32;
                                    [
                                        ab[0][0] + (ab[1][0] - ab[0][0]) * t,
                                        ab[0][1] + (ab[1][1] - ab[0][1]) * t,
                                    ]
                                })
                            })
                            .filter_map(|q| gentle.cell_of(q))
                            .filter(|(i, j)| {
                                gentle.cost[j * gentle.n + i] == BLOCKED
                                    && walk.cost[j * walk.n + i] != BLOCKED
                            })
                            .collect();
                        cut.dedup();
                        let cut: Vec<[f32; 2]> =
                            cut.iter().map(|(i, j)| gentle.centre(*i, *j)).collect();
                        let steep = cut
                            .iter()
                            .copied()
                            .max_by(|a, b| hm.slope_deg(*a).total_cmp(&hm.slope_deg(*b)))
                            .unwrap_or(mid);
                        (
                            format!(
                                "; there is one only over ground steeper than {ROUTE_GRADE}° ({:.0} m of it, worst {:.0}° here)",
                                cut.len() as f32 * gentle.cell,
                                hm.slope_deg(steep)
                            ),
                            steep,
                        )
                    }
                    None => ("; none on any walkable ground either".into(), mid),
                }
            };
            find(
                &mut r,
                Rule::Choice,
                format!("no second way to {} within {CHOICE_DETOUR}x{why}", route.to),
                at,
            );
        }
    }

    // -- Sentries ----------------------------------------------------------------
    {
        let named: Vec<(String, Vec<P2>)> = routes
            .iter()
            .map(|rt| (rt.to.clone(), rt.points.clone()))
            .collect();
        let c = crate::map_sentry::check(map, hm, &walk, ins, &named, |p| inside_fence(map, p));
        r.measures.extend(c.measures);
        for (what, at) in c.findings {
            find(&mut r, Rule::Sentry, what, at);
        }
        let c = crate::map_spotter::check(map, hm, &walk, ins, &named, |p| inside_fence(map, p));
        r.measures.extend(c.measures);
        for (what, at) in c.findings {
            find(&mut r, Rule::Spotter, what, at);
        }
        let c = crate::map_mines::check(map, hm, &walk, &named);
        r.measures.extend(c.measures);
        for (what, at) in c.findings {
            find(&mut r, Rule::Mines, what, at);
        }
    }

    // -- Doors -----------------------------------------------------------------------
    let dc = crate::map_doors::check(map, hm, &walk, &walk_seen);
    r.measures.push(format!(
        "doors: {}, {} findings, worst step to the ground {:.2} m, {} a walker can't step ({WALKER_STEP} m)",
        dc.doors,
        dc.findings.len(),
        dc.worst_step,
        dc.walker_blocked
    ));
    for (what, at) in dc.findings {
        find(&mut r, Rule::Door, what, at);
    }

    // -- Pockets ----------------------------------------------------------------------
    {
        let p = crate::map_pockets::pockets(&walk, hm, ins, crate::map_nav::MAX_SLOPE, |p| {
            inside_fence(map, p)
        });
        r.measures.push(format!(
            "pockets: {} false steps on the walkers' grid; pockets {}",
            p.false_steps,
            p.pockets
                .iter()
                .take(6)
                .map(|(a, at)| format!("{a:.0} m² at ({:.0}, {:.0})", at[0], at[1]))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for (area, at) in p
            .pockets
            .iter()
            .filter(|(a, _)| *a >= crate::map_pockets::POCKET_MIN)
        {
            find(
                &mut r,
                Rule::Pocket,
                format!(
                    "{area:.0} m² the walkers' grid reaches only over a bank too steep to walk"
                ),
                *at,
            );
        }
    }

    // -- Render load -----------------------------------------------------------------
    let views = route_pts
        .iter()
        .step_by(5)
        .copied()
        .chain(pois.iter().map(|p| p.1));
    let mut worst = (0, ins);
    let mut light_most = 0;
    for p in views {
        // A kiosk is a building to draw like any other; a light prop isn't.
        let (light, n): (Vec<&Solid>, Vec<&Solid>) = solids
            .iter()
            .filter(|q| dist(q.rect.at, p) <= VIEW_RANGE * k)
            .partition(|q| VIEW_LIGHT.contains(&q.prefab.as_str()));
        light_most = light_most.max(light.len());
        if n.len() > worst.0 {
            worst = (n.len(), p);
        }
    }
    r.measures.push(format!(
        "render load: at most {} placements within {VIEW_RANGE} m, and up to {light_most} light props ({})",
        worst.0,
        VIEW_LIGHT.join(", ")
    ));
    if worst.0 > VIEW_MAX {
        find(
            &mut r,
            Rule::ViewLoad,
            format!("{} placements within {VIEW_RANGE} m", worst.0),
            worst.1,
        );
    }

    // -- The Surge: a roof in reach, and its anomalies off the walls -------------
    crate::map_surge::check(map, hm, &comfy, &comfy_seen, &routes, &targets, &mut r);

    r.routes = routes;
    r.findings.sort_by_key(|f| f.rule);
    r
}

/// The fall of the ground under a truck standing at `at` facing `yaw`:
/// its front axle's ground less its rear axles', over their 4.7 m, as an
/// angle (radians; `raid::truck::parked`).
pub fn truck_fall(hm: &Heightmap, at: P2, yaw: f32) -> f32 {
    let (s, c) = yaw.sin_cos();
    let ground = |x: f32, z: f32| hm.height([at[0] + x * c + z * s, at[1] - x * s + z * c]);
    let axle = |z: f32| (ground(-1.0, z) + ground(1.0, z)) * 0.5;
    (axle(-3.9) - axle(0.8)).atan2(4.7)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{CINDER_WORKS, TRAINING_YARD};

    fn run(name: &str, text: &str) -> Report {
        let map = MapDef::from_ron(text).unwrap();
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        println!("{}", r.text());
        if let Ok(dir) = std::env::var("LINT_DUMP") {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(format!("{dir}/{name}_routes.json"), r.routes_json());
        }
        r
    }

    /// Every valley on [`crate::map::VALLEYS`] lints clean: a new valley is
    /// held to the guide the moment it's listed. They lint side by side.
    #[test]
    fn every_valley_passes_the_map_lint() {
        let reports: Vec<Report> = std::thread::scope(|s| {
            let jobs: Vec<_> = crate::map::VALLEYS
                .iter()
                .map(|(name, text)| s.spawn(move || run(name, text)))
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap()).collect()
        });
        let dirty: Vec<String> = reports
            .iter()
            .filter(|r| !r.findings.is_empty())
            .map(Report::text)
            .collect();
        assert!(
            dirty.is_empty(),
            "valleys break the map design rules:\n{}",
            dirty.join("\n")
        );
    }

    /// The paper map (`scripts/field_map.py`) draws every landmark as a
    /// tower with its lamp, named: it must take the same height to call
    /// something a landmark, draw it big enough to pick out (3.5 mm on the
    /// sheet), and have a name for every landmark the maps place.
    #[test]
    fn field_map_draws_the_landmarks() {
        let src = include_str!("../../../scripts/field_map.py");
        let num = |name: &str| -> f32 {
            src.lines()
                .find_map(|l| l.strip_prefix(&format!("{name} = ")))
                .and_then(|v| v.split('#').next()?.trim().parse().ok())
                .unwrap_or_else(|| panic!("field_map.py has no plain `{name} = <number>` line"))
        };
        assert_eq!(num("LANDMARK_M"), LANDMARK_HEIGHT);
        assert!(num("LANDMARK_MM") >= 3.5, "landmark glyph under 3.5 mm");
        let mut unnamed = Vec::new();
        for (name, text) in crate::map::VALLEYS
            .iter()
            .map(|(n, t)| (*n, *t))
            .chain([("training_yard", TRAINING_YARD)])
        {
            let map = MapDef::from_ron(text).unwrap();
            for p in &map.placements {
                let base = crate::map::prefab_base(&p.prefab);
                if prefab_footprint(&p.prefab)[1] >= LANDMARK_HEIGHT
                    && !src.contains(&format!("    \"{base}\": \""))
                {
                    unnamed.push(format!("{name}: {base}"));
                }
            }
        }
        assert!(
            unnamed.is_empty(),
            "landmarks with no name in field_map.py's LANDMARK_NAMES:\n{}",
            unnamed.join("\n")
        );
        // Each landmark's lamp comes from the dump (map-dump's placements'
        // `lamp`, the game's own assignment), the beacons' from its table.
        assert!(
            src.contains("p.get(\"lamp\""),
            "field_map.py doesn't read the dump's lamps"
        );
        for i in 0..4 {
            let want = crate::map_landmarks::beacon_character(i).name;
            assert!(
                src.contains(&format!("\"{want}\"")),
                "field_map.py's BEACON_CHARACTERS lacks {want}"
            );
        }
    }

    /// A valley whose early places all lie one way offers no choice: strip
    /// Marrow's west of the drop and the first ten minutes are one walk.
    #[test]
    fn one_way_out_of_the_drop_is_found() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        map.markers
            .retain(|m| !(matches!(m.kind, MarkerKind::Poi) && m.at[0] < 40.0));
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Choice && f.what.contains("first ten minutes")),
            "{}",
            r.text()
        );
    }

    /// With only one rendezvous, every rich place has one way home; and a
    /// spotter over a rendezvous watches the last stretch to it.
    #[test]
    fn one_way_home_and_a_watched_rendezvous_are_found() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let mut seen = 0;
        map.markers.retain(|m| {
            if matches!(m.kind, MarkerKind::Rendezvous) {
                seen += 1;
                seen == 1
            } else {
                true
            }
        });
        let rv = map.markers_of(&MarkerKind::Rendezvous).next().unwrap().at;
        map.markers.push(crate::map::Marker {
            kind: MarkerKind::Spotter,
            at: [rv[0] + 20.0, rv[1] + 20.0],
            rot: 0.0,
            name: "Test spotter".into(),
            y: 0.0,
        });
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings.iter().any(|f| f.what.contains("one way home")),
            "{}",
            r.text()
        );
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Spotter && f.what.contains("last")),
            "{}",
            r.text()
        );
    }

    /// Take away the light cover round Marrow's creek bridge and the first
    /// job's way to the radio mast runs in the open.
    #[test]
    fn a_first_job_walk_in_the_open_is_found() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        map.placements.retain(|p| {
            !(dist(p.at, [0.0, -10.0]) < 45.0 && VIEW_LIGHT.contains(&p.prefab.as_str()))
        });
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Cover && f.what.contains("first job plant")),
            "{}",
            r.text()
        );
    }

    /// The playtest of 28 October found Marrow "a couple of points in an
    /// otherwise empty plane", seen from too far, its fights heard by every
    /// machine. Undo each fix and its rule finds it: the rises by the
    /// creek's banks, the ford and the Hollow's rim; the hot zones' capped
    /// notice; the quiet places kept clear of work; the roofs and loot
    /// between the places.
    #[test]
    fn marrow_undone_is_open_loud_and_empty() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        map.features.retain(|f| {
            !matches!(f, crate::map::TerrainFeature::Ridge { points, .. }
                if [[-84.0, -54.0], [26.0, -36.0], [56.0, -14.0], [60.0, -64.0], [66.0, 46.0]]
                    .contains(&points[0]))
        });
        for reg in &mut map.regions {
            reg.notice = None;
        }
        let pois: Vec<P2> = map.markers_of(&MarkerKind::Poi).map(|m| m.at).collect();
        for at in pois {
            map.markers.push(crate::map::Marker {
                kind: MarkerKind::WorkSite,
                at: [at[0] + 25.0, at[1]],
                rot: 0.0,
                name: String::new(),
                y: 0.0,
            });
        }
        map.placements
            .retain(|p| !crate::map_surge::roofed(&p.prefab));
        map.markers
            .retain(|m| !matches!(m.kind, MarkerKind::Loot { .. }));
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        for what in [
            "seen past a walker's sight",
            "others on average",
            "a quiet place",
            "nothing to stop at",
            "notice you from any range",
        ] {
            assert!(
                r.findings.iter().any(|f| f.what.contains(what)),
                "{what}: {}",
                r.text()
            );
        }
    }

    /// The designer's first raid in Marrow went up the road to the Village,
    /// past the Old farm's hidden turn, by way of a shed and the cars by
    /// it: without the farm track the easy way to the career's first job
    /// heads north, off its bearing; with a shed by the road again, it
    /// draws a newcomer off.
    #[test]
    fn the_first_job_is_the_obvious_way_from_the_drop() {
        let base = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let check = |map: &MapDef, what: &str| {
            let hm = Heightmap::build(map);
            let r = lint(map, &hm);
            assert!(
                r.findings
                    .iter()
                    .any(|f| f.rule == Rule::Choice && f.what.contains(what)),
                "{what}: {}",
                r.text()
            );
        };
        let mut map = base.clone();
        map.roads.retain(|r| r.name != "Farm track");
        check(&map, "turns");
        let mut map = base.clone();
        map.placements.push(crate::map::Placement {
            prefab: "shed".into(),
            at: [4.0, 102.5],
            rot: 90.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        check(&map, "draws a newcomer off");
    }

    /// Take away the bus shelter at Marrow's Bus depot and the wait for
    /// the truck there, under the depot drones' orbit, has nothing to
    /// wait behind.
    #[test]
    fn a_wait_under_the_drones_without_shelter_is_found() {
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        map.placements.retain(|p| p.tag != "bus_shelter");
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Cover && f.what.contains("to wait behind at Bus depot")),
            "{}",
            r.text()
        );
    }

    /// Every wreck is a light prop (`VIEW_LIGHT` names them again).
    #[test]
    fn every_wreck_is_a_light_prop() {
        for w in WRECKS {
            assert!(VIEW_LIGHT.contains(w), "{w} missing from VIEW_LIGHT");
        }
    }

    /// The Training Yard belongs to onboarding: its findings are printed
    /// for them, and only the rules that would break the tutorial hold.
    #[test]
    fn training_yard_map_lint() {
        let r = run("training_yard", TRAINING_YARD);
        for rule in [Rule::Reach, Rule::Footing, Rule::Overlap] {
            assert_eq!(r.count(rule), 0, "{}", r.text());
        }
    }

    /// Landmarks are whole structures of a kind: a chapel's tower and a
    /// steeple in one view are two you can't tell apart, two masts side by
    /// side are one; and a rendezvous with a landmark by it is found.
    #[test]
    fn landmarks_are_structures_and_rendezvous_have_one() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        let lone = lint(&map, &Heightmap::build(&map));
        assert!(
            lone.findings
                .iter()
                .any(|f| f.rule == Rule::Landmark && f.what.contains("North post")),
            "{}",
            lone.text()
        );
        for (prefab, at) in [
            ("chapel", [-60.0, 0.0]),
            ("steeple", [60.0, 0.0]),
            ("radio_mast", [-20.0, -60.0]),
            ("radio_mast", [-30.0, -60.0]),
        ] {
            map.placements.push(crate::map::Placement {
                prefab: prefab.into(),
                at,
                rot: 0.0,
                y: 0.0,
                absolute: false,
                tag: String::new(),
            });
        }
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        let landmark = |s: &str| {
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Landmark && f.what.contains(s))
        };
        assert!(landmark("two steeples"), "{}", r.text());
        assert!(!landmark("two radio_masts"), "{}", r.text());
        assert!(!landmark("within 200 m of North post"), "{}", r.text());
    }

    #[test]
    fn rects_measure_their_gap() {
        let a = Rect::of("shed", [0.0, 0.0], 0.0); // 4 x 3
        let b = Rect::of("shed", [5.0, 0.0], 0.0);
        assert!((a.gap(&b) - 1.0).abs() < 0.05, "{}", a.gap(&b));
        let c = Rect::of("shed", [3.0, 0.0], 0.0);
        assert!(a.gap(&c) < -0.9);
        // Turned a quarter, a 4 x 3 shed is 3 wide along x.
        let d = Rect::of("shed", [4.0, 0.0], 90.0);
        assert!((a.gap(&d) - 0.5).abs() < 0.05, "{}", a.gap(&d));
    }

    #[test]
    fn a_walled_off_place_is_unreachable() {
        let mut map = MapDef::from_ron(TRAINING_YARD).unwrap();
        // A ring of sheds round the practice house.
        let c = [28.0, 18.0];
        for k in 0..16 {
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            map.placements.push(crate::map::Placement {
                prefab: "shed".into(),
                at: [c[0] + a.cos() * 11.0, c[1] + a.sin() * 11.0],
                rot: 90.0 - a.to_degrees(),
                y: 0.0,
                absolute: false,
                tag: String::new(),
            });
        }
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Reach && f.what.contains("house_plank")),
            "{}",
            r.text()
        );
    }

    /// Marrow Creek's depot exit: a kiosk 115 m from its site's placement.
    fn marrow_exit() -> (MapDef, P2) {
        let map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let exit = crate::underground::placed(&map)
            .iter()
            .find(|s| s.id == "depot_marrow")
            .unwrap()
            .exits()[0]
            .0;
        (map, exit)
    }

    #[test]
    fn a_building_on_an_exit_kiosk_is_found() {
        let (mut map, exit) = marrow_exit();
        map.placements.push(crate::map::Placement {
            prefab: "shed".into(),
            at: [exit[0] + 2.0, exit[1]],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings.iter().any(|f| f.rule == Rule::Overlap
                && f.what.contains("depot_marrow exit")
                && f.what.contains("shed")),
            "{}",
            r.text()
        );
    }

    #[test]
    fn an_exit_kiosk_in_the_water_is_found() {
        let (mut map, exit) = marrow_exit();
        map.water.push(crate::map::WaterDef {
            name: "test ditch".into(),
            points: vec![[exit[0] - 20.0, exit[1]], [exit[0] + 20.0, exit[1]]],
            width: 6.0,
            // The exit's pad is levelled into a hillside: deep enough to
            // stand over it.
            depth: 4.0,
        });
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Footing && f.what.contains("depot_marrow exit")),
            "{}",
            r.text()
        );
    }

    /// A garden fence across a door, a road or a main route is found, though
    /// a run is only 0.2 m deep: thinner than the door strip's samples were
    /// apart and than the nav grid's cells.
    #[test]
    fn a_fence_across_a_door_a_road_or_a_route_is_found() {
        let fence = |at: P2, rot: f32| crate::map::Placement {
            prefab: "fence_garden".into(),
            at,
            rot,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        };
        let mut map = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        // The village's north house, turned 90: its door faces east, its
        // face 3.59 m out. A run along z 2.125 m in front of it, between
        // two rows of 0.25 m samples.
        let house = map
            .placements
            .iter()
            .find(|p| p.prefab == "house_plank" && p.at == [-20.2, 63.5] && p.rot == 90.0)
            .expect("the village's north house moved");
        let face = [house.at[0] + 3.59, house.at[1]];
        map.placements.push(fence([face[0] + 2.125, face[1]], 90.0));
        // Across the district road in the village, which runs north-south.
        map.placements.push(fence([0.3, 28.5], 0.0));
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        let found = |rule: Rule, s: &str| {
            r.findings
                .iter()
                .any(|f| f.rule == rule && f.what.contains(s))
        };
        assert!(
            found(Rule::Door, "fence_garden stands in front of it"),
            "{}",
            r.text()
        );
        assert!(
            found(Rule::Overlap, "fence_garden is on the District road"),
            "{}",
            r.text()
        );
        // Routes are planned round footprints, so lay one across a route as
        // planned, square to it, between two of its samples.
        let route = &r.routes[0];
        let k = route.points.len() / 2;
        let (a, b) = (route.points[k], route.points[k + 1]);
        let along = geom::sub(b, a);
        let rot = along[1].atan2(along[0]).to_degrees() + 90.0;
        map.placements.push(fence(geom::lerp(a, b, 0.5), rot));
        let across = fences_across(&solids(&map), &r.routes);
        assert!(
            across
                .iter()
                .any(|(what, _)| what.ends_with(&format!("the route to {}", route.to))),
            "{across:?}"
        );
    }

    /// A wreck on the carriageway is on the road; one across a hillside
    /// stands with its high side buried; and neither is levelled for, nor
    /// is the ground round it settled: the land is as it was without it.
    #[test]
    fn a_wreck_on_a_road_or_a_slope_is_found() {
        let wreck = |prefab: &str, at: P2, rot: f32| crate::map::Placement {
            prefab: prefab.into(),
            at,
            rot,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        };
        let bare = MapDef::from_ron(crate::map::MARROW_CREEK).unwrap();
        let mut map = bare.clone();
        // On the district road in the village, which runs north-south.
        map.placements.push(wreck("wreck_lada", [0.3, 28.5], 0.0));
        // Across the side of the radio mast's hill, 17° there.
        map.placements
            .push(wreck("wreck_lorry", [-47.0, -125.0], 90.0));
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        let found = |rule: Rule, s: &str| {
            r.findings
                .iter()
                .any(|f| f.rule == rule && f.what.contains(s))
        };
        assert!(
            found(Rule::Overlap, "wreck_lada is on the District road"),
            "{}",
            r.text()
        );
        assert!(
            found(Rule::Footing, "the wreck_lorry at (-47, -125) has ground"),
            "{}",
            r.text()
        );
        let was = Heightmap::build(&bare);
        for at in [[-47.0, -125.0], [-47.0, -131.0], [-37.0, -125.0]] {
            assert_eq!(hm.height(at), was.height(at), "the ground moved at {at:?}");
        }
    }

    #[test]
    fn a_blocked_tailgate_is_found() {
        let mut map = MapDef::from_ron(CINDER_WORKS).unwrap();
        // A barn across the way in, behind the truck at an RV.
        let rv = map
            .markers_of(&MarkerKind::Rendezvous)
            .next()
            .unwrap()
            .clone();
        let l = (rv.at[0] * rv.at[0] + rv.at[1] * rv.at[1]).sqrt();
        let back = [-rv.at[0] / l, -rv.at[1] / l];
        map.placements.push(crate::map::Placement {
            prefab: "barn".into(),
            at: [rv.at[0] + back[0] * TAILGATE, rv.at[1] + back[1] * TAILGATE],
            rot: 0.0,
            y: 0.0,
            absolute: false,
            tag: String::new(),
        });
        let hm = Heightmap::build(&map);
        let r = lint(&map, &hm);
        assert!(
            r.findings
                .iter()
                .any(|f| f.rule == Rule::Footing && f.what.contains("tailgate")),
            "{}",
            r.text()
        );
    }
}
