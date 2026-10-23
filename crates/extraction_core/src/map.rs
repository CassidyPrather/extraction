//! The map file format: a hand-authored description of a Zone.
//!
//! A map is designed, not generated. This file says where the hills are,
//! where the creek runs, where the roads go and which buildings sit where.
//! The game, the headless sims and the preview tool all build from it.
//! Coordinates are metres in `[x, z]` with the origin at the map centre,
//! `x` east and `+z` south (so north is `-z`, as in Bevy).

use crate::geom::P2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Side length of the square map, metres.
    pub size: f32,
    /// Heightmap cell size, metres.
    #[serde(default = "default_cell")]
    pub cell: f32,
    #[serde(default)]
    pub base_height: f32,
    #[serde(default)]
    pub noise: NoiseDef,
    /// The raised rim that keeps the playable area in.
    #[serde(default)]
    pub rim: RimDef,
    /// Distance from the map edge to the quarantine fence (0: no fence).
    #[serde(default)]
    pub fence_inset: f32,
    #[serde(default)]
    pub features: Vec<TerrainFeature>,
    #[serde(default)]
    pub roads: Vec<RoadDef>,
    #[serde(default)]
    pub water: Vec<WaterDef>,
    #[serde(default)]
    pub forests: Vec<ForestDef>,
    #[serde(default)]
    pub placements: Vec<Placement>,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub regions: Vec<Region>,
    /// The valley's own air: a colour the sky and fog are multiplied by
    /// (linear rgb; none is neutral).
    #[serde(default)]
    pub haze: Option<[f32; 3]>,
    /// Ash drifting down when it isn't raining, 0..1.
    #[serde(default)]
    pub ash: f32,
    /// How cold the valley is, 0..=1: up high in the wind a body chills
    /// (`cold`). Most valleys aren't.
    #[serde(default)]
    pub cold: f32,
    /// How much of the valley is scree, 0..=1: rock fields everywhere
    /// rather than where the slow noise puts them (`rocks`). Bare high
    /// ground is all stone.
    #[serde(default)]
    pub scree: f32,
    /// Where snow can lie (m): above it, falling weather comes as snow
    /// and lies on what faces up. None: the valley's too low
    /// (`world::snow`).
    #[serde(default)]
    pub snow_line: Option<f32>,
    /// Snow that lies above the line whatever the weather, 0..1: a high,
    /// cold valley's snowfields. Falling snow adds to it.
    #[serde(default)]
    pub snow: f32,
    /// What the valley sounds like.
    #[serde(default)]
    pub sounds: SoundsDef,
}

/// A valley's ambience: sound set ids from `assets/audio/sounds.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundsDef {
    /// The wind bed, always on.
    #[serde(default = "default_wind")]
    pub wind: String,
    /// Birds calling now and then out in the fog.
    #[serde(default = "default_birds")]
    pub birds: Vec<String>,
    /// Now and then, far off (rockfall in the mountains).
    #[serde(default)]
    pub distant: Vec<String>,
}

impl Default for SoundsDef {
    fn default() -> Self {
        Self {
            wind: default_wind(),
            birds: default_birds(),
            distant: Vec::new(),
        }
    }
}

fn default_wind() -> String {
    "ambience/wind_conifers".into()
}

fn default_birds() -> Vec<String> {
    vec!["ambience/crow".into()]
}

const fn default_cell() -> f32 {
    1.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoiseDef {
    pub seed: u32,
    /// Peak-ish amplitude in metres.
    pub amplitude: f32,
    /// Wavelength of the largest octave, metres.
    pub scale: f32,
    pub octaves: u32,
    /// Small-scale bumpiness on top, metres.
    #[serde(default)]
    pub detail: f32,
    /// How much natural relief goes on top, as a share of `amplitude`:
    /// patches of knolls and hummocks. 0 leaves the plain noise.
    #[serde(default = "one")]
    pub relief: f32,
    /// How hard rain has worn the land (gullies down the slopes, the silt
    /// spread at their feet). 0 leaves it unworn.
    #[serde(default = "one")]
    pub erosion: f32,
}

const fn one() -> f32 {
    1.0
}

impl Default for NoiseDef {
    fn default() -> Self {
        Self {
            seed: 1,
            amplitude: 3.0,
            scale: 120.0,
            octaves: 5,
            detail: 0.25,
            relief: 1.0,
            erosion: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RimDef {
    /// How far in from the edge the rise starts.
    pub width: f32,
    pub height: f32,
}

impl Default for RimDef {
    fn default() -> Self {
        Self {
            width: 40.0,
            height: 18.0,
        }
    }
}

/// Shapes the land. Features add together, then plateaus flatten.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TerrainFeature {
    /// A smooth bump (or, with negative height, a hollow).
    Hill { at: P2, radius: f32, height: f32 },
    /// A raised spine along a spline.
    Ridge {
        points: Vec<P2>,
        width: f32,
        height: f32,
    },
    /// A carved valley or creek bed along a spline.
    Channel {
        points: Vec<P2>,
        width: f32,
        depth: f32,
    },
    /// Flattens the land inside a polygon: to `height` if given, otherwise
    /// to the average height already there (a building pad).
    Plateau {
        polygon: Vec<P2>,
        #[serde(default)]
        height: Option<f32>,
        blend: f32,
    },
    /// A plane tilt across the whole map: metres of rise per 100 m along
    /// `dir`.
    Tilt { dir: P2, rise_per_100m: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoadKind {
    Asphalt,
    Gravel,
    Dirt,
    /// Rail bed: ballast with sleepers and rails on top.
    Rail,
    /// A worn footpath.
    Path,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoadDef {
    #[serde(default)]
    pub name: String,
    pub kind: RoadKind,
    pub points: Vec<P2>,
    pub width: f32,
    /// Blend distance beyond the edge.
    #[serde(default = "default_shoulder")]
    pub shoulder: f32,
    /// Metres of centreline the grade is averaged over: larger is gentler
    /// (rail beds), smaller hugs the land (tracks, fords).
    #[serde(default = "default_smoothing")]
    pub smoothing: f32,
    /// Spans where the road leaves the ground: the terrain underneath is
    /// left alone and the game builds a bridge deck instead.
    #[serde(default)]
    pub bridges: Vec<BridgeDef>,
    /// Spans where the road runs under an avalanche gallery: a concrete
    /// roof on the uphill side's wall, open to the valley (on columns along
    /// the downhill edge). Same form as `bridges`: centre `at` (snapped to
    /// the road) and `length`. The road under it is graded as usual; the
    /// gallery adds nothing to the nav grid (its wall stands on the cut
    /// bank beyond the shoulder, its columns beyond the road's edge).
    #[serde(default)]
    pub galleries: Vec<BridgeDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeDef {
    /// Centre of the span (snapped to the road).
    pub at: P2,
    /// Deck length, metres.
    pub length: f32,
}

const fn default_shoulder() -> f32 {
    3.0
}
const fn default_smoothing() -> f32 {
    20.0
}

/// Standing water along a channel: a creek, a pond, a flooded ditch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterDef {
    #[serde(default)]
    pub name: String,
    pub points: Vec<P2>,
    pub width: f32,
    /// Water surface height above the lowest bed point nearby.
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TreeSpecies {
    Spruce,
    Pine,
    Birch,
    /// A dead snag: grey, barkless, broken top.
    Snag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForestDef {
    #[serde(default)]
    pub name: String,
    pub polygon: Vec<P2>,
    /// Trees per 100 m².
    pub density: f32,
    pub mix: Vec<(TreeSpecies, f32)>,
    /// 0..=1 bushes and ferns between trees.
    #[serde(default = "default_undergrowth")]
    pub undergrowth: f32,
    /// Scatter seed.
    #[serde(default)]
    pub seed: u32,
}

const fn default_undergrowth() -> f32 {
    0.5
}

/// A prefab (building, prop, robot structure) placed in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    pub prefab: String,
    pub at: P2,
    /// Yaw in degrees, clockwise from north looking down.
    #[serde(default)]
    pub rot: f32,
    /// Added to the terrain height (or the absolute height if `absolute`).
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub absolute: bool,
    /// Optional instance tag for scripting and loot.
    #[serde(default)]
    pub tag: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MarkerKind {
    /// Where the truck drops you.
    Insertion,
    /// Where the truck can pick you up (the far side).
    Rendezvous,
    /// The costly extract near insertion.
    ShortExfil,
    /// Where robots enter and leave the map.
    RobotGate,
    /// A place loot can appear, rolled from `table`.
    Loot { table: String },
    /// A named point of interest, for jobs and telemetry.
    Poi,
    /// A patch of ground where artifacts form.
    ArtifactField,
    /// One of the Zone's weird item-eating interactables.
    Offering,
    /// Where robot work sites get built.
    WorkSite,
    /// Where the truck parks along the perimeter road.
    TruckStop,
    /// A sheet of paper lying on whatever is built here (a table, a floor)
    /// or on the ground: someone's note.
    Note { text: String },
    /// A painted board on a post, facing `rot`.
    Sign { text: String },
    /// A computer terminal on whatever is built here, screen facing `rot`;
    /// press it for the next page.
    Terminal { pages: Vec<String> },
    /// Where this valley's depot key lies until someone takes it home.
    ValleyKey { item: String },
    /// A standing anomaly that the Surge never moves: `kind` names an
    /// `anomaly::AnomalyKind` (Well, Mirror, ...). It lies here every cycle.
    Anomaly { kind: String, radius: f32 },
    /// A sentry's mast (`robots::sentry`). Its arc is centred straight
    /// along `rot` (its front faces the way a sign's board would), `arc`
    /// degrees wide; behind it is blind. `power` is where its junction
    /// box stands (default: 12 m straight behind the mast).
    Sentry {
        #[serde(default = "sentry_arc")]
        arc: f32,
        #[serde(default)]
        power: Option<P2>,
    },
    /// A spotter's winch (`robots::spotter`): a balloon on a tether over
    /// it, watching the ground round about and calling in the others.
    Spotter,
}

fn sentry_arc() -> f32 {
    220.0
}

impl MarkerKind {
    /// The words a note, sign or terminal carries (all of a terminal's
    /// pages), for checks and exports.
    pub fn text(&self) -> Option<String> {
        match self {
            MarkerKind::Note { text } | MarkerKind::Sign { text } => Some(text.clone()),
            MarkerKind::Terminal { pages } => Some(pages.join("\n\n")),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Marker {
    pub kind: MarkerKind,
    pub at: P2,
    #[serde(default)]
    pub rot: f32,
    #[serde(default)]
    pub name: String,
    /// Height above terrain (loot on a shelf, a gate on a platform).
    #[serde(default)]
    pub y: f32,
}

/// A named area, for danger tiers, loot quality and telemetry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub polygon: Vec<P2>,
    /// 0 calm .. 3 deadly.
    #[serde(default)]
    pub danger: u8,
    /// How many loot rolls each spot here gets (fractions are a chance of
    /// one more). By default it follows danger: risk pays.
    #[serde(default)]
    pub loot: Option<f32>,
}

impl Region {
    pub fn richness(&self) -> f32 {
        self.loot.unwrap_or(0.8 + 0.2 * self.danger as f32)
    }
}

/// A ruined house's prefab id is its house's with the ruin's state after
/// it (`house_plank_burnt`); built from the same plan, it stands on the
/// same footprint with the same doors.
pub const RUIN_STATES: [&str; 3] = ["_damaged", "_burnt", "_drowned"];

/// The prefab a ruin is a ruin of (any other id as it is).
pub fn prefab_base(id: &str) -> &str {
    RUIN_STATES
        .iter()
        .find_map(|s| id.strip_suffix(s))
        .unwrap_or(id)
}

/// Walls a prefab builds out past its footprint, as centre and half
/// extents (x, z) in the prefab's own frame, unrotated. They aren't
/// footprint (the lint doesn't count them as standing anywhere), but the
/// nav grid blocks them: nothing walks through a wall it can't see. The
/// prefab's own tests check these agree with what it builds.
pub fn prefab_walls(id: &str) -> &'static [([f32; 2], [f32; 2])] {
    match prefab_base(id) {
        // The face stair's walls (world::dam::stair_wall): along the
        // stair and the landing's south side, and the landing's east side.
        "dam_face" => &[([79.0, 10.6], [66.5, 1.5]), ([144.0, 5.55], [1.5, 3.55])],
        _ => &[],
    }
}

/// Width (x), height, depth (z) of each prefab, metres, unrotated.
/// Whether a prefab is a vehicle (a wreck, rolling stock): ivy doesn't
/// climb it as it does a ruin's walls (`world::overgrowth`).
pub fn prefab_is_vehicle(id: &str) -> bool {
    prefab_wheels(id).is_some() || matches!(id, "boxcar" | "funicular_car")
}

/// Where a wreck's wheels meet the ground, (x, z) in car space (front-left,
/// front-right, rear-left, rear-right; its front to -Z), from Kit's models
/// (`art/vehicles/wrecks.py`; the lorry's rear twins at their middle). Its
/// tyres' bottoms are 2 cm under its origin. `None` for anything without
/// wheels.
pub fn prefab_wheels(id: &str) -> Option<[[f32; 2]; 4]> {
    let (front, rear, fz, rz) = match id {
        "wreck_lada" => (0.68, 0.68, -1.21, 1.21),
        "wreck_uaz" => (0.72, 0.72, -1.19, 1.19),
        "wreck_lorry" => (0.8, 0.86, -1.65, 1.65),
        _ => return None,
    };
    Some([[-front, fz], [front, fz], [-rear, rz], [rear, rz]])
}

pub fn prefab_footprint(id: &str) -> [f32; 3] {
    let mut f = match prefab_base(id) {
        "quarantine_gate" => [12.0, 7.0, 3.0],
        "guard_booth" => [2.6, 2.7, 2.6],
        "sawmill" => [24.0, 6.3, 12.0],
        "cabin_log" => [7.0, 5.2, 6.0],
        "log_pile" => [2.5, 1.6, 7.0],
        "farmhouse" => [10.0, 7.0, 8.0],
        "barn" => [12.0, 10.0, 16.0],
        "shed" => [4.0, 3.0, 3.0],
        "house_plank" => [8.0, 6.0, 7.0],
        "house_brick" => [10.0, 6.6, 8.0],
        "shop" => [9.0, 3.7, 7.0],
        "pump_station" => [14.0, 8.0, 10.0],
        "footbridge" => [1.6, 1.2, 24.0],
        "anomaly_core" => [10.0, 3.1, 10.0],
        "bus_depot" => [30.0, 11.1, 18.0],
        "radio_mast" => [2.0, 45.0, 2.0],
        "mast_hut" => [5.0, 3.0, 4.0],
        "rail_shed" => [20.0, 10.3, 12.0],
        "boxcar" => [3.0, 4.2, 14.0],
        "ranger_tower" => [6.0, 12.7, 5.0],
        // The ranger tower's legs and stair, open on top under its survey
        // mast and beacon.
        "survey_tower" => [6.0, 16.6, 5.0],
        // Kit's car wrecks (`art/vehicles/wrecks.py`).
        "wreck_lada" => [1.8, 1.45, 4.3],
        "wreck_uaz" => [1.9, 2.0, 4.1],
        "wreck_lorry" => [2.6, 2.2, 6.4],
        "robot_gate" => [17.6, 14.0, 4.0],
        "factory_hall" => [36.0, 13.8, 20.0],
        "chimney" => [5.0, 38.0, 5.0],
        "target" => [2.4, 1.7, 2.0],
        // Landmarks (src/world/buildings.rs); the headframe's back struts
        // reach 7 m behind its shaft.
        "water_tower" => [6.0, 18.2, 6.0],
        "headframe" => [6.0, 19.6, 15.0],
        "signal_box" => [8.4, 6.9, 4.2],
        "steeple" => [4.4, 19.0, 4.4],
        // A nave 10 x 14 with its steps at the west door, the bell tower
        // off the east end.
        "chapel" => [10.4, 19.0, 23.0],
        // A garden fence's run: four panels between five posts.
        "fence_garden" => [8.5, 1.3, 0.2],
        // Hero landmarks: a valley's one unmistakable silhouette.
        "mill_race" => [7.8, 28.5, 54.6],
        // A mine's aerial ropeway: the upper station (its ropes run out
        // 34 m along -z, not counted), the line's pylons, one fallen.
        "ropeway_station" => [12.0, 26.2, 16.2],
        "ropeway_pylon" => [7.2, 20.7, 4.9],
        "ropeway_pylon_down" => [7.6, 4.1, 27.0],
        // A blast furnace with its stoves, chimney, dust catcher, skip
        // bridge and cast house.
        "blast_furnace" => [45.6, 38.5, 59.2],
        // Sallow's dam (docs/FOURTH_VALLEY.md): its two faces either side
        // of the crest (the crest itself is terrain, walked on), the
        // draw-off tower on the bed, and the Custodian's stock in rows.
        "dam_face" => [265.0, 23.0, 4.0],
        "dam_face_end" => [25.0, 23.0, 4.0],
        "dam_back" => [245.0, 21.5, 3.0],
        "dam_back_end" => [35.0, 21.5, 3.0],
        "draw_off_tower" => [9.0, 34.0, 9.0],
        "stock_buses" => [13.0, 3.3, 12.0],
        "stock_bicycles" => [2.4, 1.2, 24.0],
        "stock_bins" => [1.6, 1.3, 20.0],
        "sorting_gantry" => [20.0, 14.0, 6.0],
        // Rime Fell's (src/world/fell.rs): the ore mill stepped down the
        // slope in four bays (its floors 4 m apart, climbing toward +z,
        // the headhouse on the top one), the observatory on its summit
        // (its radio mast to 20 m), the funicular's two stations (the
        // platform along each one's -x face is inside; the track runs
        // past outside it) and its car on the grade.
        "ore_mill" => [20.0, 22.0, 40.0],
        "observatory" => [12.0, 20.0, 10.0],
        "funicular_lower" => [10.0, 8.0, 16.0],
        "funicular_upper" => [12.0, 12.0, 14.0],
        "funicular_car" => [3.2, 4.0, 8.0],
        // An underground site: its entrance kiosk and apron.
        id if crate::underground::is_site(id) => crate::underground::FOOTPRINT,
        _ => [4.0, 3.0, 4.0],
    };
    // A drowned house's roof went with the water: only its walls stand.
    if id.ends_with("_drowned") && prefab_base(id) != "chapel" {
        f[1] = 2.8;
    }
    f
}

/// An outside door or doorway of a prefab, in prefab space (x, z; -Z is
/// the front): the middle of the opening on the wall's outer face, the
/// way out, its width, and how far out from the face to keep clear (its
/// steps, then room to stand). The game builds these into the prefab;
/// `prefab_entrances` mirrors them, and a test in the game keeps the two
/// the same.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entrance {
    pub at: [f32; 2],
    pub out: [f32; 2],
    pub width: f32,
    pub clear: f32,
}

/// A prefab's outside doors and doorways (see [`Entrance`]). Kept in step
/// with the game's recipes by `buildings::tests::entrances_match_the_map`;
/// regenerate it from the recipes when they change.
pub fn prefab_entrances(id: &str) -> &'static [Entrance] {
    // A drowned house's doorway is a breach, its silt banked up to it:
    // wider, and further out.
    if let Some(base) = id.strip_suffix("_drowned") {
        const fn door(z: f32) -> Entrance {
            Entrance {
                at: [0.0, z],
                out: [0.0, -1.0],
                width: 1.6,
                clear: 3.6,
            }
        }
        const PLANK: [Entrance; 1] = [door(-3.59)];
        const WIDE: [Entrance; 1] = [door(-4.09)];
        const CHAPEL: [Entrance; 1] = [Entrance {
            at: [0.0, -7.18],
            out: [0.0, -1.0],
            width: 2.0,
            clear: 3.6,
        }];
        return match base {
            "house_plank" => &PLANK,
            "farmhouse" | "house_brick" => &WIDE,
            "chapel" => &CHAPEL,
            _ => &[],
        };
    }
    match prefab_base(id) {
        "house_plank" => &[Entrance {
            at: [0.0, -3.59],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.61,
        }],
        "farmhouse" => &[Entrance {
            at: [0.0, -4.09],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.61,
        }],
        "house_brick" => &[Entrance {
            at: [0.0, -4.09],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.61,
        }],
        "cabin_log" => &[Entrance {
            at: [-1.5, -3.14],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.26,
        }],
        "shop" => &[
            Entrance {
                at: [0.0, -3.63],
                out: [0.0, -1.0],
                width: 0.95,
                clear: 3.28,
            },
            Entrance {
                at: [-3.0, 3.63],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 3.28,
            },
        ],
        "shed" => &[Entrance {
            at: [-1.0, -1.55],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.05,
        }],
        "barn" => &[
            Entrance {
                at: [0.0, -8.08],
                out: [0.0, -1.0],
                width: 4.0,
                clear: 1.0,
            },
            Entrance {
                at: [6.08, -4.0],
                out: [1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [0.0, 8.08],
                out: [0.0, 1.0],
                width: 3.0,
                clear: 1.0,
            },
        ],
        "pump_station" => &[
            Entrance {
                at: [0.0, -5.15],
                out: [0.0, -1.0],
                width: 0.95,
                clear: 3.25,
            },
            Entrance {
                at: [-7.15, 0.0],
                out: [-1.0, 0.0],
                width: 0.95,
                clear: 3.25,
            },
        ],
        "bus_depot" => &[
            Entrance {
                at: [-6.0, -9.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [6.0, -9.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [10.5, 9.13],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 2.08,
            },
        ],
        "rail_shed" => &[
            Entrance {
                at: [-4.0, -6.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [4.0, -6.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [7.0, 6.13],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 2.08,
            },
        ],
        "mast_hut" => &[Entrance {
            at: [-1.3, -2.13],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 2.98,
        }],
        "ranger_tower" | "survey_tower" => &[Entrance {
            at: [2.04, 1.2],
            out: [1.0, 0.0],
            width: 0.95,
            clear: 1.0,
        }],
        "guard_booth" => &[Entrance {
            at: [0.0, -1.14],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.66,
        }],
        "boxcar" => &[Entrance {
            at: [1.53, 0.0],
            out: [1.0, 0.0],
            width: 2.0,
            clear: 3.0,
        }],
        "factory_hall" => &[
            Entrance {
                at: [-7.2, -10.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [7.2, -10.13],
                out: [0.0, -1.0],
                width: 5.0,
                clear: 2.08,
            },
            Entrance {
                at: [12.6, 10.13],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 2.08,
            },
        ],
        "signal_box" => &[Entrance {
            at: [-1.5, -1.95],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.25,
        }],
        "chapel" => &[Entrance {
            at: [0.0, -7.18],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 3.53,
        }],
        // The mill's front door, the door onto each bay's terrace on the
        // +x side (their floors at 0, 4, 8 and 12 m), the back door on
        // the top bay.
        "ore_mill" => &[
            Entrance {
                at: [-3.0, -20.0],
                out: [0.0, -1.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [10.0, -15.0],
                out: [1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [10.0, -5.0],
                out: [1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [10.0, 5.0],
                out: [1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [10.0, 15.0],
                out: [1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [4.0, 20.0],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 1.0,
            },
        ],
        "observatory" => &[Entrance {
            at: [0.0, -5.0],
            out: [0.0, -1.0],
            width: 0.95,
            clear: 1.0,
        }],
        "funicular_lower" => &[
            Entrance {
                at: [1.6, 7.9],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 1.2,
            },
            Entrance {
                at: [-1.8, 6.8],
                out: [-1.0, 0.0],
                width: 0.95,
                clear: 1.2,
            },
        ],
        "funicular_upper" => &[
            Entrance {
                at: [-3.2, 5.7],
                out: [-1.0, 0.0],
                width: 0.95,
                clear: 1.0,
            },
            Entrance {
                at: [2.0, 7.0],
                out: [0.0, 1.0],
                width: 0.95,
                clear: 1.0,
            },
        ],
        _ => &[],
    }
}

/// How far below its footing a prefab's foundation reaches (the plinth a
/// house stands on), metres: it may sit that much above the lowest ground
/// under it, less a margin, and still be footed. `qa` in the game checks
/// this against the models.
pub fn prefab_foundation(id: &str) -> f32 {
    match prefab_base(id) {
        "house_plank" | "farmhouse" | "house_brick" | "cabin_log" | "shop" | "shed"
        | "mast_hut" | "guard_booth" | "pump_station" | "signal_box" | "chapel" => 1.5,
        "bus_depot" | "rail_shed" | "factory_hall" => 0.92,
        // Rime Fell's: stone footings and the stations' platforms.
        "observatory" | "funicular_lower" | "funicular_upper" => 0.6,
        _ => 0.0,
    }
}

/// How far above the lowest ground under its footprint a building may sit
/// (on a slope it sits on its highest corner, if that's no higher): its
/// foundation shows on the downhill side rather than the hill being dug
/// out on the uphill one.
pub fn prefab_lift(id: &str) -> f32 {
    // A goods wagon stands on its bogies on the rail's grade, often up an
    // embankment: its door's steps go down as far as the bank falls.
    if id == "boxcar" {
        return 0.6;
    }
    (prefab_foundation(id) - 0.4).clamp(0.0, 0.9)
}

/// How far a drowned house's silt bank runs out from its front wall at
/// most (m): sloped gently enough for a body to walk up it at a slant,
/// from the silt at the sills down to the lowest the ground in front can
/// be ([`prefab_lift`]). Above the ground only where the ground falls away.
/// (The drowned chapel's sills stand higher: its bank is short.)
pub const SILT_BANK_REACH: f32 = 6.6;

/// Radius around a placement kept free of trees: a drowned house's out
/// over its silt bank.
pub fn prefab_clearance(id: &str) -> f32 {
    let [w, _, d] = prefab_footprint(id);
    let r = (w.max(d) * 0.5 + 4.0).max(6.0);
    if id.ends_with("_drowned") && prefab_base(id) != "chapel" {
        r.max(d * 0.5 + SILT_BANK_REACH + 1.0)
    } else {
        r
    }
}

/// A prefab at least this many times longer than it's wide (a dam's face,
/// a mill race) clears a strip round itself rather than the circle on its
/// longest side ([`prefab_clearance`]), which for a 265 m dam emptied the
/// valley for 130 m round it of every tree and rock.
pub const CLEAR_STRIP: f32 = 3.0;
/// How far round its footprint a long prefab's strip reaches.
const STRIP_MARGIN: f32 = 4.0;

/// Whether `p` is in the ground a placement keeps clear of trees and
/// rocks, `extra` metres further out: the circle of [`prefab_clearance`]
/// round it, or a long one's strip.
pub fn placement_clears(pl: &Placement, p: P2, extra: f32) -> bool {
    let [w, _, d] = prefab_footprint(&pl.prefab);
    let (dx, dz) = (p[0] - pl.at[0], p[1] - pl.at[1]);
    if w.max(d) < CLEAR_STRIP * w.min(d).max(0.1) {
        return (dx * dx + dz * dz).sqrt() < prefab_clearance(&pl.prefab) + extra;
    }
    // In its own frame, as the nav grid turns a footprint.
    let (s, c) = pl.rot.to_radians().sin_cos();
    let (lx, lz) = (dx * c + dz * s, -dx * s + dz * c);
    lx.abs() < w * 0.5 + STRIP_MARGIN + extra && lz.abs() < d * 0.5 + STRIP_MARGIN + extra
}

/// Whether a prefab wants the ground levelled under it (buildings do;
/// bridges, gates over roads and the anomaly don't, nor a garden fence:
/// its posts follow the ground, and a pad levelled for a run that meets a
/// house's back wall reached under the house and lifted its door off its
/// steps; `map_lint` holds its ground to [`FENCE_RISE`](crate::map_lint::FENCE_RISE)),
/// nor a car wreck: a car left on a verge sits on the verge as it is (a
/// pad would cut a terrace into the bank beside the road); `map_lint` holds its
/// ground to [`WRECK_RISE`](crate::map_lint::WRECK_RISE).
pub fn prefab_wants_pad(id: &str) -> bool {
    !matches!(
        id,
        "footbridge"
            | "anomaly_core"
            | "quarantine_gate"
            | "radio_mast"
            | "boxcar"
            | "target"
            | "fence_garden"
            // Stepped down its slope: the map terraces the ground for it.
            | "ore_mill"
            // On the line's grade.
            | "funicular_car"
            | "wreck_lada"
            | "wreck_uaz"
            | "wreck_lorry"
    )
}

impl MapDef {
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn half(&self) -> f32 {
        self.size * 0.5
    }

    pub fn markers_of<'a>(&'a self, kind: &'a MarkerKind) -> impl Iterator<Item = &'a Marker> {
        self.markers
            .iter()
            .filter(move |m| std::mem::discriminant(&m.kind) == std::mem::discriminant(kind))
    }

    /// Which region a point is in, if any (first match wins).
    /// Loot rolls per spot at `p` (1 outside any region).
    pub fn richness_at(&self, p: P2) -> f32 {
        self.region_at(p).map_or(1.0, Region::richness)
    }

    pub fn region_at(&self, p: P2) -> Option<&Region> {
        self.regions
            .iter()
            .find(|r| crate::geom::point_in_polygon(&r.polygon, p))
    }
}

/// The first map, shipped in the binary for tests and the headless sims.
pub const MARROW_CREEK: &str = include_str!("../../../assets/maps/marrow_creek.ron");
/// The second map.
pub const KETTLE_PASS: &str = include_str!("../../../assets/maps/kettle_pass.ron");
/// The third.
pub const CINDER_WORKS: &str = include_str!("../../../assets/maps/cinder_works.ron");
/// The fourth (docs/FOURTH_VALLEY.md).
pub const SALLOW_RESERVOIR: &str = include_str!("../../../assets/maps/sallow_reservoir.ron");
/// The fifth (docs/FIFTH_VALLEY.md).
pub const RIME_FELL: &str = include_str!("../../../assets/maps/rime_fell.ron");
pub const TRAINING_YARD: &str = include_str!("../../../assets/maps/training_yard.ron");

/// Every valley the truck drives to, (map id, map text), in the order a
/// career meets them. Tests that hold "every valley" to something go over
/// this list, so a new valley is checked by all of them; a test that a map
/// file outside it fails keeps the list whole.
pub const VALLEYS: [(&str, &str); 5] = [
    ("marrow_creek", MARROW_CREEK),
    ("kettle_pass", KETTLE_PASS),
    ("cinder_works", CINDER_WORKS),
    ("sallow_reservoir", SALLOW_RESERVOIR),
    ("rime_fell", RIME_FELL),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Heightmap;

    /// A road's avalanche galleries read like its bridges, and a road
    /// written before galleries existed has none.
    #[test]
    fn road_galleries_parse_and_default_empty() {
        let road = |extra: &str| -> RoadDef {
            ron::from_str(&format!(
                "(kind: Gravel, points: [(0.0, 0.0), (100.0, 0.0)], width: 6.0{extra})"
            ))
            .unwrap()
        };
        assert!(road("").galleries.is_empty());
        let r = road(", galleries: [(at: (50.0, 1.0), length: 40.0)]");
        assert_eq!(r.galleries.len(), 1);
        assert_eq!(r.galleries[0].at, [50.0, 1.0]);
        assert_eq!(r.galleries[0].length, 40.0);
        assert!(r.bridges.is_empty());
    }

    /// Every map file but the Training Yard is a valley on [`VALLEYS`], and
    /// the list's text is the file's.
    /// A long prefab clears a strip round itself, not the circle on its
    /// longest side: trees and rocks grow 20 m off a dam's face, not 130.
    #[test]
    fn a_long_prefab_clears_a_strip() {
        let pl = Placement {
            prefab: "dam_face".into(),
            at: [0.0, 0.0],
            rot: 0.0,
            ..MapDef::from_ron(SALLOW_RESERVOIR).unwrap().placements[0].clone()
        };
        assert!(placement_clears(&pl, [100.0, 3.0], 0.0), "beside the face");
        assert!(
            !placement_clears(&pl, [0.0, 20.0], 0.0),
            "20 m off the face"
        );
        assert!(!placement_clears(&pl, [100.0, -40.0], 0.0), "40 m off it");
        let turned = Placement {
            rot: 90.0,
            ..pl.clone()
        };
        assert!(
            placement_clears(&turned, [3.0, 100.0], 0.0),
            "beside it, turned"
        );
        assert!(
            !placement_clears(&turned, [20.0, 0.0], 0.0),
            "off it, turned"
        );
        // A house keeps its circle.
        let house = Placement {
            prefab: "house_brick".into(),
            ..pl
        };
        assert!(placement_clears(&house, [5.0, 5.0], 0.0));
    }

    #[test]
    fn every_valley_is_on_the_list() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension()? == "ron").then(|| p.file_stem()?.to_str().map(String::from))?
            })
            .filter(|id| id != "training_yard")
            .collect();
        files.sort();
        let mut listed: Vec<String> = VALLEYS.iter().map(|v| v.0.to_string()).collect();
        listed.sort();
        assert_eq!(files, listed, "assets/maps and map::VALLEYS disagree");
        for (id, text) in VALLEYS {
            let on_disk = std::fs::read_to_string(dir.join(format!("{id}.ron"))).unwrap();
            assert_eq!(on_disk, text, "{id}");
        }
    }

    #[test]
    fn kettle_pass_parses_and_builds() {
        let map = MapDef::from_ron(KETTLE_PASS).expect("map parses");
        assert_eq!(map.markers_of(&MarkerKind::Rendezvous).count(), 3);
        let hm = Heightmap::build(&map);
        assert!(
            hm.water_level([40.0, 130.0]).is_some(),
            "the beck under the bridge"
        );
        assert!(hm.water_level([-42.0, 125.0]).is_none(), "the Row is dry");
        // It climbs: the pass is well above the gate.
        let gate = hm.height([120.0, 270.0]);
        let pass = hm.height([10.0, -250.0]);
        assert!(pass - gate > 15.0, "gate {gate:.1}, pass {pass:.1}");
        for m in map.markers.iter().filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Insertion | MarkerKind::Rendezvous | MarkerKind::Offering
            )
        }) {
            assert!(hm.slope_deg(m.at) < 25.0, "{} is on a cliff", m.name);
            assert!(hm.water_level(m.at).is_none(), "{} is under water", m.name);
        }
    }

    #[test]
    fn cinder_works_parses_and_builds() {
        let map = MapDef::from_ron(CINDER_WORKS).expect("map parses");
        assert_eq!(map.markers_of(&MarkerKind::Rendezvous).count(), 3);
        let hm = Heightmap::build(&map);
        assert!(
            hm.water_level([20.0, 56.0]).is_some(),
            "the canal under the bridge"
        );
        assert!(hm.water_level([-50.0, 155.0]).is_none(), "Ash Row is dry");
        assert!(hm.water_level([75.0, -20.0]).is_none(), "the works are dry");
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap();
        for rv in map.markers_of(&MarkerKind::Rendezvous) {
            assert!(
                crate::geom::dist(ins.at, rv.at) > 350.0,
                "{} is too close to the insertion",
                rv.name
            );
        }
        for m in map.markers.iter().filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Insertion | MarkerKind::Rendezvous | MarkerKind::Offering
            )
        }) {
            assert!(hm.slope_deg(m.at) < 25.0, "{} is on a cliff", m.name);
            assert!(hm.water_level(m.at).is_none(), "{} is under water", m.name);
        }
    }

    /// Sallow Reservoir (docs/FOURTH_VALLEY.md): a dam, then a hole. The
    /// crest stands well over the valley below and the drained bed behind
    /// it; the bed is dry but for the old river; the upper arm, behind the
    /// causeway, is still full, its lower street wadeable.
    #[test]
    fn sallow_reservoir_parses_and_builds() {
        let map = MapDef::from_ron(SALLOW_RESERVOIR).expect("map parses");
        assert_eq!(map.markers_of(&MarkerKind::Rendezvous).count(), 3);
        let hm = Heightmap::build(&map);
        let crest = hm.height([0.0, 140.0]);
        let below = hm.height([25.0, 230.0]);
        let bed = hm.height([40.0, 60.0]);
        assert!(crest - below > 18.0, "crest {crest:.1}, below {below:.1}");
        assert!(crest - bed > 12.0, "crest {crest:.1}, bed {bed:.1}");
        for p in [
            [40.0, 60.0],
            [-70.0, 10.0],
            [80.0, -100.0],
            [-150.0, -100.0],
        ] {
            assert!(hm.water_level(p).is_none(), "the bed is wet at {p:?}");
        }
        let arm = hm
            .water_level([-180.0, -170.0])
            .expect("the upper arm holds water");
        assert!(arm - hm.height([-180.0, -170.0]) > 2.0, "the arm is deep");
        let street = [-210.0, -204.0];
        let wade = hm.water_level(street).expect("the lower street is flooded") - hm.height(street);
        assert!(
            (0.3..0.9).contains(&wade),
            "the lower street is {wade:.2} m deep"
        );
        assert!(
            hm.height([-160.0, -128.0]) > arm,
            "the causeway holds the arm"
        );
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap();
        for m in map.markers.iter().filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Insertion | MarkerKind::Rendezvous | MarkerKind::Offering
            )
        }) {
            assert!(hm.slope_deg(m.at) < 25.0, "{} is on a cliff", m.name);
            assert!(hm.water_level(m.at).is_none(), "{} is under water", m.name);
            if m.kind == MarkerKind::Rendezvous {
                assert!(
                    crate::geom::dist(ins.at, m.at) > 350.0,
                    "{} is near",
                    m.name
                );
                // The truck stops on the far side of the dam.
                assert!(m.at[1] < 100.0, "{} is below the dam", m.name);
            }
        }
    }

    /// Rime Fell (docs/FIFTH_VALLEY.md): high and cold, climbing north the
    /// whole way. The settlement's bench stands well over the Bottoms and
    /// under the col; the observatory's summit is the valley's top; the
    /// mill's levels step 4 m up the knoll.
    #[test]
    fn rime_fell_parses_and_builds() {
        let map = MapDef::from_ron(RIME_FELL).expect("map parses");
        assert_eq!(map.markers_of(&MarkerKind::Rendezvous).count(), 3);
        let hm = Heightmap::build(&map);
        let gate = hm.height([-10.0, 266.0]);
        let bench = hm.height([-30.0, 20.0]);
        let col = hm.height([70.0, -262.0]);
        let summit = hm.height([-208.0, -210.0]);
        assert!(bench > gate + 30.0, "bench {bench} over the Bottoms {gate}");
        assert!(col > bench + 30.0, "col {col} over the bench {bench}");
        assert!(summit > col + 8.0, "summit {summit} over the col {col}");
        for (k, z) in [45.0, 25.0, 15.0, 5.0].into_iter().enumerate() {
            let h = hm.height([125.0, z]);
            let want = 90.0 + 4.0 * k as f32;
            assert!(
                (h - want).abs() < 0.3,
                "mill terrace {k} at {h}, not {want}"
            );
        }
        assert!(hm.water_level([168.0, -185.0]).is_some(), "the tarn");
        for m in map.markers.iter().filter(|m| {
            matches!(
                m.kind,
                MarkerKind::Insertion | MarkerKind::Rendezvous | MarkerKind::Offering
            )
        }) {
            assert!(hm.slope_deg(m.at) < 25.0, "{} is on a cliff", m.name);
            assert!(hm.water_level(m.at).is_none(), "{} is under water", m.name);
        }
    }

    #[test]
    #[ignore = "prints water depths"]
    fn water_depths() {
        for (name, src) in VALLEYS {
            let map = MapDef::from_ron(src).unwrap();
            let hm = Heightmap::build(&map);
            let mut deepest: f32 = 0.0;
            let n = (map.size / 2.0) as i32;
            for i in -n..n {
                for j in -n..n {
                    let p = [i as f32 * 2.0, j as f32 * 2.0];
                    if let Some(w) = hm.water_level(p) {
                        deepest = deepest.max(w - hm.height(p));
                    }
                }
            }
            println!("DEPTH {name} {deepest:.2}");
        }
    }

    #[test]
    fn risk_pays_in_loot() {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let village = map.richness_at([5.0, 70.0]);
        let hollow = map.richness_at([150.0, -40.0]);
        let rail = map.richness_at([185.0, -215.0]);
        assert!(village < 1.0, "the village is picked over ({village})");
        assert!(rail > 1.0 && hollow > rail, "{rail} {hollow}");
    }

    #[test]
    fn marrow_creek_parses_and_builds() {
        let map = MapDef::from_ron(MARROW_CREEK).expect("map parses");
        assert_eq!(map.size, 640.0);
        assert!(map.markers_of(&MarkerKind::Rendezvous).count() >= 2);
        let hm = Heightmap::build(&map);
        // The creek has water, and the village bridge leaves it alone.
        assert!(
            hm.water_level([11.0, 10.0]).is_some(),
            "creek under the bridge"
        );
        assert!(hm.water_level([5.0, 70.0]).is_none(), "village is dry");
        // Every insertion and rendezvous is on walkable ground.
        for m in map
            .markers
            .iter()
            .filter(|m| matches!(m.kind, MarkerKind::Insertion | MarkerKind::Rendezvous))
        {
            assert!(hm.slope_deg(m.at) < 25.0, "{} is on a cliff", m.name);
            assert!(hm.water_level(m.at).is_none(), "{} is under water", m.name);
        }
    }

    #[test]
    fn rendezvous_points_are_across_the_creek_from_insertion() {
        let map = MapDef::from_ron(MARROW_CREEK).unwrap();
        let ins = map.markers_of(&MarkerKind::Insertion).next().unwrap();
        for rv in map.markers_of(&MarkerKind::Rendezvous) {
            assert!(
                crate::geom::dist(ins.at, rv.at) > 350.0,
                "{} is too close to the insertion",
                rv.name
            );
        }
    }
}
