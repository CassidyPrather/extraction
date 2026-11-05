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
//!   wrecked, every robot's tell, then the next cycle at night in a storm
//!   with snow), each logged as it goes, for the log above to time
//!   (`scripts/pipelines.sh`).
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
            app.init_resource::<Shelf>()
                .init_resource::<Warmed>()
                .add_systems(
                    Last,
                    (
                        record_layouts,
                        proxy_eye,
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
                            shelve::<crate::world::halos::HaloMaterial>,
                            shelve::<crate::world::beams::BeamMaterial>,
                            shelve::<crate::world::storm::SheetMaterial>,
                            shelve::<crate::world::storm::BoltMaterial>,
                            shelve::<crate::anomaly::visuals::MarkMaterial>,
                        ),
                        shelve_unworn,
                        shelve_models,
                        shelve_end,
                    )
                        .chain(),
                );
        }
        if std::env::var_os("EXTRACTION_PIPELINES").is_some() {
            app.add_systems(Last, (log_first_drawn, log_key_changes));
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

/// With `EXTRACTION_PIPELINES=1`, once the shelf is done: each mesh that
/// appears, by name (or its parent's), its material and its vertex
/// layout, so a pipeline queued at first sight (the line just after) can
/// be traced to what needed it. Only one line per distinct kind.
#[allow(clippy::type_complexity)]
fn log_first_drawn(
    shelf: Option<Res<Shelf>>,
    added: Query<
        (
            Entity,
            &Mesh3d,
            Option<&Name>,
            Option<&ChildOf>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Has<bevy::light::NotShadowCaster>,
            Ref<Mesh3d>,
        ),
        Or<(Added<Mesh3d>, Changed<MeshMaterial3d<StandardMaterial>>)>,
    >,
    names: Query<&Name>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut seen: Local<std::collections::HashSet<String>>,
) {
    if shelf.is_none_or(|s| s.frame <= s.warm_for) {
        return;
    }
    for (e, mesh, name, parent, mat, no_shadow, fresh) in &added {
        // A material swapped on a mesh already drawn (a fade, a wreck's
        // scorch) is a new key for it as much as a new mesh is.
        let how = if fresh.is_added() {
            "first drawn"
        } else {
            "material swapped"
        };
        let who = name
            .map(|n| n.to_string())
            .or_else(|| {
                parent
                    .and_then(|p| names.get(p.parent()).ok())
                    .map(|n| format!("{n}>"))
            })
            .unwrap_or_else(|| format!("{e}"));
        // (A mesh whose data went to the GPU and was dropped here has
        // none to read: `attributes` panics on it.)
        let layout = meshes
            .get(&mesh.0)
            .and_then(|m| m.try_attributes().ok())
            .map_or("(on the GPU only)".to_string(), |a| {
                a.map(|(a, _)| a.name.trim_start_matches("Vertex_").to_string())
                    .collect::<Vec<_>>()
                    .join("+")
            });
        let what = mat
            .and_then(|m| materials.get(&m.0))
            .map_or("another material".to_string(), |m| {
                format!("{:?}{}", m.alpha_mode, if m.unlit { " unlit" } else { "" })
            });
        let kind = format!(
            "{what} on {layout}{}",
            if no_shadow { "" } else { ", casts" }
        );
        let short: String = who.split(['(', ' ']).next().unwrap_or("").into();
        if seen.insert(format!("{how}|{short}|{kind}")) {
            info!("{how} after the shelf: {who}: {kind}");
        }
    }
}

/// With `EXTRACTION_PIPELINES=1`, once the shelf is done: a material
/// changed in place so its pipeline key changed (its alpha mode, depth
/// bias, a texture added), named by what wears it. Neither a new mesh nor
/// a swapped material, so `log_first_drawn` can't see it.
fn log_key_changes(
    shelf: Option<Res<Shelf>>,
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
    worn: Query<(
        &MeshMaterial3d<StandardMaterial>,
        Option<&Name>,
        Option<&ChildOf>,
    )>,
    names: Query<&Name>,
    mut keys: Local<std::collections::HashMap<AssetId<StandardMaterial>, u64>>,
) {
    let key = |m: &StandardMaterial| {
        hash_of((
            m.bind_group_data(),
            format!("{:?}", m.alpha_mode()),
            m.depth_bias().to_bits(),
        ))
    };
    let done = shelf.is_some_and(|s| s.frame > s.warm_for);
    for ev in events.read() {
        let (AssetEvent::Modified { id } | AssetEvent::Added { id }) = ev else {
            continue;
        };
        let Some(m) = materials.get(*id) else {
            continue;
        };
        let k = key(m);
        let before = keys.insert(*id, k);
        if done && before.is_some_and(|b| b != k) {
            let who: Vec<String> = worn
                .iter()
                .filter(|(w, ..)| w.0.id() == *id)
                .take(3)
                .map(|(_, n, p)| {
                    n.map(|n| n.to_string())
                        .or_else(|| {
                            p.and_then(|p| names.get(p.parent()).ok())
                                .map(|n| format!("{n}>"))
                        })
                        .unwrap_or_else(|| "?".into())
                })
                .collect();
            info!(
                "material key changed after the shelf: now {:?}, depth bias {}, worn by {:?}",
                m.alpha_mode, m.depth_bias, who
            );
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
                // A depth-only pipeline (a shadow's, a prepass's) has no
                // fragment stage: its vertex defs name the layout.
                let mut defs: Vec<String> = d
                    .fragment
                    .as_ref()
                    .map_or(&d.vertex.shader_defs, |f| &f.shader_defs)
                    .iter()
                    .map(|s| format!("{s:?}"))
                    .collect();
                defs.sort();
                // The vertex layout too (stride and formats): the same
                // shader defs serve meshes of different layouts, and each
                // is a pipeline of its own.
                let buffers: Vec<String> = d
                    .vertex
                    .buffers
                    .iter()
                    .map(|b| {
                        let f: Vec<String> = b
                            .attributes
                            .iter()
                            .map(|a| format!("{:?}", a.format))
                            .collect();
                        format!("{}B {}", b.array_stride, f.join("+"))
                    })
                    .collect();
                format!(
                    "{} | {} | [{}] | {}",
                    d.label.as_deref().unwrap_or("-"),
                    d.fragment
                        .as_ref()
                        .map_or(shader(&d.vertex.shader), |f| shader(&f.shader)),
                    buffers.join("; "),
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
    // The flat free camera, or in a headset an eye (never an image
    // camera's), so the headset's own pipelines are the ones measured.
    let free = world
        .query_filtered::<&GlobalTransform, With<super::FreeCamera>>()
        .iter(world)
        .next()
        .map(|g| g.compute_transform());
    let Some(cam) = free.or_else(|| {
        world
            .query_filtered::<&GlobalTransform, super::EyeCamera>()
            .iter(world)
            .next()
            .map(|g| g.compute_transform())
    }) else {
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
        s if s == 5 + kinds.len() as u32 => {
            // A sentry in view (they're placed, not brought out): its
            // chirp's halo is the next step's.
            let zone = world.resource::<crate::world::Zone>().clone();
            let at = ahead(9.0) + flat.cross(Vec3::Y) * 4.0;
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut c = Commands::new(&mut queue, world);
            let face = (-flat).z.atan2(-flat.x).to_degrees();
            crate::robots::sentry::spawn_sentry(&mut c, &zone, at.xz(), face, 90.0, at.xz());
            queue.apply(world);
            "a sentry placed".into()
        }
        s if s == 6 + kinds.len() as u32 => {
            robots_tell(world);
            "every robot's tell".into()
        }
        s if s == 7 + kinds.len() as u32 => {
            // The next cycle, as after the Surge in the same session (a
            // valley change relaunches, a new cycle doesn't): at night, in
            // a storm, snowing, all of which the shelf at load hadn't seen.
            {
                let mut o = world.resource_mut::<crate::launch::LaunchOptions>();
                o.hour = Some("night".into());
                o.weather = Some("storm".into());
                o.snow = Some(1.0);
            }
            world.resource_mut::<crate::raid::Raid>().cycle += 1;
            "the next cycle: night, a storm, snow".into()
        }
        _ => {
            if step == 8 + kinds.len() as u32 {
                info!("first use: done at frame {frame}");
            }
            return;
        }
    };
    info!("first use: {name} at frame {frame}");
}

/// Every robot near enough to be drawn goes into its tell, as in a
/// fight's first second: a drone's lock and a walker's aim and a sorter's
/// windup (their lens flares), a sentry's chirp, and the melee machines'
/// leads (a crawler charging, a hound coiled, a heavy's arm raised), whose
/// lens glows are made the first time they're needed.
fn robots_tell(world: &mut World) {
    use crate::robots::{drone, heavy, hound_mind, indoor, sentry, sorter, walker};
    let at = |world: &World, e: Entity| {
        world
            .get::<GlobalTransform>(e)
            .map_or(Vec3::ZERO, |g| g.translation())
    };
    let of = |world: &mut World, f: fn(&mut World) -> Vec<Entity>| f(world);
    let drones = of(world, |w| {
        w.query_filtered::<Entity, With<drone::Drone>>()
            .iter(w)
            .collect()
    });
    for d in drones {
        let a = at(world, d);
        world.write_message(drone::DroneLock { drone: d, at: a });
    }
    let walkers = of(world, |w| {
        w.query_filtered::<Entity, With<walker::Walker>>()
            .iter(w)
            .collect()
    });
    for e in walkers {
        let a = at(world, e);
        world.write_message(walker::WalkerAim { walker: e, at: a });
    }
    let sorters = of(world, |w| {
        w.query_filtered::<Entity, With<sorter::Sorter>>()
            .iter(w)
            .collect()
    });
    for e in sorters {
        let a = at(world, e);
        world.write_message(sorter::SorterWindup { sorter: e, at: a });
    }
    let sentries = of(world, |w| {
        w.query_filtered::<Entity, With<sentry::Sentry>>()
            .iter(w)
            .collect()
    });
    for e in sentries {
        let a = at(world, e);
        world.write_message(sentry::SentryEvent {
            sentry: e,
            at: a,
            what: sentry::SentryAct::Chirp,
        });
    }
    for mut c in world.query::<&mut indoor::Crawler>().iter_mut(world) {
        c.charge = indoor::ZAP_CHARGE * 0.5;
    }
    for mut h in world.query::<&mut hound_mind::HoundMind>().iter_mut(world) {
        h.crouch = hound_mind::CROUCH_TIME * 0.5;
    }
    for mut a in world.query::<&mut heavy::Arm>().iter_mut(world) {
        a.state = heavy::ArmState::Raise;
        a.t = 0.0;
    }
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
    /// For each layout, a mesh of it that draws nothing: see [`point_twin`].
    twins: std::collections::HashMap<u64, Handle<Mesh>>,
    /// A robot primitive's weathered twin (`robots::weathering` adds vertex
    /// colours to every robot mesh as it first appears, a layout the model
    /// as loaded doesn't have): its point twin and that layout.
    weathered: std::collections::HashMap<AssetId<Mesh>, Option<(Handle<Mesh>, u64)>>,
    /// Every alpha mode in every cull mode, as plain `StandardMaterial`s:
    /// shelved unworn, they warm the keys an effect made mid-raid on the
    /// standard layout can have (a glow made at a robot's first tell, an
    /// anomaly's flash), which no material at load had.
    catalogue: Vec<Handle<StandardMaterial>>,
    shelved: usize,
    /// What was shelved, by material type.
    by_type: std::collections::BTreeMap<String, usize>,
    /// One anomaly of each kind, far under the world, for the first
    /// restock: a valley's cycle may hold none of a kind the Surge can
    /// open, and its looks are only built for an anomaly.
    decoys: Vec<Entity>,
    /// The frame of the restock made once the world had settled (forest
    /// and props in), on this camera.
    settled_at: Option<u32>,
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
            twins: default(),
            weathered: default(),
            catalogue: Vec::new(),
            shelved: 0,
            by_type: default(),
            decoys: Vec::new(),
            settled_at: None,
        }
    }
}

/// Where the decoys are: under the world, out of every reach.
const DECOYS_AT: Vec3 = Vec3::new(0.0, -5000.0, 0.0);

impl Shelf {
    /// Its time's up: stocked its frames, and out of them.
    fn done(&self) -> bool {
        self.frame > self.warm_for + KEEP
    }

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

/// A mesh with `mesh`'s layout (its attributes, topology and indexing)
/// whose one triangle is its first vertex three times over: whatever a
/// vertex shader makes of it, the three corners land on one point and no
/// fragment is drawn, but the pipeline is built all the same. The shelf
/// draws these, not the things themselves: a shader that places its
/// vertices in the world itself (rain, the storm's sheets and bolts)
/// ignores the shelf's tiny scale, and its copy drew full size round the
/// eye on every restock.
fn point_twin(mesh: &Mesh) -> Option<Mesh> {
    use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues as V};
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let mut twin = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    for (attr, values) in mesh.try_attributes().ok()? {
        macro_rules! thrice {
            ($($v:ident),*) => {
                match values {
                    $(V::$v(a) => V::$v(vec![*a.first()?; 3]),)*
                }
            };
        }
        let first = thrice!(
            Uint8,
            Uint8x2,
            Uint8x4,
            Sint8,
            Sint8x2,
            Sint8x4,
            Unorm8,
            Unorm8x2,
            Unorm8x4,
            Snorm8,
            Snorm8x2,
            Snorm8x4,
            Uint16,
            Uint16x2,
            Uint16x4,
            Sint16,
            Sint16x2,
            Sint16x4,
            Unorm16,
            Unorm16x2,
            Unorm16x4,
            Snorm16,
            Snorm16x2,
            Snorm16x4,
            Float16,
            Float16x2,
            Float16x4,
            Float32,
            Float32x2,
            Float32x3,
            Float32x4,
            Uint32,
            Uint32x2,
            Uint32x3,
            Uint32x4,
            Sint32,
            Sint32x2,
            Sint32x3,
            Sint32x4,
            Float64,
            Float64x2,
            Float64x3,
            Float64x4,
            Unorm8x4Bgra,
            Unorm10_10_10_2
        );
        twin.insert_attribute(attr.clone(), first);
    }
    match mesh.try_indices_option().ok()? {
        Some(Indices::U16(_)) => twin.insert_indices(Indices::U16(vec![0, 1, 2])),
        Some(Indices::U32(_)) => twin.insert_indices(Indices::U32(vec![0, 1, 2])),
        None => {}
    }
    Some(twin)
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
/// data. Every mesh not yet seen, each frame of the warm-up, and those
/// changed: a mesh added after the frame's asset events (in `PostUpdate`
/// past `AssetEventSystems`, as a resting gun's merged one can be) has its
/// event a frame late, by when a render-world-only mesh's data is gone,
/// and the shelf drew the real mesh in its place.
fn record_layouts(
    mut shelf: ResMut<Shelf>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if shelf.frame > shelf.warm_for {
        events.clear();
        return;
    }
    let changed: Vec<AssetId<Mesh>> = events
        .read()
        .filter_map(|ev| match ev {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let new: Vec<AssetId<Mesh>> = meshes
        .ids()
        .filter(|id| !shelf.layouts.contains_key(id))
        .chain(changed)
        .collect();
    for id in new {
        let Some(mesh) = meshes.get(id) else {
            continue;
        };
        let Some(l) = layout_of(mesh) else {
            continue;
        };
        shelf.layouts.insert(id, l);
        if !shelf.twins.contains_key(&l) {
            if let Some(twin) = point_twin(mesh) {
                let twin = meshes.add(twin);
                shelf.twins.insert(l, twin);
            }
        }
    }
}

/// Whether the shelf has shelved the settled world on the camera it's on
/// (the proxy eye, with a headset coming) and kept it out long enough for
/// its pipelines to be queued: `timeline::WorldReady` waits on it, and on
/// those pipelines being built.
#[derive(Resource, Default)]
pub struct Warmed(pub bool);

/// In a headset game, for as long as the shelf is stocked: an eye like
/// the headset's (its custom projection, look and samples), drawing into a
/// small image under the world, for the shelf to hang from. The shelf is
/// never on a view the player sees: on the playtest's eye it restocked
/// every 45 frames for 900, and the lamp and trees at Marrow Creek's
/// spawn flickered "on a steady pulse". A pipeline is built per kind of
/// view, and the eyes' custom projection is a kind of its own: warmed on
/// the flat camera, every pipeline was built again as the session began,
/// 58 of them, in the player's first seconds in the headset.
#[derive(Component)]
pub struct ProxyEye;

fn proxy_eye(
    mut commands: Commands,
    gate: Option<Res<crate::rig::xr::SessionGate>>,
    shelf: Res<Shelf>,
    proxies: Query<Entity, With<ProxyEye>>,
    weather: Option<Res<crate::world::sky::Weather>>,
    comfort: Option<Res<crate::rig::comfort::Comfort>>,
    mut images: ResMut<Assets<Image>>,
) {
    // For as long as the shelf's stocked: it's the shelf's camera in a
    // headset game, the eyes and mirror the player sees never are.
    let want = gate.is_some() && !shelf.done();
    if !want {
        for e in &proxies {
            commands.entity(e).try_despawn();
        }
        return;
    }
    if !proxies.is_empty() {
        return;
    }
    let Some(weather) = weather else {
        return;
    };
    let image = images.add(Image::new_target_texture(
        64,
        64,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let samples = comfort.map_or(Msaa::Sample4, |c| c.msaa_setting());
    let proxy = commands.spawn((
        Name::new("Proxy eye"),
        ProxyEye,
        Camera3d::default(),
        Camera {
            order: -20,
            ..default()
        },
        bevy::camera::RenderTarget::Image(image.into()),
        Projection::custom(bevy_mod_xr::camera::XrProjection {
            near: super::NEAR_CLIP,
            ..default()
        }),
        super::camera_look(&weather),
        Transform::from_translation(DECOYS_AT + Vec3::Y * 100.0)
            .looking_at(DECOYS_AT + Vec3::new(0.0, 0.0, 1.0), Vec3::Y),
    ));
    // Over the look's own samples: the eyes' (`Comfort::msaa`).
    let id = proxy.id();
    commands.entity(id).insert(samples);
}

fn shelve_start(
    mut shelf: ResMut<Shelf>,
    mut commands: Commands,
    gate: Option<Res<crate::rig::xr::SessionGate>>,
    shown: Option<Res<crate::rig::loading::LoadingView>>,
    proxies: Query<Entity, With<ProxyEye>>,
    // The flat view's (a free camera, or the player's own head's).
    flat: Query<
        Entity,
        (
            super::EyeCamera,
            Without<ProxyEye>,
            Without<bevy_mod_xr::camera::XrCamera>,
        ),
    >,
    cameras: Query<(), With<Camera>>,
    forest: Option<Res<crate::world::trees::ForestBuilt>>,
    props: Query<(), With<crate::world::buildings::PropsPending>>,
    mut warmed: ResMut<Warmed>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // In a headset game the proxy eye, from the start and to the end: the
    // shelf never goes on a view the player sees (the eyes, the mirror).
    // Flat, the flat camera. The shelf stays on the camera it took (there
    // can be two, in no set order: starting over each time the query named
    // the other one never finished, and left decoys behind each time).
    if shelf.camera.is_some_and(|c| !cameras.contains(c)) {
        shelf.camera = None;
    }
    if let Some(proxy) = proxies.iter().next().filter(|p| shelf.camera != Some(*p)) {
        for (e, _) in shelf.on.drain(..) {
            commands.entity(e).try_despawn();
        }
        let (layouts, twins, decoys) = (
            std::mem::take(&mut shelf.layouts),
            std::mem::take(&mut shelf.twins),
            std::mem::take(&mut shelf.decoys),
        );
        *shelf = Shelf {
            camera: Some(proxy),
            layouts,
            twins,
            decoys,
            ..default()
        };
    } else if shelf.camera.is_none() && gate.is_none() {
        shelf.camera = flat.iter().min();
    }
    // In a headset game the shelf's put away as the world shows: its
    // restocks touch what every view shares (the visibility ranges' buffer
    // among it, which the lamp's halo and the trees' fades read), and on
    // the playtest those two flickered on its 45-frame pulse.
    if gate.is_some()
        && shown.is_some_and(|v| v.lifted_at.is_some())
        && shelf.warm_for > shelf.frame
    {
        info!(
            "warm-up: shelf put away as the world shows ({} frames, {} kinds)",
            shelf.frame,
            shelf.kinds.len()
        );
        shelf.warm_for = shelf.frame;
    }
    shelf.stocking = false;
    // The world settled: the forest built, the buildings dressed (the
    // impostors' pictures are views of their own, not the world's).
    let settled = forest.is_some() && props.is_empty();
    let unwatched = shelf.camera.is_none();
    if unwatched || shelf.frame > shelf.warm_for + KEEP {
        // Its time is up, or the world's in and there's no eye to warm
        // for: nothing to wait on. (The shelf once looked only for a free
        // camera, and flat play without one waited on it for ever.)
        if (!unwatched || settled) && !warmed.0 {
            warmed.0 = true;
        }
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
        let cube = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        shelf.cube = Some(meshes.add(point_twin(&cube).unwrap_or(cube)));
    }
    // The decoys come in now, are dressed next frame, are shelved at the
    // second restock and go.
    if now == 1 {
        commands.queue(|world: &mut World| {
            let mut decoys: Vec<Entity> = extraction_core::anomaly::AnomalyKind::ALL
                .into_iter()
                .map(|kind| {
                    let (lo, hi) = kind.radius_range();
                    crate::anomaly::spawn_anomaly(world, kind, DECOYS_AT, (lo + hi) * 0.5, None)
                })
                .collect();
            // A light's beam: spotlights (lamps at night, a torch) may not
            // be lit yet, and a cycle turned to night in the same session
            // built the beam's pipeline at first sight.
            decoys.push(crate::world::beams::decoy(world, DECOYS_AT));
            world.resource_mut::<Shelf>().decoys = decoys;
        });
    }
    if now == RESTOCK + 2 {
        for e in std::mem::take(&mut shelf.decoys) {
            commands.entity(e).try_despawn();
        }
    }
    shelf.stocking = now <= shelf.warm_for && now % RESTOCK == 1;
    // Once the world has settled: a restock now (what streamed in since
    // the last waits no longer), and warm once it's been out its frames.
    if settled && shelf.settled_at.is_none() {
        shelf.settled_at = Some(now);
        shelf.stocking = true;
    }
    let warm = shelf.settled_at.is_some_and(|at| now >= at + KEEP);
    if warmed.0 != warm {
        warmed.0 = warm;
    }
    // Every restock shelves everything again: a material isn't drawn
    // (nor its pipeline built) until its textures are in, which can be
    // after the restock that first saw it.
    if shelf.stocking {
        shelf.seen.clear();
    }
}

/// One entry for every distinct (material key, mesh layout, range,
/// shadow) among the things in the world wearing an `M`. Hidden ones
/// count: not yet seen is what the shelf is for, and the machines' seeds
/// (a plough's bank, a minelayer's mine and its lamp) are hidden things
/// worn only so they're shelved here. Don't filter on visibility.
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
        // Its layout's twin, which draws nothing, where there is one.
        let shown = shelf.twins.get(&layout).cloned().unwrap_or(mesh.0.clone());
        if let Some(e) = shelf.put(&mut commands, shown, ranged, !no_shadow) {
            commands
                .entity(e)
                .insert((MeshMaterial3d(mat.0.clone()), ShelfMark));
        }
    }
}

/// The models asked for at start (`first_sight::PreloadPlugin`) that
/// nothing in the world wears yet: an item's, a gun's, a robot's first
/// appearance mid-raid would build its pipelines on the frame it's first
/// drawn. Each primitive goes on the shelf with the material it will
/// wear (`ModelMaterials`, as items take theirs), ranged (items fade) and
/// not.
#[allow(clippy::too_many_arguments)]
fn shelve_models(
    mut shelf: ResMut<Shelf>,
    mut commands: Commands,
    models: Res<crate::models::Models>,
    server: Res<AssetServer>,
    gltfs: Res<Assets<bevy::gltf::Gltf>>,
    gnodes: Res<Assets<bevy::gltf::GltfNode>>,
    gmeshes: Res<Assets<bevy::gltf::GltfMesh>>,
    mut model_mats: ResMut<crate::models::ModelMaterials>,
    mut lib: ResMut<super::materials::MaterialLibrary>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if !shelf.stocking {
        return;
    }
    fn walk<'a>(
        nodes: &'a [crate::models::ModelNode],
        out: &mut Vec<&'a crate::models::ModelPrimitive>,
    ) {
        for n in nodes {
            out.extend(n.primitives.iter());
            walk(&n.children, out);
        }
    }
    let ty = std::any::type_name::<StandardMaterial>();
    for handle in models.handles() {
        let Some(gltf) = gltfs.get(handle) else {
            continue;
        };
        let robot = server
            .get_path(handle)
            .is_some_and(|p| p.path().starts_with("models/robots"));
        let nodes = crate::models::model_nodes(gltf, &gnodes, &gmeshes);
        let mut prims = Vec::new();
        walk(&nodes, &mut prims);
        for prim in prims {
            let mat = model_mats.get(
                prim.material.as_deref().unwrap_or("Blued"),
                &mut lib,
                &server,
                &mut materials,
            );
            let Some(m) = materials.get(&mat) else {
                continue;
            };
            let layout = shelf
                .layouts
                .get(&prim.mesh.id())
                .copied()
                .unwrap_or_else(|| hash_of(prim.mesh.id()));
            let as_loaded = (
                shelf
                    .twins
                    .get(&layout)
                    .cloned()
                    .unwrap_or(prim.mesh.clone()),
                layout,
            );
            // A robot's mesh as it's drawn: weathered, with vertex colours.
            let weathered = if robot {
                let id = prim.mesh.id();
                if !shelf.weathered.contains_key(&id) {
                    let built = meshes.get(id).and_then(|m| {
                        let mut w = m.clone();
                        let n = w.count_vertices();
                        w.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; n]);
                        let l = layout_of(&w)?;
                        Some((point_twin(&w).unwrap_or(w), l))
                    });
                    let made = built.map(|(twin, l)| (meshes.add(twin), l));
                    shelf.weathered.insert(id, made);
                }
                shelf.weathered.get(&id).cloned().flatten()
            } else {
                None
            };
            for (shown, layout) in std::iter::once(as_loaded).chain(weathered) {
                for ranged in [true, false] {
                    let key = hash_of((
                        ty,
                        m.bind_group_data(),
                        format!("{:?}", m.alpha_mode()),
                        m.depth_bias().to_bits(),
                        layout,
                        ranged,
                        false,
                    ));
                    if !shelf.seen.insert(key) {
                        continue;
                    }
                    if shelf.kinds.insert(key) {
                        *shelf.by_type.entry("model".into()).or_default() += 1;
                    }
                    if let Some(e) = shelf.put(&mut commands, shown.clone(), ranged, true) {
                        commands
                            .entity(e)
                            .insert((MeshMaterial3d(mat.clone()), ShelfMark));
                    }
                }
            }
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
    if shelf.catalogue.is_empty() {
        use bevy::render::render_resource::Face;
        for alpha_mode in [
            AlphaMode::Opaque,
            AlphaMode::Mask(0.5),
            AlphaMode::Blend,
            AlphaMode::Premultiplied,
            AlphaMode::Add,
        ] {
            for cull_mode in [Some(Face::Back), None, Some(Face::Front)] {
                let h = materials.add(StandardMaterial {
                    alpha_mode,
                    cull_mode,
                    double_sided: cull_mode.is_none(),
                    ..default()
                });
                shelf.catalogue.push(h);
            }
        }
    }
    let ids: Vec<AssetId<StandardMaterial>> = materials.ids().collect();
    for id in ids {
        let Some(m) = materials.get(id) else {
            continue;
        };
        let (data, alpha, bias) = (
            m.bind_group_data(),
            format!("{:?}", m.alpha_mode()),
            m.depth_bias().to_bits(),
        );
        let Some(handle) = materials.get_strong_handle(id) else {
            continue;
        };
        // Casting a shadow and not, faded by a visibility range and not: a
        // material first worn mid-raid by something that casts (a gun part,
        // an item brought out) or that fades with distance (a robot's lamp
        // halo) built that variant at first sight when only the plain one
        // was warmed here.
        for (ranged, casts) in [(false, false), (false, true), (true, false), (true, true)] {
            let key = hash_of((ty, data, alpha.clone(), bias, layout, ranged, !casts));
            if !shelf.seen.insert(key) {
                continue;
            }
            if shelf.kinds.insert(key) {
                *shelf.by_type.entry("unworn".into()).or_default() += 1;
            }
            if let Some(e) = shelf.put(&mut commands, cube.clone(), ranged, casts) {
                commands
                    .entity(e)
                    .insert((MeshMaterial3d(handle.clone()), ShelfMark));
            }
        }
    }
}

fn shelve_end(shelf: Res<Shelf>, mut logged: Local<bool>) {
    if shelf.frame > shelf.warm_for && !*logged {
        *logged = true;
        super::timeline::mark("warm-up shelf done");
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

    /// Every material type the game registers is on the shelf: one left
    /// off would have every pipeline of its built at first use, in the
    /// raid. (Read from the source: a `MaterialPlugin::<M>` anywhere means
    /// `shelve::<..M>` here.)
    #[test]
    fn every_material_is_shelved() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let here = std::fs::read_to_string(root.join("render/warmup.rs")).unwrap();
        let mut missing = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if path.ends_with("render/warmup.rs") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for part in text.split("MaterialPlugin::<").skip(1) {
                    let ty = part.split('>').next().unwrap_or("").trim();
                    let last = ty.rsplit("::").next().unwrap_or(ty);
                    if !last.is_empty() && !here.contains(&format!("{last}>,")) {
                        missing.push(format!("{} ({})", ty, path.display()));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "material types not on the warm-up shelf (add a shelve::<..>): {missing:?}"
        );
    }

    /// The shelf stocks itself in the first seconds, its decoy anomalies
    /// go after the second restock, and nothing of it is left after.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_the_shelf_stocks_and_clears() {
        // Counts: what's shelved, the decoys.
        let _counting = super::super::budget::tests::counted();
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
        let mut real = std::collections::BTreeSet::new();
        for _ in 0..(WARM_FOR + KEEP + 20) {
            app.update();
            most = most.max(shelved(&mut app));
            saw_decoys = saw_decoys.max(decoys(&mut app));
            // Every entry wears a twin (a mesh that draws nothing).
            let twins: std::collections::HashSet<AssetId<Mesh>> = app
                .world()
                .resource::<Shelf>()
                .twins
                .values()
                .map(|h| h.id())
                .chain(app.world().resource::<Shelf>().cube.iter().map(|h| h.id()))
                .collect();
            let w = app.world_mut();
            for (mesh, name) in w
                .query_filtered::<(&Mesh3d, Option<&Name>), With<ShelfMark>>()
                .iter(w)
            {
                if !twins.contains(&mesh.0.id()) {
                    real.insert(format!("{name:?} {:?}", mesh.0.id()));
                }
            }
        }
        assert!(
            real.is_empty(),
            "shelf entries drawing the real mesh: {real:?}"
        );
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
    /// A twin has its mesh's layout, and its one triangle is a point.
    #[test]
    fn a_twin_is_a_point() {
        let mut card = Mesh::from(Cuboid::new(2.0, 3.0, 4.0));
        card.insert_attribute(
            crate::world::trees::ATTRIBUTE_CARD_CENTRE,
            vec![[1.0f32, 2.0, 3.0]; card.count_vertices()],
        );
        for mesh in [
            Mesh::from(Cuboid::new(2.0, 3.0, 4.0)),
            card,
            Mesh::from(Sphere::new(1.0)),
        ] {
            let twin = point_twin(&mesh).expect("a triangle list has a twin");
            assert_eq!(layout_of(&twin), layout_of(&mesh));
            let pos = twin
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
                .as_float3()
                .unwrap();
            assert_eq!(pos.len(), 3);
            assert!(pos.iter().all(|p| *p == pos[0]));
            assert_eq!(twin.indices().map(|i| i.len()), Some(3));
        }
    }

    /// In a headset game the shelf is on the proxy eye from its first
    /// frame to its last, never on an eye the player sees (on the
    /// playtest's, it restocked every 45 frames for 900 and Marrow Creek's
    /// spawn flickered on that pulse). There are two eyes, and which one a
    /// query names first can change: it doesn't move the shelf, which
    /// still finishes and leaves no decoys (each with its lights) behind.
    #[test]
    #[ignore = "gauge: cargo test --lib -- --ignored gauge_"]
    fn gauge_two_eyes_one_shelf() {
        // Counts: the shelves, their lights.
        let _counting = super::super::budget::tests::counted();
        let mut app = super::super::budget::tests::client_threads("marrow_creek", "clearing", None);
        app.init_resource::<crate::rig::xr::SessionGate>();
        let eye = |app: &mut App, n: u32| {
            app.world_mut()
                .spawn((
                    bevy_mod_xr::camera::XrCamera(n),
                    Transform::from_xyz(0.0, 2.0, 0.0),
                ))
                .id()
        };
        let (a, b) = (eye(&mut app, 0), eye(&mut app, 1));
        let decoys = |app: &mut App| {
            let w = app.world_mut();
            w.query_filtered::<&Transform, With<crate::anomaly::Anomaly>>()
                .iter(w)
                .filter(|t| t.translation.y < DECOYS_AT.y + 1.0)
                .count()
        };
        let kinds = extraction_core::anomaly::AnomalyKind::ALL.len();
        let mut most = 0;
        for i in 0..(WARM_FOR + KEEP + 20) {
            // Every so often the first eye goes to the back of the queue.
            if i % 30 == 15 {
                let w = app.world_mut();
                let cam = w.entity_mut(a).take::<bevy_mod_xr::camera::XrCamera>();
                w.entity_mut(a).insert(cam.unwrap());
            }
            app.update();
            most = most.max(decoys(&mut app));
            let on = app.world().resource::<Shelf>().camera;
            assert!(
                on != Some(a) && on != Some(b),
                "frame {i}: the shelf on an eye"
            );
            if let Some(on) = on {
                assert!(
                    app.world().get::<ProxyEye>(on).is_some(),
                    "frame {i}: not the proxy"
                );
            }
        }
        assert_eq!(most, kinds, "one set of decoys, at most");
        assert_eq!(decoys(&mut app), 0, "the decoys gone");
        let w = app.world_mut();
        assert_eq!(
            w.query_filtered::<(), With<ShelfMark>>().iter(w).count(),
            0,
            "the shelf cleared"
        );
        let shelf = app.world().resource::<Shelf>();
        assert!(
            shelf.frame > WARM_FOR,
            "the warm-up finished ({} frames)",
            shelf.frame
        );
    }
}
