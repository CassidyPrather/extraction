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
}

impl Zone {
    pub fn load(name: &str) -> Self {
        let map = load_map_def(name);
        let hm = Heightmap::build(&map);
        Self {
            map: Arc::new(map),
            hm: Arc::new(hm),
        }
    }

    /// Ground height under a world x/z.
    pub fn ground(&self, x: f32, z: f32) -> f32 {
        self.hm.height([x, z])
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
    pub fn forest_cover(&self, x: f32, z: f32) -> f32 {
        use extraction_core::geom::{point_in_polygon, polygon_edge_distance, smoothstep};
        let wander = extraction_core::noise::fbm(x * 0.045, z * 0.045, 0xF0E5, 3, 0.5) * 6.0;
        self.map
            .forests
            .iter()
            .map(|f| {
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
            )
                .chain(),
        )
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
        assert!(load_map_def(TEST_FIELD_ID).name.starts_with("Test field"));
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
