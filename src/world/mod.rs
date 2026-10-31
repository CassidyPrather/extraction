//! The Zone: map data, terrain, sky, vegetation, water, placed structures.
//!
//! [`WorldPlugin`] is the logic half (map, heightmap, colliders) and runs
//! headless. [`WorldVisualsPlugin`] attaches everything you can see.

pub mod ao;
pub mod beams;
pub mod bridges;
pub mod buildings;
pub mod cage;
pub mod clutter;
pub mod cold;
pub mod cover;
pub mod dam;
pub mod dark;
pub mod decay;
pub mod echo;
pub mod fell;
pub mod fell_sounds;
pub mod furnace;
pub mod glow;
pub mod halos;
pub mod hinge;
pub mod impostors;
pub mod live;
pub mod lockdown;
pub mod minefields;
pub mod notes;
pub mod overgrowth;
pub mod puddles;
pub mod rain;
pub mod roadworks;
pub mod rocks;
pub mod scenes;
pub mod shafts;
pub mod sky;
pub mod snow;
pub mod storm;
pub mod story;
pub mod structures;
pub mod surfaces;
pub mod terrain;
pub mod trees;
pub mod underground;
pub mod water;
pub mod weather;
pub mod workings;

use avian3d::prelude::*;
use bevy::prelude::*;
use extraction_core::map::{MapDef, MarkerKind};
use extraction_core::terrain::Heightmap;
use std::sync::Arc;

use crate::launch::LaunchOptions;

/// The loaded map and its built terrain. Cheap to clone (shared).
#[derive(Resource, Clone)]
pub struct Zone {
    pub map: Arc<MapDef>,
    pub hm: Arc<Heightmap>,
    /// Each forest polygon's bounds (`forest_cover` skips the far ones).
    forest_bounds: Arc<Vec<([f32; 2], [f32; 2])>>,
    /// The rocks, scattered the first time they're asked for (`rocks`).
    rocks: Arc<std::sync::OnceLock<Arc<Vec<extraction_core::rocks::RockSpot>>>>,
}

/// How far outside a forest its cover can reach: 2 m past its edge, which
/// wanders up to 6 m either way (`fbm` stays within 1), so 12 m out it's
/// nothing.
const FOREST_REACH: f32 = 12.0;

impl Zone {
    pub fn load(name: &str) -> Self {
        let map = load_map_def(name);
        let hm = Heightmap::build(&map);
        Self::new(Arc::new(map), Arc::new(hm))
    }

    pub fn new(map: Arc<MapDef>, hm: Arc<Heightmap>) -> Self {
        let forest_bounds = map
            .forests
            .iter()
            .map(|f| extraction_core::geom::bounds(&f.polygon))
            .collect();
        Self {
            map,
            hm,
            forest_bounds: Arc::new(forest_bounds),
            rocks: Default::default(),
        }
    }

    /// The map's rocks (`scatter_rocks`), scattered once: the colliders and
    /// the meshes each scattered them (Rime Fell's 3,253 took 240 ms).
    pub fn rocks(&self) -> Arc<Vec<extraction_core::rocks::RockSpot>> {
        self.rocks
            .get_or_init(|| Arc::new(extraction_core::rocks::scatter_rocks(&self.map, &self.hm)))
            .clone()
    }

    /// Ground height under a world x/z.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.hm.height([x, z])
    }

    /// Whether the ground's collider is there at x/z: inside the map, and
    /// not a cell opened over an underground stairwell.
    pub fn ground_solid_at(&self, x: f32, z: f32) -> bool {
        let hm = &self.hm;
        let (fi, fj) = ((x - hm.origin) / hm.cell, (z - hm.origin) / hm.cell);
        if fi < 0.0 || fj < 0.0 {
            return false;
        }
        let (i, j) = (fi as usize, fj as usize);
        i + 1 < hm.n && j + 1 < hm.n && !hm.holes[hm.idx(i, j)]
    }

    /// A point on the ground.
    pub fn on_ground(&self, x: f32, z: f32) -> Vec3 {
        Vec3::new(x, self.ground(x, z), z)
    }

    pub fn marker(&self, kind: &MarkerKind) -> Option<&extraction_core::map::Marker> {
        self.map
            .markers
            .iter()
            .find(|m| std::mem::discriminant(&m.kind) == std::mem::discriminant(kind))
    }

    /// How much forest floor covers the ground here, 0 to 1: all of it well
    /// inside a forest, easing out over a few metres across the edge, and
    /// the edge itself wandering a few metres either way. (Taken straight
    /// from the polygon, the leaf litter met the grass in a ruled line.)
    /// A forest whose bounds are farther than `FOREST_REACH` gives none,
    /// so isn't measured: the terrain asks this of every vertex at load
    /// (Kettle Pass's 13 forests were 0.6 s of it).
    pub fn forest_cover(&self, x: f32, z: f32) -> f32 {
        use extraction_core::geom::{point_in_polygon, polygon_edge_distance, smoothstep};
        let mut wander = None;
        self.map
            .forests
            .iter()
            .zip(self.forest_bounds.iter())
            .filter(|(_, (lo, hi))| {
                x > lo[0] - FOREST_REACH
                    && x < hi[0] + FOREST_REACH
                    && z > lo[1] - FOREST_REACH
                    && z < hi[1] + FOREST_REACH
            })
            .map(|(f, _)| {
                let wander = *wander.get_or_insert_with(|| {
                    extraction_core::noise::fbm(x * 0.045, z * 0.045, 0xF0E5, 3, 0.5) * 6.0
                });
                let d = polygon_edge_distance(&f.polygon, [x, z]);
                let inside = point_in_polygon(&f.polygon, [x, z]);
                smoothstep(-2.0, 3.0, if inside { d } else { -d } + wander)
            })
            .fold(0.0, f32::max)
    }

    /// Whether a forest polygon covers this point.
    pub fn in_forest(&self, x: f32, z: f32) -> bool {
        self.map
            .forests
            .iter()
            .any(|f| extraction_core::geom::point_in_polygon(&f.polygon, [x, z]))
    }
}

/// Reads `assets/maps/<name>.ron`, falling back to the built-in copy of
/// Marrow Creek so tests and odd working directories still get a map.
pub(crate) fn load_map_def(name: &str) -> MapDef {
    // Tests only: no build you play can load the test field.
    #[cfg(test)]
    if name == extraction_core::map::TEST_FIELD_ID {
        return MapDef::from_ron(extraction_core::map::TEST_FIELD).expect("test field parses");
    }
    let candidates = [
        crate::asset_root_path()
            .join("maps")
            .join(format!("{name}.ron")),
        std::path::PathBuf::from(format!("assets/maps/{name}.ron")),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            match MapDef::from_ron(&text) {
                Ok(map) => return map,
                Err(e) => error!("{}: {e}", path.display()),
            }
        }
    }
    warn!("map `{name}` not found; using the built-in Marrow Creek");
    MapDef::from_ron(extraction_core::map::MARROW_CREEK).expect("built-in map parses")
}

/// Physics layers. Kept small and explicit so queries can filter sanely.
#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum Layer {
    #[default]
    World,
    Player,
    /// Items and props that can be picked up.
    Item,
    Robot,
    /// Hand and held-item probes (overlap only).
    Hand,
    /// Inside the backpack: items here collide with nothing.
    Stowed,
    /// Shards and splinters (`destruction::debris`): they fall on the
    /// world and nothing else, and no query looks for them.
    Debris,
    /// A container's walls and floor (the stash crate): loose things land
    /// on them and stay out of them, and nothing else (the body, a hand, a
    /// held thing's leash, a line of sight) treats them as a wall.
    Container,
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            weather::WeatherPlugin,
            snow::SnowPlugin,
            scenes::ScenesPlugin,
            underground::UndergroundPlugin,
            roadworks::RoadworksPlugin,
            cold::ColdPlugin,
        ))
        .add_systems(PreStartup, init_zone)
        .add_systems(
            Update,
            (
                hinge::take_hold,
                hinge::swing_by_hand,
                buildings::swing_doors,
                hinge::take_parts,
                hinge::move_parts,
            )
                .chain(),
        )
        .add_systems(FixedUpdate, hinge::fit_counters)
        .add_systems(Update, minefields::lay_for_gallery)
        .add_systems(
            Startup,
            (
                spawn_terrain_collider,
                structures::spawn_structures,
                structures::spawn_boundary,
                buildings::spawn_buildings,
                bridges::spawn_bridges,
                trees::spawn_tree_colliders,
                rocks::spawn_rock_colliders,
            ),
        );
    }
}

fn init_zone(mut commands: Commands, opts: Res<LaunchOptions>) {
    let started = std::time::Instant::now();
    let zone = Zone::load(&opts.map);
    info!(
        "zone `{}` built in {:.0} ms ({}x{} heightmap)",
        zone.map.name,
        started.elapsed().as_secs_f32() * 1000.0,
        zone.hm.n,
        zone.hm.n
    );
    commands.insert_resource(zone);
}

/// The ground's collider (one heightfield for the whole map).
#[derive(Component)]
pub struct TerrainCollider;

/// Whether a sphere is inside or touching the world's solid things.
///
/// The terrain is left out of the physics query and asked of the heightmap:
/// a sphere's test against the whole map's heightfield is about 28 ms in
/// Marrow Creek (parry walks its cells), against 0.001 ms for a building's
/// compound (`budget`'s `head_query_cost`). Every other World collider is
/// tested as usual.
pub fn sphere_in_world(
    spatial: &SpatialQuery,
    zone: &Zone,
    terrain: Option<Entity>,
    at: Vec3,
    radius: f32,
) -> bool {
    if zone.ground_solid_at(at.x, at.z) && at.y - radius < zone.ground(at.x, at.z) {
        return true;
    }
    let mut filter = SpatialQueryFilter::from_mask([Layer::World]);
    if let Some(t) = terrain {
        filter = filter.with_excluded_entities([t]);
    }
    let mut any = false;
    spatial.shape_intersections_callback(
        &Collider::sphere(radius),
        at,
        Quat::IDENTITY,
        &filter,
        |_| {
            any = true;
            false
        },
    );
    any
}

/// The ground: one heightfield collider for the whole map.
fn spawn_terrain_collider(mut commands: Commands, zone: Res<Zone>) {
    let hm = &zone.hm;
    let n = hm.n;
    // Parry stores heights as a column-major Z-by-X matrix; through
    // Avian's row-major `Vec<Vec<_>>` that means the outer index is X and
    // the inner is Z. See `terrain_collider_matches_heightmap`.
    let heights: Vec<Vec<f32>> = (0..n)
        .map(|i| (0..n).map(|j| hm.heights[hm.idx(i, j)]).collect())
        .collect();
    let extent = (n - 1) as f32 * hm.cell;
    let centre = hm.origin + extent * 0.5;
    // Cells opened over underground stairwells (`underground`) are left
    // out. Parry's rows run along Z (see above): its cell (row, column) is
    // the heightmap's (j, i).
    let mut collider = Collider::heightfield(heights, Vec3::new(extent, 1.0, extent));
    if hm.holes.iter().any(|h| *h) {
        let mut shape = collider.shape().clone();
        if let Some(hf) = shape.make_mut().as_heightfield_mut() {
            for j in 0..n - 1 {
                for i in 0..n - 1 {
                    if hm.holes[hm.idx(i, j)] {
                        hf.set_cell_status(
                            j,
                            i,
                            avian3d::parry::shape::HeightFieldCellStatus::CELL_REMOVED,
                        );
                    }
                }
            }
        }
        collider = Collider::from(shape);
    }
    commands.spawn((
        Name::new("Terrain collider"),
        TerrainCollider,
        RigidBody::Static,
        collider,
        CollisionLayers::new(Layer::World, LayerMask::ALL),
        Friction::new(0.8),
        Transform::from_xyz(centre, 0.0, centre),
    ));
}

/// Visual half of the world.
pub struct WorldVisualsPlugin;

impl Plugin for WorldVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            sky::SkyPlugin,
            rain::RainPlugin,
            terrain::TerrainVisualsPlugin,
            water::WaterPlugin,
            trees::TreesPlugin,
            notes::NotesPlugin,
            scenes::ScenesVisualsPlugin,
            rocks::RockVisualsPlugin,
            cover::CoverPlugin,
            underground::UndergroundVisualsPlugin,
            shafts::ShaftsPlugin,
            beams::BeamsPlugin,
            halos::HalosPlugin,
            storm::StormPlugin,
            snow::SnowVisualsPlugin,
        ))
        .add_plugins(roadworks::RoadworksVisualsPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test field is never a valley you play: it isn't on the valleys'
    /// list or the jobs', and it isn't a file in `assets/maps`, where the
    /// truck's route card, a saved career and `--map` find a valley. Only
    /// the game's own tests are served it (`load_map_def`).
    #[test]
    fn the_test_field_cannot_be_reached_from_play() {
        use extraction_core::map::{TEST_FIELD_ID, VALLEYS};
        assert!(VALLEYS.iter().all(|(id, _)| *id != TEST_FIELD_ID));
        assert!(
            crate::progression::valleys()
                .iter()
                .all(|(id, _)| *id != TEST_FIELD_ID)
        );
        let file = format!("{TEST_FIELD_ID}.ron");
        assert!(!crate::asset_root_path().join("maps").join(&file).exists());
        assert!(!std::path::Path::new("assets/maps").join(&file).exists());
        assert!(
            load_map_def(TEST_FIELD_ID)
                .description
                .starts_with("Tests only")
        );
    }

    /// The heightfield collider must agree with the heightmap it was built
    /// from (parry's row/column convention is easy to get transposed).
    #[test]
    fn terrain_collider_matches_heightmap() {
        let opts = crate::LaunchOptions {
            robots: false,
            map: extraction_core::map::TEST_FIELD_ID.into(),
            ..Default::default()
        };
        let mut app = crate::headless_app(opts, false);
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        let zone = app.world().resource::<Zone>().clone();
        let mut state = bevy::ecs::system::SystemState::<(
            SpatialQuery,
            Query<(), With<TerrainCollider>>,
        )>::new(app.world_mut());
        let (spatial, terrain) = state.get(app.world()).expect("spatial query");
        for (x, z) in [
            (40.0, 262.0),
            (-100.0, -200.0),
            (165.0, -60.0),
            (0.0, 0.0),
            (-250.0, 100.0),
            (200.0, -190.0),
            (-60.0, 220.0),
        ] {
            // The ground itself, not a rock or a trunk standing on it.
            let hit = spatial
                .cast_ray_predicate(
                    Vec3::new(x, 200.0, z),
                    Dir3::NEG_Y,
                    500.0,
                    true,
                    &SpatialQueryFilter::default(),
                    &|e| terrain.contains(e),
                )
                .expect("ray hits terrain");
            let y = 200.0 - hit.distance;
            let expect = zone.ground(x, z);
            assert!(
                (y - expect).abs() < 0.15,
                "at ({x}, {z}) collider {y:.2} vs heightmap {expect:.2}"
            );
        }
    }
}

#[cfg(test)]
mod terrain_queries {
    /// No shape or point query that can take in the World layer tests the
    /// terrain's heightfield: each such test is ~28 ms in Marrow Creek
    /// (parry walks the cells), and one per frame was a quarter of the
    /// busy moment's main thread (blink's head test). A query whose filter
    /// names `Layer::World` must leave the terrain out
    /// (`with_excluded_entities`, the `TerrainCollider`) or go through
    /// `sphere_in_world`, which asks the heightmap. Casts (move-and-slide)
    /// take another path and aren't held to this.
    #[test]
    fn no_shape_query_tests_the_terrain_heightfield() {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        assert!(files.len() > 50, "found the sources: {}", files.len());
        let mut wrong = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                let query = [
                    "shape_intersections",
                    "project_point",
                    "point_intersections",
                ]
                .iter()
                .any(|q| l.contains(&format!("{q}(")) || l.contains(&format!(".{q}")));
                if !query || l.trim_start().starts_with("//") || l.contains("fn ") {
                    continue;
                }
                let window = lines[i..(i + 12).min(lines.len())].join("\n");
                let before = lines[i.saturating_sub(12)..i].join("\n");
                let world = window.contains("Layer::World") || window.contains("LayerMask::ALL");
                let terrain_safe = |t: &str| {
                    t.contains("excluded_entities")
                        || t.contains("sphere_in_world")
                        || t.contains("ground_solid_at")
                };
                if world && !terrain_safe(&window) && !terrain_safe(&before) {
                    wrong.push(format!("{}:{}", f.display(), i + 1));
                }
            }
        }
        let mut wrong: Vec<String> = wrong
            .into_iter()
            .filter(|w| !w.contains("render/budget.rs") && !w.contains("destruction/tests.rs"))
            .collect();
        wrong.sort();
        assert!(
            wrong.is_empty(),
            "World-layer shape queries that test the terrain: {wrong:#?}"
        );
    }
}

#[cfg(test)]
mod forest_cover_tests {
    use super::Zone;

    /// What every forest gives, measured: what `forest_cover` was before
    /// it skipped the far ones.
    fn every_forest(zone: &Zone, x: f32, z: f32) -> f32 {
        use extraction_core::geom::{point_in_polygon, polygon_edge_distance, smoothstep};
        let wander = extraction_core::noise::fbm(x * 0.045, z * 0.045, 0xF0E5, 3, 0.5) * 6.0;
        zone.map
            .forests
            .iter()
            .map(|f| {
                let d = polygon_edge_distance(&f.polygon, [x, z]);
                let inside = point_in_polygon(&f.polygon, [x, z]);
                smoothstep(-2.0, 3.0, if inside { d } else { -d } + wander)
            })
            .fold(0.0, f32::max)
    }

    /// Skipping the forests out of reach changes no point's cover, in any
    /// valley.
    #[test]
    fn forest_cover_skips_only_what_gives_none() {
        for map in ["marrow_creek", "kettle_pass", "rime_fell"] {
            let zone = Zone::load(map);
            let hm = &zone.hm;
            let mut covered = 0;
            for j in (0..hm.n).step_by(5) {
                for i in (0..hm.n).step_by(5) {
                    let p = hm.pos(i, j);
                    let (fast, all) = (
                        zone.forest_cover(p[0], p[1]),
                        every_forest(&zone, p[0], p[1]),
                    );
                    assert_eq!(fast.to_bits(), all.to_bits(), "{map} at {p:?}");
                    covered += usize::from(all > 0.0);
                }
            }
            assert!(covered > 0, "{map} has forest");
        }
    }
}
