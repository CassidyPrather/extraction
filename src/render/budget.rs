//! The VR frame budget: what one eye's view asks of the machine, counted
//! the same way everywhere (the render stats, the benchmark, the gauges),
//! and the limits it is held to. `docs/PERFORMANCE.md` says where the
//! numbers come from.
//!
//! - [`ViewLoad`]: meshes drawn, distinct meshes (what instancing can
//!   batch), triangles, and the same for the sun's shadow cascades of
//!   that view (exactly what Bevy queued for them, ranges and culling
//!   applied), plus shadowed point and spot lights.
//! - [`CpuSpans`]: the main schedule's time a frame and the physics step's
//!   within it, logged every ten seconds next to the frame times.
//! - The benchmark: `EXTRACTION_BENCH=out.jsonl` with a `--tour` writes a
//!   line a view (`scripts/bench.sh`).

use bevy::camera::visibility::{
    CascadesVisibleEntities, CubemapVisibleEntities, VisibleEntities, VisibleMeshEntities,
};
use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// The eye the budget is for: the playtest's Bigscreen Beyond 2 is about
/// 100 degrees across each way per eye, on a square panel. The benchmark
/// and the gauges look through this frustum (the flat window's 75 degrees
/// would undercount what a headset draws).
pub const EYE_FOV_DEG: f32 = 100.0;

/// What one eye may ask for, per view, to hold 90 Hz in stereo on a laptop
/// RTX 3060 (see `docs/PERFORMANCE.md`, "Budget"). Both eyes draw it all,
/// shadows included: the frame is twice this.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Budget {
    /// Mesh entities drawn in the main pass.
    pub draws: usize,
    /// Triangles in the main pass.
    pub tris: usize,
    /// Mesh entities drawn into the sun's cascades, all cascades together.
    pub shadow_draws: usize,
    /// Triangles drawn into the sun's cascades.
    pub shadow_tris: usize,
    /// Point and spot lights casting shadows (six or one extra views each).
    pub shadowed_lights: usize,
    /// Layers a pixel is shaded for, mean over the eye and at the 95th
    /// percentile (`render::overdraw`; depth test off, so an upper bound).
    pub overdraw_mean: f32,
    pub overdraw_p95: u16,
}

pub const BUDGET: Budget = Budget {
    draws: 1200,
    tris: 900_000,
    shadow_draws: 900,
    shadow_tris: 700_000,
    shadowed_lights: 2,
    // (11.5 for a while, when Kettle Pass's quarry view stood inside a
    // spruce clump at 10.27; moved clear, and near cards shrinking away,
    // it's 4.4, and the worst view anywhere is 7.3.)
    overdraw_mean: 10.0,
    overdraw_p95: 35,
};

/// The sun's shadow map, a side, per cascade per eye. Bevy's default is
/// 2048; at 1024 the software frame was 7-18% quicker and the shadows
/// look the same, a touch softer (which the overcast wants anyway).
pub const SHADOW_MAP: usize = 1024;

/// The CPU a busy raid moment may take (`budget::tests`: the whole
/// client without a GPU, one thread, so each system's time is its own
/// work and the frame's is the total; a dev build). Wall milliseconds
/// swing with the machine and whatever else it's running (beside the rest
/// of the suite they more than double, memory contention a reference loop
/// doesn't feel), so the schedules are held as shares of the frame, which
/// survive that, and the frame only loosely: in units of a fixed
/// reference workload timed beside it, at three times today's (53),
/// against a collapse. Not a verdict on 90 Hz (that's the headset log's
/// `cpu:` line); the bug it was made for put PostUpdate at 68% of a
/// frame twice today's.
pub const CPU_FRAME_UNITS: f64 = 160.0;
pub const CPU_SHARE: [(&str, f64); 6] = [
    ("PostUpdate", 0.5),
    ("Update", 0.4),
    ("FixedUpdate", 0.3),
    ("FixedPostUpdate", 0.35),
    ("PreUpdate", 0.2),
    ("Last", 0.15),
];

/// One eye's load.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ViewLoad {
    pub draws: usize,
    pub distinct: usize,
    pub tris: usize,
    pub shadow_draws: usize,
    pub shadow_tris: usize,
    pub shadowed_lights: usize,
    /// Meshes and triangles by the nearest named ancestor, biggest first.
    pub by: Vec<(String, usize, usize)>,
    pub shadow_by: Vec<(String, usize, usize)>,
}

impl ViewLoad {
    /// What's over the budget, in words; empty if nothing.
    pub fn over(&self, b: &Budget) -> Vec<String> {
        let mut out = Vec::new();
        for (what, have, max) in [
            ("draws", self.draws, b.draws),
            ("triangles", self.tris, b.tris),
            ("shadow draws", self.shadow_draws, b.shadow_draws),
            ("shadow triangles", self.shadow_tris, b.shadow_tris),
            ("shadowed lights", self.shadowed_lights, b.shadowed_lights),
        ] {
            if have > max {
                out.push(format!("{what} {have} > {max}"));
            }
        }
        out
    }

    pub fn summary(&self) -> String {
        format!(
            "{} meshes drawn ({} distinct), {:.0}k triangles; sun shadows {} meshes, {:.0}k \
             triangles; {} shadowed lights",
            self.draws,
            self.distinct,
            self.tris as f64 / 1000.0,
            self.shadow_draws,
            self.shadow_tris as f64 / 1000.0,
            self.shadowed_lights,
        )
    }
}

pub struct BudgetPlugin;

impl Plugin for BudgetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CpuSpans>()
            .add_systems(First, start_main)
            .add_systems(Last, end_main)
            .add_systems(FixedFirst, start_tick)
            .add_systems(FixedLast, end_tick)
            .add_systems(Update, log_cpu);
        if std::env::var("EXTRACTION_COUNTS").is_ok_and(|v| v != "0") {
            app.add_systems(Last, log_counts);
        }
        // Every real session keeps one (for the playtest report,
        // `crate::report`); `EXTRACTION_FRAMELOG` puts it elsewhere.
        let opts = app
            .world()
            .get_resource::<crate::launch::LaunchOptions>()
            .cloned();
        let path = std::env::var_os("EXTRACTION_FRAMELOG")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let o = opts.as_ref()?;
                (!o.headless && !o.tooling() && crate::persist::owns_profile(o))
                    .then(|| FrameLog::in_logs(&o.map))
                    .flatten()
            });
        if let Some(path) = path {
            let map = opts.map_or_else(String::new, |o| o.map);
            app.insert_resource(FrameLog::open(path, &map))
                .add_systems(Last, log_each_frame);
        }
        let span = RenderSpan::default();
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            use bevy::render::{Render, RenderSystems};
            render.insert_resource(span.clone()).add_systems(
                Render,
                (
                    start_render.in_set(RenderSystems::ExtractCommands),
                    end_render
                        .after(RenderSystems::Prepare)
                        .before(RenderSystems::Render),
                ),
            );
            app.insert_resource(span)
                .add_systems(Last, read_render.before(end_main));
        }
        {
            use avian3d::prelude::PhysicsSystems;
            app.add_systems(
                FixedPostUpdate,
                (
                    start_physics.before(PhysicsSystems::First),
                    end_physics.after(PhysicsSystems::Last),
                ),
            );
        }
        if counting() {
            app.init_resource::<Triangles>();
        }
        app.add_systems(
            Last,
            count_triangles
                .run_if(resource_exists::<Triangles>)
                .before(end_main),
        );
        app.add_systems(
            PostUpdate,
            (
                reach_doors,
                late_reach,
                range_items,
                small_shadows.after(bevy::camera::visibility::VisibilitySystems::CalculateBounds),
                small_ranges,
                shadow_reach,
            )
                .chain()
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
        let knobs = Knobs::from_env();
        app.insert_resource(bevy::light::DirectionalLightShadowMap {
            size: knobs.shadow_map.unwrap_or(SHADOW_MAP),
        });
        // The player's SMOOTHING setting, unless a knob sets MSAA.
        if knobs.msaa.is_none() {
            app.add_systems(PostUpdate, apply_msaa_setting);
        }
        if knobs != Knobs::default() {
            info!("perf knobs: {knobs:?}");
            app.insert_resource(knobs)
                .add_systems(PostUpdate, apply_knobs);
        }
        if let Some(out) = std::env::var_os("EXTRACTION_BENCH") {
            app.insert_resource(Bench {
                out: out.into(),
                ..default()
            })
            .add_systems(Update, second_eye)
            .add_systems(Last, bench.after(count_triangles).before(end_main));
        }
    }
}

/// An item's meshes fade out where the item would be a few pixels: 300
/// times its longest side, 15 to 60 m (a tin at 36 m, a rifle at 60).
/// Loot had no range, so every tin in the valley the eye faced was drawn
/// (40-odd in most views, 30k triangles, all casting shadows). Artifacts
/// glow and are worth spotting: they keep theirs.
pub fn item_range(size: f32) -> f32 {
    (size * 300.0).clamp(15.0, 60.0)
}

/// Every mesh under this entity (its own, its children's, models
/// swapped in later) fades out by this distance from the eye: for
/// many small things a `VisibilityRange` can't be put on one at a time
/// because their meshes arrive with a model (fence posts).
#[derive(Component, Clone, Copy)]
pub struct LodReach(pub f32);

/// Doors go once their building's stand-in has taken over: a door fades
/// over 120-150 m, the stand-in (which has the doorway) in over 110-130,
/// so the doorway is never empty.
fn reach_doors(
    mut commands: Commands,
    doors: Query<Entity, Added<crate::world::buildings::DoorLeaf>>,
) {
    for e in &doors {
        commands
            .entity(e)
            .try_insert(LodReach(super::lod::FAR_BUILDING + 30.0));
    }
}

/// A `LodReach` put on after its meshes arrived: range them now.
fn late_reach(
    mut commands: Commands,
    added: Query<(Entity, &LodReach), Added<LodReach>>,
    children: Query<&Children>,
    meshes: Query<
        (),
        (
            With<Mesh3d>,
            Without<bevy::camera::visibility::VisibilityRange>,
        ),
    >,
) {
    for (root, reach) in &added {
        for e in std::iter::once(root).chain(children.iter_descendants(root)) {
            if meshes.contains(e) {
                commands
                    .entity(e)
                    .try_insert(bevy::camera::visibility::VisibilityRange {
                        start_margin: 0.0..0.0,
                        end_margin: (reach.0 * 0.8)..reach.0,
                        use_aabb: false,
                    });
            }
        }
    }
}

fn range_items(
    mut commands: Commands,
    added: Query<
        Entity,
        (
            Added<Mesh3d>,
            Without<bevy::camera::visibility::VisibilityRange>,
        ),
    >,
    parents: Query<&ChildOf>,
    items: Query<&crate::items::Item>,
    reaches: Query<&LodReach>,
    catalog: Res<crate::items::Catalog>,
) {
    for e in &added {
        let mut at = e;
        let mut far = None;
        for _ in 0..6 {
            if let Ok(r) = reaches.get(at) {
                far = Some(r.0);
                break;
            }
            if let Ok(item) = items.get(at) {
                let def = catalog.get(item.kind());
                if def.category != extraction_core::items::Category::Artifact {
                    far = Some(item_range(def.size_m().into_iter().fold(0.0, f32::max)));
                }
                break;
            }
            match parents.get(at) {
                Ok(p) => at = p.parent(),
                Err(_) => break,
            }
        }
        let Some(far) = far else {
            continue;
        };
        commands
            .entity(e)
            .try_insert(bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: (far * 0.8)..far,
                use_aabb: false,
            });
    }
}

/// A mesh that casts the sun's shadow only this near the eye
/// (horizontally, give or take a couple of metres so it doesn't flicker
/// at the line). The shadow cascades' own range is the camera's, so a
/// `VisibilityRange` can't stop a caster without hiding it: this can.
/// The forest's near cards carry it: in a wood they were two thirds of
/// the shadow passes, and beyond a few tens of metres in the fog the
/// trunks' shadows read the same.
#[derive(Component, Clone, Copy)]
pub struct ShadowReach(pub f32);

/// Below this size (the largest side, world metres) a mesh casts the
/// sun's shadow only near the eye: [`SMALL_REACH`] under 5 cm,
/// [`SMALLISH_REACH`] under it. At the gates half the shadow passes'
/// meshes were smaller than 10 cm (buttons, fittings, door furniture,
/// tins): a shadow a texel or two wide past a few metres.
pub const SMALL_CASTER: f32 = 0.12;
pub const SMALL_REACH: f32 = 3.0;
pub const SMALLISH_REACH: f32 = 6.0;

/// Gives each newly bounded small mesh its [`ShadowReach`].
#[allow(clippy::type_complexity)]
fn small_shadows(
    mut commands: Commands,
    added: Query<
        (Entity, &bevy::camera::primitives::Aabb, &GlobalTransform),
        (
            Added<bevy::camera::primitives::Aabb>,
            With<Mesh3d>,
            Without<NotShadowCaster>,
            Without<ShadowReach>,
        ),
    >,
) {
    // (`EXTRACTION_SMALL_SHADOWS=0`: every small thing casts at any
    // distance, to compare.)
    if std::env::var("EXTRACTION_SMALL_SHADOWS").is_ok_and(|v| v == "0") {
        return;
    }
    for (e, aabb, gt) in &added {
        let size = (Vec3::from(aabb.half_extents) * gt.compute_transform().scale.abs())
            .max_element()
            * 2.0;
        if size < SMALL_CASTER {
            let reach = if size < 0.05 {
                SMALL_REACH
            } else {
                SMALLISH_REACH
            };
            commands.entity(e).try_insert(ShadowReach(reach));
        }
    }
}

/// Below this size (the largest side, world metres) a mesh that isn't
/// part of an item (items have [`item_range`]) or under a [`LodReach`]
/// fades out where it would be a pixel or two: [`small_range`]. Display
/// guns (every round in a drum its own mesh), a piano's keys and hammers,
/// a desk's fittings were drawn from anywhere: Marrow Creek's aerial view
/// drew 96 piano parts and some 250 gun parts from 60 m up.
pub const SMALL_DRAWN: f32 = 1.0;

/// How far a small thing (`size` its largest side, metres) is drawn: 200
/// times its size, 20 to 200 m (a 10 cm fitting at 20 m, about three
/// pixels a side on the headset; a 60 cm crate at 120 m).
pub fn small_range(size: f32) -> f32 {
    (size * 200.0).clamp(20.0, 200.0)
}

/// Gives each newly bounded small mesh its range ([`small_range`]).
/// `EXTRACTION_SMALL_RANGES=0` leaves them all drawn at any distance.
#[allow(clippy::type_complexity)]
fn small_ranges(
    mut commands: Commands,
    added: Query<
        (Entity, &bevy::camera::primitives::Aabb, &GlobalTransform),
        (
            Added<bevy::camera::primitives::Aabb>,
            With<Mesh3d>,
            With<MeshMaterial3d<StandardMaterial>>,
            Without<bevy::camera::visibility::VisibilityRange>,
            Without<super::warmup::ShelfMark>,
        ),
    >,
    parents: Query<&ChildOf>,
    kept: Query<(), Or<(With<crate::items::Item>, With<LodReach>)>>,
) {
    if std::env::var("EXTRACTION_SMALL_RANGES").is_ok_and(|v| v == "0") {
        return;
    }
    'next: for (e, aabb, gt) in &added {
        let size = (Vec3::from(aabb.half_extents) * gt.compute_transform().scale.abs())
            .max_element()
            * 2.0;
        if size >= SMALL_DRAWN {
            continue;
        }
        // An item's (or a reach's) own range comes from `range_items`.
        let mut at = e;
        for _ in 0..12 {
            if kept.contains(at) {
                continue 'next;
            }
            match parents.get(at) {
                Ok(p) => at = p.parent(),
                Err(_) => break,
            }
        }
        let far = small_range(size);
        commands
            .entity(e)
            .try_insert(bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: (far * 0.8)..far,
                use_aabb: true,
            });
    }
}

fn shadow_reach(
    mut commands: Commands,
    cams: Query<(&GlobalTransform, Option<&bevy::camera::RenderTarget>), crate::render::EyeCamera>,
    casters: Query<(Entity, &GlobalTransform, &ShadowReach, Has<NotShadowCaster>)>,
) {
    let eyes: Vec<Vec2> = crate::render::eyes(&cams)
        .map(|g| g.translation().xz())
        .collect();
    if eyes.is_empty() {
        return;
    }
    for (e, gt, reach, off) in &casters {
        let p = gt.translation().xz();
        let d = eyes
            .iter()
            .map(|e| e.distance(p))
            .fold(f32::INFINITY, f32::min);
        if off && d < reach.0 - 2.0 {
            commands.entity(e).try_remove::<NotShadowCaster>();
        } else if !off && d > reach.0 + 2.0 {
            commands.entity(e).try_insert(NotShadowCaster);
        }
    }
}

/// `EXTRACTION_PERF="msaa=2,shadows=0,cascades=1,shadow_map=1024,shadow_far=60"`:
/// render settings to time against each other without a rebuild (the
/// bench here, the session log's `gpu:` lines in a headset).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Knobs {
    pub msaa: Option<u32>,
    pub shadows: Option<bool>,
    pub cascades: Option<usize>,
    pub shadow_map: Option<usize>,
    pub shadow_far: Option<f32>,
}

impl Knobs {
    pub fn parse(text: &str) -> Self {
        let mut k = Self::default();
        for (key, val) in text.split(',').filter_map(|kv| kv.split_once('=')) {
            let (key, val) = (key.trim(), val.trim());
            match key {
                "msaa" => k.msaa = val.parse().ok(),
                "shadows" => k.shadows = Some(val != "0" && val != "off"),
                "cascades" => k.cascades = val.parse().ok(),
                "shadow_map" => k.shadow_map = val.parse().ok(),
                "shadow_far" => k.shadow_far = val.parse().ok(),
                _ => warn!("EXTRACTION_PERF: no knob {key:?}"),
            }
        }
        k
    }

    fn from_env() -> Self {
        std::env::var("EXTRACTION_PERF")
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }
}

/// The comfort board's SMOOTHING (`Comfort::msaa`, `--msaa`) on every
/// view of the world: the headset's eyes, the flat camera, the mirror.
/// Not offscreen views (a scope's picture, an impostor capture), which
/// set their own. With it off, FXAA instead: one cheap pass that takes
/// the edge off wires and rails (Visual design's call; no SMAA, no TAA,
/// whose ghosting in a headset is a comfort problem). Foliage's alpha to
/// coverage follows the samples by itself (`trees::foliage_alpha`).
#[allow(clippy::type_complexity)]
fn apply_msaa_setting(
    mut commands: Commands,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut cams: Query<
        (
            Entity,
            &mut Msaa,
            Option<&bevy::camera::RenderTarget>,
            Has<bevy::anti_alias::fxaa::Fxaa>,
        ),
        (With<Camera3d>, Without<crate::render::OffscreenCamera>),
    >,
) {
    let Some(comfort) = comfort else {
        return;
    };
    let want = comfort.msaa_setting();
    let all = comfort.is_changed();
    for (e, mut m, target, fxaa) in &mut cams {
        if !all && !m.is_added() {
            continue;
        }
        if matches!(target, Some(bevy::camera::RenderTarget::Image(_))) {
            continue;
        }
        if *m != want {
            *m = want;
        }
        match (want == Msaa::Off, fxaa) {
            (true, false) => {
                commands
                    .entity(e)
                    .insert(bevy::anti_alias::fxaa::Fxaa::default());
            }
            (false, true) => {
                commands.entity(e).remove::<bevy::anti_alias::fxaa::Fxaa>();
            }
            _ => {}
        }
    }
}

fn apply_knobs(
    knobs: Res<Knobs>,
    mut cams: Query<&mut Msaa, (Changed<Msaa>, Without<crate::render::OffscreenCamera>)>,
    mut eyes: Query<
        &mut Msaa,
        (
            Added<Msaa>,
            With<crate::render::OffscreenCamera>,
            With<Camera3d>,
        ),
    >,
    mut suns: Query<(&mut DirectionalLight, &mut bevy::light::CascadeShadowConfig)>,
) {
    if let Some(n) = knobs.msaa {
        let want = match n {
            0 | 1 => Msaa::Off,
            2 => Msaa::Sample2,
            8 => Msaa::Sample8,
            _ => Msaa::Sample4,
        };
        for mut m in cams.iter_mut().chain(eyes.iter_mut()) {
            if *m != want && *m != Msaa::Off {
                *m = want;
            }
        }
    }
    for (mut sun, mut cascades) in &mut suns {
        // Every frame: the weather sets the sun's shadows as it changes.
        if let Some(on) = knobs.shadows {
            if sun.shadow_maps_enabled != on {
                sun.shadow_maps_enabled = on;
            }
        }
        if !sun.is_added() {
            continue;
        }
        if knobs.cascades.is_some() || knobs.shadow_far.is_some() {
            let n = knobs.cascades.unwrap_or(cascades.bounds.len()).max(1);
            let far = knobs
                .shadow_far
                .unwrap_or(cascades.bounds.last().copied().unwrap_or(90.0));
            *cascades = bevy::light::CascadeShadowConfigBuilder {
                num_cascades: n,
                first_cascade_far_bound: if n == 1 { far } else { 15.0f32.min(far) },
                maximum_distance: far,
                ..default()
            }
            .build();
        }
    }
}

/// Triangles are counted only when something will read them.
pub fn counting() -> bool {
    std::env::var_os("EXTRACTION_RENDER_STATS").is_some()
        || std::env::var_os("EXTRACTION_BENCH").is_some()
}

/// Triangles per mesh, counted as each arrives (once a mesh is handed to
/// the render world its data is gone from this one).
#[derive(Resource, Default)]
pub struct Triangles(pub HashMap<AssetId<Mesh>, usize>);

pub fn count_triangles(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    mut tris: ResMut<Triangles>,
) {
    for e in events.read() {
        // A mesh dropped is forgotten: one replaced every few frames (the
        // far-off flocks' merged mesh) once counted up for ever, and no
        // valley's `settle` ever saw its meshes hold still (each ran its
        // whole 3000 frames).
        if let AssetEvent::Removed { id } | AssetEvent::Unused { id } = e {
            tris.0.remove(id);
            continue;
        }
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = e {
            if let Some(mesh) = meshes.get(*id) {
                let n = match mesh.try_indices_option() {
                    Ok(Some(i)) => i.len() / 3,
                    Ok(None) => mesh.count_vertices() / 3,
                    Err(_) => continue,
                };
                tris.0.insert(*id, n);
            }
        }
    }
}

/// Everything [`ViewLoad`] is measured from.
#[derive(SystemParam)]
pub struct LoadCounter<'w, 's> {
    meshes:
        Query<'w, 's, (&'static Mesh3d, Has<NotShadowCaster>), Without<super::warmup::ShelfMark>>,
    names: Query<'w, 's, &'static Name>,
    parents: Query<'w, 's, &'static ChildOf>,
    tris: Option<Res<'w, Triangles>>,
    suns: Query<'w, 's, &'static CascadesVisibleEntities>,
    points: Query<'w, 's, (&'static PointLight, &'static CubemapVisibleEntities)>,
    spots: Query<'w, 's, (&'static SpotLight, &'static VisibleMeshEntities)>,
}

impl LoadCounter<'_, '_> {
    /// What a mesh is part of: its root's name, and the nearest named
    /// ancestor's when that's another ("Building>chair").
    fn label(&self, mut e: Entity) -> String {
        let word = |n: &Name| n.as_str().split(' ').next().unwrap_or("").to_string();
        let mut nearest = None;
        let mut root = None;
        for _ in 0..32 {
            if let Ok(n) = self.names.get(e) {
                nearest.get_or_insert_with(|| word(n));
                root = Some(word(n));
            }
            match self.parents.get(e) {
                Ok(p) => e = p.parent(),
                Err(_) => break,
            }
        }
        match (root, nearest) {
            (Some(r), Some(n)) if r != n => format!("{r}>{n}"),
            (Some(r), _) => r,
            _ => "(unnamed)".to_string(),
        }
    }

    fn tris_of(&self, e: Entity) -> Option<(AssetId<Mesh>, usize)> {
        let (m, _) = self.meshes.get(e).ok()?;
        let id = m.0.id();
        let n = self
            .tris
            .as_ref()
            .and_then(|t| t.0.get(&id).copied())
            .unwrap_or(0);
        Some((id, n))
    }

    /// One view's load: `view` is the camera entity, `seen` its visible
    /// entities.
    pub fn measure(&self, view: Entity, seen: &VisibleEntities) -> ViewLoad {
        let mut load = ViewLoad::default();
        let mut kinds = HashSet::new();
        let mut by: HashMap<String, (usize, usize)> = HashMap::new();
        for &e in seen.get(std::any::TypeId::of::<Mesh3d>()) {
            let Some((id, n)) = self.tris_of(e) else {
                continue;
            };
            load.draws += 1;
            load.tris += n;
            kinds.insert(id);
            let entry = by.entry(self.label(e)).or_default();
            entry.0 += 1;
            entry.1 += n;
        }
        load.distinct = kinds.len();
        let mut shadow_by: HashMap<String, (usize, usize)> = HashMap::new();
        for sun in &self.suns {
            let Some(cascades) = sun.entities.get(&view) else {
                continue;
            };
            for cascade in cascades {
                for &e in &cascade.entities {
                    let Some((_, n)) = self.tris_of(e) else {
                        continue;
                    };
                    load.shadow_draws += 1;
                    load.shadow_tris += n;
                    let entry = shadow_by.entry(self.label(e)).or_default();
                    entry.0 += 1;
                    entry.1 += n;
                }
            }
        }
        load.shadowed_lights = self
            .points
            .iter()
            .filter(|(l, v)| l.shadow_maps_enabled && v.iter().any(|f| !f.entities.is_empty()))
            .count()
            + self
                .spots
                .iter()
                .filter(|(l, v)| l.shadow_maps_enabled && !v.entities.is_empty())
                .count();
        let sorted = |m: HashMap<String, (usize, usize)>| {
            let mut v: Vec<_> = m.into_iter().map(|(k, (c, t))| (k, c, t)).collect();
            v.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
            v
        };
        load.by = sorted(by);
        load.shadow_by = sorted(shadow_by);
        load
    }
}

/// Wall time a frame in the main schedule (all of `First`..`Last`; the
/// render world runs beside it) and in the physics step within it, both
/// as if the frame had run one fixed tick: a slow frame (the software
/// renderer's, a hitch) runs several to catch up, and a headset at 90 Hz
/// runs one.
#[derive(Resource, Default)]
pub struct CpuSpans {
    frame_start: Option<std::time::Instant>,
    tick_start: Option<std::time::Instant>,
    physics_start: Option<std::time::Instant>,
    ticks: u32,
    fixed_now: f64,
    physics_now: f64,
    /// The last whole frame's: main schedule and physics, ms.
    pub main_ms: f64,
    pub physics_ms: f64,
    /// The render world's latest: its per-mesh work (`ExtractCommands`
    /// through `Prepare`: specialise, queue, sort, batch), not encoding or
    /// submitting the passes. It runs beside the main schedule and has to
    /// fit in a frame on its own.
    pub render_ms: f64,
    log: Vec<(f64, f64, f64)>,
}

/// The render world's span, handed across to the main world.
#[derive(Resource, Clone, Default)]
struct RenderSpan(std::sync::Arc<std::sync::Mutex<(Option<std::time::Instant>, f64)>>);

fn start_render(span: Res<RenderSpan>) {
    if let Ok(mut s) = span.0.lock() {
        s.0 = Some(std::time::Instant::now());
    }
}

fn end_render(span: Res<RenderSpan>) {
    if let Ok(mut s) = span.0.lock() {
        if let Some(t) = s.0.take() {
            s.1 = t.elapsed().as_secs_f64() * 1000.0;
        }
    }
}

fn read_render(span: Res<RenderSpan>, mut s: ResMut<CpuSpans>) {
    if let Ok(r) = span.0.lock() {
        s.render_ms = r.1;
    }
}

fn start_main(mut s: ResMut<CpuSpans>) {
    s.frame_start = Some(std::time::Instant::now());
}

fn end_main(mut s: ResMut<CpuSpans>) {
    let Some(t) = s.frame_start.take() else {
        return;
    };
    let whole = t.elapsed().as_secs_f64() * 1000.0;
    let ticks = std::mem::take(&mut s.ticks).max(1) as f64;
    let fixed = std::mem::take(&mut s.fixed_now);
    let physics = std::mem::take(&mut s.physics_now);
    s.main_ms = whole - fixed + fixed / ticks;
    s.physics_ms = physics / ticks;
    let entry = (s.main_ms, s.physics_ms, s.render_ms);
    s.log.push(entry);
}

fn start_tick(mut s: ResMut<CpuSpans>) {
    s.tick_start = Some(std::time::Instant::now());
}

fn end_tick(mut s: ResMut<CpuSpans>) {
    if let Some(t) = s.tick_start.take() {
        s.fixed_now += t.elapsed().as_secs_f64() * 1000.0;
        s.ticks += 1;
    }
}

fn start_physics(mut s: ResMut<CpuSpans>) {
    s.physics_start = Some(std::time::Instant::now());
}

fn end_physics(mut s: ResMut<CpuSpans>) {
    if let Some(t) = s.physics_start.take() {
        s.physics_now += t.elapsed().as_secs_f64() * 1000.0;
    }
}

/// Every ten seconds, next to `frames:`: "cpu: main 4.1 ms a frame (1%
/// 6.0), physics 1.2 (1% 2.3), render world 3.0 (1% 4.2)". The main
/// schedule and the render world each have to fit in the headset's
/// 11.1 ms; they run side by side.
fn log_cpu(time: Res<Time<Real>>, mut s: ResMut<CpuSpans>, mut since: Local<f32>) {
    *since += time.delta_secs();
    if *since < 10.0 || s.log.is_empty() {
        return;
    }
    *since = 0.0;
    let log = std::mem::take(&mut s.log);
    let stat = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        (mean, v[((v.len() as f64 * 0.99) as usize).min(v.len() - 1)])
    };
    let (main, main99) = stat(log.iter().map(|l| l.0).collect());
    let (phys, phys99) = stat(log.iter().map(|l| l.1).collect());
    let (render, render99) = stat(log.iter().map(|l| l.2).collect());
    info!(
        "cpu: main {main:.1} ms a frame (1% {main99:.1}), physics {phys:.1} (1% {phys99:.1}), \
         render world {render:.1} (1% {render99:.1})"
    );
}

/// The benchmark's state: where it writes, the view being measured.
#[derive(Resource, Default)]
struct Bench {
    out: std::path::PathBuf,
    view: Option<String>,
    load: ViewLoad,
    cpu: Vec<(f64, f64, f64, f64)>,
    gpu: HashMap<String, Vec<f64>>,
    gpu_seen: Option<std::time::Instant>,
    written: bool,
}

/// One benchmark line.
#[derive(serde::Serialize)]
struct BenchLine<'a> {
    map: &'a str,
    view: &'a str,
    #[serde(flatten)]
    load: &'a ViewLoad,
    over: Vec<String>,
    /// Main schedule and physics, mean ms over the frames held.
    cpu_main_ms: f64,
    cpu_physics_ms: f64,
    /// Wall time a frame: under a software renderer, mostly rendering.
    frame_ms: f64,
    /// The render world's per-mesh CPU work (see [`CpuSpans::render_ms`]).
    cpu_render_ms: f64,
    /// GPU ms per pass and frame (both eyes), where timestamps exist.
    gpu_ms: HashMap<String, f64>,
}

/// The other eye: a second camera 64 mm to the right rendering to a
/// texture the window's size, so the GPU does what a headset's does
/// (both eyes, each with its own shadow cascades).
fn second_eye(
    mut commands: Commands,
    cams: Query<Entity, Added<crate::render::FreeCamera>>,
    windows: Query<&Window>,
    weather: Res<crate::world::sky::Weather>,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::camera::RenderTarget;
    use bevy::render::render_resource::TextureFormat;
    // `EXTRACTION_BENCH_EYES=1`: one eye, to see what the second costs.
    let one = std::env::var("EXTRACTION_BENCH_EYES").is_ok_and(|v| v == "1");
    for cam in &cams {
        if one {
            commands
                .entity(cam)
                .insert(Projection::Perspective(PerspectiveProjection {
                    fov: EYE_FOV_DEG.to_radians(),
                    near: crate::render::NEAR_CLIP,
                    far: 2000.0,
                    ..default()
                }));
            continue;
        }
        let size = windows
            .iter()
            .next()
            .map_or(UVec2::splat(1024), |w| w.physical_size());
        let image = Image::new_target_texture(size.x, size.y, TextureFormat::Bgra8UnormSrgb, None);
        let target = images.add(image);
        commands.spawn((
            Name::new("Bench right eye"),
            crate::render::OffscreenCamera,
            crate::render::camera_bundle(&weather),
            Camera {
                order: -1,
                ..default()
            },
            RenderTarget::Image(target.into()),
            Transform::from_xyz(0.064, 0.0, 0.0),
            ChildOf(cam),
        ));
        commands
            .entity(cam)
            .insert(Projection::Perspective(PerspectiveProjection {
                fov: EYE_FOV_DEG.to_radians(),
                near: crate::render::NEAR_CLIP,
                far: 2000.0,
                ..default()
            }));
    }
}

fn bench(
    mut bench: ResMut<Bench>,
    tour: Res<crate::tour::TourAt>,
    opts: Res<crate::launch::LaunchOptions>,
    cams: Query<(Entity, &VisibleEntities), With<crate::render::FreeCamera>>,
    counter: LoadCounter,
    spans: Res<CpuSpans>,
    store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
    time: Res<Time<Real>>,
) {
    let bench = &mut *bench;
    if tour.view != bench.view {
        // The previous view is done: write it.
        if let Some(view) = bench.view.take() {
            let cpu = std::mem::take(&mut bench.cpu);
            // The first frames at a view stream detail in.
            let held: Vec<_> = cpu.iter().skip(cpu.len() / 2).collect();
            let mean = |f: fn(&(f64, f64, f64, f64)) -> f64| {
                held.iter().map(|c| f(c)).sum::<f64>() / held.len().max(1) as f64
            };
            // Every camera's pass lands on the same path: the sum over the
            // frames held is both eyes'.
            let frames = held.len().max(1) as f64;
            let gpu = std::mem::take(&mut bench.gpu)
                .into_iter()
                .map(|(k, v)| {
                    let n = v.len() - v.len() * (cpu.len() - held.len()) / cpu.len().max(1);
                    (k, v[v.len() - n..].iter().sum::<f64>() / frames)
                })
                .collect();
            let line = BenchLine {
                map: &opts.map,
                view: &view,
                load: &bench.load,
                over: bench.load.over(&BUDGET),
                cpu_main_ms: mean(|c| c.0),
                cpu_physics_ms: mean(|c| c.1),
                frame_ms: mean(|c| c.2),
                cpu_render_ms: mean(|c| c.3),
                gpu_ms: gpu,
            };
            info!("bench {view}: {}", bench.load.summary());
            for (title, by) in [
                ("drawn", &bench.load.by),
                ("shadows", &bench.load.shadow_by),
            ] {
                for (what, count, tris) in by.iter().take(8) {
                    info!(
                        "bench {view}   {title}: {what}: {count} meshes, {:.0}k triangles",
                        *tris as f64 / 1000.0
                    );
                }
            }
            if let Ok(json) = serde_json::to_string(&line) {
                use std::io::Write;
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&bench.out);
                if let Ok(mut f) = file {
                    let _ = writeln!(f, "{json}");
                }
                bench.written = true;
            }
        }
        bench.view = tour.view.clone();
    }
    if bench.view.is_none() {
        return;
    }
    if let Some((cam, seen)) = cams.iter().next() {
        bench.load = counter.measure(cam, seen);
    }
    bench.cpu.push((
        spans.main_ms,
        spans.physics_ms,
        time.delta_secs_f64() * 1000.0,
        spans.render_ms,
    ));
    if let Some(store) = store {
        let mut newest = bench.gpu_seen;
        for d in store.iter() {
            let path = d.path().as_str();
            let Some(pass) = path
                .strip_prefix("render/")
                .and_then(|p| p.strip_suffix("/elapsed_gpu"))
            else {
                continue;
            };
            for m in d.measurements() {
                if bench.gpu_seen.is_some_and(|s| m.time <= s) {
                    continue;
                }
                bench.gpu.entry(pass.to_string()).or_default().push(m.value);
                newest = newest.max(Some(m.time));
            }
        }
        bench.gpu_seen = newest;
    }
}

/// Frames the tour holds each view for under the benchmark
/// (`EXTRACTION_BENCH_HOLD`, default 12): half settle, half are timed.
pub fn hold() -> u32 {
    if std::env::var_os("EXTRACTION_BENCH").is_none() {
        return 0;
    }
    std::env::var("EXTRACTION_BENCH_HOLD")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(12)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The whole client with no window and no GPU (as
    /// `client_systems_initialise`): visibility, LOD ranges and the
    /// cascades' culling all run on the CPU, so a view's load can be
    /// counted without rendering it.
    /// The client-based gauges and tests each build a whole valley. Those
    /// whose verdict is a time (a CPU frame, a load) run alone: side by
    /// side they'd skew each other's timings ([`heavy`]). Those that count
    /// (draws, triangles, overdraw, meshes, shelf entries, frames) run
    /// beside each other, never beside a timed one ([`counted`]): each app
    /// runs its schedules on one thread, and the counts don't depend on
    /// how fast the frames come. About 1.5 GB a valley.
    static HEAVY: std::sync::RwLock<()> = std::sync::RwLock::new(());

    /// Alone: for what measures a time.
    pub(crate) fn heavy() -> std::sync::RwLockWriteGuard<'static, ()> {
        HEAVY.write().unwrap_or_else(|e| e.into_inner())
    }

    /// Beside other counting gauges, never a timed one: for what counts.
    pub(crate) fn counted() -> std::sync::RwLockReadGuard<'static, ()> {
        HEAVY.read().unwrap_or_else(|e| e.into_inner())
    }

    fn client(map: &str) -> App {
        client_in(
            map,
            &std::env::var("BUDGET_WEATHER").unwrap_or("clearing".into()),
        )
    }

    fn client_in(map: &str, weather: &str) -> App {
        client_threads(map, weather, None)
    }

    /// `client_in` on a fixed number of task-pool threads (one: every
    /// system's time is its own work, and the frame's is the total).
    pub(crate) fn client_threads(map: &str, weather: &str, threads: Option<usize>) -> App {
        client_with(map, weather, None, threads)
    }

    /// A client under this weather, with this much snow lying (`--snow`).
    fn client_with(map: &str, weather: &str, snow: Option<f32>, threads: Option<usize>) -> App {
        client_flash(map, weather, snow, false, threads)
    }

    /// `client_with`, with a storm's strike held lit (every frame a flash
    /// frame: the sun's cascades on, the bolt drawn).
    fn client_flash(
        map: &str,
        weather: &str,
        snow: Option<f32>,
        flash: bool,
        threads: Option<usize>,
    ) -> App {
        use bevy::render::RenderPlugin;
        use bevy::render::settings::WgpuSettings;
        let dir = std::env::temp_dir().join(format!("extraction-budget-{map}"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(bevy::window::WindowPlugin {
                    primary_window: None,
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: WgpuSettings {
                        backends: None,
                        ..default()
                    }
                    .into(),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: crate::asset_root_path().to_string_lossy().into_owned(),
                    ..default()
                })
                .set(match threads {
                    Some(n) => bevy::app::TaskPoolPlugin {
                        task_pool_options: bevy::app::TaskPoolOptions::with_num_threads(n),
                    },
                    None => crate::task_pools(),
                })
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::log::LogPlugin>(),
        );
        // BC formats, as the game's GPUs have: the baked textures load as
        // they ship (`render::textures`), and never reach a GPU here.
        app.insert_resource(bevy::image::CompressedImageFormatSupport(
            bevy::image::CompressedImageFormats::BC,
        ));
        app.insert_resource(crate::LaunchOptions {
            flat: true,
            map: map.into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            data_dir: Some(dir),
            weather: Some(weather.into()),
            snow,
            storm_flash: flash,
            camera: Some((Vec3::new(0.0, 0.0, 0.0), 0.0, 0.0)),
            ..default()
        })
        .insert_resource(Time::<Fixed>::from_hz(90.0))
        .init_resource::<Triangles>()
        // Without a renderer nothing drains it, but despawning a camera
        // (the impostors' captures) needs it there.
        .add_plugins(bevy::render::sync_world::SyncWorldPlugin)
        .add_plugins(avian3d::prelude::PhysicsPlugins::default())
        .add_plugins((crate::GamePlugin, crate::ClientPlugin));
        app.finish();
        app.cleanup();
        // One thread for the schedules: the counts are per-frame totals,
        // and beside the rest of the suite a multi-threaded executor only
        // contends (the Verifier measured these apps twice as fast so).
        crate::single_threaded(&mut app);
        app
    }

    /// Runs until no new meshes have arrived for a while (forest chunks
    /// and grass build in the background, furniture waits for its models).
    pub(crate) fn settle(app: &mut App) {
        let (mut last, mut still) = (0, 0);
        for _ in 0..3000 {
            app.update();
            let n = app.world().resource::<Triangles>().0.len();
            still = if n == last { still + 1 } else { 0 };
            last = n;
            if still > 60 {
                return;
            }
        }
    }

    fn measure(app: &mut App) -> ViewLoad {
        app.world_mut()
            .run_system_once(
                |cams: Query<(Entity, &VisibleEntities), With<crate::render::FreeCamera>>,
                 counter: LoadCounter| {
                    let (cam, seen) = cams.single().expect("the free camera");
                    counter.measure(cam, seen)
                },
            )
            .unwrap()
    }

    /// The view's load once its counts have held still for three checks
    /// ten frames apart (at most 600 frames).
    fn settled(app: &mut App) -> ViewLoad {
        // Within half a percent (and two draws) three checks running: some
        // views never hold exactly still (grass patches swapping at their
        // edge as it sways, a chunk's streaming on its timer), and waited
        // out the 600-frame cap for nothing.
        let near = |a: usize, b: usize| a.abs_diff(b) <= (a.max(b) / 200).max(2);
        let mut last = (0, 0);
        let mut still = 0;
        for _ in 0..60 {
            for _ in 0..10 {
                app.update();
            }
            let load = measure(app);
            let now = (load.draws, load.tris);
            still = if near(now.0, last.0) && near(now.1 / 1000, last.1 / 1000) {
                still + 1
            } else {
                0
            };
            last = now;
            if still >= 3 {
                return load;
            }
        }
        println!("    (not still after 600 frames: {last:?})");
        measure(app)
    }

    /// Every lookbook view of `map`, seen through the headset's eye.
    fn loads(map: &str) -> Vec<(String, ViewLoad, super::super::overdraw::Overdraw)> {
        loads_with(client(map), map)
    }

    fn loads_with(
        mut app: App,
        map: &str,
    ) -> Vec<(String, ViewLoad, super::super::overdraw::Overdraw)> {
        settle(&mut app);
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let file = crate::asset_root_path()
            .join("..")
            .join(format!("scripts/lookbook/{map}.views"));
        let views = crate::tour::scripted_views(&zone, &file);
        assert!(!views.is_empty(), "no views in {}", file.display());
        let mut out = Vec::new();
        for v in views {
            {
                let w = app.world_mut();
                let mut q = w.query_filtered::<(&mut Transform, &mut Projection), With<crate::render::FreeCamera>>();
                let (mut t, mut p) = q.single_mut(w).expect("the free camera");
                *t = Transform::from_translation(Vec3::from_array(v.at))
                    .looking_at(Vec3::from_array(v.look), Vec3::Y);
                if let Projection::Perspective(p) = &mut *p {
                    p.fov = EYE_FOV_DEG.to_radians();
                    p.aspect_ratio = 1.0;
                }
            }
            // Detail near the new spot (grass, near trees) streams in:
            // count once the view has stopped changing, so the count
            // doesn't depend on how far the streaming had got (Marrow
            // Creek's aerial view read 1193-1201 draws at a fixed 30
            // frames, 1195 every time settled).
            let load = settled(&mut app);
            let cam = {
                let w = app.world_mut();
                w.query_filtered::<Entity, With<crate::render::FreeCamera>>()
                    .single(w)
                    .unwrap()
            };
            let od = super::super::overdraw::measure(app.world_mut(), cam);
            out.push((v.file.trim_end_matches(".png").to_string(), load, od));
        }
        out
    }

    /// Whether the game's own textures are here (`scripts/fetch_assets.py`).
    /// Without them the trees are an untextured fallback (no cards, no
    /// impostors): a different load, which has to fit the budget too
    /// (a playtest has run from a bare checkout). The gauges say which
    /// they measured.
    fn fetched_foliage() -> bool {
        let root = crate::asset_root_path().join("textures/foliage");
        root.join("spruce_branch/albedo.ktx2").exists()
            && root.join("birch_leaves/albedo.ktx2").exists()
    }

    fn holds(map: &str) {
        holds_with(map, None);
    }

    /// `holds`, under a given weather (and snow), instead of the clearing.
    fn holds_with(map: &str, weather: Option<(&str, Option<f32>)>) {
        // Counts: draws, triangles, overdraw.
        let _counting = counted();
        // Two different scenes: the forest with its fetched textures (cards
        // and impostors, what ships) or without (a lighter fallback). Each
        // is held to the budget; every message says which was measured.
        let path = if fetched_foliage() {
            "the fetched foliage (the shipped scene)"
        } else {
            "the FALLBACK foliage (no textures/foliage: scripts/fetch_assets.py \
             not run), not the shipped scene"
        };
        println!("{map}: measuring {path}");
        let loads = match weather {
            Some((w, snow)) => {
                println!("{map}: in {w}, snow {snow:?}");
                // A storm is measured on its flash frames, its dearest.
                let flash = w == "storm";
                loads_with(client_flash(map, w, snow, flash, None), map)
            }
            None => loads(map),
        };
        let mut over = Vec::new();
        for (view, load, od) in &loads {
            println!("{map} {view}: {}", load.summary());
            println!("    {}", od.summary());
            for (what, l) in od.by.iter().take(5) {
                println!("    layers {what}: {l:.2}");
            }
            for (what, c, t) in load.by.iter().take(4) {
                println!("    drawn {what}: {c} meshes, {:.0}k", *t as f64 / 1000.0);
            }
            let mut most = load.by.clone();
            most.sort_by(|a, b| b.1.cmp(&a.1));
            for (what, c, t) in most.iter().take(5) {
                println!("    many {what}: {c} meshes, {:.0}k", *t as f64 / 1000.0);
            }
            // `BUDGET_LIST=1`: every kind drawn, most first (to find what
            // a merge added, when it's spread over the long tail).
            if std::env::var("BUDGET_LIST").is_ok_and(|v| v != "0") {
                for (what, c, t) in &most {
                    println!("    all {what}: {c} meshes, {:.0}k", *t as f64 / 1000.0);
                }
            }
            for (what, c, t) in load.shadow_by.iter().take(4) {
                println!("    shadow {what}: {c} meshes, {:.0}k", *t as f64 / 1000.0);
            }
            let mut most = load.shadow_by.clone();
            most.sort_by(|a, b| b.1.cmp(&a.1));
            for (what, c, t) in most.iter().take(5) {
                println!(
                    "    many shadow {what}: {c} meshes, {:.0}k",
                    *t as f64 / 1000.0
                );
            }
            let mut o = load.over(&BUDGET);
            if od.mean > BUDGET.overdraw_mean {
                o.push(format!(
                    "overdraw mean {:.2} > {}",
                    od.mean, BUDGET.overdraw_mean
                ));
            }
            if od.p95 > BUDGET.overdraw_p95 {
                o.push(format!("overdraw p95 {} > {}", od.p95, BUDGET.overdraw_p95));
            }
            if !o.is_empty() {
                over.push(format!("{view}: {}", o.join(", ")));
            }
        }
        // A view that counted nothing would pass any budget. (A floor, not
        // a measure: the Training Yard's emptiest views draw 48-90 meshes
        // by path and schedule, so it sits well under those.)
        assert!(
            loads
                .iter()
                .all(|(_, l, _)| l.draws > 20 && l.tris > 10_000),
            "{map}, {path}: a view counted (almost) nothing: {:?}",
            loads
                .iter()
                .map(|(v, l, _)| (v, l.draws, l.tris))
                .collect::<Vec<_>>()
        );
        assert!(
            over.is_empty(),
            "{map} over the frame budget, measuring {path}:\n{}",
            over.join("\n")
        );
    }

    /// What the unnamed meshes in a view are: `BUDGET_MAP=.. BUDGET_VIEW=..
    /// cargo test --lib budget::tests::unnamed -- --ignored --nocapture`.
    #[test]
    #[ignore = "diagnosis"]
    fn unnamed() {
        let map = std::env::var("BUDGET_MAP").unwrap_or("marrow_creek".into());
        let only = std::env::var("BUDGET_VIEW").unwrap_or("m02".into());
        let mut app = client(&map);
        settle(&mut app);
        let zone = app.world().resource::<crate::world::Zone>().clone();
        let file = crate::asset_root_path()
            .join("..")
            .join(format!("scripts/lookbook/{map}.views"));
        let v = crate::tour::scripted_views(&zone, &file)
            .into_iter()
            .find(|v| v.file.contains(&only))
            .unwrap();
        {
            let w = app.world_mut();
            let mut q = w.query_filtered::<(&mut Transform, &mut Projection), With<crate::render::FreeCamera>>();
            let (mut t, mut p) = q.single_mut(w).unwrap();
            *t = Transform::from_translation(Vec3::from_array(v.at))
                .looking_at(Vec3::from_array(v.look), Vec3::Y);
            if let Projection::Perspective(p) = &mut *p {
                p.fov = EYE_FOV_DEG.to_radians();
                p.aspect_ratio = 1.0;
            }
        }
        for _ in 0..30 {
            app.update();
        }
        let w = app.world_mut();
        let seen: Vec<Entity> = w
            .query_filtered::<&VisibleEntities, With<crate::render::FreeCamera>>()
            .single(w)
            .unwrap()
            .get(std::any::TypeId::of::<Mesh3d>())
            .to_vec();
        let mut kinds: HashMap<String, usize> = HashMap::new();
        for &e in &seen {
            let r = w.entity(e);
            let k = if r.contains::<crate::world::trees::ImpostorForest>() {
                "forest: impostors"
            } else if r.contains::<crate::world::trees::FarForest>() {
                "forest: far LOD"
            } else {
                continue;
            };
            *kinds.entry(k.to_string()).or_default() += 1;
        }
        for e in seen {
            let mut root = e;
            let mut named = false;
            loop {
                if w.get::<Name>(root).is_some() {
                    named = true;
                }
                match w.get::<ChildOf>(root) {
                    Some(p) => root = p.parent(),
                    None => break,
                }
            }
            if named {
                continue;
            }
            let r = w.entity(root);
            let comps = if r.contains::<crate::world::trees::Tree>() {
                "tree".to_string()
            } else if r.contains::<crate::items::Item>() {
                "item".to_string()
            } else if r.contains::<avian3d::prelude::RigidBody>() {
                "body".to_string()
            } else if let Some(m) = r.get::<Mesh3d>() {
                let at = r.get::<GlobalTransform>().unwrap().translation();
                let ext = r
                    .get::<bevy::camera::primitives::Aabb>()
                    .map_or(Vec3::ZERO, |b| Vec3::from(b.half_extents) * 2.0);
                let mat = r
                    .get::<MeshMaterial3d<StandardMaterial>>()
                    .and_then(|h| w.resource::<Assets<StandardMaterial>>().get(&h.0))
                    .map(|m| format!("{:?} {:?}", m.base_color, m.alpha_mode));
                let tris = w.resource::<Triangles>().0.get(&m.0.id()).copied();
                format!(
                    "mesh {:?} {tris:?} tris size {ext:.3} at {at:.0} mat {mat:?} range {}",
                    m.0.id(),
                    r.contains::<bevy::camera::visibility::VisibilityRange>()
                )
            } else {
                let at = r
                    .get::<GlobalTransform>()
                    .map(|g| g.translation())
                    .unwrap_or_default();
                format!(
                    "other {} components swapin {} kids {} e.g. {at:.0}",
                    r.archetype().component_count(),
                    r.contains::<crate::render::world_props::SwapIn>(),
                    r.get::<Children>().map_or(0, |c| c.len())
                )
            };
            let comps = vec![comps];
            let key = comps.join(",");
            let key = match key.find(" at ") {
                Some(i) if kinds.keys().any(|k| k.starts_with(&key[..i])) => kinds
                    .keys()
                    .find(|k| k.starts_with(&key[..i]))
                    .unwrap()
                    .clone(),
                _ => key,
            };
            *kinds.entry(key).or_default() += 1;
        }
        let mut kinds: Vec<_> = kinds.into_iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in kinds.iter().take(15) {
            println!("{n:5} {k}");
        }
    }

    /// A raid's busy moment in `map`, in the whole client with no GPU, one
    /// fixed tick a frame: the player beside an anomaly in a downpour,
    /// ten robots (six drones, four walkers) hunting them and shooting
    /// (impacts, casings, wrecks as they fall). Returns each schedule's ms a
    /// frame (mean, 1%) over the measured frames.
    fn busy_moment(map: &str) -> Vec<(&'static str, f64, f64)> {
        busy_moment_with(map, || {})
    }

    thread_local! {
        /// The reference's ms, frame by frame, over the last busy moment.
        static REFERENCE: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// The busy moment's scene, warmed up: `map` in a downpour on one
    /// thread at 90 Hz, the player beside the anomaly nearest the middle
    /// with two guns, ten robots on them (`perf::fight`).
    /// The game's own executor again (`client_flash` gives the count
    /// gauges one thread): the timing gauges measure the frame as it runs,
    /// schedule shares and all (PostUpdate's culling and transforms side
    /// by side read 79% of a one-thread frame).
    pub(crate) fn as_the_game_runs(app: &mut App) {
        let mut schedules = app.world_mut().resource_mut::<Schedules>();
        for (_, schedule) in schedules.iter_mut() {
            schedule.set_executor(bevy::ecs::schedule::MultiThreadedExecutor::new());
        }
    }

    pub(crate) fn busy_scene(map: &str) -> App {
        let mut app = client_threads(map, "downpour", Some(1));
        as_the_game_runs(&mut app);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / 90.0),
        ));
        settle(&mut app);
        let zone = app.world().resource::<crate::world::Zone>().clone();
        // Beside the anomaly nearest the map's middle.
        let near = {
            let w = app.world_mut();
            w.query_filtered::<&GlobalTransform, With<crate::anomaly::Anomaly>>()
                .iter(w)
                .map(|g| g.translation())
                .min_by(|a, b| a.length().total_cmp(&b.length()))
                .unwrap_or(Vec3::ZERO)
        };
        let at = zone.on_ground(near.x + 9.0, near.z);
        {
            let w = app.world_mut();
            let player = w
                .query_filtered::<Entity, With<crate::rig::LocalPlayer>>()
                .single(w)
                .unwrap();
            w.get_mut::<Transform>(player).unwrap().translation = at;
            let mut q = w.query_filtered::<&mut Transform, With<crate::render::FreeCamera>>();
            *q.single_mut(w).unwrap() = Transform::from_translation(at + Vec3::Y * 1.7)
                .looking_at(near + Vec3::Y * 1.0, Vec3::Y);
        }
        for _ in 0..30 {
            app.update();
        }
        // Two guns in hand, ten robots on the player (`perf::fight`).
        crate::perf::fight::arm(&mut app);
        crate::perf::fight::hunt(&mut app);
        // Warm up (assets for the robots, first shots).
        for _ in 0..90 {
            crate::perf::fight::keep_alive(&mut app);
            app.update();
        }
        app
    }

    /// `busy_moment`, calling `measuring` as the measured frames begin.
    fn busy_moment_with(map: &str, measuring: impl Fn()) -> Vec<(&'static str, f64, f64)> {
        let mut app = busy_scene(map);
        let keep_alive = crate::perf::fight::keep_alive;
        let skip = app
            .world()
            .resource::<super::super::cpu::ScheduleTimes>()
            .frames
            .len();
        measuring();
        REFERENCE.with(|r| r.borrow_mut().clear());
        for _ in 0..450 {
            keep_alive(&mut app);
            app.update();
            let ms = crate::perf::fight::reference_ms();
            REFERENCE.with(|r| r.borrow_mut().push(ms));
        }
        if std::env::var_os("BUDGET_CENSUS").is_some() {
            census(&mut app);
        }
        app.world()
            .resource::<super::super::cpu::ScheduleTimes>()
            .stats(skip)
    }

    /// What the per-frame mesh systems iterate: mesh entities, those
    /// without bounds, and the mesh and material assets modified in a
    /// frame (`BUDGET_CENSUS=1`).
    fn census(app: &mut App) {
        use bevy::camera::primitives::Aabb;
        let w = app.world_mut();
        let meshes = w.query_filtered::<(), With<Mesh3d>>().iter(w).count();
        let unbounded: Vec<Entity> = w
            .query_filtered::<Entity, (With<Mesh3d>, Without<Aabb>)>()
            .iter(w)
            .collect();
        println!(
            "census: {meshes} mesh entities, {} without bounds",
            unbounded.len()
        );
        let mut why: HashMap<String, usize> = HashMap::new();
        for e in unbounded.iter().take(5000) {
            let r = w.entity(*e);
            let m = r.get::<Mesh3d>().unwrap();
            let state = match w.resource::<Assets<Mesh>>().get(&m.0) {
                None => "mesh asset gone".to_string(),
                Some(mesh) => match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                    None => "no positions".to_string(),
                    Some(p) if p.len() == 0 => "empty".to_string(),
                    Some(_) => "has positions".to_string(),
                },
            };
            let mut at = *e;
            let mut name = "(unnamed)".to_string();
            for _ in 0..8 {
                if let Some(n) = w.get::<Name>(at) {
                    name = n.as_str().split(' ').next().unwrap_or("").to_string();
                    break;
                }
                match w.get::<ChildOf>(at) {
                    Some(p) => at = p.parent(),
                    None => break,
                }
            }
            *why.entry(format!("{state}: {name}")).or_default() += 1;
        }
        let mut why: Vec<_> = why.into_iter().collect();
        why.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, n) in why.iter().take(12) {
            println!("  unbounded {n:6} {k}");
        }
        // What calculate_bounds works on, a frame: a probe beside it.
        {
            use bevy::asset::prelude::AssetChanged;
            use bevy::camera::visibility::{NoAutoAabb, NoFrustumCulling};
            #[derive(Resource, Default)]
            struct Probe(Vec<(usize, usize, usize, usize, String)>);
            app.init_resource::<Probe>();
            app.add_systems(
                PostUpdate,
                (|meshes: Res<Assets<Mesh>>,
                  new: Query<
                    &Mesh3d,
                    (
                        Without<Aabb>,
                        Without<NoFrustumCulling>,
                        Without<NoAutoAabb>,
                    ),
                >,
                  changed: Query<
                    (Entity, &Mesh3d, Ref<Mesh3d>),
                    (
                        Or<(AssetChanged<Mesh3d>, Changed<Mesh3d>)>,
                        Without<NoFrustumCulling>,
                        Without<NoAutoAabb>,
                    ),
                >,
                  names: Query<&Name>,
                  parents: Query<&ChildOf>,
                  mut probe: ResMut<Probe>| {
                    let verts = |m: &Mesh3d| meshes.get(m).map_or(0, |m| m.count_vertices());
                    let n: Vec<usize> = new.iter().map(verts).collect();
                    let mut who: HashMap<String, usize> = HashMap::new();
                    let mut cv = 0;
                    let mut cn = 0;
                    for (e, m, r) in &changed {
                        cn += 1;
                        cv += verts(m);
                        let mut at = e;
                        let mut name = "(unnamed)".to_string();
                        for _ in 0..8 {
                            if let Ok(nm) = names.get(at) {
                                name = nm.as_str().split(' ').next().unwrap_or("").into();
                                break;
                            }
                            match parents.get(at) {
                                Ok(p) => at = p.parent(),
                                Err(_) => break,
                            }
                        }
                        let k = format!(
                            "{name}{}",
                            if r.is_changed() {
                                " (Mesh3d changed)"
                            } else {
                                " (asset changed)"
                            }
                        );
                        *who.entry(k).or_default() += verts(m);
                    }
                    let mut who: Vec<_> = who.into_iter().collect();
                    who.sort_by(|a, b| b.1.cmp(&a.1));
                    probe.0.push((
                        n.len(),
                        n.iter().sum(),
                        cn,
                        cv,
                        format!("{:?}", &who[..who.len().min(5)]),
                    ));
                })
                .before(bevy::camera::visibility::VisibilitySystems::CalculateBounds),
            );
            for _ in 0..4 {
                app.update();
            }
            for p in app.world().resource::<Probe>().0.iter().skip(1) {
                println!(
                    "  calculate_bounds sees new {} ({} verts), changed {} ({} verts): {}",
                    p.0, p.1, p.2, p.3, p.4
                );
            }
        }
        // Asset modifications over a few frames.
        let mut mods: HashMap<String, usize> = HashMap::new();
        for _ in 0..10 {
            app.update();
            let w = app.world_mut();
            macro_rules! count {
                ($t:ty, $label:expr) => {{
                    let n = w
                        .resource_mut::<Messages<AssetEvent<$t>>>()
                        .iter_current_update_messages()
                        .filter(|e| matches!(e, AssetEvent::Modified { .. }))
                        .count();
                    *mods.entry($label.to_string()).or_default() += n;
                }};
            }
            count!(Mesh, "Mesh");
            let ids: Vec<AssetId<Mesh>> = w
                .resource_mut::<Messages<AssetEvent<Mesh>>>()
                .iter_current_update_messages()
                .filter_map(|e| match e {
                    AssetEvent::Modified { id } => Some(*id),
                    _ => None,
                })
                .collect();
            for id in ids {
                let verts = w
                    .resource::<Assets<Mesh>>()
                    .get(id)
                    .map_or(0, |m| m.count_vertices());
                let users: Vec<Entity> = w
                    .query::<(Entity, &Mesh3d)>()
                    .iter(w)
                    .filter(|(_, m)| m.0.id() == id)
                    .map(|(e, _)| e)
                    .collect();
                let name = users.first().map_or("(none)".to_string(), |e| {
                    let mut at = *e;
                    for _ in 0..8 {
                        if let Some(n) = w.get::<Name>(at) {
                            return n.as_str().to_string();
                        }
                        match w.get::<ChildOf>(at) {
                            Some(p) => at = p.parent(),
                            None => break,
                        }
                    }
                    "(unnamed)".into()
                });
                *mods
                    .entry(format!("mesh {verts} verts, {} users, {name}", users.len()))
                    .or_default() += 1;
            }
            count!(StandardMaterial, "StandardMaterial");
            // And whose: a material modified is every mesh wearing it
            // re-specialised (`check_entities_needing_specialization`).
            let ids: Vec<AssetId<StandardMaterial>> = w
                .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
                .iter_current_update_messages()
                .filter_map(|e| match e {
                    AssetEvent::Modified { id } => Some(*id),
                    _ => None,
                })
                .collect();
            for id in ids {
                let users: Vec<Entity> = w
                    .query::<(Entity, &MeshMaterial3d<StandardMaterial>)>()
                    .iter(w)
                    .filter(|(_, m)| m.0.id() == id)
                    .map(|(e, _)| e)
                    .collect();
                let name = users.first().map_or("(none)".to_string(), |e| {
                    let mut at = *e;
                    for _ in 0..8 {
                        if let Some(n) = w.get::<Name>(at) {
                            return n.as_str().to_string();
                        }
                        match w.get::<ChildOf>(at) {
                            Some(p) => at = p.parent(),
                            None => break,
                        }
                    }
                    "(unnamed)".into()
                });
                *mods
                    .entry(format!("material, {} users, {name}", users.len()))
                    .or_default() += 1;
            }
            count!(
                crate::anomaly::visuals::ParticleMaterial,
                "ParticleMaterial"
            );
            count!(crate::anomaly::visuals::XrayMaterial, "XrayMaterial");
            count!(crate::world::rain::RainMaterial, "RainMaterial");
            count!(crate::world::sky::SkyMaterial, "SkyMaterial");
            count!(crate::world::trees::TreeSwayMaterial, "TreeSwayMaterial");
            count!(crate::world::cover::SwayMaterial, "cover SwayMaterial");
            count!(
                crate::render::terrain_material::TerrainMaterial,
                "TerrainMaterial"
            );
        }
        for (k, n) in mods {
            println!("  modified a frame: {k} {:.1}", n as f32 / 10.0);
        }
    }

    /// Time a frame per system, from Bevy's per-system tracing spans:
    /// only with Bevy's `trace` feature (`cargo test --lib system_profile
    /// --features bevy/trace -- --ignored --nocapture`).
    #[derive(Default, Clone)]
    pub(crate) struct SystemTimes(
        pub(crate) std::sync::Arc<std::sync::Mutex<HashMap<String, f64>>>,
    );

    struct Named(String);
    struct Entered(std::time::Instant);

    impl<S> bevy::log::tracing_subscriber::Layer<S> for SystemTimes
    where
        S: bevy::log::tracing::Subscriber
            + for<'a> bevy::log::tracing_subscriber::registry::LookupSpan<'a>,
    {
        fn on_new_span(
            &self,
            attrs: &bevy::log::tracing::span::Attributes<'_>,
            id: &bevy::log::tracing::span::Id,
            ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            if attrs.metadata().name() != "system" {
                return;
            }
            struct V(Option<String>);
            impl bevy::log::tracing::field::Visit for V {
                fn record_debug(
                    &mut self,
                    f: &bevy::log::tracing::field::Field,
                    v: &dyn std::fmt::Debug,
                ) {
                    if f.name() == "name" {
                        self.0 = Some(format!("{v:?}").trim_matches('"').to_string());
                    }
                }
                fn record_str(&mut self, f: &bevy::log::tracing::field::Field, v: &str) {
                    if f.name() == "name" {
                        self.0 = Some(v.to_string());
                    }
                }
            }
            let mut v = V(None);
            attrs.record(&mut v);
            if let (Some(name), Some(span)) = (v.0, ctx.span(id)) {
                span.extensions_mut().insert(Named(name));
            }
        }

        fn on_enter(
            &self,
            id: &bevy::log::tracing::span::Id,
            ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            if let Some(span) = ctx.span(id) {
                if span.extensions().get::<Named>().is_some() {
                    span.extensions_mut()
                        .replace(Entered(std::time::Instant::now()));
                }
            }
        }

        fn on_exit(
            &self,
            id: &bevy::log::tracing::span::Id,
            ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            if let Some(span) = ctx.span(id) {
                let ext = span.extensions();
                if let (Some(Named(name)), Some(Entered(at))) =
                    (ext.get::<Named>(), ext.get::<Entered>())
                {
                    *self.0.lock().unwrap().entry(name.clone()).or_default() +=
                        at.elapsed().as_secs_f64() * 1000.0;
                }
            }
        }
    }

    #[test]
    #[ignore = "profile: needs --features bevy/trace"]
    fn system_profile() {
        use bevy::log::tracing_subscriber::layer::SubscriberExt;
        let times = SystemTimes::default();
        let subscriber = bevy::log::tracing_subscriber::registry().with(times.clone());
        bevy::log::tracing::subscriber::set_global_default(subscriber).unwrap();
        let map = std::env::var("BUDGET_MAP").unwrap_or("marrow_creek".into());
        // Count only the measured frames: clear when they start.
        let (stats, frames) = {
            let t = times.clone();
            let out = busy_moment_with(&map, move || t.0.lock().unwrap().clear());
            (out, 450.0)
        };
        let mut v: Vec<(String, f64)> = times.0.lock().unwrap().clone().into_iter().collect();
        if v.is_empty() {
            println!("no system spans: run with --features bevy/trace");
            return;
        }
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        println!("{map}, a busy moment: ms a frame by system (top 40)");
        for (name, ms) in v.iter().take(40) {
            println!("  {:7.3}  {name}", ms / frames);
        }
        for (name, mean, p99) in stats.iter().take(4) {
            println!("  schedule {name}: {mean:.2} (1% {p99:.2})");
        }
        // How the frame's systems spread: many near-free ones are the
        // executor's and their empty queries' cost, not work.
        let ours: Vec<f64> = v
            .iter()
            .filter(|(n, _)| n.starts_with("extraction::"))
            .map(|(_, ms)| ms / frames)
            .collect();
        for (lo, hi) in [(0.0, 0.005), (0.005, 0.02), (0.02, 0.1), (0.1, f64::MAX)] {
            let n: Vec<&f64> = ours.iter().filter(|m| **m >= lo && **m < hi).collect();
            println!(
                "  game systems {:>5.3}..{:<5} ms: {:4} of them, {:6.2} ms a frame",
                lo,
                if hi == f64::MAX {
                    "".to_string()
                } else {
                    format!("{hi}")
                },
                n.len(),
                n.iter().copied().sum::<f64>()
            );
        }
        // The near-free ones by module: where run conditions would pay.
        let mut groups: std::collections::BTreeMap<String, (usize, f64)> = default();
        for (name, ms) in &v {
            let ms = ms / frames;
            if !name.starts_with("extraction::") || ms >= 0.02 {
                continue;
            }
            let module: Vec<&str> = name.split("::").take(3).collect();
            let g = groups
                .entry(module[..module.len().min(2)].join("::"))
                .or_default();
            g.0 += 1;
            g.1 += ms;
        }
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by(|a, b| b.1.1.total_cmp(&a.1.1));
        println!("  near-free game systems by module (count, ms a frame):");
        for (module, (n, ms)) in groups.iter().take(30) {
            println!("    {n:4} {ms:6.3}  {module}");
        }
    }

    /// Mesh entities in a settled valley by what they belong to (their
    /// root's name), and how many of those roots move or can be handled:
    /// what merging static props per material could save
    /// (`cargo test --lib mesh_census -- --ignored --nocapture`,
    /// `BUDGET_MAP` for another valley).
    #[test]
    #[ignore = "census: prints, asserts nothing"]
    fn mesh_census() {
        let _counting = counted();
        let map = std::env::var("BUDGET_MAP").unwrap_or("marrow_creek".into());
        let mut app = client_threads(&map, "clearing", None);
        settle(&mut app);
        let w = app.world_mut();
        let meshes: Vec<Entity> = w.query_filtered::<Entity, With<Mesh3d>>().iter(w).collect();
        let mut by_root: HashMap<String, (usize, usize)> = HashMap::new();
        let mut roots: HashMap<String, std::collections::HashSet<Entity>> = HashMap::new();
        // An unnamed root's meshes by their own (or nearest) name.
        let mut unnamed: HashMap<String, usize> = HashMap::new();
        let mut moving = 0;
        for e in &meshes {
            let mut root = *e;
            let mut dynamic = false;
            loop {
                if w.get::<avian3d::prelude::RigidBody>(root)
                    .is_some_and(|b| !b.is_static())
                {
                    dynamic = true;
                }
                match w.get::<ChildOf>(root) {
                    Some(p) => root = p.parent(),
                    None => break,
                }
            }
            let name = w
                .get::<Name>(root)
                .map_or("(unnamed)".to_string(), |n| n.to_string());
            roots.entry(name.clone()).or_default().insert(root);
            if name == "(unnamed)" && std::env::var_os("CENSUS_COMPONENTS").is_some() {
                let names: Vec<String> = w
                    .inspect_entity(*e)
                    .map(|it| {
                        it.map(|c| {
                            c.name()
                                .to_string()
                                .rsplit("::")
                                .next()
                                .unwrap_or("")
                                .to_string()
                        })
                        .collect()
                    })
                    .unwrap_or_default();
                *unnamed.entry(names.join(",")).or_default() += 1;
                let path = w
                    .get::<Mesh3d>(*e)
                    .and_then(|m| w.resource::<AssetServer>().get_path(m.0.id()))
                    .map_or("(made in code)".to_string(), |p| {
                        p.path().to_string_lossy().to_string()
                    });
                *unnamed.entry(format!("mesh from {path}")).or_default() += 1;
            }
            if name == "(unnamed)" {
                let mut at = *e;
                let mut own = format!(
                    "(no name; {} components)",
                    w.entity(at).archetype().component_count()
                );
                for _ in 0..8 {
                    if let Some(n) = w.get::<Name>(at) {
                        own = n.to_string();
                        break;
                    }
                    match w.get::<ChildOf>(at) {
                        Some(p) => at = p.parent(),
                        None => break,
                    }
                }
                *unnamed.entry(own).or_default() += 1;
            }
            let entry = by_root.entry(name).or_default();
            entry.0 += 1;
            if dynamic {
                entry.1 += 1;
                moving += 1;
            }
        }
        let mut v: Vec<_> = by_root.into_iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        println!(
            "{map}: {} mesh entities, {moving} under a moving body; by root (meshes, moving):",
            meshes.len()
        );
        for (name, (n, m)) in v.iter().take(60) {
            println!(
                "  {n:6} {m:6}  {name} ({} roots)",
                roots.get(name).map_or(0, |r| r.len())
            );
        }
        let mut u: Vec<_> = unnamed.into_iter().collect();
        u.sort_by(|a, b| b.1.cmp(&a.1));
        println!("unnamed roots' meshes by their own name:");
        for (name, n) in u.iter().take(40) {
            println!("  {n:6}  {name}");
        }
    }

    /// What `blink::head_in_solid` costs at the busy moment's head (it
    /// runs every frame): under a millisecond. A sphere tested against the
    /// whole map's heightfield was 28 ms (blink alone was 25 ms of a 78 ms
    /// main schedule); `world::sphere_in_world` asks the heightmap for the
    /// ground instead. Prints what each query costs there, and which
    /// colliders' boxes reach the head.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_head_test_is_cheap() {
        use avian3d::prelude::*;
        let _one_at_a_time = heavy();
        let mut app = busy_scene("marrow_creek");
        app.update();
        let w = app.world_mut();
        let at = w
            .query_filtered::<&GlobalTransform, With<crate::rig::RigHead>>()
            .iter(w)
            .next()
            .map(|g| g.translation())
            .expect("a head");
        let zone = w.resource::<crate::world::Zone>().clone();
        let terrain = w
            .query_filtered::<Entity, With<crate::world::TerrainCollider>>()
            .iter(w)
            .next();
        let mut state = bevy::ecs::system::SystemState::<SpatialQuery>::new(w);
        let spatial = state.get(w).expect("the spatial query");
        let n = 50;
        let t0 = std::time::Instant::now();
        let mut hits = 0;
        for _ in 0..n {
            hits += usize::from(super::super::blink::head_in_solid(
                &spatial, &zone, terrain, at,
            ));
        }
        let full = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let filter = SpatialQueryFilter::from_mask([crate::world::Layer::World]);
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            let mut any = false;
            spatial.shape_intersections_callback(
                &Collider::sphere(0.09),
                at,
                Quat::IDENTITY,
                &filter,
                |_| {
                    any = true;
                    false
                },
            );
            hits += usize::from(any);
        }
        let early = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            let mut any = false;
            spatial.point_intersections_callback(at, &filter, |_| {
                any = true;
                false
            });
            hits += usize::from(any);
        }
        let point = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!(
            "head at {at}: head_in_solid {full:.3} ms, early-out {early:.3} ms, \
             a point {point:.3} ms ({hits} hits)"
        );
        assert!(
            full < 1.0,
            "head_in_solid {full:.3} ms a call: it runs every frame"
        );
        // Which colliders a head-sized box reaches, and the sphere test's
        // cost against each alone.
        let r = Vec3::splat(0.09);
        let t0 = std::time::Instant::now();
        for _ in 0..n {
            let _ = spatial.aabb_intersections_with_aabb(ColliderAabb::new(at, r));
        }
        let boxes = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        let t0 = std::time::Instant::now();
        let mut near_hits = 0;
        for _ in 0..n {
            if spatial
                .project_point(at, true, &filter)
                .is_some_and(|p| p.is_inside || p.point.distance(at) < 0.09)
            {
                near_hits += 1;
            }
        }
        let project = t0.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!(
            "  a box query {boxes:.3} ms; project_point {project:.3} ms ({near_hits} within 9 cm); \
             Collider::sphere's own box {:?}",
            Collider::sphere(0.09).aabb(at, Quat::IDENTITY)
        );
        let near = spatial.aabb_intersections_with_aabb(ColliderAabb::new(at, r));
        println!("  {} colliders' boxes reach the head's", near.len());
        let names: Vec<(Entity, String)> = near
            .iter()
            .map(|e| {
                let n = w
                    .get::<Name>(*e)
                    .map_or("(unnamed)".to_string(), |n| n.to_string());
                let kind = w
                    .get::<Collider>(*e)
                    .map_or("?".to_string(), |c| format!("{:?}", c.shape().shape_type()));
                (*e, format!("{n} [{kind}]"))
            })
            .collect();
        for (e, name) in names {
            let (Some(c), Some(p), Some(r)) = (
                w.get::<Collider>(e),
                w.get::<Position>(e),
                w.get::<Rotation>(e),
            ) else {
                println!("    {name}: no collider pose");
                continue;
            };
            let sphere = Collider::sphere(0.09);
            let t0 = std::time::Instant::now();
            for _ in 0..10 {
                let _ = avian3d::collision::collider::contact_query::intersection_test(
                    c,
                    p.0,
                    *r,
                    &sphere,
                    at,
                    Quat::IDENTITY,
                );
            }
            let ms = t0.elapsed().as_secs_f64() * 100.0;
            println!(
                "    {name}: {ms:.3} ms a test, layers {:?}",
                w.get::<CollisionLayers>(e).map(|l| l.memberships)
            );
        }
    }

    /// No material many meshes wear is rewritten (nearly) every frame in
    /// a busy moment: each write is every one of them re-specialised on the
    /// main thread. The robots' shared shell was, as the day's light crept
    /// (1.4 ms a frame of `check_entities_needing_specialization` in
    /// Marrow Creek's busy moment, with tracing).
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_no_shared_material_rewritten_each_frame() {
        let _counting = counted();
        let mut app = busy_scene("marrow_creek");
        let keep_alive = crate::perf::fight::keep_alive;
        let frames = 20;
        let mut writes: HashMap<AssetId<StandardMaterial>, usize> = HashMap::new();
        // What a rewritten material's write changed (its fields, as Debug
        // shows them), for the failure to say which writer it is.
        let mut before: HashMap<AssetId<StandardMaterial>, String> = HashMap::new();
        let mut what: HashMap<AssetId<StandardMaterial>, std::collections::BTreeSet<String>> =
            HashMap::new();
        for _ in 0..frames {
            keep_alive(&mut app);
            app.update();
            let w = app.world_mut();
            for (id, m) in w.resource::<Assets<StandardMaterial>>().iter() {
                let now = format!("{m:?}");
                if let Some(old) = before.get(&id).filter(|o| **o != now) {
                    let diff = now
                        .split(", ")
                        .filter(|f| !old.split(", ").any(|g| g == *f))
                        .map(|f| f.split(':').next().unwrap_or(f).trim().to_string());
                    what.entry(id).or_default().extend(diff);
                }
                before.insert(id, now);
            }
            let ids: std::collections::HashSet<AssetId<StandardMaterial>> = w
                .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
                .iter_current_update_messages()
                .filter_map(|e| match e {
                    AssetEvent::Modified { id } => Some(*id),
                    _ => None,
                })
                .collect();
            for id in ids {
                *writes.entry(id).or_default() += 1;
            }
        }
        let w = app.world_mut();
        let mut wrong = Vec::new();
        for (id, n) in writes {
            if n * 4 < frames * 3 {
                continue;
            }
            let users: Vec<Entity> = w
                .query::<(Entity, &MeshMaterial3d<StandardMaterial>)>()
                .iter(w)
                .filter(|(_, m)| m.0.id() == id)
                .map(|(e, _)| e)
                .collect();
            if users.len() < 4 {
                continue;
            }
            let name = w
                .get::<Name>(users[0])
                .or_else(|| {
                    w.get::<ChildOf>(users[0])
                        .and_then(|p| w.get::<Name>(p.parent()))
                })
                .map_or("(unnamed)".to_string(), |n| n.to_string());
            wrong.push(format!(
                "{name}'s material ({} meshes) written in {n} of {frames} frames, changing {:?}",
                users.len(),
                what.get(&id)
            ));
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// No game system takes more than `SYSTEM_SHARE` of the main
    /// schedule at any valley's busy moment, timed by Bevy's own
    /// per-system spans: a share, which survives a busy machine and the
    /// spans' own cost, where milliseconds don't. One system at 25 ms (the
    /// eyes' head test, 32% of the frame) reached a playtest build; this
    /// fails it, and names the system and its valley. Today's biggest game
    /// system is about 1% (`wildlife::visuals::build_far`). Needs the
    /// spans: `cargo test --lib --features systime -- --ignored
    /// gauge_game_systems`.
    #[cfg(feature = "systime")]
    #[test]
    #[ignore = "gauge: cargo test --lib --features systime -- --ignored gauge_game_systems"]
    fn gauge_game_systems_hold_their_share() {
        use bevy::log::tracing_subscriber::layer::SubscriberExt;
        const SYSTEM_SHARE: f64 = 0.02;
        let _one_at_a_time = heavy();
        let times = SystemTimes::default();
        let subscriber = bevy::log::tracing_subscriber::registry().with(times.clone());
        bevy::log::tracing::subscriber::set_global_default(subscriber).unwrap();
        let mut wrong = Vec::new();
        for map in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let t = times.clone();
            let stats = busy_moment_with(map, move || t.0.lock().unwrap().clear());
            let frame = stats
                .iter()
                .find(|(n, _, _)| *n == "main schedule")
                .map_or(0.0, |s| s.1);
            let mut ours: Vec<(String, f64)> = times
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|(n, _)| n.starts_with("extraction::"))
                .map(|(n, ms)| (n.clone(), ms / 450.0))
                .collect();
            ours.sort_by(|a, b| b.1.total_cmp(&a.1));
            println!("{map}: main schedule {frame:.2} ms; the heaviest game systems:");
            for (name, ms) in ours.iter().take(6) {
                println!(
                    "    {ms:7.3} ms ({:4.1}%)  {name}",
                    100.0 * ms / frame.max(1e-9)
                );
            }
            for (name, ms) in &ours {
                if *ms > frame * SYSTEM_SHARE {
                    wrong.push(format!(
                        "{map}: {name} {ms:.2} ms a frame, {:.0}% of {frame:.1}",
                        100.0 * ms / frame
                    ));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "game systems over {}% of the frame: {wrong:#?}",
            SYSTEM_SHARE * 100.0
        );
    }

    /// A busy moment in Marrow Creek holds its CPU budget: the frame in
    /// reference units (`CPU_FRAME_UNITS`), each schedule its share
    /// (`CPU_SHARE`).
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_a_busy_moment_holds_the_cpu_budget() {
        let _one_at_a_time = heavy();
        let stats = busy_moment("marrow_creek");
        let reference = REFERENCE.with(|r| {
            let mut v = r.borrow().clone();
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        });
        let frame = stats
            .iter()
            .find(|(n, _, _)| *n == "main schedule")
            .map_or(0.0, |s| s.1);
        let units = frame / reference;
        println!(
            "busy moment: {frame:.2} ms a frame, reference {reference:.3} ms: {units:.1} units"
        );
        let mut over = Vec::new();
        if units > CPU_FRAME_UNITS {
            over.push(format!(
                "the frame: {units:.1} reference units > {CPU_FRAME_UNITS}"
            ));
        }
        for (name, mean, p99) in &stats {
            let share = mean / frame.max(1e-9);
            println!(
                "    {name:>16} {mean:7.2} ms (1% {p99:7.2}), {:4.0}%",
                share * 100.0
            );
            if let Some((_, limit)) = CPU_SHARE.iter().find(|(n, _)| n == name) {
                if share > *limit {
                    over.push(format!(
                        "{name}: {:.0}% of the frame > {:.0}%",
                        share * 100.0,
                        limit * 100.0
                    ));
                }
            }
        }
        assert!(
            over.is_empty(),
            "a busy raid moment is over its CPU budget (`cargo test --lib system_profile \
             --features \"bevy/trace bevy/debug\" -- --ignored --nocapture` names the \
             systems):\n{}",
            over.join("\n")
        );
    }

    #[test]
    #[ignore = "timing: cargo test --lib busy -- --ignored --nocapture"]
    fn busy_moments() {
        for map in [
            "marrow_creek",
            "kettle_pass",
            "cinder_works",
            "sallow_reservoir",
            "rime_fell",
        ] {
            let stats = busy_moment(map);
            println!("{map}, a busy moment (ms a frame, mean / 1%):");
            for (name, mean, p99) in stats {
                println!("    {name:>16} {mean:7.2} {p99:7.2}");
            }
        }
    }

    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_marrow_creek_holds_the_budget() {
        holds("marrow_creek");
    }

    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_kettle_pass_holds_the_budget() {
        holds("kettle_pass");
    }

    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_cinder_works_holds_the_budget() {
        holds("cinder_works");
    }

    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_sallow_reservoir_holds_the_budget() {
        holds("sallow_reservoir");
    }

    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_rime_fell_holds_the_budget() {
        holds("rime_fell");
    }

    #[test]
    fn training_yard_holds_the_budget() {
        holds("training_yard");
    }

    /// A thunderstorm, measured on a flash frame (its dearest: the sun's
    /// cascades on for it, the bolt, the rain sheets).
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_marrow_creek_holds_the_budget_in_a_storm() {
        holds_with("marrow_creek", Some(("storm", None)));
    }

    /// The first snow, lying: flakes falling, the snow on every surface.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_kettle_pass_holds_the_budget_in_snow() {
        holds_with("kettle_pass", Some(("drizzle", Some(1.0))));
    }
}

/// `EXTRACTION_COUNTS=1`: every ten seconds, what the world holds (all
/// entities, lights, meshes, the names that grew most since last time).
/// For something that piles up while you stand still.
fn log_counts(
    time: Res<Time<Real>>,
    mut last: Local<(f32, std::collections::HashMap<String, usize>)>,
    all: Query<Option<&Name>>,
    points: Query<(), With<PointLight>>,
    spots: Query<(), With<SpotLight>>,
    meshes: Query<&ViewVisibility, With<Mesh3d>>,
) {
    let now = time.elapsed_secs();
    if now - last.0 < 10.0 {
        return;
    }
    last.0 = now;
    let mut names: std::collections::HashMap<String, usize> = default();
    let mut n = 0;
    for name in &all {
        n += 1;
        *names
            .entry(name.map_or("(unnamed)".into(), |n| n.as_str().to_owned()))
            .or_default() += 1;
    }
    let shown = meshes.iter().filter(|v| v.get()).count();
    let mut grew: Vec<(String, i64)> = names
        .iter()
        .map(|(k, v)| (k.clone(), *v as i64 - *last.1.get(k).unwrap_or(&0) as i64))
        .filter(|(_, d)| *d != 0)
        .collect();
    grew.sort_by_key(|(_, d)| -d.abs());
    grew.truncate(8);
    info!(
        "counts: {n} entities, {} point lights, {} spot lights, {} meshes ({shown} visible); grew {grew:?}",
        points.iter().count(),
        spots.iter().count(),
        meshes.iter().count(),
    );
    last.1 = names;
}

/// A line a frame, `frame cpu_ms gpu_ms eye_scale` (the eyes' width as a
/// share of the runtime's recommended, `render::hold_rate`; -1 with no
/// headset), after `# map <valley>`, with
/// `# world in` once `timeline::WorldReady` (the frames before it are the
/// load's). Every real session writes `<data dir>/logs/frames-<when>-
/// <valley>.txt` (the newest ten kept); `EXTRACTION_FRAMELOG=<path>` writes
/// there instead.
/// cpu_ms is the frame's real time (in a headset, what the player feels
/// when the CPU is the slow side); gpu_ms is the newest timed frame's
/// top-level render passes summed (a few frames late, as timestamps read
/// back), or -1 when none came back this frame. `scripts/frames.py`
/// gives the worst frame and the 99.9th percentile of each, so a single
/// long frame isn't lost in an average (`docs/PERFORMANCE.md`, "Hitches").
#[derive(Resource)]
struct FrameLog {
    out: Option<std::io::BufWriter<std::fs::File>>,
    frame: u64,
    seen: Option<std::time::Instant>,
    world_in: bool,
}

impl FrameLog {
    /// This session's frame log in the logs folder, keeping the newest ten.
    fn in_logs(map: &str) -> Option<std::path::PathBuf> {
        let dir = crate::persist::data_dir().join("logs");
        std::fs::create_dir_all(&dir).ok()?;
        let mut old: Vec<_> = std::fs::read_dir(&dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("frames-"))
            })
            .collect();
        old.sort();
        while old.len() >= 10 {
            let _ = std::fs::remove_file(old.remove(0));
        }
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Some(dir.join(format!("frames-{when}-{map}.txt")))
    }

    fn open(path: std::path::PathBuf, map: &str) -> Self {
        use std::io::Write;
        let mut out = std::fs::File::create(&path)
            .map_err(|e| warn!("frame log {}: {e}", path.display()))
            .ok()
            .map(std::io::BufWriter::new);
        if let Some(out) = &mut out {
            let _ = writeln!(out, "# map {map}");
        }
        Self {
            out,
            frame: 0,
            seen: None,
            world_in: false,
        }
    }
}

/// The GPU's milliseconds for the newest frame whose timestamps came back
/// since `seen` (top-level render passes summed, every view's), if any did.
///
/// A frame's spans all land at one instant, a measurement each: the two
/// eyes' and the mirror's `main_opaque_pass_3d` are three measurements of
/// one diagnostic. Its newest measurement alone was one view's, and the
/// headset's governor saw a third of the frame (the playtest's 3080: "gpu
/// 4-8 ms" of frames that ran at half rate).
pub(crate) fn newest_gpu_ms(
    store: &bevy::diagnostic::DiagnosticsStore,
    seen: &mut Option<std::time::Instant>,
) -> Option<f64> {
    let passes = || {
        store.iter().filter(|d| {
            d.path()
                .as_str()
                .strip_prefix("render/")
                .and_then(|p| p.strip_suffix("/elapsed_gpu"))
                // Top-level passes only (nested ones are inside them).
                .is_some_and(|pass| !pass.contains('/'))
        })
    };
    let newest = passes()
        .filter_map(|d| d.measurement().map(|m| m.time))
        .max()?;
    if seen.is_some_and(|s| newest <= s) {
        return None;
    }
    *seen = Some(newest);
    Some(
        passes()
            .flat_map(|d| d.measurements())
            .filter(|m| m.time == newest)
            .map(|m| m.value)
            .sum(),
    )
}

/// Both eyes' passes (and a mirror's) are the frame's GPU time, not the
/// last view's alone.
#[cfg(test)]
#[test]
fn every_views_passes_are_the_frames_gpu_time() {
    use bevy::diagnostic::{Diagnostic, DiagnosticMeasurement, DiagnosticPath, DiagnosticsStore};
    let mut store = DiagnosticsStore::default();
    let opaque = DiagnosticPath::new("render/main_opaque_pass_3d/elapsed_gpu");
    let nested = DiagnosticPath::new("render/main_opaque_pass_3d/inner/elapsed_gpu");
    store.add(Diagnostic::new(opaque.clone()));
    store.add(Diagnostic::new(nested.clone()));
    let at = std::time::Instant::now();
    let add = |store: &mut DiagnosticsStore, p: &DiagnosticPath, time, value| {
        store
            .get_mut(p)
            .unwrap()
            .add_measurement(DiagnosticMeasurement { time, value });
    };
    // A frame before, already seen.
    add(&mut store, &opaque, at, 1.0);
    let mut seen = None;
    assert_eq!(newest_gpu_ms(&store, &mut seen), Some(1.0));
    let next = at + std::time::Duration::from_millis(11);
    // Left eye, right eye, mirror.
    for v in [4.0, 4.5, 1.5] {
        add(&mut store, &opaque, next, v);
    }
    add(&mut store, &nested, next, 2.0);
    assert_eq!(newest_gpu_ms(&store, &mut seen), Some(10.0));
    assert_eq!(newest_gpu_ms(&store, &mut seen), None, "nothing new");
}

fn log_each_frame(
    time: Res<Time<Real>>,
    store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
    mut log: ResMut<FrameLog>,
    world: crate::render::timeline::WorldReady,
    eyes: Option<Res<crate::render::hold_rate::EyeRect>>,
) {
    use std::io::Write;
    let cpu = time.delta_secs_f64() * 1000.0;
    let mut seen = log.seen;
    let gpu = store
        .as_deref()
        .and_then(|s| newest_gpu_ms(s, &mut seen))
        .unwrap_or(-1.0);
    log.seen = seen;
    // The eyes' width as a share of the runtime's recommended (-1: no
    // headset).
    let scale = eyes.map_or(-1.0, |e| e.scale);
    log.frame += 1;
    let frame = log.frame;
    let world_in = !log.world_in && world.ready();
    log.world_in |= world_in;
    if let Some(out) = &mut log.out {
        if world_in {
            let _ = writeln!(out, "# world in");
        }
        let _ = writeln!(out, "{frame} {cpu:.3} {gpu:.3} {scale:.3}");
        if frame % 90 == 0 {
            let _ = out.flush();
        }
    }
}
