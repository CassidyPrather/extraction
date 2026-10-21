//! First use. Bevy builds a pipeline the first time a mesh needs one (a
//! material and mesh layout it hasn't drawn with before), on the async
//! compute pool: the render thread doesn't wait, the mesh just isn't drawn
//! until its pipeline is ready. On a headset that's a muzzle flash that
//! never shows (it lives 35 ms) or an anomaly that blooms a beat late.
//!
//! So for the first seconds of a session ([`WARM_FOR`] frames, while the
//! player is still in the truck) every kind of thing the world holds is
//! drawn once on a shelf in front of the eye, too small to cover a pixel:
//! one entry for each material key, mesh layout, and the flags that
//! change a pipeline (a visibility range's dither, casting a shadow),
//! plus every effect material nothing wears yet (a muzzle flash, a
//! spark). Each pipeline is queued then, not when the effect first shows.
//! `EXTRACTION_WARMUP=0` leaves the shelf out (to measure without it);
//! `EXTRACTION_WARMUP=<frames>` keeps it stocked that long.
//!
//! - `EXTRACTION_PIPELINES=1` logs every pipeline as it's queued and as
//!   it's ready (frames and ms after it was queued).
//! - `EXTRACTION_FIRSTS=<frame>` sets off the raid's effects one by one
//!   in front of the camera from that frame on (a shot, impacts on every
//!   surface, a robot struck, a grenade, the Surge's blooms, a walker
//!   wrecked), each logged as it goes, for the log above to time.
//!
//! See docs/PERFORMANCE.md, "Hitches".

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;

pub struct WarmupPlugin;

impl Plugin for WarmupPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("EXTRACTION_WARMUP").map_or(true, |v| v != "0") {
            use crate::anomaly::visuals::{HazeMaterial, ParticleMaterial, XrayMaterial};
            use crate::world::{cover, impostors, rain, shafts, sky, trees};
            app.init_resource::<Shelf>().add_systems(
                Last,
                (
                    record_layouts,
                    shelve_start,
                    (
                        shelve::<StandardMaterial>,
                        shelve::<super::terrain_material::TerrainMaterial>,
                        shelve::<super::water_material::WaterMaterial>,
                        shelve::<trees::TreeSwayMaterial>,
                        shelve::<trees::FarCrownMaterial>,
                        shelve::<impostors::ImpostorMaterial>,
                        shelve::<cover::SwayMaterial>,
                        shelve::<rain::RainMaterial>,
                        shelve::<shafts::ShaftMaterial>,
                        shelve::<sky::SkyMaterial>,
                        shelve::<HazeMaterial>,
                        shelve::<ParticleMaterial>,
                        shelve::<XrayMaterial>,
                    ),
                    shelve_unworn,
                    shelve_end,
                )
                    .chain(),
            );
        }
        if std::env::var_os("EXTRACTION_PIPELINES").is_some() {
            if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
                use bevy::render::{Render, RenderSystems};
                render.add_systems(Render, watch_pipelines.in_set(RenderSystems::Cleanup));
            }
        }
        if let Some(from) = std::env::var("EXTRACTION_FIRSTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
        {
            app.insert_resource(Firsts { from, ..default() })
                .add_systems(Update, firsts);
        }
    }
}

/// The pipelines seen so far, and those still building (id: label, the
/// frame and time it was queued).
#[derive(Default)]
struct Watch {
    frame: u64,
    seen: usize,
    building: Vec<(usize, String, u64, std::time::Instant)>,
}

fn watch_pipelines(cache: Res<bevy::render::render_resource::PipelineCache>, mut w: Local<Watch>) {
    use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
    w.frame += 1;
    let frame = w.frame;
    let pipelines: Vec<_> = cache.pipelines().collect();
    for (id, p) in pipelines.iter().enumerate().skip(w.seen) {
        let what = match &p.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(d) => {
                let shader = |h: &Handle<bevy::shader::Shader>| {
                    h.path().map_or("?".to_string(), |p| p.to_string())
                };
                let mut defs: Vec<String> = d
                    .fragment
                    .as_ref()
                    .map(|f| f.shader_defs.iter().map(|s| format!("{s:?}")).collect())
                    .unwrap_or_default();
                defs.sort();
                format!(
                    "{} | {} | {}",
                    d.label.as_deref().unwrap_or("-"),
                    d.fragment
                        .as_ref()
                        .map_or(shader(&d.vertex.shader), |f| shader(&f.shader)),
                    defs.join(" ")
                )
            }
            PipelineDescriptor::ComputePipelineDescriptor(d) => {
                format!("compute {}", d.label.as_deref().unwrap_or("-"))
            }
        };
        info!("pipeline queued #{id} at frame {frame}: {what}");
        w.building
            .push((id, what, frame, std::time::Instant::now()));
    }
    w.seen = pipelines.len();
    w.building
        .retain(|(id, what, at, t)| match &pipelines[*id].state {
            CachedPipelineState::Ok(_) => {
                info!(
                    "pipeline ready #{id} after {} frames, {:.0} ms: {}",
                    frame - at,
                    t.elapsed().as_secs_f64() * 1000.0,
                    what.split(" | ").next().unwrap_or("")
                );
                false
            }
            CachedPipelineState::Err(_) => false,
            _ => true,
        });
}

/// The scripted first uses (`EXTRACTION_FIRSTS`).
#[derive(Resource, Default)]
struct Firsts {
    from: u32,
    frame: u32,
    gun: Option<Entity>,
    grenade: Option<Entity>,
    walker: Option<Entity>,
}

/// Frames between one effect and the next.
const FIRSTS_EVERY: u32 = 90;

fn firsts(world: &mut World) {
    let (frame, from) = {
        let mut f = world.resource_mut::<Firsts>();
        f.frame += 1;
        (f.frame, f.from)
    };
    let Some(cam) = world
        .query_filtered::<&GlobalTransform, With<super::FreeCamera>>()
        .iter(world)
        .next()
        .map(|g| g.compute_transform())
    else {
        return;
    };
    let Some(zone) = world.get_resource::<crate::world::Zone>().cloned() else {
        return;
    };
    let mut flat = cam.forward().as_vec3();
    flat.y = 0.0;
    let flat = flat.normalize_or(Vec3::NEG_Z);
    let ahead = |d: f32| {
        let p = cam.translation + flat * d;
        zone.on_ground(p.x, p.z)
    };
    // The props, a second early (their own arrival isn't a first use).
    if frame + 90 == from {
        let catalog = world.resource::<crate::items::Catalog>().0.clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let (gun, grenade, walker) = {
            let mut c = Commands::new(&mut queue, world);
            let mut item = |id: &str, at: Vec3| {
                let e = crate::items::spawn_item(
                    &mut c,
                    &catalog,
                    crate::items::SpawnItem {
                        kind: catalog.kind(id).expect("in the catalogue"),
                        transform: Transform::from_translation(at).with_rotation(cam.rotation),
                        condition: 1.0,
                    },
                );
                c.entity(e).insert(avian3d::prelude::RigidBody::Static);
                e
            };
            let gun = item(
                "ak74",
                cam.translation + cam.forward() * 1.2 - Vec3::Y * 0.3,
            );
            let grenade = item("rgd5", ahead(7.0) + Vec3::Y * 0.3);
            let at = ahead(16.0);
            let walker = crate::robots::walker::spawn_walker(
                &mut c,
                &zone,
                at + flat.cross(Vec3::Y) * 5.0,
                crate::robots::Job {
                    kind: crate::robots::JobKind::Survey,
                    site: at,
                    duration: 999.0,
                    exit: at,
                },
            );
            c.entity(walker).remove::<crate::robots::Senses>();
            (gun, grenade, walker)
        };
        queue.apply(world);
        let mut f = world.resource_mut::<Firsts>();
        (f.gun, f.grenade, f.walker) = (Some(gun), Some(grenade), Some(walker));
        return;
    }
    if frame < from || (frame - from) % FIRSTS_EVERY != 0 {
        return;
    }
    let step = (frame - from) / FIRSTS_EVERY;
    let (gun, grenade, walker) = {
        let f = world.resource::<Firsts>();
        (f.gun, f.grenade, f.walker)
    };
    use crate::weapons::impacts::{Impact, ImpactKind};
    let impact = |kind, point: Vec3, part| Impact {
        kind,
        point,
        normal: Vec3::Y,
        velocity: (point - cam.translation).normalize_or(Vec3::NEG_Z) * 700.0,
        entity: None,
        decal: true,
        part,
        hurt: true,
    };
    let kinds = extraction_core::anomaly::AnomalyKind::ALL;
    let name: String = match step {
        0 => {
            if let Some(gun) = gun {
                let at = world
                    .get::<GlobalTransform>(gun)
                    .map_or(cam.translation, |g| g.translation());
                world.write_message(crate::weapons::GunEvent {
                    gun,
                    item: "ak74".into(),
                    event: extraction_core::firearm::Event::Fired { barrel: 0 },
                    at,
                });
            }
            "a shot".into()
        }
        1 => {
            for (i, kind) in [
                ImpactKind::Masonry,
                ImpactKind::Wood,
                ImpactKind::Metal,
                ImpactKind::Glass,
                ImpactKind::Soil,
                ImpactKind::Water,
            ]
            .into_iter()
            .enumerate()
            {
                let side = flat.cross(Vec3::Y) * (i as f32 - 2.5) * 0.8;
                world.write_message(impact(kind, ahead(5.0) + side, None));
            }
            "impacts on every surface".into()
        }
        2 => {
            use crate::robots::PartRole;
            for (i, part) in [
                PartRole::Core,
                PartRole::Sensor,
                PartRole::Weapon,
                PartRole::Leg(0),
                PartRole::GasCell(0),
                PartRole::Armour(0),
            ]
            .into_iter()
            .enumerate()
            {
                let side = flat.cross(Vec3::Y) * (i as f32 - 2.5) * 0.8;
                world.write_message(impact(
                    ImpactKind::Metal,
                    ahead(6.0) + side + Vec3::Y,
                    Some(part),
                ));
            }
            "a robot struck".into()
        }
        3 => {
            if let Some(mut live) =
                grenade.and_then(|g| world.get_mut::<crate::weapons::grenades::Live>(g))
            {
                live.grenade.fuse = extraction_core::grenade::Fuse::Burning(1e-4);
            }
            "a grenade".into()
        }
        s if (4..4 + kinds.len() as u32).contains(&s) => {
            let kind = kinds[(s - 4) as usize];
            bloom(world, kind, ahead(14.0));
            format!("a {kind:?} blooms")
        }
        s if s == 4 + kinds.len() as u32 => {
            if let Some(mut r) = walker.and_then(|w| world.get_mut::<crate::robots::Robot>(w)) {
                r.wrecked = true;
            }
            "a walker wrecked".into()
        }
        _ => {
            if step == 5 + kinds.len() as u32 {
                info!("first use: done at frame {frame}");
            }
            return;
        }
    };
    info!("first use: {name} at frame {frame}");
}

/// An anomaly opening in the Surge, as `anomaly::surge::surge_blooms`
/// opens one.
pub(crate) fn bloom(world: &mut World, kind: extraction_core::anomaly::AnomalyKind, at: Vec3) {
    use crate::anomaly::{AnomalyAct, AnomalyEvent, BLOOM_LIFE, spawn_anomaly};
    let (lo, hi) = kind.radius_range();
    let e = spawn_anomaly(world, kind, at, (lo + hi) * 0.5, Some(BLOOM_LIFE));
    world.entity_mut(e).insert((
        crate::anomaly::surge::SurgeBloom,
        crate::anomaly::surge::Forming(extraction_core::anomaly::SURGE_FORMING),
        crate::anomaly::detector::Touched {
            inside: Vec::new(),
            revealed: crate::anomaly::detector::REVEAL_TIME,
        },
    ));
    world.write_message(AnomalyEvent {
        anomaly: e,
        kind,
        at,
        what: AnomalyAct::Bloom,
    });
}

/// Frames from the first camera that the shelf is kept stocked.
pub const WARM_FOR: u32 = 900;
/// Frames between restocking (what's streamed in since gets its turn).
const RESTOCK: u32 = 45;
/// Frames a shelf entry stays: enough to be specialised and queued.
const KEEP: u32 = 8;

/// The warm-up's state: what has been shelved (by pipeline-relevant
/// signature), what's on the shelf now, and the camera it hangs from.
#[derive(Resource)]
struct Shelf {
    /// Frames to keep stocked (`WARM_FOR`, or `EXTRACTION_WARMUP=<n>`).
    warm_for: u32,
    frame: u32,
    camera: Option<Entity>,
    stocking: bool,
    /// Shelved this restock.
    seen: std::collections::HashSet<u64>,
    /// Shelved at all (each a pipeline's worth, more or less).
    kinds: std::collections::HashSet<u64>,
    on: Vec<(Entity, u32)>,
    layouts: std::collections::HashMap<AssetId<Mesh>, u64>,
    cube: Option<Handle<Mesh>>,
    shelved: usize,
    /// What was shelved, by material type.
    by_type: std::collections::BTreeMap<String, usize>,
    /// One anomaly of each kind, far under the world, for the first
    /// restock: a valley's cycle may hold none of a kind the Surge can
    /// open, and its looks are only built for an anomaly.
    decoys: Vec<Entity>,
}

impl Default for Shelf {
    fn default() -> Self {
        Self {
            warm_for: std::env::var("EXTRACTION_WARMUP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(WARM_FOR),
            frame: 0,
            camera: None,
            stocking: false,
            seen: default(),
            kinds: default(),
            on: Vec::new(),
            layouts: default(),
            cube: None,
            shelved: 0,
            by_type: default(),
            decoys: Vec::new(),
        }
    }
}

/// Where the decoys are: under the world, out of every reach.
const DECOYS_AT: Vec3 = Vec3::new(0.0, -5000.0, 0.0);

impl Shelf {
    /// Puts one thing on the shelf: tiny, in front of the eye, with the
    /// flags its pipeline depends on.
    fn put(
        &mut self,
        commands: &mut Commands,
        mesh: Handle<Mesh>,
        ranged: bool,
        caster: bool,
    ) -> Option<Entity> {
        let cam = self.camera?;
        let mut e = commands.spawn((
            Name::new("Warm-up shelf"),
            Mesh3d(mesh),
            Transform::from_xyz(0.0, 0.0, -0.3).with_scale(Vec3::splat(1e-6)),
            // Bounds of its own: some meshes can't be bounded from their
            // data (particles placed in the shader, or the data's gone to
            // the render world), and an unbounded mesh isn't drawn.
            bevy::camera::primitives::Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            bevy::camera::visibility::NoAutoAabb,
            ChildOf(cam),
        ));
        if ranged {
            e.insert(bevy::camera::visibility::VisibilityRange {
                start_margin: 0.0..0.0,
                end_margin: 1e5..1e5 + 1.0,
                use_aabb: false,
            });
        }
        if !caster {
            e.insert(bevy::light::NotShadowCaster);
        }
        let e = e.id();
        self.on.push((e, self.frame + KEEP));
        self.shelved += 1;
        Some(e)
    }
}

fn hash_of(v: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// What a mesh's pipeline depends on: its attributes and topology (none
/// once its data has gone to the render world).
fn layout_of(mesh: &Mesh) -> Option<u64> {
    let attrs: Vec<String> = mesh
        .try_attributes()
        .ok()?
        .map(|(a, _)| format!("{:?}", a.id))
        .collect();
    let indexed = mesh.try_indices_option().ok()?.is_some();
    Some(hash_of((
        attrs,
        format!("{:?}", mesh.primitive_topology()),
        indexed,
    )))
}

/// Each mesh's layout as it arrives, before the render world takes its
/// data.
fn record_layouts(
    mut shelf: ResMut<Shelf>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
) {
    if shelf.frame > shelf.warm_for {
        events.clear();
        return;
    }
    for ev in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = ev {
            if let Some(l) = meshes.get(*id).and_then(layout_of) {
                shelf.layouts.insert(*id, l);
            }
        }
    }
}

fn shelve_start(
    mut shelf: ResMut<Shelf>,
    mut commands: Commands,
    eyes: Query<Entity, With<bevy_mod_xr::camera::XrCamera>>,
    free: Query<Entity, With<super::FreeCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // The headset's eye once there is one (it comes with the session),
    // else the flat camera.
    if let Some(eye) = eyes.iter().next() {
        if shelf.camera != Some(eye) {
            for (e, _) in shelf.on.drain(..) {
                commands.entity(e).try_despawn();
            }
            *shelf = Shelf {
                camera: Some(eye),
                ..default()
            };
        }
    } else if shelf.camera.is_none() {
        shelf.camera = free.iter().next();
    }
    shelf.stocking = false;
    if shelf.camera.is_none() || shelf.frame > shelf.warm_for + KEEP {
        return;
    }
    shelf.frame += 1;
    let now = shelf.frame;
    shelf.on.retain(|(e, until)| {
        if *until > now {
            return true;
        }
        commands.entity(*e).try_despawn();
        false
    });
    if shelf.cube.is_none() {
        shelf.cube = Some(meshes.add(Cuboid::new(1.0, 1.0, 1.0)));
    }
    // The decoys come in now, are dressed next frame, are shelved at the
    // second restock and go.
    if now == 1 {
        commands.queue(|world: &mut World| {
            let decoys: Vec<Entity> = extraction_core::anomaly::AnomalyKind::ALL
                .into_iter()
                .map(|kind| {
                    let (lo, hi) = kind.radius_range();
                    crate::anomaly::spawn_anomaly(world, kind, DECOYS_AT, (lo + hi) * 0.5, None)
                })
                .collect();
            world.resource_mut::<Shelf>().decoys = decoys;
        });
    }
    if now == RESTOCK + 2 {
        for e in std::mem::take(&mut shelf.decoys) {
            commands.entity(e).try_despawn();
        }
    }
    shelf.stocking = now <= shelf.warm_for && now % RESTOCK == 1;
    // Every restock shelves everything again: a material isn't drawn
    // (nor its pipeline built) until its textures are in, which can be
    // after the restock that first saw it.
    if shelf.stocking {
        shelf.seen.clear();
    }
}

/// One entry for every distinct (material key, mesh layout, range,
/// shadow) among the things in the world wearing an `M`.
#[allow(clippy::type_complexity)]
fn shelve<M: Material>(
    mut shelf: ResMut<Shelf>,
    mut commands: Commands,
    worn: Query<
        (
            &Mesh3d,
            &MeshMaterial3d<M>,
            Has<bevy::camera::visibility::VisibilityRange>,
            Has<bevy::light::NotShadowCaster>,
        ),
        (
            Without<bevy::mesh::skinning::SkinnedMesh>,
            Without<ShelfMark>,
        ),
    >,
    materials: Res<Assets<M>>,
) where
    M::Data: std::hash::Hash,
{
    if !shelf.stocking {
        return;
    }
    let ty = std::any::type_name::<M>();
    for (mesh, mat, ranged, no_shadow) in &worn {
        let Some(m) = materials.get(&mat.0) else {
            continue;
        };
        // A mesh whose layout wasn't seen counts as its own.
        let layout = shelf
            .layouts
            .get(&mesh.0.id())
            .copied()
            .unwrap_or_else(|| hash_of(mesh.0.id()));
        let key = hash_of((
            ty,
            m.bind_group_data(),
            format!("{:?}", m.alpha_mode()),
            m.depth_bias().to_bits(),
            layout,
            ranged,
            no_shadow,
        ));
        if !shelf.seen.insert(key) {
            continue;
        }
        if shelf.kinds.insert(key) {
            let short = ty.rsplit("::").next().unwrap_or(ty).to_string();
            *shelf.by_type.entry(short).or_default() += 1;
        }
        if let Some(e) = shelf.put(&mut commands, mesh.0.clone(), ranged, !no_shadow) {
            commands
                .entity(e)
                .insert((MeshMaterial3d(mat.0.clone()), ShelfMark));
        }
    }
}

/// Marks a shelf entry, so restocking doesn't count it.
#[derive(Component)]
pub struct ShelfMark;

/// The effects' materials nothing wears yet (spawned only when a shot
/// lands or a gun fires): on a box, which has their meshes' layout.
fn shelve_unworn(
    mut shelf: ResMut<Shelf>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !shelf.stocking {
        return;
    }
    let Some(cube) = shelf.cube.clone() else {
        return;
    };
    let layout = layout_of(&Mesh::from(Cuboid::new(1.0, 1.0, 1.0))).unwrap_or_default();
    let ty = std::any::type_name::<StandardMaterial>();
    let ids: Vec<AssetId<StandardMaterial>> = materials.ids().collect();
    for id in ids {
        let Some(m) = materials.get(id) else {
            continue;
        };
        let key = hash_of((
            ty,
            m.bind_group_data(),
            format!("{:?}", m.alpha_mode()),
            m.depth_bias().to_bits(),
            layout,
            false,
            true,
        ));
        if !shelf.seen.insert(key) {
            continue;
        }
        let Some(handle) = materials.get_strong_handle(id) else {
            continue;
        };
        if shelf.kinds.insert(key) {
            *shelf.by_type.entry("unworn".into()).or_default() += 1;
        }
        if let Some(e) = shelf.put(&mut commands, cube.clone(), false, false) {
            commands
                .entity(e)
                .insert((MeshMaterial3d(handle), ShelfMark));
        }
    }
}

fn shelve_end(shelf: Res<Shelf>, mut logged: Local<bool>) {
    if shelf.frame > shelf.warm_for && !*logged {
        *logged = true;
        info!(
            "warm-up: {} kinds of thing shelved ({} entries) over {} frames: {:?}",
            shelf.kinds.len(),
            shelf.shelved,
            shelf.warm_for,
            shelf.by_type
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shelf stocks itself in the first seconds, its decoy anomalies
    /// go after the second restock, and nothing of it is left after.
    #[test]
    fn the_shelf_stocks_and_clears() {
        let _one_at_a_time = super::super::budget::tests::heavy();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        let decoys = |app: &mut App| {
            let w = app.world_mut();
            w.query_filtered::<&Transform, With<crate::anomaly::Anomaly>>()
                .iter(w)
                .filter(|t| t.translation.y < DECOYS_AT.y + 1.0)
                .count()
        };
        let shelved = |app: &mut App| {
            let w = app.world_mut();
            w.query_filtered::<(), With<ShelfMark>>().iter(w).count()
        };
        let mut most = 0;
        let mut saw_decoys = 0;
        for _ in 0..(WARM_FOR + KEEP + 20) {
            app.update();
            most = most.max(shelved(&mut app));
            saw_decoys = saw_decoys.max(decoys(&mut app));
        }
        let kinds = extraction_core::anomaly::AnomalyKind::ALL.len();
        assert_eq!(saw_decoys, kinds, "a decoy of each kind");
        assert_eq!(decoys(&mut app), 0, "the decoys gone");
        assert!(most > 20, "the shelf stocked ({most} at most)");
        assert_eq!(shelved(&mut app), 0, "the shelf cleared");
        let shelf = app.world().resource::<Shelf>();
        println!(
            "{} kinds shelved ({} entries): {:?}",
            shelf.kinds.len(),
            shelf.shelved,
            shelf.by_type
        );
    }
}
