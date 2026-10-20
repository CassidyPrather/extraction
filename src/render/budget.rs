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
    overdraw_mean: 10.0,
    overdraw_p95: 35,
};

/// The sun's shadow map, a side, per cascade per eye. Bevy's default is
/// 2048; at 1024 the software frame was 7-18% quicker and the shadows
/// look the same, a touch softer (which the overcast wants anyway).
pub const SHADOW_MAP: usize = 1024;

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
            (reach_doors, late_reach, range_items, shadow_reach)
                .chain()
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
        let knobs = Knobs::from_env();
        app.insert_resource(bevy::light::DirectionalLightShadowMap {
            size: knobs.shadow_map.unwrap_or(SHADOW_MAP),
        });
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

fn shadow_reach(
    mut commands: Commands,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::render::OffscreenCamera>)>,
    casters: Query<(Entity, &GlobalTransform, &ShadowReach, Has<NotShadowCaster>)>,
) {
    let eyes: Vec<Vec2> = cams.iter().map(|g| g.translation().xz()).collect();
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
    meshes: Query<'w, 's, (&'static Mesh3d, Has<NotShadowCaster>)>,
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
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The whole client with no window and no GPU (as
    /// `client_systems_initialise`): visibility, LOD ranges and the
    /// cascades' culling all run on the CPU, so a view's load can be
    /// counted without rendering it.
    fn client(map: &str) -> App {
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
                .disable::<bevy::winit::WinitPlugin>()
                .disable::<bevy::log::LogPlugin>(),
        );
        app.insert_resource(crate::LaunchOptions {
            flat: true,
            map: map.into(),
            map_explicit: true,
            new_game: true,
            robots: false,
            data_dir: Some(dir),
            weather: Some(std::env::var("BUDGET_WEATHER").unwrap_or("clearing".into())),
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
        app
    }

    /// Runs until no new meshes have arrived for a while (forest chunks
    /// and grass build in the background, furniture waits for its models).
    fn settle(app: &mut App) {
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

    /// Every lookbook view of `map`, seen through the headset's eye.
    fn loads(map: &str) -> Vec<(String, ViewLoad, super::super::overdraw::Overdraw)> {
        let mut app = client(map);
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
            // Detail near the new spot (grass, near trees) streams in.
            for _ in 0..30 {
                app.update();
            }
            let load = measure(&mut app);
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
        println!(
            "{map}: foliage {}",
            if fetched_foliage() {
                "fetched"
            } else {
                "FALLBACK (no foliage textures: scripts/fetch_assets.py not run)"
            }
        );
        let loads = loads(map);
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
        // A view that counted nothing would pass any budget.
        assert!(
            loads
                .iter()
                .all(|(_, l, _)| l.draws > 50 && l.tris > 10_000),
            "a view counted (almost) nothing: {:?}",
            loads
                .iter()
                .map(|(v, l, _)| (v, l.draws, l.tris))
                .collect::<Vec<_>>()
        );
        assert!(
            over.is_empty(),
            "{map} over the frame budget:\n{}",
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

    #[test]
    fn marrow_creek_holds_the_budget() {
        holds("marrow_creek");
    }

    #[test]
    fn kettle_pass_holds_the_budget() {
        holds("kettle_pass");
    }

    #[test]
    fn cinder_works_holds_the_budget() {
        holds("cinder_works");
    }

    #[test]
    fn training_yard_holds_the_budget() {
        holds("training_yard");
    }
}
