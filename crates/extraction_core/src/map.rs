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

/// Width (x), height, depth (z) of each prefab, metres, unrotated.
pub fn prefab_footprint(id: &str) -> [f32; 3] {
    match id {
        "quarantine_gate" => [12.0, 4.0, 3.0],
        "guard_booth" => [2.6, 2.7, 2.6],
        "sawmill" => [24.0, 7.0, 12.0],
        "cabin_log" => [7.0, 4.5, 6.0],
        "log_pile" => [2.5, 1.6, 7.0],
        "farmhouse" => [10.0, 7.0, 8.0],
        "barn" => [12.0, 9.0, 16.0],
        "shed" => [4.0, 3.0, 3.0],
        "house_plank" => [8.0, 6.0, 7.0],
        "house_brick" => [10.0, 7.5, 8.0],
        "shop" => [9.0, 5.0, 7.0],
        "pump_station" => [14.0, 7.0, 10.0],
        "footbridge" => [1.6, 1.2, 24.0],
        "anomaly_core" => [10.0, 4.0, 10.0],
        "bus_depot" => [30.0, 8.0, 18.0],
        "radio_mast" => [2.0, 45.0, 2.0],
        "mast_hut" => [5.0, 3.0, 4.0],
        "rail_shed" => [20.0, 9.0, 12.0],
        "boxcar" => [3.0, 4.2, 14.0],
        "ranger_tower" => [6.0, 12.0, 5.0],
        "robot_gate" => [17.6, 14.0, 4.0],
        "factory_hall" => [36.0, 10.0, 20.0],
        "chimney" => [5.0, 38.0, 5.0],
        "target" => [2.4, 1.7, 2.0],
        _ => [4.0, 3.0, 4.0],
    }
}

/// How far below its footing a prefab's foundation reaches (the plinth a
/// house stands on), metres: it may sit that much above the lowest ground
/// under it, less a margin, and still be footed. `qa` in the game checks
/// this against the models.
pub fn prefab_foundation(id: &str) -> f32 {
    match id {
        "house_plank" | "farmhouse" | "house_brick" | "cabin_log" | "shop" | "shed"
        | "mast_hut" | "guard_booth" | "pump_station" => 1.5,
        "bus_depot" | "rail_shed" | "factory_hall" => 0.92,
        _ => 0.0,
    }
}

/// How far above the lowest ground under its footprint a building may sit
/// (on a slope it sits on its highest corner, if that's no higher): its
/// foundation shows on the downhill side rather than the hill being dug
/// out on the uphill one.
pub fn prefab_lift(id: &str) -> f32 {
    (prefab_foundation(id) - 0.4).clamp(0.0, 0.9)
}

/// Radius around a placement kept free of trees.
pub fn prefab_clearance(id: &str) -> f32 {
    let [w, _, d] = prefab_footprint(id);
    (w.max(d) * 0.5 + 4.0).max(6.0)
}

/// Whether a prefab wants the ground levelled under it (buildings do;
/// bridges, gates over roads and the anomaly don't).
pub fn prefab_wants_pad(id: &str) -> bool {
    !matches!(
        id,
        "footbridge" | "anomaly_core" | "quarantine_gate" | "radio_mast" | "boxcar" | "target"
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
pub const TRAINING_YARD: &str = include_str!("../../../assets/maps/training_yard.ron");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Heightmap;

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

    #[test]
    #[ignore = "prints water depths"]
    fn water_depths() {
        for (name, src) in [
            ("marrow", MARROW_CREEK),
            ("kettle", KETTLE_PASS),
            ("cinder", CINDER_WORKS),
        ] {
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
