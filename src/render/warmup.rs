//! First use. Bevy builds a pipeline the first time a mesh needs one (a
//! material and mesh layout it hasn't drawn with before), on the async
//! compute pool: the render thread doesn't wait, the mesh just isn't drawn
//! until its pipeline is ready. On a headset that's a muzzle flash that
//! never shows (it lives 35 ms) or an anomaly that blooms a beat late.
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

pub struct WarmupPlugin;

impl Plugin for WarmupPlugin {
    fn build(&self, app: &mut App) {
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
